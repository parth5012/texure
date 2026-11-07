package com.texure.gradle

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * L1 unit tests for the release-leak detection logic behind [TexureReleaseLeakCheck]
 * (Guardrail G1, docs/lld-plugin.md §4, DECISIONS.md round 10).
 */
class TexureLeakDetectorTest {

    @Test
    fun flagsBridgeGroupDependency() {
        assertTrue(TexureLeakDetector.isBridgeComponent("com.texure", "bridge"))
        assertTrue(TexureLeakDetector.isBridgeComponent("com.texure", "bridge-anything"))
    }

    @Test
    fun flagsBridgeArtifactFromOtherGroup() {
        assertTrue(TexureLeakDetector.isBridgeComponent("org.acme", "bridge"))
    }

    @Test
    fun cleanComponentsPass() {
        assertFalse(TexureLeakDetector.isBridgeComponent("com.texure.sample", "sample"))
        assertFalse(TexureLeakDetector.isBridgeComponent("org.acme", "notable"))
    }

    @Test
    fun manifestScanFindsBridgeTokens() {
        val leaked = """
            <manifest package="com.velavoice.app">
                <provider android:name="com.texure.bridge.init.TexureInitProvider" />
            </manifest>
        """.trimIndent()
        assertTrue(TexureLeakDetector.scanManifest(leaked).isNotEmpty())
        assertTrue(TexureLeakDetector.scanManifest(leaked).any { it.contains("com.texure.bridge") })
    }

    @Test
    fun manifestScanIgnoresHostPackagePrefix() {
        // Host applicationId shares the com.texure.sample prefix; it is NOT a leak.
        val clean = """
            <manifest package="com.texure.sample">
                <activity android:name="com.texure.sample.MainActivity" />
            </manifest>
        """.trimIndent()
        assertEquals(emptyList<String>(), TexureLeakDetector.scanManifest(clean))
    }

    @Test
    fun manifestScanFindsBridgeService() {
        val leaked = """
            <manifest package="com.velavoice.app">
                <service android:name="com.texure.bridge.BridgeService" />
            </manifest>
        """.trimIndent()
        assertTrue(TexureLeakDetector.scanManifest(leaked).isNotEmpty())
    }

    @Test
    fun failureMessageMatchesLldFormat() {
        val message = TexureLeakDetector.failureMessage(
            variant = "release",
            dependencyHits = listOf("com.texure:bridge:0.1.0"),
            manifestHits = listOf("com.texure.bridge"),
        )
        assertTrue(message.contains("Guardrail G1 Violation!"))
        assertTrue(message.contains("Bridge code detected in release variant 'release'!"))
        assertTrue(message.contains("com.texure:bridge"))
        assertTrue(message.contains("debugImplementation"))
    }
}
