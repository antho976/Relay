package com.forge.app.data.repo

import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.entities.DayNameOverride
import com.forge.app.data.db.entities.ExerciseCustomization
import com.forge.app.data.db.entities.OverlaySource
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

    suspend fun getSwap(exerciseId: String): ExerciseCustomization? =
        customizationDao.get(exerciseId)

    suspend fun setSwap(
        exerciseId: String,
        swappedName: String,
        swappedUnit: String,
        source: String = OverlaySource.USER
    ) {
        val existing = customizationDao.get(exerciseId)
        // copy() so untouched fields survive (restTimerOverrideSeconds AND pinnedNote — the old
        // positional rebuild dropped the pinned note). [source] tags the writer: a coach swap
        // (COACH) is cleared on regenerate and never coach-locks its slot; a user swap (USER) locks.
        customizationDao.upsert(
            existing?.copy(swappedName = swappedName, swappedUnit = swappedUnit, source = source)
                ?: ExerciseCustomization(exerciseId, swappedName, swappedUnit, source = source)
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

    suspend fun setPinnedNote(exerciseId: String, note: String) {
        val existing = customizationDao.get(exerciseId)
        if (existing != null) {
            customizationDao.upsert(existing.copy(pinnedNote = note))
        } else {
            customizationDao.upsert(ExerciseCustomization(exerciseId, "", "", null, note))
        }
    }

    /**
     * Revert the swap, preserving the rest-timer override and pinned note that share this row
     * (#59, #112). A blank `swappedName` already means "no swap" everywhere (see [setRestTimerOverride],
     * which legitimately creates blank-swap rows), so we only DELETE the row when nothing else is set.
     * The old unconditional DELETE silently wiped the rest timer and pinned note.
     */
    suspend fun clearSwap(exerciseId: String) {
        val existing = customizationDao.get(exerciseId) ?: return
        if (existing.restTimerOverrideSeconds == null && existing.pinnedNote.isBlank()) {
            customizationDao.clear(exerciseId)
        } else {
            customizationDao.upsert(existing.copy(swappedName = "", swappedUnit = ""))
        }
    }

    // ─── Day name overrides ────────────────────────────────────────────────────

    fun observeAllDayNames(): Flow<List<DayNameOverride>> = dayNameDao.observeAll()

    suspend fun getDayName(dayKey: String): DayNameOverride? = dayNameDao.get(dayKey)

    suspend fun setDayName(dayKey: String, customName: String) {
        dayNameDao.upsert(DayNameOverride(dayKey, customName))
    }

    suspend fun clearDayName(dayKey: String) = dayNameDao.clear(dayKey)
}
