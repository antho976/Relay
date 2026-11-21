package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.program.DayPlan
import com.forge.app.program.ExercisePlan

/**
 * Pure assembly of an [AdaptationSnapshot] from raw entity lists. The repository fetches;
 * this groups — keeping the join/grouping logic unit-testable with fake rows and the
 * repository a thin fetch-and-delegate.
 */
object SnapshotAssembler {

    /**
     * @param sessions finished, *tracked* sessions only (the repository filters untracked —
     *   #110 promises they feed no signal). Any order; sorted here.
     * @param swapCandidateIds resolves a slot's swap pool (equipment/dislike-filtered library
     *   ids). Passed as a function so this object never touches [com.forge.app.program.ExerciseLibrary].
     */
    fun assemble(
        nowMs: Long,
        program: List<DayPlan>,
        swapCandidateIds: (ExercisePlan) -> List<String>,
        sessions: List<Session>,
        loggedExercises: List<LoggedExercise>,
        loggedSets: List<LoggedSet>,
        prefs: PrefsSnap,
        moods: List<MoodEntry> = emptyList(),
        cardio: List<CardioEntry> = emptyList(),
        /** Bodyweight log (A1); any order, sorted newest-first here. Empty until the user logs one. */
        bodyweight: List<BodyweightEntry> = emptyList(),
        /** Off-app recovery signals from Health Connect; empty when the user hasn't connected it. */
        health: HealthSnap = HealthSnap(),
        // No default: a missing zone silently binned every session into UTC weeks, giving non-UTC
        // users wrong "best time of day" insights. Callers must choose (production: systemDefault). (F18.)
        zoneId: java.time.ZoneId
    ): AdaptationSnapshot {
        val orderedSessions = sessions.sortedBy { it.startedAt }
        // The whole parent session, not just its start time: A1 carries the session TYPE onto every
        // bout so advisors can exclude test/technique/first-back work from progression reads.
        val sessionById = orderedSessions.associateBy { it.id }
        val setsByLoggedExercise = loggedSets.groupBy { it.loggedExerciseId }

        val history = loggedExercises
            .mapNotNull { le ->
                // Drop rows whose parent session isn't in the (finished, tracked) list.
                val session = sessionById[le.sessionId] ?: return@mapNotNull null
                val startedAt = session.startedAt
                // Key by the SLOT, not the re-keyed exercise_id (#11): every engine consumer looks
                // history up by the program slot id (ProgramSlotSnap.exerciseId = plan.id), and the
                // `slots` map that resolves names/muscles is slot-keyed too. A swapped row's
                // exercise_id is the swapped exercise, so keying on it would hide all swapped-slot
                // sessions from the coach (plateau ladder, volume/skip decisions, OutcomeWatcher,
                // insights). effectiveSlotId = slotId ?: exerciseId, which equals the slot id for
                // both pre-v22 rows (exercise_id IS the slot) and post-v22 swapped rows.
                le.effectiveSlotId to ExerciseBout(
                    sessionStartedAt = startedAt,
                    effort = le.difficulty,
                    hitFullTarget = le.hitFullTarget,
                    skipped = le.skipped,
                    swappedName = le.swappedName,
                    sets = setsByLoggedExercise[le.id].orEmpty().sortedBy { it.setIndex },
                    // The slot is the key; the lift performed IN it is carried alongside, because a
                    // swapped row is filed under the slot and is not the same exercise as its
                    // neighbours. Comparing e1RMs across a swap is comparing two lifts (H-06).
                    performedExerciseId = le.exerciseId,
                    sessionType = session.sessionType
                )
            }
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, bouts) -> bouts.sortedBy { it.sessionStartedAt } }

        val programSnap = program.map { day ->
            ProgramDaySnap(
                dayKey = day.key,
                name = day.defaultName,
                slots = day.exercises.map { plan ->
                    ProgramSlotSnap(
                        exerciseId = plan.id,
                        name = plan.name,
                        muscle = plan.muscle,
                        unit = plan.unit,
                        tags = plan.tags,
                        targetSets = plan.sets,
                        repsText = plan.reps,
                        swapCandidateIds = swapCandidateIds(plan)
                    )
                }
            )
        }

        return AdaptationSnapshot(
            nowMs = nowMs,
            zoneId = zoneId,
            program = programSnap,
            sessions = orderedSessions,
            exerciseHistory = history,
            moods = moods.sortedByDescending { it.recordedAt },
            cardio = cardio.sortedByDescending { it.date },
            bodyweight = bodyweight.sortedByDescending { it.recordedAt },
            prefs = prefs,
            health = health
        )
    }
}
