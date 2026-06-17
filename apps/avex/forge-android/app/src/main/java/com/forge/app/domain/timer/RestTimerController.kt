package com.forge.app.domain.timer

import com.forge.app.core.time.Clock
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
 *
 * Remaining time is derived from a wall-clock end instant ([endAtMs]) rather than by
 * decrementing a counter once per `delay(1000)`. This means the countdown stays accurate
 * across scheduling drift and app backgrounding (the Main dispatcher can stall while
 * backgrounded; a decrement-based timer would lag real time), and makes the whole
 * machine deterministically unit-testable with a fake [Clock].
 */
class RestTimerController(
    private val scope: CoroutineScope,
    private val clock: Clock,
    private val defaultSeconds: Int = DEFAULT_REST_SECONDS
) {
    private val _state = MutableStateFlow<RestTimerState?>(null)
    val state: StateFlow<RestTimerState?> = _state.asStateFlow()

    private var tickJob: Job? = null

    /** Wall-clock instant (ms) the countdown reaches zero. Authoritative while not paused. */
    private var endAtMs: Long = 0L

    /** (Re)start the timer at [seconds] and begin counting down. */
    fun start(seconds: Int = defaultSeconds) {
        endAtMs = clock.nowMs() + seconds * 1000L
        _state.value = RestTimerState(
            totalSeconds = seconds,
            secondsRemaining = seconds,
            isPaused = false
        )
        relaunchTickJob()
    }

    fun pause() {
        val current = _state.value ?: return
        tickJob?.cancel()
        tickJob = null
        // Freeze the live remaining value so resume() can rebuild the end instant from it.
        _state.value = current.copy(secondsRemaining = remainingNow(current), isPaused = true)
    }

    fun resume() {
        val current = _state.value ?: return
        // The rest is already over — "Resume" means get back to working out, so dismiss the timer
        // instead of trying to resume a 0-second countdown (which was a no-op).
        if (current.secondsRemaining <= 0) { stop(); return }
        if (!current.isPaused) return
        endAtMs = clock.nowMs() + current.secondsRemaining * 1000L
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

    /** Add [seconds] to the remaining time. Resumes if the timer was paused. */
    fun addSeconds(seconds: Int) {
        val current = _state.value ?: return
        if (current.isPaused) {
            val updated = (current.secondsRemaining + seconds).coerceAtLeast(0)
            endAtMs = clock.nowMs() + updated * 1000L
            _state.value = current.copy(secondsRemaining = updated, isPaused = false)
        } else {
            endAtMs += seconds * 1000L
            _state.value = current.copy(secondsRemaining = remainingNow(current.copy(isPaused = false)))
        }
        relaunchTickJob()
    }

    /** Seconds left per the wall clock (never negative). Rounds up so a fresh 150 reads 150, not 149. */
    private fun remainingNow(state: RestTimerState): Int {
        if (state.isPaused) return state.secondsRemaining
        var ms = endAtMs - clock.nowMs()
        // A BACKWARD wall-clock jump (NTP correction on a stale-clock boot, or a manual time change)
        // pushes (endAtMs − now) far past any real rest. Detect it (remaining beyond the ceiling) and
        // RE-ANCHOR the end instant to the last known-good remaining, so the countdown keeps running
        // and still finishes — instead of either showing hours or freezing for the length of the jump.
        if (ms > MAX_REST_SECONDS * 1000L) {
            endAtMs = clock.nowMs() + state.secondsRemaining * 1000L
            ms = endAtMs - clock.nowMs()
        }
        return if (ms <= 0) 0 else ((ms + 999) / 1000).toInt().coerceAtMost(MAX_REST_SECONDS)
    }

    private fun relaunchTickJob() {
        tickJob?.cancel()
        tickJob = scope.launch {
            while (true) {
                val current = _state.value ?: break
                if (current.isPaused) break
                val remaining = remainingNow(current)
                if (remaining <= 0) {
                    _state.value = current.copy(secondsRemaining = 0, isPaused = true)
                    break
                }
                if (remaining != current.secondsRemaining) {
                    _state.value = current.copy(secondsRemaining = remaining)
                }
                delay(1_000)
            }
        }
    }

    companion object {
        const val DEFAULT_REST_SECONDS: Int = 150 // 2:30
        /** Upper bound on displayed remaining time AND the threshold past which a backward wall-clock
         *  jump is detected and re-anchored. 1 h is far above any real rest period or "+30s" extension. */
        const val MAX_REST_SECONDS: Int = 3600
    }
}
