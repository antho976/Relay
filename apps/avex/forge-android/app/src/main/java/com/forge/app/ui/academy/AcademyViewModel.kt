package com.forge.app.ui.academy

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AcademyRepository
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonTrack
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The Academy index: every lesson that ships, with what the ledger knows about each one.
 *
 * Reading state lives in [LessonViewModel] since 2026-08-20, when the lesson sheet became a screen.
 * This one only answers "what is on the shelf", which is why it can be a plain observer.
 */
@HiltViewModel
class AcademyViewModel @Inject constructor(
    private val academyRepo: AcademyRepository
) : ViewModel() {

    /**
     * ## The gate is gone (2026-08-16)
     *
     * `unlocked` used to decide VISIBILITY: 27 of the 31 lessons were hidden behind coach moments,
     * so a screen calling itself a hub of knowledge was an 87% locked list. Antho's read was that it
     * felt like an achievement tree rather than somewhere to learn, and he was right — that is what
     * a mostly-locked inventory is.
     *
     * Nothing in the domain changed to fix it. `unlocked` already meant "a coach moment fired for
     * this reader", which is a statement about RELEVANCE, not entitlement. The UI simply stopped
     * treating it as permission: every lesson is readable from install, and a fired moment now only
     * flags a lesson as relevant right now — the little poke, in Antho's words. The ledger, the
     * notifications feed, `ArrivalController` and the tab badge are all untouched and keep working,
     * because `isNew` (fired and unread) is still exactly what they count.
     */
    data class UiState(
        /**
         * Every lesson, ledger state attached. Nothing is filtered out of this list.
         *
         * Seeded from the registry rather than starting empty, for the same reason
         * [LibraryViewModel] does: the catalogue is static in-app content and only the read marks
         * come from the database, so the page can render in full on the first frame. Starting at
         * `emptyList()` opened the Academy on "0 PIECES" for a frame, which §12 calls a state
         * nobody drew rather than a loading state.
         */
        val all: List<AcademyRegistry.LessonState> = AcademyRegistry.stateFrom(emptyList()),
        /**
         * Whether [all] carries the ledger's read marks yet. The seeded list reads as all unread, so
         * the opening drawing waits for this rather than picking from it and reshuffling a moment
         * later. True by default so a state built by hand (a preview) renders in full.
         */
        val loaded: Boolean = true
    ) {
        val newCount: Int get() = all.count { it.isNew }

        /**
         * What the coach has flagged as relevant and the reader has not opened, newest first.
         *
         * They lead the page's opening drawing, which turns through them before anything unread.
         */
        val forYou: List<AcademyRegistry.LessonState>
            get() = all.filter { it.isNew }.sortedByDescending { it.unlockedAtMs ?: 0L }

        fun lessonsIn(track: LessonTrack): List<AcademyRegistry.LessonState> =
            // Registry order, which for Training IS its reading order. No sort by state: putting
            // "yours" first would re-impose a ranking the page does not claim.
            all.filter { it.lesson.track == track }
    }

    private val _state = MutableStateFlow(UiState(loaded = false))
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        // Observed rather than fetched once: a lesson is read on its own screen now, so the marks
        // change while this page is on the back stack and have to be true again when it returns.
        // Side by side, not one after the other: the sync builds an engine snapshot, and the read
        // marks in the contents list should not wait on it.
        viewModelScope.launch {
            runCatching {
                academyRepo.observeStates().collect { states ->
                    _state.update { it.copy(all = states) }
                }
            }
        }
        // The opening drawing picks from what the coach has flagged, so it holds until the sync has
        // run and its unlocks are read back, rather than choosing from a list about to change.
        viewModelScope.launch {
            runCatching { academyRepo.syncCoachMoments() }
            val synced = runCatching { academyRepo.states() }.getOrNull()
            _state.update { if (synced != null) it.copy(all = synced, loaded = true) else it.copy(loaded = true) }
        }
    }
}
