package com.forge.app.service.wear

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Session-scoped UI facts the wrist mirror needs but Room never stores. Today that is one thing:
 * which slots the user declared "done with this exercise" early (DayUiEvent.FinishExerciseEarly is
 * ViewModel state only — done < planned in Room forever), so the mirror's current-slot pick can
 * scan PAST them instead of pinning the wrist to an exercise the phone already filed under DONE.
 *
 * Ephemeral by design: keyed to a session id and dropped when another session's mark arrives.
 * Process death loses it — the mirror then degrades to its pure set-count heuristic, never breaks.
 */
@Singleton
class WearFocusHolder @Inject constructor() {

    /** Slot ids (plan-slot ids, not swapped exercise ids) finished early, for [sessionId]. */
    data class EarlyDone(val sessionId: Long, val slotIds: Set<String>)

    private val _earlyDone = MutableStateFlow<EarlyDone?>(null)
    val earlyDone: StateFlow<EarlyDone?> = _earlyDone

    fun markEarlyDone(sessionId: Long, slotId: String) {
        _earlyDone.value = _earlyDone.value
            ?.takeIf { it.sessionId == sessionId }
            ?.let { it.copy(slotIds = it.slotIds + slotId) }
            ?: EarlyDone(sessionId, setOf(slotId))
    }

    /** The early-done slot ids for [sessionId] (empty when the marks belong to another session). */
    fun earlyDoneFor(sessionId: Long): Set<String> =
        _earlyDone.value?.takeIf { it.sessionId == sessionId }?.slotIds ?: emptySet()
}
