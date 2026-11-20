package com.texure.bridge.logcat

import org.junit.Assert.*
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.InputStream
import java.io.OutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

class BridgeLogcatReaderTest {

    @Test
    fun testRegexParsesThreadtimeFormat() {
        val line = "11-20 14:33:05.123  1234  5678 D TexureBridge: Hello world"
        val entry = BridgeLogcatReader.parseLine(line, defaultYear = 2026)
        assertNotNull(entry)
        assertEquals(1234, entry!!.pid)
        assertEquals(5678, entry.tid)
        assertEquals(LogLevel.DEBUG, entry.level)
        assertEquals("TexureBridge", entry.tag)
        assertEquals("Hello world", entry.message)
        assertEquals("11-20 14:33:05.123", entry.rawTimestamp)
        assertTrue(entry.timestamp > 0)
    }

    @Test
    fun testLogLevelMapping() {
        val levels = listOf(
            "V" to LogLevel.VERBOSE,
            "D" to LogLevel.DEBUG,
            "I" to LogLevel.INFO,
            "W" to LogLevel.WARN,
            "E" to LogLevel.ERROR,
            "F" to LogLevel.ERROR
        )

        for ((char, expected) in levels) {
            val line = "01-01 00:00:00.000    10    20 $char MyTag: msg"
            val entry = BridgeLogcatReader.parseLine(line, defaultYear = 2026)
            assertNotNull("Failed for level $char", entry)
            assertEquals(expected, entry!!.level)
        }
    }

    @Test
    fun testTagWithSpacesAndEmptyMessage() {
        val line = "08-18 13:52:33.456  1000  1001 I Activity Manager: "
        val entry = BridgeLogcatReader.parseLine(line, defaultYear = 2026)
        assertNotNull(entry)
        assertEquals("Activity Manager", entry!!.tag)
        assertEquals("", entry.message)
    }

    @Test
    fun testInvalidOrSystemHeadersIgnored() {
        assertNull(BridgeLogcatReader.parseLine("--------- beginning of main"))
        assertNull(BridgeLogcatReader.parseLine("--------- beginning of system"))
        assertNull(BridgeLogcatReader.parseLine("Random non-log line"))
    }

    @Test
    fun testContinuationLinesAppendedToPreviousMessage() {
        val ringBuffer = LogRingBuffer(10)
        val reader = BridgeLogcatReader(ringBuffer = ringBuffer)

        reader.consumeLine("11-20 10:00:00.000 100 101 E CrashApp: java.lang.NullPointerException: boom")
        reader.consumeLine("    at com.example.MainActivity.onCreate(MainActivity.kt:42)")
        reader.consumeLine("Caused by: java.lang.IllegalStateException: nested")

        assertEquals(1, ringBuffer.size)
        val entry = ringBuffer.getAll().first()
        assertEquals(LogLevel.ERROR, entry.level)
        assertTrue(entry.message.contains("NullPointerException"))
        assertTrue(entry.message.contains("MainActivity.kt:42"))
        assertTrue(entry.message.contains("Caused by: java.lang.IllegalStateException"))
    }

    @Test
    fun testRingBufferCapacityBounding() {
        val capacity = 50
        val buffer = LogRingBuffer(capacity)

        for (i in 1..100) {
            buffer.add(
                LogEntry(
                    timestamp = i.toLong(),
                    pid = 100,
                    tid = 101,
                    level = LogLevel.INFO,
                    tag = "Tag$i",
                    message = "Message $i"
                )
            )
        }

        assertEquals(50, buffer.size)
        val entries = buffer.getAll()
        assertEquals(50, entries.size)
        assertEquals("Tag51", entries.first().tag)
        assertEquals("Tag100", entries.last().tag)
    }

    @Test
    fun testRingBufferDefaultCapacityIs2000() {
        val buffer = LogRingBuffer()
        assertEquals(2000, buffer.capacity)
    }

    @Test
    fun testRingBufferQueryFilters() {
        val buffer = LogRingBuffer(100)

        buffer.add(LogEntry(1000L, 1, 1, LogLevel.VERBOSE, "Network", "req start"))
        buffer.add(LogEntry(2000L, 1, 1, LogLevel.DEBUG, "Auth", "user token"))
        buffer.add(LogEntry(3000L, 1, 1, LogLevel.INFO, "UI", "screen drawn"))
        buffer.add(LogEntry(4000L, 1, 1, LogLevel.WARN, "Network", "slow response"))
        buffer.add(LogEntry(5000L, 1, 1, LogLevel.ERROR, "CrashHandler", "unhandled exception"))

        // Level filtering
        val errors = buffer.query(level = LogLevel.ERROR)
        assertEquals(1, errors.size)
        assertEquals("CrashHandler", errors.first().tag)

        val warningsAndAbove = buffer.query(level = LogLevel.WARN)
        assertEquals(2, warningsAndAbove.size)

        // Tag substring filtering
        val netLogs = buffer.query(tag = "net")
        assertEquals(2, netLogs.size)

        // Since timestamp filtering
        val recentLogs = buffer.query(since = 3000L)
        assertEquals(2, recentLogs.size)
        assertEquals(4000L, recentLogs[0].timestamp)
        assertEquals(5000L, recentLogs[1].timestamp)

        // Limit
        val limited = buffer.query(limit = 2)
        assertEquals(2, limited.size)

        // Clear
        buffer.clear()
        assertEquals(0, buffer.size)
        assertTrue(buffer.query().isEmpty())
    }

    @Test
    fun testRingBufferThreadSafety() {
        val buffer = LogRingBuffer(500)
        val threads = 10
        val entriesPerThread = 100
        val latch = CountDownLatch(threads)

        for (t in 1..threads) {
            thread {
                try {
                    for (i in 1..entriesPerThread) {
                        buffer.add(LogEntry(i.toLong(), t, i, LogLevel.DEBUG, "T$t", "msg $i"))
                    }
                } finally {
                    latch.countDown()
                }
            }
        }

        assertTrue(latch.await(5, TimeUnit.SECONDS))
        assertEquals(500, buffer.size)
    }

    @Test
    fun testReaderProcessLifecycleAndCallback() {
        val errorLogs = mutableListOf<LogEntry>()
        val mockData = """
            11-20 12:00:00.000  10  20 I InfoTag: Normal info message
            11-20 12:00:01.000  10  20 E ErrorTag: Something blew up!
            11-20 12:00:02.000  10  20 D DebugTag: Debugging info
        """.trimIndent()

        val mockProcess = object : java.lang.Process() {
            private val stream = ByteArrayInputStream(mockData.toByteArray(Charsets.UTF_8))
            override fun getOutputStream(): OutputStream = OutputStream.nullOutputStream()
            override fun getInputStream(): InputStream = stream
            override fun getErrorStream(): InputStream = ByteArrayInputStream(ByteArray(0))
            override fun waitFor(): Int = 0
            override fun exitValue(): Int = 0
            override fun destroy() {}
        }

        val ringBuffer = LogRingBuffer(10)
        val reader = BridgeLogcatReader(
            ringBuffer = ringBuffer,
            pidProvider = { 12345 },
            processProvider = { mockProcess },
            onErrorLogged = { errorLogs.add(it) }
        )

        reader.start()
        // Wait briefly for daemon reader loop to consume mock data
        Thread.sleep(200)

        assertEquals(3, ringBuffer.size)
        assertEquals(1, errorLogs.size)
        assertEquals("ErrorTag", errorLogs.first().tag)
        assertEquals(LogLevel.ERROR, errorLogs.first().level)

        // Verify stop shuts down cleanly
        reader.stop()
        assertFalse(reader.running)

        // Verify restarting does not leak or error
        reader.start()
        Thread.sleep(100)
        reader.stop()
        assertFalse(reader.running)
    }
}
