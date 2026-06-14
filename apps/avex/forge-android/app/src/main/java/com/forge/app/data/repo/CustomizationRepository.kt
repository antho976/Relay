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
        source: String = OverlaySource.USER,
        /** The swapped exercise's real library id for PR/stats re-attribution (#11). Null = preserve the
         *  existing id (e.g. a unit-only edit that must not wipe the swap's real id). */
        swappedExerciseId: String? = null
    ) = upsertSwap(exerciseId, swappedName, swappedUnit, source) { existing ->
        // Preserve the existing re-attribution id when none is passed (e.g. a unit-only edit).
        swappedExerciseId ?: existing?.swappedExerciseId
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
     *
     * Also nulls [ExerciseCustomization.swappedExerciseId] (#11): a surviving row must not keep the
     * re-attribution id, or every PR/stats read would still key on the now-removed swap's exercise.
     */
    suspend fun clearSwap(exerciseId: String) {
        val existing = customizationDao.get(exerciseId) ?: return
        if (existing.restTimerOverrideSeconds == null && existing.pinnedNote.isBlank()) {
            customizationDao.clear(exerciseId)
        } else {
            customizationDao.upsert(existing.copy(swappedName = "", swappedUnit = "", swappedExerciseId = null))
        }
    }

    /**
     * Restore a swap to an EXACT prior state (coach undo, #11). Unlike [setSwap], which preserves the
     * existing [ExerciseCustomization.swappedExerciseId] when passed null, this force-sets it — so an
     * undo back to a swap that had no re-attribution id correctly clears the coach's id instead of
     * leaving it stranded. Rest-timer override and pinned note on the shared row are preserved.
     */
    suspend fun restoreSwap(
        exerciseId: String,
        swappedName: String,
        swappedUnit: String,
        source: String = OverlaySource.USER,
        swappedExerciseId: String?
    ) = upsertSwap(exerciseId, swappedName, swappedUnit, source) { swappedExerciseId }

    /**
     * Shared swap upsert. copy() so untouched fields survive (restTimerOverrideSeconds AND pinnedNote —
     * the old positional rebuild dropped the pinned note). [source] tags the writer: a coach swap
     * (COACH) is cleared on regenerate and never coach-locks its slot; a user swap (USER) locks.
     * [resolveId] decides the re-attribution id from the existing row (preserve vs force-set).
     */
    private suspend fun upsertSwap(
        exerciseId: String,
        swappedName: String,
        swappedUnit: String,
        source: String,
        resolveId: (ExerciseCustomization?) -> String?
    ) {
        val existing = customizationDao.get(exerciseId)
        customizationDao.upsert(
            existing?.copy(
                swappedName = swappedName, swappedUnit = swappedUnit, source = source,
                swappedExerciseId = resolveId(existing)
            ) ?: ExerciseCustomization(exerciseId, swappedName, swappedUnit, source = source, swappedExerciseId = resolveId(null))
        )
    }

    // ─── Day name overrides ────────────────────────────────────────────────────

    fun observeAllDayNames(): Flow<List<DayNameOverride>> = dayNameDao.observeAll()

    suspend fun getDayName(dayKey: String): DayNameOverride? = dayNameDao.get(dayKey)

    suspend fun setDayName(dayKey: String, customName: String) {
        dayNameDao.upsert(DayNameOverride(dayKey, customName))
    }

    suspend fun clearDayName(dayKey: String) = dayNameDao.clear(dayKey)
}
