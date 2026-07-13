package com.forge.app.ui.gym.session.state

import com.forge.app.data.db.types.EffortRating
import com.forge.app.ui.gym.stats.state.MuscleSetCount

/** The metric the detail-page charts plot. Drives both the session overview and per-exercise charts. */
enum class SessionMetric(val label: String) {
    WEIGHT("Weight"),
    VOLUME("Volume"),
    REPS("Reps"),
    RPE("RPE")
}

/** How the per-exercise set charts render — switchable from the page controls. */
enum class SessionChartStyle(val label: String) {
    BARS("Bars"),
    LINE("Line")
}

/** One logged set, as the detail page renders it. */
data class SetDetail(
    /** 1-based position within the exercise (for the set-table left column). */
    val number: Int,
    val weightLb: Double?,
    /** Raw user text (e.g. "135", "BW") — preserves bodyweight/unit annotations the parsed lb loses. */
    val weightText: String,
    val reps: Int,
    val rpe: Double?,
    val isTopSet: Boolean,
    val isAmrap: Boolean,
    val toFailure: Boolean,
    val isAssisted: Boolean,
    val dropAnnotation: String?,
    /** Advanced set type (null = normal | "warmup" | "drop" | …) — drives the WARM/DROP badges. */
    val setType: String?,
    /** Held time in seconds for a timed-hold set (GYMAP-51); null = a normal rep set. */
    val durationSeconds: Int? = null
) {
    val volumeLb: Double get() = (weightLb ?: 0.0) * reps

    fun metricValue(metric: SessionMetric): Double = when (metric) {
        SessionMetric.WEIGHT -> weightLb ?: 0.0
        SessionMetric.VOLUME -> volumeLb
        SessionMetric.REPS -> reps.toDouble()
        SessionMetric.RPE -> rpe ?: 0.0
    }
}

/** One exercise in the session, with all its sets and per-exercise rollups. */
data class ExerciseDetail(
    val name: String,
    val isPr: Boolean,
    val effort: EffortRating?,
    val hitFullTarget: Boolean,
    val note: String?,
    val topWeightLb: Double?,
    val totalReps: Int,
    val volumeLb: Double,
    /** Best working e1RM (Epley) for this exercise in this session; null when no weighted working set. */
    val e1rmLb: Double?,
    /** True when [e1rmLb] beats every prior session's best working e1RM for this exercise. */
    val e1rmIsBest: Boolean,
    val sets: List<SetDetail>
) {
    /** Avg RPE across the sets that logged one (0 when none) — the overview's per-exercise RPE rollup. */
    val avgRpe: Double get() = sets.mapNotNull { it.rpe }.let { if (it.isEmpty()) 0.0 else it.average() }

    /** The per-exercise rollup for the session overview chart (top weight / total volume / total reps / avg RPE). */
    fun metricValue(metric: SessionMetric): Double = when (metric) {
        SessionMetric.WEIGHT -> topWeightLb ?: 0.0
        SessionMetric.VOLUME -> volumeLb
        SessionMetric.REPS -> totalReps.toDouble()
        SessionMetric.RPE -> avgRpe
    }
}

/**
 * Everything the detail page shows for one session — assembled off the main thread by
 * [com.forge.app.data.repo.StatsRepository.getSessionDetail] and wrapped into [SessionDetailUiState].
 */
data class SessionDetailData(
    val sessionId: Long,
    val title: String,
    val dateMs: Long,
    val tag: String,
    val durationMin: Int?,
    val volumeLb: Double?,
    val prCount: Int,
    val setCount: Int,
    /** Same-training previous session's volume / set count, for the summary-tile trend carets (null = no prior). */
    val prevVolumeLb: Double? = null,
    val prevSetCount: Int? = null,
    val intensity: String,
    /** Stored session-type key (#109); resolve its pill via [com.forge.app.domain.session.SessionType]. */
    val sessionType: String,
    val deload: Boolean,
    val journal: String,
    val avgRpe: Double?,
    /** Working-set count per muscle group worked in this session, busiest first. */
    val muscleSplit: List<MuscleSetCount>,
    val exercises: List<ExerciseDetail>
)

data class SessionDetailUiState(
    val isLoading: Boolean = true,
    val data: SessionDetailData? = null
)
