package com.forge.app.data.db

import androidx.room.withTransaction
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session

/**
 * The two writes during a live session that must be indivisible, in one place.
 *
 * Both are read-then-write pairs — "does this slot have a row yet?" then insert; "what is the
 * highest set index?" then insert — and both were written as two separate statements. That is safe
 * only while there is one writer, and this app has two whenever a watch is paired: the wrist's
 * commands arrive at `WearSyncService` and are executed by `SetLogUseCase` **in this same process,
 * against this same Room instance**, by a coroutine that shares none of the day screen's locks. One
 * lifter double-tapping LOG SET produces the same interleaving without a watch at all.
 *
 * What the gap costs:
 *
 *  - **A forked slot.** Two `logged_exercise` rows for one program slot, each holding half the
 *    session's sets for that lift. The day screen resolves the slot to the first row, so the other
 *    half is invisible — and not lost either, which is worse: it still counts toward the session's
 *    volume and feeds progression, so the numbers disagree with the sets the user can see.
 *  - **A duplicate set index.** `ORDER BY set_index` then has no defined order between the two,
 *    the wrist's prefill resolves `maxByOrNull { setIndex }` against the tie, and undo removes one
 *    of two rows the user reads as a single set.
 *
 * Room serialises transactions on its single writer connection, so wrapping each pair closes the
 * gap against every writer in the process. These live here rather than inside the repository so the
 * DAO suites can drive the real code concurrently rather than a copy of it.
 */
internal object SessionWrites {

    /**
     * The active session, or [candidate] freshly inserted when there is none — as ONE transaction.
     *
     * Returns the row and whether THIS call inserted it. The app's invariant is at most one active
     * session, and the DAO's `LIMIT 1` reads it rather than enforcing it: two starts racing (a
     * double-tapped day, a wrist command landing as the phone opens the day) both read no active
     * session and both inserted one, and the loser's row became a live workout nobody could see
     * (M-07). Room serialises transactions on its single writer connection, so the second caller
     * runs its read after the first's insert has committed, and resumes it.
     */
    suspend fun startOrResume(db: ForgeDatabase, candidate: Session): Pair<Session, Boolean> =
        db.withTransaction {
            db.sessionDao().getActiveSession()?.let { active -> return@withTransaction active to false }
            val id = db.sessionDao().insert(candidate)
            candidate.copy(id = id) to true
        }

    /**
     * The `logged_exercise` row for [slotId] in [sessionId], creating it if it does not exist.
     *
     * [exerciseId] is the exercise actually performed — the swapped one when a swap is active — and
     * differs from [slotId] exactly then, which is when `slot_id` is stored.
     */
    suspend fun ensureSlotRow(
        db: ForgeDatabase,
        sessionId: Long,
        slotId: String,
        exerciseId: String,
        orderIndex: Int,
        swappedName: String? = null,
        swappedUnit: String? = null
    ): Long = db.withTransaction {
        db.loggedExerciseDao().forSessionSlot(sessionId, slotId)?.id
            ?: db.loggedExerciseDao().insert(
                LoggedExercise(
                    sessionId = sessionId,
                    exerciseId = exerciseId,
                    orderIndex = orderIndex,
                    swappedName = swappedName,
                    swappedUnit = swappedUnit,
                    // Keep the slot link only while the entry actually differs from its slot.
                    slotId = slotId.takeIf { it != exerciseId }
                )
            )
    }

    /**
     * Insert [set] at the next free index under its exercise, returning the new row id.
     *
     * The index comes from `MAX(set_index)`, not from a row count: deleting a set from the middle
     * leaves indices 0 and 2 with a count of 2, and a count would write a second index 2.
     */
    suspend fun insertSetWithNextIndex(db: ForgeDatabase, set: LoggedSet): Long = db.withTransaction {
        val next = (db.loggedSetDao().maxSetIndex(set.loggedExerciseId) ?: -1) + 1
        db.loggedSetDao().insert(set.copy(setIndex = next))
    }

    /**
     * Apply a session swap to a `logged_exercise` row, preserving every other column (superset
     * group, note, rating, etc.). Re-keys `exercise_id` to [swapExerciseId] so PRs/stats attribute to
     * the exercise actually performed (#11), stashing the original slot in `slot_id` so the day
     * screen still maps the row to its plan slot.
     *
     * The count-then-write is one transaction, and it is all or nothing: once any set exists under
     * the row, `exercise_id` records what was performed and must never change, and neither may the
     * name or unit. A relabel without a re-key would name one movement while every stat stayed keyed
     * to another, and later input would be read in the new unit under the old identity (H-11). The
     * window is real — the wrist writes sets straight into Room while the phone's swap sheet is open
     * — so the refusal is decided against the row's set count inside the same transaction that would
     * have written the swap, and the caller is told so it can drop its stale sheet.
     */
    suspend fun applySessionSwap(
        db: ForgeDatabase,
        loggedExerciseId: Long,
        swappedName: String?,
        swappedUnit: String?,
        swapExerciseId: String
    ): SessionSwapResult = db.withTransaction {
        val ex = db.loggedExerciseDao().get(loggedExerciseId)
            ?: return@withTransaction SessionSwapResult.NOT_FOUND
        if (db.loggedSetDao().countForLoggedExercise(loggedExerciseId) > 0) {
            return@withTransaction SessionSwapResult.REFUSED_SETS_LOGGED
        }
        val slot = ex.effectiveSlotId
        db.loggedExerciseDao().update(
            ex.copy(
                exerciseId = swapExerciseId,
                // Keep the slot link only while this entry actually differs from its slot — swapping
                // back to the original exercise clears it (slot == exercise again).
                slotId = slot.takeIf { it != swapExerciseId },
                swappedName = swappedName,
                swappedUnit = swappedUnit
            )
        )
        SessionSwapResult.APPLIED
    }
}

/**
 * Outcome of [SessionWrites.applySessionSwap]. Anything but [APPLIED] wrote nothing.
 *
 * Public, not internal, because it is the return type of the public
 * [com.forge.app.data.repo.WorkoutRepository.setSessionSwap].
 */
enum class SessionSwapResult {
    /** The row was re-keyed and relabelled. */
    APPLIED,

    /** A set already existed under the row (possibly one that landed while the sheet was open). */
    REFUSED_SETS_LOGGED,

    /** No such row — the session was discarded or the entry deleted underneath the caller. */
    NOT_FOUND
}
