package com.texure.bridge.intent

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.texure.bridge.tree.BridgeTreeExtractor
import org.json.JSONArray
import org.json.JSONObject

sealed class IntentDispatchResult {
    data class Success(
        val action: String,
        val uri: String?,
        val pkg: String,
        val rev: Long
    ) : IntentDispatchResult()

    data class Error(
        val code: String,
        val message: String,
        val httpStatus: Int = 400
    ) : IntentDispatchResult()
}

class BridgeIntentDispatcher(
    private val context: Context,
    private val treeExtractor: BridgeTreeExtractor,
    private val broadcastTreeChanged: ((Long) -> Unit)? = null
) {

    fun dispatch(payload: JSONObject): IntentDispatchResult {
        // 1. Guardrail G6: Package scoping enforcement
        val ownPackage = context.packageName
        val requestedPackage = payload.optString("package").takeIf { it.isNotBlank() }
        if (requestedPackage != null && requestedPackage != ownPackage) {
            return IntentDispatchResult.Error(
                code = "CROSS_PACKAGE_FORBIDDEN",
                message = "Cross-package intents forbidden by Guardrail G6. Target package '$requestedPackage' != '$ownPackage'",
                httpStatus = 400
            )
        }

        // 2. Parse URI
        val uriStr = payload.optString("uri").takeIf { it.isNotBlank() }
        val uri = if (uriStr != null) {
            try {
                Uri.parse(uriStr)
            } catch (e: Exception) {
                return IntentDispatchResult.Error(
                    code = "BAD_REQUEST",
                    message = "Invalid intent URI '$uriStr': ${e.message}",
                    httpStatus = 400
                )
            }
        } else {
            null
        }

        // 3. Action
        val action = payload.optString("action").takeIf { it.isNotBlank() }
            ?: if (uri != null) Intent.ACTION_VIEW else Intent.ACTION_MAIN

        val intent = Intent(action)
        if (uri != null) {
            intent.data = uri
        }

        // 4. Strictly package-scoped
        intent.`package` = ownPackage

        // 5. Flags
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val flagsArray = payload.optJSONArray("flags")
        if (flagsArray != null) {
            for (i in 0 until flagsArray.length()) {
                val flagName = flagsArray.optString(i)
                if (flagName.isNotBlank()) {
                    resolveFlag(flagName)?.let { flagVal ->
                        intent.addFlags(flagVal)
                    }
                }
            }
        }

        // 6. Extras (primitives: String, Int, Long, Double, Boolean)
        val extrasObj = payload.optJSONObject("extras")
        if (extrasObj != null) {
            val keys = extrasObj.keys()
            while (keys.hasNext()) {
                val key = keys.next()
                when (val value = extrasObj.opt(key)) {
                    is Boolean -> intent.putExtra(key, value)
                    is Int -> intent.putExtra(key, value)
                    is Long -> intent.putExtra(key, value)
                    is Double -> intent.putExtra(key, value)
                    is String -> intent.putExtra(key, value)
                    is Number -> {
                        if (value.toDouble() == value.toLong().toDouble()) {
                            intent.putExtra(key, value.toLong())
                        } else {
                            intent.putExtra(key, value.toDouble())
                        }
                    }
                    JSONObject.NULL, null -> {}
                    else -> intent.putExtra(key, value.toString())
                }
            }
        }

        // 7. Verify intent resolution
        val pm = context.packageManager
        val resolveInfo = pm.resolveActivity(intent, 0)
        if (resolveInfo == null) {
            return IntentDispatchResult.Error(
                code = "ACTIVITY_NOT_FOUND",
                message = "No activity found to handle intent (action=$action, uri=$uriStr) in package '$ownPackage'",
                httpStatus = 400
            )
        }

        // 8. Dispatch activity
        return try {
            context.startActivity(intent)
            treeExtractor.bumpRev()
            val newRev = treeExtractor.getRev()
            broadcastTreeChanged?.invoke(newRev)

            IntentDispatchResult.Success(
                action = action,
                uri = uriStr,
                pkg = ownPackage,
                rev = newRev
            )
        } catch (e: Exception) {
            IntentDispatchResult.Error(
                code = "INJECT_FAILED",
                message = "Failed to start activity for intent: ${e.message}",
                httpStatus = 400
            )
        }
    }

    private fun resolveFlag(flagName: String): Int? {
        return try {
            val field = Intent::class.java.getField(flagName)
            field.getInt(null)
        } catch (_: Exception) {
            null
        }
    }
}
