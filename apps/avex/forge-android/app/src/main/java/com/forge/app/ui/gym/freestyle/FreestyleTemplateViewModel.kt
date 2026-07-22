package com.forge.app.ui.gym.freestyle

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/** One row in the template picker: a past finished workout, summarised for a glance. */
data class FreestyleTemplateSummary(
    val sessionId: Long,
    /** Human day name — a program day ("Pull B") or "Open workout" for a past freestyle log. */
    val title: String,
    val startedAtMs: Long,
    /** The moves that session contained, in order — the row's at-a-glance preview. */
    val exerciseNames: List<String>,
    val setCount: Int
)

/** One drafted set from a template: the load performed (null = bodyweight) and reps. */
data class FreestyleTemplateSet(val weightLb: Double?, val reps: Int)

/** One exercise from a template: a library id + the sets performed that session. */
data class FreestyleTemplateExercise(val libId: String, val sets: List<FreestyleTemplateSet>)

/**
 * Backs the freestyle "start from a past workout" picker (GYMAP-48). Lists every finished session as a
 * reusable template and, on pick, resolves that session's exercises + sets into a seed the logger loads
 * as its starting point. Reads the DAOs directly (rather than routing through WorkoutRepository) so the
 * live [FreestyleLogViewModel] stays untouched — this is a self-contained, screen-local concern.
 */
@HiltViewModel
class FreestyleTemplateViewModel @Inject constructor(
    sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao
) : ViewModel() {

    /**
     * Every finished workout as a template row, newest first. Joins each session to its (non-skipped)
     * exercise names off the main thread; sessions that logged nothing are dropped (nothing to reuse).
     */
    val templates: StateFlow<List<FreestyleTemplateSummary>> = combine(
        sessionDao.observeAllFinishedSessions(),
        loggedExerciseDao.observeSessionExerciseIds()
    ) { sessions, exerciseRows ->
        val namesBySession = exerciseRows
            .groupBy { it.sessionId }
            .mapValues { (_, rows) -> rows.map { Program.exerciseDisplayName(it.exerciseId, it.swappedName) } }
        sessions.mapNotNull { s ->
            val names = namesBySession[s.id].orEmpty()
            if (names.isEmpty()) return@mapNotNull null
            FreestyleTemplateSummary(
                sessionId = s.id,
                title = Program.dayDisplayName(s.dayKey),
                startedAtMs = s.startedAt,
                exerciseNames = names,
                setCount = s.setCount
            )
        }
    }.flowOn(Dispatchers.Default)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    /**
     * Resolve a past session into a seed for the logger: its exercises in performed order, each with the
     * sets logged that day. Skipped entries are dropped (they weren't done) and a repeated exercise is
     * merged into one entry (its sets concatenated) so the logger's libId-keyed list stays unique.
     */
    suspend fun loadTemplate(sessionId: Long): List<FreestyleTemplateExercise> {
        val exercises = loggedExerciseDao.forSession(sessionId).filter { !it.skipped }
        val byLib = LinkedHashMap<String, MutableList<FreestyleTemplateSet>>()
        exercises.forEach { le ->
            val sets = loggedSetDao.forLoggedExercise(le.id)
                .sortedBy { it.setIndex }
                .map { FreestyleTemplateSet(weightLb = it.weightLb, reps = it.reps) }
            byLib.getOrPut(le.exerciseId) { mutableListOf() }.addAll(sets)
        }
        return byLib.map { (libId, sets) -> FreestyleTemplateExercise(libId, sets) }
    }
}
