package com.forge.app.data.repo

import com.forge.app.core.time.TimeSignals
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.*
import javax.inject.Inject
import javax.inject.Singleton

/** React to engine inputs, excluding edits that only touch an unfinished workout. */
@Singleton
class EngineInputSignals @Inject constructor(
    private val db: ForgeDatabase,
    private val settings: SettingsRepository,
    private val program: ProgramRepository,
    private val time: TimeSignals
) {
    @OptIn(kotlinx.coroutines.FlowPreview::class)
    fun changes(): Flow<Unit> {
        val history = combine(
            db.sessionDao().observeAllFinishedSessions().distinctUntilChanged(),
            db.loggedExerciseDao().observeAllForFinishedSessions().distinctUntilChanged(),
            db.loggedSetDao().observeAllForFinishedSessions().distinctUntilChanged()
        ) { _, _, _ -> Unit }
        val preferences = combine(listOf<Flow<Any?>>(
            settings.availableEquipment, settings.likedExercises, settings.dislikedExercises,
            settings.pinnedExercises, settings.frozenExerciseIds, settings.plateWeightLb,
            settings.maxDbWeightLb, settings.deloadWeekStartMs
        )) { it.toList() }.distinctUntilChanged()
        val otherInputs = db.invalidationTracker.createFlow(
            "mood_entry", "cardio_entry", "bodyweight_entry", "checkin_entry",
            "injury_restriction", "vacation_period", "program_customization", "exercise_customization"
        )
        return combine(history, preferences, otherInputs, program.revision, time.dayStarts()) {
            _, _, _, _, _ -> Unit
        }.debounce(50).flowOn(Dispatchers.Default)
    }
}
