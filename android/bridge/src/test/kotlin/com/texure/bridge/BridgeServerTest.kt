package com.texure.bridge

import android.app.Activity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.service.BridgeService
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
class BridgeServerTest {

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
            id = 101
            text = "Click Me"
            contentDescription = "Test button"
        }
        testInput = EditText(activity).apply {
            id = 102
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

    @Test
    fun testAuthHeaderMissingFails() {
        val (code, body) = sendRequest("GET", "/ui-tree", token = null)
        assertEquals(401, code)
        val json = JSONObject(body)
        assertEquals(BridgeError.AUTH_FAILED.name, json.getJSONObject("error").getString("code"))
    }

    @Test
    fun testAuthHeaderMismatchFails() {
        val (code, body) = sendRequest("GET", "/ui-tree", token = "wrong-token")
        assertEquals(401, code)
        val json = JSONObject(body)
        assertEquals(BridgeError.AUTH_FAILED.name, json.getJSONObject("error").getString("code"))
    }

    @Test
    fun testGetUiTreeReturnsNodesAndRev() {
        val (code, body) = sendRequest("GET", "/ui-tree")
        assertEquals(200, code)
        val json = JSONObject(body)
        assertTrue(json.has("rev"))
        assertTrue(json.has("window"))
        val nodes = json.getJSONArray("nodes")
        assertTrue("Expected non-empty nodes", nodes.length() > 0)
    }

    @Test
    fun testTapWithValidRevAndClickableNodeSucceeds() {
        val (treeCode, treeBody) = sendRequest("GET", "/ui-tree")
        assertEquals(200, treeCode)
        val treeJson = JSONObject(treeBody)
        val currentRev = treeJson.getLong("rev")

        val tapPayload = JSONObject().apply {
            put("node", 0)
            put("rev", currentRev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = tapPayload)
        assertEquals(200, tapCode)
        val tapJson = JSONObject(tapBody)
        assertTrue(tapJson.optBoolean("ok"))
    }

    @Test
    fun testTapWithStaleRevReturnsNodeOutdated() {
        val staleRev = 999999L
        val tapPayload = JSONObject().apply {
            put("node", 0)
            put("rev", staleRev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = tapPayload)
        assertEquals(400, tapCode)
        val errorJson = JSONObject(tapBody)
        assertEquals(BridgeError.NODE_OUTDATED.name, errorJson.getJSONObject("error").getString("code"))
    }

    @Test
    fun testTypeIntoEditableSucceeds() {
        val (_, treeBody) = sendRequest("GET", "/ui-tree")
        val currentRev = JSONObject(treeBody).getLong("rev")

        val typePayload = JSONObject().apply {
            put("node", 1)
            put("text", "hello world")
            put("rev", currentRev)
        }.toString()

        val (typeCode, typeBody) = sendRequest("POST", "/type", body = typePayload)
        assertEquals("Failed with body: $typeBody", 200, typeCode)
        assertTrue(JSONObject(typeBody).optBoolean("ok"))
    }

    @Test
    fun testKeypressEnterSucceeds() {
        val keyPayload = JSONObject().apply {
            put("key", "ENTER")
        }.toString()

        val (keyCode, keyBody) = sendRequest("POST", "/keypress", body = keyPayload)
        assertEquals(200, keyCode)
        assertTrue(JSONObject(keyBody).optBoolean("ok"))
    }

    @Test
    fun testBackSucceeds() {
        val (backCode, backBody) = sendRequest("POST", "/back")
        assertEquals(200, backCode)
        assertTrue(JSONObject(backBody).optBoolean("ok"))
    }

    private fun sendRequestBytes(method: String, path: String): Pair<Int, ByteArray> {
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
        val future = executor.submit<Pair<Int, ByteArray>> {
            val url = URL("http://127.0.0.1:$testPort$path")
            val conn = url.openConnection() as HttpURLConnection
            conn.requestMethod = method
            conn.setRequestProperty("Authorization", "Bearer $testToken")
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val bytes = stream?.readBytes() ?: ByteArray(0)
            Pair(code, bytes)
        }
        while (!future.isDone) {
            org.robolectric.shadows.ShadowLooper.idleMainLooper()
            Thread.sleep(10)
        }
        val result = future.get()
        executor.shutdown()
        return result
    }

    private fun assertPngHeader(bytes: ByteArray) {
        assertTrue("PNG too short: ${bytes.size}", bytes.size > 24)
        assertEquals(0x89.toByte(), bytes[0])
        assertEquals(0x50.toByte(), bytes[1]) // P
        assertEquals(0x4E.toByte(), bytes[2]) // N
        assertEquals(0x47.toByte(), bytes[3]) // G
    }

    private fun pngWidth(bytes: ByteArray): Int =
        ((bytes[16].toInt() and 0xFF) shl 24) or ((bytes[17].toInt() and 0xFF) shl 16) or
            ((bytes[18].toInt() and 0xFF) shl 8) or (bytes[19].toInt() and 0xFF)

    private fun pngHeight(bytes: ByteArray): Int =
        ((bytes[20].toInt() and 0xFF) shl 24) or ((bytes[21].toInt() and 0xFF) shl 16) or
            ((bytes[22].toInt() and 0xFF) shl 8) or (bytes[23].toInt() and 0xFF)

    @Test
    fun testScreenshotWithRegionAndScaleReturnsCroppedPng() {
        val (code, bytes) = sendRequestBytes("GET", "/screenshot?region=0,0,100,100&scale=1.0")
        assertEquals(200, code)
        assertPngHeader(bytes)
        assertEquals(100, pngWidth(bytes))
        assertEquals(100, pngHeight(bytes))
    }

    @Test
    fun testScreenshotScaledPngIsSmallerThanFullSize() {
        val (fullCode, full) = sendRequestBytes("GET", "/screenshot?scale=1.0")
        val (smallCode, small) = sendRequestBytes("GET", "/screenshot?region=0,0,200,200&scale=0.25")
        assertEquals(200, fullCode)
        assertEquals(200, smallCode)
        assertPngHeader(small)
        assertTrue("cropped+scaled must be smaller than full", small.size < full.size)
        assertEquals(50, pngWidth(small))
        assertEquals(50, pngHeight(small))
    }

    @Test
    fun testScreenshotWithMalformedRegionReturns400() {
        val (code, body) = sendRequest("GET", "/screenshot?region=foo")
        assertEquals(400, code)
        assertEquals("BAD_REQUEST", JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun testScreenshotWithZeroWidthRegionReturns400() {
        val (code, body) = sendRequest("GET", "/screenshot?region=0,0,0,100")
        assertEquals(400, code)
        assertEquals("BAD_REQUEST", JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun testScreenshotWithOutOfBoundsRegionClamps() {
        val (code, bytes) = sendRequestBytes("GET", "/screenshot?region=900,1800,500,500&scale=1.0")
        assertEquals(200, code)
        assertPngHeader(bytes)
        assertTrue(pngWidth(bytes) in 1..500)
        assertTrue(pngHeight(bytes) in 1..500)
    }
}
