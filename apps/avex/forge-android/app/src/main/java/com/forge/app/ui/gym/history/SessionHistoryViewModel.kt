package com.forge.app.ui.gym.history

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.mood.Mood
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import javax.inject.Inject

enum class SessionHistoryFilter { SHORT, LONG, HIGH_VOLUME }

data class SessionHistoryUiState(
    val all: List<Session> = emptyList(),
    val moodFilter: Mood? = null,
    val durationFilter: SessionHistoryFilter? = null,
    val volumeFilter: SessionHistoryFilter? = null,
    val moodsBySessionId: Map<Long, String> = emptyMap()
) {
    val filtered: List<Session>
        get() {
            var list = all
            moodFilter?.let { mood ->
                val sessionIds = moodsBySessionId.entries
                    .filter { it.value == mood.code }
                    .map { it.key }
                    .toSet()
                list = list.filter { it.id in sessionIds }
            }
            durationFilter?.let { f ->
                list = when (f) {
                    SessionHistoryFilter.SHORT -> list.filter { s ->
                        s.finishedAt != null && (s.finishedAt - s.startedAt) / 60_000 < 45
                    }
                    SessionHistoryFilter.LONG -> list.filter { s ->
                        s.finishedAt != null && (s.finishedAt - s.startedAt) / 60_000 > 60
                    }
                    else -> list
                }
            }
            volumeFilter?.let { f ->
                list = when (f) {
                    SessionHistoryFilter.HIGH_VOLUME -> list.filter { it.totalVolumeLb != null && it.totalVolumeLb > 3000 }
                    else -> list
                }
            }
            return list
        }
}

@HiltViewModel
class SessionHistoryViewModel @Inject constructor(
    private val sessionDao: SessionDao,
    private val moodDao: MoodDao
) : ViewModel() {

    private val filters = MutableStateFlow(Triple<Mood?, SessionHistoryFilter?, SessionHistoryFilter?>(null, null, null))

    val state: StateFlow<SessionHistoryUiState> = combine(
        sessionDao.observeAllFinishedSessions(),
        moodDao.observeAll(),
        filters
    ) { sessions, moods, (moodFilter, durationFilter, volumeFilter) ->
        SessionHistoryUiState(
            all = sessions,
            moodFilter = moodFilter,
            durationFilter = durationFilter,
            volumeFilter = volumeFilter,
            moodsBySessionId = moods.filter { it.sessionId != null }.associate { it.sessionId!! to it.mood }
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SessionHistoryUiState())

    fun setMoodFilter(mood: Mood?) = filters.update { it.copy(first = mood) }
    fun setDurationFilter(f: SessionHistoryFilter?) = filters.update { it.copy(second = f) }
    fun setVolumeFilter(f: SessionHistoryFilter?) = filters.update { it.copy(third = f) }
}
