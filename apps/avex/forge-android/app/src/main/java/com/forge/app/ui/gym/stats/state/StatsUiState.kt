package com.forge.app.ui.gym.stats.state

import com.forge.app.program.MuscleGroup

/**
 * State for the rebuilt Gym → Stats screen. Trimmed to exactly what the four tabs render
 * (Strength / Volume / Body / Trends) — fields the old long-scroll screen carried but no current
 * chart reads were dropped along with their repository computation (see [StatsRepository.GymStats]).
 */
data class StatsUiState(
    val isLoading: Boolean = true,
    /** True when a stats aggregation threw — the screen shows an error message, not a silent empty. */
    val loadError: Boolean = false,
    /** PRs on a time axis + the most-recent spelled out (Strength tab). */
    val recentPrs: List<PrEntry> = emptyList(),
    /** Estimated 1RM per main lift — current value + per-session history (Strength tab). */
    val e1rmLifts: List<E1rmLift> = emptyList(),
    /** Per-exercise load-rep scatter + e1RM for the strength curve (Strength tab). */
    val strengthCurves: List<StrengthCurve> = emptyList(),
    /** Working sets per muscle this ISO week (Volume tab). */
    val weeklySetsByMuscle: List<MuscleSetCount> = emptyList(),
    /** Planned weekly sets per muscle from the active program (Volume tab). */
    val plannedSetsByMuscle: Map<MuscleGroup, Int> = emptyMap(),
    /** Tonnage per ISO week, deload weeks marked (Volume trend). */
    val weeklyTonnage: List<WeeklyTonnage> = emptyList(),
    /** Always-on push/pull + quad/ham balance bars (Volume tab, System 4 counting). */
    val balanceRatios: List<BalanceRatioUi> = emptyList(),
    /** Dated bodyweight points, oldest → newest — feeds the Body tab's time-axis trend. */
    val bodyweightPoints: List<BodyweightPoint> = emptyList(),
    /** User's sex ("male" | "female" | "") — selects the bodyweight-relative strength bands (Body tab). */
    val userSex: String = "",
    /** Fatigue pulse (System 5). Null until the engine read lands or while data gates fail (Body tab). */
    val readinessPulse: ReadinessPulse? = null,
    /** The learned deload score threshold — draws the threshold line + bands on the fatigue gauge (Body tab). */
    val readinessThreshold: Int? = null,
    /** Per-day training load for the adherence calendar + Banister form curves (Trends tab). */
    val dailyActivity: List<DayLoad> = emptyList(),
    /** Count of sets at each RPE value + overall average (Trends tab). */
    val rpeDistribution: List<RpeBucket> = emptyList(),
    val avgRpe: Double? = null,
    /** Sessions per day of week + best training hour (Trends tab). */
    val trainingTimes: TrainingTimes? = null,
    /** PR count by day of week (Mon–Sun, index 0=Mon) (Trends tab). */
    val prsByDayOfWeek: List<Int> = List(7) { 0 },
    /** This ISO week vs last week, side by side (Overview tab). */
    val weekComparison: PeriodComparison? = null,
    /** All-time best set per lift, heaviest first (Overview "records"). */
    val hallOfFame: List<PrRecord> = emptyList(),
    /** Lifetime totals for the Overview at-a-glance tiles. */
    val lifetime: LifetimeMetrics? = null
)

/** One ISO week's total tonnage for the Volume trend bars. */
data class WeeklyTonnage(
    val weekLabel: String,
    val volumeLb: Double,
    val isDeload: Boolean
)

/** Sessions per day of week (Mon..Sun) + the sets-weighted best training hour. */
data class TrainingTimes(
    val sessionsByDayOfWeek: List<Int>,
    /** e.g. "evening (18:00)" — null until enough sets accrue. */
    val bestHourLabel: String?
)

/** Median session length for one ISO week. */
data class WeeklyDuration(
    val weekLabel: String,
    val medianMin: Int
)

/** One dated bodyweight entry for the Body trend line. */
data class BodyweightPoint(
    val recordedAt: Long,
    val weightLb: Double
)

/** Weekly average of per-lift best e1RM across the tracked lifts, oldest → newest. */
data class OverloadSummary(
    /** Last week-with-data's average e1RM. */
    val current: Double,
    /** Avg e1RM per ISO week that had data, oldest → newest (≤ 12 weeks). */
    val weekly: List<Double>
) {
    val prevWeek: Double? get() = if (weekly.size >= 2) weekly[weekly.size - 2] else null
    val deltaVsPrevWeek: Double? get() = prevWeek?.let { current - it }
}

/** Days since the last PR — overall and per exercise. */
data class PrRecency(
    val daysSinceLast: Int,
    val byExercise: Map<String, Int>
)

/** One axis of the movement-pattern radar: recent-window best vs all-time best e1RM. */
data class PatternAxis(
    val label: String,
    val currentE1rm: Double,
    val peakE1rm: Double
) {
    /** Current shape vs your own peak — the only honest cross-pattern comparison. */
    val fraction: Double get() = if (peakE1rm > 0) (currentE1rm / peakE1rm).coerceIn(0.0, 1.0) else 0.0
}

/** Working-set count for one muscle group in the current week (Phase 2). */
data class MuscleSetCount(val muscle: MuscleGroup, val sets: Int) {
    /** Rudimentary landmark guidance: under ~10 = low, over ~20 = high. */
    val low: Int get() = 10
    val high: Int get() = 20
}

/** Number of sets logged at a given RPE value (Phase 3). */
data class RpeBucket(val rpe: Double, val count: Int)

/** Estimated 1RM progression for one lift (Epley: w × (1 + reps/30)). */
data class E1rmLift(
    val exerciseId: String,
    val exerciseName: String,
    val currentE1rm: Double,
    /** Best e1RM per session, oldest → newest. */
    val history: List<Double>,
    /** Average growth per month, as a percent. Null if <2 sessions. */
    val monthlyPct: Double? = null,
    /** True when the last few sessions have been flat (no meaningful progress). */
    val stalling: Boolean = false
) {
    /** Change from first recorded e1RM to current. */
    val delta: Double get() = if (history.size >= 2) currentE1rm - history.first() else 0.0
}

data class PrEntry(
    val date: Long,
    val exerciseName: String,
    val weightLb: Double,
    val reps: Int
)

/** All-time best set per exercise, shown in the Records hall-of-fame (#14). */
data class PrRecord(
    val exerciseId: String,
    val exerciseName: String,
    val maxWeightLb: Double,
    val bestReps: Int,
    val sessionDate: Long,
    val muscle: MuscleGroup,
    /** Relative strength as multiple of bodyweight. Null if no bodyweight logged (#77). */
    val relativeStrength: Double? = null
)

/** One point on the volume trend chart, with a deload marker (#126). */
data class VolumeDeloadPoint(
    val sessionDate: Long,
    val dayKey: String,
    val totalVolumeLb: Double,
    val isDeload: Boolean
)

/** Stats for a time window (week or month) used for period-over-period comparison (#34, #130). */
data class PeriodStats(
    val sessions: Int,
    val volumeLb: Double,
    val prs: Int,
    val sets: Int
)

/** Side-by-side comparison of two consecutive periods (#34 week, #130 month). */
data class PeriodComparison(
    val label: String,
    val current: PeriodStats,
    val previous: PeriodStats
) {
    val volumeDelta: Double get() = current.volumeLb - previous.volumeLb
    val sessionsDelta: Int get() = current.sessions - previous.sessions
}

/** Session efficiency / lifetime metrics row (#40). */
data class LifetimeMetrics(
    val lifetimeVolumeLb: Double,
    val totalSessions: Int,
    val avgSessionVolumeLb: Double,
    val avgSetCount: Double
)
