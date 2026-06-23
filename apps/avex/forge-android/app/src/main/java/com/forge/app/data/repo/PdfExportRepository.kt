package com.forge.app.data.repo

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.pdf.PdfDocument
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.cardioDetailParts
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.program.Program
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.first

/**
 * Generates a single-page PDF for a session (#149).
 * Uses Android's built-in PdfDocument — no external dependencies.
 */
@Singleton
class PdfExportRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val moodDao: com.forge.app.data.db.dao.MoodDao,
    private val cardioDao: CardioDao,
    private val bodyweightDao: BodyweightDao,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository
) {
    private val zone = ZoneId.systemDefault()
    private val dateFmt = DateTimeFormatter.ofPattern("MMMM d, yyyy")
    private val timeFmt = DateTimeFormatter.ofPattern("h:mm a")

    suspend fun exportLastSessionPdf(): File? {
        // Most recently finished by finish time — don't rely on the allFinished() list order.
        val lastSession = sessionDao.allFinished().maxByOrNull { it.finishedAt ?: it.startedAt } ?: return null
        return exportSessionPdf(lastSession.id)
    }

    suspend fun exportSessionPdf(sessionId: Long): File? {
        val session = sessionDao.get(sessionId) ?: return null
        val useKg = settingsRepo.useKg.first()
        val exercises = loggedExerciseDao.forSession(sessionId)
        val dayName = Program.dayDisplayName(session.dayKey)
        val dateStr = Instant.ofEpochMilli(session.startedAt).atZone(zone).format(dateFmt)
        val timeStr = Instant.ofEpochMilli(session.startedAt).atZone(zone).format(timeFmt)
        val durationMin = session.durationMinutes()
        val mood = moodDao.forSession(sessionId)?.mood

        val doc = PdfDocument()
        val pageInfo = PdfDocument.PageInfo.Builder(595, 842, 1).create() // A4
        val page = doc.startPage(pageInfo)
        val canvas: Canvas = page.canvas

        val titlePaint = Paint().apply { textSize = 24f; color = Color.BLACK; isFakeBoldText = true }
        val headerPaint = Paint().apply { textSize = 14f; color = Color.BLACK; isFakeBoldText = true }
        val bodyPaint = Paint().apply { textSize = 11f; color = Color.DKGRAY }
        val mutedPaint = Paint().apply { textSize = 10f; color = Color.GRAY }
        val linePaint = Paint().apply { color = Color.LTGRAY; strokeWidth = 1f }

        var y = 60f
        val margin = 50f

        // Header
        canvas.drawText("FORGE — Session Report", margin, y, titlePaint)
        y += 30f
        canvas.drawText("$dayName · $dateStr · $timeStr", margin, y, bodyPaint)
        y += 16f
        if (durationMin != null) {
            canvas.drawText("Duration: ${durationMin}m  ·  Volume: ${formatVolume(session.totalVolumeLb ?: 0.0, useKg)}  ·  PRs: ${session.prCount}", margin, y, bodyPaint)
            y += 16f
        }
        if (!mood.isNullOrBlank() || session.tags.isNotBlank()) {
            val bits = listOfNotNull(
                mood?.takeIf { it.isNotBlank() }?.let { "Mood: $it" },
                session.tags.takeIf { it.isNotBlank() }?.let { "Tags: $it" }
            )
            canvas.drawText(bits.joinToString("  ·  "), margin, y, mutedPaint)
            y += 16f
        }

        canvas.drawLine(margin, y + 4, 595 - margin, y + 4, linePaint)
        y += 20f

        // Exercise sections
        var drawnExercises = 0
        for (ex in exercises) {
            if (y > 780f) break // safety: don't overflow page
            val sets = loggedSetDao.forLoggedExercise(ex.id)
            val exName = Program.exerciseDisplayName(ex.exerciseId, ex.swappedName)
            val effort = ex.difficulty?.name?.lowercase()?.replace('_', ' ')
            canvas.drawText(exName + if (effort != null) "  —  felt $effort" else "", margin, y, headerPaint)
            y += 16f
            sets.forEachIndexed { i, set ->
                val rpe = set.rpe?.let { r -> "  @ RPE " + (if (r % 1.0 == 0.0) "${r.toInt()}" else "%.1f".format(r)) } ?: ""
                val wLabel = if (set.weightLb != null && set.weightLb > 0) formatWeight(set.weightLb, useKg) else set.weightText
                canvas.drawText("  Set ${i + 1}: $wLabel × ${set.reps}$rpe", margin + 8, y, bodyPaint)
                y += 13f
            }
            if (!ex.note.isNullOrBlank()) {
                canvas.drawText("  note: ${ex.note}", margin + 8, y, mutedPaint)
                y += 13f
            }
            y += 6f
            drawnExercises++
        }
        // A single A4 page can't hold every exercise of a long session — say so rather than
        // silently dropping the overflow.
        val omitted = exercises.size - drawnExercises
        if (omitted > 0) {
            canvas.drawText(
                "… $omitted more exercise${if (omitted == 1) "" else "s"} not shown (one-page limit)",
                margin, minOf(y, 806f), mutedPaint
            )
        }

        // Session journal (free-text reflection), if any and there's room.
        if (session.journal.isNotBlank() && y < 770f) {
            y += 6f
            canvas.drawText("Journal", margin, y, headerPaint)
            y += 16f
            session.journal.chunked(90).forEach { line ->
                if (y > 800f) return@forEach
                canvas.drawText(line, margin, y, bodyPaint)
                y += 13f
            }
        }

        // Cardio logged the same week as this session — standalone entries (not joined to the
        // session), included so the report covers everything trained that week, with all fields.
        val sessionDate = Instant.ofEpochMilli(session.startedAt).atZone(zone).toLocalDate()
        val weekStart = sessionDate.with(DayOfWeek.MONDAY)
        val weekStartMs = weekStart.atStartOfDay(zone).toInstant().toEpochMilli()
        val weekEndMs = weekStart.plusDays(7).atStartOfDay(zone).toInstant().toEpochMilli()
        // Bounded range query (oldest-first, rest excluded) — touches only this week's rows instead of
        // loading the whole history-since-week-start into memory just to filter it down.
        val weekCardio = cardioDao.between(weekStartMs, weekEndMs)
        if (weekCardio.isNotEmpty()) {
            if (y < 770f) {
                val bodyweightLb = bodyweightDao.latest()?.weightLb
                y += 6f
                canvas.drawText("Cardio this week", margin, y, headerPaint)
                y += 16f
                var drawnCardio = 0
                for (c in weekCardio) {
                    if (y > 800f) break
                    val type = CardioType.fromCode(c.type)
                    val parts = cardioDetailParts(c, type, bodyweightLb, includeEffort = true)
                    canvas.drawText("  ${type.displayName}: ${parts.joinToString(" · ")}", margin + 8, y, bodyPaint)
                    y += 13f
                    if (!c.note.isNullOrBlank() && y < 805f) {
                        canvas.drawText("    note: ${c.note}", margin + 8, y, mutedPaint)
                        y += 13f
                    }
                    drawnCardio++
                }
                // Same one-page honesty as the exercise list: name what didn't fit rather than dropping it.
                val omittedCardio = weekCardio.size - drawnCardio
                if (omittedCardio > 0) {
                    canvas.drawText(
                        "… $omittedCardio more cardio entr${if (omittedCardio == 1) "y" else "ies"} not shown (one-page limit)",
                        margin, minOf(y, 806f), mutedPaint
                    )
                }
            } else {
                // No room for the section at all — acknowledge the week's cardio so the report doesn't
                // silently read as "no cardio this week".
                canvas.drawText(
                    "Cardio this week: ${weekCardio.size} entr${if (weekCardio.size == 1) "y" else "ies"} (see app — one-page limit)",
                    margin, minOf(y, 806f), mutedPaint
                )
            }
        }

        // Footer
        canvas.drawText("Generated by Forge · ${System.currentTimeMillis()}", margin, 820f, mutedPaint)

        doc.finishPage(page)
        // Fixed filename (overwrite) so exported PDFs don't accumulate in filesDir (#84); the file
        // is shared out immediately, so there's nothing to keep per session.
        val file = File(context.filesDir, "forge_session.pdf")
        file.outputStream().use { doc.writeTo(it) }
        doc.close()
        return file
    }
}
