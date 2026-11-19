package com.texure.bridge.network

/**
 * An active mock rule registered with Texure network mocking.
 */
data class MockRule(
    val id: String,
    val method: String,
    val urlPattern: Regex,
    val status: Int,
    val headers: Map<String, String> = emptyMap(),
    val body: String = "",
    val delayMs: Long = 0L,
    var times: Int? = null,
) {
    /**
     * Checks if this rule matches an incoming HTTP method and URL.
     * Method matching is case-insensitive, with "*" or empty matching all methods.
     */
    fun matches(requestMethod: String, requestUrl: String): Boolean {
        val methodMatches = method.isBlank() || method == "*" || method.equals(requestMethod, ignoreCase = true)
        return methodMatches && urlPattern.containsMatchIn(requestUrl)
    }
}

/**
 * A captured HTTP request recorded in the rolling history buffer.
 */
data class CapturedRequest(
    val url: String,
    val method: String,
    val status: Int,
    val requestBody: String?,
    val latencyMs: Long,
    val timestamp: Long,
)
