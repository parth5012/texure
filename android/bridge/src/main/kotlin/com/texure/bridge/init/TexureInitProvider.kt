package com.texure.bridge.init

import android.app.Activity
import android.app.Application
import android.content.ContentProvider
import android.content.ContentValues
import android.content.pm.ApplicationInfo
import android.database.Cursor
import android.net.Uri
import android.os.Bundle
import com.texure.bridge.crash.BridgeCrashHandler
import com.texure.bridge.overlay.BridgeOverlay
import com.texure.bridge.service.BridgeService
import java.util.WeakHashMap

/**
 * Automatic initialization ContentProvider for the Texure debug bridge.
 *
 * Runs on application startup in debug builds to attach [Application.ActivityLifecycleCallbacks],
 * initialize the debug overlay, and configure the bridge crash handler without requiring
 * any manual initialization code in the host application.
 */
class TexureInitProvider : ContentProvider() {

    private val overlays = WeakHashMap<Activity, BridgeOverlay>()

    /**
     * Initializes the bridge if the host application is debuggable.
     *
     * Returns true immediately in non-debuggable release builds or if context is unavailable.
     */
    override fun onCreate(): Boolean {
        val app = context?.applicationContext as? Application ?: return true
        val isDebuggable = (app.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0
        if (!isDebuggable) return true

        try {
            BridgeCrashHandler(app).install()
        } catch (_: Exception) {}

        app.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}

            override fun onActivityStarted(activity: Activity) {}

            override fun onActivityResumed(activity: Activity) {
                BridgeService.activeActivity = activity
                if (!overlays.containsKey(activity)) {
                    val overlay = BridgeOverlay.install(activity)
                    overlays[activity] = overlay
                }
            }

            override fun onActivityPaused(activity: Activity) {}

            override fun onActivityStopped(activity: Activity) {
                if (BridgeService.activeActivity == activity) {
                    BridgeService.activeActivity = null
                }
            }

            override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}

            override fun onActivityDestroyed(activity: Activity) {
                overlays.remove(activity)?.detach()
                if (BridgeService.activeActivity == activity) {
                    BridgeService.activeActivity = null
                }
            }
        })

        return true
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0
}
