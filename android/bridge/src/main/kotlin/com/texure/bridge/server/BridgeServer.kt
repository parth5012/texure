package com.texure.bridge.server

import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import com.texure.bridge.crash.BridgeCrashHandler
import com.texure.bridge.model.BridgeError
import com.texure.bridge.model.BridgeState
import com.texure.bridge.model.Key
import com.texure.bridge.storage.BridgeTokenManager
import com.texure.bridge.tree.BridgeTreeExtractor
import com.texure.bridge.worker.BridgeWorker
import com.texure.bridge.worker.WorkerResult
import fi.iki.elonen.NanoHTTPD
import fi.iki.elonen.NanoWSD
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class BridgeServer(
    private val context: Context,
    port: Int = 8765,
    private val tokenManager: BridgeTokenManager = BridgeTokenManager(context),
    private val activityProvider: () -> Activity?,
    private val crashHandler: BridgeCrashHandler? = null,
    private val treeExtractor: BridgeTreeExtractor = BridgeTreeExtractor(),
    private val worker: BridgeWorker = BridgeWorker(activityProvider, treeExtractor)
) : NanoWSD(port) {

    private val mainHandler = Handler(Looper.getMainLooper())
    private val activeWebSockets = CopyOnWriteArraySet<BridgeWebSocket>()
    private val currentState = AtomicReference(BridgeState.IDLE)

    // Single-flight lock
    @Volatile private var activeOwner: String? = null
    @Volatile private var lastOwnerActivityTime: Long = 0L
    private val lockTimeoutMs = 60_000L

    val state: BridgeState get() = currentState.get()

    fun setHumanSteering() {
        currentState.set(BridgeState.HUMAN_STEERING)
        treeExtractor.bumpRev()
        broadcastTreeChanged(treeExtractor.getRev())
    }

    override fun openWebSocket(session: IHTTPSession): WebSocket {
        return BridgeWebSocket(session)
    }

    override fun serveHttp(session: IHTTPSession): Response {
        val uri = session.uri
        val method = session.method

        // 1. Validate auth token
        val authHeader = session.headers["authorization"]
        if (!tokenManager.validateToken(authHeader)) {
            return jsonResponse(Response.Status.UNAUTHORIZED, BridgeError.AUTH_FAILED.toJson())
        }

        // 2. Concurrency: single-flight lock
        val clientHandle = session.headers["remote-addr"] ?: session.remoteIpAddress ?: "client"
        val lockResponse = checkSingleFlightLock(clientHandle)
        if (lockResponse != null) {
            return lockResponse
        }

        crashHandler?.recordAction("${method.name} $uri")

        return try {
            when {
                method == Method.GET && uri == "/ui-tree" -> handleGetUiTree(session)
                method == Method.POST && uri == "/tap" -> handlePostTap(session)
                method == Method.POST && uri == "/type" -> handlePostType(session)
                method == Method.POST && uri == "/keypress" -> handlePostKeypress(session)
                method == Method.POST && uri == "/back" -> handlePostBack()
                method == Method.GET && uri == "/screenshot" -> handleGetScreenshot(session)
                method == Method.GET && uri == "/last-crash" -> handleGetLastCrash()
                else -> jsonResponse(Response.Status.NOT_FOUND, JSONObject().apply {
                    put("error", JSONObject().apply {
                        put("code", "NOT_FOUND")
                        put("message", "Route not found: ${method.name} $uri")
                    })
                })
            }
        } catch (e: Exception) {
            jsonResponse(Response.Status.INTERNAL_ERROR, JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "INTERNAL_ERROR")
                    put("message", e.message ?: "Unknown server error")
                })
            })
        }
    }

    private fun handleGetUiTree(session: IHTTPSession): Response {
        val params = session.parms
        val subtreeId = params["subtree"]
        val maxDepth = params["max_depth"]?.toIntOrNull() ?: BridgeTreeExtractor.DEFAULT_MAX_DEPTH

        val extracted = worker.extractTreeOnMain(subtreeId, maxDepth)
        if (extracted.subtreeNotFound) {
            return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.NODE_NOT_FOUND.toJson())
        }

        val json = JSONObject().apply {
            put("rev", extracted.rev)
            put("window", extracted.window)
            val nodesArray = JSONArray()
            extracted.nodes.forEach { nodesArray.put(it.toJson()) }
            put("nodes", nodesArray)
        }

        if (currentState.get() == BridgeState.HUMAN_STEERING) {
            currentState.set(BridgeState.IDLE)
        }

        return jsonResponse(Response.Status.OK, json)
    }

    private fun handlePostTap(session: IHTTPSession): Response {
        val refusal = checkDeviceReadiness(mutating = true)
        if (refusal != null) return refusal

        val body = parseBodyAsJson(session) ?: return badRequest("Invalid JSON body")
        val rev = body.optLong("rev", -1L)
        if (rev < 0) return badRequest("Missing rev parameter")

        val targetIndex = if (body.has("index")) {
            body.getInt("index")
        } else if (body.has("node")) {
            body.getInt("node")
        } else {
            return badRequest("Missing node index parameter")
        }

        currentState.set(BridgeState.ACTIVE)
        return when (val result = worker.executeTap(targetIndex, rev)) {
            is WorkerResult.Success -> {
                currentState.set(BridgeState.IDLE)
                broadcastTreeChanged(treeExtractor.getRev())
                jsonResponse(Response.Status.OK, JSONObject().put("ok", true))
            }
            is WorkerResult.Error -> {
                currentState.set(BridgeState.IDLE)
                jsonResponse(Response.Status.lookup(result.bridgeError.httpStatus) ?: Response.Status.BAD_REQUEST, result.bridgeError.toJson())
            }
        }
    }

    private fun handlePostType(session: IHTTPSession): Response {
        val refusal = checkDeviceReadiness(mutating = true)
        if (refusal != null) return refusal

        val body = parseBodyAsJson(session) ?: return badRequest("Invalid JSON body")
        val rev = body.optLong("rev", -1L)
        if (rev < 0) return badRequest("Missing rev parameter")

        val text = body.optString("text")
        val targetIndex = body.optInt("node", -1)
        if (targetIndex < 0) return badRequest("Missing or invalid node parameter")

        currentState.set(BridgeState.ACTIVE)
        return when (val result = worker.executeType(targetIndex, text, rev)) {
            is WorkerResult.Success -> {
                currentState.set(BridgeState.IDLE)
                broadcastTreeChanged(treeExtractor.getRev())
                jsonResponse(Response.Status.OK, JSONObject().put("ok", true))
            }
            is WorkerResult.Error -> {
                currentState.set(BridgeState.IDLE)
                val errJson = result.bridgeError.toJson()
                if (result.detail != null) {
                    errJson.getJSONObject("error").put("detail", result.detail)
                }
                jsonResponse(Response.Status.lookup(result.bridgeError.httpStatus) ?: Response.Status.BAD_REQUEST, errJson)
            }
        }
    }

    private fun handlePostKeypress(session: IHTTPSession): Response {
        val refusal = checkDeviceReadiness(mutating = true)
        if (refusal != null) return refusal

        val body = parseBodyAsJson(session) ?: return badRequest("Invalid JSON body")
        val keyName = body.optString("key")
        val key = try {
            Key.valueOf(keyName.uppercase())
        } catch (_: Exception) {
            return badRequest("Invalid key enum: $keyName")
        }

        currentState.set(BridgeState.ACTIVE)
        return when (val result = worker.executeKey(key)) {
            is WorkerResult.Success -> {
                currentState.set(BridgeState.IDLE)
                broadcastTreeChanged(treeExtractor.getRev())
                jsonResponse(Response.Status.OK, JSONObject().put("ok", true))
            }
            is WorkerResult.Error -> {
                currentState.set(BridgeState.IDLE)
                jsonResponse(Response.Status.lookup(result.bridgeError.httpStatus) ?: Response.Status.BAD_REQUEST, result.bridgeError.toJson())
            }
        }
    }

    private fun handlePostBack(): Response {
        val refusal = checkDeviceReadiness(mutating = true)
        if (refusal != null) return refusal

        currentState.set(BridgeState.ACTIVE)
        return when (val result = worker.executeBack()) {
            is WorkerResult.Success -> {
                currentState.set(BridgeState.IDLE)
                broadcastTreeChanged(treeExtractor.getRev())
                jsonResponse(Response.Status.OK, JSONObject().put("ok", true))
            }
            is WorkerResult.Error -> {
                currentState.set(BridgeState.IDLE)
                jsonResponse(Response.Status.lookup(result.bridgeError.httpStatus) ?: Response.Status.BAD_REQUEST, result.bridgeError.toJson())
            }
        }
    }

    private fun handleGetScreenshot(session: IHTTPSession): Response {
        val refusal = checkDeviceReadiness(mutating = false)
        if (refusal != null) return refusal

        val scaleParam = session.parms["scale"]?.toFloatOrNull() ?: 0.5f
        val scale = when {
            scaleParam <= 0.35f -> 0.25f
            scaleParam <= 0.75f -> 0.5f
            else -> 1.0f
        }

        val activity = activityProvider() ?: return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.CAPTURE_FAILED.toJson())
        val latch = CountDownLatch(1)
        var bitmapBytes: ByteArray? = null

        mainHandler.post {
            try {
                val decor = activity.window.decorView
                val width = decor.width.coerceAtLeast(1)
                val height = decor.height.coerceAtLeast(1)

                val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
                val canvas = Canvas(bitmap)
                decor.draw(canvas)

                val scaled = if (scale != 1.0f) {
                    val sw = (width * scale).toInt().coerceAtLeast(1)
                    val sh = (height * scale).toInt().coerceAtLeast(1)
                    Bitmap.createScaledBitmap(bitmap, sw, sh, true)
                } else {
                    bitmap
                }

                val baos = ByteArrayOutputStream()
                scaled.compress(Bitmap.CompressFormat.PNG, 100, baos)
                bitmapBytes = baos.toByteArray()
                if (scaled != bitmap) {
                    scaled.recycle()
                }
                bitmap.recycle()
            } catch (_: Exception) {
                bitmapBytes = null
            } finally {
                latch.countDown()
            }
        }

        latch.await(1500, TimeUnit.MILLISECONDS)
        val bytes = bitmapBytes ?: return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.CAPTURE_FAILED.toJson())

        return newFixedLengthResponse(
            Response.Status.OK,
            "image/png",
            ByteArrayInputStream(bytes),
            bytes.size.toLong()
        )
    }

    private fun handleGetLastCrash(): Response {
        val lastCrash = crashHandler?.getLastCrash()
        return if (lastCrash != null) {
            jsonResponse(Response.Status.OK, lastCrash)
        } else {
            jsonResponse(Response.Status.NOT_FOUND, JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "NOT_FOUND")
                    put("message", "no crash bundle available")
                })
            })
        }
    }

    private fun checkDeviceReadiness(mutating: Boolean): Response? {
        val activity = activityProvider()
        if (activity == null) {
            return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.APP_NOT_FOREGROUND.toJson())
        }

        try {
            val pm = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
            if (pm != null && !pm.isInteractive) {
                return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.SCREEN_OFF.toJson())
            }

            val km = context.getSystemService(Context.KEYGUARD_SERVICE) as? KeyguardManager
            if (km != null && km.isKeyguardLocked) {
                return jsonResponse(Response.Status.BAD_REQUEST, BridgeError.DEVICE_LOCKED.toJson())
            }
        } catch (_: Exception) {}

        if (mutating && currentState.get() == BridgeState.HUMAN_STEERING) {
            return jsonResponse(Response.Status.CONFLICT, BridgeError.HUMAN_STEERING.toJson())
        }

        return null
    }

    @Synchronized
    private fun checkSingleFlightLock(clientHandle: String): Response? {
        val now = System.currentTimeMillis()
        val currentOwner = activeOwner
        if (currentOwner == null || (now - lastOwnerActivityTime) > lockTimeoutMs) {
            activeOwner = clientHandle
            lastOwnerActivityTime = now
            return null
        }

        if (currentOwner != clientHandle) {
            return jsonResponse(Response.Status.CONFLICT, JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", BridgeError.BUSY.name)
                    put("message", "Single-flight lock held by $currentOwner")
                    put("hint", BridgeError.BUSY.hint)
                })
            })
        }

        lastOwnerActivityTime = now
        return null
    }

    private fun parseBodyAsJson(session: IHTTPSession): JSONObject? {
        val map = HashMap<String, String>()
        session.parseBody(map)
        val postData = map["postData"] ?: return null
        return try {
            JSONObject(postData)
        } catch (_: Exception) {
            null
        }
    }

    private fun badRequest(message: String): Response {
        return jsonResponse(Response.Status.BAD_REQUEST, JSONObject().apply {
            put("error", JSONObject().apply {
                put("code", "BAD_REQUEST")
                put("message", message)
            })
        })
    }

    private fun jsonResponse(status: Response.IStatus, json: JSONObject): Response {
        val bytes = json.toString().toByteArray(Charsets.UTF_8)
        return newFixedLengthResponse(
            status,
            "application/json",
            ByteArrayInputStream(bytes),
            bytes.size.toLong()
        )
    }

    private fun broadcastTreeChanged(rev: Long) {
        val message = JSONObject().apply {
            put("type", "tree_changed")
            put("rev", rev)
        }.toString()

        for (ws in activeWebSockets) {
            try {
                if (ws.isOpen) {
                    ws.send(message)
                }
            } catch (_: Exception) {}
        }
    }

    override fun stop() {
        worker.shutdown()
        for (ws in activeWebSockets) {
            try {
                ws.close(NanoWSD.WebSocketFrame.CloseCode.NormalClosure, "server stopped", false)
            } catch (_: Exception) {}
        }
        activeWebSockets.clear()
        super.stop()
    }

    inner class BridgeWebSocket(session: IHTTPSession) : WebSocket(session) {
        override fun onOpen() {
            activeWebSockets.add(this)
        }

        override fun onClose(code: NanoWSD.WebSocketFrame.CloseCode?, reason: String?, initiatedByRemote: Boolean) {
            activeWebSockets.remove(this)
        }

        override fun onMessage(message: NanoWSD.WebSocketFrame?) {}

        override fun onPong(pong: NanoWSD.WebSocketFrame?) {}

        override fun onException(exception: IOException?) {
            activeWebSockets.remove(this)
        }
    }
}
