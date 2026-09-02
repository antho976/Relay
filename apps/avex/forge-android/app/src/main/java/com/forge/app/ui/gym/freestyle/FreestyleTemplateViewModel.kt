package com.forge.app.ui.gym.freestyle

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.CustomExerciseRegistry
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
data class FreestyleTemplateSet(
    val weightLb: Double?,
    val reps: Int,
    /** Held seconds for a timed set (GYMAP-51); null for a rep set. */
    val durationSeconds: Int? = null
)

/**
 * One exercise from a template: a library id + the sets performed that session.
 *
 * [customName] is set for a user-created move, which has no [ExerciseLibrary] row to re-derive a
 * name from. Without it the conversion back into the logger resolved every id through the library
 * and `mapNotNull` dropped whatever it could not find — so a workout built from custom exercises
 * produced a template that silently omitted them, and one built ENTIRELY from them produced an
 * empty template that looked like a bug in the picker.
 */
data class FreestyleTemplateExercise(
    val libId: String,
    val sets: List<FreestyleTemplateSet>,
    val customName: String? = null,
    /**
     * [com.forge.app.program.ExerciseUnit.code] as it was logged, for a custom move.
     *
     * A library move re-derives its unit from its catalogue entry; a custom one has no entry, and
     * the rebuild assumed a weighted rep exercise. So a custom BODYWEIGHT movement came back with a
     * weight field, and a custom timed hold came back as a rep set — reusing a past workout quietly
     * changed what the movement was.
     */
    val unitCode: String? = null,
    /**
     * [com.forge.app.program.MuscleGroup.code] for a custom move, from the custom-exercise registry.
     * No logged row stores a muscle, so without this the rebuild defaulted every custom move to the
     * first enum value (Chest) and the athlete had to re-pick it on every reuse.
     */
    val muscleCode: String? = null
)

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
        // A custom move stores its name on the row (swappedName), because there is no library entry
        // to look it up in. Carried through so the logger can rebuild it.
        val customNames = LinkedHashMap<String, String>()
        val unitCodes = LinkedHashMap<String, String>()
        exercises.forEach { le ->
            val sets = loggedSetDao.forLoggedExercise(le.id)
                .sortedBy { it.setIndex }
                .map {
                    FreestyleTemplateSet(
                        weightLb = it.weightLb,
                        reps = it.reps,
                        durationSeconds = it.durationSeconds
                    )
                }
            byLib.getOrPut(le.exerciseId) { mutableListOf() }.addAll(sets)
            if (isCustomExerciseId(le.exerciseId)) {
                le.swappedName?.takeIf { it.isNotBlank() }?.let { customNames[le.exerciseId] = it }
                // The unit is on the row for exactly this reason; the rebuild was ignoring it.
                le.swappedUnit?.takeIf { it.isNotBlank() }?.let { unitCodes[le.exerciseId] = it }
            }
        }
        return byLib.map { (libId, sets) ->
            // The registry is the only place a custom move's muscle was ever kept; its name is the
            // fallback for a row whose swapped_name did not survive.
            val registered = if (isCustomExerciseId(libId)) CustomExerciseRegistry.get(libId) else null
            FreestyleTemplateExercise(
                libId, sets,
                customName = customNames[libId] ?: registered?.name,
                unitCode = unitCodes[libId],
                muscleCode = registered?.muscleCode
            )
        }
    }
}
