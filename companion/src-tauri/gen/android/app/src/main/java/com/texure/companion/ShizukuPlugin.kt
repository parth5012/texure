package com.texure.companion

import android.content.Context
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import rikka.shizuku.Shizuku
import java.lang.reflect.Method
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Result of executing an elevated shell command via Shizuku.
 */
data class ShellResult(
    val ok: Boolean,
    val exitCode: Int,
    val stdout: String,
    val stderr: String,
    val timedOut: Boolean = false,
    val error: String? = null,
    val errorHint: String? = null
)

/**
 * Native plugin managing Shizuku binder connection, permission lifecycle,
 * and remote lifecycle supervisor shell execution for Texure companion.
 */
class ShizukuPlugin(
    private val context: Context,
    private val eventSink: ((event: String, payload: Map<String, Any?>) -> Unit)? = null
) : Shizuku.OnRequestPermissionResultListener,
    Shizuku.OnBinderReceivedListener,
    Shizuku.OnBinderDeadListener {

    internal var processRunner: ((cmd: Array<String>, timeoutMs: Long) -> ShellResult)? = null

    private val mainHandler = Handler(Looper.getMainLooper())
    private val listenersRegistered = AtomicBoolean(false)

    companion object {
        const val REQUEST_CODE_PERMISSION = 1001
        const val EVENT_BINDER_STATE = "shizuku://binder-state"
        const val MAX_OUTPUT_BYTES = 256 * 1024 // 256 KiB stream capture limit
        val SHIZUKU_PACKAGES = listOf("moe.shizuku.privileged.api", "rikka.shizuku")

        private val newProcessMethod: Method? by lazy {
            try {
                val method = Shizuku::class.java.getDeclaredMethod(
                    "newProcess",
                    Array<String>::class.java,
                    Array<String>::class.java,
                    String::class.java
                )
                method.isAccessible = true
                method
            } catch (e: Throwable) {
                null
            }
        }

        /**
         * Invokes Shizuku's private `newProcess` IPC method.
         */
        fun newProcess(cmd: Array<String>, env: Array<String>? = null, dir: String? = null): Process {
            val method = newProcessMethod
                ?: throw UnsupportedOperationException("Shizuku newProcess method unavailable on this runtime")
            return method.invoke(null, cmd, env, dir) as Process
        }

        // --- Sanitization & Validation Helpers ---

        val PACKAGE_REGEX = Regex("^[a-zA-Z][a-zA-Z0-9_]*(\\.[a-zA-Z][a-zA-Z0-9_]*)+$")
        val ACTIVITY_REGEX = Regex("^[a-zA-Z][a-zA-Z0-9_]*(\\.[a-zA-Z][a-zA-Z0-9_]*)*$")
        val PERMISSION_REGEX = Regex("^[a-zA-Z0-9_.]+$")
        val KEYCODE_REGEX = Regex("^[a-zA-Z0-9_]+$")

        fun validatePackageName(packageName: String) {
            require(packageName.matches(PACKAGE_REGEX)) {
                "Invalid package name: '$packageName' contains invalid characters or format"
            }
        }

        fun validateActivityName(activityName: String, expectedPackageName: String? = null) {
            val target = if (activityName.contains("/")) {
                val parts = activityName.split("/")
                require(parts.size == 2) { "Invalid component syntax: '$activityName'" }
                validatePackageName(parts[0])
                if (expectedPackageName != null) {
                    require(parts[0] == expectedPackageName) {
                        "Component package '${parts[0]}' does not match expected package '$expectedPackageName'"
                    }
                }
                parts[1]
            } else {
                activityName
            }
            val classPart = if (target.startsWith(".")) target.substring(1) else target
            require(classPart.isNotEmpty() && classPart.matches(ACTIVITY_REGEX)) {
                "Invalid activity name: '$activityName' contains invalid characters or format"
            }
        }

        fun validatePermission(permission: String) {
            require(permission.matches(PERMISSION_REGEX)) {
                "Invalid permission name: '$permission' contains invalid characters"
            }
        }

        // --- Command Builder Helpers ---

        fun buildWakeCommand(): Array<String> = arrayOf("input", "keyevent", "KEYCODE_WAKEUP")

        fun buildDismissKeyguardCommand(): Array<String> = arrayOf("wm", "dismiss-keyguard")

        fun buildStartAppCommand(packageName: String, activityName: String? = null): Array<String> {
            validatePackageName(packageName)
            return if (!activityName.isNullOrBlank()) {
                validateActivityName(activityName, packageName)
                val component = if (activityName.contains("/")) {
                    activityName
                } else if (activityName.startsWith(".")) {
                    "$packageName/$activityName"
                } else if (activityName.startsWith("$packageName.")) {
                    "$packageName/$activityName"
                } else {
                    "$packageName/.$activityName"
                }
                arrayOf("am", "start", "-n", component)
            } else {
                arrayOf(
                    "am", "start",
                    "-a", "android.intent.action.MAIN",
                    "-c", "android.intent.category.LAUNCHER",
                    "-p", packageName
                )
            }
        }

        fun buildStopAppCommand(packageName: String): Array<String> {
            validatePackageName(packageName)
            return arrayOf("am", "force-stop", packageName)
        }

        fun buildGrantPermissionCommand(packageName: String, permission: String): Array<String> {
            validatePackageName(packageName)
            validatePermission(permission)
            // Prepend android.permission. only if permission has no namespace (does not contain '.')
            val perm = if (!permission.contains(".")) {
                "android.permission.$permission"
            } else {
                permission
            }
            return arrayOf("pm", "grant", packageName, perm)
        }

        /**
         * Builds input tap command. Assumes coordinates are non-negative and within valid screen bounds.
         */
        fun buildSystemTapCommand(x: Int, y: Int): Array<String> {
            require(x >= 0) { "x coordinate must be non-negative: $x" }
            require(y >= 0) { "y coordinate must be non-negative: $y" }
            return arrayOf("input", "tap", x.toString(), y.toString())
        }

        fun buildPressKeyCommand(keycode: Int): Array<String> {
            require(keycode >= 0) { "keycode must be non-negative: $keycode" }
            return arrayOf("input", "keyevent", keycode.toString())
        }

        /**
         * Builds input keyevent command. Validates syntax format against KEYCODE_REGEX
         * (alphanumeric and underscores) to reject shell metacharacters.
         */
        fun buildPressKeyCommand(keycodeName: String): Array<String> {
            require(keycodeName.matches(KEYCODE_REGEX)) {
                "Invalid keycode: '$keycodeName' contains invalid characters"
            }
            return arrayOf("input", "keyevent", keycodeName)
        }
    }

    init {
        registerListeners()
    }

    /**
     * Registers binder received/dead and permission result listeners with Shizuku.
     */
    @Synchronized
    fun registerListeners() {
        if (listenersRegistered.compareAndSet(false, true)) {
            try {
                Shizuku.addBinderReceivedListenerSticky(this)
                Shizuku.addBinderDeadListener(this)
                Shizuku.addRequestPermissionResultListener(this)
            } catch (e: Throwable) {
                listenersRegistered.set(false)
            }
        }
    }

    /**
     * Unregisters all Shizuku listeners.
     */
    @Synchronized
    fun unregisterListeners() {
        if (listenersRegistered.compareAndSet(true, false)) {
            runCatching { Shizuku.removeBinderReceivedListener(this) }
            runCatching { Shizuku.removeBinderDeadListener(this) }
            runCatching { Shizuku.removeRequestPermissionResultListener(this) }
        }
    }

    /**
     * Cleans up listeners and resources on destroy.
     */
    fun destroy() {
        unregisterListeners()
    }

    /**
     * Checks if Shizuku manager app is installed on the device.
     */
    fun shizuku_is_installed(): Boolean {
        val pm = context.packageManager
        return SHIZUKU_PACKAGES.any { pkg ->
            try {
                pm.getPackageInfo(pkg, 0)
                true
            } catch (e: Throwable) {
                false
            }
        }
    }

    /**
     * Checks if Shizuku daemon service is currently running and responding.
     */
    fun shizuku_is_running(): Boolean {
        return try {
            Shizuku.pingBinder()
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * Checks if the app has been granted Shizuku authorization.
     */
    fun shizuku_has_permission(): Boolean {
        if (!shizuku_is_running()) {
            return false
        }
        return try {
            Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * Requests Shizuku authorization dialog.
     */
    fun shizuku_request_permission(requestCode: Int = REQUEST_CODE_PERMISSION): Boolean {
        if (!shizuku_is_running()) {
            return false
        }
        return try {
            if (Shizuku.isPreV11()) {
                return false
            }
            if (Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) {
                emitBinderState(connected = true, permission = true)
                true
            } else {
                Shizuku.requestPermission(requestCode)
                true
            }
        } catch (e: Throwable) {
            false
        }
    }

    // --- IPC Shell Wrapper & Elevated / Supervisor Commands ---

    /**
     * Executes shell command via Shizuku with timeout and output capture.
     */
    fun executeShellCommand(cmd: Array<String>, timeoutMs: Long = 5000L): ShellResult {
        // If a test or custom runner is injected, use it with error containment
        val runner = processRunner
        if (runner != null) {
            return try {
                runner.invoke(cmd, timeoutMs)
            } catch (e: Throwable) {
                ShellResult(
                    ok = false,
                    exitCode = -1,
                    stdout = "",
                    stderr = e.message ?: e.toString(),
                    error = "RUNNER_ERROR"
                )
            }
        }

        if (!shizuku_is_running() || !shizuku_has_permission()) {
            return ShellResult(
                ok = false,
                exitCode = -1,
                stdout = "",
                stderr = "Shizuku is not running or permission is not granted",
                error = "PERMISSION_DENIED",
                errorHint = "request Shizuku permission on the device"
            )
        }

        return try {
            val process = newProcess(cmd, null, null)
            runCatching { process.outputStream.close() }

            val stdoutBuffer = StringBuilder()
            val stderrBuffer = StringBuilder()

            val stdoutThread = Thread {
                try {
                    process.inputStream.bufferedReader().useLines { lines ->
                        for (line in lines) {
                            if (Thread.currentThread().isInterrupted) break
                            synchronized(stdoutBuffer) {
                                if (stdoutBuffer.length + line.length < MAX_OUTPUT_BYTES) {
                                    if (stdoutBuffer.isNotEmpty()) stdoutBuffer.append("\n")
                                    stdoutBuffer.append(line)
                                }
                            }
                        }
                    }
                } catch (ignored: Throwable) {}
            }.apply { isDaemon = true }

            val stderrThread = Thread {
                try {
                    process.errorStream.bufferedReader().useLines { lines ->
                        for (line in lines) {
                            if (Thread.currentThread().isInterrupted) break
                            synchronized(stderrBuffer) {
                                if (stderrBuffer.length + line.length < MAX_OUTPUT_BYTES) {
                                    if (stderrBuffer.isNotEmpty()) stderrBuffer.append("\n")
                                    stderrBuffer.append(line)
                                }
                            }
                        }
                    }
                } catch (ignored: Throwable) {}
            }.apply { isDaemon = true }

            stdoutThread.start()
            stderrThread.start()

            val completed = try {
                val waitTimeoutMethod = process.javaClass.getMethod(
                    "waitForTimeout",
                    Long::class.javaPrimitiveType,
                    TimeUnit::class.java
                )
                waitTimeoutMethod.invoke(process, timeoutMs, TimeUnit.MILLISECONDS) as Boolean
            } catch (_: Throwable) {
                process.waitFor(timeoutMs, TimeUnit.MILLISECONDS)
            }

            if (!completed) {
                runCatching { process.inputStream.close() }
                runCatching { process.errorStream.close() }
                runCatching { process.destroy() }
                stdoutThread.interrupt()
                stderrThread.interrupt()
                runCatching { stdoutThread.join(500) }
                runCatching { stderrThread.join(500) }
                ShellResult(
                    ok = false,
                    exitCode = -1,
                    stdout = synchronized(stdoutBuffer) { stdoutBuffer.toString() },
                    stderr = "Command timed out after ${timeoutMs}ms",
                    timedOut = true,
                    error = "TIMEOUT",
                    errorHint = "increase timeout or verify device responsiveness"
                )
            } else {
                runCatching { stdoutThread.join(1000) }
                runCatching { stderrThread.join(1000) }
                runCatching { process.inputStream.close() }
                runCatching { process.errorStream.close() }
                val exitCode = process.exitValue()
                val isOk = (exitCode == 0)
                val err = if (!isOk) "NON_ZERO_EXIT" else null
                val hint = if (!isOk) "process exited with code $exitCode" else null
                ShellResult(
                    ok = isOk,
                    exitCode = exitCode,
                    stdout = synchronized(stdoutBuffer) { stdoutBuffer.toString() },
                    stderr = synchronized(stderrBuffer) { stderrBuffer.toString() },
                    error = err,
                    errorHint = hint
                )
            }
        } catch (e: Throwable) {
            ShellResult(
                ok = false,
                exitCode = -1,
                stdout = "",
                stderr = e.message ?: e.toString(),
                error = "EXECUTION_ERROR"
            )
        }
    }

    private inline fun executeSafely(timeoutMs: Long, commandBuilder: () -> Array<String>): ShellResult {
        return try {
            val cmd = commandBuilder()
            executeShellCommand(cmd, timeoutMs)
        } catch (e: IllegalArgumentException) {
            ShellResult(
                ok = false,
                exitCode = -1,
                stdout = "",
                stderr = e.message ?: "Invalid arguments",
                error = "INVALID_ARGUMENT",
                errorHint = "verify command parameters match regex constraints"
            )
        }
    }

    /**
     * Wakes the device screen via KEYCODE_WAKEUP.
     */
    fun shizuku_wake_screen(timeoutMs: Long = 5000L): ShellResult {
        return executeShellCommand(buildWakeCommand(), timeoutMs)
    }

    /**
     * Dismisses the lock screen / keyguard.
     */
    fun shizuku_dismiss_keyguard(timeoutMs: Long = 5000L): ShellResult {
        return executeShellCommand(buildDismissKeyguardCommand(), timeoutMs)
    }

    /**
     * Cold-starts target app component via `am start`.
     */
    fun shizuku_start_app(
        packageName: String,
        activityName: String? = null,
        timeoutMs: Long = 10000L
    ): ShellResult {
        return executeSafely(timeoutMs) { buildStartAppCommand(packageName, activityName) }
    }

    /**
     * Force-stops target app package via `am force-stop`.
     */
    fun shizuku_stop_app(packageName: String, timeoutMs: Long = 5000L): ShellResult {
        return executeSafely(timeoutMs) { buildStopAppCommand(packageName) }
    }

    /**
     * Grants elevated runtime permission via `pm grant`.
     */
    fun shizuku_grant_permission(
        packageName: String,
        permission: String,
        timeoutMs: Long = 5000L
    ): ShellResult {
        return executeSafely(timeoutMs) { buildGrantPermissionCommand(packageName, permission) }
    }

    /**
     * Simulates a system tap at screen coordinates (x, y) on system windows.
     */
    fun shizuku_system_tap(x: Int, y: Int, timeoutMs: Long = 5000L): ShellResult {
        return executeSafely(timeoutMs) { buildSystemTapCommand(x, y) }
    }

    /**
     * Injects a hardware keycode via `input keyevent`.
     */
    fun shizuku_press_key(keycode: Int, timeoutMs: Long = 5000L): ShellResult {
        return executeSafely(timeoutMs) { buildPressKeyCommand(keycode) }
    }

    /**
     * Injects a named keyevent via `input keyevent`.
     */
    fun shizuku_press_key(keycodeName: String, timeoutMs: Long = 5000L): ShellResult {
        return executeSafely(timeoutMs) { buildPressKeyCommand(keycodeName) }
    }

    // --- Shizuku Listener Callbacks ---

    override fun onBinderReceived() {
        val hasPerm = shizuku_has_permission()
        emitBinderState(connected = true, permission = hasPerm)
    }

    override fun onBinderDead() {
        emitBinderState(connected = false, permission = false)
    }

    override fun onRequestPermissionResult(requestCode: Int, grantResult: Int) {
        if (requestCode != REQUEST_CODE_PERMISSION) return
        val granted = grantResult == PackageManager.PERMISSION_GRANTED
        emitBinderState(connected = shizuku_is_running(), permission = granted)
    }

    // --- Event Emission ---

    private fun emitBinderState(connected: Boolean, permission: Boolean) {
        val isRunning = shizuku_is_running()
        val version = try {
            if (connected) Shizuku.getVersion() else 0
        } catch (e: Throwable) {
            0
        }
        val payload = mapOf(
            "connected" to connected,
            "running" to (connected && isRunning),
            "permission" to permission,
            "version" to version
        )
        if (Looper.myLooper() == Looper.getMainLooper()) {
            eventSink?.invoke(EVENT_BINDER_STATE, payload)
        } else {
            mainHandler.post {
                eventSink?.invoke(EVENT_BINDER_STATE, payload)
            }
        }
    }
}
