package com.forge.app.domain.adapt

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
        zoneId: java.time.ZoneId = java.time.ZoneOffset.UTC
    ): AdaptationSnapshot {
        val orderedSessions = sessions.sortedBy { it.startedAt }
        val startedAtBySessionId = orderedSessions.associate { it.id to it.startedAt }
        val setsByLoggedExercise = loggedSets.groupBy { it.loggedExerciseId }

        val history = loggedExercises
            .mapNotNull { le ->
                // Drop rows whose parent session isn't in the (finished, tracked) list.
                val startedAt = startedAtBySessionId[le.sessionId] ?: return@mapNotNull null
                le.exerciseId to ExerciseBout(
                    sessionStartedAt = startedAt,
                    effort = le.difficulty,
                    hitFullTarget = le.hitFullTarget,
                    skipped = le.skipped,
                    swappedName = le.swappedName,
                    sets = setsByLoggedExercise[le.id].orEmpty().sortedBy { it.setIndex }
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
            prefs = prefs
        )
    }
}
