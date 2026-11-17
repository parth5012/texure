package com.texure.companion

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
}
