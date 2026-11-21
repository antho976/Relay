package com.forge.app.data.db.dao

import android.app.Application
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class PreviousSessionBoundaryTest {
    private val db = inMemoryForgeDb()
    @After fun close() = db.close()
    @Test fun `older detail compares to predecessor with timestamp ties and untracked rows excluded`() = runBlocking {
        val dao = db.sessionDao()
        val a = dao.insert(session(finishedAt = 1000))
        val b = dao.insert(session(finishedAt = 2000))
        val c = dao.insert(session(finishedAt = 3000))
        assertNull(dao.previousFinishedForDay("upper-a", a, 1000))
        assertEquals(a, dao.previousFinishedForDay("upper-a", b, 2000)!!.id)
        assertEquals(b, dao.previousFinishedForDay("upper-a", c, 3000)!!.id)
        val tie = dao.insert(session(finishedAt = 3000))
        assertEquals(c, dao.previousFinishedForDay("upper-a", tie, 3000)!!.id)
        dao.insert(session(finishedAt = 2500, untracked = true))
        assertEquals(b, dao.previousFinishedForDay("upper-a", c, 3000)!!.id)
    }
}
