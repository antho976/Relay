package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Refresh-ties-into-coach: what regeneration learns from the decision record. */
class CoachGenBiasTest {

    private var nextId = 1L

    private fun decision(
        type: String,
        targetKey: String,
        status: String = "applied",
        outcome: String = "pending",
        payload: String? = null
    ) = CoachDecision(
        id = nextId++, weekId = "2026-W01", type = type, targetKey = targetKey,
        targetName = targetKey, summary = "s", reason = "r", status = status,
        dayKey = "upper-a", payload = payload, appliedAt = nextId, outcome = outcome
    )

    @Test
    fun emptyHistory_isExactlyNeutral() {
        assertEquals(GenBias.NEUTRAL, CoachGenBias.from(emptyList()))
    }

    @Test
    fun appliedVolumeChanges_netPerMuscle() {
        // CHEST: +1 +1 −1 = +1; QUADS: +1.
        val bias = CoachGenBias.from(
            listOf(
                decision("volume_up", "db-bench-press"),
                decision("volume_up", "incline-db-bench-press"),
                decision("volume_down", "db-fly"),
                decision("volume_up", "goblet-squat")
            )
        )
        assertEquals(mapOf(MuscleGroup.CHEST to 1, MuscleGroup.QUADS to 1), bias.volumeBias)
    }

    @Test
    fun revertedAndSkippedVolumeChanges_dropOut() {
        val bias = CoachGenBias.from(
            listOf(
                decision("volume_up", "db-bench-press", status = "reverted", outcome = "failed"),
                decision("volume_up", "goblet-squat", status = "skipped")
            )
        )
        assertTrue(bias.volumeBias.isEmpty())
    }

    @Test
    fun volumeBiasIsClampedToTheDriftCap() {
        val bias = CoachGenBias.from(
            (1..4).map { decision("volume_up", "db-bench-press") }
        )
        assertEquals(CoachGenBias.VOLUME_CLAMP, bias.volumeBias[MuscleGroup.CHEST])
    }

    @Test
    fun provenSwapsArePreferred_failedOnesAvoided() {
        val bias = CoachGenBias.from(
            listOf(
                decision("swap", "db-row", outcome = "ok", payload = "mwm-standing-bicep-curl"),
                decision("swap", "db-curl", outcome = "failed", payload = "db-concentration-curl")
            )
        )
        assertEquals(setOf("mwm-standing-bicep-curl"), bias.prefer)
        assertEquals(setOf("db-concentration-curl"), bias.avoid)
    }

    @Test
    fun aLaterSuccessOutranksAnEarlierFailure() {
        // The same replacement failed once, then proved itself — prefer wins.
        val bias = CoachGenBias.from(
            listOf(
                decision("swap", "db-row", status = "reverted", outcome = "failed", payload = "mwm-standing-bicep-curl"),
                decision("swap", "db-row", outcome = "ok", payload = "mwm-standing-bicep-curl")
            )
        )
        assertEquals(setOf("mwm-standing-bicep-curl"), bias.prefer)
        assertTrue(bias.avoid.isEmpty())
    }

    @Test
    fun fromIsDeterministic() {
        val rows = listOf(
            decision("volume_up", "db-bench-press"),
            decision("swap", "db-row", outcome = "ok", payload = "mwm-standing-bicep-curl")
        )
        assertEquals(CoachGenBias.from(rows), CoachGenBias.from(rows))
    }

    // ─── Seam fix: folded = baked into the baseline, still feeds the bias ──────

    @Test
    fun foldedVolumeStillCountsTowardBias() {
        // A regenerate baked this +1 into the baseline (status=folded); the bias must keep carrying
        // it, otherwise the next regenerate would silently forget it (finding 0/1).
        val bias = CoachGenBias.from(listOf(decision("volume_up", "db-bench-press", status = "folded")))
        assertEquals(mapOf(MuscleGroup.CHEST to 1), bias.volumeBias)
    }

    @Test
    fun biasIsAFixedPoint_whenAnAppliedRowBecomesFolded() {
        // Folding an applied delta must NOT change what the bias produces — the property that makes
        // repeated regeneration idempotent rather than compounding (finding 2/16).
        val applied = CoachGenBias.from(listOf(decision("volume_up", "db-bench-press", status = "applied")))
        val folded = CoachGenBias.from(listOf(decision("volume_up", "db-bench-press", status = "folded")))
        assertEquals(applied.volumeBias, folded.volumeBias)
    }

    @Test
    fun failedVolumeIsExcluded_evenWhileStillApplied() {
        // A fatigue-failed +1 must drop out of the bias immediately (it's owed a revert), not keep
        // re-folding into every future baseline including the deload (finding 4).
        val bias = CoachGenBias.from(
            listOf(decision("volume_up", "db-bench-press", status = "applied", outcome = "failed"))
        )
        assertTrue(bias.volumeBias.isEmpty())
    }

    @Test
    fun repShiftSurvivesViaRepBias_latestNonFailedWins() {
        val bias = CoachGenBias.from(
            listOf(
                decision("rep_shift", "db-bench-press", status = "applied", payload = "6-8"),
                decision("rep_shift", "db-bench-press", status = "folded", payload = "5-7"),
                decision("rep_shift", "db-row", status = "applied", outcome = "failed", payload = "12-15")
            )
        )
        // Latest non-failed wins for bench; the failed db-row shift is excluded entirely.
        assertEquals(mapOf("db-bench-press" to "5-7"), bias.repBias)
    }

    @Test
    fun foldedSwapIsPreferred_evenBeforeTheWatcherCouldJudgeIt() {
        // Folding stops the watcher, so a folded swap can never reach outcome=ok — it earns prefer
        // by having been accepted and absorbed, otherwise the rotation would be lost (finding 3).
        val bias = CoachGenBias.from(
            listOf(decision("swap", "db-row", status = "folded", outcome = "pending", payload = "mwm-standing-bicep-curl"))
        )
        assertEquals(setOf("mwm-standing-bicep-curl"), bias.prefer)
        assertTrue(bias.avoid.isEmpty())
    }
}
