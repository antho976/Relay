package com.forge.app.data.db

import android.app.Application
import androidx.room.withTransaction
import kotlinx.coroutines.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class FinishedSessionWriteTest {
    private val db = inMemoryForgeDb()
    @After fun close() = db.close()

    @Test fun `finish and set insertion cannot leave stamped totals behind real sets`() = runBlocking {
        repeat(20) {
            val sid = db.sessionDao().insert(session(finishedAt = null))
            val eid = db.loggedExerciseDao().insert(loggedExercise(sessionId = sid))
            withContext(Dispatchers.Default) {
                val finish = async { db.withTransaction {
                    db.sessionDao().finishIfUnfinished(sid, 9999)
                    val sets = db.loggedSetDao().allForSession(sid)
                    db.sessionDao().setFinishTotals(sid, 0.0, 0, sets.size, 1)
                } }
                val log = async {
                    try { SessionWrites.insertSetWithNextIndex(db, loggedSet(loggedExerciseId = eid)) }
                    catch (_: SessionClosedException) { }
                }
                awaitAll(finish, log)
            }
            assertEquals(db.loggedSetDao().allForSession(sid).size, db.sessionDao().get(sid)!!.setCount)
            try {
                SessionWrites.insertSetWithNextIndex(db, loggedSet(loggedExerciseId = eid))
                fail("finished session must reject a late set")
            } catch (_: SessionClosedException) { }
        }
    }
}
