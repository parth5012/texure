package com.texure.bridge.service

import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.texure.bridge.crash.BridgeCrashHandler
import com.texure.bridge.model.BridgeError
import com.texure.bridge.model.BridgeState
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import java.io.IOException
import java.net.InetAddress
import java.net.NetworkInterface

class BridgeService : Service() {

    private val binder = LocalBinder()
    private var server: BridgeServer? = null
    private var tokenManager: BridgeTokenManager? = null
    private var crashHandler: BridgeCrashHandler? = null

    private var currentPort = DEFAULT_PORT
    private var isPortInUse = false

    inner class LocalBinder : Binder() {
        fun getService(): BridgeService = this@BridgeService
    }

    override fun onBind(intent: Intent?): IBinder = binder

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        tokenManager = BridgeTokenManager(this)
        crashHandler = BridgeCrashHandler(this).apply { install() }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        if (action == ACTION_STOP) {
            stopBridge()
            stopSelf()
            return START_NOT_STICKY
        }

        startForeground(NOTIFICATION_ID, buildNotification("Starting bridge..."))
        startBridgeServer()
        return START_STICKY
    }

    fun startBridgeServer(activity: Activity? = null) {
        if (server != null) return

        val tm = tokenManager ?: BridgeTokenManager(this).also { tokenManager = it }
        val ch = crashHandler ?: BridgeCrashHandler(this).also { crashHandler = it }

        activeActivity = activity ?: activeActivity

        val newServer = BridgeServer(
            context = this,
            port = currentPort,
            tokenManager = tm,
            activityProvider = { activeActivity },
            crashHandler = ch
        )

        try {
            newServer.start(NanoHTTPD.SOCKET_READ_TIMEOUT, false)
            server = newServer
            isPortInUse = false
            updateNotification("Bridge running on port $currentPort (@IDLE)")
            android.util.Log.i("TexureBridge", "Pairing token: ${tm.getToken()} (port $currentPort)")
        } catch (e: IOException) {
            // Bind conflict -> stop immediately and surface PORT_IN_USE
            isPortInUse = true
            try { newServer.stop() } catch (_: Exception) {}
            stopBridge()
            updateNotification("Error: ${BridgeError.PORT_IN_USE.name} on port $currentPort")
            stopSelf()
        }
    }

    fun stopBridge() {
        server?.stop()
        server = null
    }

    fun rotateToken(): String {
        val tm = tokenManager ?: BridgeTokenManager(this).also { tokenManager = it }
        val newToken = tm.rotateToken()
        android.util.Log.i("TexureBridge", "Pairing token rotated: $newToken (port $currentPort)")
        updateNotification("Bridge running on port $currentPort (@IDLE)")
        return newToken
    }

    fun getToken(): String {
        return tokenManager?.getToken() ?: ""
    }

    fun getLocalIpAddress(): String {
        try {
            val interfaces = NetworkInterface.getNetworkInterfaces()
            while (interfaces.hasMoreElements()) {
                val iface = interfaces.nextElement()
                val addresses = iface.inetAddresses
                while (addresses.hasMoreElements()) {
                    val addr = addresses.nextElement()
                    if (!addr.isLoopbackAddress && addr is java.net.Inet4Address) {
                        return addr.hostAddress ?: "127.0.0.1"
                    }
                }
            }
        } catch (_: Exception) {}
        return "127.0.0.1"
    }

    fun getServerState(): BridgeState {
        return server?.state ?: BridgeState.IDLE
    }

    fun isRunning(): Boolean = server != null

    fun isPortConflict(): Boolean = isPortInUse

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Texure Bridge Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Developer bridge for agent testing"
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(statusText: String): Notification {
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Texure Bridge")
            .setContentText(statusText)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setOngoing(true)
            .build()
    }

    private fun updateNotification(statusText: String) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, buildNotification(statusText))
    }

    override fun onDestroy() {
        stopBridge()
        super.onDestroy()
    }

    companion object {
        const val DEFAULT_PORT = 8765
        const val CHANNEL_ID = "texure-bridge"
        const val NOTIFICATION_ID = 87650
        const val ACTION_STOP = "com.texure.bridge.STOP"

        var activeActivity: Activity? = null
    }
}
