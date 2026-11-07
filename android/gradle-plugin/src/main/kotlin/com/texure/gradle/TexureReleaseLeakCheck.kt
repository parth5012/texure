package com.texure.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.artifacts.component.ModuleComponentIdentifier
import org.gradle.api.artifacts.component.ProjectComponentIdentifier
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
 *    `bridge` (§4.1 inspection logic).
 * 2. The merged release manifest for bridge-only tokens
 *    (`com.texure.bridge`, `TexureInitProvider`, `BridgeService`)
 *    (§4.1 inspection logic).
 *
 * Any hit = hard build failure with the loud G1 diagnostic; warnings are
 * lawless. Unresolvable scan inputs (missing configuration or manifest) also
 * fail — the check must never degrade to a silent pass.
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
     * manifest-producing task is wired via `dependsOn(artifactProvider)` in
     * the plugin, and the check is never up-to-date-skipped.
     */
    @get:Internal
    abstract val mergedManifestFile: Property<File>

    @TaskAction
    fun check() {
        val variant = variantName.get()

        val dependencyHits = mutableListOf<String>()
        val missingInputs = mutableListOf<String>()
        for (configName in listOfNotNull(
            runtimeClasspathConfigName.orNull,
            compileClasspathConfigName.orNull,
        )) {
            val configuration = project.configurations.findByName(configName)
            if (configuration == null) {
                missingInputs += "configuration '$configName' not found for variant '$variant'"
                continue
            }
            for (artifact in configuration.resolvedConfiguration.resolvedArtifacts) {
                val id = artifact.id.componentIdentifier
                // Same rule as module components (docs/lld-plugin.md §4.1):
                // scan types by group/artifact AND the in-repo project module
                // `:bridge` (a project dependency is invisible to manifest
                // scanning of library manifests, so classpaths are the catch).
                when (id) {
                    is ModuleComponentIdentifier -> {
                        if (TexureLeakDetector.isBridgeComponent(id.group, id.module)) {
                            dependencyHits += "${id.group}:${id.module}:${id.version} ($configName)"
                        }
                    }
                    is ProjectComponentIdentifier -> {
                        val projectModule = id.projectName.substringAfterLast(':')
                        if (TexureLeakDetector.isBridgeComponent("", projectModule)) {
                            dependencyHits += "$projectModule (project dependency, $configName)"
                        }
                    }
                    else -> Unit
                }
            }
        }

        val manifestFile = mergedManifestFile.orNull
        if (manifestFile == null || !manifestFile.exists()) {
            missingInputs += "merged release manifest not found" +
                (manifestFile?.absolutePath?.let { " ($it)" } ?: "")
        }
        val manifestHits = manifestFile
            ?.takeIf { it.exists() }
            ?.let { TexureLeakDetector.scanManifest(it.readText()) }
            ?: emptyList()

        if (dependencyHits.isEmpty() && manifestHits.isEmpty() && missingInputs.isEmpty()) {
            logger.info("checkTexureReleaseLeak: no bridge leakage in release variant '{}'", variant)
            return
        }
        throw GradleException(
            TexureLeakDetector.failureMessage(
                variant,
                dependencyHits.distinct(),
                manifestHits.distinct(),
                missingInputs.distinct(),
            ),
        )
    }
}
