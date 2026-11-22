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
import org.junit.Assert.assertFalse
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
    fun testMissingChannelFallsBackToDefaultChannel() {
        val payload = JSONObject().apply {
            put("title", "Fallback Channel Alert")
            put("body", "Testing channel fallback")
            put("channel_id", "non_existent_channel_xyz")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Success)
        val success = result as NotificationSimulateResult.Success
        assertEquals(BridgeNotificationSimulator.DEFAULT_CHANNEL_ID, success.channelId)
    }

    @Test
    fun testNotificationsDisabledReturns403() {
        val shadowNm = shadowOf(notificationManager)
        shadowNm.setNotificationsEnabled(false)

        val payload = JSONObject().apply {
            put("title", "Disabled Alert")
            put("body", "Should fail due to permission")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("NOTIFICATIONS_DISABLED", error.code)
        assertEquals(403, error.httpStatus)

        // Restore for other tests
        shadowNm.setNotificationsEnabled(true)
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

        // 3. DELETE /notification without id fails 400
        val delNoIdUrl = URL("http://127.0.0.1:$testPort/notification")
        val delNoIdConn = delNoIdUrl.openConnection() as HttpURLConnection
        delNoIdConn.requestMethod = "DELETE"
        delNoIdConn.setRequestProperty("Authorization", "Bearer $testToken")
        assertEquals(400, delNoIdConn.responseCode)
    }

    @Test
    fun testSimulateWithValidCustomId() {
        val payload = JSONObject().apply {
            put("title", "Custom ID Alert")
            put("body", "Notification with custom id")
            put("id", 42)
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Success)
        val success = result as NotificationSimulateResult.Success
        assertEquals(42, success.id)

        // Cancel with custom ID returns isCancelled = true
        val cancelResult = simulator.cancel(42)
        assertTrue(cancelResult.isCancelled)
    }

    @Test
    fun testSimulateWithInvalidCustomIdNegativeReturns400() {
        val payload = JSONObject().apply {
            put("title", "Negative ID Alert")
            put("body", "Should fail due to negative id")
            put("id", -1)
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testSimulateWithInvalidCustomIdZeroReturns400() {
        val payload = JSONObject().apply {
            put("title", "Zero ID Alert")
            put("body", "Should fail due to zero id")
            put("id", 0)
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testSimulateWithInvalidCustomIdStringReturns400() {
        val payload = JSONObject().apply {
            put("title", "String ID Alert")
            put("body", "Should fail due to non-integer id")
            put("id", "not-a-number")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testSimulateWithCoercedStringIdReturns400() {
        val payload = JSONObject().apply {
            put("title", "Coerced String ID Alert")
            put("body", "String numeric id should fail without coercion")
            put("id", "5")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testSimulateWithFloatIdReturns400() {
        val payload = JSONObject().apply {
            put("title", "Float ID Alert")
            put("body", "Float id should not be truncated")
            put("id", 5.5)
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testSimulateWithOverflowIdReturns400() {
        val payload = JSONObject().apply {
            put("title", "Overflow ID Alert")
            put("body", "ID exceeding Int.MAX_VALUE should fail")
            put("id", 3000000000L)
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Error)
        val error = result as NotificationSimulateResult.Error
        assertEquals("BAD_REQUEST", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testCancelUnknownNotificationReturnsFalse() {
        val cancelResult = simulator.cancel(999)
        assertFalse(cancelResult.isCancelled)
        assertEquals(999, cancelResult.id)
    }

    @Test
    fun testCancelTwiceReturnsFalseSecondTime() {
        val payload = JSONObject().apply {
            put("title", "Repeat Cancel")
            put("body", "Testing double cancellation")
        }
        val result = simulator.simulate(payload)
        assertTrue(result is NotificationSimulateResult.Success)
        val id = (result as NotificationSimulateResult.Success).id

        val firstCancel = simulator.cancel(id)
        assertTrue(firstCancel.isCancelled)

        val secondCancel = simulator.cancel(id)
        assertFalse(secondCancel.isCancelled)
    }
}
