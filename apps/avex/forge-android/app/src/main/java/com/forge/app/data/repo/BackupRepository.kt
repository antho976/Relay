package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext

/**
 * Handles data export and backup operations (#5, #6, #86, #138).
 * All exports go to app-private files directory — no storage permissions needed.
 */
@Singleton
class BackupRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val cardioDao: CardioDao,
    private val settingsRepo: SettingsRepository,
    private val db: ForgeDatabase
) {

    private val zone = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    /** Export this week's data as JSON for AI analysis (#5). Returns the file path. */
    suspend fun exportWeeklyJson(): File {
        val weekStartMs = System.currentTimeMillis() - 7L * 24 * 3600 * 1000
        // Window on finish time so a session that started before the boundary but finished this
        // week is still included in the export.
        val sessions = sessionDao.finishedByFinishTimeInRange(weekStartMs, System.currentTimeMillis())
        val cardioEntries = cardioDao.since(weekStartMs)

        val root = JSONObject().apply {
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            put("periodDays", 7)
            val sessArr = JSONArray()
            sessions.forEach { s ->
                val exercises = loggedExerciseDao.forSession(s.id)
                val sObj = JSONObject().apply {
                    put("id", s.id)
                    put("dayKey", s.dayKey)
                    put("date", dateFmt.format(Instant.ofEpochMilli(s.startedAt).atZone(zone)))
                    put("durationMin", s.finishedAt?.let { ((it - s.startedAt) / 60_000).toInt() } ?: 0)
                    put("totalVolumeLb", s.totalVolumeLb ?: 0)
                    put("prCount", s.prCount)
                    put("setCount", s.setCount)
                    put("intensity", s.intensity)
                    put("tags", s.tags)
                    val exArr = JSONArray()
                    exercises.forEach { ex ->
                        val sets = loggedSetDao.forLoggedExercise(ex.id)
                        exArr.put(JSONObject().apply {
                            put("exerciseId", ex.exerciseId)
                            put("name", ex.swappedName ?: ex.exerciseId)
                            put("difficulty", ex.difficulty?.name ?: "")
                            put("skipped", ex.skipped)
                            val setArr = JSONArray()
                            sets.forEach { set ->
                                setArr.put(JSONObject().apply {
                                    put("weightLb", set.weightLb ?: 0)
                                    put("reps", set.reps)
                                    put("isPr", false)
                                })
                            }
                            put("sets", setArr)
                        })
                    }
                    put("exercises", exArr)
                }
                sessArr.put(sObj)
            }
            put("sessions", sessArr)
            val cardioArr = JSONArray()
            cardioEntries.forEach { c ->
                cardioArr.put(JSONObject().apply {
                    put("date", dateFmt.format(Instant.ofEpochMilli(c.date).atZone(zone)))
                    put("type", c.type)
                    put("durationMin", c.durationMin)
                    put("distanceKm", c.distanceKm ?: 0)
                    put("effort", c.effort ?: "")
                })
            }
            put("cardio", cardioArr)
        }

        // Fixed filename (overwrite) so repeated exports don't accumulate forever (#84).
        val file = File(context.filesDir, "forge_weekly_export.json")
        file.writeText(root.toString(2))
        return file
    }

    /**
     * Full data dump as JSON — every session + exercise + set + cardio entry + a snapshot of key
     * settings. This is a *lossy, human/AI-readable export*, NOT a restore source: nothing reads it
     * back in. The real restore path is the whole-DB backup ([backupToUri] / [restoreFromUri]).
     * Named so it doesn't imply recoverability (#70).
     */
    suspend fun exportFullDataJson(): File {
        val allSessions = sessionDao.allFinished()
        val allCardio = cardioDao.since(0L)

        val root = JSONObject().apply {
            put("exportVersion", 1)
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            put("appVersion", "tier6")

            // User-facing preferences (the JSON export documents 'settings'). The whole-DB
            // VACUUM backup remains the authoritative restore source.
            put("settings", JSONObject().apply {
                put("useKg", settingsRepo.useKg.first())
                put("userGoal", settingsRepo.userGoal.first())
                put("userName", settingsRepo.userName.first())
                put("daysPerWeek", settingsRepo.daysPerWeek.first())
                put("firstDayMonday", settingsRepo.firstDayMonday.first())
            })

            val sessArr = JSONArray()
            allSessions.forEach { s ->
                val exercises = loggedExerciseDao.forSession(s.id)
                val sObj = JSONObject().apply {
                    put("id", s.id)
                    put("dayKey", s.dayKey)
                    put("startedAt", s.startedAt)
                    put("finishedAt", s.finishedAt ?: 0)
                    put("totalVolumeLb", s.totalVolumeLb ?: 0)
                    put("prCount", s.prCount)
                    put("setCount", s.setCount)
                    put("sessionType", s.sessionType)
                    put("intensity", s.intensity)
                    put("isUntracked", s.isUntracked)
                    put("tags", s.tags)
                    put("journal", s.journal)
                    val exArr = JSONArray()
                    exercises.forEach { ex ->
                        val sets = loggedSetDao.forLoggedExercise(ex.id)
                        exArr.put(JSONObject().apply {
                            put("exerciseId", ex.exerciseId)
                            put("swappedName", ex.swappedName ?: "")
                            put("orderIndex", ex.orderIndex)
                            put("difficulty", ex.difficulty?.name ?: "")
                            put("skipped", ex.skipped)
                            put("note", ex.note ?: "")
                            val setArr = JSONArray()
                            sets.forEach { set ->
                                setArr.put(JSONObject().apply {
                                    put("weightText", set.weightText)
                                    put("weightLb", set.weightLb ?: 0)
                                    put("reps", set.reps)
                                    put("completedAt", set.completedAt)
                                    put("difficultyTag", set.difficultyTag ?: "")
                                })
                            }
                            put("sets", setArr)
                        })
                    }
                    put("exercises", exArr)
                }
                sessArr.put(sObj)
            }
            put("sessions", sessArr)

            val cardioArr = JSONArray()
            allCardio.forEach { c ->
                cardioArr.put(JSONObject().apply {
                    put("date", c.date)
                    put("type", c.type)
                    put("durationMin", c.durationMin)
                    put("distanceKm", c.distanceKm ?: 0)
                    put("effort", c.effort ?: "")
                    put("restReason", c.restReason ?: "")
                    put("note", c.note ?: "")
                })
            }
            put("cardio", cardioArr)
        }

        // Fixed filename (overwrite) — see #84; avoids unbounded accumulation in filesDir.
        val file = File(context.filesDir, "forge_export.json")
        file.writeText(root.toString(2))
        return file
    }

    /** Export as CSV — sessions summary (#138). */
    suspend fun exportSessionsCsv(): File {
        val allSessions = sessionDao.allFinished()
        val sb = StringBuilder()
        sb.appendLine("id,dayKey,date,durationMin,volumeLb,prs,sets,intensity,tags")
        allSessions.forEach { s ->
            val date = dateFmt.format(Instant.ofEpochMilli(s.startedAt).atZone(zone))
            val dur = s.finishedAt?.let { ((it - s.startedAt) / 60_000).toInt() } ?: 0
            sb.appendLine("${s.id},${s.dayKey},$date,$dur,${s.totalVolumeLb ?: 0},${s.prCount},${s.setCount},${s.intensity},\"${s.tags}\"")
        }
        // Fixed filename (overwrite) — see #84.
        val file = File(context.filesDir, "forge_sessions.csv")
        file.writeText(sb.toString())
        return file
    }

    /** Auto-backup: runs silently, overwrites the weekly auto-backup slot (#86). */
    suspend fun autoBackup(): File {
        val file = File(context.filesDir, "forge_auto_backup.json")
        val full = exportFullDataJson()
        full.copyTo(file, overwrite = true)
        full.delete()
        return file
    }

    // ─── Complete database backup & restore ───────────────────────────────────
    // Unlike the JSON exports above (lossy + app-private), these capture the *entire*
    // database file PLUS the DataStore preferences (settings + the whole program-generation
    // config), zipped together (#14). A backup grabs every table and column — including any
    // added later — and restore brings both back. The real safety net. Legacy raw-.db
    // backups (pre-#14) still restore via the format sniff in restoreFromUri.

    /**
     * Consolidated single-file snapshot of the whole DB. Written to cache; callers stream it to a
     * user-chosen destination.
     *
     * Preferred path is `VACUUM INTO` — one file, no WAL/-shm sidecars, a consistent point-in-time
     * copy. But `VACUUM INTO` needs SQLite ≥ 3.27, which only ships with Android 11+ (the bundled
     * SQLite is tied to the OS version); on older devices it fails to compile with
     * "near 'INTO': syntax error". So we fall back to checkpointing the WAL into the main DB and
     * copying the file: `wal_checkpoint(TRUNCATE)` merges every pending frame into the main file
     * and empties the WAL, and a user-initiated backup on this single-user offline app has no
     * concurrent writers, so the on-disk file is a consistent snapshot.
     */
    private fun snapshotDatabase(): File {
        val temp = File(context.cacheDir, "forge_snapshot_${System.currentTimeMillis()}.db")
        if (temp.exists()) temp.delete()
        val writable = db.openHelper.writableDatabase
        try {
            val safePath = temp.absolutePath.replace("'", "''")
            writable.execSQL("VACUUM INTO '$safePath'")
        } catch (vacuumError: Throwable) {
            // Old-SQLite fallback. Start from a clean temp in case VACUUM left a partial file.
            if (temp.exists()) temp.delete()
            try {
                writable.query("PRAGMA wal_checkpoint(TRUNCATE)").use { it.moveToFirst() }
                val livePath = writable.path ?: context.getDatabasePath(DB_NAME).path
                File(livePath).copyTo(temp, overwrite = true)
                // The live DB is WAL-mode, so the copy's header says WAL but has no -wal/-shm
                // beside it — which older SQLite (< 3.22) can't open read-only, and restore
                // validation opens read-only. Rewrite the copy to a rollback-journal DB so it
                // loads anywhere, matching the clean non-WAL file VACUUM INTO would have produced.
                android.database.sqlite.SQLiteDatabase.openDatabase(
                    temp.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READWRITE
                ).use { it.execSQL("PRAGMA journal_mode=DELETE") }
            } catch (copyError: Throwable) {
                temp.delete() // don't leave a half-written snapshot in cache if both paths fail
                vacuumError.addSuppressed(copyError)
                throw vacuumError
            }
        }
        return temp
    }

    /**
     * Write a complete backup to a user-picked [uri] (e.g. Downloads). Survives uninstall,
     * unlike the app-private exports. The backup is a ZIP of the DB snapshot + the DataStore
     * preferences file, so a restore brings settings back too (#14).
     */
    suspend fun backupToUri(uri: Uri) = withContext(Dispatchers.IO) {
        val snap = snapshotDatabase()
        try {
            context.contentResolver.openOutputStream(uri)?.use { out ->
                java.util.zip.ZipOutputStream(out).use { zip ->
                    zip.putNextEntry(java.util.zip.ZipEntry(ZIP_DB_ENTRY))
                    snap.inputStream().use { it.copyTo(zip) }
                    zip.closeEntry()
                    // Preferences may not exist yet on a brand-new install — only add if present.
                    val prefs = preferencesFile()
                    if (prefs.exists()) {
                        zip.putNextEntry(java.util.zip.ZipEntry(ZIP_PREFS_ENTRY))
                        prefs.inputStream().use { it.copyTo(zip) }
                        zip.closeEntry()
                    }
                }
            } ?: error("Could not open the chosen destination")
        } finally {
            snap.delete()
        }
    }

    /**
     * Replace the live database with the backup at [uri]. Validates it's a real Forge DB
     * first; only then closes Room and swaps the file. Returns true on success — the caller
     * MUST restart the app afterward (Room is closed and the file replaced underneath it).
     */
    suspend fun restoreFromUri(uri: Uri): Boolean = withContext(Dispatchers.IO) {
        val incoming = File(context.cacheDir, "forge_restore_in_${System.currentTimeMillis()}")
        if (incoming.exists()) incoming.delete()
        context.contentResolver.openInputStream(uri)?.use { input ->
            incoming.outputStream().use { input.copyTo(it) }
        } ?: return@withContext false

        val temps = mutableListOf(incoming) // cache-dir temp files to clean up before returning
        try {
            // Sniff the format: a #14 backup is a ZIP { database.db, settings.preferences_pb };
            // a pre-#14 backup is the raw SQLite DB. Restore both.
            var dbFile = incoming
            var prefsFile: File? = null
            if (isZip(incoming)) {
                val exDb = File(context.cacheDir, "forge_restore_db_${System.currentTimeMillis()}.db")
                    .also { it.delete(); temps.add(it) }
                var sawDb = false
                java.util.zip.ZipInputStream(incoming.inputStream()).use { zin ->
                    var entry = zin.nextEntry
                    while (entry != null) {
                        when (entry.name) {
                            ZIP_DB_ENTRY -> { exDb.outputStream().use { zin.copyTo(it) }; sawDb = true }
                            ZIP_PREFS_ENTRY -> {
                                val exPrefs = File(context.cacheDir, "forge_restore_prefs_${System.currentTimeMillis()}.pb")
                                    .also { it.delete(); temps.add(it) }
                                exPrefs.outputStream().use { zin.copyTo(it) }
                                prefsFile = exPrefs
                            }
                        }
                        zin.closeEntry()
                        entry = zin.nextEntry
                    }
                }
                if (!sawDb) return@withContext false
                dbFile = exDb
            }

            if (!isForgeDatabase(dbFile)) return@withContext false
            // Reject a backup newer than this build's schema: Room has no downgrade path and would
            // crash on open. Older versions migrate forward normally, so only a strictly-newer
            // user_version is rejected. (We stage rather than swap live, so nothing is lost on reject.)
            val currentVersion = db.openHelper.readableDatabase.version
            if (databaseUserVersion(dbFile) > currentVersion) return@withContext false

            // Don't close Room and swap the file here — that races with any flow still reading the DB
            // until the process is killed. Stage the files instead; ForgeApp.applyPendingRestore swaps
            // them in at next boot, before Room/DataStore open. The caller restarts the app on success.
            val pendingDb = File(context.filesDir, "pending_restore.db")
            if (pendingDb.exists()) pendingDb.delete()
            dbFile.copyTo(pendingDb, overwrite = true)

            val pendingPrefs = File(context.filesDir, "pending_restore_prefs.pb")
            if (pendingPrefs.exists()) pendingPrefs.delete()
            prefsFile?.copyTo(pendingPrefs, overwrite = true)

            return@withContext true
        } finally {
            temps.forEach { it.delete() }
        }
    }

    /** Cheap sanity check that a candidate file is a SQLite DB containing our `session` table. */
    private fun isForgeDatabase(file: File): Boolean = runCatching {
        android.database.sqlite.SQLiteDatabase.openDatabase(
            file.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READONLY
        ).use { dbFile ->
            dbFile.rawQuery(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='session'", null
            ).use { c -> c.moveToFirst() && c.getInt(0) > 0 }
        }
    }.getOrDefault(false)

    /** The SQLite user_version (Room schema version) of a candidate DB file; MAX if unreadable (→ rejected). */
    private fun databaseUserVersion(file: File): Int = runCatching {
        android.database.sqlite.SQLiteDatabase.openDatabase(
            file.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READONLY
        ).use { it.version }
    }.getOrDefault(Int.MAX_VALUE)

    /**
     * Zip the crash logs (ForgeApp writes them to filesDir/crashes) into [uri].
     * Returns how many logs were included (0 = none yet).
     */
    suspend fun exportCrashLogsToUri(uri: Uri): Int = withContext(Dispatchers.IO) {
        val dir = File(context.filesDir, "crashes")
        val files = dir.listFiles()?.filter { it.isFile }?.sortedByDescending { it.lastModified() } ?: emptyList()
        context.contentResolver.openOutputStream(uri)?.use { out ->
            java.util.zip.ZipOutputStream(out).use { zip ->
                files.forEach { f ->
                    zip.putNextEntry(java.util.zip.ZipEntry(f.name))
                    f.inputStream().use { it.copyTo(zip) }
                    zip.closeEntry()
                }
            }
        } ?: error("Could not open the chosen destination")
        files.size
    }

    /** The DataStore preferences file backing [com.forge.app.data.prefs.forgePreferences]. */
    private fun preferencesFile(): File =
        File(context.filesDir, "datastore/$PREFS_DATASTORE_NAME.preferences_pb")

    /** True if [file] starts with the ZIP local-file-header magic (PK). */
    private fun isZip(file: File): Boolean = runCatching {
        file.inputStream().use { ins ->
            val sig = ByteArray(4)
            ins.read(sig) == 4 &&
                sig[0] == 0x50.toByte() && sig[1] == 0x4B.toByte() &&
                sig[2] == 0x03.toByte() && sig[3] == 0x04.toByte()
        }
    }.getOrDefault(false)

    companion object {
        /** Must match the name in [com.forge.app.di.DatabaseModule] and ForgeApp.applyPendingRestore. */
        private const val DB_NAME = "forge.db"
        /** Must match the name in [com.forge.app.data.prefs.forgePreferences]. */
        private const val PREFS_DATASTORE_NAME = "forge_settings"
        private const val ZIP_DB_ENTRY = "database.db"
        private const val ZIP_PREFS_ENTRY = "settings.preferences_pb"
    }
}
