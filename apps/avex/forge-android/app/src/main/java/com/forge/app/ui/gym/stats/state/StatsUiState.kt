package com.forge.app.ui.gym.stats.state

import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import java.time.LocalDate
import java.time.YearMonth

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
    val totals: Totals = Totals(),
    val heatmap: List<HeatmapCell> = emptyList(),
    val volumeByMuscle: List<MuscleVolume> = emptyList(),
    val strengthCurve: StrengthCurve? = null,
    val recentPrs: List<PrEntry> = emptyList(),
    val hallOfFame: List<PrRecord> = emptyList(),
    val exerciseHistory: Map<String, List<HistoryPoint>> = emptyMap(),
    val monthCalendar: MonthCalendarData? = null,
    /** Sessions per exercise in past 8 weeks (#73). */
    val exerciseFrequency: List<ExerciseFrequency> = emptyList(),
    /** Avg days between PRs per exercise — only exercises with 2+ PRs (#74). */
    val timeToPr: List<TimeToPrEntry> = emptyList(),
    /** Weekly effort distribution: EASY/JUST_RIGHT/HARD/BRUTAL counts per week (#75). */
    val effortDistribution: List<WeeklyEffortCounts> = emptyList(),
    /** PR count by day of week (Mon–Sun, index 0=Mon) (#85). */
    val prsByDayOfWeek: List<Int> = List(7) { 0 },
    /** Sessions with volume + deload marker for the trend chart (#126). */
    val volumeDeloadTrend: List<VolumeDeloadPoint> = emptyList(),
    /** Best vs average volume per day type (#132). */
    val dayTypeBestVsAvg: List<DayTypeVolumeStats> = emptyList(),
    /** This week vs last week comparison (#34). */
    val weekComparison: PeriodComparison? = null,
    /** This month vs last month comparison (#130). */
    val monthComparison: PeriodComparison? = null,
    /** Year-over-year peak weight per exercise for exercises with 12+ months of data (#131). */
    val exerciseYoY: List<ExerciseYoY> = emptyList(),
    /** Volume per session per exercise for the volume-over-time chart (#72). */
    val exerciseVolumeHistory: Map<String, List<VolumePoint>> = emptyMap(),
    /** Max weight per compound exercise for the radar chart (#124): exerciseId → max lb. */
    val compoundMaxes: Map<String, Double> = emptyMap(),
    /** PR session timestamps for clustering scatter (#128). */
    val prSessionTimestamps: List<Long> = emptyList(),
    /** Mood over time for effort chart (#95). */
    val moodOverTime: List<com.forge.app.data.db.dao.SessionDao.MoodOverTime> = emptyList(),
    /** Behavioral insight flags (#41, #80). */
    val insights: List<InsightFlag> = emptyList(),
    /** Per-day-type breakdown (#134). */
    val dayTypeBreakdown: List<DayTypeBreakdown> = emptyList(),
    /** Lifetime session metrics (#40). */
    val lifetimeMetrics: LifetimeMetrics? = null,
    /** Mon–Sun rows for "What I did this week" editorial section. Always 7 entries. */
    val weekActivity: List<WeekActivityRow> = emptyList(),
    /** Cardio minutes this ISO week (excludes rest-type entries). */
    val thisWeekCardioMin: Int = 0
)

data class Totals(
    val workouts: Int = 0,
    val exercisesLogged: Int = 0,
    val prs: Int = 0
)

/** One day cell in the frequency heatmap. [count] = number of exercises logged that day. */
data class HeatmapCell(
    val date: LocalDate,
    val count: Int
)

/** Per-muscle weekly volume, sorted descending by volume in the repository. */
data class MuscleVolume(
    val muscle: MuscleGroup,
    val volumeLb: Double
)

/** Up to ~10 most recent (max-weight-per-session) data points for a single exercise. */
data class StrengthCurve(
    val plan: ExercisePlan,
    val points: List<Double>
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

/** Mini summary for a single day in the monthly calendar (#54). */
data class SessionDaySummary(
    val dayName: String,
    val totalVolumeLb: Double,
    val prCount: Int
)

/** Monthly training calendar: one entry per day of month that had a session (#54). */
data class MonthCalendarData(
    val yearMonth: YearMonth,
    val sessionDays: Map<Int, SessionDaySummary>,
    /** Day-of-month → rest day type for days marked as intentional rest (#114 #115). */
    val restDays: Map<Int, String> = emptyMap()
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

/** Year-over-year peak weight for one exercise (#131). */
data class ExerciseYoY(
    val exerciseId: String,
    val exerciseName: String,
    val thisYearMaxLb: Double,
    val lastYearMaxLb: Double
) {
    val delta: Double get() = thisYearMaxLb - lastYearMaxLb
}

/** One data point for the volume-over-time chart (#72). */
data class VolumePoint(
    val sessionDate: Long,
    val totalVolumeLb: Double
)

/** Behavioral insight flag — shown in the Stats/Overview screen (#41, #80). */
data class InsightFlag(
    val emoji: String,
    val title: String,
    val body: String
)

/** Per-day-type breakdown row (#134). */
data class DayTypeBreakdown(
    val dayKey: String,
    val dayName: String,
    val avgDurationMin: Int,
    val prRate: Double,
    val skipRate: Double,
    val sessionCount: Int
)

/** Session efficiency / lifetime metrics row (#40). */
data class LifetimeMetrics(
    val lifetimeVolumeLb: Double,
    val totalSessions: Int,
    val avgSessionVolumeLb: Double,
    val avgSetCount: Double
)
