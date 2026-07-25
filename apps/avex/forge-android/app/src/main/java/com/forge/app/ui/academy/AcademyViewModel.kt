package com.forge.app.ui.academy

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AcademyRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.domain.academy.AcademyRegistry
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The Academy (Coach v3 B3): what you've unlocked, what's still ahead, and the lesson you're
 * reading.
 *
 * Upcoming lessons are shown locked-but-visible on purpose — the same rule as a COMING_SOON signal
 * slot. A user should be able to see the shape of what the coach knows before they've earned it.
 */
@HiltViewModel
class AcademyViewModel @Inject constructor(
    private val academyRepo: AcademyRepository,
    private val adaptationRepo: AdaptationRepository
) : ViewModel() {

    data class UiState(
        val unlocked: List<AcademyRegistry.LessonState> = emptyList(),
        val upcoming: List<AcademyRegistry.LessonState> = emptyList(),
        val openLessonId: String? = null,
        /** Live values for the reader's own numbers inside a lesson. */
        val examples: Map<String, String> = emptyMap()
    ) {
        val newCount: Int get() = unlocked.count { it.isNew }
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
            unlocked = states.filter { it.unlocked }.sortedByDescending { it.unlockedAtMs ?: 0L },
            upcoming = states.filter { !it.unlocked },
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
