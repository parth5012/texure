package com.texure.companion

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

/**
 * Foreground service of type `connectedDevice` that maintains the Android companion
 * tunnel and prevents Android process kills during agent runs.
 */
class CompanionDaemonService : Service() {

    companion object {
        const val CHANNEL_ID = "texure_daemon_channel"
        const val CHANNEL_NAME = "Texure Relay Service"
        const val NOTIFICATION_ID = 1001

        const val ACTION_START = "com.texure.companion.action.START_DAEMON"
        const val ACTION_STOP = "com.texure.companion.action.STOP_DAEMON"
        const val ACTION_UPDATE_STATUS = "com.texure.companion.action.UPDATE_STATUS"
        const val ACTION_STOP_RELAY = "com.texure.companion.action.STOP_RELAY"
        const val ACTION_RESTART_APP = "com.texure.companion.action.RESTART_APP"

        const val EXTRA_STATUS = "extra_status"
        const val EXTRA_ACQUIRE_WAKE_LOCK = "extra_acquire_wake_lock"
        const val DEFAULT_STATUS = "Connected to PC"
        const val DEFAULT_WAKE_LOCK_TIMEOUT_MS = 60 * 60 * 1000L // 1 hour safety default

        const val WAKE_LOCK_TAG = "texure:companion_daemon"

        // Observable thread-safe runtime state for tests & clients
        private val _isRunning = AtomicBoolean(false)
        private val _isWakeLockHeld = AtomicBoolean(false)
        private val _currentStatus = AtomicReference("Stopped")

        var isRunning: Boolean
            get() = _isRunning.get()
            internal set(value) { _isRunning.set(value) }

        var isWakeLockHeld: Boolean
            get() = _isWakeLockHeld.get()
            internal set(value) { _isWakeLockHeld.set(value) }

        var currentStatus: String
            get() = _currentStatus.get()
            internal set(value) { _currentStatus.set(value) }

        /**
         * Starts the daemon service in the foreground.
         */
        fun start(
            context: Context,
            status: String = DEFAULT_STATUS,
            acquireWakeLock: Boolean = false
        ) {
            val intent = Intent(context, CompanionDaemonService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_STATUS, status)
                putExtra(EXTRA_ACQUIRE_WAKE_LOCK, acquireWakeLock)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /**
         * Stops the daemon service and releases resources.
         */
        fun stop(context: Context) {
            val intent = Intent(context, CompanionDaemonService::class.java).apply {
                action = ACTION_STOP
            }
            context.startService(intent)
        }

        /**
         * Updates status text in the ongoing foreground notification.
         */
        fun updateStatus(context: Context, status: String) {
            val intent = Intent(context, CompanionDaemonService::class.java).apply {
                action = ACTION_UPDATE_STATUS
                putExtra(EXTRA_STATUS, status)
            }
            context.startService(intent)
        }

        /**
         * Helper to create notification channel on API 26+.
         */
        fun createNotificationChannel(context: Context) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
                val channel = NotificationChannel(
                    CHANNEL_ID,
                    CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "Texure background relay connection state"
                    setShowBadge(false)
                }
                manager?.createNotificationChannel(channel)
            }
        }

        /**
         * Builds the ongoing notification with actions.
         */
        fun buildNotification(context: Context, status: String): Notification {
            createNotificationChannel(context)

            val stopIntent = Intent(context, CompanionDaemonService::class.java).apply {
                action = ACTION_STOP_RELAY
            }
            val stopPendingIntent = PendingIntent.getService(
                context,
                1,
                stopIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            val restartIntent = Intent(context, MainActivity::class.java).apply {
                action = ACTION_RESTART_APP
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
            }
            val restartPendingIntent = PendingIntent.getActivity(
                context,
                2,
                restartIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            return NotificationCompat.Builder(context, CHANNEL_ID)
                .setContentTitle("Texure Relay")
                .setContentText("Texure Relay: $status")
                .setSmallIcon(android.R.drawable.stat_notify_sync)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .addAction(
                    android.R.drawable.ic_menu_close_clear_cancel,
                    "Stop Relay",
                    stopPendingIntent
                )
                .addAction(
                    android.R.drawable.ic_menu_rotate,
                    "Restart App",
                    restartPendingIntent
                )
                .build()
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel(this)
        isRunning = true
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action ?: ACTION_START

        when (action) {
            ACTION_STOP, ACTION_STOP_RELAY -> {
                handleStop()
                return START_NOT_STICKY
            }

            ACTION_UPDATE_STATUS -> {
                val status = intent?.getStringExtra(EXTRA_STATUS) ?: DEFAULT_STATUS
                handleUpdateStatus(status)
            }

            ACTION_START -> {
                val status = intent?.getStringExtra(EXTRA_STATUS) ?: DEFAULT_STATUS
                val shouldWakeLock = intent?.getBooleanExtra(EXTRA_ACQUIRE_WAKE_LOCK, false) ?: false

                currentStatus = status
                val notification = buildNotification(this, status)

                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    startForeground(
                        NOTIFICATION_ID,
                        notification,
                        ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
                    )
                } else {
                    startForeground(NOTIFICATION_ID, notification)
                }

                if (shouldWakeLock) {
                    acquireWakeLock()
                }
            }
        }

        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        releaseWakeLock()
        isRunning = false
        currentStatus = "Stopped"
        super.onDestroy()
    }

    private fun handleUpdateStatus(status: String) {
        currentStatus = status
        val notification = buildNotification(this, status)
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
        manager?.notify(NOTIFICATION_ID, notification)
    }

    private fun handleStop() {
        releaseWakeLock()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        isRunning = false
        currentStatus = "Stopped"
        stopSelf()
    }

    /**
     * Acquires partial wake lock to keep CPU active during agent test runs.
     * Uses a safety timeout (default 1 hour) to prevent indefinite battery drain.
     */
    fun acquireWakeLock(timeoutMs: Long = DEFAULT_WAKE_LOCK_TIMEOUT_MS) {
        if (wakeLock == null) {
            val pm = getSystemService(Context.POWER_SERVICE) as? PowerManager
            wakeLock = pm?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, WAKE_LOCK_TAG)?.apply {
                setReferenceCounted(false)
            }
        }
        wakeLock?.let { wl ->
            if (!wl.isHeld) {
                if (timeoutMs > 0) {
                    wl.acquire(timeoutMs)
                } else {
                    wl.acquire(DEFAULT_WAKE_LOCK_TIMEOUT_MS)
                }
            }
        }
        isWakeLockHeld = wakeLock?.isHeld == true
    }

    /**
     * Releases partial wake lock.
     */
    fun releaseWakeLock() {
        try {
            if (wakeLock?.isHeld == true) {
                wakeLock?.release()
            }
        } catch (_: Throwable) {
        }
        isWakeLockHeld = false
    }
}
