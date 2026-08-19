package com.forge.app.ui.academy

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.LibraryRepository
import com.forge.app.domain.academy.ArticleRegistry
import com.forge.app.domain.academy.ArticleTopic
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The Library index: everything that ships, filtered by what it is about.
 *
 * The filter axis is deliberately topic and never difficulty. "What is this about" is a question a
 * reader can answer before opening something; "how hard is it" is one they can only answer after,
 * so making it a filter would hide articles behind a judgement the reader has not made yet. The
 * level rides along as a label on the row instead.
 */
@HiltViewModel
class LibraryViewModel @Inject constructor(
    private val libraryRepo: LibraryRepository
) : ViewModel() {

    data class UiState(
        /**
         * Seeded from the registry rather than starting empty. The catalogue is static in-app
         * content and only the read marks come from the database, so the list can render in full on
         * the first frame and let the ledger refine it. Starting at `emptyList()` would flash the
         * "still being written" hint at every reader for one frame, which §12 calls a state nobody
         * drew rather than a loading state.
         */
        val articles: List<ArticleRegistry.ArticleState> = ArticleRegistry.stateFrom(emptyList()),
        val query: String = "",
        /** Null is "everything". Only topics that hold an article are ever offered. */
        val topic: ArticleTopic? = null
    ) {
        /** The topics with content, in enum order — the filter row. Empty until content exists. */
        val topics: List<ArticleTopic> get() = ArticleRegistry.topicsWithContent()

        /**
         * The rows to draw: topic filter first, then a free-text match over the title and deck.
         *
         * Order is the registry's own, which is authoring order within each topic. There is no
         * "recommended" sort and no recency sort, because both imply the Library has an opinion
         * about what you should read next, and it does not.
         */
        val visible: List<ArticleRegistry.ArticleState>
            get() = articles
                .filter { topic == null || topic in it.article.topics }
                .filter { state ->
                    query.isBlank() ||
                        state.article.title.contains(query, ignoreCase = true) ||
                        state.article.deck.contains(query, ignoreCase = true)
                }

        /** True when a search or filter is responsible for an empty list, rather than an empty shelf. */
        val filtered: Boolean get() = query.isNotBlank() || topic != null
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            libraryRepo.observeStates().collect { states ->
                _state.value = _state.value.copy(articles = states)
            }
        }
    }

    fun onQueryChange(query: String) {
        _state.value = _state.value.copy(query = query)
    }

    /** Tapping the selected topic clears it, so the row never traps you inside one shelf. */
    fun onTopicSelected(topic: ArticleTopic?) {
        _state.value = _state.value.copy(
            topic = if (topic == _state.value.topic) null else topic
        )
    }
}
