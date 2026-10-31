package com.texure.bridge.crash

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.StringWriter
import java.io.PrintWriter
import java.util.concurrent.ConcurrentLinkedQueue

class BridgeCrashHandler(
    private val context: Context,
    private val treeProvider: () -> JSONObject? = { null }
) : Thread.UncaughtExceptionHandler {

    private val previousHandler: Thread.UncaughtExceptionHandler? =
        Thread.getDefaultUncaughtExceptionHandler()

    private val recentActions = ConcurrentLinkedQueue<String>()
    private val maxActions = 20

    fun install() {
        Thread.setDefaultUncaughtExceptionHandler(this)
    }

    fun uninstall() {
        if (Thread.getDefaultUncaughtExceptionHandler() == this) {
            Thread.setDefaultUncaughtExceptionHandler(previousHandler)
        }
    }

    fun recordAction(action: String) {
        recentActions.add(action)
        while (recentActions.size > maxActions) {
            recentActions.poll()
        }
    }

    override fun uncaughtException(thread: Thread, throwable: Throwable) {
        try {
            recordCrash(
                thread = thread,
                throwable = throwable,
                lastTree = treeProvider(),
                recentActions = recentActions.toList()
            )
        } catch (_: Throwable) {
            // Failsafe: never crash inside the crash handler
        } finally {
            previousHandler?.uncaughtException(thread, throwable)
        }
    }

    fun recordCrash(
        thread: Thread,
        throwable: Throwable,
        lastTree: JSONObject?,
        recentActions: List<String>
    ) {
        val sw = StringWriter()
        throwable.printStackTrace(PrintWriter(sw))
        val stackTrace = sw.toString()

        val buildInfo = JSONObject().apply {
            put("application_id", context.packageName)
            try {
                val pInfo = context.packageManager.getPackageInfo(context.packageName, 0)
                put("version_name", pInfo.versionName ?: "unknown")
            } catch (_: Exception) {
                put("version_name", "1.0")
            }
            put("bridge_rev", "0.1.0")
        }

        val actionsArray = JSONArray()
        recentActions.forEach { actionsArray.put(it) }

        val bundle = JSONObject().apply {
            put("timestamp", System.currentTimeMillis())
            put("thread_name", thread.name)
            put("stack_trace", stackTrace)
            put("last_tree", lastTree ?: JSONObject.NULL)
            put("last_actions", actionsArray)
            put("build_info", buildInfo)
        }

        val crashDir = File(context.filesDir, CRASH_DIR_NAME).apply { mkdirs() }
        val crashFile = File(crashDir, CRASH_FILE_NAME)
        crashFile.writeText(bundle.toString(2))
    }

    fun getLastCrash(): JSONObject? {
        val crashFile = File(File(context.filesDir, CRASH_DIR_NAME), CRASH_FILE_NAME)
        if (!crashFile.exists()) return null
        return try {
            JSONObject(crashFile.readText())
        } catch (_: Exception) {
            null
        }
    }

    fun clearCrash() {
        val crashFile = File(File(context.filesDir, CRASH_DIR_NAME), CRASH_FILE_NAME)
        if (crashFile.exists()) {
            crashFile.delete()
        }
    }

    companion object {
        const val CRASH_DIR_NAME = "texure_crash"
        const val CRASH_FILE_NAME = "last_crash.json"
    }
}
