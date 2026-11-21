package com.forge.app.domain.coach

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A single lift left alone for weeks comes back eased, though training never stopped. */
class StaleLiftTest {

    private val day = 24L * 60 * 60 * 1000
    private val now = 1_000 * day

    @Test
    fun `a lift trained last week is not eased`() {
        assertEquals(1.0, LifeEvents.staleLiftScale(now - 7 * day, now), 0.0)
        assertNull(LifeEvents.staleLiftNote(now - 7 * day, now))
    }

    @Test
    fun `a lift left for three weeks takes the ramp, and says why`() {
        assertEquals(LifeEvents.RAMP_SCALE, LifeEvents.staleLiftScale(now - 21 * day, now), 0.0)
        assertEquals("first time on this lift in 3 weeks", LifeEvents.staleLiftNote(now - 21 * day, now))
        assertEquals("first time on this lift in 6 weeks", LifeEvents.staleLiftNote(now - 45 * day, now))
    }

    @Test
    fun `a lift with no earlier bout has nothing to ease from`() {
        assertEquals(1.0, LifeEvents.staleLiftScale(null, now), 0.0)
        assertNull(LifeEvents.staleLiftNote(null, now))
    }
}
