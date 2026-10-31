package com.texure.bridge.injection

import android.app.Activity
import android.app.Instrumentation
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.widget.EditText
import com.texure.bridge.model.Key
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

sealed class InjectionResult {
    object Success : InjectionResult()
    data class Failed(val reason: String) : InjectionResult()
}

interface InjectionAdapter {
    fun tap(x: Float, y: Float): InjectionResult
    fun swipe(startX: Float, startY: Float, endX: Float, endY: Float, durationMs: Long): InjectionResult
    fun text(view: View, text: String): InjectionResult
    fun key(key: Key): InjectionResult
}

class DefaultInjectionAdapter(
    private val activityProvider: () -> Activity?
) : InjectionAdapter {

    constructor(activity: Activity) : this({ activity })

    private val mainHandler = Handler(Looper.getMainLooper())
    private val instrumentation: Instrumentation by lazy { Instrumentation() }

    override fun tap(x: Float, y: Float): InjectionResult {
        // If already on main thread, run direct dispatch
        if (Looper.myLooper() == Looper.getMainLooper()) {
            return dispatchTouchEventTap(x, y)
        }

        // Try sendPointerSync primary (must run on background thread)
        val instrumentationSuccess = trySendPointerSyncTap(x, y)
        if (instrumentationSuccess) {
            return InjectionResult.Success
        }

        // Fallback: dispatchTouchEvent replay on decorView
        return dispatchTouchEventTap(x, y)
    }

    override fun swipe(
        startX: Float,
        startY: Float,
        endX: Float,
        endY: Float,
        durationMs: Long
    ): InjectionResult {
        val steps = (durationMs / 16).toInt().coerceAtLeast(5)
        val downTime = SystemClock.uptimeMillis()

        if (Looper.myLooper() != Looper.getMainLooper()) {
            // Primary: sendPointerSync on background thread
            try {
                val down = obtainMotionEvent(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY)
                try {
                    instrumentation.sendPointerSync(down)
                } finally {
                    try { down.recycle() } catch (_: Exception) {}
                }

                for (i in 1..steps) {
                    val t = i.toFloat() / steps
                    val currentX = startX + (endX - startX) * t
                    val currentY = startY + (endY - startY) * t
                    val eventTime = downTime + (durationMs * t).toLong()
                    val move = obtainMotionEvent(downTime, eventTime, MotionEvent.ACTION_MOVE, currentX, currentY)
                    try {
                        instrumentation.sendPointerSync(move)
                    } finally {
                        try { move.recycle() } catch (_: Exception) {}
                    }
                    Thread.sleep(16)
                }

                val upTime = downTime + durationMs
                val up = obtainMotionEvent(downTime, upTime, MotionEvent.ACTION_UP, endX, endY)
                try {
                    instrumentation.sendPointerSync(up)
                } finally {
                    try { up.recycle() } catch (_: Exception) {}
                }
                return InjectionResult.Success
            } catch (_: Exception) {}
        }

        // Fallback: dispatchTouchEvent
        val activity = activityProvider() ?: return InjectionResult.Failed("No active Activity")
        val decor = activity.window.decorView

        val action = {
            val down = obtainMotionEvent(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY)
            try {
                decor.dispatchTouchEvent(down)
            } finally {
                try { down.recycle() } catch (_: Exception) {}
            }

            for (i in 1..steps) {
                val t = i.toFloat() / steps
                val currentX = startX + (endX - startX) * t
                val currentY = startY + (endY - startY) * t
                val move = obtainMotionEvent(downTime, downTime + i * 16, MotionEvent.ACTION_MOVE, currentX, currentY)
                try {
                    decor.dispatchTouchEvent(move)
                } finally {
                    try { move.recycle() } catch (_: Exception) {}
                }
            }

            val up = obtainMotionEvent(downTime, downTime + durationMs, MotionEvent.ACTION_UP, endX, endY)
            try {
                decor.dispatchTouchEvent(up)
            } finally {
                try { up.recycle() } catch (_: Exception) {}
            }
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            return try {
                action()
                InjectionResult.Success
            } catch (e: Exception) {
                InjectionResult.Failed("Swipe failed: ${e.message}")
            }
        }

        val latch = CountDownLatch(1)
        var success = false
        mainHandler.post {
            try {
                action()
                success = true
            } catch (_: Exception) {
                success = false
            } finally {
                latch.countDown()
            }
        }

        val completed = latch.await(1000 + durationMs, TimeUnit.MILLISECONDS)
        if (!completed || !success) {
            return InjectionResult.Failed("Swipe injection timed out or failed on main thread")
        }
        return InjectionResult.Success
    }

    override fun text(view: View, text: String): InjectionResult {
        val action = {
            if (view is EditText) {
                view.requestFocus()
                view.append(text)
            } else {
                view.requestFocus()
                for (char in text) {
                    val keyCode = charToKeyCode(char)
                    if (keyCode != KeyEvent.KEYCODE_UNKNOWN) {
                        view.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, keyCode))
                        view.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, keyCode))
                    }
                }
            }
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            return try {
                action()
                InjectionResult.Success
            } catch (e: Exception) {
                InjectionResult.Failed("Text injection failed: ${e.message}")
            }
        }

        val latch = CountDownLatch(1)
        var success = false
        mainHandler.post {
            try {
                action()
                success = true
            } catch (_: Exception) {
                success = false
            } finally {
                latch.countDown()
            }
        }

        val completed = latch.await(1000, TimeUnit.MILLISECONDS)
        if (!completed || !success) {
            return InjectionResult.Failed("Text injection timed out or failed on main thread")
        }
        return InjectionResult.Success
    }

    override fun key(key: Key): InjectionResult {
        val keyCode = when (key) {
            Key.ENTER -> KeyEvent.KEYCODE_ENTER
            Key.BACK -> KeyEvent.KEYCODE_BACK
            Key.DEL -> KeyEvent.KEYCODE_DEL
            Key.TAB -> KeyEvent.KEYCODE_TAB
        }

        if (Looper.myLooper() != Looper.getMainLooper()) {
            try {
                instrumentation.sendKeyDownUpSync(keyCode)
                return InjectionResult.Success
            } catch (_: Exception) {}
        }

        val activity = activityProvider() ?: return InjectionResult.Failed("No active Activity")
        val action = {
            val target = activity.currentFocus ?: activity.window.decorView
            val down = KeyEvent(KeyEvent.ACTION_DOWN, keyCode)
            val up = KeyEvent(KeyEvent.ACTION_UP, keyCode)
            target.dispatchKeyEvent(down)
            target.dispatchKeyEvent(up)
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            return try {
                action()
                InjectionResult.Success
            } catch (e: Exception) {
                InjectionResult.Failed("Key injection failed: ${e.message}")
            }
        }

        val latch = CountDownLatch(1)
        var success = false
        mainHandler.post {
            try {
                action()
                success = true
            } catch (_: Exception) {
                success = false
            } finally {
                latch.countDown()
            }
        }

        val completed = latch.await(1000, TimeUnit.MILLISECONDS)
        if (!completed || !success) {
            return InjectionResult.Failed("Key injection timed out or failed on main thread")
        }
        return InjectionResult.Success
    }

    private fun trySendPointerSyncTap(x: Float, y: Float): Boolean {
        var down: MotionEvent? = null
        var up: MotionEvent? = null
        return try {
            val downTime = SystemClock.uptimeMillis()
            down = obtainMotionEvent(downTime, downTime, MotionEvent.ACTION_DOWN, x, y)
            up = obtainMotionEvent(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y)

            instrumentation.sendPointerSync(down)
            instrumentation.sendPointerSync(up)
            true
        } catch (_: Exception) {
            false
        } finally {
            try { down?.recycle() } catch (_: Exception) {}
            try { up?.recycle() } catch (_: Exception) {}
        }
    }

    private fun dispatchTouchEventTap(x: Float, y: Float): InjectionResult {
        val activity = activityProvider() ?: return InjectionResult.Failed("No active Activity")
        val decor = activity.window.decorView
        val downTime = SystemClock.uptimeMillis()

        val action = {
            val down = obtainMotionEvent(downTime, downTime, MotionEvent.ACTION_DOWN, x, y)
            val up = obtainMotionEvent(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y)
            try {
                decor.dispatchTouchEvent(down)
                decor.dispatchTouchEvent(up)
            } finally {
                try { down.recycle() } catch (_: Exception) {}
                try { up.recycle() } catch (_: Exception) {}
            }
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            return try {
                action()
                InjectionResult.Success
            } catch (e: Exception) {
                InjectionResult.Failed("dispatchTouchEvent tap failed: ${e.message}")
            }
        }

        val latch = CountDownLatch(1)
        var success = false
        mainHandler.post {
            try {
                action()
                success = true
            } catch (_: Exception) {
                success = false
            } finally {
                latch.countDown()
            }
        }

        val completed = latch.await(1000, TimeUnit.MILLISECONDS)
        if (!completed || !success) {
            return InjectionResult.Failed("dispatchTouchEvent tap timed out or failed on main thread")
        }
        return InjectionResult.Success
    }

    private fun obtainMotionEvent(
        downTime: Long,
        eventTime: Long,
        action: Int,
        x: Float,
        y: Float
    ): MotionEvent {
        return MotionEvent.obtain(
            downTime,
            eventTime,
            action,
            x,
            y,
            0
        ).apply {
            try {
                source = android.view.InputDevice.SOURCE_TOUCHSCREEN
            } catch (_: Exception) {}
        }
    }

    private fun charToKeyCode(char: Char): Int {
        return when (char) {
            in 'a'..'z', in 'A'..'Z' -> KeyEvent.KEYCODE_A + (char.lowercaseChar() - 'a')
            in '0'..'9' -> KeyEvent.KEYCODE_0 + (char - '0')
            ' ' -> KeyEvent.KEYCODE_SPACE
            '\n' -> KeyEvent.KEYCODE_ENTER
            '\t' -> KeyEvent.KEYCODE_TAB
            else -> KeyEvent.KEYCODE_UNKNOWN
        }
    }

    companion object {
        const val SYNTHETIC_DEVICE_ID = 0x7E01
    }
}
