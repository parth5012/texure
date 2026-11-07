package com.texure.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.artifacts.component.ModuleComponentIdentifier
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.TaskAction
import java.io.File

/**
 * Release-leak verification task registered per release variant by
 * [TexureBridgePlugin] (`checkTexureReleaseLeak<Variant>`, Guardrail G1,
 * docs/lld-plugin.md §4).
 *
 * Scans:
 * 1. The variant's resolved `RuntimeClasspath` and `CompileClasspath`
 *    configurations for components with group `com.texure` or artifact
 *    `bridge` (§4.2).
 * 2. The merged release manifest for bridge-only tokens
 *    (`com.texure.bridge`, `TexureInitProvider`, `BridgeService`) (§4.3).
 *
 * Any hit = hard build failure with the loud G1 diagnostic; warnings are lawless.
 */
abstract class TexureReleaseLeakCheck : DefaultTask() {

    @get:Input
    abstract val variantName: Property<String>

    @get:Input
    abstract val runtimeClasspathConfigName: Property<String>

    @get:Input
    abstract val compileClasspathConfigName: Property<String>

    /**
     * Merged manifest file for the release variant (AGP Variant API). The
     * manifest-producing task is wired explicitly via `dependsOn
     * (artifact.buildDependencies)` in the plugin, and the check is never
     * up-to-date-skipped.
     */
    @get:Internal
    abstract val mergedManifestFile: Property<File>

    @TaskAction
    fun check() {
        val variant = variantName.get()

        val dependencyHits = mutableListOf<String>()
        for (configName in listOf(runtimeClasspathConfigName.orNull, compileClasspathConfigName.orNull).filterNotNull()) {
            val configuration = project.configurations.findByName(configName) ?: continue
            for (artifact in configuration.resolvedConfiguration.resolvedArtifacts) {
                val id = artifact.id.componentIdentifier
                if (id is ModuleComponentIdentifier && TexureLeakDetector.isBridgeComponent(id.group, id.module)) {
                    dependencyHits += "${id.group}:${id.module}:${id.version} ($configName)"
                }
            }
        }

        val manifestHits = mergedManifestFile.orNull
            ?.takeIf { it.exists() }
            ?.let { TexureLeakDetector.scanManifest(it.readText()) }
            ?: emptyList()

        if (dependencyHits.isEmpty() && manifestHits.isEmpty()) {
            logger.info("checkTexureReleaseLeak: no bridge leakage in release variant '{}'", variant)
            return
        }
        throw GradleException(
            TexureLeakDetector.failureMessage(variant, dependencyHits.distinct(), manifestHits.distinct()),
        )
    }
}
