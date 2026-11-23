package com.texure.bridge.config

import android.app.Activity
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.view.View
import androidx.appcompat.app.AppCompatDelegate
import com.texure.bridge.tree.BridgeTreeExtractor
import org.json.JSONObject
import java.util.Locale

sealed class ConfigStressResult {
    data class Success(
        val fontScale: Float?,
        val darkMode: Boolean?,
        val locale: String?,
        val orientation: String?,
        val rev: Long
    ) : ConfigStressResult()

    data class ResetSuccess(
        val rev: Long
    ) : ConfigStressResult()

    data class Error(
        val code: String,
        val message: String,
        val httpStatus: Int = 400
    ) : ConfigStressResult()
}

class BridgeConfigStress(
    private val activityProvider: () -> Activity?,
    private val treeExtractor: BridgeTreeExtractor,
    private val broadcastTreeChanged: ((Long) -> Unit)? = null
) {
    // Baseline configuration snapshot before any stress override
    private var baselineCaptured = false
    private var baselineFontScale: Float = 1.0f
    private var baselineNightMode: Int = AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
    private var baselineUiMode: Int = Configuration.UI_MODE_NIGHT_NO
    private var baselineLocale: Locale = Locale.getDefault()
    private var baselineLayoutDirection: Int = View.LAYOUT_DIRECTION_LTR
    private var baselineRequestedOrientation: Int = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    private var baselineOrientation: Int = Configuration.ORIENTATION_UNDEFINED

    @Synchronized
    fun applyStress(payload: JSONObject): ConfigStressResult {
        val activity = activityProvider()
            ?: return ConfigStressResult.Error(
                code = "APP_NOT_FOREGROUND",
                message = "Cannot apply configuration stress without foreground activity",
                httpStatus = 400
            )

        // Parse & validate font_scale
        val fontScale: Float? = if (payload.has("font_scale") && !payload.isNull("font_scale")) {
            val scale = payload.optDouble("font_scale").toFloat()
            if (scale < 0.5f || scale > 3.0f || scale.isNaN()) {
                return ConfigStressResult.Error(
                    code = "BAD_REQUEST",
                    message = "font_scale must be between 0.5 and 3.0; received $scale",
                    httpStatus = 400
                )
            }
            scale
        } else {
            null
        }

        // Parse & validate dark_mode
        val darkMode: Boolean? = if (payload.has("dark_mode") && !payload.isNull("dark_mode")) {
            payload.optBoolean("dark_mode")
        } else {
            null
        }

        // Parse & validate locale
        val localeTag: String? = if (payload.has("locale") && !payload.isNull("locale")) {
            val tag = payload.optString("locale").trim()
            if (tag.isEmpty()) {
                return ConfigStressResult.Error(
                    code = "BAD_REQUEST",
                    message = "locale must be a non-empty BCP-47 language tag (e.g. ar-AE, de-DE)",
                    httpStatus = 400
                )
            }
            tag
        } else {
            null
        }

        // Parse & validate orientation
        val orientationStr: String? = if (payload.has("orientation") && !payload.isNull("orientation")) {
            val orient = payload.optString("orientation").trim().uppercase(Locale.ROOT)
            if (orient != "PORTRAIT" && orient != "LANDSCAPE") {
                return ConfigStressResult.Error(
                    code = "BAD_REQUEST",
                    message = "orientation must be PORTRAIT or LANDSCAPE; received '$orient'",
                    httpStatus = 400
                )
            }
            orient
        } else {
            null
        }

        if (fontScale == null && darkMode == null && localeTag == null && orientationStr == null) {
            return ConfigStressResult.Error(
                code = "BAD_REQUEST",
                message = "At least one stress parameter must be provided (font_scale, dark_mode, locale, orientation)",
                httpStatus = 400
            )
        }

        try {
            captureBaselineIfNeeded(activity)

            val resources = activity.resources
            val currentConfig = resources.configuration
            val newConfig = Configuration(currentConfig)
            val metrics = resources.displayMetrics

            if (fontScale != null) {
                newConfig.fontScale = fontScale
                @Suppress("DEPRECATION")
                metrics.scaledDensity = metrics.density * fontScale
            }

            if (darkMode != null) {
                val nightMask = Configuration.UI_MODE_NIGHT_MASK
                val nightVal = if (darkMode) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO
                newConfig.uiMode = (newConfig.uiMode and nightMask.inv()) or nightVal

                try {
                    val appCompatMode = if (darkMode) AppCompatDelegate.MODE_NIGHT_YES else AppCompatDelegate.MODE_NIGHT_NO
                    AppCompatDelegate.setDefaultNightMode(appCompatMode)
                } catch (_: Throwable) {
                    // AppCompat optional
                }
            }

            if (localeTag != null) {
                val parsedLocale = Locale.forLanguageTag(localeTag)
                Locale.setDefault(parsedLocale)
                newConfig.setLocale(parsedLocale)
                newConfig.setLayoutDirection(parsedLocale)
            }

            if (orientationStr != null) {
                val requested = if (orientationStr == "PORTRAIT") {
                    ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                } else {
                    ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                }
                activity.requestedOrientation = requested
                newConfig.orientation = if (orientationStr == "PORTRAIT") {
                    Configuration.ORIENTATION_PORTRAIT
                } else {
                    Configuration.ORIENTATION_LANDSCAPE
                }
            }

            @Suppress("DEPRECATION")
            resources.updateConfiguration(newConfig, metrics)
            @Suppress("DEPRECATION")
            activity.applicationContext.resources.updateConfiguration(newConfig, metrics)
        } catch (t: Throwable) {
            return ConfigStressResult.Error(
                code = "INTERNAL_ERROR",
                message = "Failed to apply configuration stress: ${t.message}",
                httpStatus = 500
            )
        }

        treeExtractor.bumpRev()
        val newRev = treeExtractor.getRev()
        broadcastTreeChanged?.invoke(newRev)

        return ConfigStressResult.Success(
            fontScale = fontScale,
            darkMode = darkMode,
            locale = localeTag,
            orientation = orientationStr,
            rev = newRev
        )
    }

    @Synchronized
    fun resetStress(): ConfigStressResult {
        val activity = activityProvider()
            ?: return ConfigStressResult.Error(
                code = "APP_NOT_FOREGROUND",
                message = "Cannot reset configuration stress without foreground activity",
                httpStatus = 400
            )

        if (!baselineCaptured) {
            treeExtractor.bumpRev()
            val newRev = treeExtractor.getRev()
            broadcastTreeChanged?.invoke(newRev)
            return ConfigStressResult.ResetSuccess(newRev)
        }

        try {
            val resources = activity.resources
            val currentConfig = resources.configuration
            val resetConfig = Configuration(currentConfig)
            val metrics = resources.displayMetrics

            resetConfig.fontScale = baselineFontScale
            @Suppress("DEPRECATION")
            metrics.scaledDensity = metrics.density * baselineFontScale

            val nightMask = Configuration.UI_MODE_NIGHT_MASK
            resetConfig.uiMode = (resetConfig.uiMode and nightMask.inv()) or (baselineUiMode and nightMask)

            try {
                AppCompatDelegate.setDefaultNightMode(baselineNightMode)
            } catch (_: Throwable) {}

            Locale.setDefault(baselineLocale)
            resetConfig.setLocale(baselineLocale)
            resetConfig.setLayoutDirection(baselineLocale)

            activity.requestedOrientation = baselineRequestedOrientation
            if (baselineOrientation != Configuration.ORIENTATION_UNDEFINED) {
                resetConfig.orientation = baselineOrientation
            }

            @Suppress("DEPRECATION")
            resources.updateConfiguration(resetConfig, metrics)
            @Suppress("DEPRECATION")
            activity.applicationContext.resources.updateConfiguration(resetConfig, metrics)

            baselineCaptured = false
        } catch (t: Throwable) {
            return ConfigStressResult.Error(
                code = "INTERNAL_ERROR",
                message = "Failed to reset configuration stress: ${t.message}",
                httpStatus = 500
            )
        }

        treeExtractor.bumpRev()
        val newRev = treeExtractor.getRev()
        broadcastTreeChanged?.invoke(newRev)

        return ConfigStressResult.ResetSuccess(newRev)
    }

    private fun captureBaselineIfNeeded(activity: Activity) {
        if (!baselineCaptured) {
            val config = activity.resources.configuration
            baselineFontScale = config.fontScale
            baselineUiMode = config.uiMode
            baselineLocale = config.locales.get(0) ?: Locale.getDefault()
            baselineLayoutDirection = config.layoutDirection
            baselineRequestedOrientation = activity.requestedOrientation
            baselineOrientation = config.orientation
            try {
                baselineNightMode = AppCompatDelegate.getDefaultNightMode()
            } catch (_: Throwable) {
                baselineNightMode = AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
            }
            baselineCaptured = true
        }
    }
}
