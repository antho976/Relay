package com.forge.app.data.prefs

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * L-06: a pin whose goal is gone must not keep occupying one of Home's three slots.
 *
 * An orphan key is invisible on Home — it resolves to nothing and is skipped — which reads as
 * harmless. It was not: the cap counted it, so the next pin evicted a LIVE key to make room and
 * Home rendered two goals in three slots, having silently dropped the third.
 */
@RunWith(RobolectricTestRunner::class)
class GoalPinTest {

    private lateinit var repo: SettingsRepository

    @Before
    fun setUp() {
        val context: Context = ApplicationProvider.getApplicationContext()
        repo = SettingsRepository(context, Clock { 0L })
    }

    private suspend fun pins() = repo.pinnedGoals.first()

    @Test
    fun pinningKeepsOrderAndCapsAtThree() = runTest {
        listOf("a", "b", "c").forEach { repo.toggleGoalPin(it, max = 3) }
        assertEquals(listOf("a", "b", "c"), pins())

        repo.toggleGoalPin("d", max = 3)
        assertEquals("the oldest pin makes way", listOf("b", "c", "d"), pins())
    }

    @Test
    fun pinningTheSameKeyAgainUnpinsIt() = runTest {
        repo.toggleGoalPin("a", max = 3)
        repo.toggleGoalPin("a", max = 3)
        assertEquals(emptyList<String>(), pins())
    }

    @Test
    fun aDeletedGoalsPinDoesNotEvictALiveOne() = runTest {
        // The audit's reproduction: pin A, B and C, delete C, then pin D.
        listOf("a", "b", "c").forEach { repo.toggleGoalPin(it, max = 3) }

        assertEquals(2, repo.removeGoalPin("c"))
        repo.toggleGoalPin("d", max = 3)

        assertEquals("A must still be pinned", listOf("a", "b", "d"), pins())
    }

    @Test
    fun removingReportsWhereThePinWasAndAnUndoPutsItBackThere() = runTest {
        listOf("a", "b", "c").forEach { repo.toggleGoalPin(it, max = 3) }

        val index = repo.removeGoalPin("b")
        assertEquals(1, index)
        assertEquals(listOf("a", "c"), pins())

        repo.restoreGoalPin("b", index, max = 3)
        assertEquals("back where the user had it, not appended", listOf("a", "b", "c"), pins())
    }

    @Test
    fun removingAKeyThatWasNeverPinnedReportsNothingAndChangesNothing() = runTest {
        repo.toggleGoalPin("a", max = 3)

        assertEquals(-1, repo.removeGoalPin("zzz"))
        assertEquals(listOf("a"), pins())
    }

    @Test
    fun restoringAPinThatIsAlreadyBackIsANoOp() = runTest {
        repo.toggleGoalPin("a", max = 3)
        repo.restoreGoalPin("a", 0, max = 3)
        assertEquals(listOf("a"), pins())
    }
}
