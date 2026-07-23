package com.forge.app.service.wear

import com.forge.app.core.time.Clock
import com.forge.app.domain.timer.RestTimerController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The ONE rest-timer instance, app-scoped (W1). It used to live inside DayViewModel; hoisting it
 * here lets three consumers share it without drift: the day screen (as before), the
 * [WearStatePublisher] (mirrors it to the wrist), and [WearSyncService] (wrist skip/+30 commands
 * act on it even when no day screen exists). The controller's coroutine work is a 1 Hz tick, so an
 * app-lifetime scope costs nothing; [RestTimerController.stop] fires when a session ends
 * (stopSessionService) so a timer can't outlive its session.
 */
@Singleton
class SessionTimerHolder @Inject constructor(clock: Clock) {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    val controller = RestTimerController(scope, clock)
}
