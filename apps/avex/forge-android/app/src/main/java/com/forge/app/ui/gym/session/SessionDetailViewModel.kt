package com.forge.app.ui.gym.session

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.BackupRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.ui.gym.session.state.SessionDetailUiState
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Loads one finished session's full breakdown for the detail page. The session id arrives via the
 * route argument ([Routes.ARG_SESSION_ID]); a session is immutable history, so one load is enough —
 * no flows to observe. Chart Metric/Style toggles are pure UI state and live in the screen.
 */
@HiltViewModel
class SessionDetailViewModel @Inject constructor(
    private val statsRepo: StatsRepository,
    private val backupRepo: BackupRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val sessionId: Long = savedStateHandle.get<Long>(Routes.ARG_SESSION_ID) ?: -1L

    private val _state = MutableStateFlow(SessionDetailUiState())
    val state: StateFlow<SessionDetailUiState> = _state.asStateFlow()

    /** Path of the just-written per-session JSON export — the screen opens the share sheet on it. */
    private val _exportPath = MutableStateFlow<String?>(null)
    val exportPath: StateFlow<String?> = _exportPath.asStateFlow()

    init {
        viewModelScope.launch {
            val data = if (sessionId >= 0) statsRepo.getSessionDetail(sessionId) else null
            _state.value = SessionDetailUiState(isLoading = false, data = data)
        }
    }

    /** In-flight export so a double-tap doesn't launch two concurrent writers of the same file. */
    private var exportJob: Job? = null

    /** Write this session's data to a JSON file, then surface its path so the screen can share it. */
    fun exportSession() {
        if (sessionId < 0 || exportJob?.isActive == true) return
        exportJob = viewModelScope.launch {
            backupRepo.exportSessionJson(sessionId)?.let { _exportPath.value = it.absolutePath }
        }
    }

    fun clearExportPath() { _exportPath.value = null }
}
