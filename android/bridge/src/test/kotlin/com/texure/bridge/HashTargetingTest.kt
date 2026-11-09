package com.texure.bridge

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
 * Map #36 #29: verify-at-execute targeting by semantic hash on /tap and /type.
 *
 * - Hash + correct index: dispatches at that index.
 * - Hash + stale/wrong index: falls back to resolving by hash across the tree.
 * - Hash only (no index): resolves by hash.
 * - Unknown hash: NODE_OUTDATED (never a blind dispatch onto a "similar" node).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class HashTargetingTest {

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

    private fun currentTree(): JSONObject {
        val (code, body) = sendRequest("GET", "/ui-tree")
        assertEquals(200, code)
        return JSONObject(body)
    }

    private fun buttonHash(tree: JSONObject): String {
        val nodes = tree.getJSONArray("nodes")
        for (i in 0 until nodes.length()) {
            val node = nodes.getJSONObject(i)
            if (node.optString("text") == "Click Me") {
                val hash = node.optString("hash")
                assertTrue("node must carry hash", hash.isNotEmpty())
                return hash
            }
        }
        error("button not in tree: $tree")
    }

    @Test
    fun uiTreeNodesCarryHash() {
        val tree = currentTree()
        val nodes = tree.getJSONArray("nodes")
        assertTrue("Expected non-empty nodes", nodes.length() > 0)
        for (i in 0 until nodes.length()) {
            val hash = nodes.getJSONObject(i).optString("hash")
            assertTrue(
                "node $i must carry 16-char hex hash, was '$hash'",
                hash.matches(Regex("[0-9a-f]{16}"))
            )
        }
    }

    @Test
    fun tapWithHashAndCorrectIndexSucceeds() {
        val tree = currentTree()
        val rev = tree.getLong("rev")
        val payload = JSONObject().apply {
            put("node", 0)
            put("hash", buttonHash(tree))
            put("rev", rev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = payload)
        assertEquals("Failed with body: $tapBody", 200, tapCode)
        assertTrue(JSONObject(tapBody).optBoolean("ok"))
    }

    @Test
    fun tapWithStaleIndexFallsBackToHashResolution() {
        val tree = currentTree()
        val rev = tree.getLong("rev")
        val hash = buttonHash(tree)
        // Index 1 is the EditText (not clickable as this selector intends);
        // the stale index must not dispatch a blind tap — hash resolution wins.
        val payload = JSONObject().apply {
            put("node", 1)
            put("hash", hash)
            put("rev", rev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = payload)
        assertEquals("Failed with body: $tapBody", 200, tapCode)
        assertTrue(JSONObject(tapBody).optBoolean("ok"))
    }

    @Test
    fun tapWithHashOnlySucceeds() {
        val tree = currentTree()
        val rev = tree.getLong("rev")
        val payload = JSONObject().apply {
            put("hash", buttonHash(tree))
            put("rev", rev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = payload)
        assertEquals("Failed with body: $tapBody", 200, tapCode)
        assertTrue(JSONObject(tapBody).optBoolean("ok"))
    }

    @Test
    fun tapWithUnknownHashReturnsNodeOutdated() {
        val tree = currentTree()
        val rev = tree.getLong("rev")
        val payload = JSONObject().apply {
            put("node", 0)
            put("hash", "0000000000000000")
            put("rev", rev)
        }.toString()

        val (tapCode, tapBody) = sendRequest("POST", "/tap", body = payload)
        assertEquals(400, tapCode)
        val errorJson = JSONObject(tapBody)
        assertEquals(BridgeError.NODE_OUTDATED.name, errorJson.getJSONObject("error").getString("code"))
    }

    @Test
    fun typeWithHashOnlySucceeds() {
        val tree = currentTree()
        val rev = tree.getLong("rev")
        val nodes = tree.getJSONArray("nodes")
        var inputHash: String? = null
        for (i in 0 until nodes.length()) {
            val node = nodes.getJSONObject(i)
            if (node.optBoolean("editable")) {
                inputHash = node.optString("hash")
            }
        }
        assertTrue("editable node must carry hash", !inputHash.isNullOrEmpty())

        val payload = JSONObject().apply {
            put("hash", inputHash)
            put("text", "hello world")
            put("rev", rev)
        }.toString()

        val (typeCode, typeBody) = sendRequest("POST", "/type", body = payload)
        assertEquals("Failed with body: $typeBody", 200, typeCode)
        assertTrue(JSONObject(typeBody).optBoolean("ok"))
    }
}
