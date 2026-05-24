package com.forge.app.data.repo

import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.entities.DayNameOverride
import com.forge.app.data.db.entities.ExerciseCustomization
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Persistent user customisations applied on top of the static program — currently
 * swap overrides (per-exercise) and day-name overrides (per day key). Both are
 * upsert-or-clear; absence of a row means "use the default from `program/`".
 */
@Singleton
class CustomizationRepository @Inject constructor(
    private val customizationDao: ExerciseCustomizationDao,
    private val dayNameDao: DayNameOverrideDao
) {

    // ─── Exercise swap overrides ───────────────────────────────────────────────

    fun observeAllSwaps(): Flow<List<ExerciseCustomization>> = customizationDao.observeAll()

    suspend fun getSwap(exerciseId: String): ExerciseCustomization? =
        customizationDao.get(exerciseId)

    suspend fun setSwap(exerciseId: String, swappedName: String, swappedUnit: String) {
        val existing = customizationDao.get(exerciseId)
        customizationDao.upsert(
            ExerciseCustomization(exerciseId, swappedName, swappedUnit,
                restTimerOverrideSeconds = existing?.restTimerOverrideSeconds)
        )
    }

    suspend fun setRestTimerOverride(exerciseId: String, seconds: Int?) {
        val existing = customizationDao.get(exerciseId)
        if (existing != null) {
            customizationDao.upsert(existing.copy(restTimerOverrideSeconds = seconds))
        } else {
            // No swap exists yet — create a minimal row to hold the override
            customizationDao.upsert(ExerciseCustomization(exerciseId, "", "", seconds))
        }
    }

    suspend fun getRestTimerOverride(exerciseId: String): Int? =
        customizationDao.get(exerciseId)?.restTimerOverrideSeconds

    suspend fun setPinnedNote(exerciseId: String, note: String) {
        val existing = customizationDao.get(exerciseId)
        if (existing != null) {
            customizationDao.upsert(existing.copy(pinnedNote = note))
        } else {
            customizationDao.upsert(ExerciseCustomization(exerciseId, "", "", null, note))
        }
    }

    suspend fun clearSwap(exerciseId: String) = customizationDao.clear(exerciseId)

    // ─── Day name overrides ────────────────────────────────────────────────────

    fun observeAllDayNames(): Flow<List<DayNameOverride>> = dayNameDao.observeAll()

    suspend fun getDayName(dayKey: String): DayNameOverride? = dayNameDao.get(dayKey)

    suspend fun setDayName(dayKey: String, customName: String) {
        dayNameDao.upsert(DayNameOverride(dayKey, customName))
    }

    suspend fun clearDayName(dayKey: String) = dayNameDao.clear(dayKey)
}
