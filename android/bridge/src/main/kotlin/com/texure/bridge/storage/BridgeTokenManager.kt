package com.texure.bridge.storage

import android.content.Context
import android.content.SharedPreferences
import java.util.UUID

class BridgeTokenManager(context: Context) {

    private val prefs: SharedPreferences = context.getSharedPreferences(
        PREFS_NAME,
        Context.MODE_PRIVATE
    )

    @Synchronized
    fun getToken(): String {
        var token = prefs.getString(KEY_TOKEN, null)
        if (token.isNullOrEmpty()) {
            token = rotateToken()
        }
        return token
    }

    fun saveToken(token: String) {
        prefs.edit().putString(KEY_TOKEN, token).apply()
    }

    fun rotateToken(): String {
        val newToken = UUID.randomUUID().toString().replace("-", "")
        saveToken(newToken)
        return newToken
    }

    fun validateToken(headerValue: String?): Boolean {
        if (headerValue.isNullOrBlank()) return false
        val token = if (headerValue.startsWith("Bearer ", ignoreCase = true)) {
            headerValue.substring(7).trim()
        } else {
            headerValue.trim()
        }
        val expectedToken = getToken()
        return java.security.MessageDigest.isEqual(
            token.toByteArray(Charsets.UTF_8),
            expectedToken.toByteArray(Charsets.UTF_8)
        )
    }

    companion object {
        private const val PREFS_NAME = "texure_bridge_pairing"
        private const val KEY_TOKEN = "pairing_token"
    }
}
