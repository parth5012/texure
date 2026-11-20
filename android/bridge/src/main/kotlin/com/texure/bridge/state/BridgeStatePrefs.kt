package com.texure.bridge.state

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject

/**
 * SharedPreferences inspector and seeder engine (Ticket #70, Map #74).
 * Provides read/write access to default or named SharedPreferences in debug builds.
 */
class BridgeStatePrefs(private val context: Context) {

    fun getDefaultPrefsName(): String {
        return "${context.packageName}_preferences"
    }

    fun getSharedPreferences(name: String?): SharedPreferences {
        val fileName = if (name.isNullOrBlank()) getDefaultPrefsName() else name
        return context.getSharedPreferences(fileName, Context.MODE_PRIVATE)
    }

    /**
     * Inspects key-value entries across default or named SharedPreferences.
     */
    fun inspect(name: String?): JSONObject {
        val resolvedName = if (name.isNullOrBlank()) getDefaultPrefsName() else name
        val prefs = getSharedPreferences(resolvedName)
        val allEntries = prefs.all

        val entriesJson = JSONObject()
        for ((key, value) in allEntries) {
            when (value) {
                is Set<*> -> {
                    val arr = JSONArray()
                    for (item in value) {
                        arr.put(item?.toString() ?: "")
                    }
                    entriesJson.put(key, arr)
                }
                is Float -> {
                    entriesJson.put(key, value.toDouble())
                }
                else -> {
                    entriesJson.put(key, value ?: JSONObject.NULL)
                }
            }
        }

        return JSONObject().apply {
            put("file", resolvedName)
            put("entries", entriesJson)
        }
    }

    /**
     * Seeds or removes key-value entries synchronously via SharedPreferences.Editor.commit().
     * Supports String, Boolean, Int, Long, Float, StringSet.
     */
    fun mutate(payload: JSONObject, onMutated: (() -> Unit)? = null): JSONObject {
        val fileName = payload.optString("file").takeIf { it.isNotBlank() }
        val resolvedName = fileName ?: getDefaultPrefsName()
        val prefs = getSharedPreferences(resolvedName)
        val editor = prefs.edit()

        val setObj = payload.optJSONObject("set")
        if (setObj != null) {
            val keys = setObj.keys()
            val existing = prefs.all

            while (keys.hasNext()) {
                val key = keys.next()
                val value = setObj.get(key)
                val existingVal = existing[key]

                if (existingVal is Long && value is Number) {
                    editor.putLong(key, value.toLong())
                } else if (existingVal is Float && value is Number) {
                    editor.putFloat(key, value.toFloat())
                } else if (existingVal is Int && value is Number) {
                    editor.putInt(key, value.toInt())
                } else {
                    when (value) {
                        is Boolean -> editor.putBoolean(key, value)
                        is Int -> editor.putInt(key, value)
                        is Long -> editor.putLong(key, value)
                        is Double -> editor.putFloat(key, value.toFloat())
                        is Float -> editor.putFloat(key, value)
                        is String -> editor.putString(key, value)
                        is JSONArray -> {
                            val set = mutableSetOf<String>()
                            for (i in 0 until value.length()) {
                                set.add(value.getString(i))
                            }
                            editor.putStringSet(key, set)
                        }
                        else -> {
                            editor.putString(key, value.toString())
                        }
                    }
                }
            }
        }

        val removeArr = payload.optJSONArray("remove")
        if (removeArr != null) {
            for (i in 0 until removeArr.length()) {
                val keyToRemove = removeArr.getString(i)
                editor.remove(keyToRemove)
            }
        }

        val committed = editor.commit()
        if (!committed) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "COMMIT_FAILED")
                    put("message", "Failed to commit SharedPreferences changes synchronously")
                })
            }
        }

        onMutated?.invoke()
        return JSONObject().apply {
            put("ok", true)
        }
    }
}
