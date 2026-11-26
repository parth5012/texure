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
        class FaultyActivity : Activity() {
            @Suppress("DEPRECATION")
            @Deprecated("Deprecated in Java")
            override fun onBackPressed() {
                throw RuntimeException("Simulated onBackPressed failure")
            }
            override fun finish() {
                throw RuntimeException("Simulated finish failure")
            }
        }

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
}
