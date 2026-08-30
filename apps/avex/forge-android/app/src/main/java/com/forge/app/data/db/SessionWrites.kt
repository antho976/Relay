package com.forge.app.data.db

import androidx.room.withTransaction
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet

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
}
