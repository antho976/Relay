package com.forge.app.data.db.dao

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.SuggestionOutcome
import com.forge.app.data.db.inMemoryForgeDb
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Undoing the first set of an exercise takes its suggestion outcome back out, by the row id the
 * insert returned, so a corrected re-log leaves exactly one calibration sample: the corrected one.
 */
@RunWith(RobolectricTestRunner::class)
class SuggestionOutcomeDaoTest {

    private val db: ForgeDatabase = inMemoryForgeDb()

    @After
    fun tearDown() = db.close()

    private fun outcome(takenLb: Double, loggedAt: Long) = SuggestionOutcome(
        exerciseId = "bench", unit = "plates", suggestedLb = 185.0,
        takenLb = takenLb, reps = 8, rangeText = "6-8", loggedAt = loggedAt
    )

    @Test
    fun deleteByIdRemovesOnlyTheUndoneSample() = runTest {
        val dao = db.suggestionOutcomeDao()
        val kept = dao.insert(outcome(takenLb = 180.0, loggedAt = 1_000L))
        val mistaken = dao.insert(outcome(takenLb = 18.0, loggedAt = 2_000L))

        dao.deleteById(mistaken)
        val corrected = dao.insert(outcome(takenLb = 185.0, loggedAt = 3_000L))

        assertEquals(listOf(corrected, kept), dao.recent().map { it.id })
        assertEquals(listOf(185.0, 180.0), dao.recent().map { it.takenLb })
    }
}
