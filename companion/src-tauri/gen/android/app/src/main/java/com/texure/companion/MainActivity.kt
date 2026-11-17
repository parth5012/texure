package com.texure.companion

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity

/**
 * Entry point for Texure Companion Android application.
 *
 * Hosts the WebView / Tauri environment and initializes the native [ShizukuPlugin]
 * bridge and lifecycle daemon.
 */
class MainActivity : AppCompatActivity() {

    internal var shizukuPlugin: ShizukuPlugin? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize Shizuku plugin bridge and subscribe to binder state updates
        shizukuPlugin = ShizukuPlugin(this) { event, payload ->
            Log.d(TAG, "Shizuku event: $event, payload: $payload")
        }
    }

    override fun onDestroy() {
        shizukuPlugin?.destroy()
        shizukuPlugin = null
        super.onDestroy()
    }

    companion object {
        private const val TAG = "TexureMainActivity"
    }
}
