package com.texure.bridge.qr

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Bitmap

/**
 * Minimal, dependency-free QR Code generator (ISO/IEC 18004).
 *
 * Supports versions 1-4, byte mode, ECC level L (single block). Pure Kotlin so
 * the bridge AAR carries no extra dependency and release builds stay clean (G1).
 */
object QrCode {

    private val DATA_CODEWORDS = intArrayOf(0, 19, 34, 55, 80)
    private val ECC_CODEWORDS = intArrayOf(0, 7, 10, 15, 20)
    private val BYTE_CAPACITY = intArrayOf(0, 17, 32, 53, 78)

    private val expTable = IntArray(512)
    private val logTable = IntArray(256)

    init {
        var x = 1
        for (i in 0..254) {
            expTable[i] = x
            logTable[x] = i
            x = x shl 1
            if (x and 0x100 != 0) x = x xor 0x11D
        }
        for (i in 255..511) expTable[i] = expTable[i - 255]
    }

    private fun penalty(m: Array<IntArray>, size: Int): Int {
        var p = 0
        // Level 1: runs of 5+ same color
        val container = IntArray(size + 1)
        for (row in 0 until size) {
            val thisRow = m[row]
            var prev = thisRow[0]
            var length = 0
            for (col in 0 until size) {
                if (thisRow[col] == prev) {
                    length++
                } else {
                    if (length >= 5) container[length]++
                    length = 1
                    prev = thisRow[col]
                }
            }
            if (length >= 5) container[length]++
        }
        for (col in 0 until size) {
            var prev = m[0][col]
            var length = 0
            for (row in 0 until size) {
                if (m[row][col] == prev) {
                    length++
                } else {
                    if (length >= 5) container[length]++
                    length = 1
                    prev = m[row][col]
                }
            }
            if (length >= 5) container[length]++
        }
        for (len in 5..size) {
            p += container[len] * (len - 2)
        }
        // Level 2: 2x2 blocks
        for (row in 0 until size - 1) {
            for (col in 0 until size - 1) {
                val tr = m[row][col + 1]
                if (tr != m[row + 1][col + 1]) continue
                if (tr != m[row][col]) continue
                if (tr != m[row + 1][col]) continue
                p += 3
            }
        }
        // Level 3: alternating patterns
        for (row in 0 until size) {
            val thisRow = m[row]
            var col = 0
            while (col < size - 10) {
                if (
                    thisRow[col + 1] == 0 &&
                    thisRow[col + 4] == 1 &&
                    thisRow[col + 5] == 0 &&
                    thisRow[col + 6] == 1 &&
                    thisRow[col + 9] == 0 &&
                    (
                        (thisRow[col + 0] == 1 && thisRow[col + 2] == 1 && thisRow[col + 3] == 1 && thisRow[col + 7] == 0 && thisRow[col + 8] == 0 && thisRow[col + 10] == 0) ||
                        (thisRow[col + 0] == 0 && thisRow[col + 2] == 0 && thisRow[col + 3] == 0 && thisRow[col + 7] == 1 && thisRow[col + 8] == 1 && thisRow[col + 10] == 1)
                    )
                ) {
                    p += 40
                    if (thisRow[col + 10] == 1) {
                        col += 2 // skip (horspool min 2)
                    } else {
                        col += 1
                    }
                } else {
                    col += 1
                }
            }
        }
        for (col in 0 until size) {
            var row = 0
            while (row < size - 10) {
                if (
                    m[row + 1][col] == 0 &&
                    m[row + 4][col] == 1 &&
                    m[row + 5][col] == 0 &&
                    m[row + 6][col] == 1 &&
                    m[row + 9][col] == 0 &&
                    (
                        (m[row + 0][col] == 1 && m[row + 2][col] == 1 && m[row + 3][col] == 1 && m[row + 7][col] == 0 && m[row + 8][col] == 0 && m[row + 10][col] == 0) ||
                        (m[row + 0][col] == 0 && m[row + 2][col] == 0 && m[row + 3][col] == 0 && m[row + 7][col] == 1 && m[row + 8][col] == 1 && m[row + 10][col] == 1)
                    )
                ) {
                    p += 40
                    if (m[row + 10][col] == 1) {
                        row += 2
                    } else {
                        row += 1
                    }
                } else {
                    row += 1
                }
            }
        }
        // Level 4: balance
        var dark = 0
        for (y in 0 until size) for (x in 0 until size) if (m[y][x] == 1) dark++
        val percent = dark.toDouble() / (size * size)
        val rating = (kotlin.math.abs(percent * 100 - 50) / 5).toInt()
        p += rating * 10
        return p
    }

    private fun maskBit(mask: Int, x: Int, y: Int): Int {
        val cond = when (mask) {
            0 -> (x + y) % 2 == 0
            1 -> y % 2 == 0
            2 -> x % 3 == 0
            3 -> (x + y) % 3 == 0
            4 -> (x / 3 + y / 2) % 2 == 0
            5 -> ((x * y) % 2 + (x * y) % 3) == 0
            6 -> (((x * y) % 2) + ((x * y) % 3)) % 2 == 0
            7 -> (((x + y) % 2) + ((x * y) % 3)) % 2 == 0
            else -> false
        }
        return if (cond) 1 else 0
    }

    private fun gfMul(a: Int, b: Int): Int {
        if (a == 0 || b == 0) return 0
        return expTable[logTable[a] + logTable[b]]
    }

    private fun generatorPoly(nsym: Int): IntArray {
        val poly = IntArray(nsym + 1)
        poly[0] = 1
        var degree = 0
        for (i in 0 until nsym) {
            val root = expTable[i] // alpha^i
            // multiply poly by (x + root)
            degree++
            for (j in degree downTo 1) {
                poly[j] = poly[j] xor gfMul(poly[j - 1], root)
            }
        }
        return poly
    }

    private fun rsEncode(data: ByteArray, nsym: Int): ByteArray {
        val gen = generatorPoly(nsym)
        val res = ByteArray(data.size + nsym)
        data.copyInto(res)
        for (i in data.indices) {
            val coef = res[i].toInt() and 0xFF
            if (coef != 0) {
                for (j in 1..nsym) {
                    res[i + j] = ((res[i + j].toInt() and 0xFF) xor gfMul(gen[j], coef)).toByte()
                }
            }
        }
        return res.copyOfRange(data.size, res.size)
    }

    private fun buildData(payloadBytes: ByteArray, capacity: Int): ByteArray {
        val bits = mutableListOf<Int>()
        fun append(value: Int, length: Int) {
            for (i in length - 1 downTo 0) bits.add((value shr i) and 1)
        }
        append(0b0100, 4) // byte mode
        append(payloadBytes.size, 8)
        for (b in payloadBytes) append(b.toInt() and 0xFF, 8)
        val totalBits = capacity * 8
        val term = minOf(4, totalBits - bits.size)
        if (term > 0) append(0, term)
        while (bits.size % 8 != 0) bits.add(0)
        val out = ByteArray(capacity)
        var idx = 0
        var i = 0
        while (i < bits.size && idx < capacity) {
            var v = 0
            for (j in 0..7) {
                v = (v shl 1) or (if (i + j < bits.size) bits[i + j] else 0)
            }
            out[idx++] = v.toByte()
            i += 8
        }
        var pad = 0xEC
        while (idx < capacity) {
            out[idx++] = pad.toByte()
            pad = if (pad == 0xEC) 0x11 else 0xEC
        }
        return out
    }

    /**
     * Encodes [payload] into a QR matrix (true = dark module) or null if the
     * payload exceeds version-4 byte capacity.
     */
    fun encode(payload: String, mask: Int = -1): Array<BooleanArray>? {
        val bytes = payload.toByteArray(Charsets.UTF_8)
        val version = (1..4).firstOrNull { BYTE_CAPACITY[it] >= bytes.size } ?: return null

        val dataCw = DATA_CODEWORDS[version]
        val eccCw = ECC_CODEWORDS[version]
        val data = buildData(bytes, dataCw)
        val ecc = rsEncode(data, eccCw)
        val all = data + ecc

        val size = 4 * version + 17
        val reserved = Array(size) { BooleanArray(size) }
        val base = Array(size) { IntArray(size) { 0 } }
        val isData = Array(size) { BooleanArray(size) }

        // Finder patterns + separators
        fun reserveFinder(x0: Int, y0: Int) {
            for (y in y0 - 1..y0 + 7) {
                for (x in x0 - 1..x0 + 7) {
                    if (x in 0 until size && y in 0 until size) {
                        reserved[y][x] = true
                        base[y][x] = 0
                    }
                }
            }
            for (dy in 0..6) {
                for (dx in 0..6) {
                    val on = (dx == 0 || dx == 6 || dy == 0 || dy == 6) ||
                        (dx in 2..4 && dy in 2..4)
                    base[y0 + dy][x0 + dx] = if (on) 1 else 0
                }
            }
        }
        reserveFinder(0, 0)
        reserveFinder(size - 7, 0)
        reserveFinder(0, size - 7)

        // Timing patterns
        for (i in 0 until size) {
            if (!reserved[6][i]) {
                reserved[6][i] = true
                base[6][i] = if (i % 2 == 0) 1 else 0
            }
            if (!reserved[i][6]) {
                reserved[i][6] = true
                base[i][6] = if (i % 2 == 0) 1 else 0
            }
        }

        // Alignment patterns (version >= 2)
        if (version >= 2) {
            val positions = when (version) {
                2 -> intArrayOf(6, 18)
                3 -> intArrayOf(6, 22)
                4 -> intArrayOf(6, 26)
                else -> intArrayOf(6)
            }
            for (cy in positions) {
                for (cx in positions) {
                    // skip if overlapping a finder
                    if (cx <= 8 && cy <= 8) continue
                    if (cx >= size - 8 && cy <= 8) continue
                    if (cx <= 8 && cy >= size - 8) continue
                    // place 5x5
                    var overlap = false
                    for (dy in -2..2) for (dx in -2..2) {
                        val x = cx + dx
                        val y = cy + dy
                        if (x in 0 until size && y in 0 until size && reserved[y][x]) overlap = true
                    }
                    if (overlap) continue
                    for (dy in -2..2) {
                        for (dx in -2..2) {
                            val x = cx + dx
                            val y = cy + dy
                            if (x !in 0 until size || y !in 0 until size) continue
                            reserved[y][x] = true
                            val on = (kotlin.math.abs(dx) == 2 || kotlin.math.abs(dy) == 2) || (dx == 0 && dy == 0)
                            base[y][x] = if (on) 1 else 0
                        }
                    }
                }
            }
        }

        // Dark module
        val darkX = 8
        val darkY = size - 8
        if (!reserved[darkY][darkX]) {
            reserved[darkY][darkX] = true
        }
        base[darkY][darkX] = 1

        // Format info positions (Python setup_type_info ordering):
        // vertical column 8
        val vPositions = mutableListOf<Pair<Int, Int>>()
        for (i in 0..14) {
            val r = if (i < 6) i else if (i < 8) i + 1 else size - 15 + i
            vPositions.add(Pair(8, r))
        }
        // horizontal row 8
        val hPositions = mutableListOf<Pair<Int, Int>>()
        for (i in 0..14) {
            val c = if (i < 8) size - i - 1 else if (i < 9) 7 else 14 - i
            hPositions.add(Pair(c, 8))
        }
        for (p in vPositions + hPositions) {
            val (x, y) = p
            if (!reserved[y][x]) { reserved[y][x] = true; base[y][x] = 0 }
        }

        // Collect data positions in zigzag order
        val dataPositions = mutableListOf<Pair<Int, Int>>()
        var upward = true
        var x = size - 1
        while (x > 0) {
            if (x == 6) x = 5
            val xs = listOf(x, x - 1)
            val ys = if (upward) (size - 1 downTo 0).toList() else (0 until size).toList()
            for (y in ys) {
                for (xx in xs) {
                    if (xx < 0) continue
                    if (!reserved[y][xx]) {
                        dataPositions.add(Pair(xx, y))
                    }
                }
            }
            upward = !upward
            x -= 2
        }

        // Place raw bits
        val bits = all.flatMap { cw -> (7 downTo 0).map { (cw.toInt() and 0xFF shr it) and 1 } }
        for (i in dataPositions.indices) {
            val (xx, yy) = dataPositions[i]
            val bit = if (i < bits.size) bits[i] else 0
            base[yy][xx] = bit
            isData[yy][xx] = true
        }

        // Choose best mask if not specified
        val chosenMask = if (mask in 0..7) mask else {
            var best = 0
            var bestPen = Int.MAX_VALUE
            for (m in 0..7) {
                val cand = Array(size) { y -> IntArray(size) { x -> base[y][x] } }
                for (y in 0 until size) for (x in 0 until size) {
                    if (isData[y][x]) cand[y][x] = base[y][x] xor maskBit(m, x, y)
                }
                // Format-info positions and the fixed dark module stay 0 (light)
                // during penalty evaluation to match Python qrcode's test mode.
                val pen = penalty(cand, size)
                if (pen < bestPen) { bestPen = pen; best = m }
            }
            best
        }

        // Format info bits
        val ecl = 1 // L
        val maskNum = chosenMask
        var formatInfo = (ecl shl 3) or maskNum
        formatInfo = formatInfo shl 10
        for (i in 14 downTo 10) {
            if ((formatInfo and (1 shl i)) != 0) {
                formatInfo = formatInfo xor (0x537 shl (i - 10))
            }
        }
        formatInfo = (((ecl shl 3) or maskNum) shl 10) or formatInfo xor 0x5412

        // Apply chosen mask to data positions
        for (y in 0 until size) {
            for (x in 0 until size) {
                if (isData[y][x]) {
                    base[y][x] = base[y][x] xor maskBit(chosenMask, x, y)
                }
            }
        }

        // Place format info bits into formatPositions in order (i=0..14 for each copy)
        // Copy1 positions already in formatPositions? We added vertical/horizontal interleaved; need order matching i.
        // Rebuild format position order per standard:
        // We'll just overwrite all format positions using a computed list in correct bit order.
        // Build ordered lists:
        // vPositions and hPositions already computed above (in reserved marking).
        val vPositions2 = mutableListOf<Pair<Int, Int>>()
        for (i in 0..14) {
            val r = if (i < 6) i else if (i < 8) i + 1 else size - 15 + i
            vPositions2.add(Pair(8, r))
        }
        val hPositions2 = mutableListOf<Pair<Int, Int>>()
        for (i in 0..14) {
            val c = if (i < 8) size - i - 1 else if (i < 9) 7 else 14 - i
            hPositions2.add(Pair(c, 8))
        }
        for (positions in listOf(vPositions2, hPositions2)) {
            for (i in 0..14) {
                val bit = (formatInfo shr i) and 1
                val (x, y) = positions[i]
                base[y][x] = bit
            }
        }
        // fixed dark module
        base[size - 8][8] = 1

        // Convert
        val matrix = Array(size) { BooleanArray(size) }
        for (y in 0 until size) {
            for (x in 0 until size) {
                matrix[y][x] = base[y][x] == 1
            }
        }
        return matrix
    }

    /**
     * Renders [matrix] into [canvas] filling a [sizePx] x [sizePx] square.
     */
    fun render(canvas: Canvas, matrix: Array<BooleanArray>, sizePx: Int) {
        val n = matrix.size
        val module = sizePx / n
        val total = module * n
        val light = Paint().apply { color = Color.WHITE; style = Paint.Style.FILL }
        val dark = Paint().apply { color = Color.BLACK; style = Paint.Style.FILL }
        canvas.drawRect(0f, 0f, total.toFloat(), total.toFloat(), light)
        for (y in 0 until n) {
            for (x in 0 until n) {
                if (matrix[y][x]) {
                    canvas.drawRect(
                        (x * module).toFloat(),
                        (y * module).toFloat(),
                        ((x + 1) * module).toFloat(),
                        ((y + 1) * module).toFloat(),
                        dark
                    )
                }
            }
        }
    }

    fun createBitmap(matrix: Array<BooleanArray>, sizePx: Int): Bitmap {
        val bmp = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        render(canvas, matrix, sizePx)
        return bmp
    }
}
