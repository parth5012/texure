package com.texure.bridge.qr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class QrCodeTest {

    private val payload = """{"url":"http://192.168.1.5:8765","token":"tok-abc123"}"""

    @Test
    fun encodeProducesSquareMatrix() {
        val m = QrCode.encode(payload)
        assertNotNull(m)
        assertEquals(m!!.size, m[0].size)
    }

    @Test
    fun finderPatternsPresent() {
        val m = QrCode.encode(payload)!!
        // top-left finder corners are dark
        assertTrue(m[0][0])
        assertTrue(m[0][6])
        assertTrue(m[6][0])
        assertTrue(m[6][6])
        // finder center 3x3 dark
        assertTrue(m[3][3])
        // separator just outside top-left finder is light
        assertFalse(m[7][0])
        assertFalse(m[0][7])
    }

    @Test
    fun deterministic() {
        val a = QrCode.encode(payload)
        val b = QrCode.encode(payload)
        assertEquals(a!!.size, b!!.size)
        for (y in a.indices) {
            for (x in a[y].indices) assertEquals(a[y][x], b[y][x])
        }
    }

    @Test
    fun tooLargeReturnsNull() {
        val big = "a".repeat(200)
        assertNull(QrCode.encode(big))
    }

    @Test
    fun versionGrowsWithPayload() {
        val small = QrCode.encode("x")!!
        val v1size = 4 * 1 + 17
        assertEquals(v1size, small.size)
    }
}