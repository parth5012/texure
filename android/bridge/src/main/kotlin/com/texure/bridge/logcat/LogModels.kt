package com.texure.bridge.logcat

import org.json.JSONObject

enum class LogLevel(val letter: Char, val priority: Int) {
    VERBOSE('V', 2),
    DEBUG('D', 3),
    INFO('I', 4),
    WARN('W', 5),
    ERROR('E', 6);

    companion object {
        fun fromLetter(c: Char): LogLevel = when (Character.toUpperCase(c)) {
            'V' -> VERBOSE
            'D' -> DEBUG
            'I' -> INFO
            'W' -> WARN
            'E', 'F' -> ERROR
            else -> DEBUG
        }

        fun fromString(s: String?): LogLevel? {
            if (s.isNullOrBlank()) return null
            val upper = s.trim().uppercase()
            return when {
                upper == "V" || upper == "VERBOSE" -> VERBOSE
                upper == "D" || upper == "DEBUG" -> DEBUG
                upper == "I" || upper == "INFO" -> INFO
                upper == "W" || upper == "WARN" || upper == "WARNING" -> WARN
                upper == "E" || upper == "ERROR" || upper == "F" || upper == "FATAL" -> ERROR
                else -> null
            }
        }
    }
}

data class LogEntry(
    val timestamp: Long,
    val pid: Int,
    val tid: Int,
    val level: LogLevel,
    val tag: String,
    val message: String,
    val rawTimestamp: String? = null
) {
    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("timestamp", timestamp)
            put("pid", pid)
            put("tid", tid)
            put("level", level.name)
            put("tag", tag)
            put("message", message)
            if (rawTimestamp != null) {
                put("raw_timestamp", rawTimestamp)
            }
        }
    }
}
