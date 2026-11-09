package com.texure.bridge.recording

import org.json.JSONArray
import org.json.JSONObject

/**
 * A single recorded workflow action (behavior-spec 1.7, contract "/recording/export").
 *
 * @param seq order within the trace
 * @param t milliseconds since session start (timestamp offsets)
 * @param action one of: tap | type | keypress | pause
 * @param node targeted node reference (tap/type); null when the point hit no node (tap) or n/a
 * @param value committed text (type; ENTER commit value excludes the trailing newline)
 * @param key recorded key (keypress: BACK | ENTER)
 * @param ms inactivity gap duration before the NEXT action (pause)
 */
data class RecordedAction(
    val seq: Int,
    val t: Long,
    val action: String,
    val node: RecordedNodeRef?,
    val value: String? = null,
    val key: String? = null,
    val ms: Long? = null
) {
    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("seq", seq)
            put("t", t)
            put("action", action)
            put("node", node?.toJson() ?: JSONObject.NULL)
            value?.let { put("value", it) }
            key?.let { put("key", it) }
            ms?.let { put("ms", it) }
        }
    }

    companion object {
        /** Node reference targeting the exported trace as an object with node details, or null. */
        fun actionsJson(actions: List<RecordedAction>): JSONArray {
            val array = JSONArray()
            actions.forEach { array.put(it.toJson()) }
            return array
        }
    }
}

/**
 * Targeted node reference correlated via [com.texure.bridge.tree.BridgeTreeExtractor].
 * `index` positions the node in the /ui-tree node list observed at record time.
 */
data class RecordedNodeRef(
    val index: Int,
    val id: String?,
    val text: String?,
    val cd: String?,
    val cls: String,
    val rev: Long
) {
    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("index", index)
            put("id", id ?: JSONObject.NULL)
            put("text", text ?: JSONObject.NULL)
            put("cd", cd ?: JSONObject.NULL)
            put("cls", cls)
            put("rev", rev)
        }
    }
}
