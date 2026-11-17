package com.texure.companion

import android.content.Context
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ShizukuPluginTest {

    private lateinit var context: Context
    private val emittedEvents = mutableListOf<Pair<String, Map<String, Any?>>>()

    @Before
    fun setup() {
        context = ApplicationProvider.getApplicationContext()
        emittedEvents.clear()
    }

    @Test
    fun testIsInstalledFalseByDefault() {
        val plugin = ShizukuPlugin(context)
        assertFalse(plugin.shizuku_is_installed())
    }

    @Test
    fun testIsInstalledTrueWhenPackagePresent() {
        val pm = context.packageManager
        val shadowPm = Shadows.shadowOf(pm)

        val pkgInfo = PackageInfo().apply {
            packageName = "moe.shizuku.privileged.api"
        }
        shadowPm.installPackage(pkgInfo)

        val plugin = ShizukuPlugin(context)
        assertTrue(plugin.shizuku_is_installed())
    }

    @Test
    fun testIsRunningFalseWhenNotBound() {
        val plugin = ShizukuPlugin(context)
        assertFalse(plugin.shizuku_is_running())
    }

    @Test
    fun testHasPermissionFalseWhenNotRunning() {
        val plugin = ShizukuPlugin(context)
        assertFalse(plugin.shizuku_has_permission())
    }

    @Test
    fun testRequestPermissionReturnsFalseWhenNotRunning() {
        val plugin = ShizukuPlugin(context)
        assertFalse(plugin.shizuku_request_permission())
    }

    @Test
    fun testOnBinderDeadEmitsEvent() {
        val plugin = ShizukuPlugin(context) { event, payload ->
            emittedEvents.add(event to payload)
        }

        plugin.onBinderDead()

        assertEquals(1, emittedEvents.size)
        val (event, payload) = emittedEvents[0]
        assertEquals(ShizukuPlugin.EVENT_BINDER_STATE, event)
        assertEquals(false, payload["connected"])
        assertEquals(false, payload["permission"])
    }

    @Test
    fun testOnRequestPermissionResultEmitsEvent() {
        val plugin = ShizukuPlugin(context) { event, payload ->
            emittedEvents.add(event to payload)
        }

        plugin.onRequestPermissionResult(
            ShizukuPlugin.REQUEST_CODE_PERMISSION,
            PackageManager.PERMISSION_GRANTED
        )

        assertEquals(1, emittedEvents.size)
        val (event, payload) = emittedEvents[0]
        assertEquals(ShizukuPlugin.EVENT_BINDER_STATE, event)
        assertEquals(true, payload["permission"])
    }

    @Test
    fun testOnRequestPermissionResultDeniedEmitsEvent() {
        val plugin = ShizukuPlugin(context) { event, payload ->
            emittedEvents.add(event to payload)
        }

        plugin.onRequestPermissionResult(
            ShizukuPlugin.REQUEST_CODE_PERMISSION,
            PackageManager.PERMISSION_DENIED
        )

        assertEquals(1, emittedEvents.size)
        val (event, payload) = emittedEvents[0]
        assertEquals(ShizukuPlugin.EVENT_BINDER_STATE, event)
        assertEquals(false, payload["permission"])
    }

    @Test
    fun testUnregisterListenersSucceedsWithoutException() {
        val plugin = ShizukuPlugin(context)
        plugin.unregisterListeners()
        plugin.unregisterListeners()
    }

    // --- Supervisor Command Synthesis Tests (Ticket #55) ---

    @Test
    fun testBuildWakeCommand() {
        val cmd = ShizukuPlugin.buildWakeCommand()
        assertArrayEquals(arrayOf("input", "keyevent", "KEYCODE_WAKEUP"), cmd)
    }

    @Test
    fun testBuildDismissKeyguardCommand() {
        val cmd = ShizukuPlugin.buildDismissKeyguardCommand()
        assertArrayEquals(arrayOf("wm", "dismiss-keyguard"), cmd)
    }

    @Test
    fun testBuildStartAppCommandWithActivity() {
        val cmd = ShizukuPlugin.buildStartAppCommand("com.texure.sample", ".MainActivity")
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/.MainActivity"), cmd)

        val fullCmd = ShizukuPlugin.buildStartAppCommand("com.texure.sample", "com.texure.sample/com.texure.sample.MainActivity")
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/com.texure.sample.MainActivity"), fullCmd)
    }

    @Test
    fun testBuildStartAppCommandLauncherDefault() {
        val cmd = ShizukuPlugin.buildStartAppCommand("com.texure.sample")
        assertArrayEquals(
            arrayOf(
                "am", "start",
                "-a", "android.intent.action.MAIN",
                "-c", "android.intent.category.LAUNCHER",
                "-p", "com.texure.sample"
            ),
            cmd
        )
    }

    @Test
    fun testBuildStopAppCommand() {
        val cmd = ShizukuPlugin.buildStopAppCommand("com.texure.sample")
        assertArrayEquals(arrayOf("am", "force-stop", "com.texure.sample"), cmd)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testBlankPackageNameThrows() {
        ShizukuPlugin.buildStopAppCommand("   ")
    }

    @Test
    fun testSupervisorExecutionWithProcessRunner() {
        var lastCmd: Array<String>? = null
        var lastTimeout: Long = 0L

        val runner = { cmd: Array<String>, timeout: Long ->
            lastCmd = cmd
            lastTimeout = timeout
            ShellResult(ok = true, exitCode = 0, stdout = "Success", stderr = "")
        }

        val plugin = ShizukuPlugin(context).apply {
            processRunner = runner
        }

        // Wake screen
        val wakeRes = plugin.shizuku_wake_screen(3000L)
        assertTrue(wakeRes.ok)
        assertArrayEquals(arrayOf("input", "keyevent", "KEYCODE_WAKEUP"), lastCmd)
        assertEquals(3000L, lastTimeout)

        // Dismiss keyguard
        val dismissRes = plugin.shizuku_dismiss_keyguard()
        assertTrue(dismissRes.ok)
        assertArrayEquals(arrayOf("wm", "dismiss-keyguard"), lastCmd)

        // Start app
        val startRes = plugin.shizuku_start_app("com.texure.sample", ".MainActivity")
        assertTrue(startRes.ok)
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/.MainActivity"), lastCmd)

        // Stop app
        val stopRes = plugin.shizuku_stop_app("com.texure.sample")
        assertTrue(stopRes.ok)
        assertArrayEquals(arrayOf("am", "force-stop", "com.texure.sample"), lastCmd)
    }

    @Test
    fun testTimeoutHandlingReportsTimedOut() {
        val runner = { _: Array<String>, _: Long ->
            ShellResult(
                ok = false,
                exitCode = -1,
                stdout = "",
                stderr = "Command timed out after 1000ms",
                timedOut = true,
                error = "TIMEOUT"
            )
        }

        val plugin = ShizukuPlugin(context).apply {
            processRunner = runner
        }
        val res = plugin.shizuku_wake_screen(1000L)

        assertFalse(res.ok)
        assertTrue(res.timedOut)
        assertEquals("TIMEOUT", res.error)
    }
}
