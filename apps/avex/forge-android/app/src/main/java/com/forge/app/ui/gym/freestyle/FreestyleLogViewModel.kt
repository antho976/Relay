package com.forge.app.ui.gym.freestyle

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.WorkoutRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** One persisted set: the raw display text the user typed, its lb value (null = bodyweight), and reps. */
data class FreestyleSetInput(val weightText: String, val weightLb: Double?, val reps: Int)

/** One persisted exercise: a library id and its sets. */
data class FreestyleExerciseInput(val libId: String, val sets: List<FreestyleSetInput>)

/**
 * Backs the dedicated freestyle ("go with the flow") logger — a log-after-the-fact workout with no
 * fixed plan. The screen owns the editable in-memory state; this VM only exposes the unit preference
 * and persists the finished workout into the normal session store (so it shows in history/stats and
 * its sets feed PR detection like any other workout).
 */
@HiltViewModel
class FreestyleLogViewModel @Inject constructor(
    private val workoutRepo: WorkoutRepository,
    settingsRepo: SettingsRepository
) : ViewModel() {

    val useKg: StateFlow<Boolean> =
        settingsRepo.useKg.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)

    /** The most recent other performance's sets for an exercise — the "copy last time" panel. */
    suspend fun lastSets(exerciseId: String): List<com.forge.app.data.db.entities.LoggedSet> =
        workoutRepo.lastPerformanceSets(exerciseId)

    /**
     * Persist the workout as a finished freestyle session, then invoke [onSaved] on the main thread.
     * [startedAtMs] is when the logger was opened — it becomes the session start so the recorded
     * duration reflects the real time spent logging instead of ~0.
     */
    fun save(items: List<FreestyleExerciseInput>, startedAtMs: Long, onSaved: () -> Unit) {
        viewModelScope.launch {
            val sessionId = workoutRepo.createFreestyleSession(startedAtMs)
            var totalVolumeLb = 0.0
            var setCount = 0
            var prCount = 0
            items.forEachIndexed { exIdx, ex ->
                val loggedExerciseId = workoutRepo.addExerciseToSession(sessionId, ex.libId, exIdx)
                ex.sets.forEachIndexed { setIdx, s ->
                    workoutRepo.logSet(loggedExerciseId, setIdx, s.weightText, s.weightLb, s.reps)
                    totalVolumeLb += (s.weightLb ?: 0.0) * s.reps
                    setCount++
                }
                // Flag wasPr the same way the live day screen does, so a PR logged after the fact still
                // counts toward the lifetime PR total + the PRs list (not just the raw max-weight stats).
                if (workoutRepo.flagPrForLoggedExercise(loggedExerciseId, ex.libId)) prCount++
            }
            workoutRepo.finishSession(sessionId, totalVolumeLb, prCount = prCount, setCount = setCount)
            onSaved()
        }
    }
}
