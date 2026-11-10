package com.texure.bridge.service

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.util.Log

/**
 * mDNS/DNS-SD advertisement for the bridge (map #36, ticket #32).
 *
 * Registers `_texure._tcp.local.` on the fixed bridge port with TXT
 * `version=0.1.0` + `appId=<debugApplicationId>` via the platform
 * [NsdManager] (no new Gradle dependency, per the map
 * zero-external-dependency decision). Registration is best-effort:
 * Robolectric has no real `NsdManager`, and devices without NSD support
 * must still run the bridge on static IP — every call is guarded so a
 * failure is logged and never crashes the service.
 */
object MdnsAdvertiser {

    const val SERVICE_TYPE = "_texure._tcp.local."
    const val BRIDGE_PORT = 8765
    const val VERSION = "0.1.0"

    /** Service instance name, e.g. `Texure-com.texure.sample`. */
    fun serviceName(appId: String): String = "Texure-$appId"

    /** TXT records advertising the bridge contract version + app id. */
    fun txtRecords(appId: String): Map<String, String> =
        mapOf("version" to VERSION, "appId" to appId)

    /** Build the [NsdServiceInfo] for this app instance. Pure; unit-testable. */
    fun buildServiceInfo(appId: String, port: Int = BRIDGE_PORT): NsdServiceInfo {
        return NsdServiceInfo().apply {
            serviceName = this@MdnsAdvertiser.serviceName(appId)
            serviceType = SERVICE_TYPE
            this.port = port
            for ((key, value) in txtRecords(appId)) {
                setAttribute(key, value)
            }
        }
    }

    /** Handle for a live registration; required to unregister cleanly. */
    class Registration(
        val info: NsdServiceInfo,
        val listener: NsdManager.RegistrationListener
    )

    private val listener = object : NsdManager.RegistrationListener {
        override fun onServiceRegistered(info: NsdServiceInfo?) {
            Log.i("TexureBridge", "mDNS registered: ${info?.serviceName} ${info?.serviceType}:${info?.port}")
        }

        override fun onRegistrationFailed(info: NsdServiceInfo?, errorCode: Int) {
            Log.w("TexureBridge", "mDNS registration failed (code $errorCode); static IP still works")
        }

        override fun onServiceUnregistered(info: NsdServiceInfo?) {
            Log.i("TexureBridge", "mDNS unregistered: ${info?.serviceName}")
        }

        override fun onUnregistrationFailed(info: NsdServiceInfo?, errorCode: Int) {
            Log.w("TexureBridge", "mDNS unregistration failed (code $errorCode)")
        }
    }

    /**
     * Register the service; returns null (and logs) when `NsdManager` is
     * unavailable or registration throws — the bridge keeps running.
     */
    fun register(context: Context, appId: String, port: Int = BRIDGE_PORT): Registration? {
        return runCatching {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.LOLLIPOP) return null
            val nsd = context.getSystemService(Context.NSD_SERVICE) as? NsdManager ?: return null
            val info = buildServiceInfo(appId, port)
            nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, listener)
            Registration(info, listener)
        }.onFailure {
            Log.w("TexureBridge", "mDNS unavailable; continuing without discovery: ${it.message}")
        }.getOrNull()
    }

    /** Unregister a previous [Registration]; never throws. */
    fun unregister(context: Context, registration: Registration?) {
        if (registration == null) return
        runCatching {
            val nsd = context.getSystemService(Context.NSD_SERVICE) as? NsdManager ?: return
            nsd.unregisterService(registration.listener)
        }.onFailure {
            Log.w("TexureBridge", "mDNS unregister failed: ${it.message}")
        }
    }
}
