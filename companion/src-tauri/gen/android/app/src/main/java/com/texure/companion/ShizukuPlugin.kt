package com.texure.companion

import android.content.Context
import android.content.pm.PackageManager
import rikka.shizuku.Shizuku
import java.lang.reflect.Method
import java.util.concurrent.TimeUnit

/**
 * Result of executing an elevated shell command via Shizuku.
 */
data class ShellResult(
    val ok: Boolean,
    val exitCode: Int,
    val stdout: String,
    val stderr: String,
    val timedOut: Boolean = false,
    val error: String? = null
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

    var processRunner: ((cmd: Array<String>, timeoutMs: Long) -> ShellResult)? = null

    companion object {
        const val REQUEST_CODE_PERMISSION = 1001
        const val EVENT_BINDER_STATE = "shizuku://binder-state"
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
        val PERMISSION_REGEX = Regex("^[a-zA-Z0-9_.]+$")
        val KEYCODE_REGEX = Regex("^[a-zA-Z0-9_]+$")

        fun validatePackageName(packageName: String) {
            require(packageName.matches(PACKAGE_REGEX)) {
                "Invalid package name: '$packageName' contains invalid characters or format"
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
                val component = if (activityName.contains("/")) {
                    activityName
                } else if (activityName.startsWith(".")) {
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

    private var listenersRegistered = false

    init {
        registerListeners()
    }

    /**
     * Registers binder received/dead and permission result listeners with Shizuku.
     */
    fun registerListeners() {
        if (!listenersRegistered) {
            try {
                Shizuku.addBinderReceivedListenerSticky(this)
                Shizuku.addBinderDeadListener(this)
                Shizuku.addRequestPermissionResultListener(this)
                listenersRegistered = true
            } catch (e: Throwable) {
                // Ignore if binder registration fails
            }
        }
    }

    /**
     * Unregisters all Shizuku listeners.
     */
    fun unregisterListeners() {
        if (listenersRegistered) {
            try {
                Shizuku.removeBinderReceivedListener(this)
                Shizuku.removeBinderDeadListener(this)
                Shizuku.removeRequestPermissionResultListener(this)
            } catch (e: Throwable) {
                // Ignore during teardown
            } finally {
                listenersRegistered = false
            }
        }
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
                error = "PERMISSION_DENIED"
            )
        }

        return try {
            val process = newProcess(cmd, null, null)
            val stdoutBuffer = StringBuilder()
            val stderrBuffer = StringBuilder()

            val stdoutThread = Thread {
                try {
                    process.inputStream.bufferedReader().useLines { lines ->
                        lines.forEach { line ->
                            if (Thread.currentThread().isInterrupted) return@useLines
                            synchronized(stdoutBuffer) {
                                if (stdoutBuffer.isNotEmpty()) stdoutBuffer.append("\n")
                                stdoutBuffer.append(line)
                            }
                        }
                    }
                } catch (ignored: Throwable) {}
            }
            val stderrThread = Thread {
                try {
                    process.errorStream.bufferedReader().useLines { lines ->
                        lines.forEach { line ->
                            if (Thread.currentThread().isInterrupted) return@useLines
                            synchronized(stderrBuffer) {
                                if (stderrBuffer.isNotEmpty()) stderrBuffer.append("\n")
                                stderrBuffer.append(line)
                            }
                        }
                    }
                } catch (ignored: Throwable) {}
            }

            stdoutThread.start()
            stderrThread.start()

            val completed = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                process.waitFor(timeoutMs, TimeUnit.MILLISECONDS)
            } else {
                val start = System.currentTimeMillis()
                var done = false
                while (System.currentTimeMillis() - start < timeoutMs) {
                    try {
                        process.exitValue()
                        done = true
                        break
                    } catch (e: IllegalThreadStateException) {
                        Thread.sleep(50)
                    }
                }
                done
            }

            if (!completed) {
                process.destroy()
                stdoutThread.interrupt()
                stderrThread.interrupt()
                ShellResult(
                    ok = false,
                    exitCode = -1,
                    stdout = synchronized(stdoutBuffer) { stdoutBuffer.toString() },
                    stderr = "Command timed out after ${timeoutMs}ms",
                    timedOut = true,
                    error = "TIMEOUT"
                )
            } else {
                stdoutThread.join(500)
                stderrThread.join(500)
                val exitCode = process.exitValue()
                ShellResult(
                    ok = (exitCode == 0),
                    exitCode = exitCode,
                    stdout = synchronized(stdoutBuffer) { stdoutBuffer.toString() },
                    stderr = synchronized(stderrBuffer) { stderrBuffer.toString() }
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
                error = "INVALID_ARGUMENT"
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
        val granted = grantResult == PackageManager.PERMISSION_GRANTED
        emitBinderState(connected = shizuku_is_running(), permission = granted)
    }

    // --- Event Emission ---

    private fun emitBinderState(connected: Boolean, permission: Boolean) {
        val version = try {
            if (connected) Shizuku.getVersion() else 0
        } catch (e: Throwable) {
            0
        }
        val payload = mapOf(
            "connected" to connected,
            "running" to connected,
            "permission" to permission,
            "version" to version
        )
        eventSink?.invoke(EVENT_BINDER_STATE, payload)
    }
}
