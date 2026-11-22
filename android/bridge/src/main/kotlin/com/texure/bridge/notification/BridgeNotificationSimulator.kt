package com.texure.bridge.notification

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicInteger

sealed class NotificationSimulateResult {
    data class Success(val id: Int, val channelId: String) : NotificationSimulateResult()
    data class Error(val code: String, val message: String, val httpStatus: Int = 400) : NotificationSimulateResult()
}

data class NotificationCancelResult(val id: Int?, val isCancelled: Boolean)

class BridgeNotificationSimulator(
    private val context: Context,
    private val notificationManager: NotificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
) {
    companion object {
        const val DEFAULT_CHANNEL_ID = "texure_debug_channel"
        const val DEFAULT_CHANNEL_NAME = "Texure Debug Channel"
        const val BRIDGE_NOTIFICATION_TAG = "texure_bridge_notification"
    }

    private val idCounter = AtomicInteger(1)

    fun simulate(payload: JSONObject): NotificationSimulateResult {
        val title = payload.optString("title").trim()
        val body = payload.optString("body").trim()

        if (title.isEmpty() || body.isEmpty()) {
            return NotificationSimulateResult.Error(
                code = "BAD_REQUEST",
                message = "Missing or empty required parameters: 'title' and 'body' are required",
                httpStatus = 400
            )
        }

        val nmCompat = NotificationManagerCompat.from(context)
        if (!nmCompat.areNotificationsEnabled()) {
            return NotificationSimulateResult.Error(
                code = "NOTIFICATIONS_DISABLED",
                message = "Notifications are disabled for package ${context.packageName}. Grant POST_NOTIFICATIONS runtime permission.",
                httpStatus = 403
            )
        }

        val requestedChannel = payload.optString("channel_id").takeIf { it.isNotBlank() }
        val effectiveChannelId = resolveOrCreateChannel(requestedChannel)

        val id = if (payload.has("id")) {
            val customId = payload.optInt("id", -1)
            if (customId <= 0) {
                return NotificationSimulateResult.Error(
                    code = "BAD_REQUEST",
                    message = "Invalid 'id' parameter: must be a positive integer",
                    httpStatus = 400
                )
            }
            customId
        } else {
            idCounter.getAndIncrement()
        }

        val builder = NotificationCompat.Builder(context, effectiveChannelId)
            .setContentTitle(title)
            .setContentText(body)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)

        // Resolve small icon
        val smallIconStr = payload.optString("small_icon").takeIf { it.isNotBlank() }
        val iconRes = resolveIcon(smallIconStr)
        builder.setSmallIcon(iconRes)

        // Resolve PendingIntent if intent_uri is provided
        val intentUriStr = payload.optString("intent_uri").takeIf { it.isNotBlank() }
        if (intentUriStr != null) {
            val deepLinkIntent = Intent(Intent.ACTION_VIEW, Uri.parse(intentUriStr)).apply {
                `package` = context.packageName
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            val pendingIntent = PendingIntent.getActivity(context, id, deepLinkIntent, flags)
            builder.setContentIntent(pendingIntent)
        }

        return try {
            notificationManager.notify(BRIDGE_NOTIFICATION_TAG, id, builder.build())
            NotificationSimulateResult.Success(id = id, channelId = effectiveChannelId)
        } catch (e: Exception) {
            NotificationSimulateResult.Error(
                code = "NOTIFICATION_FAILED",
                message = "Failed to dispatch notification: ${e.message}",
                httpStatus = 500
            )
        }
    }

    fun cancel(id: Int?): NotificationCancelResult {
        return if (id != null) {
            try {
                notificationManager.cancel(BRIDGE_NOTIFICATION_TAG, id)
                NotificationCancelResult(id = id, isCancelled = true)
            } catch (_: Exception) {
                NotificationCancelResult(id = id, isCancelled = false)
            }
        } else {
            NotificationCancelResult(id = null, isCancelled = false)
        }
    }

    private fun resolveOrCreateChannel(channelId: String?): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val trimmed = channelId?.trim()
            if (!trimmed.isNullOrEmpty()) {
                val existing = notificationManager.getNotificationChannel(trimmed)
                if (existing != null) {
                    return trimmed
                }
            }
            // Fall back to DEFAULT_CHANNEL_ID (texure_debug_channel)
            if (notificationManager.getNotificationChannel(DEFAULT_CHANNEL_ID) == null) {
                val channel = NotificationChannel(
                    DEFAULT_CHANNEL_ID,
                    DEFAULT_CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = "Texure debug notification simulation channel"
                }
                notificationManager.createNotificationChannel(channel)
            }
            return DEFAULT_CHANNEL_ID
        }
        return channelId?.trim()?.ifEmpty { DEFAULT_CHANNEL_ID } ?: DEFAULT_CHANNEL_ID
    }

    private fun resolveIcon(iconName: String?): Int {
        if (iconName != null) {
            val resId = context.resources.getIdentifier(iconName, "drawable", context.packageName)
            if (resId != 0) return resId
        }
        val appIcon = context.applicationInfo.icon
        return if (appIcon != 0) appIcon else android.R.drawable.stat_notify_chat
    }
}
