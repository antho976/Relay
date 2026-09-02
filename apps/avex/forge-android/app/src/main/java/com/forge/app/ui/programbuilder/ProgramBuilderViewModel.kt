package com.forge.app.ui.programbuilder

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.program.ExerciseLibrary
import com.forge.app.ui.common.ProgramChangeGuard
import com.forge.app.ui.common.moved
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/**
 * The slice of the data layer the builder touches. An interface so a test can stand in a fake and
 * construct the real ViewModel (the repositories are final and need Room, DataStore and Health
 * Connect behind them); production binds [RepositoryProgramBuilderStore] in the @Inject constructor.
 */
internal interface ProgramBuilderStore {
    val freestyleMode: Flow<Boolean>
    suspend fun currentDayRows(): List<ProgramDay>
    suspend fun slotRowsForDay(dayId: String): List<ProgramSlot>
    suspend fun saveCustomProgram(days: List<ProgramDay>, slots: List<ProgramSlot>)
    suspend fun setFreestyleMode(v: Boolean)
    /** Run [action] through the shared program-change guard (confirm first when a workout is active). */
    suspend fun guardProgramChange(action: suspend () -> Unit)
}

internal class RepositoryProgramBuilderStore(
    private val programRepository: ProgramRepository,
    private val settingsRepo: SettingsRepository,
    private val programChangeGuard: ProgramChangeGuard
) : ProgramBuilderStore {
    override val freestyleMode: Flow<Boolean> get() = settingsRepo.freestyleMode
    override suspend fun currentDayRows(): List<ProgramDay> = programRepository.currentDayRows()
    override suspend fun slotRowsForDay(dayId: String): List<ProgramSlot> = programRepository.slotRowsForDay(dayId)
    override suspend fun saveCustomProgram(days: List<ProgramDay>, slots: List<ProgramSlot>) {
        programRepository.saveCustomProgram(days, slots)
    }
    override suspend fun setFreestyleMode(v: Boolean) {
        settingsRepo.setFreestyleMode(v)
    }
    override suspend fun guardProgramChange(action: suspend () -> Unit) {
        programChangeGuard.run(action = action)
    }
}

/**
 * In-memory routine builder. The user edits days/exercises freely (add, rename, reorder, remove, set
 * sets/reps); nothing persists until [save], which writes the whole plan as the base program
 * ([ProgramRepository.saveCustomProgram]) — no overlay. Launched blank (build-your-own from
 * onboarding) or pre-loaded from the current program (edit existing).
 *
 * "In-memory" survives the process: every edit and every editor move (open a day, open its sheet)
 * writes the whole draft into [SavedStateHandle] as one small JSON string, and a ViewModel recreated
 * after Android killed the process behind a retained task restores from it in `init`, before
 * [loadIfNeeded] can reload the saved program over it. Before this, that recreation quietly produced
 * an empty, non-dirty builder, so every unsaved day and exercise was gone and no discard warning
 * appeared. The draft is cleared only by [save] and [discardEdits].
 */
@HiltViewModel
class ProgramBuilderViewModel internal constructor(
    private val store: ProgramBuilderStore,
    private val savedStateHandle: SavedStateHandle
) : ViewModel() {

    @Inject
    constructor(
        programRepository: ProgramRepository,
        settingsRepo: SettingsRepository,
        programChangeGuard: ProgramChangeGuard,
        savedStateHandle: SavedStateHandle
    ) : this(RepositoryProgramBuilderStore(programRepository, settingsRepo, programChangeGuard), savedStateHandle)

    var days by mutableStateOf<List<BuilderDay>>(emptyList())
        private set
    var dirty by mutableStateOf(false)
        private set
    var saving by mutableStateOf(false)
        private set

    /** The day open in [ProgramBuilderDayDetail], or null on the plan overview. */
    var openDayUid by mutableStateOf<String?>(null)
        private set

    /** The dialog/sheet open inside the day editor. */
    var dayDialog by mutableStateOf<DayDialog>(DayDialog.None)
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
        store.freestyleMode.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)

    private var loaded = false

    init {
        // Restore BEFORE the screen's loadIfNeeded runs: a restored draft counts as loaded, so the
        // saved program is never fetched over the top of it. A blob this build cannot read (older
        // schema, corrupt) is ignored and the builder loads the saved program as it always did.
        savedStateHandle.get<String>(KEY_DRAFT)?.let { ProgramBuilderDraft.fromJson(it) }?.let { draft ->
            days = draft.days
            dirty = draft.dirty
            openDayUid = draft.openDayUid
            dayDialog = draft.dialog
            loaded = true
            loadComplete = true
        }
    }

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
        openDayUid = null
        dayDialog = DayDialog.None
        clearDraft()
        loadComplete = false
        viewModelScope.launch {
            days = loadDays()
            loadComplete = true
        }
    }

    /** Open [dayUid] in the day editor (a stale uid simply renders the overview — see [day]). */
    fun openDay(dayUid: String) {
        openDayUid = dayUid
        dayDialog = DayDialog.None
        persistDraft()
    }

    /** Back out of the day editor to the plan overview. */
    fun closeDay() {
        openDayUid = null
        dayDialog = DayDialog.None
        persistDraft()
    }

    /**
     * Open (or, with [DayDialog.None], close) a dialog/sheet inside the day editor.
     *
     * NOT `setDayDialog`: `var dayDialog ... private set` already generates a JVM
     * `setDayDialog(DayDialog)`, and a second function with the same erased signature is a platform
     * declaration clash that will not compile.
     */
    fun updateDayDialog(dialog: DayDialog) {
        dayDialog = dialog
        persistDraft()
    }

    /**
     * Snapshot the whole draft into the handle. Only once the initial load has landed: a draft
     * persisted over the empty pre-load list would restore as an empty, "loaded" plan.
     */
    private fun persistDraft() {
        if (!loadComplete) return
        savedStateHandle[KEY_DRAFT] = ProgramBuilderDraft(days, dirty, openDayUid, dayDialog).toJson()
    }

    private fun clearDraft() {
        savedStateHandle.remove<String>(KEY_DRAFT)
    }

    private suspend fun loadDays(): List<BuilderDay> =
        store.currentDayRows().map { pd ->
            val slots = store.slotRowsForDay(pd.id)
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
        persistDraft()
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
                store.guardProgramChange {
                    store.saveCustomProgram(dayRows, slotRows)
                    store.setFreestyleMode(false)
                    dirty = false
                    // The saved program IS the document now; a recreation reloads it from Room.
                    clearDraft()
                    onSaved()
                }
            } finally {
                saving = false
            }
        }
    }

    companion object {
        /** SavedStateHandle key for the JSON draft — see [ProgramBuilderDraft]. */
        internal const val KEY_DRAFT = "programBuilderDraft"
    }
}
