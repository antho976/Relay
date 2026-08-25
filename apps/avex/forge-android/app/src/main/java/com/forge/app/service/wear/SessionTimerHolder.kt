package com.forge.app.service.wear

import com.forge.app.core.time.Clock
import com.forge.app.core.time.ElapsedClock
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.timer.RestTimerController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The ONE rest-timer instance, app-scoped (W1). It used to live inside DayViewModel; hoisting it
 * here lets three consumers share it without drift: the day screen (as before), the
 * [WearStatePublisher] (mirrors it to the wrist), and [WearSyncService] (wrist skip/+30 commands
 * act on it even when no day screen exists). The controller's coroutine work is a 1 Hz tick, so an
 * app-lifetime scope costs nothing; [RestTimerController.stop] fires when a session ends
 * (stopSessionService) so a timer can't outlive its session.
 *
 * The timer is also PERSISTED here. Everything the day screen holds lives in memory, and
 * `leaveAndResume` — "keep the workout, I'll come back" — deliberately stops the foreground service
 * that was keeping the process resident. So leaving mid-rest to check Stats and having Android
 * reclaim the process meant returning to no countdown and no buzz: the user waits on a cue that
 * will never come, which is the failure the service exists to prevent. Sets themselves were never
 * at risk (they are in Room); the rest interval was.
 */
@Singleton
class SessionTimerHolder @Inject constructor(
    elapsed: ElapsedClock,
    private val clock: Clock,
    private val settingsRepo: SettingsRepository
) {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    val controller = RestTimerController(scope, elapsed)

    init {
        // Written on every structural change (start / pause / resume / ±30s / stop) and never on a
        // tick, so this is a handful of small writes per rest, not one a second.
        controller.onStructuralChange = { state ->
            scope.launch {
                withContext(NonCancellable) {
                    runCatching {
                        if (state == null) settingsRepo.clearRestTimer()
                        else settingsRepo.saveRestTimer(
                            endAtMs = clock.nowMs() + state.secondsRemaining * 1000L,
                            totalSeconds = state.totalSeconds,
                            pausedRemainingSeconds = if (state.isPaused) state.secondsRemaining else 0
                        )
                    }
                }
            }
        }
        scope.launch { runCatching { restoreSaved() } }
    }

    /**
     * Rebuild a rest that outlived its process. A PAUSED timer restores at exactly the seconds it
     * froze at; a running one restores at what is genuinely left, and one whose end instant has
     * already passed is simply dropped — arriving back to a buzz for a rest that ended twenty
     * minutes ago would be worse than arriving back to nothing.
     */
    private suspend fun restoreSaved() {
        val (endAtMs, total, pausedRemaining) = settingsRepo.savedRestTimer() ?: return
        if (pausedRemaining > 0) {
            controller.restore(totalSeconds = total, remainingSeconds = pausedRemaining, paused = true)
            return
        }
        val remainingSec = ((endAtMs - clock.nowMs()) / 1000L).toInt()
        if (remainingSec <= 0) {
            runCatching { settingsRepo.clearRestTimer() }
            return
        }
        controller.restore(totalSeconds = total, remainingSeconds = remainingSec, paused = false)
    }
}
