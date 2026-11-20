package com.texure.bridge.state

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream

/**
 * Day-0 clean slate reset and named state snapshot engine (Ticket #72, Map #74).
 * Handles full reset (prefs, db, cache) and zip-compressed state snapshots in app-private storage.
 */
class BridgeStateSnapshot(private val context: Context) {

    private val statePrefs = BridgeStatePrefs(context)

    private val snapshotsDir: File
        get() = File(context.filesDir, "texure_snapshots").apply { mkdirs() }

    private val sharedPrefsDir: File
        get() {
            val fromDataDir = File(context.applicationInfo.dataDir, "shared_prefs")
            if (fromDataDir.exists()) return fromDataDir
            val fromFiles = File(context.filesDir.parentFile, "shared_prefs")
            if (fromFiles.exists()) return fromFiles
            return fromDataDir
        }

    private val databasesDir: File
        get() = context.getDatabasePath("probe").parentFile
            ?: File(context.applicationInfo.dataDir, "databases")

    companion object {
        private const val PAIRING_PREFS_NAME = "texure_bridge_pairing"
        private const val PAIRING_PREFS_FILE = "texure_bridge_pairing.xml"
    }

    /**
     * Completely clears preferences, deletes databases, and wipes internal cache directories.
     */
    @Synchronized
    fun reset(scope: String, onMutated: (() -> Unit)? = null): JSONObject {
        val normalizedScope = scope.lowercase()
        if (normalizedScope !in listOf("all", "prefs", "db", "cache")) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Invalid reset scope '$scope'. Supported: all, prefs, db, cache")
                })
            }
        }

        if (normalizedScope == "all" || normalizedScope == "prefs") {
            clearSharedPreferences()
        }
        if (normalizedScope == "all" || normalizedScope == "db") {
            clearDatabases()
        }
        if (normalizedScope == "all" || normalizedScope == "cache") {
            clearCache()
        }

        onMutated?.invoke()
        return JSONObject().apply {
            put("ok", true)
            put("reset", normalizedScope)
        }
    }

    private fun clearSharedPreferences() {
        val spDir = sharedPrefsDir
        if (spDir.exists() && spDir.isDirectory) {
            val files = spDir.listFiles() ?: emptyArray()
            for (file in files) {
                if (file.name == PAIRING_PREFS_FILE || file.name.startsWith(PAIRING_PREFS_NAME)) {
                    continue
                }
                val prefName = file.name.removeSuffix(".bak").removeSuffix(".xml")
                try {
                    context.getSharedPreferences(prefName, Context.MODE_PRIVATE).edit().clear().commit()
                } catch (_: Exception) {}
                file.delete()
            }
        }
        // Also clear default prefs explicitly
        try {
            context.getSharedPreferences(statePrefs.getDefaultPrefsName(), Context.MODE_PRIVATE)
                .edit().clear().commit()
        } catch (_: Exception) {}
    }

    private fun clearDatabases() {
        val dbNames = context.databaseList() ?: emptyArray()
        for (name in dbNames) {
            try {
                context.deleteDatabase(name)
            } catch (_: Exception) {}
        }
        val dbDir = databasesDir
        if (dbDir.exists() && dbDir.isDirectory) {
            val files = dbDir.listFiles() ?: emptyArray()
            for (f in files) {
                f.delete()
            }
        }
    }

    private fun clearCache() {
        try {
            context.cacheDir?.deleteRecursively()
            context.cacheDir?.mkdirs()
        } catch (_: Exception) {}

        try {
            context.codeCacheDir?.deleteRecursively()
            context.codeCacheDir?.mkdirs()
        } catch (_: Exception) {}

        try {
            context.externalCacheDir?.deleteRecursively()
            context.externalCacheDir?.mkdirs()
        } catch (_: Exception) {}
    }

    /**
     * Lists all saved snapshot names.
     */
    @Synchronized
    fun listSnapshots(): JSONObject {
        val files = snapshotsDir.listFiles() ?: emptyArray()
        val names = files
            .filter { it.isFile && it.name.endsWith(".zip") && !it.name.endsWith(".tmp") }
            .map { it.name.removeSuffix(".zip") }
            .sorted()

        return JSONObject().apply {
            put("snapshots", JSONArray(names))
        }
    }

    /**
     * Creates a compressed archive of current shared_prefs and databases in app-private backup directory.
     */
    @Synchronized
    fun saveSnapshot(name: String): JSONObject {
        if (!isValidSnapshotName(name)) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Invalid snapshot name '$name'. Use alphanumeric, underscore, or hyphen.")
                })
            }
        }

        val tmpZipFile = File(snapshotsDir, "$name.zip.tmp")
        val finalZipFile = File(snapshotsDir, "$name.zip")
        try {
            ZipOutputStream(BufferedOutputStream(FileOutputStream(tmpZipFile))).use { zos ->
                // Write snapshot metadata
                val metaJson = JSONObject().apply {
                    put("version", 1)
                    put("name", name)
                    put("timestamp", System.currentTimeMillis())
                }
                val metaBytes = metaJson.toString().toByteArray(Charsets.UTF_8)
                val metaEntry = ZipEntry("snapshot_meta.json")
                metaEntry.time = System.currentTimeMillis()
                zos.putNextEntry(metaEntry)
                zos.write(metaBytes)
                zos.closeEntry()

                // Discover all active preference files
                val prefNames = mutableSetOf<String>()
                val spDir = sharedPrefsDir
                if (spDir.exists() && spDir.isDirectory) {
                    val files = spDir.listFiles() ?: emptyArray()
                    for (file in files) {
                        if (file.name == PAIRING_PREFS_FILE || file.name.startsWith(PAIRING_PREFS_NAME)) {
                            continue
                        }
                        if (file.isFile && file.name.endsWith(".xml")) {
                            prefNames.add(file.name.removeSuffix(".xml"))
                            addFileToZip(zos, file, "shared_prefs_files/${file.name}")
                        }
                    }
                }

                // Check default preferences as well
                val defaultName = statePrefs.getDefaultPrefsName()
                if (context.getSharedPreferences(defaultName, Context.MODE_PRIVATE).all.isNotEmpty()) {
                    prefNames.add(defaultName)
                }

                // Archive structured json representation of each preference file to ensure in-memory cache update on restore
                for (prefName in prefNames) {
                    val data = statePrefs.inspect(prefName)
                    val entryBytes = data.toString().toByteArray(Charsets.UTF_8)
                    val entry = ZipEntry("shared_prefs_json/$prefName.json")
                    entry.time = System.currentTimeMillis()
                    zos.putNextEntry(entry)
                    zos.write(entryBytes)
                    zos.closeEntry()
                }

                // Archive databases
                val dbDir = databasesDir
                if (dbDir.exists() && dbDir.isDirectory) {
                    val files = dbDir.listFiles() ?: emptyArray()
                    for (file in files) {
                        if (file.isFile) {
                            addFileToZip(zos, file, "databases/${file.name}")
                        }
                    }
                }
            }

            if (finalZipFile.exists()) {
                finalZipFile.delete()
            }
            if (!tmpZipFile.renameTo(finalZipFile)) {
                tmpZipFile.copyTo(finalZipFile, overwrite = true)
                tmpZipFile.delete()
            }

            return JSONObject().apply {
                put("ok", true)
                put("name", name)
            }
        } catch (e: Exception) {
            tmpZipFile.delete()
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "SNAPSHOT_ERROR")
                    put("message", e.message ?: "Failed to create snapshot")
                })
            }
        }
    }

    /**
     * Restores an archived snapshot instantly.
     */
    @Synchronized
    fun restoreSnapshot(name: String, onMutated: (() -> Unit)? = null): JSONObject {
        if (!isValidSnapshotName(name)) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "BAD_REQUEST")
                    put("message", "Invalid snapshot name '$name'.")
                })
            }
        }

        val zipFile = File(snapshotsDir, "$name.zip")
        if (!zipFile.exists()) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "SNAPSHOT_NOT_FOUND")
                    put("message", "Snapshot '$name' not found")
                })
            }
        }

        val stagingDir = File(context.cacheDir, "snapshot_restore_${System.currentTimeMillis()}")
        try {
            stagingDir.mkdirs()
            val restoredPrefsJsons = mutableMapOf<String, String>()
            val stagedDatabases = mutableListOf<File>()
            val stagedPrefsFiles = mutableListOf<File>()
            var hasMeta = false

            ZipInputStream(BufferedInputStream(FileInputStream(zipFile))).use { zis ->
                var entry: ZipEntry? = zis.nextEntry
                while (entry != null) {
                    val entryName = entry.name
                    // Prevent zip slip path traversal
                    if (entryName.contains("..") || entryName.startsWith("/")) {
                        throw SecurityException("Invalid zip entry: $entryName")
                    }

                    if (!entry.isDirectory) {
                        if (entryName == "snapshot_meta.json") {
                            hasMeta = true
                        } else if (entryName.startsWith("shared_prefs_json/") && entryName.endsWith(".json")) {
                            val prefName = entryName.removePrefix("shared_prefs_json/").removeSuffix(".json")
                            val content = zis.bufferedReader(Charsets.UTF_8).readText()
                            // Validate JSON structure
                            JSONObject(content)
                            restoredPrefsJsons[prefName] = content
                        } else if (entryName.startsWith("databases/")) {
                            val fileName = entryName.removePrefix("databases/")
                            val outFile = File(stagingDir, "databases/$fileName")
                            outFile.parentFile?.mkdirs()
                            FileOutputStream(outFile).use { fos ->
                                zis.copyTo(fos)
                            }
                            stagedDatabases.add(outFile)
                        } else if (entryName.startsWith("shared_prefs_files/")) {
                            val fileName = entryName.removePrefix("shared_prefs_files/")
                            val outFile = File(stagingDir, "shared_prefs_files/$fileName")
                            outFile.parentFile?.mkdirs()
                            FileOutputStream(outFile).use { fos ->
                                zis.copyTo(fos)
                            }
                            stagedPrefsFiles.add(outFile)
                        }
                    }
                    zis.closeEntry()
                    entry = zis.nextEntry
                }
            }

            if (!hasMeta) {
                throw java.io.IOException("Invalid snapshot archive: missing snapshot_meta.json")
            }

            // Staging and archive validation complete! Safely clear live state and move staged files
            clearSharedPreferences()
            clearDatabases()

            for (stagedFile in stagedDatabases) {
                val relPath = stagedFile.relativeTo(File(stagingDir, "databases")).path
                val targetFile = File(databasesDir, relPath)
                targetFile.parentFile?.mkdirs()
                stagedFile.copyTo(targetFile, overwrite = true)
            }

            for (stagedFile in stagedPrefsFiles) {
                val relPath = stagedFile.relativeTo(File(stagingDir, "shared_prefs_files")).path
                val targetFile = File(sharedPrefsDir, relPath)
                targetFile.parentFile?.mkdirs()
                stagedFile.copyTo(targetFile, overwrite = true)
            }

            // Apply restored preferences through BridgeStatePrefs to update both in-memory cache and file
            for ((prefName, jsonStr) in restoredPrefsJsons) {
                val json = JSONObject(jsonStr)
                val entries = json.optJSONObject("entries")
                if (entries != null) {
                    val payload = JSONObject().apply {
                        put("file", prefName)
                        put("set", entries)
                    }
                    statePrefs.mutate(payload)
                }
            }

            onMutated?.invoke()
            return JSONObject().apply {
                put("ok", true)
                put("name", name)
            }
        } catch (e: Exception) {
            return JSONObject().apply {
                put("error", JSONObject().apply {
                    put("code", "RESTORE_ERROR")
                    put("message", e.message ?: "Failed to restore snapshot")
                })
            }
        } finally {
            stagingDir.deleteRecursively()
        }
    }

    private fun addFileToZip(zos: ZipOutputStream, file: File, entryPath: String) {
        val entry = ZipEntry(entryPath)
        entry.time = file.lastModified()
        zos.putNextEntry(entry)
        FileInputStream(file).use { fis ->
            fis.copyTo(zos)
        }
        zos.closeEntry()
    }

    private fun isValidSnapshotName(name: String): Boolean {
        if (name.isBlank() || name.length > 64) return false
        return name.matches(Regex("^[a-zA-Z0-9_-]+$"))
    }
}
