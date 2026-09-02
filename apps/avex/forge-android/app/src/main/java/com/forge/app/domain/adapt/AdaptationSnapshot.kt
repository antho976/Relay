package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.session.SessionType
import com.forge.app.program.ExerciseTag
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup

/**
 * Everything the adaptation engine reads, captured as one immutable value — the engine's
 * mirror of [com.forge.app.domain.trophy.TrophyStatsSnapshot]. Built by
 * [com.forge.app.data.repo.AdaptationRepository] (the only impure piece); every advisor is
 * a pure function of this snapshot, so tests construct it directly from fake data.
 *
 * Room entities ([Session], [LoggedSet]) are reused directly rather than re-mapped — they're
 * plain data classes and the PR/volume domain modules already take them (PrDetector precedent).
 *
 * Untracked sessions (#110) are excluded at build time: they must not feed any signal.
 * Later phases extend this with moods, cardio, and realized-rest events.
 */
data class AdaptationSnapshot(
    /** "Now" injected by the builder so advisors stay clock-free and deterministic. */
    val nowMs: Long,
    /** The user's timezone, injected for the same reason — hour-of-day rules stay pure. */
    val zoneId: java.time.ZoneId = java.time.ZoneOffset.UTC,
    /** The active program, in day/slot order. */
    val program: List<ProgramDaySnap>,
    /** Finished, tracked sessions, oldest-first. */
    val sessions: List<Session>,
    /** Per static exercise id: every bout (one session's work on it), oldest-first. */
    val exerciseHistory: Map<String, List<ExerciseBout>>,
    /** Post-workout moods, newest-first (System 5/6 recovery signal). */
    val moods: List<MoodEntry> = emptyList(),
    /** Cardio entries, newest-first — restReason sore/sick feeds recovery (System 5/6). */
    val cardio: List<CardioEntry> = emptyList(),
    /**
     * Bodyweight log, newest-first (A1). The one body series the engine was missing: weight phase
     * (cut/maintain/bulk) reinterprets stalls, and readiness reads weight flux. Empty until the
     * user logs a weight — additive like every other signal.
     */
    val bodyweight: List<BodyweightEntry> = emptyList(),
    val prefs: PrefsSnap,
    /**
     * Off-app recovery signals read from Health Connect (sleep, resting HR), when the user has
     * connected it. Last + defaulted on purpose: every advisor treats it as additive, so no HC
     * means no behavior change, and the existing test corpus keeps building without touching it.
     */
    val health: HealthSnap = HealthSnap()
)

/**
 * Recovery telemetry mirrored out of Health Connect (the only external source Avex reads).
 * Kept as plain Kotlin — no androidx.health types leak into the engine — so advisors stay pure
 * and unit tests build these from fake data exactly like [AdaptationSnapshot.moods]/[AdaptationSnapshot.cardio].
 */
data class HealthSnap(
    /** Recent sleep sessions (any order; the advisor windows them). */
    val sleepNights: List<SleepNight> = emptyList(),
    /** Recent resting-heart-rate readings (any order). */
    val restingHr: List<RestingHrSample> = emptyList(),
    /** Recent heart-rate-variability readings (RMSSD ms, any order) — a watch's overnight HRV (W6).
     *  Additive readiness input; empty when not granted / not produced. */
    val hrv: List<HrvSample> = emptyList(),
    /** Recent per-day step totals (W6) — the daily-movement readiness input. Empty when not granted. */
    val dailySteps: List<DailySteps> = emptyList()
)

/**
 * One night's sleep: when it ended (epoch-ms) and how long it lasted (minutes). [deepMin]/[remMin]
 * carry the provider's sleep stages when it reports them (W6); 0 = stages absent, never "no deep
 * sleep" — consumers gate on `deepMin + remMin > 0`.
 */
data class SleepNight(
    val endedAtMs: Long,
    val durationMin: Int,
    val deepMin: Int = 0,
    val remMin: Int = 0
)

/** One resting-HR reading: when it was taken (epoch-ms) and the value in beats per minute. */
data class RestingHrSample(val timeMs: Long, val bpm: Int)

/** One HRV reading: when it was taken (epoch-ms) and the RMSSD value in milliseconds (W6). */
data class HrvSample(val timeMs: Long, val rmssdMs: Double)

/** One day's total steps: local-midnight epoch-ms and the count (W6). */
data class DailySteps(val dayStartMs: Long, val steps: Int)

data class ProgramDaySnap(
    val dayKey: String,
    val name: String,
    val slots: List<ProgramSlotSnap>
)

data class ProgramSlotSnap(
    val exerciseId: String,
    val name: String,
    val muscle: MuscleGroup,
    val unit: ExerciseUnit,
    val tags: List<ExerciseTag>,
    val targetSets: Int,
    val repsText: String,
    /**
     * Library ids a swap suggestion may offer for this slot — pre-filtered by the user's
     * equipment + dislikes at snapshot-build time so advisors don't touch the library.
     */
    val swapCandidateIds: List<String> = emptyList()
)

/**
 * One session's work on one exercise. [sets] are in performed order; assisted sets are
 * carried (advisors filter them per-rule, mirroring [com.forge.app.domain.pr.PrDetector]).
 */
data class ExerciseBout(
    val sessionStartedAt: Long,
    val effort: EffortRating?,
    val hitFullTarget: Boolean,
    val skipped: Boolean,
    val swappedName: String?,
    val sets: List<LoggedSet>,
    /**
     * The library id of the lift ACTUALLY performed, which is not always the slot this bout is
     * filed under (H-06).
     *
     * History is keyed by the program SLOT so Coach can target a plan row whatever was done in it.
     * That is right for targeting and wrong for comparison: a slot holds a 300 lb bench one week
     * and a 50 lb fly swapped into it the next, and anything reading consecutive bouts as one lift's
     * progress sees an 83% collapse followed by a 500% surge. [VolumeResponse] turned exactly that
     * fiction into a muscle's volume verdict and the Stats insight beside it.
     *
     * Null for a bout built without one, which reads as "the slot's own lift" — true for every row
     * that was never swapped, and the only answer available to a caller that never had the id.
     */
    val performedExerciseId: String? = null,
    /**
     * The parent session's type key ([com.forge.app.domain.session.SessionType]), carried onto the
     * bout in A1 so advisors can exclude sessions that aren't ordinary training. Defaults to
     * "normal" — a bout with no known parent type reads as a normal training bout, which is what
     * every pre-A1 row is.
     */
    val sessionType: String = SessionType.NORMAL.key
)

/**
 * Session types whose bouts must not feed progression, plateau or fatigue reads (A1):
 * a TEST day's top single is not a working bout, a TECHNIQUE day is deliberately light, and a
 * FIRST_BACK ramp is a return-from-layoff week. Counting any of them as ordinary training reads
 * as a stall (or a PR) that never happened. DELOAD stays *in*: a deload week is planned training
 * and the fatigue model already reasons about it explicitly.
 */
val EXCLUDED_FROM_PROGRESSION: Set<String> = setOf(
    SessionType.TEST.key,
    SessionType.TECHNIQUE.key,
    SessionType.FIRST_BACK.key
)

/** True when this bout is ordinary training — see [EXCLUDED_FROM_PROGRESSION]. */
val ExerciseBout.countsForProgression: Boolean
    get() = sessionType !in EXCLUDED_FROM_PROGRESSION

/** Settings the engine needs, snapshotted from DataStore. */
data class PrefsSnap(
    val plateLb: Double = 15.0,
    val likedIds: Set<String> = emptySet(),
    val dislikedIds: Set<String> = emptySet(),
    val pinnedIds: Set<String> = emptySet(),
    /** Heaviest dumbbell owned (lb); null = no ceiling (auto-coach Phase 0). */
    val maxDbLb: Double? = null,
    /**
     * When the current deload week was applied (epoch-ms), or null if not in one. Persists the
     * deload so DeloadAdvisor can suppress repeat proposals immediately after an apply — before any
     * deload-week session exists to scan (seam fix, finding 18).
     */
    val lastDeloadAppliedMs: Long? = null
)
