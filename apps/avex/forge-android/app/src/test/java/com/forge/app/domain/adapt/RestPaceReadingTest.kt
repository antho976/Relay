package com.forge.app.domain.adapt

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RestPaceReadingTest {

    @Test fun `nothing until a role has enough rests`() {
        val tuning = RestTuning(mapOf(MovementRole.COMPOUND to 0.8), mapOf(MovementRole.COMPOUND to 3))
        assertNull(RestAdvisor.paceReading(tuning, 120, 90))
        assertNull(RestAdvisor.paceReading(RestTuning.NEUTRAL, 120, 90))
    }

    @Test fun `a shorter pace reads as the time the timer now runs`() {
        val tuning = RestTuning(
            factors = mapOf(MovementRole.COMPOUND to 0.875, MovementRole.ISOLATION to 0.8),
            sampleCounts = mapOf(MovementRole.COMPOUND to 20, MovementRole.ISOLATION to 12)
        )
        assertEquals(
            "Your pace so far: compounds about 1:45, isolations about 1:15, from 32 rests.",
            RestAdvisor.paceReading(tuning, 120, 90)
        )
    }

    @Test fun `a longer pace keeps the full base, since the timer only shortens`() {
        val tuning = RestTuning(mapOf(MovementRole.COMPOUND to 1.3), mapOf(MovementRole.COMPOUND to 10))
        assertEquals("Your pace so far: compounds the full 2:30, from 10 rests.", RestAdvisor.paceReading(tuning, 150, 90))
    }

    @Test fun `the count covers only the roles shown`() {
        val tuning = RestTuning(
            factors = mapOf(MovementRole.COMPOUND to 0.875, MovementRole.ISOLATION to 0.8),
            sampleCounts = mapOf(MovementRole.COMPOUND to 20, MovementRole.ISOLATION to 3)
        )
        assertEquals("Your pace so far: compounds about 1:45, from 20 rests.", RestAdvisor.paceReading(tuning, 120, 90))
    }
}
