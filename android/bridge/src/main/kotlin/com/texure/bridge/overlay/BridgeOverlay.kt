package com.texure.bridge.overlay

import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.graphics.Color
import android.graphics.Typeface
import android.os.Build
import android.os.IBinder
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import com.texure.bridge.crash.BridgeCrashHandler
import com.texure.bridge.service.BridgeService
import com.texure.bridge.storage.BridgeTokenManager

class BridgeOverlay(
    private val activity: Activity
) {

    private var overlayContainer: LinearLayout? = null
    private var infoText: TextView? = null
    private var crashText: TextView? = null
    private var bridgeService: BridgeService? = null
    private var isBound = false

    private val tokenManager = BridgeTokenManager(activity)
    private val crashHandler = BridgeCrashHandler(activity)

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            val binder = service as? BridgeService.LocalBinder
            bridgeService = binder?.getService()
            isBound = true
            updateDisplay()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            bridgeService = null
            isBound = false
            updateDisplay()
        }
    }

    fun attach() {
        if (overlayContainer != null) return

        BridgeService.activeActivity = activity

        // Attach keep screen on for active debug sessions
        activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        val decorView = activity.window.decorView as? ViewGroup ?: return

        val container = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#E6202028"))
            setPadding(16, 16, 16, 16)
            elevation = 100f
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM
            }
        }

        val title = TextView(activity).apply {
            text = "Texure Bridge (Debug Overlay)"
            setTextColor(Color.parseColor("#00E676"))
            textSize = 14f
            setTypeface(null, Typeface.BOLD)
        }
        container.addView(title)

        infoText = TextView(activity).apply {
            setTextColor(Color.WHITE)
            textSize = 12f
            setPadding(0, 8, 0, 8)
        }
        container.addView(infoText)

        crashText = TextView(activity).apply {
            setTextColor(Color.parseColor("#FF5252"))
            textSize = 11f
            visibility = View.GONE
        }
        container.addView(crashText)

        val buttonBar = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, 8, 0, 0)
        }

        val startStopBtn = Button(activity).apply {
            text = "Start Service"
            textSize = 11f
            setOnClickListener {
                if (bridgeService?.isRunning() == true) {
                    bridgeService?.stopBridge()
                    text = "Start Service"
                } else {
                    startForegroundService()
                    text = "Stop Service"
                }
                updateDisplay()
            }
        }
        buttonBar.addView(startStopBtn)

        val rotateBtn = Button(activity).apply {
            text = "Rotate Token"
            textSize = 11f
            setOnClickListener {
                tokenManager.rotateToken()
                bridgeService?.rotateToken()
                updateDisplay()
            }
        }
        buttonBar.addView(rotateBtn)

        container.addView(buttonBar)
        decorView.addView(container)
        overlayContainer = container

        bindBridgeService()
        updateDisplay()
    }

    private fun startForegroundService() {
        val intent = Intent(activity, BridgeService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            activity.startForegroundService(intent)
        } else {
            activity.startService(intent)
        }
        bindBridgeService()
    }

    private fun bindBridgeService() {
        if (!isBound) {
            val intent = Intent(activity, BridgeService::class.java)
            activity.bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
        }
    }

    fun updateDisplay() {
        val token = tokenManager.getToken()
        val ip = bridgeService?.getLocalIpAddress() ?: "127.0.0.1"
        val state = bridgeService?.getServerState()?.name ?: "IDLE"
        val running = bridgeService?.isRunning() == true
        val portConflict = bridgeService?.isPortConflict() == true

        val statusText = if (portConflict) {
            "Status: PORT_IN_USE (8765 occupied)\nToken: $token"
        } else if (running) {
            "URL: http://$ip:8765\nToken: $token\nState: @$state"
        } else {
            "URL: http://$ip:8765 (Stopped)\nToken: $token\nState: @OFFLINE"
        }
        infoText?.text = statusText

        // Check for latest crash (Ticket #4)
        val lastCrash = crashHandler.getLastCrash()
        if (lastCrash != null) {
            val ts = lastCrash.optLong("timestamp", 0L)
            crashText?.visibility = View.VISIBLE
            crashText?.text = "Last Crash: ${lastCrash.optString("thread_name")} at $ts\nSee GET /last-crash"
        } else {
            crashText?.visibility = View.GONE
        }
    }

    fun detach() {
        if (BridgeService.activeActivity == activity) {
            BridgeService.activeActivity = null
        }
        if (isBound) {
            activity.unbindService(serviceConnection)
            isBound = false
        }
        overlayContainer?.let {
            val parent = it.parent as? ViewGroup
            parent?.removeView(it)
        }
        overlayContainer = null
        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    companion object {
        fun install(activity: Activity): BridgeOverlay {
            val overlay = BridgeOverlay(activity)
            overlay.attach()
            return overlay
        }
    }
}
