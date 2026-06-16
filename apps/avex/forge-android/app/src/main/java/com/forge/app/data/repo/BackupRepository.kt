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
    private val photoRepo: ProgressPhotoRepository,
    private val db: ForgeDatabase
) {

    private val zone = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    // ── Active-time helpers (per-sitting timing) ────────────────────────────────
    /** Real active seconds: the stamped sum of sittings, falling back to wall-clock for old rows. */
    private fun activeSecondsOf(s: com.forge.app.data.db.entities.Session): Int =
        if (s.activeSeconds > 0) s.activeSeconds
        else s.finishedAt?.let { ((it - s.startedAt) / 1000L).toInt().coerceAtLeast(0) } ?: 0

    private fun activeMinutesOf(s: com.forge.app.data.db.entities.Session): Int = activeSecondsOf(s) / 60

    /** Per-sitting breakdown: [{startedAt, durationSec}] so the 13+40 split is preserved in exports. */
    private suspend fun segmentsJson(sessionId: Long): JSONArray {
        val arr = JSONArray()
        db.sessionSegmentDao().forSession(sessionId).forEach { seg ->
            arr.put(JSONObject().apply {
                put("startedAt", seg.startedAt)
                put("durationSec", seg.endedAt?.let { ((it - seg.startedAt) / 1000L).coerceAtLeast(0) } ?: 0)
            })
        }
        return arr
    }

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
                    put("activeMin", activeMinutesOf(s))
                    put("totalVolumeLb", s.totalVolumeLb ?: 0)
                    put("prCount", s.prCount)
                    put("setCount", s.setCount)
                    put("intensity", s.intensity)
                    put("tags", s.tags)
                    put("journal", s.journal)
                    put("mood", db.moodDao().forSession(s.id)?.mood ?: "")
                    put("segments", segmentsJson(s.id))
                    val exArr = JSONArray()
                    exercises.forEach { ex ->
                        val sets = loggedSetDao.forLoggedExercise(ex.id)
                        exArr.put(JSONObject().apply {
                            put("exerciseId", ex.exerciseId)
                            put("name", ex.swappedName ?: ex.exerciseId)
                            put("effort", ex.difficulty?.name ?: "")
                            put("note", ex.note ?: "")
                            put("skipped", ex.skipped)
                            val setArr = JSONArray()
                            sets.forEach { set ->
                                setArr.put(JSONObject().apply {
                                    put("weightLb", set.weightLb ?: 0)
                                    put("reps", set.reps)
                                    put("rpe", set.rpe ?: 0)
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
                    put("activeSeconds", activeSecondsOf(s))
                    put("totalVolumeLb", s.totalVolumeLb ?: 0)
                    put("prCount", s.prCount)
                    put("setCount", s.setCount)
                    put("sessionType", s.sessionType)
                    put("intensity", s.intensity)
                    put("isUntracked", s.isUntracked)
                    put("tags", s.tags)
                    put("journal", s.journal)
                    put("mood", db.moodDao().forSession(s.id)?.mood ?: "")
                    put("segments", segmentsJson(s.id))
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
                                    put("rpe", set.rpe ?: 0)
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

    /** RFC 4180 CSV field: quote and double embedded quotes when the value holds a comma/quote/newline. */
    private fun csv(value: String): String =
        if (value.any { it == ',' || it == '"' || it == '\n' || it == '\r' })
            "\"" + value.replace("\"", "\"\"") + "\""
        else value

    /** Export as CSV — sessions summary (#138). */
    suspend fun exportSessionsCsv(): File {
        val allSessions = sessionDao.allFinished()
        val sb = StringBuilder()
        sb.appendLine("id,dayKey,date,durationMin,volumeLb,prs,sets,intensity,tags")
        allSessions.forEach { s ->
            val date = dateFmt.format(Instant.ofEpochMilli(s.startedAt).atZone(zone))
            // Active (summed-sittings) minutes, NOT wall-clock — a "resume later" session spanning
            // days must report real training time, matching the JSON/PDF exports.
            val dur = activeMinutesOf(s)
            sb.appendLine("${s.id},${csv(s.dayKey)},$date,$dur,${s.totalVolumeLb ?: 0},${s.prCount},${s.setCount},${csv(s.intensity)},${csv(s.tags)}")
        }
        // Fixed filename (overwrite) — see #84.
        val file = File(context.filesDir, "forge_sessions.csv")
        file.writeText(sb.toString())
        return file
    }

    /**
     * Auto-backup: runs silently, overwrites the weekly auto-backup slot (#86). Writes a real,
     * RESTORABLE ZIP (DB + prefs + progress photos) — the same format as [backupToUri] — instead of
     * the lossy JSON export it used to write, which nothing could ever read back in.
     */
    suspend fun autoBackup(): File = withContext(Dispatchers.IO) {
        val file = File(context.filesDir, AUTO_BACKUP_NAME)
        val snap = snapshotDatabase()
        try {
            file.outputStream().use { out -> writeBackupZip(out, snap) }
        } finally {
            snap.delete()
        }
        // Drop the stale lossy JSON slot from earlier builds so it can't mislead a future restore.
        File(context.filesDir, "forge_auto_backup.json").delete()
        file
    }

    /** When the auto-backup slot was last written, or null if none exists yet (#86 restore affordance). */
    fun autoBackupSavedAtMs(): Long? =
        File(context.filesDir, AUTO_BACKUP_NAME).takeIf { it.exists() }?.lastModified()

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
                writeBackupZip(out, snap)
            } ?: error("Could not open the chosen destination")
        } finally {
            snap.delete()
        }
    }

    /**
     * Writes the backup archive to [out]: the DB snapshot, the DataStore prefs (if present), and
     * every progress-photo file (under [PHOTOS_PREFIX]). Shared by [backupToUri] and [autoBackup]
     * so the two formats can never drift. The ZipOutputStream's use{} closes [out].
     */
    private fun writeBackupZip(out: java.io.OutputStream, snap: File) {
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
            // Progress photos live as files outside the DB (ProgressPhotoRepository) — fold the folder
            // in so a restore brings the physique photos back too (#138). Flat: progress_photos/<name>.
            val photosDir = photoRepo.dir
            if (photosDir.exists()) {
                photosDir.listFiles()?.forEach { f ->
                    if (f.isFile) {
                        zip.putNextEntry(java.util.zip.ZipEntry("$PHOTOS_PREFIX${f.name}"))
                        f.inputStream().use { it.copyTo(zip) }
                        zip.closeEntry()
                    }
                }
            }
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
        restoreFromIncoming(incoming)
    }

    /**
     * Restore from the weekly auto-backup slot (#86) — the same validation + staging as a user-picked
     * file, so this is the in-app recovery path for the otherwise write-only auto-backup. Copies the
     * slot to a cache temp first so [restoreFromIncoming]'s cleanup never deletes the live slot itself.
     */
    suspend fun restoreFromAutoBackup(): Boolean = withContext(Dispatchers.IO) {
        val auto = File(context.filesDir, AUTO_BACKUP_NAME)
        if (!auto.exists()) return@withContext false
        val incoming = File(context.cacheDir, "forge_restore_in_${System.currentTimeMillis()}")
        if (incoming.exists()) incoming.delete()
        auto.copyTo(incoming, overwrite = true)
        restoreFromIncoming(incoming)
    }

    /**
     * Validate [incoming] and, if it's a real Forge backup, STAGE the DB, prefs and progress photos
     * as pending files that [com.forge.app.ForgeApp.applyPendingRestore] swaps in atomically at next
     * boot — DB, prefs and photos together, so a kill or copy failure can never leave the live DB and
     * photo folder from different backups. Returns true once everything is staged. Deletes [incoming].
     */
    private suspend fun restoreFromIncoming(incoming: File): Boolean = withContext(Dispatchers.IO) {
        val temps = mutableListOf(incoming) // cache-dir temp files to clean up before returning
        var photoStage: File? = null        // extracted progress photos, staged only after validation
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
                        val name = entry.name
                        when {
                            name == ZIP_DB_ENTRY -> { exDb.outputStream().use { zin.copyTo(it) }; sawDb = true }
                            name == ZIP_PREFS_ENTRY -> {
                                val exPrefs = File(context.cacheDir, "forge_restore_prefs_${System.currentTimeMillis()}.pb")
                                    .also { it.delete(); temps.add(it) }
                                exPrefs.outputStream().use { zin.copyTo(it) }
                                prefsFile = exPrefs
                            }
                            name.startsWith(PHOTOS_PREFIX) -> {
                                val photoName = name.removePrefix(PHOTOS_PREFIX)
                                // Flat basename only — zip-slip guard against path-traversal entries.
                                if (photoName.isNotBlank() && !photoName.contains('/') && !photoName.contains('\\')) {
                                    val stage = photoStage ?: File(
                                        context.cacheDir, "forge_restore_photos_${System.currentTimeMillis()}"
                                    ).also { it.deleteRecursively(); it.mkdirs(); photoStage = it }
                                    File(stage, photoName).outputStream().use { zin.copyTo(it) }
                                }
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
            //
            // Also reject anything below the migration floor (<12, the pre-lock versions): there is no
            // migration path for those, so Room's fallbackToDestructiveMigrationFrom(1..11) would DROP
            // ALL TABLES on open — restoring such a backup would silently wipe it. Refuse instead.
            val currentVersion = db.openHelper.readableDatabase.version
            val incomingVersion = databaseUserVersion(dbFile)
            if (incomingVersion > currentVersion || incomingVersion < MIN_RESTORABLE_VERSION) return@withContext false

            // Don't close Room and swap the file here — that races with any flow still reading the DB
            // until the process is killed. Stage the files instead; ForgeApp.applyPendingRestore swaps
            // them in at next boot, before Room/DataStore open. The caller restarts the app on success.
            val pendingDb = File(context.filesDir, "pending_restore.db")
            if (pendingDb.exists()) pendingDb.delete()
            dbFile.copyTo(pendingDb, overwrite = true)

            val pendingPrefs = File(context.filesDir, "pending_restore_prefs.pb")
            if (pendingPrefs.exists()) pendingPrefs.delete()
            prefsFile?.copyTo(pendingPrefs, overwrite = true)

            // Photos passed validation alongside the DB — stage them as a pending folder rather than
            // touching the live one. ForgeApp.applyPendingRestore swaps it in at boot in the SAME pass
            // as the DB, so the two can't end up from different backups (and a refused restore, which
            // returns before here, never creates the pending folder → existing photos stay put). A
            // null photoStage (pre-photos backup) leaves the live folder untouched, as before.
            photoStage?.let { stage ->
                val pendingPhotos = File(context.filesDir, PENDING_PHOTOS_DIR)
                pendingPhotos.deleteRecursively()
                // cacheDir and filesDir share the app's internal filesystem, so the move is atomic and
                // free; fall back to a copy if a rename across the two is ever refused.
                if (!stage.renameTo(pendingPhotos)) {
                    pendingPhotos.mkdirs()
                    stage.listFiles()?.forEach { it.copyTo(File(pendingPhotos, it.name), overwrite = true) }
                }
            }

            return@withContext true
        } finally {
            temps.forEach { it.delete() }
            photoStage?.deleteRecursively()
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
        /** Prefix for progress-photo entries in the backup ZIP: progress_photos/<basename>. */
        private const val PHOTOS_PREFIX = "progress_photos/"
        /** The weekly auto-backup slot, written by [autoBackup] and read by [restoreFromAutoBackup] (#86). */
        private const val AUTO_BACKUP_NAME = "forge_auto_backup.zip"
        /** Staged restored photos; ForgeApp.applyPendingRestore swaps this over progress_photos/ at boot. */
        private const val PENDING_PHOTOS_DIR = "pending_restore_photos"
        /**
         * Lowest schema version restore will accept. Below this are the pre-lock versions Room
         * destructively resets (DatabaseModule.fallbackToDestructiveMigrationFrom(1..11)), so
         * restoring one would wipe rather than recover. Must stay one above that destructive range.
         */
        private const val MIN_RESTORABLE_VERSION = 12
    }
}
