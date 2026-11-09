package com.texure.bridge.recording

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.text.Editable
import android.text.TextWatcher
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.Window
import android.widget.EditText
import com.texure.bridge.injection.DefaultInjectionAdapter
import com.texure.bridge.model.TreeNode
import com.texure.bridge.tree.BridgeTreeExtractor
import com.texure.bridge.tree.ExtractedTree
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONObject

/**
 * Records human-originated workflow actions on the bridge's own windows and buffers a
 * timestamped trace (behavior-spec 1.7: BB-B12..BB-B15).
 *
 * Capture strategy: while recording, the activity's Window.Callback is wrapped. The
 * wrapper observes [Window.Callback.dispatchTouchEvent] / [KeyEvent.Callback.onKeyDown],
 * records without ever consuming, then delegates to the original callback so the touch
 * stream and back handling are untouched (no gesture hijack, BB-B13; own window only,
 * guardrails G5/G6). Agent-injected mutations are excluded two ways: bridge server
 * mutations raise the mute window around worker execution, and synthetic events carry
 * the marker deviceId (behavior-spec 1.4, human-wins law).
 *
 * Control surface: POST /recording/start|stop, GET /recording/export, DELETE /recording
 * and the debug overlay Record toggle. Guardrail G7: no new MCP tools.
 */
class RecordingController(
    private val activityProvider: () -> Activity?,
    val treeExtractor: BridgeTreeExtractor,
    private val timeSource: () -> Long = { SystemClock.uptimeMillis() }
) {

    private class Session {
        var sessionStart: Long = 0L
        val events = mutableListOf<RecordedAction>()
        var seq: Int = 0
        var lastActionTime: Long? = null
        var watchedEditable: EditText? = null
        var watcherIndex: Int = -1
        var watcherNode: TreeNode? = null
        var textWatcher: TextWatcher? = null
        var baseText: String = ""
        var pendingDelta: String = ""
        var downTime: Long? = null
        var downX: Float = 0f
        var downY: Float = 0f
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private val recording = AtomicBoolean(false)
    private val muteCount = AtomicInteger(0)
    private val sessionRef = AtomicReference<Session?>(null)

    // Installed only while a session is active
    private var originalWindowCallback: Window.Callback? = null
    private var proxyWindowCallback: RecordingWindowCallback? = null

    fun isRecording(): Boolean = recording.get()

    fun count(): Int {
        val session = sessionRef.get() ?: return 0
        return synchronized(session) { session.events.size }
    }

    /** Begins a fresh recording session (fresh trace). Idempotent while active. */
    fun start() {
        if (!recording.compareAndSet(false, true)) {
            return // already recording: contract keeps the buffer untouched
        }
        val session = Session()
        session.sessionStart = now()
        sessionRef.set(session)
        installHooks()
    }

    /**
     * Ends the active session. Returns the recorded action count,
     * or null when idle (maps to `RECORDING_IDLE`).
     */
    fun stop(): Int? {
        if (!isRecording()) return null
        val session = sessionRef.getAndSet(null) ?: return null
        recording.set(false)
        uninstallHooks()
        return synchronized(session) { session.events.size }
    }

    /** Clears the buffered trace (works idle or while recording). */
    fun clear() {
        val session = sessionRef.get() ?: return
        synchronized(session) { session.events.clear() }
    }

    fun export(): JSONObject {
        val session = sessionRef.get()
        return JSONObject().apply {
            put("recording", isRecording())
            if (session == null) {
                put("count", 0)
                put("actions", RecordedAction.actionsJson(emptyList()))
            } else {
                val snapshot = synchronized(session) { session.events.toList() }
                put("count", snapshot.size)
                put("actions", RecordedAction.actionsJson(snapshot))
            }
        }
    }

    /** Excludes agent-injected mutations routed through the bridge server from the trace. */
    fun beginAgentMutation() {
        muteCount.incrementAndGet()
    }

    fun endAgentMutation() {
        muteCount.updateAndGet { current -> maxOf(0, current - 1) }
    }

    private val muted: Boolean get() = muteCount.get() > 0

    private fun now(): Long = timeSource()

    // ---------------------------------------------------------------------------
    // Trace assembly (single seq authority: appendEvent)
    // ---------------------------------------------------------------------------

    private fun appendEvent(
        session: Session,
        t: Long,
        action: String,
        node: RecordedNodeRef? = null,
        value: String? = null,
        key: String? = null,
        ms: Long? = null
    ) {
        synchronized(session) {
            // pause: >= PAUSE_THRESHOLD_MS of inactivity since the previous recorded action
            val last = session.lastActionTime
            if (last != null) {
                val gap = t - last
                if (gap >= PAUSE_THRESHOLD_MS) {
                    session.events.add(RecordedAction(session.seq++, last, "pause", null, null, null, gap))
                }
            }
            session.lastActionTime = t
            session.events.add(RecordedAction(session.seq++, t, action, node, value, key, ms))
        }
    }

    /** Commits pending typed text when the user moves on to another target. */
    private fun commitPendingType(session: Session) {
        val watched = session.watchedEditable ?: return
        val delta = session.pendingDelta
        if (delta.isEmpty()) return
        val tree = extractCurrentTree()
        val ref = correlateNode(tree, watched) ?: return
        val t = (now() - session.sessionStart).coerceAtLeast(0)
        appendEvent(session, t, "type", ref, value = delta)
        session.baseText = watched.text?.toString() ?: ""
        session.pendingDelta = ""
    }

    // ---------------------------------------------------------------------------
    // Hook install / uninstall (main thread only)
    // ---------------------------------------------------------------------------

    private fun installHooks() {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            installHooksOnMain()
            return
        }
        mainHandler.post { installHooksOnMain() }
    }

    private fun installHooksOnMain() {
        if (proxyWindowCallback != null) return
        val activity = activityProvider() ?: return
        val window = activity.window
        val original = window.callback ?: return
        try {
            val proxy = RecordingWindowCallback(this@RecordingController, activity, original)
            window.callback = proxy
            originalWindowCallback = original
            proxyWindowCallback = proxy
        } catch (_: Exception) {
            proxyWindowCallback = null
            originalWindowCallback = null
        }
    }

    private fun uninstallHooks() {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            uninstallHooksOnMain()
            return
        }
        mainHandler.post { uninstallHooksOnMain() }
    }

    private fun uninstallHooksOnMain() {
        val proxy = proxyWindowCallback ?: return
        val activity = activityProvider()
        try {
            if (activity?.window?.callback === proxy) {
                activity.window.callback = originalWindowCallback
            }
        } catch (_: Exception) {}
        proxyWindowCallback = null
        originalWindowCallback = null
        sessionRef.get()?.let { session ->
            unwatch(session)
            session.downTime = null
        }
    }

    private class RecordingWindowCallback(
        private val controller: RecordingController,
        private val activity: Activity,
        private val delegate: Window.Callback
    ) : Window.Callback by delegate, KeyEvent.Callback {

        override fun dispatchKeyEvent(event: KeyEvent?): Boolean {
            event?.let { controller.onKeyObserved(event) }
            return delegate.dispatchKeyEvent(event)
        }

        override fun dispatchTouchEvent(event: MotionEvent?): Boolean {
            event?.let { controller.onTouchObserved(it) }
            return delegate.dispatchTouchEvent(event)
        }

        override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
            return (delegate as? KeyEvent.Callback)?.onKeyDown(keyCode, event) ?: false
        }

        override fun onKeyLongPress(keyCode: Int, event: KeyEvent?): Boolean {
            return (delegate as? KeyEvent.Callback)?.onKeyLongPress(keyCode, event) ?: false
        }

        override fun onKeyUp(keyCode: Int, event: KeyEvent?): Boolean {
            return (delegate as? KeyEvent.Callback)?.onKeyUp(keyCode, event) ?: false
        }

        override fun onKeyMultiple(keyCode: Int, repeatCount: Int, event: KeyEvent?): Boolean {
            return (delegate as? KeyEvent.Callback)?.onKeyMultiple(keyCode, repeatCount, event) ?: false
        }
    }

    // ---------------------------------------------------------------------------
    // Observation (called on the main thread by the callback wrapper)
    // ---------------------------------------------------------------------------

    private fun onTouchObserved(event: MotionEvent) {
        if (event.deviceId == DefaultInjectionAdapter.SYNTHETIC_DEVICE_ID) return
        val session = sessionRef.get() ?: return
        if (!isRecording()) return
        val activity = activityProvider() ?: return

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                session.downTime = event.eventTime
                session.downX = event.x
                session.downY = event.y
                if (session.watchedEditable != null) commitPendingType(session)
            }
            MotionEvent.ACTION_UP -> {
                val down = session.downTime
                val gestureDuration = if (down != null) event.eventTime - down else -1L
                val dx = event.x - session.downX
                val dy = event.y - session.downY
                val travel = dx * dx + dy * dy
                session.downTime = null
                if (down != null && gestureDuration in 0..TAP_THRESHOLD_MS &&
                    travel <= TAP_TRAVEL_SLOP * TAP_TRAVEL_SLOP
                ) {
                    recordTap(session, activity, event)
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                session.downTime = null
            }
        }
    }

    /**
     * BACK / hardware ENTER arrive via the window callback dispatch route
     * (ViewRootImpl -> DecorView -> window callback). The Activity-level fallback
     * dispatches to the Activity object directly and bypasses the proxy — by design:
     * agent /back mutations run muted through the server instead.
     */
    private fun onKeyObserved(event: KeyEvent) {
        val session = sessionRef.get() ?: return
        if (!isRecording()) return
        if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) {
            val keyName = when (event.keyCode) {
                KeyEvent.KEYCODE_BACK -> "BACK"
                KeyEvent.KEYCODE_ENTER -> "ENTER"
                else -> null
            }
            if (keyName != null) {
                val t = (now() - session.sessionStart).coerceAtLeast(0)
                appendEvent(session, t, "keypress", key = keyName)
            }
        }
    }

    // --- tap + node correlation -------------------------------------------------

    private fun recordTap(session: Session, activity: Activity, event: MotionEvent) {
        if (isInsideBridgeOverlay(activity, event)) return
        val tree = extractCurrentTree()
        val match = correlateNodeByPoint(tree, event.x, event.y)
        val t = (now() - session.sessionStart).coerceAtLeast(0)
        val nodeRef = match?.let { (index, node, _) ->
            RecordedNodeRef(index, node.id, node.text, node.cd, node.cls, treeExtractor.getRev())
        }
        appendEvent(session, t, "tap", nodeRef)
        val hitView = match?.third
        if (hitView is EditText) {
            watch(session, hitView, tree)
        }
    }

    private fun isInsideBridgeOverlay(activity: Activity, event: MotionEvent): Boolean {
        // Exclude touches landing on the bridge's own debug overlay controls
        val decor = activity.window.decorView as? android.view.ViewGroup ?: return false
        for (i in 0 until decor.childCount) {
            val child = decor.getChildAt(i)
            if (child.tag == BRIDGE_OVERLAY_TAG) {
                val loc = IntArray(2)
                child.getLocationOnScreen(loc)
                return event.x >= loc[0] && event.x <= loc[0] + child.width &&
                    event.y >= loc[1] && event.y <= loc[1] + child.height
            }
        }
        return false
    }

    private fun extractCurrentTree(): ExtractedTree {
        return treeExtractor.extractTree(activityProvider())
    }

    /** Smallest own-view containing the touched point (behavior-spec 1.7 event mapping). */
    private fun correlateNodeByPoint(tree: ExtractedTree, x: Float, y: Float): Triple<Int, TreeNode, android.view.View>? {
        var bestIndex = -1
        var bestArea = Int.MAX_VALUE
        var bestNode: TreeNode? = null
        var bestView: android.view.View? = null
        for (i in tree.nodes.indices) {
            val view = tree.resolvedViews.getOrNull(i) ?: continue
            val loc = IntArray(2)
            view.getLocationInWindow(loc)
            if (x >= loc[0] && x <= loc[0] + view.width && y >= loc[1] && y <= loc[1] + view.height) {
                val area = view.width * view.height
                if (area < bestArea) {
                    bestArea = area
                    bestIndex = i
                    bestNode = tree.nodes[i]
                    bestView = view
                }
            }
        }
        val node = bestNode ?: return null
        val view = bestView ?: return null
        return Triple(bestIndex, node, view)
    }

    private fun correlateNode(tree: ExtractedTree, view: android.view.View): RecordedNodeRef? {
        val index = tree.resolvedViews.indexOf(view)
        val node = tree.nodes.getOrNull(index) ?: return null
        return RecordedNodeRef(index, node.id, node.text, node.cd, node.cls, treeExtractor.getRev())
    }

    // --- typing capture -------------------------------------------------------

    private fun watch(session: Session, editable: EditText, tree: ExtractedTree) {
        if (session.watchedEditable === editable) return
        unwatch(session)
        val index = tree.resolvedViews.indexOf(editable)
        val node = tree.nodes.getOrNull(index) ?: return
        val watcher = object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) {
                onWatchTextChanged(session, index, node, s?.toString() ?: "")
            }
        }
        editable.addTextChangedListener(watcher)
        session.watchedEditable = editable
        session.textWatcher = watcher
        session.baseText = editable.text?.toString() ?: ""
        session.pendingDelta = ""
    }

    private fun onWatchTextChanged(session: Session, index: Int, node: TreeNode, current: String) {
        if (session.watchedEditable == null) return // watcher already detached
        if (!isRecording() || muted) {
            session.baseText = current
            session.pendingDelta = ""
            return
        }
        session.pendingDelta = if (
            current.length > session.baseText.length &&
            current.startsWith(session.baseText)
        ) {
            current.substring(session.baseText.length)
        } else {
            session.baseText = ""
            current
        }

        if (session.pendingDelta.contains('\n')) {
            val committed = session.pendingDelta.substringBefore('\n')
            session.baseText = current
            session.pendingDelta = ""
            if (committed.isNotEmpty()) {
                val t = (now() - session.sessionStart).coerceAtLeast(0)
                val nodeRef = RecordedNodeRef(index, node.id, node.text, node.cd, node.cls, treeExtractor.getRev())
                appendEvent(session, t, "keypress", key = "ENTER")
                appendEvent(session, t, "type", nodeRef, value = committed)
            }
        }
    }

    private fun unwatch(session: Session) {
        session.watchedEditable?.let { editable ->
            try { editable.removeTextChangedListener(session.textWatcher) } catch (_: Exception) {}
        }
        session.watchedEditable = null
        session.textWatcher = null
        session.watcherIndex = -1
        session.watcherNode = null
        session.pendingDelta = ""
    }

    companion object {
        const val PAUSE_THRESHOLD_MS = 2_000L
        const val TAP_THRESHOLD_MS = 500L
        const val TAP_TRAVEL_SLOP = 40f
        const val BRIDGE_OVERLAY_TAG = "texure-bridge-overlay"
    }
}
