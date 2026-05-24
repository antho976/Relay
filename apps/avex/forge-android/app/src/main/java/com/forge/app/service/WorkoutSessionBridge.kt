package com.forge.app.service

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Singleton bridge between [com.forge.app.ui.gym.train.DayViewModel] and
 * [WorkoutSessionService]. The VM writes session state here; the service
 * observes and drives notifications without any direct VM–Service binding.
 *
 * - [sessionState]: non-null while a workout is active. Setting to null signals
 *   the service to call [android.app.Service.stopSelf].
 * - [timerDone]: one-shot events emitted each time the rest timer hits 0.
 *   The service handles each emission independently (vibrate + notification).
 */
@Singleton
class WorkoutSessionBridge @Inject constructor() {

    private val _sessionState = MutableStateFlow<SessionNotifState?>(null)
    val sessionState: StateFlow<SessionNotifState?> = _sessionState.asStateFlow()

    private val _timerDone = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val timerDone: SharedFlow<Unit> = _timerDone.asSharedFlow()

    fun startSession(state: SessionNotifState) { _sessionState.value = state }
    fun endSession() { _sessionState.value = null }
    fun notifyTimerDone() { _timerDone.tryEmit(Unit) }
}

data class SessionNotifState(
    val dayName: String,
    val startedAtMs: Long
)
