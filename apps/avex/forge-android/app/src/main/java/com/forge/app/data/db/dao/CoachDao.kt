package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass

@Dao
interface CoachDao {

    @Insert
    suspend fun insertPass(pass: CoachPass)

    @Insert
    suspend fun insertDecisions(decisions: List<CoachDecision>)

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

    /** Applied changes the watcher still owes a verdict (hardening decision 5). */
    @Query("SELECT * FROM coach_decision WHERE status = 'applied' AND outcome = 'pending'")
    suspend fun appliedPendingOutcome(): List<CoachDecision>

    /** Applied changes since [since] — drift caps + per-type trust math read these. */
    @Query("SELECT * FROM coach_decision WHERE status = 'applied' AND applied_at >= :since")
    suspend fun appliedSince(since: Long): List<CoachDecision>

    /** Every decision ever, oldest first — TrustLedger's input (auto-coach Phase 4). */
    @Query("SELECT * FROM coach_decision ORDER BY id")
    suspend fun allDecisions(): List<CoachDecision>

    @Query("DELETE FROM coach_pass")
    suspend fun deleteAllPasses()

    @Query("DELETE FROM coach_decision")
    suspend fun deleteAllDecisions()
}
