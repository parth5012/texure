package com.texure.bridge.recording

import android.app.Activity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL

/**
 * BB-B12 / BB-B15: /recording/start|stop|export|delete HTTP contract (behavior-spec 1.7).
 * BB-B13 partially: agent mutations routed through the bridge are NOT recorded.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class RecordingEndpointsTest {

    private lateinit var activity: Activity
    private lateinit var testButton: Button
    private lateinit var testInput: EditText
    private lateinit var server: BridgeServer
    private val testPort = 8765
    private val testToken = "test-token-12345"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val layout = LinearLayout(activity)

        testButton = Button(activity).apply {
            text = "Click Me"
            contentDescription = "Test button"
        }
        testInput = EditText(activity).apply {
            hint = "Type here"
        }

        layout.addView(testButton)
        layout.addView(testInput)
        activity.setContentView(layout)
        layout.measure(
            android.view.View.MeasureSpec.makeMeasureSpec(1080, android.view.View.MeasureSpec.EXACTLY),
            android.view.View.MeasureSpec.makeMeasureSpec(1920, android.view.View.MeasureSpec.EXACTLY)
        )
        layout.layout(0, 0, 1080, 1920)

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

    private fun sendRequest(
        method: String,
        path: String,
        body: String? = null,
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
            if (body != null) {
                conn.doOutput = true
                conn.setRequestProperty("Content-Type", "application/json")
                conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
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

    // BB-B12: auth on the new control routes (same as every route, BB-B3)
    @Test
    fun recordingRoutesRequireAuth() {
        val (code, body) = sendRequest("POST", "/recording/start", body = "{}", token = null)
        assertEquals(401, code)
        assertEquals(
            BridgeError.AUTH_FAILED.name,
            JSONObject(body).getJSONObject("error").getString("code")
        )
    }

    // BB-B12: lifecycle — idle stop refused; start ok; idempotent start ok; stop ok; stop idle refused
    @Test
    fun startStopLifecycle() {
        var (code, body) = sendRequest("POST", "/recording/stop", body = "{}")
        assertEquals(400, code)
        assertEquals("RECORDING_IDLE", JSONObject(body).getJSONObject("error").getString("code"))

        val (startCode, startBody) = sendRequest("POST", "/recording/start", body = "{}")
        assertEquals(200, startCode)
        assertTrue(JSONObject(startBody).getBoolean("recording"))

        val (againCode, againBody) = sendRequest("POST", "/recording/start", body = "{}")
        assertEquals(200, againCode)
        assertTrue(JSONObject(againBody).getBoolean("recording"))

        val (stopCode, stopBody) = sendRequest("POST", "/recording/stop", body = "{}")
        assertEquals(200, stopCode)
        val stopped = JSONObject(stopBody)
        assertEquals(false, stopped.getBoolean("recording"))
        assertEquals(0, stopped.getInt("count"))

        val (idleCode, _) = sendRequest("POST", "/recording/stop", body = "{}")
        assertEquals(400, idleCode)
    }

    // BB-B15: export shape on a fresh server
    @Test
    fun exportIdleShape() {
        val (code, body) = sendRequest("GET", "/recording/export")
        assertEquals(200, code)
        val json = JSONObject(body)
        assertEquals(false, json.getBoolean("recording"))
        assertEquals(0, json.getInt("count"))
        assertEquals(0, json.getJSONArray("actions").length())
    }

    // BB-B15: DELETE clears the buffer (works idle)
    @Test
    fun deleteClearsBuffer() {
        val (code, body) = sendRequest("DELETE", "/recording")
        assertEquals(200, code)
        assertEquals(true, JSONObject(body).getBoolean("ok"))

        val (_, exportBody) = sendRequest("GET", "/recording/export")
        val json = JSONObject(exportBody)
        assertEquals(0, json.getInt("count"))
        assertEquals(0, json.getJSONArray("actions").length())
    }

    // BB-B13: agent mutations via bridge HTTP are excluded from the recording
    @Test
    fun agentMutationsAreNotRecorded() {
        sendRequest("POST", "/recording/start", body = "{}")

        val (_, treeBody) = sendRequest("GET", "/ui-tree")
        val tree = JSONObject(treeBody)
        val rev = tree.getLong("rev")
        val nodeIndex = tree.getJSONArray("nodes").let { arr ->
            var idx = -1
            for (i in 0 until arr.length()) {
                if (arr.getJSONObject(i).optString("text") == "Click Me") {
                    idx = i
                    break
                }
            }
            idx
        }
        assertTrue("button must be in tree", nodeIndex >= 0)
        sendRequest("POST", "/tap", body = JSONObject().put("node", nodeIndex).put("rev", rev).toString())

        val (_, exportBody) = sendRequest("GET", "/recording/export")
        val actions = JSONObject(exportBody).getJSONArray("actions")
        assertEquals("agent-routed tap must not be recorded", 0, actions.length())
    }
}
