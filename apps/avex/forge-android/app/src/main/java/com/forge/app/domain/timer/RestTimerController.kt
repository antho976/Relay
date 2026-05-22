package com.forge.app.domain.timer

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * Immutable snapshot of the rest timer. `null` means no timer is running — UI hides
 * the bubble entirely in that case rather than showing a zeroed-out one.
 */
data class RestTimerState(
    val totalSeconds: Int,
    val secondsRemaining: Int,
    val isPaused: Boolean
) {
    val isFinished: Boolean get() = secondsRemaining <= 0
}

/**
 * Standalone rest-timer state machine, owned by [com.forge.app.ui.gym.train.DayViewModel]
 * so the VM doesn't carry the tick-job bookkeeping itself.
 *
 * The controller takes the scope it should run in via the constructor — passing
 * `viewModelScope` means the tick coroutine is cleaned up automatically when the
 * VM is cleared. No need for explicit dispose.
 */
class RestTimerController(
    private val scope: CoroutineScope,
    private val defaultSeconds: Int = DEFAULT_REST_SECONDS
) {
    private val _state = MutableStateFlow<RestTimerState?>(null)
    val state: StateFlow<RestTimerState?> = _state.asStateFlow()

    private var tickJob: Job? = null

    /** (Re)start the timer at [seconds] and begin counting down. */
    fun start(seconds: Int = defaultSeconds) {
        _state.value = RestTimerState(
            totalSeconds = seconds,
            secondsRemaining = seconds,
            isPaused = false
        )
        relaunchTickJob()
    }

    fun pause() {
        tickJob?.cancel()
        tickJob = null
        _state.update { it?.copy(isPaused = true) }
    }

    fun resume() {
        val current = _state.value ?: return
        if (!current.isPaused) return
        _state.update { it?.copy(isPaused = false) }
        relaunchTickJob()
    }

    /** Reset to the original total seconds and pause. */
    fun reset() {
        val current = _state.value ?: return
        tickJob?.cancel()
        tickJob = null
        _state.value = current.copy(
            secondsRemaining = current.totalSeconds,
            isPaused = true
        )
    }

    /** Stop the timer entirely — bubble disappears. */
    fun stop() {
        tickJob?.cancel()
        tickJob = null
        _state.value = null
    }

    private fun relaunchTickJob() {
        tickJob?.cancel()
        tickJob = scope.launch {
            while (true) {
                delay(1_000)
                val current = _state.value ?: break
                if (current.isPaused) break
                if (current.secondsRemaining <= 1) {
                    _state.value = current.copy(secondsRemaining = 0, isPaused = true)
                    break
                }
                _state.value = current.copy(secondsRemaining = current.secondsRemaining - 1)
            }
        }
    }

    companion object {
        const val DEFAULT_REST_SECONDS: Int = 150 // 2:30
    }
}
