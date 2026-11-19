package com.texure.gradle

import org.gradle.api.model.ObjectFactory
import org.gradle.api.provider.Property
import javax.inject.Inject

/**
 * Gradle configuration extension for Texure (`texure { ... }`).
 */
open class TexureExtension @Inject constructor(objects: ObjectFactory) {

    /**
     * When true (default), automatically instruments OkHttpClient in debug builds
     * to attach TexureNetworkInterceptor for dynamic mock rule matching and history.
     * Set to false to disable OkHttp bytecode instrumentation at build time.
     */
    val networkMocking: Property<Boolean> = objects.property(Boolean::class.java).convention(true)

    /**
     * Convenience setter for Groovy / Kotlin DSL property assignment.
     */
    fun setNetworkMocking(enabled: Boolean) {
        networkMocking.set(enabled)
    }

    /**
     * Kotlin DSL function invocation support: `texure { networkMocking(false) }`.
     */
    fun networkMocking(enabled: Boolean) {
        networkMocking.set(enabled)
    }
}
