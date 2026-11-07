package com.texure.gradle

import com.android.build.api.artifact.SingleArtifact
import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import com.android.build.api.variant.ApplicationVariant
import com.android.build.api.variant.LibraryAndroidComponentsExtension
import com.android.build.api.variant.LibraryVariant
import com.android.build.api.variant.Variant
import org.gradle.api.Action
import org.gradle.api.DefaultTask
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.tasks.TaskProvider

/**
 * texure adoption plugin (plugin ID `com.texure.bridge`).
 *
 * Applied to any Android application/library project (Groovy or Kotlin DSL), it:
 * 1. Injects `debugImplementation("com.texure:bridge:<version>")` so target apps
 *    keep exactly one adoption line and zero source changes (Guardrail G2,
 *    docs/lld-plugin.md §1).
 * 2. Registers `checkTexureReleaseLeak<Variant>` verification tasks that fail the
 *    build (never warn) when bridge code resolves for a release variant
 *    (Guardrail G1, docs/lld-plugin.md §4).
 *
 * Nothing else: no bytecode transforms, no source edits, no release wiring.
 */
class TexureBridgePlugin : Plugin<Project> {

    override fun apply(project: Project) {
        val aggregate: TaskProvider<DefaultTask> = project.tasks.register(
            AGGREGATE_TASK_NAME,
            DefaultTask::class.java,
            Action<DefaultTask> { task ->
                task.group = VERIFICATION_GROUP
                task.description =
                    "Runs every checkTexureReleaseLeak<Variant> release-leak verification task."
            },
        )

        listOf(ANDROID_APPLICATION_PLUGIN, ANDROID_LIBRARY_PLUGIN).forEach { pluginId ->
            project.pluginManager.withPlugin(pluginId) { configure(project, aggregate) }
        }
    }

    private fun configure(project: Project, aggregate: TaskProvider<DefaultTask>) {
        val appComponents = project.extensions.findByType(
            ApplicationAndroidComponentsExtension::class.java,
        )
        if (appComponents != null) {
            appComponents.onVariants(
                appComponents.selector().withBuildType(RELEASE_BUILD_TYPE),
                Action<ApplicationVariant> { variant ->
                    wireReleaseVariant(project, aggregate, variant)
                },
            )
        } else {
            val libComponents = project.extensions.findByType(
                LibraryAndroidComponentsExtension::class.java,
            )
                ?: error(
                    "com.texure.bridge must be applied to an Android project "
                        + "(after 'com.android.application' or 'com.android.library')",
                )
            libComponents.onVariants(
                libComponents.selector().withBuildType(RELEASE_BUILD_TYPE),
                Action<LibraryVariant> { variant ->
                    wireReleaseVariant(project, aggregate, variant)
                },
            )
        }

        // Debug-only dependency injection (deferred so the Android configuration exists).
        project.afterEvaluate {
            val configuration = project.configurations.findByName(DEBUG_ONLY_CONFIG)
                ?: project.configurations.maybeCreate(DEBUG_ONLY_CONFIG)
            configuration.dependencies.add(
                project.dependencies.create(
                    mapOf(
                        "group" to TexureLeakDetector.BRIDGE_GROUP,
                        "name" to TexureLeakDetector.BRIDGE_ARTIFACT,
                        "version" to BRIDGE_VERSION,
                    ),
                ),
            )
        }
    }

    private fun wireReleaseVariant(
        project: Project,
        aggregate: TaskProvider<DefaultTask>,
        variant: Variant,
    ) {
        val capitalized = variant.name.replaceFirstChar { it.uppercaseChar() }
        val checkTask = project.tasks.register(
            "${AGGREGATE_TASK_NAME}$capitalized",
            TexureReleaseLeakCheck::class.java,
            Action<TexureReleaseLeakCheck> { task ->
                task.group = VERIFICATION_GROUP
                task.description =
                    "Fails (never warns) if bridge code leaks into release variant '${variant.name}'."
                task.variantName.set(variant.name)
                task.runtimeClasspathConfigName.set("${variant.name}RuntimeClasspath")
                task.compileClasspathConfigName.set("${variant.name}CompileClasspath")
                val manifestLocation = variant.artifacts.get(SingleArtifact.MERGED_MANIFEST)
                task.mergedManifestFile.set(
                    manifestLocation.map { location ->
                        val file = location.asFile
                        file.takeIf { it.isFile } ?: file.resolve("AndroidManifest.xml")
                    },
                )
                // Guardrail tasks are never skipped by up-to-date checking.
                task.outputs.upToDateWhen { false }
            },
        )
        // Wire the merged-manifest producer explicitly (the artifact provider
        // carries the 'process<Variant>MainManifest' producer) so reading the
        // mapped value inside the action is always safe (no premature query).
        checkTask.configure(
            Action { it.dependsOn(variant.artifacts.get(SingleArtifact.MERGED_MANIFEST)) },
        )
        // Runs before packaging/assembly of the release variant (docs/lld-plugin.md §4.1).
        project.tasks.matching { it.name == "assemble$capitalized" }.configureEach {
            it.dependsOn(checkTask)
        }
        aggregate.configure(Action { it.dependsOn(checkTask) })
    }

    private companion object {
        const val RELEASE_BUILD_TYPE = "release"
        const val AGGREGATE_TASK_NAME = "checkTexureReleaseLeak"
        const val VERIFICATION_GROUP = "verification"
        const val DEBUG_ONLY_CONFIG = "debugImplementation"
        const val BRIDGE_VERSION = "0.1.0"
        const val ANDROID_APPLICATION_PLUGIN = "com.android.application"
        const val ANDROID_LIBRARY_PLUGIN = "com.android.library"
    }
}
