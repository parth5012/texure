package com.texure.bridge.logcat

import java.util.ArrayDeque

class LogRingBuffer(val capacity: Int = DEFAULT_CAPACITY) {
    companion object {
        const val DEFAULT_CAPACITY = 2000
    }

    private val lock = Any()
    private val buffer = ArrayDeque<LogEntry>(capacity.coerceAtLeast(1))

    fun add(entry: LogEntry) = synchronized(lock) {
        if (buffer.size >= capacity) {
            buffer.removeFirst()
        }
        buffer.addLast(entry)
    }

    fun appendToLastMessage(extraText: String) = synchronized(lock) {
        if (buffer.isNotEmpty()) {
            val last = buffer.removeLast()
            val updated = last.copy(message = "${last.message}\n$extraText")
            buffer.addLast(updated)
        }
    }

    val size: Int get() = synchronized(lock) { buffer.size }

    fun clear() = synchronized(lock) {
        buffer.clear()
    }

    fun getAll(): List<LogEntry> = synchronized(lock) {
        buffer.toList()
    }

    fun query(
        level: LogLevel? = null,
        tag: String? = null,
        since: Long? = null,
        limit: Int = 100
    ): List<LogEntry> = synchronized(lock) {
        val minPriority = level?.priority ?: 0
        val clampedLimit = limit.coerceIn(1, 1000)

        var filtered = buffer.filter { entry ->
            (entry.level.priority >= minPriority) &&
            (tag.isNullOrBlank() || entry.tag.contains(tag, ignoreCase = true)) &&
            (since == null || entry.timestamp > since)
        }

        if (filtered.size > clampedLimit) {
            filtered = if (since != null) {
                filtered.take(clampedLimit)
            } else {
                filtered.takeLast(clampedLimit)
            }
        }
        filtered
    }
}
