pluginManagement {
    repositories {
        // texure Gradle plugin is resolved from mavenLocal by target apps
        // (docs/lld-plugin.md §6, DECISIONS.md round 10). exclusiveContent
        // pins ONLY the texure plugin artifacts to mavenLocal so every other
        // plugin resolves from its intended repository.
        exclusiveContent {
            forRepository { mavenLocal() }
            filter {
                includeModule("com.texure.bridge", "com.texure.bridge.gradle.plugin")
                includeModule("com.texure.bridge", "gradle-plugin")
            }
        }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // com.texure:bridge AAR resolves from mavenLocal (bridge:publishToMavenLocal).
        // Restricted to that module: mavenLocal cannot shadow unrelated deps.
        exclusiveContent {
            forRepository { mavenLocal() }
            filter {
                includeModule("com.texure", "bridge")
            }
        }
        google()
        mavenCentral()
    }
}

// Standalone build for the Gradle plugin; tasks are addressable as
// :gradle-plugin:publishToMavenLocal (docs/lld-ci.md §3).
includeBuild("gradle-plugin")

rootProject.name = "texure-android"
include(":bridge")
include(":sample")
