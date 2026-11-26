package com.texure.bridge.worker

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.widget.EditText
import com.texure.bridge.injection.DefaultInjectionAdapter
import com.texure.bridge.injection.InjectionAdapter
import com.texure.bridge.injection.InjectionResult
import com.texure.bridge.model.BridgeError
import com.texure.bridge.model.Key
import com.texure.bridge.tree.BridgeTreeExtractor
import com.texure.bridge.tree.ExtractedTree
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

sealed class WorkerResult<out T> {
    data class Success<out T>(val value: T) : WorkerResult<T>()
    data class Error(val bridgeError: BridgeError, val detail: String? = null) : WorkerResult<Nothing>()
}

class BridgeWorker(
    private val activityProvider: () -> Activity?,
    private val treeExtractor: BridgeTreeExtractor,
    private val injectionAdapter: InjectionAdapter = DefaultInjectionAdapter(activityProvider)
) {

    private val executor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "texure-bridge-worker")
    }
    private val mainHandler = Handler(Looper.getMainLooper())

    fun executeTap(targetNodeIndex: Int, expectedRev: Long, expectedHash: String? = null): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            // 1. Check rev staleness
            if (expectedRev != treeExtractor.getRev()) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            // 2. Verify-at-execute on main thread (index, or hash fallback)
            val tree = extractTreeOnMain()
            val resolvedIndex = resolveTargetIndex(tree, targetNodeIndex, expectedHash)
                ?: return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)

            val targetView = tree.resolvedViews[resolvedIndex]
            if (!targetView.isClickable) {
                return@submit WorkerResult.Error(BridgeError.NODE_NOT_CLICKABLE)
            }

            val loc = IntArray(2)
            targetView.getLocationInWindow(loc)
            val x = loc[0] + targetView.width / 2f
            val y = loc[1] + targetView.height / 2f

            // 3. Dispatch tap
            when (injectionAdapter.tap(x, y)) {
                is InjectionResult.Success -> {
                    treeExtractor.bumpRev()
                    WorkerResult.Success(Unit)
                }
                is InjectionResult.Failed -> WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
        }

        return try {
            future.get(2000, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    fun executeType(targetNodeIndex: Int, text: String, expectedRev: Long, expectedHash: String? = null): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            if (expectedRev != treeExtractor.getRev()) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val tree = extractTreeOnMain()
            val resolvedIndex = resolveTargetIndex(tree, targetNodeIndex, expectedHash)
                ?: return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)

            val targetView = tree.resolvedViews[resolvedIndex]
            val isEditable = targetView is EditText || (targetView.isFocusable && targetView.isFocusableInTouchMode)
            if (!isEditable) {
                return@submit WorkerResult.Error(BridgeError.NODE_NOT_EDITABLE)
            }

            when (val res = injectionAdapter.text(targetView, text)) {
                is InjectionResult.Success -> {
                    treeExtractor.bumpRev()
                    WorkerResult.Success(Unit)
                }
                is InjectionResult.Failed -> WorkerResult.Error(BridgeError.INJECT_FAILED, res.reason)
            }
        }

        return try {
            future.get(2000, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    fun executeKey(key: Key): WorkerResult<Unit> {
        if (key == Key.BACK) {
            return executeBack()
        }
        val future = executor.submit<WorkerResult<Unit>> {
            when (injectionAdapter.key(key)) {
                is InjectionResult.Success -> {
                    treeExtractor.bumpRev()
                    WorkerResult.Success(Unit)
                }
                is InjectionResult.Failed -> WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
        }

        return try {
            future.get(2500, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            future.cancel(false)
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    /**
     * Dynamic view tagging (map #36, #31). Attaches [tag] to the view at
     * [targetNodeIndex] via `View.setTag`. The rev guard matches /tap|/type;
     * index must resolve against a fresh traversal or refuse NODE_OUTDATED.
     * setTag runs on the main thread; the View object persists across tree
     * refreshes, so the tag survives re-extraction and surfaces as the node
     * `id` via resolveNodeId. That rotates the node's `hash` (id input) —
     * a lawful identity change; rev bumps so agents re-fetch and re-target.
     */
    fun executeTag(targetNodeIndex: Int, tag: String, expectedRev: Long): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            if (expectedRev != treeExtractor.getRev()) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val tree = extractTreeOnMain()
            if (targetNodeIndex !in tree.resolvedViews.indices) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val targetView = tree.resolvedViews[targetNodeIndex]

            if (Looper.myLooper() == Looper.getMainLooper()) {
                targetView.setTag(tag)
                treeExtractor.bumpRev()
                return@submit WorkerResult.Success(Unit)
            }

            val latch = CountDownLatch(1)
            mainHandler.post {
                try {
                    targetView.setTag(tag)
                } finally {
                    latch.countDown()
                }
            }
            if (!latch.await(500, TimeUnit.MILLISECONDS)) {
                return@submit WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
            treeExtractor.bumpRev()
            WorkerResult.Success(Unit)
        }

        return try {
            future.get(2000, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    private fun dispatchBack(activity: Activity): Boolean {
        return try {
            val method = activity.javaClass.methods.firstOrNull { it.name == "getOnBackPressedDispatcher" }
            if (method != null) {
                val dispatcher = method.invoke(activity)
                val backMethod = dispatcher?.javaClass?.getMethod("onBackPressed")
                backMethod?.invoke(dispatcher)
            } else {
                @Suppress("DEPRECATION")
                activity.onBackPressed()
            }
            true
        } catch (_: Exception) {
            try {
                @Suppress("DEPRECATION")
                activity.onBackPressed()
                true
            } catch (_: Exception) {
                false
            }
        }
    }

    fun executeBack(): WorkerResult<Unit> {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            val activity = activityProvider() ?: return WorkerResult.Error(BridgeError.APP_NOT_FOREGROUND)
            return if (dispatchBack(activity)) {
                treeExtractor.bumpRev()
                WorkerResult.Success(Unit)
            } else {
                WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
        }

        val future = executor.submit<WorkerResult<Unit>> {
            val activity = activityProvider() ?: return@submit WorkerResult.Error(BridgeError.APP_NOT_FOREGROUND)
            val latch = CountDownLatch(1)
            val state = AtomicInteger(STATE_PENDING)
            var success = false
            val posted = mainHandler.post {
                if (!state.compareAndSet(STATE_PENDING, STATE_RUNNING)) {
                    latch.countDown()
                    return@post
                }
                try {
                    val currentActivity = activityProvider() ?: activity
                    if (dispatchBack(currentActivity)) {
                        if (state.get() == STATE_RUNNING) {
                            treeExtractor.bumpRev()
                            success = true
                        }
                    }
                } finally {
                    state.set(STATE_COMPLETED)
                    latch.countDown()
                }
            }
            if (!posted) {
                return@submit WorkerResult.Error(BridgeError.INJECT_FAILED)
            }

            val completed = latch.await(1000, TimeUnit.MILLISECONDS)
            if (!completed) {
                if (state.compareAndSet(STATE_PENDING, STATE_CANCELLED)) {
                    return@submit WorkerResult.Error(BridgeError.INJECT_FAILED)
                }
                // Dispatch already started on main thread; await completion
                val completedSecond = latch.await(1000, TimeUnit.MILLISECONDS)
                if (!completedSecond) {
                    state.set(STATE_CANCELLED)
                    return@submit WorkerResult.Error(BridgeError.INJECT_FAILED)
                }
            }

            if (success) {
                WorkerResult.Success(Unit)
            } else {
                WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
        }

        return try {
            future.get(2500, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            future.cancel(false)
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    /**
     * Verify-at-execute target resolution (map #36, #29). Without a hash the
     * requested index is authoritative. With a hash: prefer the requested
     * index when that node still carries the hash, else resolve by hash
     * across the fresh tree; null means refuse with `NODE_OUTDATED`.
     * `resolvedViews` and `nodes` are parallel lists from the same traversal.
     */
    private fun resolveTargetIndex(tree: ExtractedTree, requestedIndex: Int, expectedHash: String?): Int? {
        if (expectedHash == null) {
            return if (requestedIndex in tree.resolvedViews.indices) requestedIndex else null
        }
        if (requestedIndex in tree.resolvedViews.indices && tree.nodes[requestedIndex].hash == expectedHash) {
            return requestedIndex
        }
        val byHash = tree.nodes.indexOfFirst { it.hash == expectedHash }
        return if (byHash >= 0) byHash else null
    }

    fun extractTreeOnMain(subtreeId: String? = null, maxDepth: Int = BridgeTreeExtractor.DEFAULT_MAX_DEPTH): ExtractedTree {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            return treeExtractor.extractTree(activityProvider(), subtreeId, maxDepth)
        }

        val latch = CountDownLatch(1)
        var tree: ExtractedTree? = null

        mainHandler.post {
            try {
                tree = treeExtractor.extractTree(activityProvider(), subtreeId, maxDepth)
            } finally {
                latch.countDown()
            }
        }

        val completed = latch.await(500, TimeUnit.MILLISECONDS)
        if (!completed || tree == null) {
            return treeExtractor.extractTree(null)
        }

        return tree ?: treeExtractor.extractTree(null)
    }

    fun shutdown() {
        executor.shutdownNow()
    }

    companion object {
        private const val STATE_PENDING = 0
        private const val STATE_RUNNING = 1
        private const val STATE_COMPLETED = 2
        private const val STATE_CANCELLED = 3
    }
}
