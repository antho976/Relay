package com.forge.app.ui.onboarding

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch

/** The saved resume draft, tri-state: don't compose the flow until the one-shot read lands. */
internal sealed interface DraftLoad {
    data object Loading : DraftLoad
    data class Ready(val draft: OnboardingDraft?) : DraftLoad
}

/**
 * The onboarding draft's synchronous state of truth, split out of [OnboardingViewModel] so the
 * lifecycle contract can be tested without Hilt, Room or DataStore.
 *
 * Every field the screen holds is plain `remember` state, which an Activity recreation (rotation,
 * multi-window resize) throws away; only the ViewModel survives. The old ViewModel exposed just its
 * one-shot disk read, so the recreated screen rehydrated from `Ready(null)` — back to page one with
 * defaults — and the debounced autosaver then wrote that blank snapshot over the good draft on disk.
 *
 * Two rules fix both halves:
 *
 * 1. [update] moves [state] forward **synchronously**, before any disk write is queued. A screen
 *    composed after a configuration change reads the answers the user last gave, not the answers
 *    the app started with. The DataStore write stays debounced behind it.
 * 2. A snapshot only reaches disk when it differs from the draft already held. On a fresh start
 *    (nothing loaded) the screen's first snapshot is its own defaults — it is adopted as the
 *    baseline so later edits diff against it, but it is never written. Nothing the user typed is in
 *    it, and if the disk read had failed rather than found nothing, writing it would have destroyed
 *    the real draft.
 */
internal class OnboardingDraftKeeper(
    scope: CoroutineScope,
    private val load: suspend () -> OnboardingDraft?,
    private val write: suspend (OnboardingDraft) -> Unit,
    private val debounceMs: Long = DEBOUNCE_MS
) {
    private val _state = MutableStateFlow<DraftLoad>(DraftLoad.Loading)

    /** [DraftLoad.Loading] until the one-shot read lands, then always the latest draft. */
    val state: StateFlow<DraftLoad> = _state

    private val pending = MutableStateFlow<OnboardingDraft?>(null)

    /** Flipped off the moment completion runs so a conflated save can't resurrect the draft the
     *  atomic completion write just removed. */
    @Volatile
    private var writesEnabled = true

    init {
        scope.launch { _state.value = DraftLoad.Ready(load()) }
        // Conflated autosave: rapid changes (typing a name) collapse into one write ~250ms after
        // the last keystroke — collectLatest cancels the stale snapshots.
        scope.launch {
            pending.filterNotNull().collectLatest { draft ->
                delay(debounceMs)
                if (writesEnabled) write(draft)
            }
        }
    }

    /** Record the screen's current answers. Cheap to call on every recomposition: an unchanged
     *  snapshot is a no-op, so the write queue only ever sees real edits. */
    fun update(draft: OnboardingDraft) {
        val current = _state.value as? DraftLoad.Ready ?: return
        if (current.draft == draft) return
        _state.value = DraftLoad.Ready(draft)
        // Rule 2: the first snapshot of a fresh flow is the baseline, not an edit.
        if (current.draft == null) return
        pending.value = draft
    }

    /** Stop the autosaver for good — called before the completion write removes the draft. */
    fun stopWrites() {
        writesEnabled = false
    }

    private companion object {
        const val DEBOUNCE_MS = 250L
    }
}
