package com.texure.companion

import android.content.Context
import android.content.pm.PackageManager
import rikka.shizuku.Shizuku

/**
 * Native plugin managing Shizuku binder connection, permission lifecycle,
 * and event notifications for the Texure companion app.
 */
class ShizukuPlugin(
    private val context: Context,
    private val eventSink: ((event: String, payload: Map<String, Any?>) -> Unit)? = null
) : Shizuku.OnRequestPermissionResultListener,
    Shizuku.OnBinderReceivedListener,
    Shizuku.OnBinderDeadListener {

    companion object {
        const val REQUEST_CODE_PERMISSION = 1001
        const val EVENT_BINDER_STATE = "shizuku://binder-state"
        val SHIZUKU_PACKAGES = listOf("moe.shizuku.privileged.api", "rikka.shizuku")
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
                // Ignore if binder registration fails (e.g. Shizuku not running)
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
