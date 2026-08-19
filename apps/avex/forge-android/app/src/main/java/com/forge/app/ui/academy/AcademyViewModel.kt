package com.forge.app.ui.academy

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AcademyRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonTrack
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The Academy (Coach v3 B3): everything the coach knows, and the lesson you're reading.
 *
 * Every lesson is open. See [UiState] for why that changed and what it cost (nothing).
 */
@HiltViewModel
class AcademyViewModel @Inject constructor(
    private val academyRepo: AcademyRepository,
    private val adaptationRepo: AdaptationRepository
) : ViewModel() {

    /**
     * ## The gate is gone (2026-08-16)
     *
     * `unlocked` used to decide VISIBILITY: 27 of the 31 lessons were hidden behind coach moments, so
     * a screen calling itself a hub of knowledge was 87% locked list. Antho's read was that it felt
     * like an achievement tree rather than somewhere to learn, and he was right — that is what a
     * mostly-locked inventory is.
     *
     * Nothing in the domain changed to fix it. `unlocked` already meant "a coach moment fired for
     * this reader", which is a statement about RELEVANCE, not entitlement. The UI simply stopped
     * treating it as permission: every lesson is readable from install, and a fired moment now only
     * flags a lesson as relevant right now — the little poke, in Antho's words. The ledger, the
     * notifications feed, `ArrivalController` and the tab badge are all untouched and keep working,
     * because `isNew` (fired and unread) is still exactly what they count.
     *
     * [LessonUnlock]'s label/detail are consequently no longer rendered anywhere. They stay on the
     * model: they are the authoring record of WHY each lesson exists, and `orphanLessons()` still
     * audits against them.
     */
    data class UiState(
        /** Every lesson, ledger state attached. Nothing is filtered out of this list. */
        val all: List<AcademyRegistry.LessonState> = emptyList(),
        val openLessonId: String? = null,
        /** Live values for the reader's own numbers inside a lesson. */
        val examples: Map<String, String> = emptyMap()
    ) {
        val newCount: Int get() = all.count { it.isNew }

        /**
         * What the coach has flagged as relevant and the reader has not opened, newest first.
         *
         * This is the poke made visible on the page, beside the same count the bell and the tab
         * badge already carry. Capped by the caller — a "for you" shelf holding nine things is a
         * backlog, and a backlog is the achievement feeling coming back in through the side door.
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
        refresh()
    }

    fun refresh() = viewModelScope.launch {
        runCatching { academyRepo.syncCoachMoments() }
        val states = runCatching { academyRepo.states() }.getOrDefault(emptyList())
        _state.value = _state.value.copy(
            all = states,
            examples = runCatching { examples() }.getOrDefault(emptyMap())
        )
    }

    /** Opening a lesson is a ledger moment: it's what clears the "new" chip. */
    fun open(lessonId: String) {
        _state.value = _state.value.copy(openLessonId = lessonId)
        viewModelScope.launch {
            runCatching { academyRepo.markOpened(lessonId) }
            refresh()
        }
    }

    /** Reaching the end counts as completion; both are recorded once. */
    fun close(finished: Boolean) {
        val id = _state.value.openLessonId
        _state.value = _state.value.copy(openLessonId = null)
        if (finished && id != null) {
            viewModelScope.launch {
                runCatching { academyRepo.markCompleted(id) }
                refresh()
            }
        }
    }

    /** The reader's own numbers, for [com.forge.app.domain.academy.LessonBlock.Example] slots. */
    private suspend fun examples(): Map<String, String> {
        val readiness = runCatching { adaptationRepo.readinessScale() }.getOrNull()
        return buildMap {
            readiness?.let {
                put("readiness_today", "${if (it.percent > 0) "+" else ""}${it.percent}%")
                put("readiness_parts", it.reason)
            }
        }
    }
}
