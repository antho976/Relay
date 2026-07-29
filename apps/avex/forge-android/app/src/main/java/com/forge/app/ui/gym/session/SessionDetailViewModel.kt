package com.forge.app.ui.gym.session

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.BackupRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.data.repo.WorkoutRepository
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
    private val workoutRepo: WorkoutRepository,
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
            // The watch's HR trace (W3), analyzed AFTER the page renders — additive: no watch, no
            // section. Sets attribute samples to exercises; rest events power the HRR read.
            if (sessionId >= 0) {
                val samples = workoutRepo.hrSamplesForSession(sessionId)
                    .map { com.forge.app.domain.health.HrPoint(timeMs = it.atMs, bpm = it.bpm) }
                if (samples.isNotEmpty()) {
                    val exercises = workoutRepo.loggedExercisesForSession(sessionId)
                    val nameByLoggedId = exercises.associate { le ->
                        le.id to (le.swappedName?.takeIf { it.isNotBlank() }
                            ?: com.forge.app.program.Program.exercise(le.exerciseId)?.name
                            ?: le.exerciseId)
                    }
                    val sets = workoutRepo.allSetsForSession(sessionId).map {
                        com.forge.app.domain.health.HrSetRef(
                            completedAtMs = it.completedAt,
                            exerciseName = nameByLoggedId[it.loggedExerciseId] ?: ""
                        )
                    }
                    val rests = workoutRepo.restEventsForSession(sessionId).map {
                        com.forge.app.domain.health.HrRestRef(
                            endedAtMs = it.loggedAt,
                            realizedSeconds = it.realizedSeconds
                        )
                    }
                    _state.value = _state.value.copy(
                        hrView = com.forge.app.domain.health.buildSessionHrView(samples, sets, rests)
                    )
                }
            }
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

    /**
     * Tag what kind of session this was (Coach v3 A1). The stored key drives the header pill AND the
     * adaptation engine: test / technique / first-back sessions are excluded from progression, stall
     * and fatigue reads, so a top-single test day never anchors the next prescription and a light
     * technique day never reads as a plateau. Retro-tagging is deliberate — you know a session was a
     * test day once it's done, and the engine only ever reads finished sessions.
     */
    fun setSessionType(key: String) {
        if (sessionId < 0) return
        val current = _state.value.data ?: return
        if (current.sessionType == key) return
        // Optimistic: the row is a single column write that can't meaningfully fail, and the header
        // pill re-reading instantly is the whole feedback (§13 — no toast for what the UI shows).
        _state.value = _state.value.copy(data = current.copy(sessionType = key))
        viewModelScope.launch { workoutRepo.setSessionType(sessionId, key) }
    }
}
