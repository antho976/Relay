package com.forge.app.ui.gym.history

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
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import javax.inject.Inject

data class SessionHistoryUiState(
    /** Merged, filtered, newest-first — computed once per emission off the main thread. */
    val filtered: List<HistoryItem> = emptyList(),
    /** Distinct quick tags across all workouts — the source for the filter chips. */
    val availableTags: List<String> = emptyList(),
    val query: String = "",
    val tagFilter: String? = null,
    val durationFilter: SessionHistoryFilter? = null,
    val volumeFilter: SessionHistoryFilter? = null
) {
    /** Whether any filter or the search box is narrowing the list (drives the empty-state copy). */
    val anyFilterActive: Boolean
        get() = query.isNotBlank() || tagFilter != null || durationFilter != null || volumeFilter != null
}

@HiltViewModel
class SessionHistoryViewModel @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val cardioRepo: CardioRepository
) : ViewModel() {

    private val filters = MutableStateFlow(HistoryFilters())

    /** Everything derived from the DB (the search index + tag list) — recomputed ONLY when the DB
     *  changes, not on every keystroke. Filtering reads this without re-resolving exercise names. */
    private data class HistoryData(
        val workouts: List<Session> = emptyList(),
        val cardio: List<CardioEntry> = emptyList(),
        val namesBySession: Map<Long, List<String>> = emptyMap(),
        val availableTags: List<String> = emptyList()
    )

    private val dataFlow = combine(
        sessionDao.observeAllFinishedSessions(),
        loggedExerciseDao.observeSessionExerciseIds(),
        cardioRepo.observeAll()
    ) { sessions, exerciseRows, cardioEntries ->
        val namesBySession = exerciseRows
            .groupBy { it.sessionId }
            .mapValues { (_, rows) -> rows.map { Program.exerciseDisplayName(it.exerciseId, it.swappedName) } }
        HistoryData(
            workouts = sessions,
            cardio = cardioEntries.filter { it.type != CardioType.REST.code },
            namesBySession = namesBySession,
            availableTags = availableTagsOf(sessions)
        )
    }.flowOn(Dispatchers.Default)

    val state: StateFlow<SessionHistoryUiState> = combine(dataFlow, filters) { data, f ->
        SessionHistoryUiState(
            filtered = buildFilteredHistory(data.workouts, data.cardio, data.namesBySession, f),
            availableTags = data.availableTags,
            query = f.query,
            tagFilter = f.tag,
            durationFilter = f.duration,
            volumeFilter = f.volume
        )
    }.flowOn(Dispatchers.Default)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SessionHistoryUiState())

    fun setQuery(q: String) = filters.update { it.copy(query = q) }
    fun setTagFilter(tag: String?) = filters.update { it.copy(tag = tag) }
    fun setDurationFilter(f: SessionHistoryFilter?) = filters.update { it.copy(duration = f) }
    fun setVolumeFilter(f: SessionHistoryFilter?) = filters.update { it.copy(volume = f) }
}
