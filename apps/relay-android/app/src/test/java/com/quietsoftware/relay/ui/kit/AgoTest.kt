package com.quietsoftware.relay.ui.kit

import org.junit.Assert.assertEquals
import org.junit.Test

class AgoTest {
    private val now = 1_800_000_000_000L

    @Test fun `a missing time says nothing rather than Jan 1`() {
        assertEquals("", ago(0, now))
        assertEquals("", ago(epoch(null), now))
        assertEquals("", ago(epoch("not a time"), now))
    }

    @Test fun `a time reads as how long ago`() {
        assertEquals("just now", ago(now - 5_000, now))
        assertEquals("5m ago", ago(now - 5 * 60_000, now))
        assertEquals("3h ago", ago(now - 3 * 3_600_000, now))
    }
}
