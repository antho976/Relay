package com.forge.app.ui.gym.notes

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import javax.inject.Inject

data class NotesSearchUiState(
    val query: String = "",
    val results: List<LoggedExerciseDao.NoteSearchResult> = emptyList()
)

@OptIn(FlowPreview::class)
@HiltViewModel
class NotesSearchViewModel @Inject constructor(
    private val loggedExerciseDao: LoggedExerciseDao
) : ViewModel() {

    private val _query = MutableStateFlow("")

    val state: StateFlow<NotesSearchUiState> = _query
        .debounce(300)
        .flatMapLatest { q ->
            flow {
                emit(if (q.isBlank()) emptyList() else loggedExerciseDao.searchNotes(q))
            }
        }
        .map { results -> NotesSearchUiState(query = _query.value, results = results) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), NotesSearchUiState())

    fun setQuery(q: String) = _query.update { q }
}
