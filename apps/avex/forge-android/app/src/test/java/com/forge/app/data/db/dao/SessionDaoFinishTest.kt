package com.forge.app.data.db.dao

import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Finishing a session is a decision, and exactly one caller may make it.
 *
 * The old shape was read → check `finishedAt == null` → write a whole Session built from that read.
 * Two callers can both pass a check that is three statements away from the write it guards: a
 * double-tapped FINISH, a finish racing the orphan-recovery pass, a wrist command landing as the
 * phone finishes. Both then stamped the session, rotated the program, refreshed state and mirrored
 * the workout to Health Connect — one workout, two of everything downstream.
 */
@RunWith(RobolectricTestRunner::class)
class SessionDaoFinishTest {

    private val db: ForgeDatabase = inMemoryForgeDb()
    private val dao get() = db.sessionDao()

    @After
    fun tearDown() = db.close()

    @Test
    fun `the first finish wins and reports it`() = runTest {
        val id = dao.insert(session(finishedAt = null))
        assertEquals(1, dao.finishIfUnfinished(id, 5_000L))
        assertEquals(5_000L, dao.get(id)!!.finishedAt)
    }

    @Test
    fun `a second finish changes nothing and says so`() = runTest {
        val id = dao.insert(session(finishedAt = null))
        dao.finishIfUnfinished(id, 5_000L)

        assertEquals("the loser must be told it lost", 0, dao.finishIfUnfinished(id, 9_000L))
        assertEquals("and must not have re-stamped it", 5_000L, dao.get(id)!!.finishedAt)
    }

    @Test
    fun `a session that does not exist reports no winner`() = runTest {
        assertEquals(0, dao.finishIfUnfinished(4242L, 5_000L))
    }

    @Test
    fun `finishing does not touch the columns the finish sheet owns`() = runTest {
        // The full-row write carried every column from a snapshot taken BEFORE the finish, so
        // marking the session untracked from the finish sheet was reverted by the finish itself.
        val id = dao.insert(session(finishedAt = null))
        dao.setUntracked(id, true)

        dao.finishIfUnfinished(id, 5_000L)
        dao.setFinishTotals(id, totalVolumeLb = 1234.0, prCount = 2, setCount = 9, activeSeconds = 3000)

        val finished = dao.get(id)!!
        assertEquals("untracked must survive the finish", true, finished.isUntracked)
        assertEquals(5_000L, finished.finishedAt)
        assertEquals(1234.0, finished.totalVolumeLb!!, 0.001)
        assertEquals(2, finished.prCount)
        assertEquals(9, finished.setCount)
        assertEquals(3000, finished.activeSeconds)
    }

    @Test
    fun `an open session stays open until someone finishes it`() = runTest {
        val id = dao.insert(session(finishedAt = null))
        assertNull(dao.get(id)!!.finishedAt)
    }
}
