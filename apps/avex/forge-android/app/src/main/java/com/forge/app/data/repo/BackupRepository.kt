package com.forge.app.data.repo

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
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
    private val coachGoalDao: com.forge.app.data.db.dao.CoachGoalDao,
    private val settingsRepo: SettingsRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository,
    private val db: ForgeDatabase
) {

    private val zone = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    /** The outcome of a restore attempt — distinct reasons so the UI can explain a failure (E6). */
    enum class RestoreOutcome { SUCCESS, NOT_A_BACKUP, NEWER_VERSION, TOO_OLD, CORRUPT, TOO_LARGE, IO_ERROR, NO_BACKUP_FILE }

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
        val file = File(context.filesDir, "avex_weekly_export.json")
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
        val allCoachGoals = coachGoalDao.all()

        val root = JSONObject().apply {
            put("exportVersion", 1)
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            put("appVersion", com.forge.app.BuildConfig.VERSION_NAME)

            // User-facing preferences (the JSON export documents 'settings'). The whole-DB
            // VACUUM backup remains the authoritative restore source.
            put("settings", JSONObject().apply {
                put("useKg", settingsRepo.useKg.first())
                put("weightUnit", settingsRepo.weightUnit.first().label)
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
                    // Per-type fields (GYMAP-38); elevation stays canonical metres like distance is km.
                    put("inclinePct", c.inclinePct ?: "")
                    put("laps", c.laps ?: "")
                    put("elevationM", c.elevationM ?: "")
                })
            }
            put("cardio", cardioArr)

            // Coach goals (v3 A2). The ZIP backup carries every table automatically; this hand-rolled
            // JSON does not, so each coach phase adds its own rows here — goals are exactly the kind of
            // thing a user wants out of the app, and they were invisible to the JSON export before.
            val goalsArr = JSONArray()
            allCoachGoals.forEach { g ->
                goalsArr.put(JSONObject().apply {
                    put("kind", g.kind)
                    put("targetKey", g.targetKey)
                    put("targetValue", g.targetValue ?: "")
                    put("priority", g.priority)
                    put("createdAt", g.createdAt)
                    put("completedAt", g.completedAt ?: "")
                    put("archivedAt", g.archivedAt ?: "")
                    put("source", g.source)
                    put("note", g.note)
                })
            }
            put("coachGoals", goalsArr)
        }

        // Fixed filename (overwrite) — see #84; avoids unbounded accumulation in filesDir.
        val file = File(context.filesDir, "avex_export.json")
        file.writeText(root.toString(2))
        return file
    }

    /**
     * Export a SINGLE finished session as JSON — the per-session "save this workout's data" action on
     * the session detail page. Same lossy/human-readable shape as one entry of [exportFullDataJson],
     * with library-resolved exercise names. Returns null when the session id no longer exists.
     */
    suspend fun exportSessionJson(sessionId: Long): File? = withContext(Dispatchers.IO) {
        val s = sessionDao.get(sessionId) ?: return@withContext null
        val exercises = loggedExerciseDao.forSession(s.id)
        val root = JSONObject().apply {
            put("exportVersion", 1)
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            put("appVersion", com.forge.app.BuildConfig.VERSION_NAME)
            put("session", JSONObject().apply {
                put("id", s.id)
                put("dayKey", s.dayKey)
                put("date", dateFmt.format(Instant.ofEpochMilli(s.startedAt).atZone(zone)))
                put("startedAt", s.startedAt)
                put("finishedAt", s.finishedAt ?: 0)
                put("activeSeconds", activeSecondsOf(s))
                put("totalVolumeLb", s.totalVolumeLb ?: 0.0)
                put("prCount", s.prCount)
                put("setCount", s.setCount)
                put("sessionType", s.sessionType)
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
                        put("name", com.forge.app.program.Program.exerciseDisplayName(ex.exerciseId, ex.swappedName))
                        put("orderIndex", ex.orderIndex)
                        put("difficulty", ex.difficulty?.name ?: "")
                        put("skipped", ex.skipped)
                        put("note", ex.note ?: "")
                        val setArr = JSONArray()
                        sets.forEach { set ->
                            setArr.put(JSONObject().apply {
                                put("weightText", set.weightText)
                                put("weightLb", set.weightLb ?: 0.0)
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
            })
        }
        // Session-id in the filename so saving several sessions doesn't overwrite one another (and a
        // re-export of the same session overwrites its own file rather than accumulating).
        val file = File(context.filesDir, "avex_session_${s.id}.json")
        file.writeText(root.toString(2))
        file
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
        val file = File(context.filesDir, "avex_sessions.csv")
        file.writeText(sb.toString())
        return file
    }

    /**
     * All-time best lift per exercise (the "hall of fame") as CSV — PRs are the most personally
     * motivating records to share or keep (#542). One row per exercise: its heaviest tracked set ever.
     * Reuses the same tracked/non-assisted set projection the Stats hall of fame is built from.
     */
    suspend fun exportPrsCsv(): File {
        val sets = loggedSetDao.observeAllFinishedSetsWithSession().first()
        val sb = StringBuilder()
        sb.appendLine("exercise,muscle,bestWeightLb,reps,date")
        // Reuse the exact aggregation the Stats hall of fame renders from, so the exported PRs can
        // never diverge from what the user sees in-app (one place to fix tie-breaking / attribution).
        buildHallOfFame(sets).forEach { pr ->
            val date = dateFmt.format(Instant.ofEpochMilli(pr.sessionDate).atZone(zone))
            sb.appendLine("${csv(pr.exerciseName)},${csv(pr.muscle.displayName)},${pr.maxWeightLb},${pr.bestReps},$date")
        }
        val file = File(context.filesDir, "avex_prs.csv")
        file.writeText(sb.toString())
        return file
    }

    /** Every bodyweight weigh-in as CSV (Cat 11). One row per entry, newest first. */
    suspend fun exportBodyweightCsv(): File {
        val entries = db.bodyweightDao().all()
        val sb = StringBuilder()
        sb.appendLine("date,weightLb")
        entries.forEach { e -> sb.appendLine("${e.dateKey},${e.weightLb}") }
        val file = File(context.filesDir, "avex_bodyweight.csv")
        file.writeText(sb.toString())
        return file
    }

    /**
     * Every cardio entry as CSV (GYMAP-43). One row per entry, newest first. The type is resolved to
     * its display name (custom activities included, GYMAP-37) so the sheet reads as words, not raw
     * `custom_…` codes; distance stays canonical km like the other exports' canonical-unit columns.
     */
    suspend fun exportCardioCsv(): File {
        val entries = cardioDao.since(0L)
        val customs = settingsRepo.customCardioTypes.first()
        val sb = StringBuilder()
        // Per-type columns (GYMAP-38) trail the note; elevation stays canonical metres like distanceKm.
        // Conditions (GYMAP-39) trail last, resolved to words (" · " joined) like the type column.
        sb.appendLine("date,type,durationMin,distanceKm,effort,restReason,intervals,hrZone,note,inclinePct,laps,elevationM,conditions")
        entries.forEach { e ->
            val date = dateFmt.format(Instant.ofEpochMilli(e.date).atZone(zone))
            val typeName = CardioActivity.resolve(e.type, customs).displayName
            val conditions = CardioCondition.decode(e.conditions).joinToString(" · ") { it.displayName }
            sb.appendLine(
                "$date,${csv(typeName)},${e.durationMin},${e.distanceKm ?: ""}," +
                    "${csv(e.effort ?: "")},${csv(e.restReason ?: "")},${e.intervalCount ?: ""}," +
                    "${csv(e.hrZone ?: "")},${csv(e.note ?: "")},${e.inclinePct ?: ""},${e.laps ?: ""},${e.elevationM ?: ""}," +
                    csv(conditions)
            )
        }
        val file = File(context.filesDir, "avex_cardio.csv")
        file.writeText(sb.toString())
        return file
    }

    /** Total on-disk database size (main file + WAL + SHM), in bytes — the Data dialog readout. */
    fun dbSizeBytes(): Long {
        val base = context.getDatabasePath(DB_NAME)
        return listOf(base.path, "${base.path}-wal", "${base.path}-shm")
            .sumOf { p -> File(p).takeIf { it.exists() }?.length() ?: 0L }
    }

    /**
     * Auto-backup: runs silently, overwrites the weekly auto-backup slot (#86). Writes a real,
     * RESTORABLE ZIP (DB + prefs + progress photos) — the same format as [backupToUri] — instead of
     * the lossy JSON export it used to write, which nothing could ever read back in.
     */
    suspend fun autoBackup(folderUri: Uri? = null): File = withContext(Dispatchers.IO) {
        val file = File(context.filesDir, AUTO_BACKUP_NAME)
        val tmp = File(context.filesDir, AUTO_BACKUP_TMP_NAME)
        val snap = snapshotDatabase()
        try {
            // Write to a temp file and rename over the slot, rather than truncating the slot and
            // writing into it. `File.outputStream()` truncates on open, so the old behaviour
            // destroyed the previous good backup the instant it started writing — and a failure
            // part-way (ENOSPC is the likely one, since a full disk is exactly when a backup is
            // attempted and fails) left a truncated, unrestorable zip. autoBackupSavedAtMs() reads
            // this file's mtime, so the user was then shown a fresh "last backed up" date for a
            // backup that could not be restored. rename(2) within a directory is atomic, so the
            // slot now only ever holds a complete zip.
            tmp.delete()
            tmp.outputStream().use { out -> writeBackupZip(out, snap) }
            if (!tmp.renameTo(file)) {
                // Same-directory rename should not fail on Android. If it somehow does, keep the
                // previous backup instead of truncating it for a copy we cannot guarantee; the
                // worker retries, and records a failure the user can see once retries run out.
                throw java.io.IOException("could not replace $AUTO_BACKUP_NAME")
            }
            // Also mirror into a user-picked folder so the backup survives an uninstall (GYMAP-67). A
            // folder write must not fail the whole backup — the internal copy already succeeded.
            if (folderUri != null) runCatching { writeZipToFolder(folderUri, snap) }
        } finally {
            tmp.delete()
            snap.delete()
        }
        // Drop the stale lossy JSON slot from earlier builds so it can't mislead a future restore.
        File(context.filesDir, "forge_auto_backup.json").delete()
        // A successful write clears any prior "last backup failed" marker.
        File(context.filesDir, AUTO_BACKUP_FAILED_MARKER).delete()
        file
    }

    /** Write the full backup zip into a user-granted SAF tree, overwriting the prior slot (GYMAP-67). */
    private fun writeZipToFolder(folderUri: Uri, snap: File) {
        val tree = DocumentFile.fromTreeUri(context, folderUri) ?: return
        // Write the replacement under a temp name FIRST, then retire the old one. Deleting the
        // previous backup before creating its replacement (the old order) meant any failure below
        // left the folder with NO backup: createFile returning null on a revoked grant,
        // openOutputStream returning null, or the volume filling mid-write. The caller wraps this
        // in runCatching and clears the "backup failed" marker regardless, so the user was told
        // the backup succeeded while their off-device copy had just been deleted.
        tree.findFile(FOLDER_TMP_NAME)?.delete()
        val tmp = tree.createFile("application/zip", FOLDER_TMP_NAME) ?: return
        val wrote = runCatching {
            val out = context.contentResolver.openOutputStream(tmp.uri) ?: return@runCatching false
            out.use { writeBackupZip(it, snap) }
            true
        }.getOrDefault(false)
        if (!wrote) { tmp.delete(); return }
        // The replacement is complete on disk: only now retire the previous backup and take its
        // name. If the rename fails the data is still present under the temp name, so leave it
        // rather than deleting the only copy in this folder.
        tree.findFile(AUTO_BACKUP_NAME)?.delete()
        tmp.renameTo(AUTO_BACKUP_NAME)
    }

    /** Take a persistable read+write grant on the picked backup folder and remember it (GYMAP-67). */
    suspend fun rememberBackupFolder(treeUri: Uri) = withContext(Dispatchers.IO) {
        runCatching {
            context.contentResolver.takePersistableUriPermission(
                treeUri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            )
        }
        settingsRepo.setBackupFolderUri(treeUri.toString())
    }

    /** Stop mirroring backups to a folder (its persisted grant is dropped by the OS in time). */
    suspend fun forgetBackupFolder() = settingsRepo.setBackupFolderUri(null)

    /** When the auto-backup slot was last written, or null if none exists yet (#86 restore affordance). */
    fun autoBackupSavedAtMs(): Long? =
        File(context.filesDir, AUTO_BACKUP_NAME).takeIf { it.exists() }?.lastModified()

    /**
     * The weekly auto-backup worker exhausted its retries (e.g. storage full): record it so Settings can
     * surface a "last backup failed" notice rather than the user silently losing their periodic backup.
     * Cleared automatically by the next successful [autoBackup].
     */
    fun recordAutoBackupFailure() {
        runCatching { File(context.filesDir, AUTO_BACKUP_FAILED_MARKER).writeText("1") }
    }

    /** True when the most recent auto-backup attempt failed and none has succeeded since. */
    fun autoBackupFailed(): Boolean = File(context.filesDir, AUTO_BACKUP_FAILED_MARKER).exists()

    /** True once any restorable backup exists — the silent weekly auto-slot OR a manual export (#5 P1). */
    fun hasAnyBackup(): Boolean =
        autoBackupSavedAtMs() != null || File(context.filesDir, MANUAL_BACKUP_MARKER).exists()

    /** Nudge the user to back up when they have data worth protecting but no backup exists yet. */
    suspend fun shouldWarnNoBackup(): Boolean = !hasAnyBackup() && sessionDao.finishedCount() > 0

    /** Plain-English summary of the live data a restore would REPLACE — shown in the confirm dialog. */
    suspend fun restoreImpactSummary(): String {
        val sessions = sessionDao.finishedCount()
        val photos = runCatching { photoRepo.photos().size }.getOrDefault(0)
        return buildList {
            add("$sessions session${if (sessions == 1) "" else "s"}")
            if (photos > 0) add("$photos progress photo${if (photos == 1) "" else "s"}")
        }.joinToString(" · ")
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
                writeBackupZip(out, snap)
            } ?: error("Could not open the chosen destination")
        } finally {
            snap.delete()
        }
        // Only reached on success: a fresh manual backup clears any stale "auto-backup failed" notice
        // (the user now has a recent backup, which is exactly what that warning asks them to make).
        File(context.filesDir, AUTO_BACKUP_FAILED_MARKER).delete()
        // Record that the user has made a real, user-controlled backup — drives the "no backup yet"
        // nudge ([shouldWarnNoBackup]). The silent weekly auto-slot also counts (see [hasAnyBackup]).
        runCatching { File(context.filesDir, MANUAL_BACKUP_MARKER).writeText("1") }
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
            // The profile avatar (single app-private file) — folded in like the photos. exists() is
            // length-checked, so a stray zero-byte avatar.jpg can't be zipped and later overwrite a good one.
            if (avatarRepo.exists()) {
                zip.putNextEntry(java.util.zip.ZipEntry(ZIP_AVATAR_ENTRY))
                avatarRepo.file.inputStream().use { it.copyTo(zip) }
                zip.closeEntry()
            }
        }
    }

    /** Stream [input] into [dest], returning false (and stopping) once it exceeds [maxBytes] (E3). */
    private fun copyAtMost(input: java.io.InputStream, dest: File, maxBytes: Long): Boolean {
        var total = 0L
        dest.outputStream().use { out ->
            val buf = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                total += n
                if (total > maxBytes) return false
                out.write(buf, 0, n)
            }
        }
        return true
    }

    /**
     * Replace the live database with the backup at [uri]. Validates it's a real Avex DB
     * first; only then stages the swap. Returns a [RestoreOutcome] — on SUCCESS the caller
     * MUST restart the app afterward (the file is swapped at next boot).
     */
    suspend fun restoreFromUri(uri: Uri): RestoreOutcome = withContext(Dispatchers.IO) {
        val incoming = File(context.cacheDir, "forge_restore_in_${System.currentTimeMillis()}")
        if (incoming.exists()) incoming.delete()
        val copied = try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                copyAtMost(input, incoming, MAX_RESTORE_BYTES)
            } ?: return@withContext RestoreOutcome.IO_ERROR
        } catch (e: java.io.IOException) {
            // A read/write failure mid-copy (unreadable source, disk full) must not leave the partial
            // staging temp behind — restoreFromIncoming, which cleans up, is never reached on this path.
            incoming.delete()
            return@withContext RestoreOutcome.IO_ERROR
        }
        if (!copied) { incoming.delete(); return@withContext RestoreOutcome.TOO_LARGE }
        restoreFromIncoming(incoming)
    }

    /**
     * Restore from the weekly auto-backup slot (#86) — the same validation + staging as a user-picked
     * file, so this is the in-app recovery path for the otherwise write-only auto-backup. Copies the
     * slot to a cache temp first so [restoreFromIncoming]'s cleanup never deletes the live slot itself.
     */
    suspend fun restoreFromAutoBackup(): RestoreOutcome = withContext(Dispatchers.IO) {
        val auto = File(context.filesDir, AUTO_BACKUP_NAME)
        if (!auto.exists()) return@withContext RestoreOutcome.NO_BACKUP_FILE
        val incoming = File(context.cacheDir, "forge_restore_in_${System.currentTimeMillis()}")
        if (incoming.exists()) incoming.delete()
        // Cap + clean up like restoreFromUri (E3): a corrupt/oversized slot can't fill the cache here either.
        val copied = try {
            auto.inputStream().use { copyAtMost(it, incoming, MAX_RESTORE_BYTES) }
        } catch (e: java.io.IOException) {
            incoming.delete()
            return@withContext RestoreOutcome.IO_ERROR
        }
        if (!copied) { incoming.delete(); return@withContext RestoreOutcome.TOO_LARGE }
        restoreFromIncoming(incoming)
    }

    /**
     * Validate [incoming] and, if it's a real Avex backup, STAGE the DB, prefs and progress photos
     * as pending files that [com.forge.app.ForgeApp.applyPendingRestore] swaps in atomically at next
     * boot — DB, prefs and photos together, so a kill or copy failure can never leave the live DB and
     * photo folder from different backups. Returns a [RestoreOutcome]. Deletes [incoming].
     */
    private suspend fun restoreFromIncoming(incoming: File): RestoreOutcome = withContext(Dispatchers.IO) {
        val temps = mutableListOf(incoming) // cache-dir temp files to clean up before returning
        // Set true only once EVERY staged component has landed. The finally below uses it to discard
        // a half-written pending set, which would otherwise be applied at the next cold start.
        var stagedOk = false
        var photoStage: File? = null        // extracted progress photos, staged only after validation
        var avatarStage: File? = null       // extracted avatar temp (in temps), applied after validation
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
                            name == ZIP_AVATAR_ENTRY -> {
                                val a = File(context.cacheDir, "forge_restore_avatar_${System.currentTimeMillis()}.jpg")
                                    .also { it.delete(); temps.add(it) }
                                a.outputStream().use { zin.copyTo(it) }
                                avatarStage = a
                            }
                        }
                        zin.closeEntry()
                        entry = zin.nextEntry
                    }
                }
                if (!sawDb) return@withContext RestoreOutcome.NOT_A_BACKUP
                dbFile = exDb
            }

            if (!isForgeDatabase(dbFile)) {
                // A file that carries the SQLite header but lacks our schema is a damaged/incomplete DB
                // (e.g. a backup truncated in transfer) — report CORRUPT so the user re-exports rather
                // than hunting for "the .zip". A file without the header was never a backup at all.
                return@withContext if (isSqlite(dbFile)) RestoreOutcome.CORRUPT else RestoreOutcome.NOT_A_BACKUP
            }
            // Reject a backup newer than this build's schema: Room has no downgrade path and would
            // crash on open. Older versions migrate forward normally, so only a strictly-newer
            // user_version is rejected. (We stage rather than swap live, so nothing is lost on reject.)
            //
            // Also reject anything below the migration floor (<12, the pre-lock versions): there is no
            // migration path for those, so Room's fallbackToDestructiveMigrationFrom(1..11) would DROP
            // ALL TABLES on open — restoring such a backup would silently wipe it. Refuse instead.
            val currentVersion = db.openHelper.readableDatabase.version
            val incomingVersion = databaseUserVersion(dbFile)
            if (incomingVersion > currentVersion) return@withContext RestoreOutcome.NEWER_VERSION
            if (incomingVersion < MIN_RESTORABLE_VERSION) return@withContext RestoreOutcome.TOO_OLD

            // Don't close Room and swap the file here — that races with any flow still reading the DB
            // until the process is killed. Stage the files instead; ForgeApp.applyPendingRestore swaps
            // them in at next boot, before Room/DataStore open. The caller restarts the app on success.
            val pendingDb = File(context.filesDir, PENDING_DB_NAME)
            if (pendingDb.exists()) pendingDb.delete()
            dbFile.copyTo(pendingDb, overwrite = true)

            val pendingPrefs = File(context.filesDir, PENDING_PREFS_NAME)
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
            // Avatar staged the same way — swapped in at boot alongside the DB/prefs/photos.
            avatarStage?.let { a ->
                val pendingAvatar = File(context.filesDir, PENDING_AVATAR_NAME)
                if (pendingAvatar.exists()) pendingAvatar.delete()
                a.copyTo(pendingAvatar, overwrite = true)
            }

            // The restored dataset is a different set of data — it hasn't been independently backed up
            // from this install, so drop the "user has backed up" latch. Otherwise the no-backup nudge
            // ([shouldWarnNoBackup]) would stay permanently suppressed after restoring an old backup,
            // leaving newly-accumulated data unprotected with no warning (#5 P1). The auto-backup slot,
            // if present, still counts toward [hasAnyBackup].
            runCatching { File(context.filesDir, MANUAL_BACKUP_MARKER).delete() }

            // Every staged component landed. Only now is the pending set complete, and only now may
            // ForgeApp.applyPendingRestore swap it in at boot.
            stagedOk = true
            return@withContext RestoreOutcome.SUCCESS
        } catch (e: java.util.zip.ZipException) {
            // A truncated or malformed ZIP (e.g. a backup that got corrupted in an email / cloud
            // transfer) fails cleanly with a distinct reason instead of escaping the restore path.
            return@withContext RestoreOutcome.CORRUPT
        } catch (e: java.io.IOException) {
            // Any other read/copy failure mid-restore (unreadable entry, disk full).
            return@withContext RestoreOutcome.IO_ERROR
        } finally {
            temps.forEach { it.delete() }
            photoStage?.deleteRecursively()
            // A restore that did not reach SUCCESS must leave NOTHING staged. `temps` only covers
            // cacheDir scratch files — the pending_restore.* set lives in filesDir and used to
            // survive a failure. ForgeApp.applyPendingRestore checks only that those files EXIST,
            // so a half-written set was swapped over the live database at the next cold start,
            // silently discarding everything logged since the failed attempt.
            if (!stagedOk) clearPendingRestore()
        }
    }

    /**
     * Remove every staged component of a pending restore.
     *
     * Called when [restoreFromIncoming] ends in anything other than SUCCESS. Clearing a previously
     * staged (successful but not-yet-rebooted) restore is deliberate: the user's most recent
     * instruction was the attempt that just failed, so applying the older one at the next boot —
     * after being told the restore failed — is the confusing outcome this guards against.
     */
    private fun clearPendingRestore() {
        runCatching { File(context.filesDir, PENDING_DB_NAME).delete() }
        runCatching { File(context.filesDir, PENDING_PREFS_NAME).delete() }
        runCatching { File(context.filesDir, PENDING_PHOTOS_DIR).deleteRecursively() }
        runCatching { File(context.filesDir, PENDING_AVATAR_NAME).delete() }
    }

    /**
     * Sanity check that a candidate file is a SQLite DB containing Avex's core tables.
     * Checking only `session` let any SQLite DB from another app pass validation and get swapped in.
     * We now require all three tables that every real Avex backup must contain — so picking the wrong
     * app's DB fails with NOT_A_BACKUP / CORRUPT instead of silently replacing your data.
     */
    private fun isForgeDatabase(file: File): Boolean = runCatching {
        android.database.sqlite.SQLiteDatabase.openDatabase(
            file.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READONLY
        ).use { dbFile ->
            dbFile.rawQuery(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN " +
                    "('session','logged_exercise','logged_set')", null
            ).use { c -> c.moveToFirst() && c.getInt(0) >= 3 }
        }
    }.getOrDefault(false)

    /** The SQLite user_version (Room schema version) of a candidate DB file; MAX if unreadable (→ rejected). */
    private fun databaseUserVersion(file: File): Int = runCatching {
        android.database.sqlite.SQLiteDatabase.openDatabase(
            file.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READONLY
        ).use { it.version }
    }.getOrDefault(Int.MAX_VALUE)

    /**
     * Read the most recent crash log files from filesDir/crashes in-app (newest-first).
     * Returns a list of (filename, fileText) pairs, capped at [limit] entries.
     * Each file's text is truncated to 20 KB so the UI string stays sane.
     */
    suspend fun readRecentCrashLogs(limit: Int = 10): List<Pair<String, String>> = withContext(Dispatchers.IO) {
        val dir = File(context.filesDir, "crashes")
        val files = dir.listFiles()
            ?.filter { it.isFile }
            ?.sortedByDescending { it.lastModified() }
            ?.take(limit)
            ?: emptyList()
        files.map { f ->
            val text = runCatching {
                f.inputStream().use { ins ->
                    val bytes = ins.readBytes()
                    val cap = 20 * 1024 // 20 KB
                    if (bytes.size > cap) {
                        String(bytes, 0, cap, Charsets.UTF_8) + "\n… [truncated]"
                    } else {
                        String(bytes, Charsets.UTF_8)
                    }
                }
            }.getOrElse { "Could not read file: ${it.message}" }
            f.name to text
        }
    }

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

    /** True if [file] starts with the SQLite format-3 magic (so a non-zip candidate is at least a DB). */
    private fun isSqlite(file: File): Boolean = runCatching {
        val magic = "SQLite format 3".toByteArray(Charsets.US_ASCII) + 0.toByte()
        file.inputStream().use { ins ->
            val buf = ByteArray(magic.size)
            ins.read(buf) == magic.size && buf.contentEquals(magic)
        }
    }.getOrDefault(false)

    /** True if [file] starts with the ZIP local-file-header magic (PK signature). */
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
        /** Temp slots written before replacing [AUTO_BACKUP_NAME], so a failed write never destroys
         *  the previous good backup. Internal storage and the user-picked SAF folder each need one. */
        private const val AUTO_BACKUP_TMP_NAME = "forge_auto_backup.zip.tmp"
        private const val FOLDER_TMP_NAME = "forge_auto_backup.zip.part"
        /** Marker written when the auto-backup worker gives up (storage full / corrupt) — see [recordAutoBackupFailure]. */
        private const val AUTO_BACKUP_FAILED_MARKER = "auto_backup_failed"
        /** Marker written after a successful user-initiated backup ([backupToUri]) — see [hasAnyBackup]. */
        private const val MANUAL_BACKUP_MARKER = "manual_backup_done"
        /** Staged restored photos; ForgeApp.applyPendingRestore swaps this over progress_photos/ at boot. */
        private const val PENDING_PHOTOS_DIR = "pending_restore_photos"
        // Staged-restore filenames. Must stay in sync with ForgeApp.applyPendingRestore, which
        // reads the same four paths out of filesDir at boot.
        private const val PENDING_DB_NAME = "pending_restore.db"
        private const val PENDING_PREFS_NAME = "pending_restore_prefs.pb"
        /** Backup-ZIP entry for the profile avatar — derived from AvatarRepository so a rename can't desync. */
        private const val ZIP_AVATAR_ENTRY = AvatarRepository.FILE_NAME
        private const val PENDING_AVATAR_NAME = "pending_restore_avatar.jpg"
        /**
         * Lowest schema version restore will accept. Below this are the pre-lock versions Room
         * destructively resets (DatabaseModule.fallbackToDestructiveMigrationFrom(1..11)), so
         * restoring one would wipe rather than recover. Must stay one above that destructive range.
         */
        private const val MIN_RESTORABLE_VERSION = 12

        /** Cap on the staged restore copy — guards a budget device's cache against an oversized file (E3). */
        private const val MAX_RESTORE_BYTES = 2L * 1024 * 1024 * 1024 // 2 GiB
    }
}
