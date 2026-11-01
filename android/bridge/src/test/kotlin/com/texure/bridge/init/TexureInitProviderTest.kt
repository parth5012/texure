package com.texure.bridge.init

import android.app.Activity
import android.app.Application
import android.content.pm.ApplicationInfo
import android.content.pm.ProviderInfo
import com.texure.bridge.service.BridgeService
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TexureInitProviderTest {

    private lateinit var app: Application

    @Before
    fun setUp() {
        app = RuntimeEnvironment.getApplication()
        BridgeService.activeActivity = null
    }

    @After
    fun tearDown() {
        BridgeService.activeActivity = null
    }

    @Test
    fun testProviderInitialization_debuggableApp_tracksActivityLifecycle() {
        // Ensure FLAG_DEBUGGABLE is set
        app.applicationInfo.flags = app.applicationInfo.flags or ApplicationInfo.FLAG_DEBUGGABLE

        val provider = TexureInitProvider()
        val providerInfo = ProviderInfo().apply {
            authority = "${app.packageName}.texure.init-provider"
        }
        provider.attachInfo(app, providerInfo)

        val controller = Robolectric.buildActivity(Activity::class.java).setup()
        val activity = controller.get()

        // Active activity should be set when resumed
        assertEquals(activity, BridgeService.activeActivity)

        // Activity paused/stopped -> activeActivity cleared
        controller.stop()
        assertNull(BridgeService.activeActivity)

        // Activity resumed again -> activeActivity restored
        controller.resume()
        assertEquals(activity, BridgeService.activeActivity)

        // Activity destroyed -> activeActivity cleared and overlay detached
        controller.destroy()
        assertNull(BridgeService.activeActivity)
    }

    @Test
    fun testProviderInitialization_nonDebuggableApp_noOp() {
        // Ensure FLAG_DEBUGGABLE is cleared (release mode)
        app.applicationInfo.flags = app.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE.inv()

        val provider = TexureInitProvider()
        val providerInfo = ProviderInfo().apply {
            authority = "${app.packageName}.texure.init-provider"
        }
        provider.attachInfo(app, providerInfo)

        Robolectric.buildActivity(Activity::class.java).setup()

        // In non-debuggable build, provider does not register callbacks, activeActivity remains null
        assertNull(BridgeService.activeActivity)
    }

    @Test
    fun testProviderEmptyCrudOperations() {
        val provider = TexureInitProvider()
        val uri = android.net.Uri.parse("content://test")
        assertNull(provider.query(uri, null, null, null, null))
        assertNull(provider.getType(uri))
        assertNull(provider.insert(uri, null))
        assertEquals(0, provider.delete(uri, null, null))
        assertEquals(0, provider.update(uri, null, null, null))
    }
}
