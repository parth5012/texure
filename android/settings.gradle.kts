pluginManagement {
    repositories {
        // texure Gradle plugin is resolved from mavenLocal by target apps
        // (docs/lld-plugin.md §6, DECISIONS.md round 10).
        mavenLocal()
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // com.texure:bridge AAR resolves from mavenLocal (bridge:publishToMavenLocal).
        mavenLocal()
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
