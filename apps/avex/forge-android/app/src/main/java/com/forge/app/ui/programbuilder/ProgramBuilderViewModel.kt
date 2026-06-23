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
import dagger.hilt.android.lifecycle.HiltViewModel
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

    private var loaded = false

    private fun uid() = UUID.randomUUID().toString().take(8)

    /** Load once: blank for build-your-own, or the current program's rows for editing. */
    fun loadIfNeeded(blank: Boolean) {
        if (loaded) return
        loaded = true
        if (blank) { days = emptyList(); return }
        viewModelScope.launch {
            days = programRepository.currentDayRows().map { pd ->
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
        }
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

    fun removeDay(dayUid: String) = mutate { it.filterNot { d -> d.uid == dayUid } }
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

    fun removeExercise(dayUid: String, exUid: String) =
        mutateDay(dayUid) { it.copy(exercises = it.exercises.filterNot { e -> e.uid == exUid }) }

    fun setExercise(dayUid: String, exUid: String, sets: Int, reps: String) = mutateDay(dayUid) { day ->
        day.copy(exercises = day.exercises.map { if (it.uid == exUid) it.copy(sets = sets, reps = reps) else it })
    }

    fun moveExercise(dayUid: String, from: Int, to: Int) =
        mutateDay(dayUid) { it.copy(exercises = it.exercises.moved(from, to)) }

    fun day(dayUid: String): BuilderDay? = days.firstOrNull { it.uid == dayUid }

    /** Persist the built plan and make it live, then invoke [onSaved] on the main thread. */
    fun save(onSaved: () -> Unit) {
        if (saving) return
        saving = true
        viewModelScope.launch {
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
            saving = false
        }
    }
}
