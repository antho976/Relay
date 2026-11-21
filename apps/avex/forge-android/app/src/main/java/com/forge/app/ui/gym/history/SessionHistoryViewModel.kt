package com.forge.app.ui.gym.history

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioType
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SessionHistoryUiState(
    /**
     * Merged, filtered, newest-first, and already grouped into calendar days — computed once per
     * emission off the main thread, so the list reads a ready-made structure and the date is
     * resolved to its human label exactly once per day rather than once per row.
     */
    val months: List<HistoryMonth> = emptyList(),
    /** "212 workouts and 64 cardio sessions since Mar 2025", for what the filters leave. */
    val meta: String = "",
    val kind: HistoryKind = HistoryKind.ALL,
    /** How many entries and how much volume the current filters leave — the reading under the title. */
    val summary: HistorySummary = HistorySummary(0, 0.0),
    /** Distinct quick tags across all workouts — the source for the filter chips. */
    val availableTags: List<String> = emptyList(),
    val query: String = "",
    val tagFilter: String? = null,
    val durationFilter: SessionHistoryFilter? = null,
    val volumeFilter: SessionHistoryFilter? = null,
    /** False only for the placeholder before the first real emission — no empty-state flash. */
    val loaded: Boolean = false
) {
    /** Whether any filter or the search box is narrowing the list (drives the empty-state copy). */
    val anyFilterActive: Boolean
        get() = query.isNotBlank() || tagFilter != null || durationFilter != null || volumeFilter != null ||
            kind != HistoryKind.ALL

    val isEmpty: Boolean get() = months.isEmpty()
}

@HiltViewModel
class SessionHistoryViewModel @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val cardioRepo: CardioRepository,
    private val settingsRepo: com.forge.app.data.prefs.SettingsRepository,
    private val statsRepo: com.forge.app.data.repo.StatsRepository
) : ViewModel() {

    /**
     * Each gym row's heaviest set and its lifts against their last time, loaded once per session as
     * its row comes into view. A finished session never changes, so the cache never goes stale.
     */
    private val details = androidx.compose.runtime.mutableStateMapOf<Long, HistoryRowDetail>()
    private val requested = HashSet<Long>()

    fun detailFor(sessionId: Long): HistoryRowDetail? = details[sessionId]

    fun requestDetail(sessionId: Long) {
        if (!requested.add(sessionId)) return
        viewModelScope.launch {
            val detail = runCatching {
                val lines = statsRepo.getSessionExerciseLines(sessionId)
                val top = lines.filter { (it.topWeightLb ?: 0.0) > 0.0 }.maxByOrNull { it.topWeightLb!! }
                val unit = settingsRepo.weightUnit.first()
                HistoryRowDetail(
                    lead = top?.let {
                        "${com.forge.app.ui.overview.compactLiftName(it.exerciseName)} " +
                            "${com.forge.app.domain.units.weightInputValue(it.topWeightLb!!, unit)}×${it.topReps ?: 0}"
                    },
                    deltas = statsRepo.liftDeltas(sessionId).map { it.pct }
                )
            }.getOrNull()
            if (detail != null) details[sessionId] = detail else requested.remove(sessionId)
        }
    }

    private val filters = MutableStateFlow(HistoryFilters())

    /** What the search field shows. Held synchronously: feeding the field from [state].query, which
     *  only updates after the filter pass has run on another thread, hands it stale text mid-typing
     *  and drops keystrokes. */
    var queryText by mutableStateOf("")
        private set

    /** Everything derived from the DB (the search index + tag list) — recomputed ONLY when the DB
     *  changes, not on every keystroke. Filtering reads this without re-resolving exercise names. */
    private data class HistoryData(
        val workouts: List<Session> = emptyList(),
        val cardio: List<CardioEntry> = emptyList(),
        val namesBySession: Map<Long, List<String>> = emptyMap(),
        val availableTags: List<String> = emptyList(),
        /** The user's own cardio activities — search must match the name the row prints. */
        val customCardioTypes: List<com.forge.app.domain.cardio.CustomCardioType> = emptyList()
    )

    private val dataFlow = combine(
        // Deduped: Room re-emits both on any write to their tables — every set of a workout in
        // progress — and each emission re-filters and regroups the whole history.
        sessionDao.observeAllFinishedSessions().distinctUntilChanged(),
        loggedExerciseDao.observeSessionExerciseIds().distinctUntilChanged(),
        cardioRepo.observeAll(),
        settingsRepo.customCardioTypes
    ) { sessions, exerciseRows, cardioEntries, customTypes ->
        val namesBySession = exerciseRows
            .groupBy { it.sessionId }
            .mapValues { (_, rows) -> rows.map { Program.exerciseDisplayName(it.exerciseId, it.swappedName) } }
        HistoryData(
            workouts = sessions,
            cardio = cardioEntries.filter { it.type != CardioType.REST.code },
            namesBySession = namesBySession,
            availableTags = availableTagsOf(sessions),
            customCardioTypes = customTypes
        )
    }.flowOn(Dispatchers.Default)

    val state: StateFlow<SessionHistoryUiState> = combine(dataFlow, filters) { data, f ->
        val items = buildFilteredHistory(
            data.workouts, data.cardio, data.namesBySession, f, data.customCardioTypes,
            heavyLb = heavyThresholdLb(data.workouts)
        )
        SessionHistoryUiState(
            months = groupByMonth(items),
            meta = historyMeta(items),
            kind = f.kind,
            summary = summarize(items),
            availableTags = data.availableTags,
            query = f.query,
            tagFilter = f.tag,
            durationFilter = f.duration,
            volumeFilter = f.volume,
            loaded = true
        )
    }.flowOn(Dispatchers.Default)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SessionHistoryUiState())

    fun setQuery(q: String) {
        queryText = q
        filters.update { it.copy(query = q) }
    }
    fun setTagFilter(tag: String?) = filters.update { it.copy(tag = tag) }
    fun setDurationFilter(f: SessionHistoryFilter?) = filters.update { it.copy(duration = f) }
    fun setVolumeFilter(f: SessionHistoryFilter?) = filters.update { it.copy(volume = f) }
    fun setKind(k: HistoryKind) = filters.update { it.copy(kind = k) }

    /** What the "All" pill does: drop every pill filter, leaving the search box alone. */
    fun clearPillFilters() = filters.update { it.copy(tag = null, duration = null, volume = null) }
}
