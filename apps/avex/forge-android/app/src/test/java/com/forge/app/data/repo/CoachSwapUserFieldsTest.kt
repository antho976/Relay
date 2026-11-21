package com.forge.app.data.repo

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.OverlaySource
import com.forge.app.data.db.inMemoryForgeDb
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Audit 2026-09-26 (08): the rest-timer override and pinned note share one row with the swap, and a
 * coach swap stamped that row coach-owned. The next regenerate deleted every coach row, taking the
 * user's note and rest time with it; a coach undo kept the row but left it coach-owned.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class CoachSwapUserFieldsTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private val dao = db.exerciseCustomizationDao()
    private val repo = CustomizationRepository(dao, db.dayNameOverrideDao())

    @After fun tearDown() = db.close()

    @Test
    fun aRegenerateKeepsTheUsersNoteAndRestUnderACoachSwap() = runTest {
        repo.setPinnedNote("bench", "seat notch 4")
        repo.setRestTimerOverride("bench", 150)
        repo.setSwap("bench", "Floor Press", "weight", source = OverlaySource.COACH, swappedExerciseId = "floor-press")
        repo.setSwap("row", "Seal Row", "weight", source = OverlaySource.COACH, swappedExerciseId = "seal-row")

        dao.clearCoachSwaps()

        val bench = dao.get("bench")!!
        assertEquals("seat notch 4", bench.pinnedNote)
        assertEquals(150, bench.restTimerOverrideSeconds)
        // The coach's swap itself is gone, and what is left belongs to the user again.
        assertEquals("", bench.swappedName)
        assertNull(bench.swappedExerciseId)
        assertEquals(OverlaySource.USER, bench.source)
        // A coach row with nothing of the user's on it is still dropped.
        assertNull(dao.get("row"))
    }

    @Test
    fun aCoachUndoHandsTheSurvivingRowBackToTheUser() = runTest {
        repo.setPinnedNote("bench", "seat notch 4")
        repo.setSwap("bench", "Floor Press", "weight", source = OverlaySource.COACH, swappedExerciseId = "floor-press")

        repo.clearSwap("bench")
        assertEquals(OverlaySource.USER, dao.get("bench")!!.source)

        dao.clearCoachSwaps()
        assertEquals("seat notch 4", dao.get("bench")!!.pinnedNote)
    }
}
