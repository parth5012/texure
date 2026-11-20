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
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeStateSnapshotTest {

    private lateinit var activity: Activity
    private lateinit var server: BridgeServer
    private lateinit var tokenManager: BridgeTokenManager
    private val testPort = 8765
    private val testToken = "test-token-snapshot"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()

        tokenManager = BridgeTokenManager(activity)
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
        File(activity.filesDir, "texure_snapshots").deleteRecursively()
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
            conn.setRequestProperty("Connection", "close")
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
    fun resetAndSnapshotRoutesRequireAuth() {
        val (codeReset, bodyReset) = sendRequest("POST", "/state/reset", body = "{}", token = null)
        assertEquals(401, codeReset)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(bodyReset).getJSONObject("error").getString("code"))

        val (codeSave, bodySave) = sendRequest("POST", "/state/snapshots/save", body = "{\"name\":\"test\"}", token = null)
        assertEquals(401, codeSave)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(bodySave).getJSONObject("error").getString("code"))

        val (codeRestore, bodyRestore) = sendRequest("POST", "/state/snapshots/restore", body = "{\"name\":\"test\"}", token = null)
        assertEquals(401, codeRestore)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(bodyRestore).getJSONObject("error").getString("code"))

        val (codeList, bodyList) = sendRequest("GET", "/state/snapshots", token = null)
        assertEquals(401, codeList)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(bodyList).getJSONObject("error").getString("code"))
    }

    @Test
    fun dayZeroResetClearsStateWhilePreservingPairingToken() {
        // Pre-populate SharedPreferences
        val appPrefs = activity.getSharedPreferences("app_settings", Context.MODE_PRIVATE)
        appPrefs.edit().putString("onboarding_complete", "true").commit()

        // Pre-populate Database
        val db = activity.openOrCreateDatabase("user_cache_db", Context.MODE_PRIVATE, null)
        db.execSQL("CREATE TABLE items (id INT, name TEXT)")
        db.execSQL("INSERT INTO items VALUES (1, 'cached_item')")
        db.close()

        // Pre-populate Cache
        val cacheFile = File(activity.cacheDir, "temp_data.bin")
        cacheFile.writeText("some-cached-content")

        // Call POST /state/reset with scope = "all"
        val (code, resp) = sendRequest("POST", "/state/reset", body = "{\"scope\":\"all\"}")
        assertEquals(200, code)
        assertTrue(JSONObject(resp).getBoolean("ok"))
        assertEquals("all", JSONObject(resp).getString("reset"))

        // Verify preferences cleared
        val readPrefs = activity.getSharedPreferences("app_settings", Context.MODE_PRIVATE)
        assertFalse("App preferences should be cleared", readPrefs.contains("onboarding_complete"))

        // Verify databases cleared
        val dbList = activity.databaseList() ?: emptyArray()
        assertFalse("Databases should be deleted", dbList.contains("user_cache_db"))

        // Verify cache cleared
        assertFalse("Cache file should be deleted", cacheFile.exists())

        // Verify pairing token PRESERVED
        assertEquals("Pairing token must survive state reset", testToken, tokenManager.getToken())
        assertTrue("Token validation must succeed after reset", tokenManager.validateToken("Bearer $testToken"))
    }

    @Test
    fun saveAndRestoreStateSnapshot() {
        // 1. Setup initial state
        val appPrefs = activity.getSharedPreferences("app_state", Context.MODE_PRIVATE)
        appPrefs.edit().putString("stage", "post_onboarding").putInt("step", 5).commit()

        val db = activity.openOrCreateDatabase("profile_db", Context.MODE_PRIVATE, null)
        db.execSQL("CREATE TABLE profile (id INT, role TEXT)")
        db.execSQL("INSERT INTO profile VALUES (42, 'member')")
        db.close()

        // 2. Save snapshot "post_onboarding"
        val (saveCode, saveResp) = sendRequest("POST", "/state/snapshots/save", body = "{\"name\":\"post_onboarding\"}")
        assertEquals(200, saveCode)
        assertTrue(JSONObject(saveResp).getBoolean("ok"))
        assertEquals("post_onboarding", JSONObject(saveResp).getString("name"))

        // 3. Verify snapshot is listed
        val (listCode, listResp) = sendRequest("GET", "/state/snapshots")
        assertEquals(200, listCode)
        val snapshots = JSONObject(listResp).getJSONArray("snapshots")
        assertEquals(1, snapshots.length())
        assertEquals("post_onboarding", snapshots.getString(0))

        // 4. Mutate state
        appPrefs.edit().putString("stage", "modified_state").putInt("step", 99).commit()
        val db2 = activity.openOrCreateDatabase("profile_db", Context.MODE_PRIVATE, null)
        db2.execSQL("UPDATE profile SET role = 'corrupted' WHERE id = 42")
        db2.close()

        // 5. Restore snapshot
        val (restoreCode, restoreResp) = sendRequest("POST", "/state/snapshots/restore", body = "{\"name\":\"post_onboarding\"}")
        assertEquals(200, restoreCode)
        assertTrue(JSONObject(restoreResp).getBoolean("ok"))

        // 6. Verify state reverted
        // Verify database content
        val verifyDb = activity.openOrCreateDatabase("profile_db", Context.MODE_PRIVATE, null)
        val cursor = verifyDb.rawQuery("SELECT role FROM profile WHERE id = 42", null)
        assertTrue(cursor.moveToFirst())
        assertEquals("member", cursor.getString(0))
        cursor.close()
        verifyDb.close()

        // Verify restored SharedPreferences via GET /state/prefs
        val (prefsCode, prefsResp) = sendRequest("GET", "/state/prefs?name=app_state")
        assertEquals(200, prefsCode)
        val entries = JSONObject(prefsResp).getJSONObject("entries")
        assertEquals("post_onboarding", entries.getString("stage"))
        assertEquals(5, entries.getInt("step"))
    }

    @Test
    fun invalidSnapshotNameReturnsBadRequest() {
        val (code, resp) = sendRequest("POST", "/state/snapshots/save", body = "{\"name\":\"../path_traversal\"}")
        assertEquals(400, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("BAD_REQUEST", error.getString("code"))
    }

    @Test
    fun restoreNonExistentSnapshotReturnsNotFound() {
        val (code, resp) = sendRequest("POST", "/state/snapshots/restore", body = "{\"name\":\"unknown_snapshot\"}")
        assertEquals(404, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("SNAPSHOT_NOT_FOUND", error.getString("code"))
    }

    @Test
    fun corruptedSnapshotRestoreDoesNotWipeExistingData() {
        val sp = activity.getSharedPreferences("keep_me", Context.MODE_PRIVATE)
        sp.edit().putString("important", "safe_value").commit()

        // Create a corrupted zip file
        val snapshotsDir = File(activity.filesDir, "texure_snapshots")
        snapshotsDir.mkdirs()
        val corruptZip = File(snapshotsDir, "corrupted_snap.zip")
        corruptZip.writeBytes(byteArrayOf(0x50, 0x4b, 0x03, 0x04, 0x00, 0x00, 0x7f, 0x7f)) // invalid truncated zip

        val (code, resp) = sendRequest("POST", "/state/snapshots/restore", body = "{\"name\":\"corrupted_snap\"}")
        assertEquals(400, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("RESTORE_ERROR", error.getString("code"))

        // Verify existing state was preserved intact!
        assertEquals("safe_value", sp.getString("important", null))
    }

    @Test
    fun malformedResetBodyReturnsBadRequestWithoutWiping() {
        val sp = activity.getSharedPreferences("keep_me_too", Context.MODE_PRIVATE)
        sp.edit().putString("key", "value").commit()

        val (code, resp) = sendRequest("POST", "/state/reset", body = "{\"scope\": \"prefs\" unclosed")
        assertEquals(400, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("BAD_REQUEST", error.getString("code"))

        // Verify data was NOT wiped
        assertEquals("value", sp.getString("key", null))
    }
}
