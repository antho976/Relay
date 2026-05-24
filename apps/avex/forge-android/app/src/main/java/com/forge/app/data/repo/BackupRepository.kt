package com.forge.app.data.repo

import android.content.Context
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
    private val settingsRepo: SettingsRepository
) {

    private val zone = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    /** Export this week's data as JSON for AI analysis (#5). Returns the file path. */
    suspend fun exportWeeklyJson(): File {
        val weekStartMs = System.currentTimeMillis() - 7L * 24 * 3600 * 1000
        val sessions = sessionDao.finishedInRange(weekStartMs, System.currentTimeMillis())
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

    /** List all backup files in app-private dir. */
    fun listBackupFiles(): List<File> =
        context.filesDir.listFiles { f -> f.name.startsWith("forge_") && f.name.endsWith(".json") }
            ?.sortedByDescending { it.lastModified() } ?: emptyList()
}
