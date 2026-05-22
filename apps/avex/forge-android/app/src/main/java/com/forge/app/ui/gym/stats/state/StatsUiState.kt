package com.forge.app.ui.gym.stats.state

import com.forge.app.program.ExercisePlan
import com.forge.app.program.MuscleGroup
import java.time.LocalDate

data class StatsUiState(
    val isLoading: Boolean = true,
    val totals: Totals = Totals(),
    val heatmap: List<HeatmapCell> = emptyList(),
    val volumeByMuscle: List<MuscleVolume> = emptyList(),
    val strengthCurve: StrengthCurve? = null,
    val recentPrs: List<PrEntry> = emptyList()
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
