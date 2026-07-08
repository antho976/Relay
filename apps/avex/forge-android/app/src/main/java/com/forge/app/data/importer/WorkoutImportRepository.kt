package com.forge.app.data.importer

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import androidx.room.withTransaction
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.volume.VolumeCalculator
import com.forge.app.program.Program
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Imports training history from other gym apps (#GYMAP-17) — the read-side counterpart of
 * [com.forge.app.data.repo.BackupRepository]'s exports. A user picks a file (SAF), we auto-detect
 * which app it came from ([importers], first match wins), parse it to the neutral [ImportedSession]
 * model, and MERGE the sessions into the DB (never replacing existing data, unlike a .zip restore).
 *
 * Exercise names are matched to the catalogue ([ExerciseNameMatcher]); an unmatched name is still
 * imported under its own label so nothing is dropped. Imported workouts are logged as open/freestyle
 * sessions ([Program.FREESTYLE_DAY_KEY]) since another app's split doesn't map to ours.
 */
@Singleton
class WorkoutImportRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: ForgeDatabase,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val settingsRepo: SettingsRepository
) {
    // Ordered by specificity: dedicated parsers first, the fuzzy generic CSV last so it only catches
    // what nothing else recognised.
    private val importers: List<GymImporter> = listOf(
        StrongImporter(),
        HevyImporter(),
        FitNotesImporter(),
        ForgeJsonImporter(),
        GenericCsvImporter()
    )

    suspend fun import(uri: Uri): ImportResult = withContext(Dispatchers.IO) {
        val text = when (val read = readBounded(uri)) {
            is Read.Ok -> read.text
            Read.TooLarge -> return@withContext ImportResult.TooLarge
            Read.Error -> return@withContext ImportResult.ReadError
        }
        if (text.isBlank()) return@withContext ImportResult.NothingToImport

        val importer = importers.firstOrNull { it.canParse(text) }
            ?: return@withContext ImportResult.UnrecognisedFormat
        val assumeKg = settingsRepo.useKg.first()
        val sessions = runCatching { importer.parse(text, assumeKg) }.getOrDefault(emptyList())
            .filter { it.exercises.isNotEmpty() }
        if (sessions.isEmpty()) return@withContext ImportResult.NothingToImport

        insert(importer.source, sessions)
    }

    /**
     * Auto-find gym-app exports in a folder the user granted (usually Downloads). Enumerates the tree,
     * sniffs each `.csv/.json/.txt` with the detectors, and returns the ones a parser recognises —
     * newest first — so the Import screen can list them for a one-tap pick (#GYMAP-17). A light parse
     * gives the workout count; the actual insert happens later via [import].
     */
    suspend fun scanFolder(treeUri: Uri): List<FoundImport> = withContext(Dispatchers.IO) {
        val tree = runCatching { DocumentFile.fromTreeUri(context, treeUri) }.getOrNull() ?: return@withContext emptyList()
        val assumeKg = settingsRepo.useKg.first()
        val candidates = runCatching { tree.listFiles() }.getOrDefault(emptyArray())
            .filter { it.isFile && it.name?.lowercase()?.let { n -> IMPORTABLE_EXTENSIONS.any(n::endsWith) } == true }
            .sortedByDescending { it.lastModified() }
            .take(MAX_SCAN_FILES)
        val found = ArrayList<FoundImport>()
        for (doc in candidates) {
            val text = (readBounded(doc.uri) as? Read.Ok)?.text ?: continue
            if (text.isBlank()) continue
            val importer = importers.firstOrNull { it.canParse(text) } ?: continue
            val count = runCatching { importer.parse(text, assumeKg).count { it.exercises.isNotEmpty() } }.getOrDefault(0)
            if (count == 0) continue
            found.add(FoundImport(doc.uri, doc.name ?: "export", importer.source, count, doc.lastModified()))
        }
        found
    }

    /** Take a persistable read grant on the picked folder and remember it, so later scans need no re-pick. */
    suspend fun rememberFolder(treeUri: Uri) = withContext(Dispatchers.IO) {
        runCatching {
            context.contentResolver.takePersistableUriPermission(treeUri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        settingsRepo.setImportFolderUri(treeUri.toString())
    }

    private suspend fun insert(source: ImportSource, sessions: List<ImportedSession>): ImportResult {
        val matchedNames = HashSet<String>()
        val unmatchedNames = HashSet<String>()
        var exerciseCount = 0
        var setCount = 0
        var duplicates = 0
        var importedSessions = 0

        // Date-only sources (FitNotes, a bare spreadsheet, Strong/Generic without a time) stamp every
        // workout that day at midnight, so two DISTINCT same-day workouts would share a start instant.
        // Nudge each one after the first at a given instant a few seconds forward — deterministic from
        // file order, so a genuine re-import reproduces the same instants and the duplicate guard below
        // still recognises them, while distinct same-day workouts no longer collide and get dropped.
        val startNonce = HashMap<Long, Int>()
        // Memoise name→catalogue-id for this import: the same movement recurs across many sessions and
        // ExerciseNameMatcher.match scans the whole library, so resolve each distinct name only once.
        val matchCache = HashMap<String, String?>()

        db.withTransaction {
            for (session in sessions) {
                val totalSets = session.exercises.sumOf { it.sets.size }
                if (totalSets == 0) continue

                val nth = startNonce.getOrDefault(session.startedAtMs, 0)
                startNonce[session.startedAtMs] = nth + 1
                val startedAt = session.startedAtMs + nth * 1000L

                // Duplicate guard (#GYMAP-17): a workout already logged at this exact start time is a
                // re-import of the same data — skip it so scanning/importing twice doesn't double-count.
                if (sessionDao.countAtStart(startedAt) > 0) { duplicates++; continue }

                // Denormalised volume from lb weights, matching how a real finished session is stamped.
                val volumeLb = session.exercises.sumOf { ex ->
                    ex.sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
                }
                val activeSec = (session.finishedAtMs
                    ?.let { ((it - startedAt) / 1000L).toInt() }
                    ?: (totalSets * SECONDS_PER_SET))
                    .coerceIn(60, 6 * 3600)
                val finishedAt = startedAt + activeSec * 1000L

                val sessionId = sessionDao.insert(
                    Session(
                        dayKey = Program.FREESTYLE_DAY_KEY,
                        startedAt = startedAt,
                        finishedAt = finishedAt,
                        totalVolumeLb = volumeLb,
                        prCount = 0,
                        setCount = totalSets,
                        sessionType = "normal",
                        journal = session.note ?: "",
                        activeSeconds = activeSec
                    )
                )

                session.exercises.forEachIndexed { orderIndex, ex ->
                    // A source that already knows the catalogue id (our own JSON export) pins it
                    // directly; everything else resolves by name (memoised across sessions).
                    val matchedId = ex.catalogueId ?: matchCache.getOrPut(ex.name) { ExerciseNameMatcher.match(ex.name) }
                    if (matchedId != null) matchedNames.add(ex.name.lowercase())
                    else unmatchedNames.add(ex.name.lowercase())

                    val loggedExerciseId = loggedExerciseDao.insert(
                        LoggedExercise(
                            sessionId = sessionId,
                            // Matched → canonical catalogue id (stats attribute correctly). Unmatched →
                            // a stable synthetic id keyed on the name, kept readable via swappedName.
                            exerciseId = matchedId ?: syntheticId(ex.name),
                            orderIndex = orderIndex,
                            swappedName = if (matchedId == null) ex.name else null,
                            note = ex.note
                        )
                    )
                    // One bulk insert per exercise instead of a DB round-trip per set — a multi-year
                    // export can carry thousands of sets.
                    loggedSetDao.insertAll(
                        ex.sets.mapIndexed { setIndex, s ->
                            LoggedSet(
                                loggedExerciseId = loggedExerciseId,
                                setIndex = setIndex,
                                weightText = weightText(s.weightLb),
                                weightLb = s.weightLb,
                                reps = s.reps,
                                completedAt = finishedAt,
                                rpe = s.rpe
                            )
                        }
                    )
                    exerciseCount++
                    setCount += ex.sets.size
                }
                importedSessions++
            }
        }

        // Everything in the file was already present — say so distinctly, not "imported 0".
        if (importedSessions == 0 && duplicates > 0) return ImportResult.NothingToImport

        return ImportResult.Success(
            source = source,
            sessions = importedSessions,
            exercises = exerciseCount,
            sets = setCount,
            matchedExercises = matchedNames.size,
            unmatchedExercises = unmatchedNames.size,
            skippedRows = 0,
            duplicatesSkipped = duplicates
        )
    }

    /** Canonical lb weight string (weightText is always stored in lb); "BW" for a bodyweight set. */
    private fun weightText(weightLb: Double?): String {
        if (weightLb == null || weightLb <= 0.0) return "BW"
        // Same bare-number formatting the log/edit fields use, so imported sets read identically.
        return com.forge.app.domain.units.weightInputValue(weightLb, useKg = false)
    }

    /** A stable id for an unmatched exercise so its sets group together across imported sessions. */
    private fun syntheticId(name: String): String {
        val slug = name.trim().lowercase()
            .map { if (it.isLetterOrDigit()) it else '-' }
            .joinToString("")
            .trim('-')
            .replace(Regex("-+"), "-")
            .take(40)
        return "ext-" + slug.ifBlank { "exercise" }
    }

    /** Outcome of a bounded file read — separates "too big" from "couldn't read" without a sentinel. */
    private sealed interface Read {
        data class Ok(val text: String) : Read
        data object TooLarge : Read
        data object Error : Read
    }

    /** Read a URI's text, capped at [MAX_IMPORT_BYTES]; [Read.TooLarge] past the cap, [Read.Error] on I/O failure. */
    private fun readBounded(uri: Uri): Read = runCatching {
        context.contentResolver.openInputStream(uri)?.use { input ->
            // Grow a buffer as we read instead of pre-allocating the 25 MB ceiling for every file —
            // exports are almost always tiny, and a folder scan reads dozens of them.
            val out = java.io.ByteArrayOutputStream(READ_CHUNK_BYTES)
            val chunk = ByteArray(READ_CHUNK_BYTES)
            while (true) {
                val n = input.read(chunk)
                if (n < 0) break
                if (out.size() + n > MAX_IMPORT_BYTES) return@use Read.TooLarge
                out.write(chunk, 0, n)
            }
            Read.Ok(String(out.toByteArray(), Charsets.UTF_8))
        } ?: Read.Error
    }.getOrDefault(Read.Error)

    companion object {
        /** Nominal per-set time when the source records no duration, so the session reads as finished. */
        private const val SECONDS_PER_SET = 150
        private const val MAX_IMPORT_BYTES = 25 * 1024 * 1024 // 25 MB — generous for a text export
        private const val READ_CHUNK_BYTES = 64 * 1024 // grow the read buffer in 64 KB chunks
        /** File extensions worth sniffing during a folder scan. */
        private val IMPORTABLE_EXTENSIONS = listOf(".csv", ".json", ".txt")
        /** Cap folder-scan work; Downloads can be large and we only need the recent exports. */
        private const val MAX_SCAN_FILES = 60
    }
}

/** A gym-app export found by [WorkoutImportRepository.scanFolder], ready to import with one tap. */
data class FoundImport(
    val uri: Uri,
    val name: String,
    val source: ImportSource,
    val sessionCount: Int,
    val lastModified: Long
)
