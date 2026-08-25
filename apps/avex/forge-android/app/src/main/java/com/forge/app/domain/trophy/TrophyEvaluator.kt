package com.forge.app.domain.trophy

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
import com.forge.app.program.UnlockRule

object TrophyEvaluator {

    /**
     * CALENDAR days since the first finished session (0 when there is none yet). Single source for
     * the anniversary trophy's match/progress/remaining math, so the day-conversion lives in one
     * place.
     *
     * Counted between dates, not by dividing elapsed milliseconds. A first session on 2025-06-15 at
     * 21:00 was 364 days old at 09:00 on 2026-06-15 — the user's anniversary by any calendar — so
     * the "One year" trophy stayed locked until nine in the evening. The same truncation is why
     * `TrophyRepository.checkComebackKid` moved to ChronoUnit before this did.
     */
    private fun trainingDaysElapsed(s: TrophyStatsSnapshot): Int {
        val first = s.firstSessionMs ?: return 0
        return java.time.temporal.ChronoUnit.DAYS.between(
            java.time.Instant.ofEpochMilli(first).atZone(s.zoneId).toLocalDate(),
            java.time.Instant.ofEpochMilli(s.nowMs).atZone(s.zoneId).toLocalDate()
        ).toInt().coerceAtLeast(0)
    }

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
        is UnlockRule.ExerciseGoalsAchievedAtLeast -> s.exerciseGoalsAchieved >= rule.n
        is UnlockRule.LifetimeTonnageAtLeast -> s.lifetimeTonnageLb >= rule.lb
        is UnlockRule.TrainingAnniversaryRule -> trainingDaysElapsed(s) >= 365
        is UnlockRule.CardioSessionsAtLeast -> s.cardioSessions >= rule.n
        is UnlockRule.CardioDistanceAtLeastKm -> s.cardioDistanceKm >= rule.km
    }

    fun progressHint(rule: UnlockRule, s: TrophyStatsSnapshot, unit: WeightUnit): String? = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> "${s.totalLoggedExercises} / ${rule.n}"
        is UnlockRule.TotalPRsAtLeast -> "${s.totalPrs} / ${rule.n}"
        is UnlockRule.BrutalCountAtLeast -> "${s.brutalRatings} / ${rule.n}"
        is UnlockRule.SwapCountAtLeast -> "${s.swapsUsed} / ${rule.n}"
        is UnlockRule.FullTargetHitsAtLeast -> "${s.fullTargetHits} / ${rule.n}"
        is UnlockRule.WorkoutsCompletedAtLeast -> "${s.finishedSessions} / ${rule.n}"
        is UnlockRule.DistinctDaysTrainedAtLeast -> "${s.distinctDayKeysTrained} / ${rule.n} days"
        is UnlockRule.MaxBenchAtLeast -> "${toDisplayWeight(s.maxBenchLb, unit).toInt()} / ${toDisplayWeight(rule.lb, unit).toInt()} ${unitLabel(unit)}"
        is UnlockRule.MaxSquatAtLeast -> "${toDisplayWeight(s.maxSquatLb, unit).toInt()} / ${toDisplayWeight(rule.lb, unit).toInt()} ${unitLabel(unit)}"
        is UnlockRule.MaxSessionVolumeAtLeast -> "${toDisplayWeight(s.maxSessionVolumeLb, unit).toInt()} / ${toDisplayWeight(rule.lb, unit).toInt()} ${unitLabel(unit)}"
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
        is UnlockRule.ExerciseGoalsAchievedAtLeast -> "${s.exerciseGoalsAchieved} / ${rule.n} goals"
        is UnlockRule.LifetimeTonnageAtLeast -> "${toDisplayWeight(s.lifetimeTonnageLb, unit).toLong()} / ${toDisplayWeight(rule.lb, unit).toLong()} ${unitLabel(unit)}"
        is UnlockRule.TrainingAnniversaryRule -> "${trainingDaysElapsed(s)} / 365 days"
        is UnlockRule.CardioSessionsAtLeast -> "${s.cardioSessions} / ${rule.n} sessions"
        is UnlockRule.CardioDistanceAtLeastKm -> "${s.cardioDistanceKm.toInt()} / ${rule.km.toInt()} km"
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
        is UnlockRule.ExerciseGoalsAchievedAtLeast -> (s.exerciseGoalsAchieved.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.LifetimeTonnageAtLeast -> (s.lifetimeTonnageLb / rule.lb).coerceIn(0.0, 1.0).toFloat()
        is UnlockRule.TrainingAnniversaryRule -> (trainingDaysElapsed(s) / 365f).coerceIn(0f, 1f)
        is UnlockRule.CardioSessionsAtLeast -> (s.cardioSessions.toFloat() / rule.n).coerceIn(0f, 1f)
        is UnlockRule.CardioDistanceAtLeastKm -> (s.cardioDistanceKm / rule.km).coerceIn(0.0, 1.0).toFloat()
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
        is UnlockRule.ExerciseGoalsAchievedAtLeast -> s.exerciseGoalsAchieved to rule.n
        is UnlockRule.CardioSessionsAtLeast -> s.cardioSessions to rule.n
        is UnlockRule.CardioDistanceAtLeastKm -> s.cardioDistanceKm.toInt() to rule.km.toInt()
        // Tonnage (lb) and anniversary (days) are near-miss-eligible too, so the Overview "Up next"
        // hook can surface them — their progressFraction/Hint already report progress.
        is UnlockRule.LifetimeTonnageAtLeast -> s.lifetimeTonnageLb.toInt() to rule.lb.toInt()
        is UnlockRule.TrainingAnniversaryRule -> trainingDaysElapsed(s) to 365
        else -> -1 to -1
    }

    fun progressRemaining(rule: UnlockRule, s: TrophyStatsSnapshot, unit: WeightUnit): String? = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> "${rule.n - s.totalLoggedExercises} exercises"
        is UnlockRule.TotalPRsAtLeast -> "${rule.n - s.totalPrs} PRs"
        is UnlockRule.BrutalCountAtLeast -> "${rule.n - s.brutalRatings} brutal ratings"
        is UnlockRule.SwapCountAtLeast -> "${rule.n - s.swapsUsed} swaps"
        is UnlockRule.FullTargetHitsAtLeast -> "${rule.n - s.fullTargetHits} full-target sets"
        is UnlockRule.WorkoutsCompletedAtLeast -> "${rule.n - s.finishedSessions} workouts"
        is UnlockRule.DistinctDaysTrainedAtLeast -> "${rule.n - s.distinctDayKeysTrained} day types"
        is UnlockRule.MaxBenchAtLeast -> "${toDisplayWeight((rule.lb - s.maxBenchLb).coerceAtLeast(0.0), unit).toInt()} ${unitLabel(unit)} on bench"
        is UnlockRule.MaxSquatAtLeast -> "${toDisplayWeight((rule.lb - s.maxSquatLb).coerceAtLeast(0.0), unit).toInt()} ${unitLabel(unit)} on goblet"
        is UnlockRule.MaxSessionVolumeAtLeast -> "${toDisplayWeight((rule.lb - s.maxSessionVolumeLb).coerceAtLeast(0.0), unit).toInt()} ${unitLabel(unit)} session volume"
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
        is UnlockRule.ExerciseGoalsAchievedAtLeast -> "${rule.n - s.exerciseGoalsAchieved} more goal(s)"
        is UnlockRule.LifetimeTonnageAtLeast -> "${toDisplayWeight((rule.lb - s.lifetimeTonnageLb).coerceAtLeast(0.0), unit).toLong()} ${unitLabel(unit)} to go"
        is UnlockRule.TrainingAnniversaryRule -> {
            val remaining = (365 - trainingDaysElapsed(s)).coerceAtLeast(0)
            if (remaining == 0) null else "$remaining more days"
        }
        is UnlockRule.CardioSessionsAtLeast -> "${(rule.n - s.cardioSessions).coerceAtLeast(0)} more"
        is UnlockRule.CardioDistanceAtLeastKm -> "${(rule.km - s.cardioDistanceKm).coerceAtLeast(0.0).toInt()} km to go"
    }
}
