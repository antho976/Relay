package com.forge.app.ui.programbuilder

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.ProgramChangeGuard
import com.forge.app.ui.common.moved
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/**
 * In-memory routine builder. The user edits days/exercises freely (add, rename, reorder, remove, set
 * sets/reps); nothing persists until [save], which writes the whole plan as the base program
 * ([ProgramRepository.saveCustomProgram]) — no overlay. Launched blank (build-your-own from
 * onboarding) or pre-loaded from the current program (edit existing).
 */
@HiltViewModel
class ProgramBuilderViewModel @Inject constructor(
    private val programRepository: ProgramRepository,
    private val settingsRepo: SettingsRepository,
    private val programChangeGuard: ProgramChangeGuard
) : ViewModel() {

    var days by mutableStateOf<List<BuilderDay>>(emptyList())
        private set
    var dirty by mutableStateOf(false)
        private set
    var saving by mutableStateOf(false)
        private set

    /**
     * The initial load has finished (or was never needed). Editing and Save wait on it.
     *
     * `loadIfNeeded` set a flag and launched, so the screen rendered an EMPTY builder while the rows
     * were still coming back from Room. Two things followed from that. An edit made in the gap was
     * overwritten wholesale when `days = loadDays()` landed — the user's change simply vanished, and
     * the plan they were looking at was replaced by the one on disk. And Save was enabled over that
     * same empty list, which does not save nothing: it writes an empty program over the real one.
     */
    var loadComplete by mutableStateOf(false)
        private set

    /** Currently "go with the flow" — saving a plan switches to follow-a-plan, so the screen confirms
     *  first (see [save], which performs the flip). */
    val freestyleMode: StateFlow<Boolean> =
        settingsRepo.freestyleMode.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)

    private var loaded = false

    /** Inverse of the last destructive remove — re-inserts just that item at its original slot, so a
     *  snackbar Undo never rolls back edits made in between (§13: undo over confirm). */
    /**
     * Inverse of the LAST destructive remove — newest wins, and the caller is told when it has
     * replaced one, so the earlier snackbar can be dismissed rather than left on screen offering an
     * Undo that no longer means what it says.
     *
     * Remove A, remove B before A's snackbar times out, tap Undo on A's snackbar: one global closure
     * had already been overwritten, so B came back and A stayed gone. Two removals and one action,
     * and the action undid the wrong one.
     */
    private var undoRemoval: (() -> Unit)? = null

    private fun uid() = UUID.randomUUID().toString().take(8)

    /** Load once: blank for build-your-own, or the current program's rows for editing. */
    fun loadIfNeeded(blank: Boolean) {
        if (loaded) return
        loaded = true
        if (blank) { days = emptyList(); loadComplete = true; return }
        viewModelScope.launch {
            val rows = loadDays()
            // `dirty` cannot be true yet — the screen gates editing on loadComplete — but the check
            // states the rule rather than relying on the gate one layer up: a late snapshot never
            // overwrites edits the user has already made.
            if (!dirty) days = rows
            loadComplete = true
        }
    }

    /** Drop unsaved edits and reload the persisted program — Back out of a pen-edit into view mode. */
    fun discardEdits() {
        dirty = false
        undoRemoval = null
        loadComplete = false
        viewModelScope.launch {
            days = loadDays()
            loadComplete = true
        }
    }

    private suspend fun loadDays(): List<BuilderDay> =
        programRepository.currentDayRows().map { pd ->
            val slots = programRepository.slotRowsForDay(pd.id)
            BuilderDay(
                uid = uid(), key = pd.id, name = pd.name, archetype = pd.archetype,
                accentHex = pd.accentHex, word = pd.word,
                exercises = slots.map { s ->
                    val def = ExerciseLibrary.byId(s.exerciseLibId)
                    BuilderExercise(uid(), s.exerciseLibId, def?.name ?: s.exerciseLibId,
                        def?.muscle?.displayName ?: "", s.sets, s.reps)
                }
            )
        }

    private fun mutate(block: (List<BuilderDay>) -> List<BuilderDay>) {
        days = block(days)
        dirty = true
    }

    private fun mutateDay(dayUid: String, block: (BuilderDay) -> BuilderDay) =
        mutate { list -> list.map { if (it.uid == dayUid) block(it) else it } }

    fun addDay() = mutate { list ->
        list + BuilderDay(
            uid = uid(), key = "day-${uid()}", name = "Day ${list.size + 1}",
            archetype = "fb", accentHex = DAY_ACCENTS[list.size % DAY_ACCENTS.size], exercises = emptyList()
        )
    }

    fun removeDay(dayUid: String) {
        val index = days.indexOfFirst { it.uid == dayUid }
        if (index < 0) return
        val removed = days[index]
        mutate { it.filterNot { d -> d.uid == dayUid } }
        // Undo re-inserts only this day at its old slot, so edits made while the snackbar showed survive.
        stageUndo { mutate { list -> list.toMutableList().apply { add(index.coerceAtMost(size), removed) } } }
    }

    /**
     * Stage [inverse] as the one thing Undo will do — newest wins.
     *
     * The screen dismisses the snackbar for any earlier removal at the same moment (see
     * `removedWithUndo`), so there is never a visible Undo whose action belongs to a different
     * removal than the message beside it.
     */
    private fun stageUndo(inverse: () -> Unit) { undoRemoval = inverse }

    /** Insert a copy of the day (fresh uids/key, "Name 2") right after the original. */
    fun duplicateDay(dayUid: String) = mutate { list ->
        val i = list.indexOfFirst { it.uid == dayUid }
        if (i < 0) return@mutate list
        val src = list[i]
        val copy = src.copy(
            uid = uid(), key = "day-${uid()}",
            name = copyName(src.name, list.map { it.name }.toSet()),
            exercises = src.exercises.map { it.copy(uid = uid()) }
        )
        list.toMutableList().apply { add(i + 1, copy) }
    }

    fun renameDay(dayUid: String, name: String) = mutateDay(dayUid) { it.copy(name = name) }
    fun setDayType(dayUid: String, archetype: String) = mutateDay(dayUid) { it.copy(archetype = archetype) }
    fun setDayAccent(dayUid: String, hex: String) = mutateDay(dayUid) { it.copy(accentHex = hex) }
    fun moveDay(from: Int, to: Int) = mutate { it.moved(from, to) }

    fun addExercises(dayUid: String, libIds: Collection<String>) = mutateDay(dayUid) { day ->
        val added = libIds.mapNotNull { id ->
            val def = ExerciseLibrary.byId(id) ?: return@mapNotNull null
            BuilderExercise(uid(), def.id, def.name, def.muscle.displayName, def.defaultSets, def.defaultReps)
        }
        day.copy(exercises = day.exercises + added)
    }

    fun removeExercise(dayUid: String, exUid: String) {
        val day = days.firstOrNull { it.uid == dayUid } ?: return
        val index = day.exercises.indexOfFirst { it.uid == exUid }
        if (index < 0) return
        val removed = day.exercises[index]
        mutateDay(dayUid) { it.copy(exercises = it.exercises.filterNot { e -> e.uid == exUid }) }
        // Undo re-inserts only this exercise at its old slot, leaving any later edits intact.
        stageUndo {
            mutateDay(dayUid) { d ->
                d.copy(exercises = d.exercises.toMutableList().apply { add(index.coerceAtMost(size), removed) })
            }
        }
    }

    /** Re-apply the last remove's inverse (snackbar Undo); a no-op once consumed or superseded. */
    fun undoRemove() {
        undoRemoval?.invoke()
        undoRemoval = null
    }

    fun setExercise(dayUid: String, exUid: String, sets: Int, reps: String) = mutateDay(dayUid) { day ->
        day.copy(exercises = day.exercises.map { if (it.uid == exUid) it.copy(sets = sets, reps = reps) else it })
    }

    /** Replace the exercise in place — same slot, same sets × reps, new movement. */
    fun swapExercise(dayUid: String, exUid: String, newLibId: String) = mutateDay(dayUid) { day ->
        val def = ExerciseLibrary.byId(newLibId) ?: return@mutateDay day
        day.copy(exercises = day.exercises.map {
            if (it.uid == exUid) it.copy(libId = def.id, name = def.name, muscle = def.muscle.displayName) else it
        })
    }

    fun moveExercise(dayUid: String, from: Int, to: Int) =
        mutateDay(dayUid) { it.copy(exercises = it.exercises.moved(from, to)) }

    fun day(dayUid: String): BuilderDay? = days.firstOrNull { it.uid == dayUid }

    /** Persist the built plan and make it live, then invoke [onSaved] on the main thread. */
    fun save(onSaved: () -> Unit) {
        if (saving) return
        saving = true
        viewModelScope.launch {
            // finally so a throw from saveCustomProgram / setFreestyleMode (DB or DataStore failure) —
            // or the guard cancelling — always releases the flag; otherwise saving stays true and the
            // Save button is permanently locked for this screen's lifetime.
            try {
                val (dayRows, slotRows) = days.toEntities()
                // saveCustomProgram rewrites the base program and discards any in-progress workout
                // (CASCADE-deleting its logged sets). Route through the shared guard so an active session
                // raises the same "discard & continue?" confirm the generate / deload / re-roll paths use
                // instead of silently wiping logged work; on cancel the staged save is dropped and the
                // builder stays open (dirty), so nothing is lost.
                programChangeGuard.run {
                    programRepository.saveCustomProgram(dayRows, slotRows)
                    settingsRepo.setFreestyleMode(false)
                    dirty = false
                    onSaved()
                }
            } finally {
                saving = false
            }
        }
    }
}
