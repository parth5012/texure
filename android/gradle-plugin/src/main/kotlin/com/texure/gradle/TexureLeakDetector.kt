package com.texure.gradle

/**
 * Pure leak-scan logic behind `checkTexureReleaseLeak<Variant>` (Guardrail G1,
 * docs/lld-plugin.md §4, DECISIONS.md round 10).
 *
 * Kept free of Gradle types so it is unit-testable; the task wires resolved
 * dependency identifiers and merged-manifest text into these functions.
 */
object TexureLeakDetector {
    const val BRIDGE_GROUP = "com.texure"
    const val BRIDGE_ARTIFACT = "bridge"

    /** Manifest tokens that only ever come from bridge code, never from the host app. */
    private val MANIFEST_TOKENS = listOf(
        "com.texure.bridge",
        "TexureInitProvider",
        "BridgeService",
    )

    /**
     * A resolved component is a leak when its group is `com.texure` OR its
     * artifact name is `bridge` (docs/lld-plugin.md §4.2).
     */
    fun isBridgeComponent(group: String, artifact: String): Boolean =
        group == BRIDGE_GROUP || artifact == BRIDGE_ARTIFACT

    /**
     * Scans merged release manifest content for bridge-only tokens.
     * Host packages like `com.texure.sample` intentionally do NOT match.
     */
    fun scanManifest(text: String): List<String> =
        MANIFEST_TOKENS.filter { text.contains(it) }

    /** Loud G1 violation message, exact format from docs/lld-plugin.md §4.1. */
    fun failureMessage(variant: String, dependencyHits: List<String>, manifestHits: List<String>): String {
        val details = buildString {
            dependencyHits.forEach { appendLine("Found dependency: $it") }
            manifestHits.forEach { appendLine("Found manifest reference: $it") }
        }
        return buildString {
            appendLine("BUILD FAILED: Guardrail G1 Violation!")
            appendLine("Bridge code detected in release variant '$variant'!")
            append(details)
            append("Bridge dependencies must only be added to debug configurations ('debugImplementation').")
        }
    }
}
