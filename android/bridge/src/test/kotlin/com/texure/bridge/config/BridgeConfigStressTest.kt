package com.texure.bridge.config

import android.app.Activity
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.view.View
import androidx.appcompat.app.AppCompatDelegate
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import com.texure.bridge.tree.BridgeTreeExtractor
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
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeConfigStressTest {

    private lateinit var activity: Activity
    private lateinit var treeExtractor: BridgeTreeExtractor
    private lateinit var configStress: BridgeConfigStress
    private lateinit var server: BridgeServer
    private val testPort = 8780
    private val testToken = "test-token-stress"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        treeExtractor = BridgeTreeExtractor()
        configStress = BridgeConfigStress(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val tokenManager = BridgeTokenManager(activity).apply {
            saveToken(testToken)
        }

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity },
            treeExtractor = treeExtractor,
            configStress = configStress
        )
        server.start()
    }

    @After
    fun tearDown() {
        if (::server.isInitialized) {
            server.stop()
        }
    }

    @Test
    fun testFontScaleOverrideAndReset() {
        val initialFontScale = activity.resources.configuration.fontScale
        val initialRev = treeExtractor.getRev()

        // 1. POST /config/stress with font_scale = 1.5
        val reqJson = JSONObject().apply {
            put("font_scale", 1.5)
        }
        val (code, response) = executeRequest("POST", "/config/stress", reqJson.toString())
        assertEquals(200, code)
        assertTrue(response.getBoolean("ok"))
        assertEquals(1.5, response.getDouble("font_scale"), 0.01)
        assertTrue(response.getLong("rev") > initialRev)

        assertEquals(1.5f, activity.resources.configuration.fontScale, 0.01f)

        // 2. DELETE /config/stress
        val (delCode, delResponse) = executeRequest("DELETE", "/config/stress")
        assertEquals(200, delCode)
        assertTrue(delResponse.getBoolean("ok"))
        assertTrue(delResponse.getBoolean("reset"))

        assertEquals(initialFontScale, activity.resources.configuration.fontScale, 0.01f)
    }

    @Test
    fun testDarkModeAndLocaleOverride() {
        // Apply dark_mode = true and RTL locale ar-AE
        val reqJson = JSONObject().apply {
            put("dark_mode", true)
            put("locale", "ar-AE")
            put("orientation", "LANDSCAPE")
        }
        val (code, response) = executeRequest("POST", "/config/stress", reqJson.toString())
        assertEquals(200, code)
        assertTrue(response.getBoolean("ok"))
        assertEquals(true, response.getBoolean("dark_mode"))
        assertEquals("ar-AE", response.getString("locale"))
        assertEquals("LANDSCAPE", response.getString("orientation"))

        val config = activity.resources.configuration
        assertEquals(Configuration.UI_MODE_NIGHT_YES, config.uiMode and Configuration.UI_MODE_NIGHT_MASK)
        assertEquals(View.LAYOUT_DIRECTION_RTL, config.layoutDirection)
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE, activity.requestedOrientation)

        // Reset
        val (delCode, delResponse) = executeRequest("DELETE", "/config/stress")
        assertEquals(200, delCode)
        assertTrue(delResponse.getBoolean("reset"))

        val resetConfig = activity.resources.configuration
        assertEquals(Configuration.UI_MODE_NIGHT_NO, resetConfig.uiMode and Configuration.UI_MODE_NIGHT_MASK)
    }

    @Test
    fun testValidationBounds() {
        // Out of bounds font_scale (< 0.5)
        val badScale1 = JSONObject().apply { put("font_scale", 0.1) }
        val (code1, res1) = executeRequest("POST", "/config/stress", badScale1.toString())
        assertEquals(400, code1)
        assertEquals("BAD_REQUEST", res1.getJSONObject("error").getString("code"))

        // Out of bounds font_scale (> 3.0)
        val badScale2 = JSONObject().apply { put("font_scale", 4.0) }
        val (code2, res2) = executeRequest("POST", "/config/stress", badScale2.toString())
        assertEquals(400, code2)
        assertEquals("BAD_REQUEST", res2.getJSONObject("error").getString("code"))

        // Invalid orientation
        val badOrient = JSONObject().apply { put("orientation", "DIAGONAL") }
        val (code3, res3) = executeRequest("POST", "/config/stress", badOrient.toString())
        assertEquals(400, code3)
        assertEquals("BAD_REQUEST", res3.getJSONObject("error").getString("code"))

        // Empty body
        val emptyBody = JSONObject()
        val (code4, res4) = executeRequest("POST", "/config/stress", emptyBody.toString())
        assertEquals(400, code4)
        assertEquals("BAD_REQUEST", res4.getJSONObject("error").getString("code"))
    }

    private fun executeRequest(method: String, path: String, body: String? = null): Pair<Int, JSONObject> {
        val url = URL("http://127.0.0.1:$testPort$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = method
        conn.setRequestProperty("Authorization", "Bearer $testToken")
        conn.setRequestProperty("Content-Type", "application/json")

        if (body != null) {
            conn.doOutput = true
            conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
        }

        val code = conn.responseCode
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream
        val responseText = stream?.bufferedReader()?.use { it.readText() } ?: "{}"
        return Pair(code, JSONObject(responseText))
    }
}
