package com.forge.app.data.importer

import com.forge.app.data.repo.sanitizeReps
import com.forge.app.data.repo.sanitizeWeightLb
import java.time.LocalDate
import java.time.ZoneId

/**
 * The bounds every imported row passes through before it can reach the database, whichever parser
 * produced it (import hardening audit, 2026-09-27).
 *
 * The logging path clamps reps and weight at its write boundary (see
 * [com.forge.app.data.repo.sanitizeReps]); the import path wrote whatever a file said. A set of
 * "Infinity" lb or 2,000,000 reps went straight into the row and into the session's denormalised
 * volume, which is the exact poisoning those clamps exist to stop, reached by a file instead of a
 * fat finger. Text had no ceiling either: a 25 MB journal is one row larger than a CursorWindow
 * (2 MB), so every later read of that session threw, and the history screen with it.
 *
 * Applied ONCE, to the parsed model, before the duplicate guard sees it. Clamping only at the
 * insert would have stored 999 reps for a file that says 5000, and the next re-import of the same
 * file would compare 5000 against 999, miss, and add the workout a second time.
 */
internal object ImportBounds {

    /** Names, weight text, tags, day keys, cardio type, effort, zone: labels, never prose. */
    const val MAX_SHORT_TEXT = 200

    /** Journals and notes. Far past anything typed on a phone, far below a CursorWindow. */
    const val MAX_LONG_TEXT = 10_000

    /** 2000-01-01T00:00Z. No phone gym app predates it; an earlier instant is a unit or parse error. */
    const val EARLIEST_IMPORT_MS = 946_684_800_000L

    /** Slack past "now" for a device whose clock trails the exporting one, or a zone ahead of it. */
    const val FUTURE_SLACK_MS = 24L * 60 * 60 * 1000

    /** One day. A longer single cardio entry is a seconds-as-minutes slip, not an ultra. */
    const val MAX_CARDIO_MINUTES = 1440

    /** Past any single-day ride or ultra; a larger figure is a metres-as-km slip. */
    const val MAX_CARDIO_DISTANCE_KM = 1000.0

    /** Treadmills top out near 40 %; anything past a vertical wall is nonsense. */
    const val MAX_INCLINE_PCT = 100.0

    /** Climb over one entry, either sign. Everest from sea level is under half of this. */
    const val MAX_ELEVATION_M = 20_000.0

    /** Heaviest plausible human, with room. Past it the row is a typo or a different unit. */
    const val MAX_BODYWEIGHT_LB = 1500.0

    /**
     * [s] cut to [max] chars. Never ends on a lone high surrogate, which would store an unpaired
     * half of an emoji that renders as a replacement box.
     */
    fun cap(s: String, max: Int): String {
        if (s.length <= max) return s
        val end = if (Character.isHighSurrogate(s[max - 1])) max - 1 else max
        return s.substring(0, end)
    }

    fun shortText(s: String?): String? = s?.let { cap(it, MAX_SHORT_TEXT) }

    fun longText(s: String?): String? = s?.let { cap(it, MAX_LONG_TEXT) }

    /** The logging path's clamp, after dropping non-finite values it would pass through (NaN
     *  survives coerceIn unchanged, and Infinity would be stored as a real 2000 lb set). */
    fun weightLb(weightLb: Double?): Double? = sanitizeWeightLb(weightLb?.takeIf { it.isFinite() })

    fun reps(reps: Int): Int = sanitizeReps(reps)

    fun inWindow(epochMs: Long, nowMs: Long): Boolean =
        epochMs >= EARLIEST_IMPORT_MS && epochMs <= nowMs + FUTURE_SLACK_MS

    fun cardioMinutes(minutes: Int): Int = minutes.coerceIn(0, MAX_CARDIO_MINUTES)

    fun distanceKm(km: Double?): Double? = km?.takeIf { it.isFinite() && it > 0.0 && it <= MAX_CARDIO_DISTANCE_KM }

    fun inclinePct(pct: Double?): Double? = pct?.takeIf { it.isFinite() && it > 0.0 && it <= MAX_INCLINE_PCT }

    fun elevationM(m: Double?): Double? = m?.takeIf { it.isFinite() && it != 0.0 && kotlin.math.abs(it) <= MAX_ELEVATION_M }

    fun bodyweightLb(lb: Double?): Double? = lb?.takeIf { it.isFinite() && it > 0.0 && it <= MAX_BODYWEIGHT_LB }

    /**
     * [parsed] with every value bounded. A workout, cardio entry or weigh-in dated outside
     * [[EARLIEST_IMPORT_MS], now + [FUTURE_SLACK_MS]] is dropped and counted in
     * [ParsedImport.skippedRows], so a file of year-1970 or year-2400 rows reports unreadable rows
     * rather than landing at the top or bottom of every chart. A per-set instant outside it is only
     * forgotten: the insert then stamps the session's finish, as it does for a source without one.
     */
    fun apply(parsed: ParsedImport, nowMs: Long): ParsedImport {
        var skipped = 0
        val sessions = parsed.sessions.mapNotNull { s ->
            val finish = s.finishedAtMs
            if (!inWindow(s.startedAtMs, nowMs) || (finish != null && !inWindow(finish, nowMs))) {
                skipped++
                null
            } else {
                boundSession(s, nowMs)
            }
        }
        val cardio = parsed.extras.cardio.mapNotNull { c ->
            if (!inWindow(c.dateMs, nowMs)) {
                skipped++
                null
            } else {
                c.copy(
                    type = cap(c.type, MAX_SHORT_TEXT),
                    durationMin = cardioMinutes(c.durationMin),
                    distanceKm = distanceKm(c.distanceKm),
                    effort = shortText(c.effort),
                    restReason = shortText(c.restReason),
                    note = longText(c.note),
                    inclinePct = inclinePct(c.inclinePct),
                    elevationM = elevationM(c.elevationM),
                    hrZone = shortText(c.hrZone),
                    conditions = shortText(c.conditions)
                )
            }
        }
        val goals = parsed.extras.coachGoals.map { g ->
            g.copy(
                kind = cap(g.kind, MAX_SHORT_TEXT),
                targetKey = cap(g.targetKey, MAX_SHORT_TEXT),
                targetValue = g.targetValue?.takeIf { it.isFinite() },
                source = cap(g.source, MAX_SHORT_TEXT),
                note = cap(g.note, MAX_LONG_TEXT)
            )
        }
        val bodyweight = parsed.extras.bodyweight.mapNotNull { b ->
            val weight = bodyweightLb(b.weightLb)
            val dayStartMs = runCatching {
                LocalDate.parse(b.dateKey).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
            }.getOrNull()
            if (weight == null || dayStartMs == null || !inWindow(dayStartMs, nowMs)) {
                skipped++
                null
            } else {
                b.copy(weightLb = weight)
            }
        }
        return ParsedImport(
            sessions = sessions,
            extras = ImportedExtras(cardio = cardio, coachGoals = goals, bodyweight = bodyweight),
            skippedRows = parsed.skippedRows + skipped
        )
    }

    private fun boundSession(s: ImportedSession, nowMs: Long): ImportedSession = s.copy(
        exercises = s.exercises.map { ex ->
            ex.copy(
                name = cap(ex.name, MAX_SHORT_TEXT),
                sets = ex.sets.map { boundSet(it, nowMs) },
                note = longText(ex.note),
                catalogueId = shortText(ex.catalogueId),
                difficulty = shortText(ex.difficulty),
                sourceExerciseId = shortText(ex.sourceExerciseId),
                swappedName = shortText(ex.swappedName),
                supersetGroup = shortText(ex.supersetGroup)
            )
        },
        title = shortText(s.title),
        note = longText(s.note),
        dayKey = shortText(s.dayKey),
        sessionType = shortText(s.sessionType),
        intensity = shortText(s.intensity),
        tags = shortText(s.tags),
        mood = shortText(s.mood)
    )

    private fun boundSet(s: ImportedSet, nowMs: Long): ImportedSet {
        val safeLb = weightLb(s.weightLb)
        return s.copy(
            weightLb = safeLb,
            reps = reps(s.reps),
            // A clamped weight takes its text with it, as it does on the logging path: keeping
            // "1000000000" beside a stored 2000 shows a number the database does not hold. Null
            // makes the insert regenerate the bare lb figure.
            weightText = if (safeLb != s.weightLb) null else shortText(s.weightText),
            rpe = s.rpe?.takeIf { it.isFinite() },
            setType = shortText(s.setType),
            difficultyTag = shortText(s.difficultyTag),
            dropAnnotation = shortText(s.dropAnnotation),
            completedAtMs = s.completedAtMs?.takeIf { inWindow(it, nowMs) }
        )
    }
}
