package com.forge.app.ui.gym.stats.state

import com.forge.app.program.MuscleGroup

/** One day row in the "What I did this week" editorial section. */
data class WeekActivityRow(
    val dayOfWeek: Int,            // 0=Mon .. 6=Sun
    val dayLabel: String,          // "MON", "TUE", etc.
    val sessionName: String? = null,
    val muscleWord: String? = null, // "PUSH", "HAMS", etc.
    val durationMin: Int? = null,
    val setCount: Int = 0,
    val hasPr: Boolean = false,
    val cardioType: String? = null, // "Run", "Walk", etc. — null if not cardio
    val cardioDurationMin: Int? = null,
    val cardioDistanceKm: Double? = null
)

data class StatsUiState(
    val isLoading: Boolean = true,
    val volumeByMuscle: List<MuscleVolume> = emptyList(),
    val recentPrs: List<PrEntry> = emptyList(),
    val hallOfFame: List<PrRecord> = emptyList(),
    val exerciseHistory: Map<String, List<HistoryPoint>> = emptyMap(),
    /** Sessions per exercise in past 8 weeks (#73). */
    val exerciseFrequency: List<ExerciseFrequency> = emptyList(),
    /** Avg days between PRs per exercise — only exercises with 2+ PRs (#74). */
    val timeToPr: List<TimeToPrEntry> = emptyList(),
    /** Weekly effort distribution: EASY/JUST_RIGHT/HARD/BRUTAL counts per week (#75). */
    val effortDistribution: List<WeeklyEffortCounts> = emptyList(),
    /** PR count by day of week (Mon–Sun, index 0=Mon) (#85). */
    val prsByDayOfWeek: List<Int> = List(7) { 0 },
    /** Best vs average volume per day type (#132). */
    val dayTypeBestVsAvg: List<DayTypeVolumeStats> = emptyList(),
    /** This week vs last week comparison (#34). */
    val weekComparison: PeriodComparison? = null,
    /** PR session timestamps (#128) — feeds PR recency. */
    val prSessionTimestamps: List<Long> = emptyList(),
    /** Mood over time for effort chart (#95). */
    val moodOverTime: List<com.forge.app.data.db.dao.SessionDao.MoodOverTime> = emptyList(),
    /** Behavioral insight flags (#41, #80). */
    val insights: List<InsightFlag> = emptyList(),
    /** Lifetime session metrics (#40). */
    val lifetimeMetrics: LifetimeMetrics? = null,
    /** Mon–Sun rows for "What I did this week" editorial section. Always 7 entries. */
    val weekActivity: List<WeekActivityRow> = emptyList(),
    /** Cardio minutes this ISO week (excludes rest-type entries). */
    val thisWeekCardioMin: Int = 0,
    /** Estimated 1RM per main lift — current value + per-session history (Phase 1b). */
    val e1rmLifts: List<E1rmLift> = emptyList(),
    /** Best weight at each rep count for the most-trained lift (Phase 1b). */
    val repMaxes: RepMaxSet? = null,
    /** Working sets per muscle this ISO week vs the productive range (Phase 2). */
    val weeklySetsByMuscle: List<MuscleSetCount> = emptyList(),
    /** Distribution of logged sets across rep ranges (Phase 2). */
    val repRangeDist: RepRangeDist? = null,
    /** Count of sets at each RPE value + overall average (Phase 3). */
    val rpeDistribution: List<RpeBucket> = emptyList(),
    val avgRpe: Double? = null,
    /** Dated bodyweight points, oldest → newest — feeds the Body tab's time-axis trend. */
    val bodyweightPoints: List<BodyweightPoint> = emptyList(),
    /** Consecutive recent weeks meeting the session target (Snapshot). */
    val consistencyStreakWeeks: Int = 0,
    /** Avg estimated-1RM growth per month across lifts, as a percent (Snapshot). */
    val progressiveOverloadPct: Double? = null,
    /** Average RPE per session, oldest → newest (Trends RPE line). */
    val avgRpePerSession: List<Double> = emptyList(),
    /** Session count per ISO week for the last ~12 weeks, oldest → newest (Trends consistency). */
    val weeklySessionCounts: List<Int> = emptyList(),
    /** Weekly avg e1RM across tracked lifts — the concrete "progressive overload" series (Snapshot). */
    val overload: OverloadSummary? = null,
    /** Days since the most recent PR, overall + per lift (Strength). */
    val prRecency: PrRecency? = null,
    /** Movement-pattern radar: recent best vs all-time best e1RM per pattern (Strength). */
    val patternRadar: List<PatternAxis> = emptyList(),
    // Engine read — loaded once per Stats open via AdaptationRepository.engineStatsRead().
    /** Fatigue pulse (System 5). Null until the engine read lands or while data gates fail. */
    val readinessPulse: ReadinessPulse? = null,
    /** Plateau ladder flags (System 1) keyed to lifts on the Strength tab. */
    val plateauFlags: List<PlateauFlagUi> = emptyList(),
    /** Always-on push/pull + quad/ham balance bars (System 4 counting). */
    val balanceRatios: List<BalanceRatioUi> = emptyList(),
    /** Planned weekly sets per muscle from the active program (VolumeTargets). */
    val plannedSetsByMuscle: Map<MuscleGroup, Int> = emptyMap(),
    /** Tonnage per ISO week, deload weeks marked (Volume trend). */
    val weeklyTonnage: List<WeeklyTonnage> = emptyList(),
    /** Sessions per day of week + best training hour (Trends "When you train"). */
    val trainingTimes: TrainingTimes? = null,
    /** Median session length per ISO week (Trends duration trend). */
    val weeklyDurations: List<WeeklyDuration> = emptyList()
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

/** How logged sets split across rep ranges (Phase 2). */
data class RepRangeDist(val strength: Int, val hypertrophy: Int, val endurance: Int) {
    val total: Int get() = strength + hypertrophy + endurance
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

/** Best weight achieved at a given rep count. */
data class RepMaxEntry(val reps: Int, val weightLb: Double)

/** Rep-max table for a single exercise. */
data class RepMaxSet(val exerciseName: String, val entries: List<RepMaxEntry>)

/** Per-muscle weekly volume, sorted descending by volume in the repository. */
data class MuscleVolume(
    val muscle: MuscleGroup,
    val volumeLb: Double
)

data class PrEntry(
    val date: Long,
    val exerciseName: String,
    val weightText: String,
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
) {
    val weightText: String get() = "${maxWeightLb.toInt()} lb"
}

/** One data point in the per-exercise weight history (#27). */
data class HistoryPoint(
    val sessionDate: Long,
    val maxWeightLb: Double
)

/** How many sessions in past 8 weeks included a given exercise (#73). */
data class ExerciseFrequency(
    val exerciseId: String,
    val exerciseName: String,
    val sessionCount: Int,
    val outOf: Int
)

/** Average days between consecutive PRs for an exercise (#74). */
data class TimeToPrEntry(
    val exerciseId: String,
    val exerciseName: String,
    val avgDaysBetween: Int,
    val prCount: Int
)

/** EASY/JUST_RIGHT/HARD/BRUTAL counts for a single ISO-week (#75). */
data class WeeklyEffortCounts(
    val weekLabel: String,
    val easy: Int,
    val justRight: Int,
    val hard: Int,
    val brutal: Int
) {
    val total: Int get() = easy + justRight + hard + brutal
}

/** One point on the volume trend chart, with a deload marker (#126). */
data class VolumeDeloadPoint(
    val sessionDate: Long,
    val dayKey: String,
    val totalVolumeLb: Double,
    val isDeload: Boolean
)

/** Best-ever vs average volume for a day type (#132). */
data class DayTypeVolumeStats(
    val dayKey: String,
    val dayName: String,
    val avgVolumeLb: Double,
    val maxVolumeLb: Double,
    val sessionCount: Int
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
    val prsDelta: Int get() = current.prs - previous.prs
}

/** Behavioral insight flag — shown in the Stats/Overview screen (#41, #80). */
data class InsightFlag(
    val emoji: String,
    val title: String,
    val body: String
)

/** Session efficiency / lifetime metrics row (#40). */
data class LifetimeMetrics(
    val lifetimeVolumeLb: Double,
    val totalSessions: Int,
    val avgSessionVolumeLb: Double,
    val avgSetCount: Double
)
