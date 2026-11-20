package com.texure.bridge.logcat

import android.os.Process
import android.util.Log
import java.io.BufferedReader
import java.io.InputStreamReader
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone
import java.util.concurrent.atomic.AtomicBoolean
import java.util.regex.Pattern

class BridgeLogcatReader(
    val ringBuffer: LogRingBuffer = LogRingBuffer(),
    private val pidProvider: () -> Int = { Process.myPid() },
    private val processProvider: (pid: Int) -> java.lang.Process = { pid ->
        ProcessBuilder("logcat", "-v", "threadtime", "--pid=$pid").start()
    },
    var onErrorLogged: ((LogEntry) -> Unit)? = null
) {
    companion object {
        // Threadtime format: MM-DD HH:MM:SS.mmm PID TID L TAG: message
        private val THREADTIME_REGEX = Pattern.compile(
            """^(\d{2}-\d{2})\s+(\d{2}:\d{2}:\d{2}\.\d{3})\s+(\d+)\s+(\d+)\s+([VDIWEF])\s+(.*?)\s*:\s?(.*)$"""
        )

        fun parseLine(line: String, defaultYear: Int? = null, nowMs: Long = System.currentTimeMillis()): LogEntry? {
            val matcher = THREADTIME_REGEX.matcher(line)
            if (!matcher.matches()) return null

            val dateStr = matcher.group(1) ?: return null
            val timeStr = matcher.group(2) ?: return null
            val pid = matcher.group(3)?.toIntOrNull() ?: 0
            val tid = matcher.group(4)?.toIntOrNull() ?: 0
            val levelChar = matcher.group(5)?.firstOrNull() ?: 'D'
            val tag = matcher.group(6)?.trim() ?: ""
            val message = matcher.group(7) ?: ""

            val timestamp = parseTimestamp(dateStr, timeStr, defaultYear, nowMs)
            return LogEntry(
                timestamp = timestamp,
                pid = pid,
                tid = tid,
                level = LogLevel.fromLetter(levelChar),
                tag = tag,
                message = message,
                rawTimestamp = "$dateStr $timeStr"
            )
        }

        private fun parseTimestamp(dateStr: String, timeStr: String, explicitYear: Int?, fallbackMs: Long): Long {
            return try {
                val year = explicitYear ?: Calendar.getInstance().apply { timeInMillis = fallbackMs }.get(Calendar.YEAR)
                val sdf = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US).apply {
                    timeZone = TimeZone.getDefault()
                }
                sdf.parse("$year-$dateStr $timeStr")?.time ?: fallbackMs
            } catch (_: Exception) {
                fallbackMs
            }
        }
    }

    private val isRunning = AtomicBoolean(false)
    private var readerThread: Thread? = null
    private var activeProcess: java.lang.Process? = null

    val running: Boolean get() = isRunning.get()

    @Synchronized
    fun start() {
        if (isRunning.getAndSet(true)) return

        val pid = try {
            pidProvider()
        } catch (_: Exception) {
            0
        }

        val thread = Thread({
            runLoop(pid)
        }, "TexureLogcatReader").apply {
            isDaemon = true
        }
        readerThread = thread
        thread.start()
    }

    private fun runLoop(pid: Int) {
        var process: java.lang.Process? = null
        try {
            process = processProvider(pid)
            activeProcess = process
            val inputStream = process.inputStream
            BufferedReader(InputStreamReader(inputStream, Charsets.UTF_8)).use { reader ->
                var line: String?
                while (isRunning.get()) {
                    line = reader.readLine() ?: break
                    consumeLine(line)
                }
            }
        } catch (e: Exception) {
            if (isRunning.get()) {
                Log.w("TexureBridge", "Logcat reader interrupted or failed: ${e.message}")
            }
        } finally {
            try { process?.destroy() } catch (_: Exception) {}
            activeProcess = null
            isRunning.set(false)
        }
    }

    fun consumeLine(line: String) {
        val entry = parseLine(line)
        if (entry != null) {
            ringBuffer.add(entry)
            if (entry.level == LogLevel.ERROR) {
                try {
                    onErrorLogged?.invoke(entry)
                } catch (e: Exception) {
                    Log.w("TexureBridge", "Error in onErrorLogged callback: ${e.message}")
                }
            }
        } else if (line.startsWith("    at ") || line.startsWith("Caused by: ") || line.startsWith("\tat ")) {
            // Continuation line for stack trace
            ringBuffer.appendToLastMessage(line)
        }
    }

    @Synchronized
    fun stop() {
        if (!isRunning.getAndSet(false)) return

        try {
            activeProcess?.destroy()
        } catch (_: Exception) {}
        activeProcess = null

        readerThread?.interrupt()
        try {
            readerThread?.join(1000)
        } catch (_: InterruptedException) {}
        readerThread = null
    }
}
