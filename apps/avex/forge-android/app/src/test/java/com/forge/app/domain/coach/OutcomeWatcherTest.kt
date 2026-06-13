package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.PrefsSnap
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Hardening decision 5 — the watcher's verdicts on applied coach changes. */
class OutcomeWatcherTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 60 * day

    private fun decision(
        id: Long = 1,
        type: String = "swap",
        appliedAtDay: Int = 50,
        undoData: String? = "∅"
    ) = CoachDecision(
        id = id, weekId = "2026-W01", type = type, targetKey = "ua1", targetName = "Lift ua1",
        summary = "Rotate Lift ua1 → alt-1", reason = "stalled", status = "applied",
        dayKey = "upper-a", payload = "alt-1", appliedAt = appliedAtDay * day,
        outcome = "pending", undoData = undoData
    )

    private fun set(weight: Double) = LoggedSet(
        loggedExerciseId = 1, setIndex = 0, weightText = "$weight",
        weightLb = weight, reps = 8, completedAt = 0
    )

    private fun bout(startDay: Int, skipped: Boolean = false, effort: EffortRating? = EffortRating.JUST_RIGHT) =
        ExerciseBout(
            sessionStartedAt = startDay * day, effort = effort, hitFullTarget = false,
            skipped = skipped, swappedName = null, sets = listOf(set(45.0))
        )

    private fun sessions(): List<Session> = (0 until 8).map {
        Session(
            id = it + 1L, dayKey = "upper-a", startedAt = (34 + it * 3) * day,
            finishedAt = (34 + it * 3) * day + 3_600_000, totalVolumeLb = 1000.0
        )
    }

    private fun snapshot(
        history: Map<String, List<ExerciseBout>> = emptyMap(),
        moods: List<MoodEntry> = emptyList(),
        cardio: List<CardioEntry> = emptyList()
    ) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = sessions(),
        exerciseHistory = history, moods = moods, cardio = cardio, prefs = PrefsSnap()
    )

    @Test
    fun insideWindow_noSkips_noVerdictYet() {
        // Applied day 50, now day 60 — 10 of 14 days elapsed, trained twice without skipping.
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision()),
            snapshot(mapOf("ua1" to listOf(bout(53), bout(57))))
        )
        assertTrue(verdicts.isEmpty())
    }

    @Test
    fun skippedTwiceSinceApply_failsTheSwap() {
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision()),
            snapshot(mapOf("ua1" to listOf(bout(53, skipped = true), bout(57, skipped = true))))
        )
        assertEquals("failed", verdicts.single().outcome)
        assertTrue(verdicts.single().failReason!!.contains("skipped"))
    }

    @Test
    fun windowElapsed_quietlyTrained_ok() {
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 44)),
            snapshot(mapOf("ua1" to listOf(bout(48), bout(52))))
        )
        assertEquals("ok", verdicts.single().outcome)
    }

    @Test
    fun volumeUp_failsWhenFatigueSpikes() {
        // Effort inflation (+2) + low moods (+2) + sore cardio (+1) = deload territory.
        val brutal = (0..8).map { bout(48 + it, effort = EffortRating.BRUTAL) }
        val moods = listOf(
            MoodEntry(1, null, "upper-a", "drained", now - 1 * day),
            MoodEntry(2, null, "upper-a", "off", now - 3 * day),
            MoodEntry(3, null, "upper-a", "drained", now - 5 * day)
        )
        val cardio = listOf(CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"))
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "volume_up", undoData = "0")),
            snapshot(mapOf("ua1" to brutal), moods = moods, cardio = cardio)
        )
        assertEquals("failed", verdicts.single().outcome)
    }

    @Test
    fun conservativeTypes_passOnceWindowCloses() {
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "volume_down", appliedAtDay = 44, undoData = "3")),
            snapshot()
        )
        assertEquals("ok", verdicts.single().outcome)
    }

    @Test
    fun revertProposals_builtFromFailedVerdictsOnly() {
        val failedSwap = decision(id = 1)
        val okShift = decision(id = 2, type = "rep_shift", appliedAtDay = 44)
        val applied = listOf(failedSwap, okShift)
        val verdicts = listOf(
            WatchVerdict(1, "failed", "being skipped"),
            WatchVerdict(2, "ok")
        )
        val proposals = OutcomeWatcher.revertProposals(applied, verdicts)
        assertEquals(1, proposals.size)
        assertEquals("revert", proposals.single().type)
        assertEquals("1", proposals.single().payload)
        assertTrue(proposals.single().summary.startsWith("Revert:"))
    }

    @Test
    fun revertProposalsFor_selfHealsFromAlreadyFailedRows() {
        // The durable path: no verdicts in hand, just rows the outcome column already marked failed
        // (e.g. a prior pass's proposal was dropped by deload-supersession). A revert reappears.
        val failed = listOf(
            decision(id = 7, type = "volume_up", undoData = "3"),
            decision(id = 8, type = "swap", undoData = null) // swap is revertable even with null undo
        )
        val proposals = OutcomeWatcher.revertProposalsFor(failed)
        assertEquals(setOf("7", "8"), proposals.map { it.payload }.toSet())
        assertTrue(proposals.all { it.type == "revert" })
    }

    @Test
    fun revertProposalsFor_skipsChangesWithNoMechanicalUndo() {
        // A deload has no undo_data and isn't a swap — it can't be reverted mechanically, so no
        // revert proposal is ever fabricated for it.
        val proposals = OutcomeWatcher.revertProposalsFor(
            listOf(decision(id = 9, type = "deload", undoData = null))
        )
        assertTrue(proposals.isEmpty())
    }

    @Test
    fun revertProposalsFor_emitsNewestFirst() {
        // Two decisions chained on one slot both failed; Apply All applies reverts in order, so the
        // NEWER (higher id) must revert first to let the per-slot LIFO guard unwind it (finding 9).
        val older = decision(id = 3, type = "rep_shift", undoData = "6-8")
        val newer = decision(id = 9, type = "rep_shift", undoData = "5-7")
        val proposals = OutcomeWatcher.revertProposalsFor(listOf(older, newer))
        assertEquals(listOf("9", "3"), proposals.map { it.payload })
    }
}
