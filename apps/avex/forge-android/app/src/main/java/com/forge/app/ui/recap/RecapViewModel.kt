package com.forge.app.ui.recap

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.CustomExerciseRegistry
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import javax.inject.Inject

/**
 * The display identity of one logged row, for the Recap's "most trained" bucket: the custom
 * registry's name for a user-created move (the one authoritative spelling, however the row's own
 * `swapped_name` was typed), then the row's swapped name, then the program/library name.
 */
internal fun recapExerciseName(exerciseId: String, swappedName: String?): String =
    CustomExerciseRegistry.name(exerciseId) ?: Program.exerciseDisplayName(exerciseId, swappedName)

private val WHITESPACE_REGEX = Regex("\\s+")

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
        val key = display.lowercase().split(WHITESPACE_REGEX).filter { it.isNotEmpty() }.joinToString(" ")
        displayByKey.putIfAbsent(key, display)
        sessionsByKey.getOrPut(key) { mutableSetOf() }.add(row.sessionId)
    }
    val top = sessionsByKey.entries.maxByOrNull { it.value.size } ?: return null
    return displayByKey[top.key]
}

/** [recapTopExercise] with how many sessions it was trained in. */
internal fun recapTopExerciseCount(
    rows: List<LoggedExerciseDao.SessionExerciseRow>,
    resolve: (exerciseId: String, swappedName: String?) -> String = ::recapExerciseName
): Pair<String, Int>? {
    val name = recapTopExercise(rows, resolve) ?: return null
    val key = name.lowercase().split(WHITESPACE_REGEX).filter { it.isNotEmpty() }.joinToString(" ")
    val count = rows.filter {
        resolve(it.exerciseId, it.swappedName).trim().lowercase().split(WHITESPACE_REGEX).filter { w -> w.isNotEmpty() }.joinToString(" ") == key
    }.map { it.sessionId }.distinct().size
    return name to count
}

/** Which recap the page shows. */
enum class RecapTab(val label: String) { MONTH("Month"), YEAR("Year") }

/** The page's state: the period chosen on each tab, and its built readings. */
data class RecapUiState(
    val isLoading: Boolean = true,
    val month: YearMonth = YearMonth.now(),
    val year: Int = YearMonth.now().year,
    val monthUi: MonthRecapUi? = null,
    val yearUi: YearRecapUi? = null,
    /** The first month with a logged session: the page steps back no further. */
    val firstMonth: YearMonth? = null,
)


@HiltViewModel
class RecapViewModel @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: com.forge.app.data.db.dao.LoggedSetDao,
    private val vacationDao: com.forge.app.data.db.dao.VacationDao,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository
) : ViewModel() {

    private val _state = MutableStateFlow(RecapUiState())
    val state: StateFlow<RecapUiState> = _state.asStateFlow()

    /** Every tracked finished session and strength set, read once; periods are cut from them. */
    private var sessions: List<com.forge.app.data.db.entities.Session>? = null
    private var sets: List<RecapSet> = emptyList()
    private var loadJob: kotlinx.coroutines.Job? = null

    init { reload() }

    /** Step the month tab one month back (-1) or forward (+1), never before the first session or past now. */
    fun stepMonth(by: Int) {
        val s = _state.value
        val next = s.month.plusMonths(by.toLong())
        if (next.isAfter(YearMonth.now()) || (s.firstMonth != null && next.isBefore(s.firstMonth))) return
        _state.update { it.copy(month = next) }
        reload()
    }

    /** Step the year tab, within the years that have sessions. */
    fun stepYear(by: Int) {
        val s = _state.value
        val next = s.year + by
        if (next > YearMonth.now().year || (s.firstMonth != null && next < s.firstMonth.year)) return
        _state.update { it.copy(year = next) }
        reload()
    }

    private fun reload() {
        loadJob?.cancel()
        loadJob = viewModelScope.launch { load() }
    }

    // Default: the grouping, name resolution and streak passes below are CPU work, and
    // viewModelScope would otherwise run them on Main during the screen's entrance.
    private suspend fun load() = withContext(Dispatchers.Default) {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val unit = settingsRepo.weightUnit.first()
        val locale = java.util.Locale.getDefault()
        // Untracked sessions are excluded (#110) so the counts agree with every other surface.
        val all = sessions ?: sessionDao.finishedInRangeTracked(0L, Long.MAX_VALUE).also { loaded ->
            sessions = loaded
            sets = loggedSetDao.observeAllFinishedStatsSets().first()
                .filter { it.isStrengthSet && (it.set.weightLb ?: 0.0) > 0.0 }
                .map { RecapSet(it.set.exerciseId, it.set.swappedName, it.set.sessionStartedAt, it.set.weightLb ?: 0.0, it.set.reps) }
        }
        val first = all.minOfOrNull { it.startedAt }?.let { YearMonth.from(Instant.ofEpochMilli(it).atZone(zone)) }
        val st = _state.value
        fun range(fromDate: LocalDate, toDate: LocalDate) = Pair(
            fromDate.atStartOfDay(zone).toInstant().toEpochMilli(),
            toDate.atStartOfDay(zone).toInstant().toEpochMilli()
        )
        fun inRange(r: Pair<Long, Long>) = all.filter { it.startedAt >= r.first && it.startedAt < r.second }
        suspend fun top(r: Pair<Long, Long>, ids: Set<Long>) = recapTopExerciseCount(
            loggedExerciseDao.sessionExerciseRowsSince(r.first).filter { it.sessionId in ids }
        )

        val mRange = range(st.month.atDay(1), st.month.plusMonths(1).atDay(1))
        val pRange = range(st.month.minusMonths(1).atDay(1), st.month.atDay(1))
        val monthSessions = inRange(mRange)
        val monthUi = buildMonthRecap(
            month = st.month,
            sessions = monthSessions,
            previous = inRange(pRange),
            sets = sets,
            topExercise = top(mRange, monthSessions.map { it.id }.toSet()),
            today = today,
            unit = unit,
            locale = locale,
            zone = zone,
            liftName = ::recapExerciseName,
            dayName = { Program.dayDisplayName(it) },
        )

        val yRange = range(LocalDate.of(st.year, 1, 1), LocalDate.of(st.year + 1, 1, 1))
        val yearSessions = inRange(yRange)
        // Longest streak, bridged across holidays and planned rest days the way the live streak
        // and Profile's best are.
        val onVacation = com.forge.app.domain.vacation.VacationCalendar.onVacation(vacationDao.all())
        val restHistory = settingsRepo.scheduleHistory.first()
        val streak = if (yearSessions.isEmpty()) 0 else com.forge.app.domain.session.longestBridgedStreak(
            yearSessions.map { Instant.ofEpochMilli(it.startedAt).atZone(zone).toLocalDate() }
        ) { d -> onVacation(d) || com.forge.app.domain.schedule.ScheduleHistory.isPlannedRest(restHistory, d) }
        val yearUi = buildYearRecap(
            year = st.year,
            sessions = yearSessions,
            sets = sets,
            topExercise = top(yRange, yearSessions.map { it.id }.toSet()),
            longestStreakDays = streak,
            today = today,
            unit = unit,
            locale = locale,
            zone = zone,
            liftName = ::recapExerciseName,
        )
        _state.update { it.copy(isLoading = false, monthUi = monthUi, yearUi = yearUi, firstMonth = first) }
    }
}
