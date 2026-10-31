package com.texure.bridge.model

import org.json.JSONArray
import org.json.JSONObject

enum class BridgeError(val message: String, val hint: String, val httpStatus: Int) {
    NO_CONFIG("no config file/env", "create ~/.config/texure/config.toml or set TEXURE_URL/TEXURE_TOKEN", 400),
    OFFLINE("device unreachable", "same LAN? app open? fg service alive?", 400),
    AUTH_FAILED("token mismatch", "re-paste fresh token from overlay", 401),
    BUSY("second client during single-flight", "wait for owner release / ask owner", 409),
    NODE_OUTDATED("tree refreshed since reference; re-run /ui-tree", "re-run device_tree, re-target", 400),
    NODE_NOT_CLICKABLE("tap onto non-clickable node", "pick a clickable node from the latest tree", 400),
    NODE_NOT_EDITABLE("type into non-editable node", "tap the editable field first, then type", 400),
    NODE_NOT_FOUND("?subtree=<id> unknown id", "ids only come from the latest device_tree", 400),
    INJECT_FAILED("both injection methods failed", "re-fetch tree, retry once; bundle evidence", 400),
    CAPTURE_FAILED("/screenshot capture failed", "foreground the app; device_tree still works", 400),
    APP_NOT_FOREGROUND("mutation while backgrounded", "bring app forward explicitly (no auto-launch)", 400),
    SCREEN_OFF("display off", "wake + reopen app (human step)", 400),
    DEVICE_LOCKED("keyguard locked", "unlock device first", 400),
    HUMAN_STEERING("human touch mid-action", "re-plan signal: device_tree then continue", 409),
    BRIDGE_STOPPED("user stopped bridge mid-action", "restart service via overlay, reconnect", 503),
    PORT_IN_USE("bind conflict on fixed port", "free the occupier; service stays stopped", 500);

    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("error", JSONObject().apply {
                put("code", name)
                put("message", message)
                put("hint", hint)
            })
        }
    }
}

enum class Key {
    ENTER, BACK, DEL, TAB
}

enum class BridgeState {
    IDLE, ACTIVE, HUMAN_STEERING
}

data class TreeNode(
    val id: String?,
    val text: String?,
    val cd: String?,
    val cls: String,
    val bounds: List<Int>, // x, y, w, h
    val clickable: Boolean,
    val editable: Boolean
) {
    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("id", id ?: JSONObject.NULL)
            put("text", text ?: JSONObject.NULL)
            put("cd", cd ?: JSONObject.NULL)
            put("cls", cls)
            val boundsArray = JSONArray()
            bounds.forEach { boundsArray.put(it) }
            put("bounds", boundsArray)
            put("clickable", clickable)
            put("editable", editable)
        }
    }
}
