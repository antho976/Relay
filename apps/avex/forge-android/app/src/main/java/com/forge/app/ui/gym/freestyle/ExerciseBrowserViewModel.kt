package com.forge.app.ui.gym.freestyle

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExerciseLibrary
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.concurrent.TimeUnit
import javax.inject.Inject

/**
 * Backs the freestyle exercise browser: the async data the pure library filter can't derive —
 * bookmarked ids and the most-performed "recent" moves. All muscle/search/equipment filtering stays
 * a pure function ([browseLibrary]) the screen owns; this only supplies favorites + recency.
 */
@HiltViewModel
class ExerciseBrowserViewModel @Inject constructor(
    private val loggedExerciseDao: LoggedExerciseDao,
    private val settingsRepo: SettingsRepository
) : ViewModel() {

    /** Bookmarked exercise ids — drives the per-card star and the Favorites filter. */
    val favorites: StateFlow<Set<String>> = settingsRepo.favoriteExercises
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptySet())

    private val _recent = MutableStateFlow<List<ExerciseDef>>(emptyList())
    /** Most-performed moves over the last 90 days, resolved to library defs (top [RECENT_LIMIT]). */
    val recent: StateFlow<List<ExerciseDef>> = _recent.asStateFlow()

    init {
        viewModelScope.launch {
            val since = System.currentTimeMillis() - TimeUnit.DAYS.toMillis(RECENT_WINDOW_DAYS)
            _recent.value = loggedExerciseDao.frequencySince(since)
                .mapNotNull { ExerciseLibrary.byId(it.exerciseId) }
                .filter { !it.curatedOnly }
                .take(RECENT_LIMIT)
        }
    }

    fun toggleFavorite(libId: String, on: Boolean) {
        viewModelScope.launch { settingsRepo.toggleFavorite(libId, on) }
    }

    private companion object {
        const val RECENT_WINDOW_DAYS = 90L
        const val RECENT_LIMIT = 8
    }
}
