package com.forge.app.ui.gym.train

import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.SessionSwapResult
import com.forge.app.program.ExerciseDef
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DislikeSwapPrompt
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * The one message for a swap that meets an already-logged set — whether the picker is refused up
 * front or a set lands (say, from the wrist) while the sheet is open and the write is refused.
 * Single literal on purpose: the design doctrine freezes this file's em-dash count.
 */
internal const val SWAP_AFTER_SETS_MESSAGE = "Can't swap after logging sets — delete this exercise's sets first."

/**
 * What the day screen does with a [SessionSwapResult] — pure, unit-tested in DaySessionSwapReactionTest.
 *
 * The sheet closes either way: on success it is done; on a refusal it is STALE, because the only way a
 * refusal happens is that a set arrived under the row after the picker opened (H-11), and leaving the
 * sheet up would invite a second identical attempt. "Make default" persists the customization only
 * once the current session actually took the swap — persisting first and then failing the session
 * write would leave every future session on a movement this one refused.
 */
internal data class SwapReaction(
    val applied: Boolean,
    val closeSheet: Boolean,
    val message: String?,
    val persistDefault: Boolean
)

internal fun swapReaction(result: SessionSwapResult, makeDefault: Boolean): SwapReaction {
    val applied = result == SessionSwapResult.APPLIED
    return SwapReaction(
        applied = applied,
        closeSheet = true,
        message = SWAP_AFTER_SETS_MESSAGE.takeIf { result == SessionSwapResult.REFUSED_SETS_LOGGED },
        persistDefault = makeDefault && applied
    )
}

internal fun DayViewModel.handleSwapEvent(event: DayUiEvent) {
    when (event) {
        is DayUiEvent.OpenSwapPicker -> viewModelScope.launch {
            // Swapping is impossible once sets are logged: re-keying would mis-attribute the performed
            // sets, and a name-only relabel would lie about what they were (#11). Warn instead of opening.
            // Confirm against the DB, not just in-memory state (SM-5): a freshly-resumed VM or an in-flight
            // refresh can show zero sets for a row that already has them, which would open the picker for
            // a swap the repository is then going to refuse.
            val ui = _state.value.exercises.firstOrNull { it.plan.id == event.exerciseId }
            val hasSets = ui?.loggedSets?.isNotEmpty() == true ||
                ui?.loggedExerciseId?.let { workoutRepo.setsFor(it).isNotEmpty() } == true
            if (hasSets) {
                _messages.trySend(SWAP_AFTER_SETS_MESSAGE)
            } else {
                _state.update { it.copy(swapPickerForExerciseId = event.exerciseId) }
            }
        }
        is DayUiEvent.CloseSwapPicker -> _state.update { it.copy(swapPickerForExerciseId = null) }
        is DayUiEvent.PickSwapForSession -> applySessionSwap(event.exerciseId, event.swap)
        is DayUiEvent.PickSwapPersistent -> applyPersistentSwap(event.exerciseId, event.swap)
        is DayUiEvent.ClearPersistentSwap -> clearPersistentSwap(event.exerciseId)

        // Post-swap "dislike the swapped-out exercise?" prompt
        is DayUiEvent.DislikeSwappedExercise -> dislikeSwappedExercise()
        is DayUiEvent.DismissDislikePrompt -> _state.update { it.copy(dislikeSwapPrompt = null) }
        is DayUiEvent.SuppressDislikePromptThisSession -> {
            dislikePromptSuppressedThisSession = true
            _state.update { it.copy(dislikeSwapPrompt = null) }
        }
        is DayUiEvent.NeverAskDislikePrompt -> neverAskDislikePrompt()
        else -> {}
    }
}

private fun DayViewModel.applySessionSwap(exerciseId: String, swap: ExerciseDef) {
    // Same re-entrancy guard the persistent path uses, for a sharper reason (SM-2 follow-up):
    // ensureLoggedExercise reads loggedExerciseId out of UI STATE, which only refreshes after the
    // write completes. Two "just today" swaps landing on the same slot before that refresh both read
    // null and both INSERT, leaving two logged_exercise rows for one slot — the day screen then draws
    // the slot twice and the session's sets split across the copies. logged_exercise has no unique
    // index on (session, slot) to catch it, so the guard is the only thing standing there.
    if (!swapsInFlight.add(exerciseId)) return
    viewModelScope.launch {
        try {
            doSessionSwap(exerciseId, swap, makeDefault = false)
        } finally {
            swapsInFlight.remove(exerciseId)
        }
    }
}

/**
 * The shared session-swap body — applied by both "Just today" and "Make default". [suspend] so the
 * persistent path can await it before deciding whether to raise the dislike prompt.
 *
 * Order matters (H-11): the current session's row is written FIRST, and the future default is persisted
 * only when that write was applied. The repository refuses the whole swap once a set exists under the
 * row — a set the wrist logged while this sheet sat open — and on a refusal nothing may change: not the
 * row, not the customization, and not the sheet's claim that a swap is still possible.
 *
 * @return true when the swap was applied to this session.
 */
private suspend fun DayViewModel.doSessionSwap(exerciseId: String, swap: ExerciseDef, makeDefault: Boolean): Boolean {
    val leId = ensureLoggedExercise(exerciseId) ?: return false
    // Update only the swap fields via copy — preserves supersetGroup, hitFullTarget, note,
    // difficulty, orderIndex, etc. (the old positional rebuild silently dropped them). swap.id
    // re-keys exercise_id so PRs/stats follow the swapped exercise (#11).
    val reaction = swapReaction(
        workoutRepo.setSessionSwap(leId, swap.name, swap.unit.code, swap.id),
        makeDefault = makeDefault
    )
    if (reaction.persistDefault) {
        customizationRepo.setSwap(exerciseId, swap.name, swap.unit.code, swappedExerciseId = swap.id)
    }
    if (reaction.closeSheet) _state.update { it.copy(swapPickerForExerciseId = null) }
    reaction.message?.let { _messages.trySend(it) }
    // Refresh either way: on success the card re-keys; on a refusal it picks up the set that caused it.
    refreshExercises()
    return reaction.applied
}

private fun DayViewModel.applyPersistentSwap(exerciseId: String, swap: ExerciseDef) {
    // Drop a duplicate "Make default" for a slot already mid-swap: two concurrent runs would both
    // capture the same pre-swap original and both raise the dislike prompt, so dismissing the first
    // would instantly re-show the second. add() returns false when the slot is already in flight.
    if (!swapsInFlight.add(exerciseId)) return
    viewModelScope.launch {
        try {
            // Capture the exercise being replaced BEFORE the swap re-keys the card (#11): effectiveExerciseId
            // is its ExerciseLibrary id (what dislikes key on), effectiveName its display name — both needed
            // for the post-swap dislike prompt.
            val ui = _state.value.exercises.firstOrNull { it.plan.id == exerciseId }
            val originalId = ui?.effectiveExerciseId?.takeIf { it.isNotBlank() } ?: exerciseId
            val originalName = ui?.effectiveName ?: originalId

            // The customization is persisted INSIDE doSessionSwap, after the session write succeeds
            // (H-11) — and a refused swap swapped nothing out, so there is nothing to dislike.
            if (doSessionSwap(exerciseId, swap, makeDefault = true)) {
                maybeShowDislikePrompt(originalId, originalName, swap.id)
            }
        } finally {
            swapsInFlight.remove(exerciseId)
        }
    }
}

/** Raise the dislike prompt for the swapped-out exercise unless the user has muted it (pref off / "not
 *  this workout") or it'd be pointless (swapped to itself, or it's already disliked). */
private suspend fun DayViewModel.maybeShowDislikePrompt(originalId: String, originalName: String, swapId: String) {
    val show = shouldShowDislikePrompt(
        promptEnabled = settingsRepo.swapDislikePromptEnabled.first(),
        suppressedThisSession = dislikePromptSuppressedThisSession,
        originalId = originalId,
        swapId = swapId,
        alreadyDisliked = originalId in settingsRepo.dislikedExercises.first()
    )
    if (show) _state.update { it.copy(dislikeSwapPrompt = DislikeSwapPrompt(originalId, originalName)) }
}

/** Pure gate for the post-swap dislike prompt — unit-tested in DaySwapDislikePromptTest. */
internal fun shouldShowDislikePrompt(
    promptEnabled: Boolean,
    suppressedThisSession: Boolean,
    originalId: String,
    swapId: String,
    alreadyDisliked: Boolean
): Boolean =
    promptEnabled && !suppressedThisSession && originalId != swapId && !alreadyDisliked

private fun DayViewModel.dislikeSwappedExercise() {
    val prompt = _state.value.dislikeSwapPrompt ?: return
    // Clear synchronously so a rapid double-tap on "Hide" reads null and bails — otherwise both taps
    // see the prompt and fire setExercisesDisliked + the snackbar twice.
    _state.update { it.copy(dislikeSwapPrompt = null) }
    viewModelScope.launch {
        // A custom exercise spans several custom_… ids (one per day it's on); hide every copy, not just
        // the swapped slot, so "won't suggest it again" holds everywhere. For a library id this is just
        // the single id.
        val ids = programCustomRepo.customSiblingIds(prompt.exerciseId)
        settingsRepo.setExercisesDisliked(ids, true)
        _messages.trySend("Won't suggest ${prompt.exerciseName} again.")
    }
}

private fun DayViewModel.neverAskDislikePrompt() {
    viewModelScope.launch {
        settingsRepo.setSwapDislikePromptEnabled(false)
        _state.update { it.copy(dislikeSwapPrompt = null) }
        _messages.trySend("Turned off — re-enable in Settings → Exercise preferences.")
    }
}

private fun DayViewModel.clearPersistentSwap(exerciseId: String) {
    viewModelScope.launch {
        customizationRepo.clearSwap(exerciseId)
        // Applying a swap eagerly created a logged entry; clearing the swap must revert that still-empty
        // entry back to the slot, or the card stays stuck on the swapped exercise (#11).
        _state.value.exercises.firstOrNull { it.plan.id == exerciseId }?.loggedExerciseId
            ?.let { workoutRepo.revertSwapToSlotIfEmpty(it) }
        _state.update { it.copy(swapPickerForExerciseId = null) }
        refreshExercises()
    }
}
