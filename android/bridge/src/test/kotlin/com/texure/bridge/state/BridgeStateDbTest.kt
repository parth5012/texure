package com.texure.bridge.state

import android.app.Activity
import com.texure.bridge.model.BridgeError
import com.texure.bridge.server.BridgeServer
import com.texure.bridge.storage.BridgeTokenManager
import fi.iki.elonen.NanoHTTPD
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.net.HttpURLConnection
import java.net.URL

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BridgeStateDbTest {

    private lateinit var activity: Activity
    private lateinit var server: BridgeServer
    private val testPort = 8765
    private val testToken = "test-token-state-db"
    private val dbName = "test_store_db"

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()

        val tokenManager = BridgeTokenManager(activity)
        tokenManager.saveToken(testToken)

        server = BridgeServer(
            context = activity,
            port = testPort,
            tokenManager = tokenManager,
            activityProvider = { activity }
        )
        server.start(NanoHTTPD.SOCKET_READ_TIMEOUT, false)
    }

    @After
    fun tearDown() {
        if (::server.isInitialized) {
            server.stop()
        }
        activity.deleteDatabase(dbName)
    }

    private fun sendRequest(
        method: String,
        path: String,
        body: String? = null,
        token: String? = testToken
    ): Pair<Int, String> {
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
        val future = executor.submit<Pair<Int, String>> {
            val url = URL("http://127.0.0.1:$testPort$path")
            val conn = url.openConnection() as HttpURLConnection
            conn.requestMethod = method
            conn.setRequestProperty("Connection", "close")
            if (token != null) {
                conn.setRequestProperty("Authorization", "Bearer $token")
            }
            if (body != null) {
                conn.doOutput = true
                conn.setRequestProperty("Content-Type", "application/json")
                conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
            }

            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val responseBody = stream?.bufferedReader()?.use { it.readText() } ?: ""
            Pair(code, responseBody)
        }

        while (!future.isDone) {
            org.robolectric.shadows.ShadowLooper.idleMainLooper()
            Thread.sleep(10)
        }
        val result = future.get()
        executor.shutdown()
        return result
    }

    @Test
    fun dbRoutesRequireAuth() {
        val (code1, body1) = sendRequest("GET", "/state/db/tables?db=$dbName", token = null)
        assertEquals(401, code1)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(body1).getJSONObject("error").getString("code"))

        val (code2, body2) = sendRequest("POST", "/state/db/query", body = "{}", token = null)
        assertEquals(401, code2)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(body2).getJSONObject("error").getString("code"))

        val (code3, body3) = sendRequest("POST", "/state/db/exec", body = "{}", token = null)
        assertEquals(401, code3)
        assertEquals(BridgeError.AUTH_FAILED.name, JSONObject(body3).getJSONObject("error").getString("code"))
    }

    @Test
    fun ddlExecTableReflectionAndQuery() {
        // 1. Create table via POST /state/db/exec
        val createSql = """
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                email TEXT,
                rating REAL
            )
        """.trimIndent()
        val createPayload = JSONObject().apply {
            put("db", dbName)
            put("sql", createSql)
        }
        val (codeCreate, respCreate) = sendRequest("POST", "/state/db/exec", body = createPayload.toString())
        assertEquals(200, codeCreate)
        assertTrue(JSONObject(respCreate).getBoolean("ok"))

        // 2. Insert rows with bound parameters
        val insertSql = "INSERT INTO users (username, email, rating) VALUES (?, ?, ?)"
        val insertPayload1 = JSONObject().apply {
            put("db", dbName)
            put("sql", insertSql)
            put("params", JSONArray().apply {
                put("Alice")
                put("alice@example.com")
                put(4.8)
            })
        }
        val (codeIns1, respIns1) = sendRequest("POST", "/state/db/exec", body = insertPayload1.toString())
        assertEquals(200, codeIns1)
        assertEquals(1, JSONObject(respIns1).getInt("affected_rows"))

        val insertPayload2 = JSONObject().apply {
            put("db", dbName)
            put("sql", insertSql)
            put("params", JSONArray().apply {
                put("Bob")
                put("bob@example.com")
                put(3.9)
            })
        }
        val (codeIns2, respIns2) = sendRequest("POST", "/state/db/exec", body = insertPayload2.toString())
        assertEquals(200, codeIns2)
        assertEquals(1, JSONObject(respIns2).getInt("affected_rows"))

        // 3. Inspect table schema via GET /state/db/tables
        val (codeTables, respTables) = sendRequest("GET", "/state/db/tables?db=$dbName")
        assertEquals(200, codeTables)
        val tablesJson = JSONObject(respTables)
        assertEquals(dbName, tablesJson.getString("db"))
        val tablesArr = tablesJson.getJSONArray("tables")
        assertEquals(1, tablesArr.length())

        val tableObj = tablesArr.getJSONObject(0)
        assertEquals("users", tableObj.getString("name"))
        val colsArr = tableObj.getJSONArray("columns")
        assertEquals(4, colsArr.length())
        assertEquals("id", colsArr.getJSONObject(0).getString("name"))
        assertTrue(colsArr.getJSONObject(0).getBoolean("primary_key"))
        assertEquals("username", colsArr.getJSONObject(1).getString("name"))
        assertTrue(colsArr.getJSONObject(1).getBoolean("not_null"))

        // 4. Query rows via POST /state/db/query
        val queryPayload = JSONObject().apply {
            put("db", dbName)
            put("sql", "SELECT id, username, email, rating FROM users WHERE username = ?")
            put("params", JSONArray().apply { put("Alice") })
        }
        val (codeQuery, respQuery) = sendRequest("POST", "/state/db/query", body = queryPayload.toString())
        assertEquals(200, codeQuery)
        val queryJson = JSONObject(respQuery)
        assertEquals(1, queryJson.getInt("count"))
        val row = queryJson.getJSONArray("rows").getJSONObject(0)
        assertEquals(1, row.getInt("id"))
        assertEquals("Alice", row.getString("username"))
        assertEquals("alice@example.com", row.getString("email"))
        assertEquals(4.8, row.getDouble("rating"), 0.01)

        // 5. Update rows via POST /state/db/exec
        val updatePayload = JSONObject().apply {
            put("db", dbName)
            put("sql", "UPDATE users SET rating = ? WHERE username = ?")
            put("params", JSONArray().apply {
                put(5.0)
                put("Alice")
            })
        }
        val (codeUpd, respUpd) = sendRequest("POST", "/state/db/exec", body = updatePayload.toString())
        assertEquals(200, codeUpd)
        assertEquals(1, JSONObject(respUpd).getInt("affected_rows"))

        // Verify updated rating
        val (codeVerify, respVerify) = sendRequest("POST", "/state/db/query", body = queryPayload.toString())
        assertEquals(200, codeVerify)
        val updatedRow = JSONObject(respVerify).getJSONArray("rows").getJSONObject(0)
        assertEquals(5.0, updatedRow.getDouble("rating"), 0.01)
    }

    @Test
    fun sqlSyntaxErrorReturnsBadRequestCleanly() {
        // Ensure DB exists first
        val createPayload = JSONObject().apply {
            put("db", dbName)
            put("sql", "CREATE TABLE dummy (id INT)")
        }
        val (initCode, _) = sendRequest("POST", "/state/db/exec", body = createPayload.toString())
        assertEquals(200, initCode)

        val badPayload = JSONObject().apply {
            put("db", dbName)
            put("sql", "INVALID SYNTAX BLABLA")
        }
        val (code, resp) = sendRequest("POST", "/state/db/query", body = badPayload.toString())
        assertEquals(400, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("SQL_ERROR", error.getString("code"))
        assertTrue(error.getString("message").isNotEmpty())
    }

    @Test
    fun nonExistentDatabaseReturnsNotFound() {
        val (code, resp) = sendRequest("GET", "/state/db/tables?db=non_existent_database_123")
        assertEquals(404, code)
        val error = JSONObject(resp).getJSONObject("error")
        assertEquals("DB_NOT_FOUND", error.getString("code"))
    }
}
