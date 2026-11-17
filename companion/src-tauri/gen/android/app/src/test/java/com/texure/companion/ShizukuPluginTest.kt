package com.texure.companion

import android.content.Context
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import androidx.test.core.app.ApplicationProvider
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
        // Shizuku binder is not bound in Robolectric test environment
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

        // Simulating granted callback
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

        // Simulating denied callback
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
        // Calling unregister again should also be safe and idempotent
        plugin.unregisterListeners()
    }
}
