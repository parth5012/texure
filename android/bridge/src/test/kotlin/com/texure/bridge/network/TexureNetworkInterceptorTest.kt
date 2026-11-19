package com.texure.bridge.network

import okhttp3.Call
import okhttp3.Connection
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.concurrent.TimeUnit

class TexureNetworkInterceptorTest {

    @Before
    fun setUp() {
        TexureNetworkInterceptor.reset()
    }

    @Test
    fun testDefaultPassthrough() {
        val request = Request.Builder()
            .url("https://api.example.com/data")
            .build()

        var proceedCalled = false
        val chain = createTestChain(request) { req ->
            proceedCalled = true
            Response.Builder()
                .request(req)
                .protocol(Protocol.HTTP_1_1)
                .code(200)
                .message("OK")
                .body("real response".toResponseBody("text/plain".toMediaTypeOrNull()))
                .build()
        }

        val response = TexureNetworkInterceptor.intercept(chain)
        assertTrue(proceedCalled)
        assertEquals(200, response.code)
        assertEquals("real response", response.body?.string())

        // History recorded
        val history = TexureNetworkInterceptor.getHistory()
        assertEquals(1, history.size)
        assertEquals("https://api.example.com/data", history[0].url)
        assertEquals("GET", history[0].method)
        assertEquals(200, history[0].status)
    }

    @Test
    fun testSyntheticMockResponse() {
        // Register mock rule
        val rule = MockRule(
            id = "rule-1",
            method = "GET",
            urlPattern = Regex("^https://api\\.example\\.com/users/.*$"),
            status = 201,
            headers = mapOf("X-Mock" to "true", "Content-Type" to "application/json"),
            body = "{\"user\": \"alice\"}",
        )
        TexureNetworkInterceptor.addMock(rule)

        val request = Request.Builder()
            .url("https://api.example.com/users/42")
            .build()

        var proceedCalled = false
        val chain = createTestChain(request) {
            proceedCalled = true
            error("Real network should not be reached when mock matches")
        }

        val response = TexureNetworkInterceptor.intercept(chain)
        assertFalse("Real network must not be called", proceedCalled)
        assertEquals(201, response.code)
        assertEquals("true", response.header("X-Mock"))
        assertEquals("{\"user\": \"alice\"}", response.body?.string())

        val history = TexureNetworkInterceptor.getHistory()
        assertEquals(1, history.size)
        assertEquals("https://api.example.com/users/42", history[0].url)
        assertEquals(201, history[0].status)
    }

    @Test
    fun testMockRuleTimesExpiry() {
        val rule = MockRule(
            id = "rule-single",
            method = "POST",
            urlPattern = Regex(".*checkout.*"),
            status = 200,
            headers = emptyMap(),
            body = "{\"status\": \"mocked\"}",
            times = 1,
        )
        TexureNetworkInterceptor.addMock(rule)

        val request = Request.Builder()
            .url("https://shop.example.com/checkout")
            .post("{\"item\":\"widget\"}".toRequestBody("application/json".toMediaTypeOrNull()))
            .build()

        var proceedCount = 0
        val chain = createTestChain(request) { req ->
            proceedCount++
            Response.Builder()
                .request(req)
                .protocol(Protocol.HTTP_1_1)
                .code(200)
                .message("OK")
                .body("real response".toResponseBody("text/plain".toMediaTypeOrNull()))
                .build()
        }

        // First call matches mock
        val resp1 = TexureNetworkInterceptor.intercept(chain)
        assertEquals(0, proceedCount)
        assertEquals("{\"status\": \"mocked\"}", resp1.body?.string())

        // Second call should pass through because times == 1 expired
        val resp2 = TexureNetworkInterceptor.intercept(chain)
        assertEquals(1, proceedCount)
        assertEquals("real response", resp2.body?.string())

        val history = TexureNetworkInterceptor.getHistory()
        assertEquals(2, history.size)
    }

    @Test
    fun testMockRuleDelay() {
        val rule = MockRule(
            id = "rule-delay",
            method = "GET",
            urlPattern = Regex(".*slow.*"),
            status = 200,
            headers = emptyMap(),
            body = "ok",
            delayMs = 60,
        )
        TexureNetworkInterceptor.addMock(rule)

        val request = Request.Builder().url("https://api.example.com/slow").build()
        val chain = createTestChain(request) {
            error("Mock should match")
        }

        val start = System.currentTimeMillis()
        val response = TexureNetworkInterceptor.intercept(chain)
        val elapsed = System.currentTimeMillis() - start

        assertEquals(200, response.code)
        assertTrue("Delay should be at least 50ms, was $elapsed ms", elapsed >= 50)
    }

    @Test
    fun testHistoryRingBufferCapAt100() {
        for (i in 1..125) {
            val request = Request.Builder().url("https://api.example.com/req/$i").build()
            val chain = createTestChain(request) { req ->
                Response.Builder()
                    .request(req)
                    .protocol(Protocol.HTTP_1_1)
                    .code(200)
                    .message("OK")
                    .body("".toResponseBody(null))
                    .build()
            }
            TexureNetworkInterceptor.intercept(chain)
        }

        val history = TexureNetworkInterceptor.getHistory()
        assertEquals(100, history.size)
        // First request remaining should be req/26 (since 1..25 were dropped)
        assertEquals("https://api.example.com/req/26", history[0].url)
        assertEquals("https://api.example.com/req/125", history[99].url)
    }

    @Test
    fun testHistoryFilter() {
        val urls = listOf(
            "https://api.example.com/auth/login",
            "https://api.example.com/users/list",
            "https://api.example.com/auth/refresh",
            "https://api.example.com/items/42",
        )
        for (url in urls) {
            val req = Request.Builder().url(url).build()
            val chain = createTestChain(req) { r ->
                Response.Builder()
                    .request(r)
                    .protocol(Protocol.HTTP_1_1)
                    .code(200)
                    .message("OK")
                    .body("".toResponseBody(null))
                    .build()
            }
            TexureNetworkInterceptor.intercept(chain)
        }

        val authHistory = TexureNetworkInterceptor.getHistory(urlContains = "auth")
        assertEquals(2, authHistory.size)
        assertEquals("https://api.example.com/auth/login", authHistory[0].url)
        assertEquals("https://api.example.com/auth/refresh", authHistory[1].url)
    }

    @Test
    fun testMockingDisabledMode() {
        TexureNetworkInterceptor.addMock(
            MockRule(
                id = "rule-disabled",
                method = "GET",
                urlPattern = Regex(".*"),
                status = 500,
                headers = emptyMap(),
                body = "error",
            ),
        )
        TexureNetworkInterceptor.setMockingEnabled(false)
        assertFalse(TexureNetworkInterceptor.isMockingEnabled())

        var proceedCalled = false
        val request = Request.Builder().url("https://api.example.com/test").build()
        val chain = createTestChain(request) { req ->
            proceedCalled = true
            Response.Builder()
                .request(req)
                .protocol(Protocol.HTTP_1_1)
                .code(200)
                .message("OK")
                .body("passthrough".toResponseBody(null))
                .build()
        }

        val response = TexureNetworkInterceptor.intercept(chain)
        assertTrue("When mocking is disabled, requests must pass through", proceedCalled)
        assertEquals(200, response.code)
    }

    private fun createTestChain(
        request: Request,
        onProceed: (Request) -> Response,
    ): Interceptor.Chain = object : Interceptor.Chain {
        override fun request(): Request = request
        override fun proceed(request: Request): Response = onProceed(request)
        override fun connection(): Connection? = null
        override fun call(): Call = throw UnsupportedOperationException()
        override fun connectTimeoutMillis(): Int = 1000
        override fun withConnectTimeout(timeout: Int, unit: TimeUnit): Interceptor.Chain = this
        override fun readTimeoutMillis(): Int = 1000
        override fun withReadTimeout(timeout: Int, unit: TimeUnit): Interceptor.Chain = this
        override fun writeTimeoutMillis(): Int = 1000
        override fun withWriteTimeout(timeout: Int, unit: TimeUnit): Interceptor.Chain = this
    }
}
