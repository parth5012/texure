package com.texure.bridge.intent

import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.net.Uri
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import com.texure.bridge.tree.BridgeTreeExtractor
import fi.iki.elonen.NanoHTTPD
import org.json.JSONArray
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
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeIntentDispatcherTest {

    private lateinit var activity: Activity
    private lateinit var treeExtractor: BridgeTreeExtractor
    private lateinit var dispatcher: BridgeIntentDispatcher
    private lateinit var server: BridgeServer
    private val testPort = 8769
    private val testToken = "test-token-84"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        treeExtractor = BridgeTreeExtractor()

        // Register a fake target activity in package manager
        val pm = activity.packageManager
        val shadowPm = shadowOf(pm)
        val targetIntent = Intent(Intent.ACTION_VIEW, Uri.parse("texure://sample/profile")).apply {
            `package` = activity.packageName
        }
        val resolveInfo = ResolveInfo().apply {
            activityInfo = ActivityInfo().apply {
                packageName = activity.packageName
                name = "com.texure.sample.ProfileActivity"
                applicationInfo = ApplicationInfo().apply {
                    packageName = activity.packageName
                }
            }
        }
        shadowPm.addResolveInfoForIntent(targetIntent, resolveInfo)

        dispatcher = BridgeIntentDispatcher(
            context = activity,
            treeExtractor = treeExtractor,
            broadcastTreeChanged = {}
        )

        val tokenManager = BridgeTokenManager(activity)
        tokenManager.saveToken(testToken)

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity },
            treeExtractor = treeExtractor
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
    fun testDispatchValidDeepLinkSetsPackageAndBumpsRev() {
        val initialRev = treeExtractor.getRev()
        val payload = JSONObject().apply {
            put("action", "android.intent.action.VIEW")
            put("uri", "texure://sample/profile")
            put("extras", JSONObject().apply {
                put("userId", 42)
                put("name", "Alice")
                put("verified", true)
                put("score", 98.5)
            })
            put("flags", JSONArray().apply {
                put("FLAG_ACTIVITY_SINGLE_TOP")
            })
        }

        val result = dispatcher.dispatch(payload)
        assertTrue("Expected Success result, got $result", result is IntentDispatchResult.Success)

        val success = result as IntentDispatchResult.Success
        assertEquals(activity.packageName, success.pkg)
        assertTrue(treeExtractor.getRev() > initialRev)

        val shadowApp = shadowOf(activity.application)
        val startedIntent = shadowApp.nextStartedActivity
        assertNotNull("Activity should have been started", startedIntent)
        assertEquals(activity.packageName, startedIntent.`package`)
        assertEquals(42, startedIntent.getIntExtra("userId", -1))
        assertEquals("Alice", startedIntent.getStringExtra("name"))
        assertEquals(true, startedIntent.getBooleanExtra("verified", false))
        assertEquals(98.5, startedIntent.getDoubleExtra("score", 0.0), 0.001)
        assertTrue((startedIntent.flags and Intent.FLAG_ACTIVITY_NEW_TASK) != 0)
        assertTrue((startedIntent.flags and Intent.FLAG_ACTIVITY_SINGLE_TOP) != 0)
    }

    @Test
    fun testCrossPackageIntentIsRejectedWith400() {
        val payload = JSONObject().apply {
            put("uri", "texure://sample/profile")
            put("package", "com.external.malicious")
        }

        val result = dispatcher.dispatch(payload)
        assertTrue("Expected Error result, got $result", result is IntentDispatchResult.Error)
        val error = result as IntentDispatchResult.Error
        assertEquals("CROSS_PACKAGE_FORBIDDEN", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testUnresolvableActivityReturns400ActivityNotFound() {
        val payload = JSONObject().apply {
            put("uri", "texure://unregistered/nowhere")
        }

        val result = dispatcher.dispatch(payload)
        assertTrue("Expected Error result, got $result", result is IntentDispatchResult.Error)
        val error = result as IntentDispatchResult.Error
        assertEquals("ACTIVITY_NOT_FOUND", error.code)
        assertEquals(400, error.httpStatus)
    }

    @Test
    fun testIntentWithCategories() {
        val catIntent = Intent(Intent.ACTION_VIEW, Uri.parse("texure://sample/profile")).apply {
            `package` = activity.packageName
            addCategory(Intent.CATEGORY_BROWSABLE)
            addCategory(Intent.CATEGORY_DEFAULT)
        }
        val resolveInfo = ResolveInfo().apply {
            activityInfo = ActivityInfo().apply {
                packageName = activity.packageName
                name = "com.texure.sample.ProfileActivity"
                applicationInfo = ApplicationInfo().apply {
                    packageName = activity.packageName
                }
            }
        }
        shadowOf(activity.packageManager).addResolveInfoForIntent(catIntent, resolveInfo)

        val payload = JSONObject().apply {
            put("action", Intent.ACTION_VIEW)
            put("uri", "texure://sample/profile")
            put("categories", org.json.JSONArray().apply {
                put(Intent.CATEGORY_BROWSABLE)
                put(Intent.CATEGORY_DEFAULT)
            })
        }
        val result = dispatcher.dispatch(payload)
        assertTrue(result is IntentDispatchResult.Success)

        val startedIntent = shadowOf(activity).nextStartedActivity
        assertNotNull(startedIntent)
        assertTrue(startedIntent.hasCategory(Intent.CATEGORY_BROWSABLE))
        assertTrue(startedIntent.hasCategory(Intent.CATEGORY_DEFAULT))
    }

    @Test
    fun testServerPostIntentEndpointWithAuth() {
        val url = URL("http://127.0.0.1:$testPort/intent")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.setRequestProperty("Authorization", "Bearer $testToken")
        conn.setRequestProperty("Content-Type", "application/json")
        conn.doOutput = true

        val body = JSONObject().apply {
            put("uri", "texure://sample/profile")
            put("extras", JSONObject().apply {
                put("source", "test")
            })
        }
        conn.outputStream.use { it.write(body.toString().toByteArray()) }

        assertEquals(200, conn.responseCode)
        val responseText = conn.inputStream.bufferedReader().use { it.readText() }
        val json = JSONObject(responseText)
        assertTrue(json.getBoolean("ok"))
        assertEquals(activity.packageName, json.getString("package"))
    }

    @Test
    fun testServerPostIntentWithoutAuthFails401() {
        val url = URL("http://127.0.0.1:$testPort/intent")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.setRequestProperty("Content-Type", "application/json")
        conn.doOutput = true
        conn.outputStream.use { it.write("{}".toByteArray()) }

        assertEquals(401, conn.responseCode)
    }
}
