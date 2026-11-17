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

    // --- Elevated Fallback Command & Sanitization Tests (Ticket #56) ---

    @Test
    fun testBuildGrantPermissionCommandValid() {
        val cmd = ShizukuPlugin.buildGrantPermissionCommand(
            "com.texure.sample",
            "android.permission.POST_NOTIFICATIONS"
        )
        assertArrayEquals(
            arrayOf("pm", "grant", "com.texure.sample", "android.permission.POST_NOTIFICATIONS"),
            cmd
        )

        // Auto-prefixes short permission name
        val shortCmd = ShizukuPlugin.buildGrantPermissionCommand(
            "com.texure.sample",
            "RECORD_AUDIO"
        )
        assertArrayEquals(
            arrayOf("pm", "grant", "com.texure.sample", "android.permission.RECORD_AUDIO"),
            shortCmd
        )
    }

    @Test(expected = IllegalArgumentException::class)
    fun testGrantPermissionRejectsPackageShellInjection() {
        ShizukuPlugin.buildGrantPermissionCommand(
            "com.texure.sample; rm -rf /",
            "android.permission.POST_NOTIFICATIONS"
        )
    }

    @Test(expected = IllegalArgumentException::class)
    fun testGrantPermissionRejectsPermissionShellInjection() {
        ShizukuPlugin.buildGrantPermissionCommand(
            "com.texure.sample",
            "POST_NOTIFICATIONS && echo pwned"
        )
    }

    @Test
    fun testBuildSystemTapCommandValid() {
        val cmd = ShizukuPlugin.buildSystemTapCommand(540, 960)
        assertArrayEquals(arrayOf("input", "tap", "540", "960"), cmd)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testSystemTapRejectsNegativeX() {
        ShizukuPlugin.buildSystemTapCommand(-1, 960)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testSystemTapRejectsNegativeY() {
        ShizukuPlugin.buildSystemTapCommand(540, -100)
    }

    @Test
    fun testBuildPressKeyCommandValid() {
        val intCmd = ShizukuPlugin.buildPressKeyCommand(4) // KEYCODE_BACK
        assertArrayEquals(arrayOf("input", "keyevent", "4"), intCmd)

        val strCmd = ShizukuPlugin.buildPressKeyCommand("KEYCODE_BACK")
        assertArrayEquals(arrayOf("input", "keyevent", "KEYCODE_BACK"), strCmd)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testPressKeyRejectsShellInjection() {
        ShizukuPlugin.buildPressKeyCommand("KEYCODE_BACK; reboot")
    }

    @Test
    fun testElevatedCommandsExecuteViaRunner() {
        var lastCmd: Array<String>? = null

        val runner = { cmd: Array<String>, _: Long ->
            lastCmd = cmd
            ShellResult(ok = true, exitCode = 0, stdout = "Success", stderr = "")
        }

        val plugin = ShizukuPlugin(context).apply {
            processRunner = runner
        }

        // Grant permission
        val grantRes = plugin.shizuku_grant_permission("com.texure.sample", "POST_NOTIFICATIONS")
        assertTrue(grantRes.ok)
        assertArrayEquals(
            arrayOf("pm", "grant", "com.texure.sample", "android.permission.POST_NOTIFICATIONS"),
            lastCmd
        )

        // System tap
        val tapRes = plugin.shizuku_system_tap(100, 200)
        assertTrue(tapRes.ok)
        assertArrayEquals(arrayOf("input", "tap", "100", "200"), lastCmd)

        // Press key int
        val keyIntRes = plugin.shizuku_press_key(66)
        assertTrue(keyIntRes.ok)
        assertArrayEquals(arrayOf("input", "keyevent", "66"), lastCmd)

        // Press key string
        val keyStrRes = plugin.shizuku_press_key("KEYCODE_ENTER")
        assertTrue(keyStrRes.ok)
        assertArrayEquals(arrayOf("input", "keyevent", "KEYCODE_ENTER"), lastCmd)
    }

    @Test
    fun testElevatedCommandsCleanErrorWhenNotBound() {
        val plugin = ShizukuPlugin(context)

        val grantRes = plugin.shizuku_grant_permission("com.texure.sample", "POST_NOTIFICATIONS")
        assertFalse(grantRes.ok)
        assertEquals("PERMISSION_DENIED", grantRes.error)
        assertEquals("request Shizuku permission on the device", grantRes.errorHint)

        val tapRes = plugin.shizuku_system_tap(100, 200)
        assertFalse(tapRes.ok)
        assertEquals("PERMISSION_DENIED", tapRes.error)

        val keyRes = plugin.shizuku_press_key(4)
        assertFalse(keyRes.ok)
        assertEquals("PERMISSION_DENIED", keyRes.error)
    }

    @Test
    fun testOnBinderReceivedEmitsConnectedState() {
        val plugin = ShizukuPlugin(context) { event, payload ->
            emittedEvents.add(event to payload)
        }

        plugin.onBinderReceived()

        assertEquals(1, emittedEvents.size)
        val (event, payload) = emittedEvents[0]
        assertEquals(ShizukuPlugin.EVENT_BINDER_STATE, event)
        assertEquals(true, payload["connected"])
    }

    @Test
    fun testOnRequestPermissionResultIgnoresWrongRequestCode() {
        val plugin = ShizukuPlugin(context) { event, payload ->
            emittedEvents.add(event to payload)
        }

        plugin.onRequestPermissionResult(9999, PackageManager.PERMISSION_GRANTED)
        assertEquals(0, emittedEvents.size)
    }

    @Test
    fun testBuildStartAppCommandValidVariants() {
        // Simple name
        val cmd1 = ShizukuPlugin.buildStartAppCommand("com.texure.sample", "MainActivity")
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/.MainActivity"), cmd1)

        // Leading dot
        val cmd2 = ShizukuPlugin.buildStartAppCommand("com.texure.sample", ".ui.DetailActivity")
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/.ui.DetailActivity"), cmd2)

        // Full component with matching package
        val cmd3 = ShizukuPlugin.buildStartAppCommand("com.texure.sample", "com.texure.sample/com.texure.sample.MainActivity")
        assertArrayEquals(arrayOf("am", "start", "-n", "com.texure.sample/com.texure.sample.MainActivity"), cmd3)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testStartAppRejectsActivityWithFlags() {
        ShizukuPlugin.buildStartAppCommand("com.texure.sample", "MainActivity -W --user 0")
    }

    @Test(expected = IllegalArgumentException::class)
    fun testStartAppRejectsActivityWithMetacharacters() {
        ShizukuPlugin.buildStartAppCommand("com.texure.sample", "MainActivity; reboot")
    }

    @Test(expected = IllegalArgumentException::class)
    fun testStartAppRejectsActivityWithMismatchedPackage() {
        ShizukuPlugin.buildStartAppCommand("com.texure.sample", "com.other.pkg/com.other.pkg.MainActivity")
    }

    @Test
    fun testNonZeroExitPopulatesError() {
        val runner = { _: Array<String>, _: Long ->
            ShellResult(ok = false, exitCode = 1, stdout = "", stderr = "some error")
        }
        val plugin = ShizukuPlugin(context).apply {
            processRunner = runner
        }
        val res = plugin.shizuku_stop_app("com.texure.sample")
        assertFalse(res.ok)
        assertEquals(1, res.exitCode)
    }

    @Test
    fun testPluginDestroyCleansListeners() {
        val plugin = ShizukuPlugin(context)
        plugin.destroy()
        plugin.destroy() // idempotent
    }
}
