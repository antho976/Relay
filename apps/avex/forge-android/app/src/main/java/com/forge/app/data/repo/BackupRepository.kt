package com.forge.app.data.repo

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.documentfile.provider.DocumentFile
import com.forge.app.RestoreManifest
import com.forge.app.core.io.exportFile
import com.forge.app.core.time.mondayStartMs
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.forgeDatabaseBuilder
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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
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
    private val db: ForgeDatabase,
    private val clock: com.forge.app.core.time.Clock
) {

    /**
     * Resolved per read, not captured once.
     *
     * This is a @Singleton, so a `val` snapshotted the device's zone for the whole process lifetime.
     * Fly Auckland → London with the process alive and every export taken afterwards stamped its
     * `date` column with NEW ZEALAND calendar days while the UI showed London ones — the file and
     * the app disagreeing about which day each session happened on.
     */
    private val zone: ZoneId get() = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    /** The outcome of a restore attempt — distinct reasons so the UI can explain a failure (E6). */
    enum class RestoreOutcome { SUCCESS, NOT_A_BACKUP, NEWER_VERSION, TOO_OLD, CORRUPT, TOO_LARGE, IO_ERROR, NO_BACKUP_FILE }

    /**
     * Write [value] under [key], or leave the key out entirely when it is null.
     *
     * A nullable NUMBER used to be written as the empty string ("elevationM": ""), which makes the
     * same field a number on one row and a string on the next — any reader using optDouble/optLong
     * on the empty form silently gets the default instead of a signal that the value was absent.
     * Omission is the JSON way to say "not recorded": our own importer already reads every one of
     * these through opt*(key, default), so a missing key lands on exactly the same default, and an
     * outside reader sees one type per field.
     *
     * Deliberately not JSONObject.NULL: org.json renders that back through optString as the literal
     * text "null", which would be worse than the empty string it replaced.
     */
    private fun JSONObject.putOrOmit(key: String, value: Any?) {
        if (value != null) put(key, value)
    }

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
    suspend fun exportWeeklyJson(): File = withContext(Dispatchers.IO) {
        val nowMs = clock.nowMs()
        // The ISO week the app itself calls "this week" everywhere else, not a rolling 7 x 24 h from
        // whenever Export was tapped — otherwise the file and the Stats screen describe different
        // sets of sessions under the same heading.
        val weekStartMs = mondayStartMs(nowMs, zone)
        // Window on finish time so a session that started before the boundary but finished this
        // week is still included in the export.
        val sessions = sessionDao.finishedByFinishTimeInRange(weekStartMs, nowMs)
        val cardioEntries = cardioDao.since(weekStartMs)

        val root = JSONObject().apply {
            put("exportedAt", dateFmt.format(Instant.now().atZone(zone)))
            // The current ISO week, so the numbers agree with everything the app calls "this week".
            put("periodStart", dateFmt.format(java.time.Instant.ofEpochMilli(weekStartMs).atZone(zone)))
            put("periodDays", 7)
            val sessArr = JSONArray()
            sessions.forEach { s ->
                val exercises = loggedExerciseDao.forSession(s.id)
                val sObj = JSONObject().apply {
                    put("id", s.id)
                    put("dayKey", s.dayKey)
                    put("date", dateFmt.format(Instant.ofEpochMilli(s.startedAt).atZone(zone)))
                    // The exact instant, alongside the human date. Without it a weekly export
                    // re-read at local midnight described a DIFFERENT session start from the full
                    // export's, so importing both files inserted every recent workout twice.
                    put("startedAt", s.startedAt)
                    put("finishedAt", s.finishedAt ?: 0)
                    put("activeMin", activeMinutesOf(s))
                    put("activeSeconds", activeSecondsOf(s))
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
                            // The DISPLAY name, the same resolution exportSessionJson uses. Falling
                            // back to the raw id wrote "ua1" as the human name for the seed-split
                            // ids, which resolve only on the display path and are deliberately not
                            // in ExerciseLibrary: re-importing the file matched nothing, so years of
                            // bench-press history came back as a movement called "Ua1", de-linked
                            // from its own stats. The AI reading this file saw the id too.
                            put("name", com.forge.app.program.Program.exerciseDisplayName(ex.exerciseId, ex.swappedName))
                            put("effort", ex.difficulty?.name ?: "")
                            put("note", ex.note ?: "")
                            put("skipped", ex.skipped)
                            val setArr = JSONArray()
                            sets.forEach { set ->
                                setArr.put(JSONObject().apply {
                                    putOrOmit("weightLb", set.weightLb)
                                    put("reps", set.reps)
                                    putOrOmit("rpe", set.rpe)
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
                    putOrOmit("distanceKm", c.distanceKm)
                    put("effort", c.effort ?: "")
                })
            }
            put("cardio", cardioArr)
        }

        // Fixed filename (overwrite) so repeated exports don't accumulate forever (#84).
        val file = exportFile(context, "avex_weekly_export.json")
        file.writeText(root.toString(2))
        file
    }

    /**
     * Full data dump as JSON — every session + exercise + set + cardio entry + a snapshot of key
     * settings. This is a *lossy, human/AI-readable export*, NOT a restore source: nothing reads it
     * back in. The real restore path is the whole-DB backup ([backupToUri] / [restoreFromUri]).
     * Named so it doesn't imply recoverability (#70).
     */
    suspend fun exportFullDataJson(): File = withContext(Dispatchers.IO) {
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
                                    putOrOmit("weightLb", set.weightLb)
                                    put("reps", set.reps)
                                    putOrOmit("rpe", set.rpe)
                                    put("completedAt", set.completedAt)
                                    put("difficultyTag", set.difficultyTag ?: "")
                                    // These change what the set MEANS, so an export without them is
                                    // not the same training history: a timed hold's reps is not a
                                    // count, and an assisted set is not PR-eligible. Re-importing an
                                    // export that omitted them turned a 90 s weighted plank into a
                                    // 90-rep 45 lb set at the top of the Hall of Fame.
                                    put("durationSeconds", set.durationSeconds ?: 0)
                                    put("isAssisted", set.isAssisted)
                                    put("isAmrap", set.isAmrap)
                                    put("toFailure", set.toFailure)
                                    put("setType", set.setType ?: "")
                                    put("dropAnnotation", set.dropAnnotation ?: "")
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
                    putOrOmit("distanceKm", c.distanceKm)
                    put("effort", c.effort ?: "")
                    put("restReason", c.restReason ?: "")
                    put("note", c.note ?: "")
                    // Per-type fields (GYMAP-38); elevation stays canonical metres like distance is km.
                    putOrOmit("inclinePct", c.inclinePct)
                    putOrOmit("laps", c.laps)
                    putOrOmit("elevationM", c.elevationM)
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
                    putOrOmit("targetValue", g.targetValue)
                    put("priority", g.priority)
                    put("createdAt", g.createdAt)
                    putOrOmit("completedAt", g.completedAt)
                    putOrOmit("archivedAt", g.archivedAt)
                    put("source", g.source)
                    put("note", g.note)
                })
            }
            put("coachGoals", goalsArr)
        }

        // Fixed filename (overwrite) — see #84; avoids unbounded accumulation in filesDir.
        val file = exportFile(context, "avex_export.json")
        file.writeText(root.toString(2))
        file
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
                                putOrOmit("weightLb", set.weightLb)
                                put("reps", set.reps)
                                putOrOmit("rpe", set.rpe)
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
        val file = exportFile(context, "avex_session_${s.id}.json")
        file.writeText(root.toString(2))
        file
    }

    /** RFC 4180 CSV field: quote and double embedded quotes when the value holds a comma/quote/newline. */
    private fun csv(value: String): String =
        if (value.any { it == ',' || it == '"' || it == '\n' || it == '\r' })
            "\"" + value.replace("\"", "\"\"") + "\""
        else value

    /** Export as CSV — sessions summary (#138). */
    suspend fun exportSessionsCsv(): File = withContext(Dispatchers.IO) {
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
        val file = exportFile(context, "avex_sessions.csv")
        file.writeText(sb.toString())
        file
    }

    /**
     * All-time best lift per exercise (the "hall of fame") as CSV — PRs are the most personally
     * motivating records to share or keep (#542). One row per exercise: its heaviest tracked set ever.
     * Reuses the same tracked/non-assisted set projection the Stats hall of fame is built from.
     */
    suspend fun exportPrsCsv(): File = withContext(Dispatchers.IO) {
        val sets = loggedSetDao.observeAllFinishedSetsWithSession().first()
        val sb = StringBuilder()
        sb.appendLine("exercise,muscle,bestWeightLb,reps,date")
        // Reuse the exact aggregation the Stats hall of fame renders from, so the exported PRs can
        // never diverge from what the user sees in-app (one place to fix tie-breaking / attribution).
        buildHallOfFame(sets).forEach { pr ->
            val date = dateFmt.format(Instant.ofEpochMilli(pr.sessionDate).atZone(zone))
            sb.appendLine("${csv(pr.exerciseName)},${csv(pr.muscle.displayName)},${pr.maxWeightLb},${pr.bestReps},$date")
        }
        val file = exportFile(context, "avex_prs.csv")
        file.writeText(sb.toString())
        file
    }

    /** Every bodyweight weigh-in as CSV (Cat 11). One row per entry, newest first. */
    suspend fun exportBodyweightCsv(): File = withContext(Dispatchers.IO) {
        val entries = db.bodyweightDao().all()
        val sb = StringBuilder()
        sb.appendLine("date,weightLb")
        entries.forEach { e -> sb.appendLine("${e.dateKey},${e.weightLb}") }
        val file = exportFile(context, "avex_bodyweight.csv")
        file.writeText(sb.toString())
        file
    }

    /**
     * Every cardio entry as CSV (GYMAP-43). One row per entry, newest first. The type is resolved to
     * its display name (custom activities included, GYMAP-37) so the sheet reads as words, not raw
     * `custom_…` codes; distance stays canonical km like the other exports' canonical-unit columns.
     */
    suspend fun exportCardioCsv(): File = withContext(Dispatchers.IO) {
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
        val file = exportFile(context, "avex_cardio.csv")
        file.writeText(sb.toString())
        file
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

    /**
     * Take a persistable read+write grant on the picked backup folder and remember it (GYMAP-67).
     *
     * A folder being REPLACED has its own grant released afterwards (M-18): a persisted grant lives
     * until it is revoked or released, so choosing folder B while A was connected used to leave
     * Avex holding A across reboots — read AND write access to a tree the user could no longer see
     * anywhere in the app. Replaced destinations accumulated one grant each.
     *
     * The new grant is taken FIRST and the old one released only after the preference has moved, so
     * a failure anywhere in between leaves the app with access to the folder it is pointing at
     * rather than to neither.
     */
    suspend fun rememberBackupFolder(treeUri: Uri) = withContext(Dispatchers.IO) {
        val previous = settingsRepo.backupFolderUri.first()
        runCatching {
            context.contentResolver.takePersistableUriPermission(
                treeUri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            )
        }
        settingsRepo.setBackupFolderUri(treeUri.toString())
        if (previous != null && previous != treeUri.toString()) releasePersistedTree(previous)
        Unit
    }

    /**
     * Stop mirroring backups to a folder, and give up the access that went with it (M-18).
     *
     * Clearing the preference alone left the persisted grant in place: "Remove folder" reported
     * that Avex was no longer connected to the tree while it kept read and write access to it
     * indefinitely, surviving reboots. The preference is cleared first — that is what the user
     * asked for and it must not depend on the release succeeding.
     */
    suspend fun forgetBackupFolder() = withContext(Dispatchers.IO) {
        val previous = settingsRepo.backupFolderUri.first()
        settingsRepo.setBackupFolderUri(null)
        previous?.let { releasePersistedTree(it) }
    }

    /**
     * Release the persisted grant on [uriString], with exactly the flags this app is actually
     * holding on it — releasing flags that were never taken throws, and a tree taken read-only by
     * an older build would otherwise fail the whole release. Unparseable, already-released and
     * never-held uris are all no-ops.
     */
    private suspend fun releasePersistedTree(uriString: String) {
        // The import folder is a separate setting that can name the SAME tree (Downloads, most
        // obviously) under its own read grant. Releasing here would take that folder's scan with it,
        // so a tree the importer is still pointing at is left alone; it has its own owner.
        if (settingsRepo.importFolderUri.first() == uriString) return
        runCatching {
            val uri = Uri.parse(uriString)
            val held = context.contentResolver.persistedUriPermissions
                .firstOrNull { it.uri == uri } ?: return
            var flags = 0
            if (held.isReadPermission) flags = flags or Intent.FLAG_GRANT_READ_URI_PERMISSION
            if (held.isWritePermission) flags = flags or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            if (flags != 0) context.contentResolver.releasePersistableUriPermission(uri, flags)
        }
    }

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
        sweepStaleTemps()
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
            // Progress photos live as files outside the DB (ProgressPhotoRepository) — fold the
            // library in so a restore brings the physique photos back too (#138). Flat:
            // progress_photos/<name>. Only what the index names, plus the index itself: zipping the
            // folder's whole listing carried every orphaned image an interrupted import had left
            // behind, hidden from the gallery, into every archive.
            val library = photoRepo.backupSnapshot()
            library.metadata.forEach { (name, bytes) ->
                zip.putNextEntry(java.util.zip.ZipEntry("$PHOTOS_PREFIX$name"))
                zip.write(bytes)
                zip.closeEntry()
            }
            library.photos.forEach { f ->
                zip.putNextEntry(java.util.zip.ZipEntry("$PHOTOS_PREFIX${f.name}"))
                f.inputStream().use { it.copyTo(zip) }
                zip.closeEntry()
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

    /**
     * Stream [input] into [dest], stopping once it exceeds [maxBytes] (E3).
     *
     * Returns the number of bytes written, or -1 when the cap was hit. The count is what lets the
     * caller enforce an ARCHIVE-wide budget on top of the per-entry one — see [ExtractionBudget].
     */
    private fun copyAtMost(input: java.io.InputStream, dest: File, maxBytes: Long): Long {
        var total = 0L
        dest.outputStream().use { out ->
            val buf = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                total += n
                if (total > maxBytes) return -1L
                out.write(buf, 0, n)
            }
        }
        return total
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
                copyAtMost(input, incoming, MAX_RESTORE_BYTES) >= 0
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
            auto.inputStream().use { copyAtMost(it, incoming, MAX_RESTORE_BYTES) >= 0 }
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
        sweepStaleTemps(keep = incoming)
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
                // Every extraction below is bounded. The incoming ARCHIVE is capped by
                // MAX_RESTORE_BYTES, but nothing capped what an entry expanded to, so a small
                // high-ratio zip could write tens of GB into internal storage mid-restore and fill
                // the device. An entry over its cap fails the whole restore rather than being
                // silently truncated into a "valid-looking" half file.
                var oversized = false
                // One budget for the WHOLE archive, on top of the per-entry caps below. See
                // [ExtractionBudget] for why the per-entry cap alone bounds nothing that matters.
                val budget = ExtractionBudget(MAX_RESTORE_TOTAL_BYTES, MAX_RESTORE_PHOTOS)
                java.util.zip.ZipInputStream(incoming.inputStream()).use { zin ->
                    var entry = zin.nextEntry
                    while (entry != null && !oversized) {
                        val name = entry.name
                        when {
                            name == ZIP_DB_ENTRY -> {
                                val written = copyAtMost(zin, exDb, MAX_RESTORE_ENTRY_BYTES)
                                if (budget.spend(written)) sawDb = true else oversized = true
                            }
                            name == ZIP_PREFS_ENTRY -> {
                                val exPrefs = File(context.cacheDir, "forge_restore_prefs_${System.currentTimeMillis()}.pb")
                                    .also { it.delete(); temps.add(it) }
                                val written = copyAtMost(zin, exPrefs, MAX_RESTORE_PREFS_BYTES)
                                if (budget.spend(written)) prefsFile = exPrefs else oversized = true
                            }
                            name.startsWith(PHOTOS_PREFIX) -> {
                                val photoName = name.removePrefix(PHOTOS_PREFIX)
                                // Flat basename only — zip-slip guard against path-traversal entries.
                                if (photoName.isNotBlank() && !photoName.contains('/') && !photoName.contains('\\')) {
                                    if (!budget.countPhoto()) {
                                        oversized = true
                                    } else {
                                        val stage = photoStage ?: File(
                                            context.cacheDir, "forge_restore_photos_${System.currentTimeMillis()}"
                                        ).also { it.deleteRecursively(); it.mkdirs(); photoStage = it }
                                        val written = copyAtMost(zin, File(stage, photoName), MAX_RESTORE_MEDIA_BYTES)
                                        if (!budget.spend(written)) oversized = true
                                    }
                                }
                            }
                            name == ZIP_AVATAR_ENTRY -> {
                                val a = File(context.cacheDir, "forge_restore_avatar_${System.currentTimeMillis()}.jpg")
                                    .also { it.delete(); temps.add(it) }
                                val written = copyAtMost(zin, a, MAX_RESTORE_MEDIA_BYTES)
                                if (budget.spend(written)) avatarStage = a else oversized = true
                            }
                        }
                        zin.closeEntry()
                        entry = zin.nextEntry
                    }
                }
                if (oversized) {
                    // Nothing half-extracted survives the refusal — the staged photo directory is
                    // the only component that can already hold hundreds of megabytes at this point,
                    // and `temps` covers the rest in the finally below.
                    photoStage?.deleteRecursively()
                    photoStage = null
                    return@withContext RestoreOutcome.TOO_LARGE
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

            // Everything above read the header, three table NAMES, the pages and a version number.
            // A same-version SQLite file with three empty tables of the right names passed all of
            // it, replaced the live database at boot, and only then failed Room's own validation,
            // with the pre-restore snapshot already gone. So the staged file is now opened the way
            // the app will open it: the production builder, its migrations, Room's identity check
            // and a read. Refused here, a bad file costs nothing; an old one is migrated forward.
            if (!opensWithRoom(pendingDb)) return@withContext RestoreOutcome.CORRUPT

            // The prefs half of a backup used to be staged with no validation at all, while the
            // database half gets a zip sniff, a SQLite magic-byte check, a schema check and two
            // version floors. Anything whose settings entry is not a Preferences protobuf — a
            // hand-built zip, a third-party tool, a future format — was swapped into the live
            // DataStore file at the next boot, before any UI exists. Read it back the way DataStore
            // will, and simply skip a blob that doesn't parse: the restore proceeds with the
            // database (the part that holds the training history) and the user keeps their current
            // settings, instead of the app failing to start.
            val pendingPrefs = File(context.filesDir, PENDING_PREFS_NAME)
            if (pendingPrefs.exists()) pendingPrefs.delete()
            prefsFile?.takeIf { isPreferencesBlob(it) }?.copyTo(pendingPrefs, overwrite = true)

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
            // ForgeApp.applyPendingRestore swap it in at boot — which it learns from the manifest,
            // published last and atomically. A process killed anywhere above leaves components
            // with no manifest, or ones that no longer match it, and the boot quarantines those
            // instead of renaming a truncated database live.
            if (!RestoreManifest.publish(context.filesDir)) return@withContext RestoreOutcome.IO_ERROR
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
     * Delete cache scratch left by a backup or restore that never got to run its `finally`.
     *
     * Every temp is named with `System.currentTimeMillis()`, so each attempt allocates a NEW file
     * rather than reusing a slot. A process killed mid-backup therefore leaves a full copy of the
     * database in `cacheDir` forever — and the kill that causes it is usually the low-storage one,
     * so three failed weekly backups on a full phone leave three DB-sized files making the next
     * attempt likelier to fail too. Android only reclaims `cacheDir` under system-wide pressure.
     *
     * Anything older than [TEMP_STALE_MS] cannot belong to a live operation; [keep] protects the
     * file the caller is working on right now.
     */
    private fun sweepStaleTemps(keep: File? = null) {
        runCatching {
            val cutoff = System.currentTimeMillis() - TEMP_STALE_MS
            context.cacheDir.listFiles()?.forEach { f ->
                val name = f.name
                if (f.absolutePath == keep?.absolutePath) return@forEach
                if (!TEMP_PREFIXES.any { name.startsWith(it) }) return@forEach
                if (f.lastModified() > cutoff) return@forEach
                if (f.isDirectory) f.deleteRecursively() else f.delete()
            }
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
        // Marker first: a clear cut short part-way must leave no READY beside a partial set.
        RestoreManifest.discard(context.filesDir)
        runCatching { File(context.filesDir, PENDING_DB_NAME).delete() }
        runCatching { File(context.filesDir, PENDING_PREFS_NAME).delete() }
        runCatching { File(context.filesDir, PENDING_PHOTOS_DIR).deleteRecursively() }
        runCatching { File(context.filesDir, PENDING_AVATAR_NAME).delete() }
    }

    /**
     * Sanity check that a candidate file is a SQLite DB containing Avex's core tables, and that the
     * rest of the file is actually readable.
     * Checking only `session` let any SQLite DB from another app pass validation and get swapped in.
     * We now require all three tables that every real Avex backup must contain — so picking the wrong
     * app's DB fails with NOT_A_BACKUP / CORRUPT instead of silently replacing your data.
     *
     * The table check reads page 1 and nothing else, so a backup whose DATA pages were damaged in
     * transit — emailed, synced through a flaky provider, copied off a failing SD card — passed it
     * with the schema page intact. It was then staged and swapped over the live database at boot, at
     * which point Room hit the bad page on the first read that touched it and its default
     * onCorruption handler DELETED the file. The original was already gone, so the user was left
     * with an empty schema and no way back. [quickCheck] reads every page before we commit to the
     * swap, which is the only point where refusing still costs nothing.
     */
    private fun isForgeDatabase(file: File): Boolean = runCatching {
        android.database.sqlite.SQLiteDatabase.openDatabase(
            file.path, null, android.database.sqlite.SQLiteDatabase.OPEN_READONLY
        ).use { dbFile ->
            val hasTables = dbFile.rawQuery(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN " +
                    "('session','logged_exercise','logged_set')", null
            ).use { c -> c.moveToFirst() && c.getInt(0) >= 3 }
            hasTables && quickCheck(dbFile)
        }
    }.getOrDefault(false)

    /**
     * Open a staged database through the production Room configuration, and hand back the file it
     * leaves behind.
     *
     * The file is copied to a probe name under `databases/`, opened with [forgeDatabaseBuilder]
     * (the same migrations, the same identity check as the app's own open), read once and closed.
     * Anything Room throws — a schema that only names our tables, a missing `room_master_table`, a
     * migration that cannot run, a page its error handler decides is corrupt — is a refusal. On
     * success the MIGRATED probe replaces the staged file, so the boot-time swap opens a database
     * already at this build's version and the pre-restore snapshot is released the moment that
     * open succeeds rather than after a migration on the main thread.
     *
     * The probe's WAL sidecars are removed after the close: a clean close checkpoints them into the
     * file, and the staged component is the single file the manifest will describe.
     */
    private fun opensWithRoom(staged: File): Boolean {
        val probe = context.getDatabasePath(PROBE_DB_NAME)
        val sidecars = listOf("-wal", "-shm", "-journal").map { File(probe.path + it) }
        fun dropProbe() {
            runCatching { probe.delete() }
            sidecars.forEach { runCatching { it.delete() } }
        }
        dropProbe()
        val opened = runCatching {
            probe.parentFile?.mkdirs()
            staged.copyTo(probe, overwrite = true)
            val room = forgeDatabaseBuilder(context, PROBE_DB_NAME).build()
            try {
                room.openHelper.writableDatabase.query("SELECT count(*) FROM session").use { it.moveToFirst() }
            } finally {
                room.close()
            }
        }.isSuccess
        sidecars.forEach { runCatching { it.delete() } }
        if (opened && probe.isFile) {
            // Same filesystem as filesDir, so this is a rename; the copy is the cross-volume fallback.
            staged.delete()
            if (!probe.renameTo(staged)) probe.copyTo(staged, overwrite = true)
        }
        dropProbe()
        return opened
    }

    /**
     * `PRAGMA quick_check` over the whole file — the per-page structural check, without
     * integrity_check's much slower cross-index verification. Returns false on anything but "ok",
     * and on a read that throws part-way through (which is itself corruption). A multi-year Avex
     * database is a few MB, so this is a one-off read of a file we are about to copy anyway.
     */
    private fun quickCheck(dbFile: android.database.sqlite.SQLiteDatabase): Boolean = runCatching {
        dbFile.rawQuery("PRAGMA quick_check(1)", null).use { c ->
            c.moveToFirst() && c.getString(0).equals("ok", ignoreCase = true)
        }
    }.getOrDefault(false)

    /**
     * True when [file] parses as the Preferences protobuf DataStore stores — the prefs-side
     * counterpart to [isForgeDatabase]. Read through DataStore itself over a throwaway copy, so
     * "this file reads" here means "this file reads" at boot.
     */
    private suspend fun isPreferencesBlob(file: File): Boolean {
        val probe = File(context.cacheDir, "restore_prefs_probe.preferences_pb")
        val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
        return try {
            runCatching {
                file.copyTo(probe, overwrite = true)
                // No corruptionHandler on purpose: an unreadable blob must THROW here, where the
                // restore can decline it, rather than be quietly replaced with defaults at boot.
                PreferenceDataStoreFactory.create(scope = scope) { probe }
                    .data.first()
                true
            }.getOrDefault(false)
        } finally {
            scope.cancel()
            probe.delete()
        }
    }

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
        /** Where [opensWithRoom] opens a candidate: a real `databases/` name, so Room needs no path. */
        private const val PROBE_DB_NAME = "forge_restore_probe.db"
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

        /**
         * Cap on the staged restore copy — guards a budget device's cache against an oversized file
         * (E3). Sized to the largest plausible Avex backup (a decade of training plus a full photo
         * gallery) rather than to "some big number": the old 2 GiB ceiling was written to disk IN
         * FULL before being rejected, which on a phone with 3 GB free is itself the outage.
         */
        private const val MAX_RESTORE_BYTES = 512L * 1024 * 1024 // 512 MiB

        /**
         * Cap on a single decompressed ZIP entry. `MAX_RESTORE_BYTES` bounds the archive as it
         * arrives; nothing bounded what it EXPANDED to. Zip compression of repetitive data runs
         * past 1000:1, so a 4 MB "backup" from a forum thread could write 40 GB into internal
         * storage during a restore and fill the device before the outer size check ever mattered.
         */
        private const val MAX_RESTORE_ENTRY_BYTES = 512L * 1024 * 1024 // 512 MiB

        /** Cap on one extracted progress photo / avatar — a JPEG, not a database. */
        private const val MAX_RESTORE_MEDIA_BYTES = 64L * 1024 * 1024 // 64 MiB

        /** Cap on the extracted preferences blob — a few KB of protobuf in practice. */
        private const val MAX_RESTORE_PREFS_BYTES = 8L * 1024 * 1024 // 8 MiB

        /**
         * Everything ONE restore may write to disk, across every entry. Generous next to any real
         * library — a thousand full-resolution photos is a few gigabytes — and finite, which is the
         * property the per-entry caps did not have between them.
         */
        private const val MAX_RESTORE_TOTAL_BYTES = 2L * 1024 * 1024 * 1024 // 2 GiB

        /** How many progress photos one archive may carry. Well past a decade of weekly shots. */
        private const val MAX_RESTORE_PHOTOS = 5_000

        /** Cache scratch this class creates, all timestamp-named. Swept by [sweepStaleTemps]. */
        private val TEMP_PREFIXES = listOf("forge_snapshot_", "forge_restore_")
        /** Nothing this old can still belong to a running backup or restore. */
        private const val TEMP_STALE_MS = 6L * 60 * 60 * 1000 // 6 hours
    }
}

/**
 * What one restore is allowed to write to disk in total, and how many photos it may carry.
 *
 * The per-entry cap alone bounds nothing that matters. Every recognised photo could expand to
 * `MAX_RESTORE_MEDIA_BYTES` and there was no limit on how many of them an archive held, so a small,
 * highly compressible ZIP full of distinct photo entries could write tens of gigabytes
 * into internal storage before anything objected — and the objection, when it came, would be the
 * device running out of space mid-restore, with the live database's room gone with it.
 *
 * Two numbers because they fail differently: the byte budget stops a compression bomb, and the count
 * stops an archive of many small entries whose real cost is inodes and time. Top-level and internal
 * so the rule can be tested without a ZIP, a Context or a database.
 */
internal class ExtractionBudget(private val maxTotalBytes: Long, private val maxPhotos: Int) {
    private var totalBytes = 0L
    private var photos = 0

    /** Bytes written so far across every entry. */
    val bytesSpent: Long get() = totalBytes

    /**
     * Record [bytes] written and report whether the archive is still within budget.
     *
     * A negative [bytes] is `copyAtMost`'s "this entry alone blew its own cap" — false either way,
     * and it deliberately does not add to the total, which would be meaningless.
     */
    fun spend(bytes: Long): Boolean {
        if (bytes < 0) return false
        totalBytes += bytes
        return totalBytes <= maxTotalBytes
    }

    /** Record one more photo entry; false once too many have been seen. */
    fun countPhoto(): Boolean = ++photos <= maxPhotos
}
