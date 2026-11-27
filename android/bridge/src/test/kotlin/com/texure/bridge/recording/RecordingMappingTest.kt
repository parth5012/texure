package com.texure.bridge.recording

import android.app.Activity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import com.texure.bridge.injection.DefaultInjectionAdapter
import com.texure.bridge.tree.BridgeTreeExtractor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicLong
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

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

    // BB-B16: background start() followed by immediate main-thread stop() before looper drains
    // must not leave a stale Window.Callback intercepting touches after stop (primary pre-fix regression test).
    @Test
    fun backgroundStartThenImmediateStopLeavesNoStaleHook() {
        val controller = newController(BridgeTreeExtractor())
        val originalCallback = activity.window.callback
        val bg = Executors.newSingleThreadExecutor()
        try {
            bg.submit { controller.start() }.get()
        } finally {
            bg.shutdown()
        }

        // Stop immediately on main thread before queued installHooksOnMain runs
        val stoppedCount = controller.stop()
        assertEquals(0, stoppedCount)
        assertTrue(!controller.isRecording())

        // Drain queued main-looper install task
        org.robolectric.shadows.ShadowLooper.idleMainLooper()

        assertSame("window.callback must stay original after stop", originalCallback, activity.window.callback)
        onMain { dispatchTap(testButton, 2_000) }
        assertEquals(0, controller.count())
    }

    // BB-B16: start -> stop -> start across background and main looper must not clobber the
    // newer active session or double-wrap Window.Callback, and HookState tracks all 4 states.
    @Test
    fun rapidStartStopStartKeepsSingleActiveHook() {
        val controller = newController(BridgeTreeExtractor())
        val originalCallback = activity.window.callback
        assertEquals(RecordingController.HookState.IDLE, controller.hookState())

        val bg = Executors.newSingleThreadExecutor()
        try {
            // 1. Background start -> INSTALLING until main looper drains -> ACTIVE
            bg.submit { controller.start() }.get()
            assertEquals(RecordingController.HookState.INSTALLING, controller.hookState())
            org.robolectric.shadows.ShadowLooper.idleMainLooper()
            assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())

            // 2. Background stop -> UNINSTALLING, then background start -> INSTALLING (supersedes uninstall)
            bg.submit { controller.stop() }.get()
            assertEquals(RecordingController.HookState.UNINSTALLING, controller.hookState())
            assertEquals(null, controller.stop()) // stop while UNINSTALLING returns null (RECORDING_IDLE)
            bg.submit { controller.start() }.get()
            assertEquals(RecordingController.HookState.INSTALLING, controller.hookState())
            org.robolectric.shadows.ShadowLooper.idleMainLooper()
            assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())

            // 3. Queued bg start + bg stop followed by synchronous main start must ignore stale runnables
            bg.submit { controller.start() }.get() // idempotent while ACTIVE
            bg.submit { controller.stop() }.get()  // queues uninstall gen N
            bg.submit { controller.stop() }.get()  // no-op
        } finally {
            bg.shutdown()
        }

        // Start on main thread BEFORE queued bg uninstall runs!
        controller.start()
        assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())
        assertTrue("controller must remain recording", controller.isRecording())
        assertTrue("window.callback must be wrapped", activity.window.callback !== originalCallback)

        onMain { dispatchTap(testButton, 3_000) }
        assertEquals("single tap must record exactly once (no drop, no double-wrap)", 1, controller.count())

        // Stopping after supersession cleanly unwraps back to the exact original Window.Callback
        assertEquals(1, controller.stop())
        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        assertEquals(RecordingController.HookState.IDLE, controller.hookState())
        assertSame("post-stop window.callback must be unwrapped cleanly", originalCallback, activity.window.callback)
    }

    // BB-B16: if activityProvider returns null during install, HookState rolls back from INSTALLING to IDLE
    // so a subsequent start() recovers instead of remaining permanently wedged in INSTALLING.
    @Test
    fun installFailureRollsBackInstallingToIdleAndRecovers() {
        var currentActivity: Activity? = null
        val controller = RecordingController(
            activityProvider = { currentActivity },
            treeExtractor = BridgeTreeExtractor(),
            timeSource = { fakeTime.get() }
        )

        val bg = Executors.newSingleThreadExecutor()
        try {
            bg.submit { controller.start() }.get()
        } finally {
            bg.shutdown()
        }
        assertEquals(RecordingController.HookState.INSTALLING, controller.hookState())

        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        assertEquals(RecordingController.HookState.IDLE, controller.hookState())
        assertTrue(!controller.isRecording())

        // Subsequent start() once Activity is present succeeds normally
        currentActivity = activity
        controller.start()
        assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())
        onMain { dispatchTap(testButton, 4_000) }
        assertEquals(1, controller.count())
    }

    // BB-B16: uninstall restores the exact Window wrapped on start(), even if activityProvider switched.
    @Test
    fun stopUnwrapsOriginalWindowEvenIfActivityProviderSwitches() {
        val originalCallback = activity.window.callback
        val secondActivity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val secondOriginalCallback = secondActivity.window.callback
        var currentActivity: Activity = activity

        val controller = RecordingController(
            activityProvider = { currentActivity },
            treeExtractor = BridgeTreeExtractor(),
            timeSource = { fakeTime.get() }
        )

        controller.start()
        assertEquals(RecordingController.HookState.ACTIVE, controller.hookState())
        assertTrue(activity.window.callback !== originalCallback)

        // Activity changes before stop()
        currentActivity = secondActivity
        controller.stop()
        org.robolectric.shadows.ShadowLooper.idleMainLooper()

        assertEquals(RecordingController.HookState.IDLE, controller.hookState())
        assertSame("original Activity window callback must be restored", originalCallback, activity.window.callback)
        assertSame("second Activity window callback must stay untouched", secondOriginalCallback, secondActivity.window.callback)
    }

    // BB-B17: agent-injected KeyEvents carrying SYNTHETIC_DEVICE_ID are excluded from capture
    @Test
    fun syntheticKeyEventIsNotRecorded() {
        val controller = newController(BridgeTreeExtractor())
        controller.start()
        onMain {}

        val decor = activity.window.decorView
        val now = fakeTime.get()
        val syntheticBack = KeyEvent(
            now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK, 0,
            0, DefaultInjectionAdapter.SYNTHETIC_DEVICE_ID, 0
        )
        val syntheticEnter = KeyEvent(
            now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER, 0,
            0, DefaultInjectionAdapter.SYNTHETIC_DEVICE_ID, 0
        )
        onMain {
            decor.dispatchKeyEvent(syntheticBack)
            decor.dispatchKeyEvent(syntheticEnter)
        }

        assertEquals("synthetic key events must not be recorded", 0, controller.count())

        // Also verify in-process instrumentation key events during muted agent mutation are excluded
        controller.beginAgentMutation()
        try {
            onMain {
                decor.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
            }
        } finally {
            controller.endAgentMutation()
        }
        assertEquals("muted agent key event must not be recorded", 0, controller.count())
    }

    // BB-B17: overlay noise exclusion must compare event.x/y against getLocationInWindow,
    // not getLocationOnScreen (e.g. when window has a vertical screen offset).
    @Test
    fun overlayHitTestUsesWindowCoordinates() {
        val controller = newController(BridgeTreeExtractor())
        val decor = activity.window.decorView as ViewGroup

        val fakeOverlay = object : View(activity) {
            override fun getLocationInWindow(outLocation: IntArray) {
                outLocation[0] = 0
                outLocation[1] = 100
            }

            override fun getLocationOnScreen(outLocation: IntArray) {
                outLocation[0] = 0
                outLocation[1] = 600 // offset from window coordinates
            }
        }.apply {
            tag = RecordingController.BRIDGE_OVERLAY_TAG
            measure(
                View.MeasureSpec.makeMeasureSpec(300, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(100, View.MeasureSpec.EXACTLY)
            )
            layout(0, 100, 300, 200)
        }
        decor.addView(fakeOverlay)

        controller.start()
        onMain {}

        // Tap inside overlay's window bounds (50, 150) — outside screen bounds (600..700)
        onMain {
            val downTime = fakeTime.get()
            decor.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 50f, 150f, 0)
            )
            fakeTime.set(downTime + 40)
            decor.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, 50f, 150f, 0)
            )
        }

        assertEquals("touch inside overlay window bounds must be excluded", 0, controller.count())
    }
}
