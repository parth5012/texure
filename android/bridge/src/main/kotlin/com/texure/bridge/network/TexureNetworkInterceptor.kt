package com.texure.bridge.network

import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import okio.Buffer

/**
 * Texure in-app OkHttp network interceptor.
 *
 * Implements:
 * 1. 100% passthrough by default when no mock rule matches or mocking is disabled.
 * 2. Dynamic regex mock rule matching with synthetic Response generation, configurable
 *    delay, headers, and automatic times expiration.
 * 3. Thread-safe rolling request history ring buffer capped at 100 requests.
 */
object TexureNetworkInterceptor : Interceptor {

    private const val MAX_HISTORY_SIZE = 100

    @Volatile
    private var mockingEnabled: Boolean = true

    private val rulesLock = Any()
    private val mockRules = mutableListOf<MockRule>()

    private val historyLock = Any()
    private val requestHistory = ArrayDeque<CapturedRequest>()

    override fun intercept(chain: Interceptor.Chain): Response {
        val request = chain.request()
        val requestBody = readRequestBody(request)
        val startTime = System.currentTimeMillis()

        // 1. If mocking is enabled, check for matching rule
        if (mockingEnabled) {
            val matchingRule = findAndConsumeMatchingRule(request.method, request.url.toString())
            if (matchingRule != null) {
                if (matchingRule.delayMs > 0) {
                    try {
                        Thread.sleep(matchingRule.delayMs)
                    } catch (_: InterruptedException) {
                        Thread.currentThread().interrupt()
                    }
                }
                val latencyMs = System.currentTimeMillis() - startTime
                val response = buildSyntheticResponse(request, matchingRule)
                recordHistory(
                    CapturedRequest(
                        url = request.url.toString(),
                        method = request.method,
                        status = matchingRule.status,
                        requestBody = requestBody,
                        latencyMs = latencyMs,
                        timestamp = startTime,
                    ),
                )
                return response
            }
        }

        // 2. Passthrough: execute real network call
        val response: Response
        try {
            response = chain.proceed(request)
        } catch (e: Exception) {
            val latencyMs = System.currentTimeMillis() - startTime
            recordHistory(
                CapturedRequest(
                    url = request.url.toString(),
                    method = request.method,
                    status = 0,
                    requestBody = requestBody,
                    latencyMs = latencyMs,
                    timestamp = startTime,
                ),
            )
            throw e
        }

        val latencyMs = System.currentTimeMillis() - startTime
        recordHistory(
            CapturedRequest(
                url = request.url.toString(),
                method = request.method,
                status = response.code,
                requestBody = requestBody,
                latencyMs = latencyMs,
                timestamp = startTime,
            ),
        )
        return response
    }

    private fun findAndConsumeMatchingRule(method: String, url: String): MockRule? {
        synchronized(rulesLock) {
            val iterator = mockRules.iterator()
            while (iterator.hasNext()) {
                val rule = iterator.next()
                if (rule.matches(method, url)) {
                    val times = rule.times
                    if (times != null) {
                        if (times <= 1) {
                            iterator.remove()
                        } else {
                            rule.times = times - 1
                        }
                    }
                    return rule
                }
            }
            return null
        }
    }

    private fun buildSyntheticResponse(request: Request, rule: MockRule): Response {
        val contentType = rule.headers["Content-Type"]
            ?: rule.headers["content-type"]
            ?: "application/json; charset=utf-8"
        val mediaType = contentType.toMediaTypeOrNull()
        val responseBody = rule.body.toResponseBody(mediaType)

        val builder = Response.Builder()
            .request(request)
            .protocol(Protocol.HTTP_1_1)
            .code(rule.status)
            .message(if (rule.status in 200..299) "OK" else "Mock Response")
            .body(responseBody)

        rule.headers.forEach { (k, v) ->
            builder.header(k, v)
        }
        return builder.build()
    }

    private fun readRequestBody(request: Request): String? {
        val body = request.body ?: return null
        if (body.isOneShot() || body.isDuplex()) {
            return "[one-shot / streaming body]"
        }
        return try {
            val buffer = Buffer()
            body.writeTo(buffer)
            val maxBytes = 64 * 1024L
            val byteCount = minOf(buffer.size, maxBytes)
            val prefix = buffer.readUtf8(byteCount)
            if (buffer.size > 0) "$prefix... [truncated]" else prefix
        } catch (_: Exception) {
            null
        }
    }

    private fun recordHistory(entry: CapturedRequest) {
        synchronized(historyLock) {
            while (requestHistory.size >= MAX_HISTORY_SIZE) {
                requestHistory.removeFirst()
            }
            requestHistory.addLast(entry)
        }
    }

    // --- Public API for Bridge Endpoints & Testing ---

    fun addMock(rule: MockRule) {
        synchronized(rulesLock) {
            mockRules.removeIf { it.id == rule.id }
            mockRules.add(rule)
        }
    }

    fun clearMocks() {
        synchronized(rulesLock) {
            mockRules.clear()
        }
    }

    fun getMocks(): List<MockRule> {
        synchronized(rulesLock) {
            return mockRules.toList()
        }
    }

    fun setMockingEnabled(enabled: Boolean) {
        mockingEnabled = enabled
    }

    fun isMockingEnabled(): Boolean = mockingEnabled

    fun getHistory(urlContains: String? = null): List<CapturedRequest> {
        synchronized(historyLock) {
            return if (urlContains.isNullOrBlank()) {
                requestHistory.toList()
            } else {
                requestHistory.filter { it.url.contains(urlContains) }
            }
        }
    }

    fun clearHistory() {
        synchronized(historyLock) {
            requestHistory.clear()
        }
    }

    fun reset() {
        clearMocks()
        clearHistory()
        mockingEnabled = true
    }
}
