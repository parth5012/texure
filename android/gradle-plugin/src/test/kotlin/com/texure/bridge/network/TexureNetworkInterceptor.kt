package com.texure.bridge.network

import okhttp3.Interceptor
import okhttp3.Response

/**
 * Test stub for verifying class loading and execution of bytecode-woven OkHttp classes.
 */
object TexureNetworkInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        return chain.proceed(chain.request())
    }
}
