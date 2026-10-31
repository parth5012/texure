package com.texure.bridge

import android.app.Activity
import com.texure.bridge.crash.BridgeCrashHandler
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
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

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class CrashHandlerTest {

    private lateinit var activity: Activity
    private lateinit var crashHandler: BridgeCrashHandler
    private lateinit var server: BridgeServer
    private val testPort = 8766
    private val testToken = "crash-test-token"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val tokenManager = BridgeTokenManager(activity)
        tokenManager.saveToken(testToken)

        crashHandler = BridgeCrashHandler(activity)
        crashHandler.clearCrash()

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity },
            crashHandler = crashHandler
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
        body: String? = null,
        token: String? = testToken
    ): Pair<Int, String> {
        val url = URL("http://127.0.0.1:$testPort$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = method
        if (token != null) {
            conn.setRequestProperty("Authorization", "Bearer $token")
        }
        if (body != null) {
            conn.doOutput = true
            conn.setRequestProperty("Content-Type", "application/json")
            conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
        }

        val code = conn.responseCode
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream
        val responseBody = stream?.bufferedReader()?.use { it.readText() } ?: ""
        return Pair(code, responseBody)
    }

    @Test
    fun testGetLastCrashReturns404WhenNoCrash() {
        val (code, body) = sendRequest("GET", "/last-crash")
        assertEquals(404, code)
        val json = JSONObject(body)
        assertEquals("NOT_FOUND", json.getJSONObject("error").getString("code"))
    }

    @Test
    fun testUncaughtExceptionWritesBundleAndExposedViaRoute() {
        val exception = RuntimeException("Test simulated crash for ticket #4")
        crashHandler.recordCrash(
            thread = Thread.currentThread(),
            throwable = exception,
            lastTree = JSONObject().apply {
                put("rev", 10)
                put("window", "TestWindow")
            },
            recentActions = listOf("tap node 0", "type node 1")
        )

        val (code, body) = sendRequest("GET", "/last-crash")
        assertEquals(200, code)
        val json = JSONObject(body)
        assertTrue(json.has("timestamp"))
        assertTrue(json.has("stack_trace"))
        assertTrue(json.getString("stack_trace").contains("Test simulated crash"))
        assertTrue(json.has("last_tree"))
        assertTrue(json.has("last_actions"))
        assertTrue(json.has("build_info"))
    }
}
