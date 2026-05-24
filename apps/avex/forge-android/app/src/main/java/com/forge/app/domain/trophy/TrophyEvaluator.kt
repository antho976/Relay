package com.forge.app.domain.trophy

import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
import com.forge.app.program.UnlockRule

object TrophyEvaluator {

    fun unlockedByRule(snapshot: TrophyStatsSnapshot, catalogue: List<Trophy> = Trophies.all): Set<String> =
        catalogue.asSequence()
            .filter { isUnlocked(it.unlock, snapshot) }
            .map { it.id }
            .toSet()

    fun isUnlocked(rule: UnlockRule, s: TrophyStatsSnapshot): Boolean = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> s.totalLoggedExercises >= rule.n
        is UnlockRule.TotalPRsAtLeast -> s.totalPrs >= rule.n
        is UnlockRule.BrutalCountAtLeast -> s.brutalRatings >= rule.n
        is UnlockRule.SwapCountAtLeast -> s.swapsUsed >= rule.n
        is UnlockRule.FullTargetHitsAtLeast -> s.fullTargetHits >= rule.n
        is UnlockRule.WorkoutsCompletedAtLeast -> s.finishedSessions >= rule.n
        is UnlockRule.DistinctDaysTrainedAtLeast -> s.distinctDayKeysTrained >= rule.n
        is UnlockRule.MaxBenchAtLeast -> s.maxBenchLb >= rule.lb
        is UnlockRule.MaxSquatAtLeast -> s.maxSquatLb >= rule.lb
        is UnlockRule.MaxSessionVolumeAtLeast -> s.maxSessionVolumeLb >= rule.lb
        is UnlockRule.MaxStreakAtLeast -> s.maxStreakEver >= rule.days
        is UnlockRule.EarlyBirdSessionsAtLeast -> s.earlyBirdSessions >= rule.n
        is UnlockRule.NightOwlSessionsAtLeast -> s.nightOwlSessions >= rule.n
        is UnlockRule.SundaysTrainedAtLeast -> s.sundaysTrainedCount >= rule.n
        is UnlockRule.SessionDurationAtLeast -> s.maxSessionDurationMinutes >= rule.minutes
        is UnlockRule.SessionDurationAtMost -> s.minFinishedSessionDurationMinutes in 5..rule.minutes
        is UnlockRule.MaxSingleExerciseRepsAtLeast -> s.maxSingleExerciseReps >= rule.n
        is UnlockRule.ComebackKidRule -> s.comebackKidEarned
        is UnlockRule.ConsistencyKingRule -> s.consistencyKingEarned
        is UnlockRule.VarietyPackRule -> s.varietyPackEarned
    }

    fun progressHint(rule: UnlockRule, s: TrophyStatsSnapshot): String? = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> "${s.totalLoggedExercises} / ${rule.n}"
        is UnlockRule.TotalPRsAtLeast -> "${s.totalPrs} / ${rule.n}"
        is UnlockRule.BrutalCountAtLeast -> "${s.brutalRatings} / ${rule.n}"
        is UnlockRule.SwapCountAtLeast -> "${s.swapsUsed} / ${rule.n}"
        is UnlockRule.FullTargetHitsAtLeast -> "${s.fullTargetHits} / ${rule.n}"
        is UnlockRule.WorkoutsCompletedAtLeast -> "${s.finishedSessions} / ${rule.n}"
        is UnlockRule.DistinctDaysTrainedAtLeast -> "${s.distinctDayKeysTrained} / ${rule.n} days"
        is UnlockRule.MaxBenchAtLeast -> "${s.maxBenchLb.toInt()} / ${rule.lb.toInt()} lb"
        is UnlockRule.MaxSquatAtLeast -> "${s.maxSquatLb.toInt()} / ${rule.lb.toInt()} lb"
        is UnlockRule.MaxSessionVolumeAtLeast -> "${s.maxSessionVolumeLb.toInt()} / ${rule.lb.toInt()} lb"
        is UnlockRule.MaxStreakAtLeast -> "${s.maxStreakEver} / ${rule.days} days"
        is UnlockRule.EarlyBirdSessionsAtLeast -> "${s.earlyBirdSessions} / ${rule.n} sessions"
        is UnlockRule.NightOwlSessionsAtLeast -> "${s.nightOwlSessions} / ${rule.n} sessions"
        is UnlockRule.SundaysTrainedAtLeast -> "${s.sundaysTrainedCount} / ${rule.n} Sundays"
        is UnlockRule.SessionDurationAtLeast -> "${s.maxSessionDurationMinutes} / ${rule.minutes} min"
        is UnlockRule.SessionDurationAtMost -> "${s.minFinishedSessionDurationMinutes.takeIf { it < Int.MAX_VALUE } ?: 0} min best"
        is UnlockRule.MaxSingleExerciseRepsAtLeast -> "${s.maxSingleExerciseReps} / ${rule.n} reps"
        is UnlockRule.ComebackKidRule -> if (s.comebackKidEarned) "Earned" else "PR after 5+ day gap"
        is UnlockRule.ConsistencyKingRule -> if (s.consistencyKingEarned) "Earned" else "No missed week in 3 months"
        is UnlockRule.VarietyPackRule -> if (s.varietyPackEarned) "Earned" else "Train all 4 days in one week"
    }

    fun progressFraction(rule: UnlockRule, s: TrophyStatsSnapshot): Float = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> (s.totalLoggedExercises.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.TotalPRsAtLeast -> (s.totalPrs.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.BrutalCountAtLeast -> (s.brutalRatings.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.SwapCountAtLeast -> (s.swapsUsed.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.FullTargetHitsAtLeast -> (s.fullTargetHits.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.WorkoutsCompletedAtLeast -> (s.finishedSessions.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.DistinctDaysTrainedAtLeast -> (s.distinctDayKeysTrained.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.MaxBenchAtLeast -> (s.maxBenchLb / rule.lb).coerceIn(0.0, 1.0).toFloat()
        is UnlockRule.MaxSquatAtLeast -> (s.maxSquatLb / rule.lb).coerceIn(0.0, 1.0).toFloat()
        is UnlockRule.MaxSessionVolumeAtLeast -> (s.maxSessionVolumeLb / rule.lb).coerceIn(0.0, 1.0).toFloat()
        is UnlockRule.MaxStreakAtLeast -> (s.maxStreakEver.toFloat() / rule.days).coerceIn(0f, 1f)
        is UnlockRule.EarlyBirdSessionsAtLeast -> (s.earlyBirdSessions.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.NightOwlSessionsAtLeast -> (s.nightOwlSessions.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.SundaysTrainedAtLeast -> (s.sundaysTrainedCount.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.SessionDurationAtLeast -> (s.maxSessionDurationMinutes.toFloat() / rule.minutes).coerceIn(0f, 1f)
        is UnlockRule.SessionDurationAtMost -> if (s.minFinishedSessionDurationMinutes <= rule.minutes) 1f else 0f
        is UnlockRule.MaxSingleExerciseRepsAtLeast -> (s.maxSingleExerciseReps.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.ComebackKidRule -> if (s.comebackKidEarned) 1f else 0f
        is UnlockRule.ConsistencyKingRule -> if (s.consistencyKingEarned) 1f else 0f
        is UnlockRule.VarietyPackRule -> if (s.varietyPackEarned) 1f else 0f
    }

    /** Returns (currentProgress, target) as integers for near-miss detection (#136). -1 = not applicable. */
    fun progressFor(trophy: com.forge.app.program.Trophy, s: TrophyStatsSnapshot): Pair<Int, Int> = when (val rule = trophy.unlock) {
        is UnlockRule.TotalSessionsAtLeast -> s.totalLoggedExercises to rule.n
        is UnlockRule.TotalPRsAtLeast -> s.totalPrs to rule.n
        is UnlockRule.BrutalCountAtLeast -> s.brutalRatings to rule.n
        is UnlockRule.SwapCountAtLeast -> s.swapsUsed to rule.n
        is UnlockRule.FullTargetHitsAtLeast -> s.fullTargetHits to rule.n
        is UnlockRule.WorkoutsCompletedAtLeast -> s.finishedSessions to rule.n
        is UnlockRule.DistinctDaysTrainedAtLeast -> s.distinctDayKeysTrained to rule.n
        is UnlockRule.MaxStreakAtLeast -> s.maxStreakEver to rule.days
        is UnlockRule.EarlyBirdSessionsAtLeast -> s.earlyBirdSessions to rule.n
        is UnlockRule.NightOwlSessionsAtLeast -> s.nightOwlSessions to rule.n
        is UnlockRule.SundaysTrainedAtLeast -> s.sundaysTrainedCount to rule.n
        is UnlockRule.MaxSingleExerciseRepsAtLeast -> s.maxSingleExerciseReps to rule.n
        else -> -1 to -1
    }

    fun progressRemaining(rule: UnlockRule, s: TrophyStatsSnapshot): String? = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> "${rule.n - s.totalLoggedExercises} exercises"
        is UnlockRule.TotalPRsAtLeast -> "${rule.n - s.totalPrs} PRs"
        is UnlockRule.BrutalCountAtLeast -> "${rule.n - s.brutalRatings} brutal ratings"
        is UnlockRule.SwapCountAtLeast -> "${rule.n - s.swapsUsed} swaps"
        is UnlockRule.FullTargetHitsAtLeast -> "${rule.n - s.fullTargetHits} full-target sets"
        is UnlockRule.WorkoutsCompletedAtLeast -> "${rule.n - s.finishedSessions} workouts"
        is UnlockRule.DistinctDaysTrainedAtLeast -> "${rule.n - s.distinctDayKeysTrained} day types"
        is UnlockRule.MaxBenchAtLeast -> "${(rule.lb - s.maxBenchLb).toInt()} lb on bench"
        is UnlockRule.MaxSquatAtLeast -> "${(rule.lb - s.maxSquatLb).toInt()} lb on goblet"
        is UnlockRule.MaxSessionVolumeAtLeast -> "${(rule.lb - s.maxSessionVolumeLb).toInt()} lb session volume"
        is UnlockRule.MaxStreakAtLeast -> "${rule.days - s.maxStreakEver} more consecutive days"
        is UnlockRule.EarlyBirdSessionsAtLeast -> "${rule.n - s.earlyBirdSessions} early sessions"
        is UnlockRule.NightOwlSessionsAtLeast -> "${rule.n - s.nightOwlSessions} night sessions"
        is UnlockRule.SundaysTrainedAtLeast -> "${rule.n - s.sundaysTrainedCount} Sundays"
        is UnlockRule.SessionDurationAtLeast -> "${rule.minutes - s.maxSessionDurationMinutes} min longer session"
        is UnlockRule.SessionDurationAtMost -> null
        is UnlockRule.MaxSingleExerciseRepsAtLeast -> "${rule.n - s.maxSingleExerciseReps} more reps"
        is UnlockRule.ComebackKidRule -> null
        is UnlockRule.ConsistencyKingRule -> null
        is UnlockRule.VarietyPackRule -> null
    }
}
