package com.forge.app.domain.timer

import com.forge.app.core.time.ElapsedClock
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
 * Remaining time is derived from an end INSTANT rather than by decrementing a counter once per
 * `delay(1000)`. This means the countdown stays accurate across scheduling drift and app
 * backgrounding (the Main dispatcher can stall while backgrounded; a decrement-based timer would
 * lag real time), and makes the whole machine deterministically unit-testable with fake time.
 *
 * That instant is on the MONOTONIC [ElapsedClock], not the wall clock. A countdown asks "how much
 * time has passed", and the wall clock answers "what time is it" — a question whose answer jumps.
 * It used to guard only the BACKWARD jump: a forward one is indistinguishable from time actually
 * passing, so it was consumed straight out of the remaining rest. A phone that had been off the
 * network, associating with the gym Wi-Fi 20 seconds into a 2:30 rest and having NTP push its
 * clock forward four minutes, buzzed "rest over" on the spot. `elapsedRealtime` keeps counting
 * through deep sleep, so anchoring on it costs nothing in accuracy while backgrounded.
 */
class RestTimerController(
    private val scope: CoroutineScope,
    private val elapsed: ElapsedClock,
    private val defaultSeconds: Int = DEFAULT_REST_SECONDS
) {
    private val _state = MutableStateFlow<RestTimerState?>(null)
    val state: StateFlow<RestTimerState?> = _state.asStateFlow()

    private var tickJob: Job? = null

    /** Elapsed-time reading (ms) at which the countdown reaches zero. Authoritative while running. */
    private var endAtElapsedMs: Long = 0L

    /** (Re)start the timer at [seconds] and begin counting down. */
    fun start(seconds: Int = defaultSeconds) {
        endAtElapsedMs = elapsed.elapsedMs() + seconds * 1000L
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
        endAtElapsedMs = elapsed.elapsedMs() + current.secondsRemaining * 1000L
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
            endAtElapsedMs = elapsed.elapsedMs() + updated * 1000L
            _state.value = current.copy(secondsRemaining = updated, isPaused = false)
        } else {
            endAtElapsedMs += seconds * 1000L
            _state.value = current.copy(secondsRemaining = remainingNow(current.copy(isPaused = false)))
        }
        relaunchTickJob()
    }

    /** Seconds left of real elapsed time (never negative). Rounds up so a fresh 150 reads 150, not 149.
     *
     *  A wall-clock correction in EITHER direction is now simply invisible here — the elapsed clock
     *  it reads cannot jump. The ceiling clamp stays as a display bound (and as a backstop should an
     *  implementation ever hand back something implausible). */
    private fun remainingNow(state: RestTimerState): Int {
        if (state.isPaused) return state.secondsRemaining
        val ms = endAtElapsedMs - elapsed.elapsedMs()
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
        /** Upper bound on displayed remaining time. 1 h is far above any real rest period or
         *  "+30s" extension. */
        const val MAX_REST_SECONDS: Int = 3600
    }
}
