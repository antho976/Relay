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
        val all: List<AcademyRegistry.LessonState> = AcademyRegistry.stateFrom(emptyList())
    ) {
        val newCount: Int get() = all.count { it.isNew }

        /**
         * What the coach has flagged as relevant and the reader has not opened, newest first.
         *
         * The page shows the first of these as its opening pointer, beside the same count the bell
         * and the tab badge already carry. It is never shown as a shelf: a "for you" queue holding
         * nine things is a backlog, and a backlog is the achievement feeling coming back in through
         * the side door.
         */
        val forYou: List<AcademyRegistry.LessonState>
            get() = all.filter { it.isNew }.sortedByDescending { it.unlockedAtMs ?: 0L }

        fun lessonsIn(track: LessonTrack): List<AcademyRegistry.LessonState> =
            // Registry order, which for Fundamentals IS its reading order. No sort by state: putting
            // "yours" first would re-impose the ranking this rework removed.
            all.filter { it.lesson.track == track }
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        // Observed rather than fetched once: a lesson is read on its own screen now, so the marks
        // change while this page is on the back stack and have to be true again when it returns.
        viewModelScope.launch {
            runCatching { academyRepo.syncCoachMoments() }
            runCatching {
                academyRepo.observeStates().collect { states ->
                    _state.value = _state.value.copy(all = states)
                }
            }
        }
    }
}
