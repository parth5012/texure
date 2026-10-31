package com.texure.sample

import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {

    private var clickCount = 0
    private var debugOverlay: Any? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val btnTest = findViewById<Button>(R.id.btn_test)
        val tvStatus = findViewById<TextView>(R.id.tv_status)

        btnTest.setOnClickListener {
            clickCount++
            tvStatus.text = "Clicks: $clickCount"
        }

        // Attach bridge overlay in debug builds (Guardrail G1 compliant)
        if (BuildConfig.DEBUG) {
            try {
                val overlayClass = Class.forName("com.texure.bridge.overlay.BridgeOverlay")
                val installMethod = overlayClass.getMethod("install", android.app.Activity::class.java)
                debugOverlay = installMethod.invoke(null, this)
            } catch (e: Exception) {
                android.util.Log.w("TexureSample", "Bridge overlay install skipped or failed", e)
            }
        }
    }

    override fun onDestroy() {
        debugOverlay?.let { overlay ->
            try {
                val detachMethod = overlay.javaClass.getMethod("detach")
                detachMethod.invoke(overlay)
            } catch (e: Exception) {
                android.util.Log.w("TexureSample", "Bridge overlay detach failed", e)
            }
            debugOverlay = null
        }
        super.onDestroy()
    }
}
