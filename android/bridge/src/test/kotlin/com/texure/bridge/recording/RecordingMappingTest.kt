package com.texure.bridge.recording

import android.app.Activity
import android.view.MotionEvent
import android.view.KeyEvent
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.texure.bridge.tree.BridgeTreeExtractor
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.concurrent.atomic.AtomicLong

/**
 * BB-B13 / BB-B14: passive capture + event mapping (tap / type / keypress / pause).
 * Controller is driven directly; the same code path serves the HTTP routes.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class RecordingMappingTest {

    private lateinit var activity: Activity
    private lateinit var testButton: Button
    private lateinit var testInput: EditText
    private val fakeTime = AtomicLong(1_000L)

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
    }

    private fun newController(extractor: BridgeTreeExtractor = BridgeTreeExtractor()): RecordingController {
        return RecordingController(
            activityProvider = { activity },
            treeExtractor = extractor,
            timeSource = { fakeTime.get() }
        )
    }

    /** Runs the block on the Robolectric main thread (touch dispatch must be on main). */
    private fun onMain(block: () -> Unit) {
        val handler = android.os.Handler(android.os.Looper.getMainLooper())
        handler.post { block() }
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
    }

    private fun dispatchTap(target: android.view.View, atTime: Long = fakeTime.get()) {
        fakeTime.set(atTime)
        val loc = IntArray(2)
        target.getLocationOnScreen(loc)
        val x = loc[0] + target.width / 2f
        val y = loc[1] + target.height / 2f
        val decor = activity.window.decorView
        val downTime = fakeTime.get()
        decor.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0)
        )
        fakeTime.set(atTime + 40)
        decor.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, x, y, 0)
        )
    }

    private fun actionsOf(controller: RecordingController): List<JSONObject> {
        val arr = controller.export().getJSONArray("actions")
        return (0 until arr.length()).map { arr.getJSONObject(it) }
    }

    // BB-B14: tap maps to the targeted node (text/index/rev) with t offset
    @Test
    fun tapIsRecordedWithNodeMapping() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()

        onMain { dispatchTap(testButton, 1_000) }
        val actions = controller.export().getJSONArray("actions")
        assertEquals(1, actions.length())
        val tap = actions.getJSONObject(0)
        assertEquals("tap", tap.getString("action"))
        val node = tap.getJSONObject("node")
        assertEquals("Click Me", node.getString("text"))
        assertTrue("node must carry the tree rev", node.getLong("rev") >= 1)
        assertTrue(tap.getLong("t") in 30..60)
        assertTrue("seq starts at 0", tap.getInt("seq") == 0)
    }

    // BB-B13: recording OFF = no interception, nothing recorded
    @Test
    fun recordingOffRecordsNothingAndDoesNotAttach() {
        val controller = newController(BridgeTreeExtractor())
        val decor = activity.window.decorView
        val childCountBefore = (decor as android.view.ViewGroup).childCount

        onMain { dispatchTap(testButton) }

        assertTrue(!controller.isRecording())
        assertEquals(0, controller.count())
        assertEquals("no capture view attached while idle", childCountBefore, decor.childCount)
    }

    // BB-B14: type committed text on the tapped editable node
    @Test
    fun typeCommitIsRecorded() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()

        onMain {
            dispatchTap(testInput, 1_000)   // focuses editable, watcher attaches
            testInput.append("hello")       // human typing
        }
        onMain { dispatchTap(testButton, 2_000) } // commit point

        val actions = actionsOf(controller)
        val typeAction = actions.firstOrNull { it.getString("action") == "type" }
        assertTrue("type action must be recorded", typeAction != null)
        assertEquals("hello", typeAction!!.getString("value"))
        val node = typeAction.getJSONObject("node")
        assertTrue("type node must be the EditText", node.getString("cls").contains("EditText"))
        val tapIndex = actionsOf(controller).indexOfFirst { it.getString("action") == "tap" && it.getJSONObject("node").optString("cls").contains("Button") }
        assertTrue("type must be recorded before the committing tap", typeAction.getLong("t") <= actionsOf(controller)[tapIndex].getLong("t"))
    }

    // BB-B14: ENTER in text commit maps to keypress ENTER
    @Test
    fun enterInTextCommitIsRecordedAsKeypress() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()

        onMain {
            dispatchTap(testInput, 1_000)
            testInput.append("hello\n")
        }

        val actions = actionsOf(controller)
        assertTrue(actions.any { it.getString("action") == "keypress" && it.getString("key") == "ENTER" })
        val typeAction = actions.firstOrNull { it.getString("action") == "type" }
        assertTrue(typeAction != null)
        assertEquals("hello", typeAction!!.getString("value"))
    }

    // BB-B14: BACK keypress via own-window key hook. Dispatch route mirrors the system
    // input path (ViewRootImpl -> DecorView -> window callback), not Activity.dispatchKeyEvent,
    // which falls back to the Activity object and bypasses the window callback.
    @Test
    fun backKeypressIsRecorded() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()
        onMain {}

        val decor = activity.window.decorView
        onMain {
            decor.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            decor.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }

        val actions = actionsOf(controller)
        assertTrue(
            "BACK must be recorded as keypress",
            actions.any { it.getString("action") == "keypress" && it.getString("key") == "BACK" }
        )
    }

    // BB-B14: pause event between actions with a >= 2s inactivity gap
    @Test
    fun gapIsRecordedAsPause() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()

        onMain { dispatchTap(testButton, 1_000) }
        onMain { dispatchTap(testButton, 5_100) } // 4s gap after first tap (t=1040)

        val actions = actionsOf(controller)
        assertEquals(3, actions.size)
        val pause = actions[1]
        assertEquals("pause", pause.getString("action"))
        assertTrue("pause must cover the >= 2s gap", pause.getLong("ms") >= 3_500)
        assertEquals("pause precedes the delayed action", "tap", actions[2].getString("action"))
    }
}
