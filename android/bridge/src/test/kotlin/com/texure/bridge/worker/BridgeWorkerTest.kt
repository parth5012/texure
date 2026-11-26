package com.texure.bridge.worker

import android.app.Activity
import com.texure.bridge.model.BridgeError
import com.texure.bridge.tree.BridgeTreeExtractor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeWorkerTest {

    open class FaultyActivity : Activity() {
        @Suppress("DEPRECATION")
        @Deprecated("Deprecated in Java")
        override fun onBackPressed() {
            throw RuntimeException("Simulated onBackPressed failure")
        }
        override fun finish() {
            throw RuntimeException("Simulated finish failure")
        }
    }

    open class TrackingActivity : Activity() {
        var backCalled = false
        @Suppress("DEPRECATION")
        @Deprecated("Deprecated in Java")
        override fun onBackPressed() {
            backCalled = true
        }
    }

    @Test
    fun testExecuteBackForegroundSuccess() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val result = worker.executeBack()
        assertTrue("Expected Success, got $result", result is WorkerResult.Success)
        assertEquals("Rev must be bumped on success", initialRev + 1, treeExtractor.getRev())
    }

    @Test
    fun testExecuteBackWhenBackgroundedReturnsAppNotForeground() {
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { null },
            treeExtractor = treeExtractor
        )

        val result = worker.executeBack()
        assertTrue("Expected Error, got $result", result is WorkerResult.Error)
        assertEquals(BridgeError.APP_NOT_FOREGROUND, (result as WorkerResult.Error).bridgeError)
        assertEquals("Rev must not be bumped when backgrounded", initialRev, treeExtractor.getRev())
    }

    @Test
    fun testExecuteBackWhenActionThrowsReturnsInjectFailedWithoutRevBump() {
        val activity = Robolectric.buildActivity(FaultyActivity::class.java).setup().get()
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val result = worker.executeBack()
        assertTrue("Expected Error, got $result", result is WorkerResult.Error)
        assertEquals(BridgeError.INJECT_FAILED, (result as WorkerResult.Error).bridgeError)
        assertEquals("Rev must not be bumped on inject failure", initialRev, treeExtractor.getRev())
    }

    @Test
    fun testExecuteBackSlowMainThreadTimesOutAndCancelsWithoutRevBump() {
        val activity = Robolectric.buildActivity(TrackingActivity::class.java).setup().get()
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val backgroundExecutor = java.util.concurrent.Executors.newSingleThreadExecutor()
        val bgFuture = backgroundExecutor.submit<WorkerResult<Unit>> {
            worker.executeBack()
        }

        // Wait for background execution to complete (it will hit the 1000ms latch timeout and cancel)
        val result = bgFuture.get(3000, java.util.concurrent.TimeUnit.MILLISECONDS)
        assertTrue("Expected Error on timeout, got $result", result is WorkerResult.Error)
        assertEquals(BridgeError.INJECT_FAILED, (result as WorkerResult.Error).bridgeError)
        assertEquals("Rev must not be bumped on timeout", initialRev, treeExtractor.getRev())

        // Now drain main looper tasks to prove the cancelled runnable does not execute action()
        org.robolectric.shadows.ShadowLooper.idleMainLooper()

        org.junit.Assert.assertFalse("Cancelled back press must not run after timeout", activity.backCalled)
        assertEquals("Rev must still be unbumped after looper drain", initialRev, treeExtractor.getRev())

        worker.shutdown()
        backgroundExecutor.shutdown()
    }

    @Test
    fun testExecuteKeyBackRoutesToExecuteBackSuccess() {
        val activity = Robolectric.buildActivity(TrackingActivity::class.java).setup().get()
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val result = worker.executeKey(com.texure.bridge.model.Key.BACK)
        assertTrue("Expected Success, got $result", result is WorkerResult.Success)
        assertEquals("Rev must be bumped on executeKey(BACK) success", initialRev + 1, treeExtractor.getRev())
        assertTrue("Activity back must be called", activity.backCalled)
        worker.shutdown()
    }

    @Test
    fun testExecuteKeyBackWhenSlowTimesOutWithoutRevBump() {
        val activity = Robolectric.buildActivity(TrackingActivity::class.java).setup().get()
        val treeExtractor = BridgeTreeExtractor()
        val initialRev = treeExtractor.getRev()
        val worker = BridgeWorker(
            activityProvider = { activity },
            treeExtractor = treeExtractor
        )

        val backgroundExecutor = java.util.concurrent.Executors.newSingleThreadExecutor()
        val bgFuture = backgroundExecutor.submit<WorkerResult<Unit>> {
            worker.executeKey(com.texure.bridge.model.Key.BACK)
        }

        val result = bgFuture.get(3000, java.util.concurrent.TimeUnit.MILLISECONDS)
        assertTrue("Expected Error on timeout, got $result", result is WorkerResult.Error)
        assertEquals(BridgeError.INJECT_FAILED, (result as WorkerResult.Error).bridgeError)
        assertEquals("Rev must not be bumped on timeout", initialRev, treeExtractor.getRev())

        org.robolectric.shadows.ShadowLooper.idleMainLooper()
        org.junit.Assert.assertFalse("Cancelled back press must not run after timeout", activity.backCalled)

        worker.shutdown()
        backgroundExecutor.shutdown()
    }
}
