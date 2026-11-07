plugins {
    kotlin("jvm") version "1.9.23"
    `java-gradle-plugin`
    `maven-publish`
}

group = "com.texure.bridge"
version = "0.1.0"

// AGP is provided by the consuming Android build (com.android.* plugin), so compileOnly.
repositories {
    gradlePluginPortal()
    google()
    mavenCentral()
}

dependencies {
    compileOnly("com.android.tools.build:gradle:8.4.0")
    testImplementation("junit:junit:4.13.2")
}

// Plugin ID: com.texure.bridge (DECISIONS.md round 10, docs/lld-plugin.md §5/§6).
gradlePlugin {
    plugins {
        create("texureBridge") {
            id = "com.texure.bridge"
            implementationClass = "com.texure.gradle.TexureBridgePlugin"
        }
    }
}

tasks.test {
    useJUnit()
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    jvmToolchain(17)
}
