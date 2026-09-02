package com.forge.app.ui.recap

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.program.CustomExerciseRegistry
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

/**
 * The display identity of one logged row, for the Recap's "most trained" bucket: the custom
 * registry's name for a user-created move (the one authoritative spelling, however the row's own
 * `swapped_name` was typed), then the row's swapped name, then the program/library name.
 */
internal fun recapExerciseName(exerciseId: String, swappedName: String?): String =
    CustomExerciseRegistry.name(exerciseId) ?: Program.exerciseDisplayName(exerciseId, swappedName)

/**
 * The most-trained movement over a set of (session, exercise, swapped name) rows: every row is
 * resolved to its display name FIRST, rows are bucketed by that name (case- and
 * whitespace-insensitively), and each bucket counts DISTINCT sessions. Grouping on the raw
 * `exercise_id` (what the aggregate query did) named a custom move by its humanized slug and merged
 * a late-swapped slot's two movements into one bucket under whichever name resolved first.
 *
 * Ties go to the bucket seen first in [rows] (the DAO orders them chronologically), so the answer
 * is stable across loads. Null when there are no rows.
 */
internal fun recapTopExercise(
    rows: List<LoggedExerciseDao.SessionExerciseRow>,
    resolve: (exerciseId: String, swappedName: String?) -> String = ::recapExerciseName
): String? {
    if (rows.isEmpty()) return null
    val displayByKey = LinkedHashMap<String, String>()
    val sessionsByKey = LinkedHashMap<String, MutableSet<Long>>()
    rows.forEach { row ->
        val display = resolve(row.exerciseId, row.swappedName).trim()
        val key = display.lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")
        displayByKey.putIfAbsent(key, display)
        sessionsByKey.getOrPut(key) { mutableSetOf() }.add(row.sessionId)
    }
    val top = sessionsByKey.entries.maxByOrNull { it.value.size } ?: return null
    return displayByKey[top.key]
}

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

        // Exclude untracked sessions (#110) so the count/volume agree with frequencySince (which now
        // filters them) — otherwise "most trained" and the session count describe different populations.
        // The tracked-only query rather than the inclusive one filtered afterwards: the same rule
        // stated once, in the place every other consumer reads it from.
        val monthSessions = sessionDao.finishedInRangeTracked(monthStart, monthEnd)
        val yearSessions = sessionDao.finishedInRangeTracked(yearStart, yearEnd)

        // Month recap
        val monthRecap = if (monthSessions.isNotEmpty()) {
            // Raw rows, resolved to a display identity BEFORE counting: see recapTopExercise.
            val topEx = recapTopExercise(loggedExerciseDao.sessionExerciseRowsSince(monthStart))
            val avgDur = monthSessions.mapNotNull { it.durationMinutes() }.average().toInt()
            val monthPrs = monthSessions.sumOf { it.prCount }
            // Only when there WERE records. maxByOrNull over a sum of zeros still returns a group —
            // the first one — so a month with no PRs at all named a "best day for records", picked
            // by nothing but map iteration order, and presented it as a finding about the user.
            val bestDay = monthSessions
                .takeIf { monthPrs > 0 }
                ?.groupBy { it.dayKey }
                ?.maxByOrNull { (_, sessions) -> sessions.sumOf { it.prCount } }
                // dayDisplayName resolves freestyle ("Open workout") and survives a day removed from the
                // plan; the raw Program.days lookup returned null for both and silently dropped the row.
                ?.key?.let { key -> Program.dayDisplayName(key) }
            MonthRecap(
                month = now,
                sessionCount = monthSessions.size,
                totalVolumeLb = monthSessions.sumOf { it.totalVolumeLb ?: 0.0 },
                totalPrs = monthPrs,
                totalSets = monthSessions.sumOf { it.setCount },
                topExercise = topEx,
                avgDurationMin = avgDur,
                bestDayName = bestDay
            )
        } else null

        // Year recap
        val yearRecap = if (yearSessions.isNotEmpty()) {
            val topEx = recapTopExercise(loggedExerciseDao.sessionExerciseRowsSince(yearStart))
            val totalVol = yearSessions.sumOf { it.totalVolumeLb ?: 0.0 }
            // Divide by calendar weeks elapsed this year, not sessions/7 (which wildly inflated it).
            val weeksElapsed = (java.time.temporal.ChronoUnit.DAYS
                .between(LocalDate.of(thisYear, 1, 1), LocalDate.now(zone)) / 7.0).coerceAtLeast(1.0)
            val avgWeekly = totalVol / weeksElapsed
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
