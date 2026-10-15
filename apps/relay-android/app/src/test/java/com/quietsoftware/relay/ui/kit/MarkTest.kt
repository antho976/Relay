package com.quietsoftware.relay.ui.kit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The opening's timeline, kept in step with start.rs's tests. */
class MarkTest {
    @Test fun `every part starts hidden and ends in place`() {
        for (span in Mark.LANES + Mark.SIGNAL_IN) {
            assertEquals(0f, Mark.phaseBack(0f, span), 1e-6f)
            assertEquals(1f, Mark.phaseBack(1f, span), 1e-6f)
        }
        for (span in listOf(Mark.WAVE, Mark.PING, Mark.WORDMARK, Mark.GREETING, Mark.QUESTION, Mark.CARD_DEV, Mark.CARD_THREADS, Mark.FOOT)) {
            assertEquals(0f, Mark.phase(0f, span), 1e-6f)
            assertEquals(1f, Mark.phase(1f, span), 1e-6f)
        }
        for (order in Mark.LANE.indices) {
            assertEquals(0f, Mark.recoil(0f, order), 1e-6f)
            assertEquals(0f, Mark.recoil(1f, order), 1e-5f)
        }
    }

    @Test fun `the lanes come in back to front and the signal leaves the front one as it lands`() {
        Mark.LANES.toList().zipWithNext().forEach { (a, b) -> assertTrue(b.from > a.from && b.to > a.to) }
        Mark.LANE.toList().zipWithNext().forEach { (a, b) -> assertTrue(b[0] < a[0]) }
        assertEquals(Mark.LANES.last().to, Mark.SIGNAL_IN.from, 1e-6f)
        assertEquals(Mark.LANE.last()[0], Mark.SIGNAL_Y, 0f)
        // A visible gap between the end of the front lane's cap and the dot.
        assertTrue(Mark.SIGNAL_X - Mark.SIGNAL_DOT - (Mark.LANE.last()[2] + Mark.STROKE / 2) > 4f)
        assertTrue(Mark.PING.from < Mark.SIGNAL_IN.to)
    }
}
