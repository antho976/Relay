package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass

@Dao
interface CoachDao {

    /** IGNORE so a concurrent week-pass creation no-ops on the week_id PK instead of crashing (#14). */
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun insertPass(pass: CoachPass): Long

    @Insert
    suspend fun insertDecisions(decisions: List<CoachDecision>)

    /**
     * Create a week's pass and its decisions atomically (seam fix, finding 14). Returns true only if
     * THIS call inserted the pass — a racing caller whose insert was IGNORED gets false and must not
     * write the decisions again (that would double them). Process death inside the transaction rolls
     * the whole thing back, so a pass row never exists without its decisions.
     */
    @Transaction
    suspend fun insertPassWithDecisions(pass: CoachPass, decisions: List<CoachDecision>): Boolean {
        if (insertPass(pass) == -1L) return false
        if (decisions.isNotEmpty()) insertDecisions(decisions)
        return true
    }

    /** Idempotency check — a pass exists for this ISO week, so the trigger no-ops. */
    @Query("SELECT * FROM coach_pass WHERE week_id = :weekId")
    suspend fun pass(weekId: String): CoachPass?

    @Query("SELECT * FROM coach_pass ORDER BY ran_at DESC LIMIT 1")
    suspend fun latestPass(): CoachPass?

    @Query("SELECT * FROM coach_decision WHERE week_id = :weekId ORDER BY id")
    suspend fun decisionsFor(weekId: String): List<CoachDecision>

    /** Coach history, newest first (Settings → Coach page, Phase 4). */
    @Query("SELECT * FROM coach_pass ORDER BY ran_at DESC LIMIT :limit")
    suspend fun recentPasses(limit: Int = 26): List<CoachPass>

    // ─── Decision lifecycle (auto-coach Phase 3) ──────────────────────────────

    @Query("SELECT * FROM coach_decision WHERE id = :id")
    suspend fun decision(id: Long): CoachDecision?

    @Query("UPDATE coach_decision SET status = :status WHERE id = :id")
    suspend fun setStatus(id: Long, status: String)

    @Query("UPDATE coach_decision SET status = 'applied', applied_at = :appliedAt, undo_data = :undoData WHERE id = :id")
    suspend fun markApplied(id: Long, appliedAt: Long, undoData: String?)

    @Query("UPDATE coach_decision SET status = 'reverted', outcome = 'failed' WHERE id = :id")
    suspend fun markReverted(id: Long)

    @Query("UPDATE coach_decision SET outcome = :outcome WHERE id = :id")
    suspend fun setOutcome(id: Long, outcome: String)

    /**
     * Retire delta decisions whose overlay a regenerate just cleared: they're now baked into the
     * baseline (CoachGenBias folds them forward), so 'folded' is a terminal status that still feeds
     * the bias but is no longer per-change undoable (auto-coach seam fix, finding 0/1). FAILED rows
     * are deliberately left applied so their revert proposal can still fire (finding 4); deload/
     * revert rows have no overlay to fold.
     */
    @Query(
        "UPDATE coach_decision SET status = 'folded' " +
            "WHERE status = 'applied' AND outcome != 'failed' " +
            "AND type IN ('volume_up','volume_down','rep_shift','swap')"
    )
    suspend fun foldAllAppliedDeltas()

    /** Single-day variant (re-roll): only this day's per-(day,exercise) deltas were cleared. */
    @Query(
        "UPDATE coach_decision SET status = 'folded' " +
            "WHERE status = 'applied' AND outcome != 'failed' AND day_key = :dayKey " +
            "AND type IN ('volume_up','volume_down','rep_shift')"
    )
    suspend fun foldAppliedDeltasForDay(dayKey: String)

    /**
     * Changes the watcher still owes a verdict (hardening decision 5). Includes 'folded' rows: a
     * regenerate that folds a change before its outcome window closes must NOT shield it from
     * judgment. The watcher reads session history by target_key (never the overlay, which folding
     * clears), so it can still rule a folded change ok/failed — which is what lets earned trust gate
     * on validation rather than mere application (auto-coach seam audit 2026-06-15, finding P1).
     */
    @Query("SELECT * FROM coach_decision WHERE status IN ('applied','folded') AND outcome = 'pending'")
    suspend fun pendingOutcome(): List<CoachDecision>

    /**
     * Still-applied changes the watcher judged FAILED but whose revert hasn't run yet. The revert
     * proposal is re-derived from this every pass, so a proposal dropped by deload-supersession / the
     * per-pass cap / an error pass self-heals next week instead of being lost (seam fix, finding 4).
     */
    @Query("SELECT * FROM coach_decision WHERE status = 'applied' AND outcome = 'failed'")
    suspend fun appliedFailed(): List<CoachDecision>

    /**
     * Still-active (applied OR folded) decisions on the same target inserted AFTER [afterId] —
     * the per-slot LIFO guard for undo: undoing an older decision while a newer one still owns the
     * overlay would silently wipe the newer change (seam fix, finding 9).
     */
    @Query("SELECT * FROM coach_decision WHERE status IN ('applied','folded') AND target_key = :targetKey AND id > :afterId")
    suspend fun activeOnTargetAfter(targetKey: String, afterId: Long): List<CoachDecision>

    /** Every decision ever, oldest first — TrustLedger's input (auto-coach Phase 4). */
    @Query("SELECT * FROM coach_decision ORDER BY id")
    suspend fun allDecisions(): List<CoachDecision>

    @Query("DELETE FROM coach_pass")
    suspend fun deleteAllPasses()

    @Query("DELETE FROM coach_decision")
    suspend fun deleteAllDecisions()
}
