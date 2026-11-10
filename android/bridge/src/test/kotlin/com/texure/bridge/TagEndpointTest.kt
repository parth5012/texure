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
import org.junit.Assert.assertNotEquals
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
 * Map #36 #31: POST /tag dynamic view tagging.
 *
 * - Tagging a node surfaces the tag as its `/ui-tree` `id` on the next extraction.
 * - The tag survives in-session re-extractions (View object persists).
 * - A successful tag mutation bumps rev; stale rev / out-of-range index
 *   refuse with NODE_OUTDATED; blank tag / missing field refuse 400 BAD_REQUEST.
 * - Tagging rotates the tagged node's hash (tag is the `id`, a #29 hash input).
 * - The tagged node is findable via ?subtree=<tag> (targeting without rebuild).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TagEndpointTest {

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

    private fun tagPayload(index: Int, rev: Long, tag: String): String {
        return JSONObject().apply {
            put("target", JSONObject().apply {
                put("index", index)
                put("rev", rev)
            })
            put("tag", tag)
        }.toString()
    }

    @Test
    fun tagSurfacesAsIdInSubsequentUiTreeAndBumpsRev() {
        val before = currentTree()
        val revBefore = before.getLong("rev")

        val (code, body) = sendRequest("POST", "/tag", body = tagPayload(0, revBefore, "my_custom_tag"))
        assertEquals("Failed with body: $body", 200, code)
        assertTrue(JSONObject(body).optBoolean("ok"))

        val after = currentTree()
        assertTrue("rev must bump on tag mutation", after.getLong("rev") > revBefore)

        val nodes = after.getJSONArray("nodes")
        var found = false
        for (i in 0 until nodes.length()) {
            if (nodes.getJSONObject(i).optString("id") == "my_custom_tag") {
                found = true
            }
        }
        assertTrue("tagged node must surface id == my_custom_tag, got: $after", found)
    }

    @Test
    fun tagSurvivesReExtraction() {
        val rev = currentTree().getLong("rev")
        val (code, _) = sendRequest("POST", "/tag", body = tagPayload(0, rev, "persistent_tag"))
        assertEquals(200, code)

        for (round in 1..2) {
            val tree = currentTree()
            val nodes = tree.getJSONArray("nodes")
            var found = false
            for (i in 0 until nodes.length()) {
                if (nodes.getJSONObject(i).optString("id") == "persistent_tag") {
                    found = true
                }
            }
            assertTrue("tag must survive re-extraction round $round", found)
        }
    }

    @Test
    fun tagWithStaleRevReturnsNodeOutdated() {
        val payload = tagPayload(0, 999999L, "my_custom_tag")
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals(BridgeError.NODE_OUTDATED.name, JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun tagWithOutOfRangeIndexReturnsNodeOutdated() {
        val rev = currentTree().getLong("rev")
        val payload = tagPayload(999, rev, "my_custom_tag")
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals(BridgeError.NODE_OUTDATED.name, JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun tagWithNegativeIndexReturnsNodeOutdated() {
        val rev = currentTree().getLong("rev")
        val payload = tagPayload(-1, rev, "my_custom_tag")
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals(BridgeError.NODE_OUTDATED.name, JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun tagWithBlankTagReturnsBadRequest() {
        val rev = currentTree().getLong("rev")
        val payload = tagPayload(0, rev, "   ")
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals("BAD_REQUEST", JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun tagWithOversizedTagReturnsBadRequest() {
        val rev = currentTree().getLong("rev")
        val payload = tagPayload(0, rev, "a".repeat(257))
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals("BAD_REQUEST", JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun tagWithMissingFieldReturnsBadRequest() {
        val payload = JSONObject().apply {
            put("tag", "my_custom_tag")
        }.toString()
        val (code, body) = sendRequest("POST", "/tag", body = payload)
        assertEquals(400, code)
        assertEquals("BAD_REQUEST", JSONObject(body).getJSONObject("error").getString("code"))
    }

    @Test
    fun taggingRotatesTaggedNodeHash() {
        val before = currentTree()
        val rev = before.getLong("rev")
        val hashBefore = before.getJSONArray("nodes").getJSONObject(0).optString("hash")

        val (code, _) = sendRequest("POST", "/tag", body = tagPayload(0, rev, "hash_tag"))
        assertEquals(200, code)

        val after = currentTree()
        val nodes = after.getJSONArray("nodes")
        var hashAfter: String? = null
        for (i in 0 until nodes.length()) {
            val node = nodes.getJSONObject(i)
            if (node.optString("id") == "hash_tag") {
                hashAfter = node.optString("hash")
            }
        }
        assertTrue("tagged node must be in tree", hashAfter != null)
        assertNotEquals("tagging is an identity change: hash must rotate", hashBefore, hashAfter)
    }

    @Test
    fun taggedNodeIsFindableViaSubtreeQuery() {
        val rev = currentTree().getLong("rev")
        val (code, _) = sendRequest("POST", "/tag", body = tagPayload(0, rev, "subtree_target"))
        assertEquals(200, code)

        val (subCode, subBody) = sendRequest("GET", "/ui-tree?subtree=subtree_target")
        assertEquals(200, subCode)
        val nodes = JSONObject(subBody).getJSONArray("nodes")
        assertTrue("subtree query must return the tagged node", nodes.length() > 0)
        assertEquals("subtree_target", nodes.getJSONObject(0).optString("id"))
    }
}
