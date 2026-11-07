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

    /**
     * Manifest tokens that only ever come from bridge code, never from the
     * host app. Host packages like `com.texure.sample` intentionally do NOT
     * match.
     */
    private val MANIFEST_TOKENS = listOf(
        "com.texure.bridge",
        "TexureInitProvider",
        "BridgeService",
    )

    /**
     * A resolved component is a leak when its group is `com.texure` OR its
     * artifact name is `bridge` (docs/lld-plugin.md §4.1 inspection logic).
     */
    fun isBridgeComponent(group: String, artifact: String): Boolean =
        group == BRIDGE_GROUP || artifact == BRIDGE_ARTIFACT

    /**
     * Scans merged release manifest content for bridge-only tokens
     * (docs/lld-plugin.md §4.1 inspection logic).
     */
    fun scanManifest(text: String): List<String> =
        MANIFEST_TOKENS.filter { text.contains(it) }

    /**
     * Loud G1 violation message. The dependency line matches the frozen
     * diagnostics block in docs/lld-plugin.md §4.1 exactly; extra scan details
     * follow as indented lines. Unresolved scan inputs (missing configuration,
     * missing manifest) are also failures — a guardrail must never degrade to
     * a silent pass.
     */
    fun failureMessage(
        variant: String,
        dependencyHits: List<String>,
        manifestHits: List<String>,
        missingInputs: List<String> = emptyList(),
    ): String = buildString {
        appendLine("BUILD FAILED: Guardrail G1 Violation!")
        appendLine("Bridge code detected in release variant '$variant'!")
        if (dependencyHits.isNotEmpty()) {
            appendLine("Found dependency: $BRIDGE_GROUP:$BRIDGE_ARTIFACT")
            dependencyHits.forEach { appendLine("    - $it") }
        }
        manifestHits.forEach { appendLine("Found manifest reference: $it") }
        missingInputs.forEach { appendLine("Missing scan input (cannot prove G1 clean): $it") }
        append(
            "Bridge dependencies must only be added to debug configurations ('debugImplementation').",
        )
    }
}
