package com.forge.app.data.db.dao

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.domain.cardio.CardioType
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Home's RECENT list reads `observeRecent(7)`. Rest days used to be dropped after the LIMIT, so a
 * week of logged rest days newer than a run filled all seven rows and the run never reached the
 * list. The exclusion has to happen before the limit.
 */
@RunWith(RobolectricTestRunner::class)
class CardioRecentRestExclusionTest {

    private val db: ForgeDatabase = inMemoryForgeDb()

    @After
    fun tearDown() = db.close()

    @Test
    fun restDaysDoNotCrowdRealCardioOutOfTheLimit() = runTest {
        val dao = db.cardioDao()
        dao.insert(CardioEntry(date = 1_000L, type = CardioType.RUN.code, durationMin = 30))
        for (i in 1..7) {
            dao.insert(CardioEntry(date = 1_000L + i * 100L, type = CardioType.REST.code, durationMin = 0))
        }

        val recent = dao.observeRecent(7).first()

        assertEquals(listOf(CardioType.RUN.code), recent.map { it.type })
    }
}
