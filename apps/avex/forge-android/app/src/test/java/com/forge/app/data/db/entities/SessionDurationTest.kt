package com.forge.app.data.db.entities

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SessionDurationTest {
    private val start = 1_000_000_000L
    private val min = 60_000L

    @Test fun `active time wins over the wall clock`() {
        // Started at night, finished the next morning, 52 minutes actually trained.
        assertEquals(52, sessionDurationMinutes(start, start + 486 * min, activeSeconds = 52 * 60))
    }

    @Test fun `wall clock when no active time was recorded`() {
        assertEquals(55, sessionDurationMinutes(start, start + 55 * min, activeSeconds = 0))
    }

    @Test fun `a short session reads at least a minute, and an open one has no length`() {
        assertEquals(1, sessionDurationMinutes(start, start + 20_000, activeSeconds = 0))
        assertNull(sessionDurationMinutes(start, null, activeSeconds = 0))
    }
}
