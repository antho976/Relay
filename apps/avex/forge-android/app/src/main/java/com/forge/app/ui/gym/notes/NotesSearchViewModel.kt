package com.forge.app.ui.gym.notes

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import javax.inject.Inject

data class NotesSearchUiState(
    val query: String = "",
    val results: List<LoggedExerciseDao.NoteSearchResult> = emptyList(),
    /** A query is typed and its search has not returned yet — [results] is empty, not "no matches". */
    val searching: Boolean = false
)

@OptIn(FlowPreview::class, kotlinx.coroutines.ExperimentalCoroutinesApi::class)
@HiltViewModel
class NotesSearchViewModel @Inject constructor(
    private val loggedExerciseDao: LoggedExerciseDao
) : ViewModel() {

    private val _query = MutableStateFlow("")

    // The field's text comes from _query directly (instant — every keystroke), while results are
    // the debounced DB search. Driving the TextField from the debounced flow made it lag/stutter.
    //
    // The results carry the query they ANSWER, and the state only publishes a pair that agrees.
    // Combining the live query with independently debounced results emitted every crossing of the
    // two: for the ~300 ms after each keystroke the screen showed the new query above the previous
    // query's hits, which is not a stale list — it is a list that looks like an answer and is not
    // one. Now a query with no result yet shows nothing, which is the truth.
    val state: StateFlow<NotesSearchUiState> = combine(
        _query,
        _query
            .debounce(300)
            .flatMapLatest { q ->
                // Escape LIKE wildcards so a literal '%' or '_' in the query isn't a wildcard.
                flow {
                    emit(q to if (q.isBlank()) emptyList() else loggedExerciseDao.searchNotes(escapeLikePattern(q)))
                }
            }
            .onStart { emit("" to emptyList()) }
    ) { query, (answeredQuery, results) ->
        NotesSearchUiState(
            query = query,
            results = if (answeredQuery == query) results else emptyList(),
            // Typed something, and the search for it has not come back yet. The screen can say so
            // instead of showing the last query's hits under this one.
            searching = query.isNotBlank() && answeredQuery != query
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), NotesSearchUiState())

    fun setQuery(q: String) = _query.update { q }
}

/** Escape SQL LIKE wildcards (matching `ESCAPE '\'` in the query) so user text matches literally. */
private fun escapeLikePattern(q: String): String =
    q.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
