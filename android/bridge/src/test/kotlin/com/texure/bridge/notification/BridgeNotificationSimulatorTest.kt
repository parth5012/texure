package com.texure.bridge.notification

import android.app.Activity
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.os.Build
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeNotificationSimulatorTest {

    private lateinit var activity: Activity
    private lateinit var notificationManager: NotificationManager
    private lateinit var simulator: BridgeNotificationSimulator
    private lateinit var server: BridgeServer
    private val testPort = 8770
    private val testToken = "test-token-85"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        notificationManager = activity.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        simulator = BridgeNotificationSimulator(activity)

        val tokenManager = BridgeTokenManager(activity)
        tokenManager.saveToken(testToken)

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity }
        )
        server.start(NanoHTTPD.SOCKET_READ_TIMEOUT, false)
    }

    @After
    fun tearDown() {
        if (::server.isInitialized) {
            server.stop()
        }
    }

    @Test
    fun testSimulateNotificationAutoCreatesDebugChannelAndDispatches() {
        val payload = JSONObject().apply {
            put("title", "Incoming Alert")
            put("body", "Test message content")
            put("intent_uri", "texure://sample/details?id=99")
        }

        val result = simulator.simulate(payload)
        assertTrue("Expected Success result, got $result", result is NotificationSimulateResult.Success)

        val success = result as NotificationSimulateResult.Success
        assertEquals("texure_debug_channel", success.channelId)
        assertTrue(success.id > 0)

        // Verify channel exists in notification manager (API 26+)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = notificationManager.getNotificationChannel("texure_debug_channel")
            assertNotNull("Debug channel should be auto-created", channel)
            assertEquals(NotificationManager.IMPORTANCE_HIGH, channel.importance)
        }

        // Verify notification in shadow notification manager
        val shadowNm = shadowOf(notificationManager)
        val notifications = shadowNm.allNotifications
        assertEquals(1, notifications.size)

        val notification = notifications[0]
        val shadowNotification = shadowOf(notification)
        assertEquals("Incoming Alert", shadowNotification.contentTitle)
        assertEquals("Test message content", shadowNotification.contentText)

        // Verify pending intent attachment
        val contentIntent = notification.contentIntent
        assertNotNull("Notification should have a contentIntent", contentIntent)
        val shadowPendingIntent = shadowOf(contentIntent)
        val savedIntent = shadowPendingIntent.savedIntent
        assertEquals(Intent.ACTION_VIEW, savedIntent.action)
        assertEquals("texure://sample/details?id=99", savedIntent.dataString)
        assertEquals(activity.packageName, savedIntent.`package`)
    }

    @Test
    fun testCancelNotification() {
        val payload = JSONObject().apply {
            put("title", "Cancel Me")
            put("body", "Will be dismissed")
        }

        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Success)
        val id = (result as NotificationSimulateResult.Success).id

        val shadowNm = shadowOf(notificationManager)
        assertEquals(1, shadowNm.allNotifications.size)

        val cancelResult = simulator.cancel(id)
        assertTrue(cancelResult.isCancelled)
        assertEquals(0, shadowNm.allNotifications.size)
    }

    @Test
    fun testMissingTitleOrBodyReturns400() {
        val payload = JSONObject().apply {
            put("title", "")
            put("body", "Missing title")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testServerPostNotificationSimulateAndCancel() {
        // 1. POST /notification/simulate
        val postUrl = URL("http://127.0.0.1:$testPort/notification/simulate")
        val postConn = postUrl.openConnection() as HttpURLConnection
        postConn.requestMethod = "POST"
        postConn.setRequestProperty("Authorization", "Bearer $testToken")
        postConn.setRequestProperty("Content-Type", "application/json")
        postConn.doOutput = true

        val body = JSONObject().apply {
            put("title", "Server Alert")
            put("body", "Sent via BridgeServer")
            put("intent_uri", "texure://sample/profile")
        }
        postConn.outputStream.use { it.write(body.toString().toByteArray()) }

        assertEquals(200, postConn.responseCode)
        val postResponse = postConn.inputStream.bufferedReader().use { it.readText() }
        val postJson = JSONObject(postResponse)
        assertTrue(postJson.getBoolean("ok"))
        val notifId = postJson.getInt("id")
        assertEquals("texure_debug_channel", postJson.getString("channel_id"))

        // 2. DELETE /notification?id=<notifId>
        val delUrl = URL("http://127.0.0.1:$testPort/notification?id=$notifId")
        val delConn = delUrl.openConnection() as HttpURLConnection
        delConn.requestMethod = "DELETE"
        delConn.setRequestProperty("Authorization", "Bearer $testToken")

        assertEquals(200, delConn.responseCode)
        val delResponse = delConn.inputStream.bufferedReader().use { it.readText() }
        val delJson = JSONObject(delResponse)
        assertTrue(delJson.getBoolean("ok"))
        assertTrue(delJson.getBoolean("cancelled"))
    }
}
