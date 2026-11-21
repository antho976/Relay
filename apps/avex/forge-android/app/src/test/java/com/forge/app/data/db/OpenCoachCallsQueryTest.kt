package com.forge.app.data.db

import android.app.Application
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Stats links a plateau read to Coach only while the LATEST pass holds an open call for it. */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class OpenCoachCallsQueryTest {

    private val db = inMemoryForgeDb()
    private val dao = db.coachDao()

    @After fun close() = db.close()

    private fun call(week: String, target: String, status: String) = CoachDecision(
        weekId = week, type = "swap", targetKey = target, targetName = target,
        summary = "s", reason = "r", status = status
    )

    @Test
    fun onlyTheLatestPassesProposedCallsCount() = runTest {
        dao.insertPass(CoachPass("2026-W41", 1_000L, "proposed", null))
        dao.insertPass(CoachPass("2026-W42", 2_000L, "proposed", null))
        dao.insertDecisions(listOf(
            // Last week's open call is stale: a new pass has run since.
            call("2026-W41", "old_lift", "proposed"),
            call("2026-W42", "lat_pulldown", "proposed"),
            call("2026-W42", "lat_pulldown", "proposed"),
            call("2026-W42", "bench", "applied"),
            call("2026-W42", "squat", "skipped"),
            // A coach that is off writes its calls shadow; there is nothing to act on.
            call("2026-W42", "row", "shadow"),
        ))

        assertEquals(listOf("lat_pulldown"), dao.observeOpenCallTargets().first())
    }
}
