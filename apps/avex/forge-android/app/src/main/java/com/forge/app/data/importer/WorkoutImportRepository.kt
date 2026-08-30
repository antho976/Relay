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
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
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
    private val moodDao: com.forge.app.data.db.dao.MoodDao,
    private val cardioDao: com.forge.app.data.db.dao.CardioDao,
    private val coachGoalDao: com.forge.app.data.db.dao.CoachGoalDao,
    private val bodyweightDao: com.forge.app.data.db.dao.BodyweightDao,
    private val settingsRepo: SettingsRepository
) {
    /** uri → (lastModified, what the scan concluded). Bounded by [MAX_SCAN_FILES] per folder. */
    private val scanCache = java.util.concurrent.ConcurrentHashMap<String, Pair<Long, FoundImport?>>()

    // Ordered by specificity: dedicated parsers first, the fuzzy generic CSV last so it only catches
    // what nothing else recognised.
    private val importers: List<GymImporter> = listOf(
        StrongImporter(),
        HevyImporter(),
        FitNotesImporter(),
        ForgeJsonImporter(),
        ForgeBodyweightCsvImporter(),
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
        // A file from a FUTURE export format would parse "successfully" against today's key names
        // and silently drop or misread whatever changed. Refuse it and say why instead.
        importer.formatVersion(text)?.let { v ->
            if (v > SUPPORTED_EXPORT_VERSION) return@withContext ImportResult.UnsupportedExportVersion(v)
        }
        val assumeKg = settingsRepo.useKg.first()
        // An OOM building a huge JSON tree is not "nothing to import". runCatching catches Throwable,
        // so a power user's multi-year export that couldn't fit in memory used to be reported as "No
        // new workouts found in that file" — they concluded the export was empty and gave up.
        val parsed = try {
            importer.parse(text, assumeKg)
        } catch (e: OutOfMemoryError) {
            return@withContext ImportResult.TooLarge
        } catch (e: Exception) {
            emptyList()
        }
        val sessions = parsed.filter { it.exercises.isNotEmpty() }
        // Cardio and coach goals are carried by our own export and used to be read by nobody, so a
        // JSON migration lost them all silently. A file with no workouts but 400 cardio entries is
        // not "nothing to import".
        val extras = try {
            importer.parseExtras(text)
        } catch (e: OutOfMemoryError) {
            return@withContext ImportResult.TooLarge
        } catch (e: Exception) {
            ImportedExtras()
        }
        if (sessions.isEmpty() && extras.isEmpty) return@withContext ImportResult.NothingToImport

        insert(importer.source, sessions, extras)
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
            val key = doc.uri.toString()
            val stamp = doc.lastModified()
            // An unchanged file gives an unchanged answer. Without this, every visit to the Import
            // screen — and every return to it after an import — re-read and re-parsed the whole
            // folder from scratch.
            val cached = scanCache[key]
            if (cached != null && cached.first == stamp) {
                cached.second?.let(found::add)
                continue
            }
            // Sniff on a bounded PREFIX. Detection is a header line (or one JSON key near the front)
            // for every importer, so pulling a 25 MB bank statement fully into memory to decide it
            // isn't a workout export was pure waste — and the dominant cost of the scan.
            val head = readPrefix(doc.uri, SNIFF_BYTES)
            if (head.isNullOrBlank()) { scanCache[key] = stamp to null; continue }
            val importer = importers.firstOrNull { it.canParse(head) }
            if (importer == null) { scanCache[key] = stamp to null; continue }
            if (importer.formatVersion(head)?.let { it > SUPPORTED_EXPORT_VERSION } == true) {
                scanCache[key] = stamp to null
                continue
            }
            val text = (readBounded(doc.uri) as? Read.Ok)?.text
            if (text == null) { scanCache[key] = stamp to null; continue }
            // A file too big to parse is skipped rather than listed — and never allowed to take the
            // whole scan down with it.
            val count = try {
                importer.parse(text, assumeKg).count { it.exercises.isNotEmpty() }
            } catch (e: OutOfMemoryError) {
                0
            } catch (e: Exception) {
                0
            }
            val entry = if (count == 0) null
            else FoundImport(doc.uri, doc.name ?: "export", importer.source, count, stamp)
            scanCache[key] = stamp to entry
            entry?.let(found::add)
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

    private suspend fun insert(
        source: ImportSource,
        sessions: List<ImportedSession>,
        extras: ImportedExtras = ImportedExtras()
    ): ImportResult {
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
        //
        // The nonce is per-run; the DB is not. A slot already occupied by a DIFFERENT workout — the
        // morning session from a FitNotes export, when the evening one arrives later in a Strong
        // export whose Date column is date-only — used to make the incoming workout "a duplicate"
        // and drop it silently. The slot search below walks past occupied instants instead, and only
        // calls it a duplicate when the stored session holds the same work.
        val startNonce = HashMap<Long, Int>()
        // Memoise name→catalogue-id for this import: the same movement recurs across many sessions and
        // ExerciseNameMatcher.match scans the whole library, so resolve each distinct name only once.
        val matchCache = HashMap<String, String?>()

        db.withTransaction {
            for (session in sessions) {
                val totalSets = session.exercises.sumOf { it.sets.size }
                if (totalSets == 0) continue

                // Denormalised volume from lb weights, matching how a real finished session is
                // stamped — including VolumeCalculator's exclusion of timed holds, whose reps is a
                // duration, not a count.
                val volumeLb = session.exercises.sumOf { ex ->
                    ex.sets.filter { it.durationSeconds == null }
                        .sumOf { (it.weightLb ?: 0.0) * it.reps }
                }

                // Duplicate guard (#GYMAP-17): a workout already logged at this start time WITH THE
                // SAME CONTENT is a re-import of the same data — skip it so scanning/importing twice
                // doesn't double-count. A different workout at the same instant takes the next slot.
                //
                // The content test is a per-exercise fingerprint, not set count plus total volume.
                // Those two agree for workouts that share nothing but arithmetic — Bench 3×10×100
                // and Row 3×10×100 are three sets and 3,000 lb either way — and a great many sources
                // record a DATE rather than a time, so every workout they carry starts at midnight
                // and every one of them is a candidate against every other. The second of two
                // legitimately different midnight workouts was reported as a duplicate and dropped,
                // silently, on an import path whose whole promise is that it does not lose anything.
                val incomingPrint = fingerprintOf(session, matchCache)
                var nth = startNonce.getOrDefault(session.startedAtMs, 0)
                var startedAt = session.startedAtMs + nth * 1000L
                var duplicate = false
                while (nth < MAX_START_NUDGES) {
                    val storedIds = sessionDao.idsAtStart(startedAt)
                    if (storedIds.isEmpty()) break
                    if (storedIds.any { storedFingerprint(it) == incomingPrint }) {
                        duplicate = true
                        break
                    }
                    nth++
                    startedAt = session.startedAtMs + nth * 1000L
                }
                startNonce[session.startedAtMs] = nth + 1
                if (duplicate) { duplicates++; continue }

                // Three separate facts, and conflating them rewrote real training history.
                //
                // `finishedAt` is when the workout ENDED — kept verbatim from the source. A session
                // started Friday evening and resumed Sunday morning legitimately spans two days.
                // `activeSeconds` is time actually spent training, which the app stores separately
                // for exactly that reason. Deriving one from the other and then clamping the result
                // to 6 h turned that 70-minute session into a claimed six-hour one, and clamped a
                // 45-second finisher UP to a minute.
                //
                // Only a SYNTHESISED duration — for a source that records no end time at all — is
                // clamped, and only into a plausible session length.
                val sourceActive = session.activeSeconds?.takeIf { it > 0 }
                val sourceFinished = session.finishedAtMs?.takeIf { it > startedAt }
                val synthesisedSec = (sourceActive ?: (totalSets * SECONDS_PER_SET))
                    .coerceIn(60, MAX_SYNTHESISED_ACTIVE_SEC)
                val finishedAt = sourceFinished ?: (startedAt + synthesisedSec * 1000L)
                val activeSec = sourceActive
                    ?: if (sourceFinished != null) {
                        // A real start/end pair from another app is the best estimate available;
                        // bound it only against nonsense, not down to a "plausible" session length.
                        ((finishedAt - startedAt) / 1000L).toInt().coerceIn(0, MAX_WALL_CLOCK_ACTIVE_SEC)
                    } else {
                        synthesisedSec
                    }

                // Another app's split doesn't map to ours, so those still land as freestyle — but our
                // OWN export names the day each workout belonged to, and forcing that to FREESTYLE
                // turned a migrated program history into 600 indistinguishable open workouts. A key
                // that isn't in the user's current plan reads fine: every consumer resolves day keys
                // with firstOrNull, and Program.dayDisplayName humanizes an unknown one by design.
                val dayKey = session.dayKey ?: Program.FREESTYLE_DAY_KEY
                val sessionId = sessionDao.insert(
                    Session(
                        dayKey = dayKey,
                        startedAt = startedAt,
                        finishedAt = finishedAt,
                        totalVolumeLb = volumeLb,
                        prCount = session.prCount ?: 0,
                        setCount = totalSets,
                        sessionType = session.sessionType ?: "normal",
                        intensity = session.intensity ?: "normal",
                        isUntracked = session.isUntracked,
                        tags = session.tags ?: "",
                        journal = session.note ?: "",
                        activeSeconds = activeSec
                    )
                )
                // Mood feeds readiness and recovery, and the export has always carried it.
                session.mood?.let { mood ->
                    moodDao.insert(
                        MoodEntry(
                            sessionId = sessionId,
                            dayKey = dayKey,
                            mood = mood,
                            recordedAt = finishedAt
                        )
                    )
                }

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
                            orderIndex = ex.orderIndex ?: orderIndex,
                            swappedName = if (matchedId == null) ex.name else null,
                            difficulty = effortRating(ex.difficulty),
                            skipped = ex.skipped,
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
                                // The source's own text when it has one (our JSON export does), so
                                // "2 plates" survives the round trip instead of coming back "135".
                                weightText = s.weightText ?: weightText(s.weightLb),
                                weightLb = s.weightLb,
                                reps = s.reps,
                                // The source's own per-set instant when it has one; otherwise the
                                // session's finish, as before. Stamping every set of a two-hour
                                // workout at the same millisecond loses the within-session ordering
                                // that the set detail and HR overlay read.
                                completedAt = s.completedAtMs ?: finishedAt,
                                rpe = s.rpe,
                                // Carried through rather than left at the entity defaults: a timed
                                // hold whose durationSeconds went missing reads as a rep set, and an
                                // assisted set that came back as unassisted becomes PR-eligible.
                                durationSeconds = s.durationSeconds,
                                isAssisted = s.isAssisted,
                                isAmrap = s.isAmrap,
                                toFailure = s.toFailure,
                                setType = s.setType ?: if (s.isWarmup) "warmup" else null,
                                difficultyTag = s.difficultyTag,
                                dropAnnotation = s.dropAnnotation
                            )
                        }
                    )
                    exerciseCount++
                    setCount += ex.sets.size
                }
                importedSessions++
            }
        }

        val extrasWritten = insertExtras(extras)

        // Everything in the file was already present — say so distinctly, not "imported 0".
        if (importedSessions == 0 && extrasWritten.none) return ImportResult.NothingToImport

        return ImportResult.Success(
            source = source,
            sessions = importedSessions,
            exercises = exerciseCount,
            sets = setCount,
            matchedExercises = matchedNames.size,
            unmatchedExercises = unmatchedNames.size,
            skippedRows = 0,
            duplicatesSkipped = duplicates,
            cardioEntries = extrasWritten.cardio,
            coachGoals = extrasWritten.goals,
            bodyweightEntries = extrasWritten.bodyweight
        )
    }

    /**
     * Writes the non-workout rows an Avex export carries, skipping ones already present so a repeat
     * import doesn't double them the way a repeat session import doesn't. Returns (cardio, goals).
     */
    private suspend fun insertExtras(extras: ImportedExtras): ExtrasWritten {
        if (extras.isEmpty) return ExtrasWritten()
        var cardioWritten = 0
        var goalsWritten = 0
        var weighInsWritten = 0
        db.withTransaction {
            for (c in extras.cardio) {
                if (cardioDao.existsAt(c.dateMs, c.type, c.durationMin)) continue
                cardioDao.insert(
                    CardioEntry(
                        date = c.dateMs,
                        type = c.type,
                        durationMin = c.durationMin,
                        distanceKm = c.distanceKm,
                        effort = c.effort,
                        restReason = c.restReason,
                        note = c.note,
                        inclinePct = c.inclinePct,
                        laps = c.laps,
                        elevationM = c.elevationM
                    )
                )
                cardioWritten++
            }
            for (g in extras.coachGoals) {
                if (coachGoalDao.existsLike(g.kind, g.targetKey, g.createdAt)) continue
                coachGoalDao.insert(
                    CoachGoal(
                        kind = g.kind,
                        targetKey = g.targetKey,
                        targetValue = g.targetValue,
                        priority = g.priority,
                        createdAt = g.createdAt,
                        completedAt = g.completedAt,
                        archivedAt = g.archivedAt,
                        source = g.source,
                        note = g.note
                    )
                )
                goalsWritten++
            }
            for (b in extras.bodyweight) {
                // An import MERGES; it never overwrites. The DAO's REPLACE upsert would silently
                // replace a weigh-in the user has since corrected on this device.
                if (bodyweightDao.byDateKey(b.dateKey) != null) continue
                bodyweightDao.upsert(
                    BodyweightEntry(
                        dateKey = b.dateKey,
                        weightLb = b.weightLb,
                        recordedAt = dateKeyToMillis(b.dateKey)
                    )
                )
                weighInsWritten++
            }
        }
        return ExtrasWritten(cardioWritten, goalsWritten, weighInsWritten)
    }

    /** What [insertExtras] actually wrote. */
    private data class ExtrasWritten(val cardio: Int = 0, val goals: Int = 0, val bodyweight: Int = 0) {
        val none: Boolean get() = cardio == 0 && goals == 0 && bodyweight == 0
    }

    /** Local midnight of a `yyyy-MM-dd` key — the entry's own date is authoritative, and `date_key`
     *  is what the unique index and every read are keyed on. */
    private fun dateKeyToMillis(dateKey: String): Long = runCatching {
        java.time.LocalDate.parse(dateKey)
            .atStartOfDay(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()
    }.getOrDefault(System.currentTimeMillis())

    /** Canonical lb weight string (weightText is always stored in lb); "BW" for a bodyweight set. */
    private fun weightText(weightLb: Double?): String {
        if (weightLb == null || weightLb <= 0.0) return "BW"
        // Same bare-number formatting the log/edit fields use, so imported sets read identically.
        return com.forge.app.domain.units.weightInputValue(weightLb, useKg = false)
    }

    /** The export writes `EffortRating.name`; older/other files may carry the storage code. Accept
     *  either, and ignore anything else rather than failing the row. */
    private fun effortRating(raw: String?): com.forge.app.data.db.types.EffortRating? {
        val v = raw?.trim().orEmpty()
        if (v.isBlank()) return null
        return com.forge.app.data.db.types.EffortRating.entries
            .firstOrNull { it.name.equals(v, ignoreCase = true) || it.code.equals(v, ignoreCase = true) }
    }

    /** A stable id for an unmatched exercise so its sets group together across imported sessions. */
    /**
     * What one imported session actually contains: each exercise's resolved catalogue id followed by
     * its sets in order, as reps and weight.
     *
     * Resolved through the SAME [matchCache] the insert below uses, so the id compared here is the
     * id that would be written — an unmatched name folds to its synthetic id on both sides. Weights
     * are rounded to a thousandth of a pound: a re-import carries bit-identical values, and no real
     * source distinguishes finer than that.
     */
    private fun fingerprintOf(
        session: ImportedSession,
        matchCache: HashMap<String, String?>
    ): String = session.exercises.joinToString("|") { ex ->
        val matched = ex.catalogueId ?: matchCache.getOrPut(ex.name) { ExerciseNameMatcher.match(ex.name) }
        val id = matched ?: syntheticId(ex.name)
        id + ":" + ex.sets.joinToString(",") { setPrint(it.reps, it.weightLb) }
    }

    /** [fingerprintOf] for a session already in the database. */
    private suspend fun storedFingerprint(sessionId: Long): String {
        val setsByExercise = loggedSetDao.allForSession(sessionId).groupBy { it.loggedExerciseId }
        return loggedExerciseDao.forSession(sessionId).joinToString("|") { le ->
            le.exerciseId + ":" + setsByExercise[le.id].orEmpty()
                .sortedBy { it.setIndex }
                .joinToString(",") { setPrint(it.reps, it.weightLb) }
        }
    }

    private fun setPrint(reps: Int, weightLb: Double?): String =
        "$reps@" + (weightLb?.let { String.format(java.util.Locale.US, "%.3f", it) } ?: "bw")

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

    /**
     * The leading [maxBytes] of a URI, decoded with the same charset sniff as [readBounded]. Used by
     * the folder scan to decide whether a file is worth reading in full. Null on any read failure.
     */
    private fun readPrefix(uri: Uri, maxBytes: Int): String? = runCatching {
        context.contentResolver.openInputStream(uri)?.use { input ->
            val out = java.io.ByteArrayOutputStream(minOf(maxBytes, READ_CHUNK_BYTES))
            val chunk = ByteArray(READ_CHUNK_BYTES)
            while (out.size() < maxBytes) {
                val n = input.read(chunk, 0, minOf(chunk.size, maxBytes - out.size()))
                if (n < 0) break
                out.write(chunk, 0, n)
            }
            decode(out.toByteArray())
        }
    }.getOrNull()

    /**
     * Decode file bytes, honouring a byte-order mark instead of assuming UTF-8.
     *
     * Excel's "Unicode Text (*.txt)" and several Windows companion tools write UTF-16LE. Read as
     * UTF-8 those bytes become replacement characters interleaved with NULs, so every `canParse`
     * failed and the user was told their own export "isn't a recognised gym-app export". The BOM is
     * consumed as part of the decode — `CsvParser` strips a UTF-8 one too, but that happens after
     * decoding and so could never have helped a UTF-16 file.
     */
    private fun decode(bytes: ByteArray): String = when {
        bytes.size >= 2 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xFE.toByte() ->
            String(bytes, 2, bytes.size - 2, Charsets.UTF_16LE)
        bytes.size >= 2 && bytes[0] == 0xFE.toByte() && bytes[1] == 0xFF.toByte() ->
            String(bytes, 2, bytes.size - 2, Charsets.UTF_16BE)
        bytes.size >= 3 && bytes[0] == 0xEF.toByte() && bytes[1] == 0xBB.toByte() && bytes[2] == 0xBF.toByte() ->
            String(bytes, 3, bytes.size - 3, Charsets.UTF_8)
        else -> String(bytes, Charsets.UTF_8)
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
            Read.Ok(decode(out.toByteArray()))
        } ?: Read.Error
    }.getOrDefault(Read.Error)

    companion object {
        /** Nominal per-set time when the source records no duration, so the session reads as finished. */
        private const val SECONDS_PER_SET = 150
        /** Ceiling for a duration we INVENTED (a date-only source with no end time) — a made-up
         *  figure shouldn't claim a marathon session. Never applied to a source's own numbers. */
        private const val MAX_SYNTHESISED_ACTIVE_SEC = 6 * 3600
        /** Sanity bound on a wall-clock delta from a source that gives real start and end times.
         *  Wide on purpose: clamping this to 6 h is what rewrote resumed sessions. */
        private const val MAX_WALL_CLOCK_ACTIVE_SEC = 24 * 3600
        private const val MAX_IMPORT_BYTES = 25 * 1024 * 1024 // 25 MB — generous for a text export
        private const val READ_CHUNK_BYTES = 64 * 1024 // grow the read buffer in 64 KB chunks
        /** File extensions worth sniffing during a folder scan. */
        private val IMPORTABLE_EXTENSIONS = listOf(".csv", ".json", ".txt")
        /** Cap folder-scan work; Downloads can be large and we only need the recent exports. */
        private const val MAX_SCAN_FILES = 60
        /** Highest `exportVersion` this build knows how to read. Bump it with the export format. */
        private const val SUPPORTED_EXPORT_VERSION = 1
        /** How much of a file the folder scan reads to decide what it is. Every importer detects
         *  from a header line or one key near the front, so this is generous. */
        private const val SNIFF_BYTES = 64 * 1024
        /** How far the start-instant nudge may walk before giving up and letting the collision stand.
         *  Nobody logs 60 distinct workouts at one midnight; this only bounds a pathological file. */
        private const val MAX_START_NUDGES = 60

        /** Two denormalised volumes describe the same work — tolerant of the 0.1 lb rounding the
         *  importer applies, and treating a missing volume as "unknown, not equal". */
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
