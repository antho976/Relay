package com.forge.app.ui.academy

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.AcademyRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.Lesson
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * One lesson, read on its own page.
 *
 * The mirror of [ArticleViewModel], deliberately: opening is recorded as soon as the screen
 * resolves its id, and finishing waits for the reader to actually reach the end. The sheet this
 * replaced recorded completion from its DISMISSAL, which counted a bounce as a read and so
 * corrupted the one signal the ledger keeps. Both writes are idempotent in the repository, so a
 * rotation or a back-and-forward cannot inflate either.
 */
@HiltViewModel
class LessonViewModel @Inject constructor(
    private val academyRepo: AcademyRepository,
    private val adaptationRepo: AdaptationRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val lessonId: String = savedStateHandle.get<String>(Routes.ARG_LESSON_ID).orEmpty()

    data class UiState(
        val lesson: Lesson? = null,
        /** Live values for the reader's own numbers inside a lesson. */
        val examples: Map<String, String> = emptyMap(),
        val next: NextPiece? = null,
        val finished: Boolean = false
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        val lesson = AcademyRegistry.lesson(lessonId)
        _state.value = UiState(lesson = lesson, next = lesson?.let(::nextAfter))
        if (lesson != null) {
            viewModelScope.launch {
                runCatching { academyRepo.markOpened(lesson.id) }
                val loadedExamples = runCatching { examples() }.getOrDefault(emptyMap())
                _state.update { it.copy(examples = loadedExamples) }
            }
        }
    }

    /** The last block scrolled into view. Recorded once, and only ever from a real scroll. */
    fun onReachedEnd() {
        val lesson = _state.value.lesson ?: return
        if (_state.value.finished) return
        _state.update { it.copy(finished = true) }
        viewModelScope.launch { runCatching { academyRepo.markCompleted(lesson.id) } }
    }

    /**
     * Where a reader goes from the end of this lesson.
     *
     * Within a track, the registry's order IS the authoring order, so the following lesson is a
     * real "next" in Fundamentals (the one track written to be read start to finish) and honestly
     * only "more" anywhere else. At the end of a track it steps to the head of the following one,
     * so no lesson is a dead end — the last one in the last track is, and that is the truth.
     */
    private fun nextAfter(lesson: Lesson): NextPiece? {
        val siblings = AcademyRegistry.byTrack(lesson.track)
        val at = siblings.indexOfFirst { it.id == lesson.id }
        val ordered = lesson.track == LessonTrack.FUNDAMENTALS

        siblings.getOrNull(at + 1)?.let {
            return it.piece(if (ordered) "Next in ${lesson.track.displayName}" else "More in ${lesson.track.displayName}")
        }

        val nextTrack = LessonTrack.entries.getOrNull(lesson.track.ordinal + 1) ?: return null
        return AcademyRegistry.byTrack(nextTrack).firstOrNull()?.piece("Next chapter")
    }

    private fun Lesson.piece(lead: String) = NextPiece(
        id = id,
        title = title,
        lead = lead,
        minutes = blocks.readMinutes()
    )

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
