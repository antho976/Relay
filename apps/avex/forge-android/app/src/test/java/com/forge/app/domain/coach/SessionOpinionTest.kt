package com.forge.app.domain.coach

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionOpinionTest {

    private fun opinion(
        setCount: Int = 10,
        prCount: Int = 0,
        ghostBeats: Int = 0,
        ghostComparable: Int = 0,
        vsLastVolumeDelta: Double? = null,
        isBestSession: Boolean = false,
        honestyPct: Int? = null,
        easingBack: Boolean = false
    ) = SessionOpinion.of(setCount, prCount, ghostBeats, ghostComparable, vsLastVolumeDelta, isBestSession, honestyPct, easingBack)

    @Test fun `empty session has no opinion`() {
        assertNull(opinion(setCount = 0, prCount = 3))
    }

    @Test fun `best session leads`() {
        // Best session outranks PRs / duel in the lead.
        val o = opinion(prCount = 2, isBestSession = true, ghostBeats = 5, ghostComparable = 5)
        assertTrue(o!!.startsWith("The most volume"))
    }

    @Test fun `prs lead when not a best session`() {
        assertTrue(opinion(prCount = 1)!!.contains("1 new PR"))
        assertTrue(opinion(prCount = 3)!!.contains("3 new PRs"))
    }

    @Test fun `clean sweep when every comparable set beaten`() {
        assertTrue(opinion(ghostBeats = 6, ghostComparable = 6)!!.startsWith("Clean sweep"))
    }

    @Test fun `won duel without a clean sweep`() {
        assertTrue(opinion(ghostBeats = 4, ghostComparable = 6)!!.contains("edged out last session on 4 of 6"))
    }

    @Test fun `volume up and down`() {
        assertTrue(opinion(vsLastVolumeDelta = 120.0)!!.contains("More volume than last time"))
        assertTrue(opinion(vsLastVolumeDelta = -120.0)!!.contains("lighter than last time"))
    }

    @Test fun `a first time through a day sets the baseline, a repeat says so`() {
        assertTrue(opinion()!!.startsWith("First time through this day"))
        assertEquals("The same volume as last time.", opinion(vsLastVolumeDelta = 0.0))
    }

    @Test fun `low completion appends a caveat`() {
        val o = opinion(prCount = 1, honestyPct = 45)!!
        assertTrue(o.contains("1 new PR"))
        assertTrue(o.contains("You logged 45% of the planned sets."))
    }

    @Test fun `high completion has no caveat`() {
        val o = opinion(prCount = 1, honestyPct = 95)!!
        assertEquals("1 new PR today.", o)
    }

    @Test fun `back from a break, lighter reads as the plan, not a warning`() {
        val o = opinion(vsLastVolumeDelta = -400.0, easingBack = true)!!
        assertTrue(o.startsWith("Lighter than before the break, as planned."))
        // A PR still leads, and a first time through still says baseline.
        assertTrue(opinion(prCount = 1, vsLastVolumeDelta = -400.0, easingBack = true)!!.startsWith("1 new PR"))
        assertTrue(opinion(easingBack = true)!!.startsWith("First time through this day"))
    }
}
