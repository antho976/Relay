package com.forge.app.ui.trophies

import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.Trophies
import com.forge.app.ui.trophies.state.TrophyFilter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * L-08: the trophies catalog must not claim "0 EARNED" on an account that has earned some.
 *
 * The unlock rows and the stats snapshot arrive separately, and the snapshot is the slow one
 * (roughly fourteen DAO reads). The old builder answered a pending snapshot by throwing the rows
 * away and returning an empty placeholder, which the screen then rendered as fact. The rows are
 * the truth about what is earned, so they are used either way; only what genuinely needs the
 * snapshot — a locked trophy's progress, and the closest-trophy nudge — waits for it.
 */
class TrophiesLoadingStateTest {

    private val earned = Trophies.all.take(2).associate { it.id to 1_700_000_000_000L }

    private fun snapshot() = TrophyStatsSnapshot(
        totalLoggedExercises = 40, totalPrs = 3, brutalRatings = 0, swapsUsed = 0,
        fullTargetHits = 0, finishedSessions = 12, distinctDayKeysTrained = 4,
        maxBenchLb = 185.0, maxSquatLb = 275.0, maxSessionVolumeLb = 12_000.0
    )

    private fun state(snapshot: TrophyStatsSnapshot?) = trophiesStateFor(
        unlockedByIdToDate = earned,
        nearMisses = emptyList(),
        snapshot = snapshot,
        filter = TrophyFilter.ALL,
        weightUnit = WeightUnit.LB
    )

    @Test
    fun aPendingSnapshotStillReportsWhatIsEarned() {
        val loading = state(snapshot = null)

        assertEquals("the unlock rows are already known", earned.size, loading.unlockedCount)
        assertEquals(Trophies.all.size, loading.totalCount)
        assertTrue("the catalog itself needs no snapshot", loading.sections.isNotEmpty())
        assertTrue("and the earned ones read as earned", loading.sections.flatMap { it.displays }.any { it.isUnlocked })
        assertTrue("so is the score", loading.cumulativeScore > 0)
    }

    @Test
    fun aPendingSnapshotClaimsNoProgressItCannotKnow() {
        val loading = state(snapshot = null)
        val locked = loading.sections.flatMap { it.displays }.filter { !it.isUnlocked }

        assertTrue(locked.isNotEmpty())
        assertTrue("no hint without the stats behind it", locked.all { it.progressHint == null })
        assertTrue("and no fraction either", locked.all { it.progressFraction == null })
        assertNull("nor a closest-trophy nudge", loading.closestTrophyNudge)
    }

    @Test
    fun theResolvedSnapshotFillsInTheProgressAndKeepsTheCounts() {
        val resolved = state(snapshot = snapshot())

        assertFalse(resolved.isLoading)
        assertEquals(earned.size, resolved.unlockedCount)
        val locked = resolved.sections.flatMap { it.displays }.filter { !it.isUnlocked }
        assertTrue("progress appears once the snapshot lands", locked.any { it.progressFraction != null })
        assertTrue("with real movement behind it", locked.any { (it.progressFraction ?: 0f) > 0f })
    }

    @Test
    fun anEmptyAccountIsStillReportedAsEmptyRatherThanUnknown() {
        val empty = trophiesStateFor(
            unlockedByIdToDate = emptyMap(),
            nearMisses = emptyList(),
            snapshot = null,
            filter = TrophyFilter.ALL,
            weightUnit = WeightUnit.LB
        )

        assertEquals(0, empty.unlockedCount)
        assertEquals(0, empty.cumulativeScore)
        assertFalse("a read that has happened is not loading, whatever it found", empty.isLoading)
    }
}
