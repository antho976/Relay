package com.forge.app.ui.onboarding

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The onboarding draft's lifecycle contract (H-09): a screen recreated by a configuration change
 * must find every answer in the ViewModel, whether or not the debounced DataStore write has landed,
 * and the write must never put a blank default snapshot over a good draft on disk.
 *
 * The screen's twenty `remember` fields die with the Activity; the ViewModel is what survives. It
 * used to expose only its one-shot disk read, so a rotation mid-setup rehydrated from `Ready(null)`
 * — page one, defaults — and the autosaver then wrote that blank snapshot over the real draft.
 * [OnboardingDraftKeeper] is the piece of the ViewModel that owns this, split out so the contract
 * can be pinned without Hilt, Room or DataStore. "Disk" here is one nullable variable.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class OnboardingDraftKeeperTest {

    /** Mirrors [OnboardingDraftKeeper]'s own (private) debounce. */
    private val debounceMs = 250L

    /** The screen's own defaults on a fresh install — what its first snapshot looks like. */
    private fun defaults(seed: Long = 7L) = OnboardingDraft(
        step = 0, planMode = "", name = "", useKg = false, useMilesChoice = false,
        distanceTouched = false, goal = "", experience = "", bodyweightInput = "", sex = null,
        daysPerWeek = 0, equipment = emptySet(), frozenIds = null, plateWeightLb = 15.0,
        problemAreas = emptySet(), cadence = "", everyN = 4, previewSeed = seed, appLock = false,
        coachChoice = null
    )

    /** Several pages in: mode, goal, experience and days answered, a curated preset locked. */
    private fun midway() = defaults(seed = 42L).copy(
        step = 4, planMode = PLAN_GENERATED, goal = "build_muscle", experience = "intermediate",
        daysPerWeek = 4, equipment = setOf("BARBELL", "DUMBBELL"), frozenIds = setOf("bench", "squat")
    )

    private class Disk(var draft: OnboardingDraft?) {
        var writes = 0
    }

    private fun TestScope.keeperOver(disk: Disk) = OnboardingDraftKeeper(
        scope = backgroundScope,
        load = { disk.draft },
        write = { disk.draft = it; disk.writes++ }
    ).also { runCurrent() }   // let the one-shot load land, as the screen waits for it

    /**
     * Run the debounced autosave, if one is due.
     *
     * NOT `advanceUntilIdle()`, which was the bug in two of these. The keeper's writer lives in
     * `backgroundScope`, and `advanceUntilIdle` does not advance virtual time on behalf of
     * background work — it decides the scheduler is idle while a `delay(250)` sits in it, and
     * returns having run nothing. Both tests then read zero writes and asserted the ViewModel had
     * failed to persist, when nothing had been given the chance to.
     *
     * Advancing PAST the debounce and then running what that released is the honest form, and it is
     * equally correct for the tests that assert a write must NOT happen: they now prove the writer
     * declined, rather than that it never woke up.
     */
    private fun TestScope.settleDebounce() {
        advanceTimeBy(debounceMs + 50)
        runCurrent()
    }

    /** What a freshly composed screen reads: the draft its `remember` initialisers hydrate from. */
    private fun OnboardingDraftKeeper.freshScreenReads(): OnboardingDraft? =
        (state.value as DraftLoad.Ready).draft

    @Test
    fun `edits are readable by a recreated screen before the debounce lands`() = runTest {
        val disk = Disk(midway())
        val keeper = keeperOver(disk)
        assertEquals(midway(), keeper.freshScreenReads())

        // Answer the gear page and type two characters of a name, then rotate immediately.
        val edited = midway().copy(step = 5, equipment = setOf("BARBELL"), frozenIds = null, name = "Al")
        keeper.update(midway().copy(step = 5, equipment = setOf("BARBELL"), frozenIds = null, name = "A"))
        keeper.update(edited)
        advanceTimeBy(100)

        assertEquals("the ViewModel is the truth, synchronously", edited, keeper.freshScreenReads())
        assertEquals("the disk still holds the previous good draft, not a blank one", midway(), disk.draft)
        assertEquals(0, disk.writes)

        // The recreated screen's first snapshot equals what it hydrated from: nothing to write.
        keeper.update(edited)
        settleDebounce()
        assertEquals("one conflated write, the latest edit", 1, disk.writes)
        assertEquals(edited, disk.draft)
    }

    @Test
    fun `edits are readable by a recreated screen after the debounce landed`() = runTest {
        val disk = Disk(midway())
        val keeper = keeperOver(disk)

        val edited = midway().copy(step = 5, problemAreas = setOf("knees"))
        keeper.update(edited)
        advanceTimeBy(300)
        assertEquals(edited, disk.draft)

        // Rotate: the fresh screen hydrates from the ViewModel and its first snapshot is identical.
        assertEquals(edited, keeper.freshScreenReads())
        keeper.update(edited)
        settleDebounce()
        assertEquals("no second write for an unchanged snapshot", 1, disk.writes)
        assertEquals(edited, disk.draft)
    }

    @Test
    fun `a fresh install's default snapshot is adopted but never written`() = runTest {
        val disk = Disk(null)
        val keeper = keeperOver(disk)
        assertNull(keeper.freshScreenReads())

        // The screen composes with its own defaults (including a freshly rolled preview seed).
        keeper.update(defaults(seed = 99L))
        settleDebounce()
        assertNull("defaults carry nothing the user said", disk.draft)
        assertEquals(0, disk.writes)

        // But they ARE the baseline now: a rotation keeps the same seed and step...
        assertEquals(defaults(seed = 99L), keeper.freshScreenReads())
        // ...and the first real answer diffs against it and is persisted.
        val picked = defaults(seed = 99L).copy(planMode = PLAN_GENERATED)
        keeper.update(picked)
        settleDebounce()
        assertEquals(picked, disk.draft)
        assertEquals(1, disk.writes)
    }

    @Test
    fun `a failed disk read cannot be papered over by defaults`() = runTest {
        // The read degraded to "nothing there" (SettingsRepository swallows IOException), but the
        // real draft is still on disk. The only safe move is to write nothing until the user acts.
        val real = midway()
        var readFails = true
        val disk = Disk(real)
        val keeper = OnboardingDraftKeeper(
            scope = backgroundScope,
            load = { if (readFails) null else disk.draft },
            write = { disk.draft = it; disk.writes++ }
        )
        runCurrent()
        readFails = false

        keeper.update(defaults())
        settleDebounce()
        assertSame("the good draft is untouched", real, disk.draft)
        assertEquals(0, disk.writes)
    }

    @Test
    fun `nothing is recorded before the load lands`() = runTest {
        val disk = Disk(midway())
        val keeper = OnboardingDraftKeeper(
            scope = backgroundScope,
            load = { disk.draft },
            write = { disk.draft = it; disk.writes++ }
        )
        // The screen never composes before Ready, but be explicit: an early snapshot can't clobber
        // the draft the load is about to hand back.
        assertTrue(keeper.state.value is DraftLoad.Loading)
        keeper.update(defaults())
        runCurrent()
        assertEquals(midway(), keeper.freshScreenReads())
        settleDebounce()
        assertEquals(0, disk.writes)
    }

    @Test
    fun `completion stops the autosaver so it cannot resurrect the removed draft`() = runTest {
        val disk = Disk(midway())
        val keeper = keeperOver(disk)

        keeper.update(midway().copy(name = "Alex"))
        keeper.stopWrites()
        // Completion's atomic write removes the draft; the pending debounced save must not bring it back.
        disk.draft = null
        settleDebounce()
        assertNull(disk.draft)
        assertEquals(0, disk.writes)
    }
}
