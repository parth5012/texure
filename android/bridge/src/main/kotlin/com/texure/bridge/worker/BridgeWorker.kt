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

    fun executeTap(targetNodeIndex: Int, expectedRev: Long): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            // 1. Check rev staleness
            if (expectedRev != treeExtractor.getRev()) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            // 2. Verify-at-execute on main thread
            val tree = extractTreeOnMain()
            if (targetNodeIndex < 0 || targetNodeIndex >= tree.resolvedViews.size) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val targetView = tree.resolvedViews[targetNodeIndex]
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

    fun executeType(targetNodeIndex: Int, text: String, expectedRev: Long): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            if (expectedRev != treeExtractor.getRev()) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val tree = extractTreeOnMain()
            if (targetNodeIndex < 0 || targetNodeIndex >= tree.resolvedViews.size) {
                return@submit WorkerResult.Error(BridgeError.NODE_OUTDATED)
            }

            val targetView = tree.resolvedViews[targetNodeIndex]
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
            future.get(2000, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
    }

    fun executeBack(): WorkerResult<Unit> {
        val future = executor.submit<WorkerResult<Unit>> {
            val activity = activityProvider() ?: return@submit WorkerResult.Error(BridgeError.APP_NOT_FOREGROUND)

            val action = {
                val method = activity.javaClass.methods.firstOrNull { it.name == "getOnBackPressedDispatcher" }
                if (method != null) {
                    val dispatcher = method.invoke(activity)
                    val backMethod = dispatcher?.javaClass?.getMethod("onBackPressed")
                    backMethod?.invoke(dispatcher)
                } else {
                    @Suppress("DEPRECATION")
                    activity.onBackPressed()
                }
            }

            if (Looper.myLooper() == Looper.getMainLooper()) {
                return@submit try {
                    action()
                    treeExtractor.bumpRev()
                    WorkerResult.Success(Unit)
                } catch (_: Exception) {
                    WorkerResult.Error(BridgeError.INJECT_FAILED)
                }
            }

            val latch = CountDownLatch(1)
            var success = false
            mainHandler.post {
                try {
                    action()
                    success = true
                } catch (_: Exception) {
                    try {
                        @Suppress("DEPRECATION")
                        activity.onBackPressed()
                        success = true
                    } catch (_: Exception) {
                        try {
                            activity.finish()
                            success = true
                        } catch (_: Exception) {}
                    }
                } finally {
                    latch.countDown()
                }
            }

            val completed = latch.await(1000, TimeUnit.MILLISECONDS)
            if (!completed || !success) {
                // Direct fallback
                try {
                    action()
                    success = true
                } catch (_: Exception) {}
            }

            if (success) {
                treeExtractor.bumpRev()
                WorkerResult.Success(Unit)
            } else {
                WorkerResult.Error(BridgeError.INJECT_FAILED)
            }
        }

        return try {
            future.get(2000, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            WorkerResult.Error(BridgeError.INJECT_FAILED)
        }
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
}
