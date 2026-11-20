package com.texure.bridge.state

import android.app.Activity
import android.content.Context
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
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
class BridgeStatePrefsTest {

    private lateinit var activity: Activity
    private lateinit var server: BridgeServer
    private val testPort = 8765
    private val testToken = "test-token-state-prefs"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()

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
    fun statePrefsRequiresAuth() {
        val (codeGet, bodyGet) = sendRequest("GET", "/state/prefs", token = null)
        assertEquals(401, codeGet)
        assertEquals(
            BridgeError.AUTH_FAILED.name,
            JSONObject(bodyGet).getJSONObject("error").getString("code")
        )

        val (codePost, bodyPost) = sendRequest("POST", "/state/prefs", body = "{}", token = null)
        assertEquals(401, codePost)
        assertEquals(
            BridgeError.AUTH_FAILED.name,
            JSONObject(bodyPost).getJSONObject("error").getString("code")
        )
    }

    @Test
    fun getAndSetDefaultSharedPreferences() {
        // Seed via POST
        val postPayload = JSONObject().apply {
            put("set", JSONObject().apply {
                put("user_name", "Alice")
                put("is_logged_in", true)
                put("login_count", 42)
                put("large_id", 9876543210L)
                put("score", 99.5)
                put("tags", JSONArray().apply {
                    put("admin")
                    put("tester")
                })
            })
        }

        val (postCode, postResp) = sendRequest("POST", "/state/prefs", body = postPayload.toString())
        assertEquals(200, postCode)
        assertTrue(JSONObject(postResp).getBoolean("ok"))

        // Read via Android API directly
        val prefs = activity.getSharedPreferences("${activity.packageName}_preferences", Context.MODE_PRIVATE)
        assertEquals("Alice", prefs.getString("user_name", null))
        assertTrue(prefs.getBoolean("is_logged_in", false))
        assertEquals(42, prefs.getInt("login_count", 0))
        assertEquals(9876543210L, prefs.getLong("large_id", 0L))
        assertEquals(99.5f, prefs.getFloat("score", 0f), 0.01f)
        assertEquals(setOf("admin", "tester"), prefs.getStringSet("tags", null))

        // Read via GET /state/prefs
        val (getCode, getResp) = sendRequest("GET", "/state/prefs")
        assertEquals(200, getCode)
        val getJson = JSONObject(getResp)
        assertEquals("${activity.packageName}_preferences", getJson.getString("file"))
        val entries = getJson.getJSONObject("entries")
        assertEquals("Alice", entries.getString("user_name"))
        assertTrue(entries.getBoolean("is_logged_in"))
        assertEquals(42, entries.getInt("login_count"))
        assertEquals(9876543210L, entries.getLong("large_id"))
        assertEquals(99.5, entries.getDouble("score"), 0.01)

        val tagsArr = entries.getJSONArray("tags")
        val tagsSet = (0 until tagsArr.length()).map { tagsArr.getString(it) }.toSet()
        assertEquals(setOf("admin", "tester"), tagsSet)
    }

    @Test
    fun getAndSetNamedSharedPreferences() {
        val postPayload = JSONObject().apply {
            put("file", "user_session")
            put("set", JSONObject().apply {
                put("session_token", "xyz-789")
                put("active", true)
            })
        }

        val (postCode, postResp) = sendRequest("POST", "/state/prefs", body = postPayload.toString())
        assertEquals(200, postCode)
        assertTrue(JSONObject(postResp).getBoolean("ok"))

        val (getCode, getResp) = sendRequest("GET", "/state/prefs?name=user_session")
        assertEquals(200, getCode)
        val getJson = JSONObject(getResp)
        assertEquals("user_session", getJson.getString("file"))
        val entries = getJson.getJSONObject("entries")
        assertEquals("xyz-789", entries.getString("session_token"))
        assertTrue(entries.getBoolean("active"))
    }

    @Test
    fun removeSharedPreferencesKey() {
        // Pre-populate
        val prefs = activity.getSharedPreferences("settings", Context.MODE_PRIVATE)
        prefs.edit().putString("theme", "dark").putString("lang", "en").commit()

        val removePayload = JSONObject().apply {
            put("file", "settings")
            put("remove", JSONArray().apply {
                put("theme")
            })
        }

        val (postCode, postResp) = sendRequest("POST", "/state/prefs", body = removePayload.toString())
        assertEquals(200, postCode)
        assertTrue(JSONObject(postResp).getBoolean("ok"))

        assertFalse(prefs.contains("theme"))
        assertEquals("en", prefs.getString("lang", null))
    }

    @Test
    fun mutationBumpsTreeRevision() {
        val (treeCode1, treeResp1) = sendRequest("GET", "/ui-tree")
        assertEquals(200, treeCode1)
        val rev1 = JSONObject(treeResp1).getLong("rev")

        val postPayload = JSONObject().apply {
            put("set", JSONObject().apply {
                put("key1", "val1")
            })
        }
        val (postCode, _) = sendRequest("POST", "/state/prefs", body = postPayload.toString())
        assertEquals(200, postCode)

        val (treeCode2, treeResp2) = sendRequest("GET", "/ui-tree")
        assertEquals(200, treeCode2)
        val rev2 = JSONObject(treeResp2).getLong("rev")
        assertTrue("Tree rev should bump on prefs mutation", rev2 > rev1)
    }
}
