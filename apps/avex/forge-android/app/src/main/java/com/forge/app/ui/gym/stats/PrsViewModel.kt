package com.forge.app.ui.gym.stats

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.projections.RecentPrRow
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.YearMonth
import java.time.ZoneId
import javax.inject.Inject

data class PrsUiState(
    val allPrs: List<RecentPrRow> = emptyList(),
    val filtered: List<PrDisplayRow> = emptyList(),
    val muscleFilter: MuscleGroup? = null,
    val periodFilter: PrPeriodFilter = PrPeriodFilter.ALL,
    val monthlyTarget: Int = 0,
    val monthlyCount: Int = 0
)

data class PrDisplayRow(
    val exerciseId: String,
    val exerciseName: String,
    val muscle: MuscleGroup?,
    val sessionDate: Long
)

enum class PrPeriodFilter(val label: String) {
    ALL("All time"),
    THIS_MONTH("This month"),
    LAST_3_MONTHS("3 months"),
    LAST_YEAR("This year")
}

@HiltViewModel
class PrsViewModel @Inject constructor(
    private val loggedExerciseDao: LoggedExerciseDao,
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    private val muscleFilter = MutableStateFlow<MuscleGroup?>(null)
    private val periodFilter = MutableStateFlow(PrPeriodFilter.ALL)

    val state: StateFlow<PrsUiState> = combine(
        loggedExerciseDao.observeAllPrs(),
        settingsRepo.monthlyPrTarget,
        muscleFilter,
        periodFilter
    ) { allPrs, target, muscle, period ->
        val zone = ZoneId.systemDefault()
        val now = YearMonth.now(zone)
        val monthStartMs = now.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()

        val cutoff: Long = when (period) {
            PrPeriodFilter.ALL -> 0L
            PrPeriodFilter.THIS_MONTH -> monthStartMs
            PrPeriodFilter.LAST_3_MONTHS -> now.minusMonths(3).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
            PrPeriodFilter.LAST_YEAR -> now.minusMonths(12).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        }

        val displayRows = allPrs.mapNotNull { row ->
            val plan = Program.exercise(row.exerciseId)
            val name = row.swappedName ?: plan?.name ?: row.exerciseId
            PrDisplayRow(
                exerciseId = row.exerciseId,
                exerciseName = name,
                muscle = plan?.muscle,
                sessionDate = row.sessionStartedAt
            )
        }.filter { row ->
            row.sessionDate >= cutoff && (muscle == null || row.muscle == muscle)
        }

        val monthlyCount = allPrs.count { it.sessionStartedAt >= monthStartMs }

        PrsUiState(
            allPrs = allPrs,
            filtered = displayRows,
            muscleFilter = muscle,
            periodFilter = period,
            monthlyTarget = target,
            monthlyCount = monthlyCount
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), PrsUiState())

    fun setMuscleFilter(muscle: MuscleGroup?) = muscleFilter.update { muscle }
    fun setPeriodFilter(period: PrPeriodFilter) = periodFilter.update { period }
    fun setMonthlyTarget(target: Int) = viewModelScope.launch { settingsRepo.setMonthlyPrTarget(target) }
}
