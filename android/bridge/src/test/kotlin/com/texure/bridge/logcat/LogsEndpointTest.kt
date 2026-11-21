package com.texure.bridge.logcat

import android.app.Activity
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class LogsEndpointTest {

    private lateinit var activity: Activity
    private lateinit var server: BridgeServer
    private lateinit var ringBuffer: LogRingBuffer
    private lateinit var logcatReader: BridgeLogcatReader
    private val testPort = 8769
    private val testToken = "logs-endpoint-test-token"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val tokenManager = BridgeTokenManager(activity)
        tokenManager.saveToken(testToken)

        ringBuffer = LogRingBuffer(100)
        logcatReader = BridgeLogcatReader(ringBuffer = ringBuffer)

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity },
            logcatReader = logcatReader
        )
        server.start(NanoHTTPD.SOCKET_READ_TIMEOUT, false)
    }

    @After
    fun tearDown() {
        if (::server.isInitialized) {
            server.stop()
        }
    }

    private fun sendRequest(
        method: String,
        path: String,
        token: String? = testToken
    ): Pair<Int, String> {
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
        val future = executor.submit<Pair<Int, String>> {
            val url = URL("http://127.0.0.1:$testPort$path")
            val conn = url.openConnection() as HttpURLConnection
            conn.requestMethod = method
            if (token != null) {
                conn.setRequestProperty("Authorization", "Bearer $token")
            }

            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val responseBody = stream?.bufferedReader()?.use { it.readText() } ?: ""
            Pair(code, responseBody)
        }

        while (!future.isDone) {
            org.robolectric.shadows.ShadowLooper.idleMainLooper()
            Thread.sleep(10)
        }
        val result = future.get()
        executor.shutdown()
        return result
    }

    @Test
    fun testAuthRequired() {
        val (code, body) = sendRequest("GET", "/logs", token = null)
        assertEquals(401, code)
        val json = JSONObject(body)
        assertEquals(BridgeError.AUTH_FAILED.name, json.getJSONObject("error").getString("code"))

        val (delCode, _) = sendRequest("DELETE", "/logs", token = "wrong-token")
        assertEquals(401, delCode)
    }

    @Test
    fun testGetLogsEmpty() {
        val (code, body) = sendRequest("GET", "/logs")
        assertEquals(200, code)
        val json = JSONObject(body)
        assertEquals(0, json.getInt("count"))
        assertEquals(0, json.getJSONArray("logs").length())
    }

    @Test
    fun testGetLogsWithFiltering() {
        ringBuffer.add(LogEntry(1000L, 10, 20, LogLevel.VERBOSE, "Network", "payload start"))
        ringBuffer.add(LogEntry(2000L, 10, 20, LogLevel.DEBUG, "Auth", "token refreshed"))
        ringBuffer.add(LogEntry(3000L, 10, 20, LogLevel.INFO, "UI", "screen rendered"))
        ringBuffer.add(LogEntry(4000L, 10, 20, LogLevel.WARN, "Network", "slow connection"))
        ringBuffer.add(LogEntry(5000L, 10, 20, LogLevel.ERROR, "CrashHandler", "NPE detected"))

        // Level filtering: level=WARN or level=W
        val (codeW, bodyW) = sendRequest("GET", "/logs?level=W")
        assertEquals(200, codeW)
        val jsonW = JSONObject(bodyW)
        assertEquals(2, jsonW.getInt("count"))

        // Tag filtering
        val (codeTag, bodyTag) = sendRequest("GET", "/logs?tag=auth")
        assertEquals(200, codeTag)
        val jsonTag = JSONObject(bodyTag)
        assertEquals(1, jsonTag.getInt("count"))
        assertEquals("Auth", jsonTag.getJSONArray("logs").getJSONObject(0).getString("tag"))

        // Since filtering
        val (codeSince, bodySince) = sendRequest("GET", "/logs?since=3000")
        assertEquals(200, codeSince)
        val jsonSince = JSONObject(bodySince)
        assertEquals(2, jsonSince.getInt("count"))

        // Limit
        val (codeLimit, bodyLimit) = sendRequest("GET", "/logs?limit=3")
        assertEquals(200, codeLimit)
        val jsonLimit = JSONObject(bodyLimit)
        assertEquals(3, jsonLimit.getInt("count"))
    }

    @Test
    fun testDeleteLogsClearsBuffer() {
        ringBuffer.add(LogEntry(1000L, 10, 20, LogLevel.ERROR, "Test", "error message"))
        assertEquals(1, ringBuffer.size)

        val (delCode, delBody) = sendRequest("DELETE", "/logs")
        assertEquals(200, delCode)
        val delJson = JSONObject(delBody)
        assertTrue(delJson.getBoolean("ok"))
        assertTrue(delJson.getBoolean("cleared"))

        val (getCode, getBody) = sendRequest("GET", "/logs")
        assertEquals(200, getCode)
        val getJson = JSONObject(getBody)
        assertEquals(0, getJson.getInt("count"))
    }

    @Test
    fun testWebSocketErrorBroadcast() {
        val receivedMessages = mutableListOf<String>()
        server.onWebSocketBroadcast = { msg ->
            receivedMessages.add(msg)
        }

        // Consume an INFO log entry -> should not broadcast
        logcatReader.consumeLine("11-20 15:00:00.000 123 456 I InfoTag: Normal info log")
        assertEquals(0, receivedMessages.size)

        // Consume an ERROR log entry -> should broadcast log_error
        logcatReader.consumeLine("11-20 15:00:01.000 123 456 E CriticalTag: Fatal error encountered")

        assertEquals(1, receivedMessages.size)
        val msgJson = JSONObject(receivedMessages.first())
        assertEquals("log_error", msgJson.getString("event"))
        assertEquals("log_error", msgJson.getString("type"))
        val entryJson = msgJson.getJSONObject("entry")
        assertEquals("CriticalTag", entryJson.getString("tag"))
        assertEquals("ERROR", entryJson.getString("level"))
        assertEquals("Fatal error encountered", entryJson.getString("message"))
    }
}
