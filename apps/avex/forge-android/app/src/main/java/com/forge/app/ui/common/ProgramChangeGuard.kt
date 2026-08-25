package com.forge.app.ui.common

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.Session
import com.forge.app.data.repo.WorkoutRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.getAndUpdate
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Guards the destructive "change the program" actions (generate / deload / single-day re-roll)
 * against an in-progress workout. Those paths discard the active session and CASCADE-delete its
 * logged sets, so when a workout is active we stage the action behind a confirm instead of running
 * it silently.
 *
 * One shared instance feeds a single confirm dialog hosted at the app root ([ProgramChangeGuardHost]),
 * so the prompt appears over whatever screen triggered it — the user discards inline, never having to
 * hunt down the workout on another screen.
 */
@Singleton
class ProgramChangeGuard @Inject constructor(
    private val workoutRepo: WorkoutRepository
) {
    /** A staged change waiting on confirmation; [loggedSets] tells the dialog how much is at stake. */
    data class Pending(val loggedSets: Int)

    /**
     * The staged action, held atomically alongside [_pending].
     *
     * This was a plain `var` on a @Singleton written by every ViewModel that can mutate the program.
     * [run] overwrote it unconditionally and [confirm] read-then-cleared it with no lock, so a
     * dialog dismissed by a recomposition or a back gesture (neither of which calls [cancel]) left
     * a stale action staged: trigger a re-roll from Settings next and "Discard & continue" ran the
     * RE-ROLL, discarding the in-progress workout for an action the user never confirmed, while the
     * deload they actually asked for never happened.
     *
     * `getAndUpdate { null }` in [confirm] means it can also never double-run.
     */
    private val stagedAction = MutableStateFlow<(suspend () -> Unit)?>(null)
    private val _pending = MutableStateFlow<Pending?>(null)
    val pending: StateFlow<Pending?> = _pending.asStateFlow()

    /**
     * Run [action] immediately when no workout would be lost; otherwise stage it and raise the
     * confirm dialog. [affectsSession] lets a caller (e.g. single-day re-roll) declare it only
     * discards the session for a matching day — an unrelated active workout then passes straight
     * through without nagging.
     */
    suspend fun run(
        affectsSession: (Session) -> Boolean = { true },
        action: suspend () -> Unit
    ) {
        val active = workoutRepo.activeSession()
        if (active == null || !affectsSession(active)) {
            action()
            return
        }
        // A newer request replaces an older one — the user's latest intent is the live one — but the
        // dialog is re-raised with it so the two can never describe different actions.
        stagedAction.value = action
        _pending.value = Pending(workoutRepo.allSetsForSession(active.id).size)
    }

    /** User chose "Discard & continue": run the staged action (its own path discards the session). */
    suspend fun confirm() {
        val action = stagedAction.getAndUpdate { null } ?: return
        _pending.value = null
        action()
    }

    /** User backed out — drop the staged action, leave the workout untouched. */
    fun cancel() {
        stagedAction.value = null
        _pending.value = null
    }
}

/** Bridges the singleton [ProgramChangeGuard] into Compose for the root host. */
@HiltViewModel
class ProgramChangeGuardViewModel @Inject constructor(
    val guard: ProgramChangeGuard
) : ViewModel() {
    fun confirm() = viewModelScope.launch { runCatching { guard.confirm() } }
    fun cancel() = guard.cancel()
}
