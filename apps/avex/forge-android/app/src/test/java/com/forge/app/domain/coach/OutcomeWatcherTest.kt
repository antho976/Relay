package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.ExerciseBout
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.SleepNight
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

    /** A non-skipped bout at a chosen working weight — for the rep_shift e1RM-trend checks. */
    private fun wbout(startDay: Int, weight: Double) =
        ExerciseBout(
            sessionStartedAt = startDay * day, effort = EffortRating.JUST_RIGHT, hitFullTarget = false,
            skipped = false, swappedName = null, sets = listOf(set(weight))
        )

    private fun sessions(): List<Session> = (0 until 8).map {
        Session(
            id = it + 1L, dayKey = "upper-a", startedAt = (34 + it * 3) * day,
            finishedAt = (34 + it * 3) * day + 3_600_000, totalVolumeLb = 1000.0
        )
    }

    private fun snapshot(
        history: Map<String, List<ExerciseBout>> = emptyMap(),
        cardio: List<CardioEntry> = emptyList(),
        health: HealthSnap = HealthSnap()
    ) = AdaptationSnapshot(
        nowMs = now, program = emptyList(), sessions = sessions(),
        exerciseHistory = history, cardio = cardio, prefs = PrefsSnap(), health = health
    )

    /** Six short nights (6h ≤ the 6.5h ceiling) inside the deload window → the sleep-debt driver (+2). */
    private fun shortNights() = (0 until 6).map { SleepNight(endedAtMs = now - (1 + it) * day, durationMin = 360) }

    /** Non-skipped bouts of the target slot on the given days — the exposure an "ok" now requires (M-08). */
    private fun trainedSince(vararg days: Int) = snapshot(mapOf("ua1" to days.map { bout(it) }))

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
    fun repShift_windowClosed_strengthClimbed_ok() {
        // Applied day 44 (window elapsed), no skips, and the lift's best e1RM climbed after the
        // change — the rep shift restarted progress, so it passes.
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "rep_shift", appliedAtDay = 44, undoData = "6-8")),
            snapshot(mapOf("ua1" to listOf(wbout(40, 45.0), wbout(48, 55.0), wbout(52, 60.0))))
        )
        assertEquals("ok", verdicts.single().outcome)
    }

    @Test
    fun repShift_windowClosed_strengthSlipped_fails() {
        // Window elapsed, no skips — but the best e1RM since the change sits below where it was
        // before. The shift backfired (judged on strength, not just attendance) → failed.
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "rep_shift", appliedAtDay = 44, undoData = "6-8")),
            snapshot(mapOf("ua1" to listOf(wbout(40, 60.0), wbout(48, 45.0), wbout(52, 45.0))))
        )
        assertEquals("failed", verdicts.single().outcome)
        assertTrue(verdicts.single().failReason!!.contains("1RM"))
    }

    @Test
    fun repShift_insideWindow_quietlyTrained_noVerdictYet() {
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "rep_shift", appliedAtDay = 50, undoData = "6-8")),
            snapshot(mapOf("ua1" to listOf(wbout(53, 50.0), wbout(57, 55.0))))
        )
        assertTrue(verdicts.isEmpty())
    }

    @Test
    fun volumeUp_failsWhenFatigueSpikes() {
        // Effort inflation (+2) + sleep debt (+2) + sore cardio (+1) = deload territory.
        val brutal = (0..8).map { bout(48 + it, effort = EffortRating.BRUTAL) }
        val cardio = listOf(CardioEntry(1, date = now - 2 * day, type = "rest", durationMin = 0, restReason = "sore"))
        val verdicts = OutcomeWatcher.evaluate(
            listOf(decision(type = "volume_up", undoData = "0")),
            snapshot(mapOf("ua1" to brutal), cardio = cardio, health = HealthSnap(sleepNights = shortNights()))
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

    // ── Three-valued verdicts (B1) ─────────────────────────────────────────────

    @Test
    fun aWindowSpentAwayClosesAsNotFollowed() {
        // Applied, then a three-week break. The change was never actually run, so it neither
        // worked nor failed — judging it either way would teach the coach from an absence.
        val life = LifeEvents.State.NONE.copy(
            layoff = LifeEvents.Layoff(
                days = 21, away = false, returning = true,
                returnedAtMs = now - day, gapStartMs = now - 22 * day
            )
        )
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 30)), trainedSince(32, 35), life = life
        ).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
        assertTrue(verdict.failReason!!.contains("away or unwell"))
    }

    @Test
    fun illnessInsideTheWindowMakesItUnjudgeable() {
        // Applied on day 40, so the window is days 40-54. A sick day at 45 sits inside it: those
        // sessions were lived unwell, and judging the change on them would teach the coach from the
        // illness rather than from the change.
        val life = LifeEvents.State.NONE.copy(sick = true, sickAtMs = listOf(45 * day))
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 40)), trainedSince(44, 48), life = life
        ).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
    }

    @Test
    fun illnessAfterTheWindowClosedStillGetsARealVerdict() {
        // The regression this scoping exists for: `sick` is a CURRENT flag (a sick check-in in the
        // last three days), and reading it as though it covered a fortnight already lived meant a
        // user who trained the whole window and then caught a cold had every decision in it written
        // to the durable outcome column as "not_followed". TrustLedger reads that column, so being
        // ill on the wrong day cost the coach every unit of trust the fortnight had earned.
        val life = LifeEvents.State.NONE.copy(sick = true, sickAtMs = listOf(59 * day))
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 40)), trainedSince(44, 48), life = life
        ).single()
        assertEquals("ok", verdict.outcome)
    }

    @Test
    fun theSickWindowReachesExactlyThreeDaysAndNoFurther() {
        // The BOUNDARY, not a value near it. Mutation testing showed that widening
        // SICK_WINDOW_DAYS from 3 to 4, and flipping the overlap comparison from >= to >, both left
        // the suite green — the existing cases sat two days inside and five days outside the edge,
        // so nothing pinned where the edge actually is. A window that silently grows suppresses
        // verdicts that should have counted, and TrustLedger reads those verdicts.
        fun verdictWithSickDay(dayIndex: Int) = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 40)),
            trainedSince(44, 48),
            life = LifeEvents.State.NONE.copy(sick = true, sickAtMs = listOf(dayIndex * day))
        ).single().outcome

        // Applied day 40. A sick day carries LifeEvents.SICK_WINDOW_DAYS forward, so day 37 is the
        // last one that still reaches the window's first moment.
        assertEquals(
            "3 days before the window start is the last day that still reaches it",
            CoachDecision.OUTCOME_NOT_FOLLOWED, verdictWithSickDay(37)
        )
        assertEquals(
            "4 days before must NOT reach it",
            "ok", verdictWithSickDay(36)
        )
        // ...and at the far end, the window closes on day 54 (40 + WINDOW_DAYS).
        assertEquals(
            "a sick day on the window's last day still suppresses",
            CoachDecision.OUTCOME_NOT_FOLLOWED, verdictWithSickDay(54)
        )
        assertEquals(
            "the day after the window closes does not",
            "ok", verdictWithSickDay(55)
        )
    }

    @Test
    fun illnessJustBeforeTheWindowStillReachesIntoIt() {
        // A sick day carries SICK_WINDOW_DAYS forward, so falling ill two days before applying a
        // change still covers the start of its window — you don't recover the moment you tap Apply.
        val life = LifeEvents.State.NONE.copy(sick = true, sickAtMs = listOf(38 * day))
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 40)), trainedSince(44, 48), life = life
        ).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
    }

    @Test
    fun anOrdinaryLifeStillGetsARealVerdict() {
        val verdict = OutcomeWatcher.evaluate(listOf(decision(appliedAtDay = 40)), trainedSince(44, 48)).single()
        assertEquals("ok", verdict.outcome)
    }

    // ── No exposure is not a success (audit M-08) ──────────────────────────────

    @Test
    fun swap_windowClosed_neverTrained_isNotFollowedNotOk() {
        // Applied day 44, window closed, and the slot was never performed. This used to fall through
        // to "ok" — and TrustLedger reads that column, so a swap nobody ever did counted toward
        // auto-apply. An empty window is evidence of nothing: it closes neutral.
        val verdict = OutcomeWatcher.evaluate(listOf(decision(appliedAtDay = 44)), snapshot()).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
        assertTrue(verdict.failReason!!.contains("wasn't trained"))
    }

    @Test
    fun swap_windowClosed_onlySkippedOnce_isNotFollowed() {
        // One skip is below the avoidance threshold (two fails it) — but it is still zero performed bouts.
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(appliedAtDay = 44)),
            snapshot(mapOf("ua1" to listOf(bout(50, skipped = true))))
        ).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
    }

    @Test
    fun swap_insideWindow_neverTrained_isStillPending() {
        // The neutral close only lands once the window shuts — a live window stays undecided.
        assertTrue(OutcomeWatcher.evaluate(listOf(decision(appliedAtDay = 50)), snapshot()).isEmpty())
    }

    @Test
    fun repShift_windowClosed_noWorkingSetsSinceTheChange_isNotFollowed() {
        // Plenty of history BEFORE the change, none after: there is no post-change e1RM to judge the
        // shift on, so it closes neutral rather than as a win.
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(type = "rep_shift", appliedAtDay = 44, undoData = "6-8")),
            snapshot(mapOf("ua1" to listOf(wbout(36, 50.0), wbout(40, 52.0))))
        ).single()
        assertEquals(CoachDecision.OUTCOME_NOT_FOLLOWED, verdict.outcome)
        assertTrue(verdict.failReason!!.contains("no logged working sets"))
    }

    @Test
    fun repShift_windowClosed_trainedWithNoPriorBaseline_ok() {
        // Performed under the new range with nothing to compare against: the change was followed and
        // there is no regression evidence, so it passes on the strength of having been done.
        val verdict = OutcomeWatcher.evaluate(
            listOf(decision(type = "rep_shift", appliedAtDay = 44, undoData = "6-8")),
            snapshot(mapOf("ua1" to listOf(wbout(48, 50.0), wbout(52, 52.0))))
        ).single()
        assertEquals("ok", verdict.outcome)
    }

    @Test
    fun aLiveWindowIsNeverPrejudged() {
        // Suppression only applies once the window closes — an in-flight change stays pending.
        val life = LifeEvents.State.NONE.copy(sick = true, sickAtMs = listOf(59 * day))
        assertTrue(
            OutcomeWatcher.evaluate(listOf(decision(appliedAtDay = 58)), snapshot(), life = life).isEmpty()
        )
    }

    @Test
    fun notFollowedNeitherEarnsNorBreaksTrust() {
        // Two clean applies with an unjudgeable one between them: the streak reads 3, because the
        // absent window is invisible rather than counted either way.
        fun applied(id: Long, outcome: String) = decision(id = id, type = "rep_shift").copy(outcome = outcome)
        val history = listOf(
            applied(1, "ok"),
            applied(2, CoachDecision.OUTCOME_NOT_FOLLOWED),
            applied(3, "ok"),
            applied(4, "ok")
        )
        val trust = TrustLedger.assess(history).first { it.type == "rep_shift" }
        assertEquals(3, trust.streak)
    }
}
