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
}
