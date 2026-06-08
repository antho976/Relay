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

        val file = File(context.filesDir, "forge_weekly_export_${System.currentTimeMillis()}.json")
        file.writeText(root.toString(2))
        return file
    }

    /** Full backup: all sessions + exercises + sets + cardio + settings (#6, #138). Returns the file. */
    suspend fun exportFullBackup(): File {
        val allSessions = sessionDao.allFinished()
        val allCardio = cardioDao.since(0L)

        val root = JSONObject().apply {
            put("backupVersion", 1)
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            put("appVersion", "tier6")

            // User-facing preferences (the JSON export documents 'settings'). The whole-DB
            // VACUUM backup remains the authoritative restore source.
            put("settings", JSONObject().apply {
                put("useKg", settingsRepo.useKg.first())
                put("userGoal", settingsRepo.userGoal.first())
                put("userName", settingsRepo.userName.first())
                put("daysPerWeek", settingsRepo.daysPerWeek.first())
                put("cardioDaysPerWeek", settingsRepo.cardioDaysPerWeek.first())
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

        val ts = System.currentTimeMillis()
        val file = File(context.filesDir, "forge_backup_$ts.json")
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
        val file = File(context.filesDir, "forge_sessions_${System.currentTimeMillis()}.csv")
        file.writeText(sb.toString())
        return file
    }

    /** Auto-backup: runs silently, overwrites the weekly auto-backup slot (#86). */
    suspend fun autoBackup(): File {
        val file = File(context.filesDir, "forge_auto_backup.json")
        val full = exportFullBackup()
        full.copyTo(file, overwrite = true)
        full.delete()
        return file
    }

    // ─── Complete database backup & restore ───────────────────────────────────
    // Unlike the JSON exports above (lossy + app-private), these copy the *entire*
    // database file. A backup captures every table and column — including any added
    // later — and restore brings it back byte-for-byte. The real safety net.

    /**
     * Consolidated single-file snapshot of the whole DB via `VACUUM INTO` — one file, no
     * WAL/-shm sidecars, a consistent point-in-time copy. Written to cache; callers stream
     * it to a user-chosen destination.
     */
    private fun snapshotDatabase(): File {
        val temp = File(context.cacheDir, "forge_snapshot_${System.currentTimeMillis()}.db")
        if (temp.exists()) temp.delete()
        val safePath = temp.absolutePath.replace("'", "''")
        try {
            db.openHelper.writableDatabase.execSQL("VACUUM INTO '$safePath'")
        } catch (t: Throwable) {
            temp.delete() // don't leave a half-written snapshot in cache if VACUUM fails
            throw t
        }
        return temp
    }

    /**
     * Write a complete backup to a user-picked [uri] (e.g. Downloads). Survives uninstall,
     * unlike the app-private exports.
     */
    suspend fun backupToUri(uri: Uri) = withContext(Dispatchers.IO) {
        val snap = snapshotDatabase()
        try {
            context.contentResolver.openOutputStream(uri)?.use { out ->
                snap.inputStream().use { it.copyTo(out) }
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
        val temp = File(context.cacheDir, "forge_restore_${System.currentTimeMillis()}.db")
        if (temp.exists()) temp.delete()
        context.contentResolver.openInputStream(uri)?.use { input ->
            temp.outputStream().use { input.copyTo(it) }
        } ?: return@withContext false

        if (!isForgeDatabase(temp)) { temp.delete(); return@withContext false }
        // Reject a backup newer than this build's schema: Room has no downgrade path and would
        // crash on open — after the live DB was already replaced (unrecoverable). Older versions
        // migrate forward normally, so only a strictly-newer user_version is rejected.
        val currentVersion = db.openHelper.readableDatabase.version
        if (databaseUserVersion(temp) > currentVersion) { temp.delete(); return@withContext false }

        // Don't close Room and swap the file here — that races with any flow still reading the DB
        // until the process is killed. Stage the file instead; ForgeApp.applyPendingRestore swaps
        // it in at next boot, before Room opens. The caller restarts the app on success.
        val pending = File(context.filesDir, "pending_restore.db")
        if (pending.exists()) pending.delete()
        temp.copyTo(pending, overwrite = true)
        temp.delete()
        return@withContext true
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
}
