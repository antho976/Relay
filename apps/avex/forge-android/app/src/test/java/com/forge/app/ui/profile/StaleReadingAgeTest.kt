package com.forge.app.ui.profile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class StaleReadingAgeTest {
    private val day = 86_400_000L
    private val now = 1_000 * day

    @Test
    fun aRecentReadingHasNoAge() {
        assertNull(staleReadingAge(now - 3 * day, now))
        assertNull(staleReadingAge(null, now))
    }

    @Test
    fun anOldReadingSaysHowOld() {
        assertEquals("2 WK AGO", staleReadingAge(now - 14 * day, now))
        assertEquals("6 WK AGO", staleReadingAge(now - 45 * day, now))
        assertEquals("3 MO AGO", staleReadingAge(now - 95 * day, now))
        assertEquals("2 YR AGO", staleReadingAge(now - 800 * day, now))
    }
}
