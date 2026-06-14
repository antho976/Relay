package com.forge.app.data.repo

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.pdf.PdfDocument
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.Program
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

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
    private val moodDao: com.forge.app.data.db.dao.MoodDao
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
        val exercises = loggedExerciseDao.forSession(sessionId)
        val dayName = Program.days.firstOrNull { it.key == session.dayKey }?.defaultName
            ?: session.dayKey
        val dateStr = Instant.ofEpochMilli(session.startedAt).atZone(zone).format(dateFmt)
        val timeStr = Instant.ofEpochMilli(session.startedAt).atZone(zone).format(timeFmt)
        // Real active time (summed sittings); fall back to wall-clock for pre-feature sessions.
        val durationMin = when {
            session.activeSeconds > 0 -> session.activeSeconds / 60
            else -> session.finishedAt?.let { ((it - session.startedAt) / 60_000).toInt() }
        }
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
            canvas.drawText("Duration: ${durationMin}m  ·  Volume: ${(session.totalVolumeLb ?: 0.0).toInt()} lb  ·  PRs: ${session.prCount}", margin, y, bodyPaint)
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
            val exName = ex.swappedName ?: Program.exercise(ex.exerciseId)?.name ?: ex.exerciseId
            val effort = ex.difficulty?.name?.lowercase()?.replace('_', ' ')
            canvas.drawText(exName + if (effort != null) "  —  felt $effort" else "", margin, y, headerPaint)
            y += 16f
            sets.forEachIndexed { i, set ->
                val rpe = set.rpe?.let { r -> "  @ RPE " + (if (r % 1.0 == 0.0) "${r.toInt()}" else "%.1f".format(r)) } ?: ""
                canvas.drawText("  Set ${i + 1}: ${set.weightText} × ${set.reps}$rpe", margin + 8, y, bodyPaint)
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
