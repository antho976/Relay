package com.forge.app.ui.gym.freestyle

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.WorkoutRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * One persisted set: the raw display text the user typed, its lb value (null = bodyweight), reps, and
 * its set-type tags (GYMAP-46). [setType] is the mutually-exclusive shape (null | "warmup" | "drop");
 * [isAmrap]/[toFailure] are independent flags; [rpe] is 1.0–10.0 in 0.5 steps or null.
 */
data class FreestyleSetInput(
    val weightText: String,
    val weightLb: Double?,
    val reps: Int,
    val setType: String? = null,
    val isAmrap: Boolean = false,
    val toFailure: Boolean = false,
    val rpe: Double? = null,
    /** Held time in seconds for a timed-hold set (GYMAP-51); null for a rep set (reps is then the count). */
    val durationSeconds: Int? = null
)

/**
 * One persisted exercise: a library id and its sets. [customName] is set only for a user-created
 * move, which has no library row to resolve a name from — it's stored on the logged row so every
 * display surface (history, PRs, stats, export) reads it through
 * [com.forge.app.program.Program.exerciseDisplayName] instead of humanizing the raw id.
 */
data class FreestyleExerciseInput(
    val libId: String,
    val sets: List<FreestyleSetInput>,
    val customName: String? = null
)

/**
 * Backs the dedicated freestyle ("go with the flow") logger — a log-after-the-fact workout with no
 * fixed plan. The screen owns the editable in-memory state; this VM only exposes the unit preference
 * and persists the finished workout into the normal session store (so it shows in history/stats and
 * its sets feed PR detection like any other workout).
 */
@HiltViewModel
class FreestyleLogViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    private val settingsRepo: SettingsRepository,
    private val customizationRepo: CustomizationRepository
) : ViewModel() {

    val weightUnit: StateFlow<com.forge.app.domain.units.WeightUnit> =
        settingsRepo.weightUnit.stateIn(
            viewModelScope, SharingStarted.WhileSubscribed(5_000), com.forge.app.domain.units.WeightUnit.LB
        )

    /** The most recent other performance's sets for an exercise — the "copy last time" panel. */
    suspend fun lastSets(exerciseId: String): List<com.forge.app.data.db.entities.LoggedSet> =
        workoutRepo.lastPerformanceSets(exerciseId)

    /**
     * The always-visible pinned cue (#112) the user attached to this exercise, or "" if none. Surfaced
     * read-only while freestyle-logging so a cue pinned on the exercise in the program shows here too
     * (GYMAP-49). Keyed by library id — the same key the Train flow writes under (getSwap(plan.id),
     * where plan.id == ExerciseDef.id). Caveat: a cue pinned while another exercise was swapped into a
     * program slot is stored under that slot's base id, so it surfaces for the slot's base move here,
     * not the swapped-in one — matches how the swap system already keys the shared row.
     */
    suspend fun pinnedNote(libId: String): String =
        customizationRepo.getSwap(libId)?.pinnedNote?.takeIf { it.isNotBlank() }.orEmpty()

    // ─── Draft persistence (autosave + resume) ───────────────────────────────

    /** Read + parse the saved in-progress draft, or null when there is none / it can't be parsed. */
    internal suspend fun loadDraft(): FreestyleDraft? =
        settingsRepo.freestyleDraft()?.let { FreestyleDraft.fromJson(it) }

    /** Autosave the current in-progress log (fire-and-forget; the screen debounces the calls). */
    internal fun saveDraft(draft: FreestyleDraft) {
        viewModelScope.launch { settingsRepo.saveFreestyleDraft(draft.toJson()) }
    }

    /** Drop the saved draft — used for "start fresh" and once the log is empty. */
    fun clearDraft() {
        viewModelScope.launch { settingsRepo.clearFreestyleDraft() }
    }

    /**
     * Persist the workout as a finished freestyle session, then invoke [onSaved] on the main thread.
     * [startedAtMs] is when the logger was opened — it becomes the session start so the recorded
     * duration reflects the real time spent logging instead of ~0.
     */
    fun save(items: List<FreestyleExerciseInput>, startedAtMs: Long, onSaved: () -> Unit) {
        viewModelScope.launch {
            val sessionId = workoutRepo.createFreestyleSession(startedAtMs)
            items.forEachIndexed { exIdx, ex ->
                val loggedExerciseId =
                    workoutRepo.addExerciseToSession(sessionId, ex.libId, exIdx, swappedName = ex.customName)
                ex.sets.forEachIndexed { setIdx, s ->
                    val setId = workoutRepo.logSet(loggedExerciseId, setIdx, s.weightText, s.weightLb, s.reps, s.durationSeconds)
                    // Persist the set-type tags via the existing per-field setters — only when set, so an
                    // untagged set writes exactly as before. Warm-ups still count toward volume/PRs (label
                    // only), matching the structured flow's set model (GYMAP-46).
                    if (s.setType != null) workoutRepo.setSetType(setId, s.setType)
                    if (s.isAmrap) workoutRepo.setAmrap(setId, true)
                    if (s.toFailure) workoutRepo.setToFailure(setId, true)
                    if (s.rpe != null) workoutRepo.setRpe(setId, s.rpe)
                }
                // Flag wasPr the same way the live day screen does, so a PR logged after the fact still
                // counts toward the lifetime PR total + the PRs list (not just the raw max-weight stats).
                workoutRepo.flagPrForLoggedExercise(loggedExerciseId, ex.libId)
            }
            workoutRepo.finishSession(sessionId)
            // The log is now a real finished session — drop its resume draft so it can't be re-offered.
            settingsRepo.clearFreestyleDraft()
            onSaved()
        }
    }
}
