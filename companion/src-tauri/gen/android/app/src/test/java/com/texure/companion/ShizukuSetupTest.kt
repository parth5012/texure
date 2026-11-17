package com.texure.companion

import android.content.Context
import android.content.pm.PackageManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import rikka.shizuku.Shizuku

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ShizukuSetupTest {

    @Test
    fun testShizukuDependencyAvailable() {
        // Verify Shizuku class is available on classpath
        val clazz = Shizuku::class.java
        assertNotNull(clazz)
    }

    @Test
    fun testShizukuProviderClassAvailable() {
        val providerClass = Class.forName("rikka.shizuku.ShizukuProvider")
        assertNotNull(providerClass)
    }

    @Test
    fun testManifestDeclaresRequiredForegroundServicePermissions() {
        val context: Context = ApplicationProvider.getApplicationContext()
        val pkgInfo = context.packageManager.getPackageInfo(
            context.packageName,
            PackageManager.GET_PERMISSIONS
        )
        val requestedPermissions = pkgInfo.requestedPermissions?.toSet() ?: emptySet()

        assertTrue(
            "Must declare FOREGROUND_SERVICE",
            requestedPermissions.contains("android.permission.FOREGROUND_SERVICE")
        )
        assertTrue(
            "Must declare FOREGROUND_SERVICE_CONNECTED_DEVICE",
            requestedPermissions.contains("android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE")
        )
        assertTrue(
            "Must declare CHANGE_NETWORK_STATE for Android 14+ connectedDevice precondition",
            requestedPermissions.contains("android.permission.CHANGE_NETWORK_STATE")
        )
        assertTrue(
            "Must declare CHANGE_WIFI_STATE for Android 14+ connectedDevice precondition",
            requestedPermissions.contains("android.permission.CHANGE_WIFI_STATE")
        )
        assertTrue(
            "Must declare POST_NOTIFICATIONS",
            requestedPermissions.contains("android.permission.POST_NOTIFICATIONS")
        )
        assertTrue(
            "Must declare WAKE_LOCK",
            requestedPermissions.contains("android.permission.WAKE_LOCK")
        )
    }
}
