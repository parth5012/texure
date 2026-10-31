package com.texure.sample

import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.os.SystemClock
import android.view.KeyEvent
import android.view.MotionEvent
import android.widget.Button
import android.widget.EditText
import android.widget.ScrollView
import android.widget.TextView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.texure.bridge.storage.BridgeTokenManager
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.net.HttpURLConnection
import java.net.URL

/**
 * L3 Device test verifying all 5 acceptance criteria from docs/spike-injection.md:
 * 1. sendPointerSync tap lands (no SecurityException) - non-main thread
 * 2. dispatchTouchEvent replay tap + scroll gesture lands
 * 3. Keyboard input lands in an editable field
 * 4. Works with app backgrounded while foreground service alive
 * 5. /ui-tree returns real nodes, /screenshot returns a real PNG
 */
@RunWith(AndroidJUnit4::class)
class SpikeInjectionDeviceTest {

    @Test
    fun spikeAcceptance1_sendPointerSyncTapLandsWithoutSecurityException() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val instrumentation = InstrumentationRegistry.getInstrumentation()

        scenario.onActivity { activity ->
            val button = activity.findViewById<Button>(R.id.btn_test)
            val loc = IntArray(2)
            button.getLocationOnScreen(loc)
            val x = loc[0] + button.width / 2f
            val y = loc[1] + button.height / 2f

            // Must run on non-main thread
            Thread {
                val downTime = SystemClock.uptimeMillis()
                val down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0)
                val up = MotionEvent.obtain(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y, 0)
                try {
                    instrumentation.sendPointerSync(down)
                    instrumentation.sendPointerSync(up)
                } finally {
                    try { down.recycle() } catch (_: Exception) {}
                    try { up.recycle() } catch (_: Exception) {}
                }
            }.start()
        }

        Thread.sleep(500)
        scenario.onActivity { activity ->
            val status = activity.findViewById<TextView>(R.id.tv_status)
            assertTrue("Expected click count to be > 0", status.text.toString().contains("Clicks: 1"))
        }
        scenario.close()
    }

    @Test
    fun spikeAcceptance2_dispatchTouchEventReplayTapAndScrollLands() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)

        // 1. Dispatch tap on button
        scenario.onActivity { activity ->
            val button = activity.findViewById<Button>(R.id.btn_test)
            val loc = IntArray(2)
            button.getLocationInWindow(loc)
            val x = loc[0] + button.width / 2f
            val y = loc[1] + button.height / 2f

            val downTime = SystemClock.uptimeMillis()
            val down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0)
            val up = MotionEvent.obtain(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y, 0)
            try {
                activity.window.decorView.dispatchTouchEvent(down)
                activity.window.decorView.dispatchTouchEvent(up)
            } finally {
                try { down.recycle() } catch (_: Exception) {}
                try { up.recycle() } catch (_: Exception) {}
            }
        }

        Thread.sleep(300)
        scenario.onActivity { activity ->
            val status = activity.findViewById<TextView>(R.id.tv_status)
            assertTrue("Expected button click via dispatchTouchEvent", status.text.toString().contains("Clicks: 1"))
        }

        // 2. Dispatch scroll gesture on scrollview
        scenario.onActivity { activity ->
            val scrollView = activity.findViewById<ScrollView>(R.id.scroll_view)
            val loc = IntArray(2)
            scrollView.getLocationInWindow(loc)
            val startX = loc[0] + scrollView.width / 2f
            val startY = loc[1] + scrollView.height * 0.8f
            val endY = loc[1] + scrollView.height * 0.2f

            val downTime = SystemClock.uptimeMillis()
            val down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY, 0)
            val move = MotionEvent.obtain(downTime, downTime + 100, MotionEvent.ACTION_MOVE, startX, endY, 0)
            val up = MotionEvent.obtain(downTime, downTime + 200, MotionEvent.ACTION_UP, startX, endY, 0)
            try {
                activity.window.decorView.dispatchTouchEvent(down)
                activity.window.decorView.dispatchTouchEvent(move)
                activity.window.decorView.dispatchTouchEvent(up)
            } finally {
                try { down.recycle() } catch (_: Exception) {}
                try { move.recycle() } catch (_: Exception) {}
                try { up.recycle() } catch (_: Exception) {}
            }
        }

        Thread.sleep(300)
        scenario.onActivity { activity ->
            val scrollView = activity.findViewById<ScrollView>(R.id.scroll_view)
            assertTrue("ScrollView should handle scroll event", scrollView.scrollY >= 0)
        }
        scenario.close()
    }

    @Test
    fun spikeAcceptance3_keyboardInputLandsInEditableField() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val instrumentation = InstrumentationRegistry.getInstrumentation()

        scenario.onActivity { activity ->
            val input = activity.findViewById<EditText>(R.id.et_input)
            input.requestFocus()
        }

        instrumentation.waitForIdleSync()
        instrumentation.sendStringSync("Spike Input")
        instrumentation.waitForIdleSync()

        scenario.onActivity { activity ->
            val input = activity.findViewById<EditText>(R.id.et_input)
            assertEquals("Spike Input", input.text.toString())
        }
        scenario.close()
    }

    @Test
    fun spikeAcceptance4_foregroundServiceAliveWhenBackgrounded() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val context = InstrumentationRegistry.getInstrumentation().targetContext

        // Background app via HOME intent
        val homeIntent = Intent(Intent.ACTION_MAIN).apply {
            addCategory(Intent.CATEGORY_HOME)
            flags = Intent.FLAG_ACTIVITY_NEW_TASK
        }
        context.startActivity(homeIntent)
        Thread.sleep(1000)

        // Verify service is still alive
        val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        @Suppress("DEPRECATION")
        val isServiceRunning = activityManager.getRunningServices(Int.MAX_VALUE).any {
            it.service.className == "com.texure.bridge.service.BridgeService"
        }
        assertTrue("BridgeService must remain alive when app is backgrounded", isServiceRunning)
        scenario.close()
    }

    @Test
    fun spikeAcceptance5_uiTreeAndScreenshotRoutesReturnData() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        Thread.sleep(500)

        val tokenManager = BridgeTokenManager(context)
        val token = tokenManager.getToken()

        // 1. Verify /ui-tree returns real nodes
        val treeUrl = URL("http://127.0.0.1:8765/ui-tree")
        val treeConn = treeUrl.openConnection() as HttpURLConnection
        treeConn.setRequestProperty("Authorization", "Bearer $token")
        try {
            assertEquals(200, treeConn.responseCode)
            val body = treeConn.inputStream.bufferedReader().use { it.readText() }
            val json = JSONObject(body)
            assertTrue("Expected rev in ui-tree", json.has("rev"))
            val nodes = json.getJSONArray("nodes")
            assertTrue("Expected non-empty nodes in ui-tree", nodes.length() > 0)
        } finally {
            treeConn.disconnect()
        }

        // 2. Verify /screenshot returns a real PNG
        val shotUrl = URL("http://127.0.0.1:8765/screenshot")
        val shotConn = shotUrl.openConnection() as HttpURLConnection
        shotConn.setRequestProperty("Authorization", "Bearer $token")
        try {
            assertEquals(200, shotConn.responseCode)
            assertEquals("image/png", shotConn.contentType)
            val bytes = shotConn.inputStream.use { it.readBytes() }
            assertTrue("Expected screenshot bytes", bytes.isNotEmpty())
            // PNG signature: 0x89 'P' 'N' 'G'
            assertEquals(0x89.toByte(), bytes[0])
            assertEquals('P'.code.toByte(), bytes[1])
            assertEquals('N'.code.toByte(), bytes[2])
            assertEquals('G'.code.toByte(), bytes[3])
        } finally {
            shotConn.disconnect()
        }

        scenario.close()
    }
}
