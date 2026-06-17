package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test

/** The set-write reps clamp — a 4-digit fat-finger must not poison e1RM/volume (robustness). */
class SanitizeRepsTest {

    @Test
    fun normalRepsAreUntouched() {
        assertEquals(8, sanitizeReps(8))
        assertEquals(1, sanitizeReps(1))
        assertEquals(MAX_LOGGED_REPS, sanitizeReps(MAX_LOGGED_REPS))
    }

    @Test
    fun fourDigitFatFingerIsCappedNotLogged() {
        assertEquals(MAX_LOGGED_REPS, sanitizeReps(9999))
        assertEquals(MAX_LOGGED_REPS, sanitizeReps(MAX_LOGGED_REPS + 1))
    }

    @Test
    fun strayNegativeFloorsToZero() {
        assertEquals(0, sanitizeReps(-5))
        assertEquals(0, sanitizeReps(0))
    }
}
