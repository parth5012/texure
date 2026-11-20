package com.texure.bridge.state

import android.content.Context
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteException
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * SQLite & Room database inspector and SQL query executor (Ticket #71, Map #74).
 * Provides table reflection, read-only queries, and DDL/DML execution in debug builds.
 */
class BridgeStateDb(private val context: Context) {

    /**
     * Lists all database names in the app databases directory, excluding temporary journal/WAL files.
     */
    fun listDatabases(): List<String> {
        val databases = context.databaseList() ?: emptyArray()
        return databases.filter { name ->
            !name.endsWith("-journal") && !name.endsWith("-wal") && !name.endsWith("-shm")
        }.sorted()
    }

    fun resolveDatabasePath(name: String): File {
        val directPath = context.getDatabasePath(name)
        if (directPath.exists()) {
            return directPath
        }
        val withExt = context.getDatabasePath("$name.db")
        if (withExt.exists()) {
            return withExt
        }
        return directPath
    }

    /**
     * Discovers table names and column schemas for the given database.
     */
    fun getTables(dbName: String?): JSONObject {
        val name = dbName?.takeIf { it.isNotBlank() }
            ?: listDatabases().firstOrNull()
            ?: return JSONObject().apply {
                put("databases", JSONArray(listDatabases()))
                put("tables", JSONArray())
            }

        val dbPath = resolveDatabasePath(name)
        if (!dbPath.exists()) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "DB_NOT_FOUND")
                    put("message", "Database '$name' not found at ${dbPath.absolutePath}")
                })
            }
        }

        var db: SQLiteDatabase? = null
        try {
            db = SQLiteDatabase.openDatabase(dbPath.absolutePath, null, SQLiteDatabase.OPEN_READONLY)
            val tablesArray = JSONArray()

            val tableCursor = db.rawQuery(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name != 'android_metadata' ORDER BY name",
                null
            )
            val tableNames = mutableListOf<String>()
            tableCursor.use {
                while (it.moveToNext()) {
                    tableNames.add(it.getString(0))
                }
            }

            for (tableName in tableNames) {
                val tableObj = JSONObject().apply {
                    put("name", tableName)
                }
                val columnsArray = JSONArray()

                val infoCursor = db.rawQuery("PRAGMA table_info(\"$tableName\")", null)
                infoCursor.use {
                    while (it.moveToNext()) {
                        val colName = it.getString(it.getColumnIndexOrThrow("name"))
                        val colType = it.getString(it.getColumnIndexOrThrow("type"))
                        val notNull = it.getInt(it.getColumnIndexOrThrow("notnull")) == 1
                        val pk = it.getInt(it.getColumnIndexOrThrow("pk")) == 1

                        columnsArray.put(JSONObject().apply {
                            put("name", colName)
                            put("type", colType)
                            put("not_null", notNull)
                            put("primary_key", pk)
                        })
                    }
                }
                tableObj.put("columns", columnsArray)
                tablesArray.put(tableObj)
            }

            return JSONObject().apply {
                put("db", name)
                put("tables", tablesArray)
            }
        } catch (e: SQLiteException) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "SQL_ERROR")
                    put("message", e.message ?: "Failed to inspect database tables")
                })
            }
        } finally {
            db?.close()
        }
    }

    /**
     * Executes a read-only SQL query and returns JSON rows.
     */
    fun query(payload: JSONObject): JSONObject {
        val dbName = payload.optString("db").takeIf { it.isNotBlank() }
            ?: return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Missing 'db' parameter in query payload")
                })
            }
        val sql = payload.optString("sql").takeIf { it.isNotBlank() }
            ?: return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Missing 'sql' parameter in query payload")
                })
            }

        val dbPath = resolveDatabasePath(dbName)
        if (!dbPath.exists()) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "DB_NOT_FOUND")
                    put("message", "Database '$dbName' not found")
                })
            }
        }

        val paramsArray = payload.optJSONArray("params")
        val selectionArgs = paramsArray?.let { arr ->
            Array(arr.length()) { i ->
                if (arr.isNull(i)) null else arr.get(i).toString()
            }
        }

        var db: SQLiteDatabase? = null
        try {
            db = SQLiteDatabase.openDatabase(dbPath.absolutePath, null, SQLiteDatabase.OPEN_READONLY)
            val cursor = db.rawQuery(sql, selectionArgs)
            val rows = JSONArray()

            cursor.use {
                val columnNames = cursor.columnNames
                while (cursor.moveToNext()) {
                    val row = JSONObject()
                    for (i in columnNames.indices) {
                        val colName = columnNames[i]
                        when (cursor.getType(i)) {
                            Cursor.FIELD_TYPE_NULL -> row.put(colName, JSONObject.NULL)
                            Cursor.FIELD_TYPE_INTEGER -> row.put(colName, cursor.getLong(i))
                            Cursor.FIELD_TYPE_FLOAT -> row.put(colName, cursor.getDouble(i))
                            Cursor.FIELD_TYPE_STRING -> row.put(colName, cursor.getString(i))
                            Cursor.FIELD_TYPE_BLOB -> {
                                val bytes = cursor.getBlob(i)
                                row.put(colName, android.util.Base64.encodeToString(bytes, android.util.Base64.NO_WRAP))
                            }
                        }
                    }
                    rows.put(row)
                }
            }

            return JSONObject().apply {
                put("rows", rows)
                put("count", rows.length())
            }
        } catch (e: SQLiteException) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "SQL_ERROR")
                    put("message", e.message ?: "SQL query execution failed")
                })
            }
        } finally {
            db?.close()
        }
    }

    /**
     * Executes DDL/DML statements and returns the affected row count.
     */
    fun exec(payload: JSONObject, onMutated: (() -> Unit)? = null): JSONObject {
        val dbName = payload.optString("db").takeIf { it.isNotBlank() }
            ?: return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Missing 'db' parameter in exec payload")
                })
            }
        val sql = payload.optString("sql").takeIf { it.isNotBlank() }
            ?: return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Missing 'sql' parameter in exec payload")
                })
            }

        val dbPath = resolveDatabasePath(dbName)
        dbPath.parentFile?.mkdirs()

        val paramsArray = payload.optJSONArray("params")
        val bindArgs = paramsArray?.let { arr ->
            List(arr.length()) { i -> if (arr.isNull(i)) null else arr.get(i) }
        }

        var db: SQLiteDatabase? = null
        try {
            db = SQLiteDatabase.openOrCreateDatabase(dbPath, null)
            val affectedRows: Int

            if (bindArgs != null && bindArgs.isNotEmpty()) {
                val stmt = db.compileStatement(sql)
                stmt.use {
                    for (i in bindArgs.indices) {
                        val index = i + 1
                        when (val arg = bindArgs[i]) {
                            is Number -> {
                                if (arg is Double || arg is Float) {
                                    stmt.bindDouble(index, arg.toDouble())
                                } else {
                                    stmt.bindLong(index, arg.toLong())
                                }
                            }
                            is Boolean -> stmt.bindLong(index, if (arg) 1L else 0L)
                            is String -> stmt.bindString(index, arg)
                            is ByteArray -> stmt.bindBlob(index, arg)
                            null, JSONObject.NULL -> stmt.bindNull(index)
                            else -> stmt.bindString(index, arg.toString())
                        }
                    }
                    val upper = sql.trim().uppercase()
                    affectedRows = if (upper.startsWith("UPDATE") || upper.startsWith("DELETE")) {
                        stmt.executeUpdateDelete()
                    } else if (upper.startsWith("INSERT")) {
                        val rowId = stmt.executeInsert()
                        if (rowId >= 0) 1 else 0
                    } else {
                        stmt.execute()
                        0
                    }
                }
            } else {
                db.execSQL(sql)
                val cursor = db.rawQuery("SELECT changes()", null)
                affectedRows = cursor.use {
                    if (it.moveToFirst()) it.getInt(0) else 0
                }
            }

            onMutated?.invoke()
            return JSONObject().apply {
                put("ok", true)
                put("affected_rows", affectedRows)
            }
        } catch (e: SQLiteException) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "SQL_ERROR")
                    put("message", e.message ?: "SQL execution error")
                })
            }
        } finally {
            db?.close()
        }
    }
}
