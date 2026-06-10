package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
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
    val prefs: PrefsSnap
)

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
    val sets: List<LoggedSet>
)

/** Settings the engine needs, snapshotted from DataStore. */
data class PrefsSnap(
    val plateLb: Double = 15.0,
    val likedIds: Set<String> = emptySet(),
    val dislikedIds: Set<String> = emptySet(),
    val pinnedIds: Set<String> = emptySet()
)
