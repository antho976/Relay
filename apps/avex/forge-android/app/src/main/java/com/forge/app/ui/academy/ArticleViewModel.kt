package com.forge.app.ui.academy

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.LibraryRepository
import com.forge.app.domain.academy.Article
import com.forge.app.domain.academy.ArticleRegistry
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * One article, read.
 *
 * Opening is recorded as soon as the screen resolves its id; finishing waits for the reader to
 * actually reach the last block ([onReachedEnd]). Both writes are idempotent in the repository, so
 * a rotation or a back-and-forward cannot inflate either.
 */
@HiltViewModel
class ArticleViewModel @Inject constructor(
    private val libraryRepo: LibraryRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val articleId: String = savedStateHandle.get<String>(Routes.ARG_ARTICLE_ID).orEmpty()

    data class UiState(
        val article: Article? = null,
        val finished: Boolean = false,
        /** The next piece on the same shelf, or null when this one closes it. */
        val next: NextPiece? = null
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        val article = libraryRepo.article(articleId)
        _state.value = UiState(article = article, next = article?.let(::nextAfter))
        if (article != null) {
            viewModelScope.launch {
                runCatching { libraryRepo.markOpened(article.id) }
                val finished = runCatching { libraryRepo.stateOf(article.id)?.finished }.getOrNull() == true
                _state.update { it.copy(finished = finished) }
            }
        }
    }

    /**
     * Where a reader goes from the end of an article.
     *
     * The next one on the same shelf, in the registry's own order, and nothing when this is the
     * last. Deliberately "more in Recovery" rather than "next": the Library has topics and no
     * curriculum, and a reader who finished one article is owed a neighbour, not an instruction.
     */
    private fun nextAfter(article: Article): NextPiece? {
        val shelf = ArticleRegistry.byTopic(article.topic)
        val at = shelf.indexOfFirst { it.id == article.id }
        val next = shelf.getOrNull(at + 1) ?: return null
        return NextPiece(
            id = next.id,
            title = next.title,
            lead = "More in ${article.topic.displayName}",
            minutes = next.readMinutes
        )
    }

    /** The last block scrolled into view. Recorded once, and only ever from a real scroll. */
    fun onReachedEnd() {
        val article = _state.value.article ?: return
        if (_state.value.finished) return
        _state.update { it.copy(finished = true) }
        viewModelScope.launch { runCatching { libraryRepo.markFinished(article.id) } }
    }
}
