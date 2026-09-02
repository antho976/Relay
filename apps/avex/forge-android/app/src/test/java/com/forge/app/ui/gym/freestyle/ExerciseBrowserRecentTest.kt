package com.forge.app.ui.gym.freestyle

import com.forge.app.program.ExerciseLibrary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The browser has two rails that can add a move: the grid and "Recently performed". The grid hides
 * what is already on the log; Recent did not, so picking the same move from each appended a second
 * row with the same `libId` — the key of the logger's lazy list — and the logger crashed on measure.
 */
@RunWith(RobolectricTestRunner::class)
class ExerciseBrowserRecentTest {

    private val recent = ExerciseLibrary.all.filterNot { it.curatedOnly }.take(4)

    @Test
    fun `recent hides what is already on the log, like the grid does`() {
        val onLog = setOf(recent[0].id, recent[2].id)
        assertEquals(listOf(recent[1], recent[3]), recentForBrowser(recent, onLog))
    }

    @Test
    fun `nothing excluded leaves recent as it was`() {
        assertEquals(recent, recentForBrowser(recent, emptySet()))
    }

    @Test
    fun `the grid and recent agree on what is offered`() {
        val onLog = setOf(recent[0].id)
        val grid = browseLibrary("", null, false, emptySet(), onLog).map { it.id }.toSet()
        assertTrue(recent[0].id !in grid)
        assertTrue(recentForBrowser(recent, onLog).all { it.id in grid })
    }
}
