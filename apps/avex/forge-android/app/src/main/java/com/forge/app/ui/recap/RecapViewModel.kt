package com.forge.app.ui.recap

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import javax.inject.Inject

data class MonthRecap(
    val month: YearMonth,
    val sessionCount: Int,
    val totalVolumeLb: Double,
    val totalPrs: Int,
    val totalSets: Int,
    val topExercise: String?,
    val avgDurationMin: Int,
    val bestDayName: String?
)

data class YearRecap(
    val year: Int,
    val sessionCount: Int,
    val totalVolumeLb: Double,
    val totalPrs: Int,
    val avgWeeklyVolume: Double,
    val topExercise: String?,
    val longestStreak: Int,
    val bestMonthName: String?
)

data class RecapUiState(
    val isLoading: Boolean = true,
    val monthRecap: MonthRecap? = null,
    val yearRecap: YearRecap? = null
)

@HiltViewModel
class RecapViewModel @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao
) : ViewModel() {

    private val _state = MutableStateFlow(RecapUiState())
    val state: StateFlow<RecapUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        val zone = ZoneId.systemDefault()
        val now = YearMonth.now(zone)
        val thisYear = now.year

        val monthStart = now.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val monthEnd = now.plusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val yearStart = LocalDate.of(thisYear, 1, 1).atStartOfDay(zone).toInstant().toEpochMilli()
        val yearEnd = LocalDate.of(thisYear + 1, 1, 1).atStartOfDay(zone).toInstant().toEpochMilli()

        val monthSessions = sessionDao.finishedInRange(monthStart, monthEnd)
        val yearSessions = sessionDao.finishedInRange(yearStart, yearEnd)

        // Month recap
        val monthRecap = if (monthSessions.isNotEmpty()) {
            val exFreq = loggedExerciseDao.frequencySince(monthStart)
            val topEx = exFreq.maxByOrNull { it.sessionCount }?.let { Program.exercise(it.exerciseId)?.name }
            val avgDur = monthSessions.mapNotNull { s ->
                s.finishedAt?.let { ((it - s.startedAt) / 60_000).toInt() }
            }.average().toInt()
            val bestDay = monthSessions.groupBy { it.dayKey }
                .maxByOrNull { (_, sessions) -> sessions.sumOf { it.prCount } }
                ?.key?.let { key -> Program.days.firstOrNull { it.key == key }?.defaultName }
            MonthRecap(
                month = now,
                sessionCount = monthSessions.size,
                totalVolumeLb = monthSessions.sumOf { it.totalVolumeLb ?: 0.0 },
                totalPrs = monthSessions.sumOf { it.prCount },
                totalSets = monthSessions.sumOf { it.setCount },
                topExercise = topEx,
                avgDurationMin = avgDur,
                bestDayName = bestDay
            )
        } else null

        // Year recap
        val yearRecap = if (yearSessions.isNotEmpty()) {
            val exFreq = loggedExerciseDao.frequencySince(yearStart)
            val topEx = exFreq.maxByOrNull { it.sessionCount }?.let { Program.exercise(it.exerciseId)?.name }
            val totalVol = yearSessions.sumOf { it.totalVolumeLb ?: 0.0 }
            val weeks = (yearSessions.size / 7.0).coerceAtLeast(1.0)
            val avgWeekly = totalVol / weeks
            val bestMonth = yearSessions
                .groupBy { Instant.ofEpochMilli(it.startedAt).atZone(zone).month.name.lowercase().replaceFirstChar { c -> c.uppercase() } }
                .maxByOrNull { (_, sessions) -> sessions.sumOf { it.totalVolumeLb ?: 0.0 } }?.key
            // Longest streak in year
            val trainingDays = yearSessions.mapTo(sortedSetOf()) {
                Instant.ofEpochMilli(it.startedAt).atZone(zone).toLocalDate()
            }
            var maxStreak = 0; var streak = 0; var prev: LocalDate? = null
            for (d in trainingDays) {
                if (prev != null && java.time.temporal.ChronoUnit.DAYS.between(prev, d) == 1L) streak++ else streak = 1
                if (streak > maxStreak) maxStreak = streak
                prev = d
            }
            YearRecap(
                year = thisYear,
                sessionCount = yearSessions.size,
                totalVolumeLb = totalVol,
                totalPrs = yearSessions.sumOf { it.prCount },
                avgWeeklyVolume = avgWeekly,
                topExercise = topEx,
                longestStreak = maxStreak,
                bestMonthName = bestMonth
            )
        } else null

        _state.value = RecapUiState(isLoading = false, monthRecap = monthRecap, yearRecap = yearRecap)
    }
}
