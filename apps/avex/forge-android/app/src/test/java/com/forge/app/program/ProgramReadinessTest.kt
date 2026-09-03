package com.forge.app.program

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * M-30: "has the program loaded" has to be something a caller can await, and it has to be able to
 * say that the answer is never coming.
 *
 * `isLoaded` was a volatile flag, so the only way to wait on it was to poll behind a timeout — and
 * a timeout cannot distinguish a slow load from a load that will not happen. Both look like "still
 * false", and the widget deep-link answered both by judging a day key against the seed split.
 */
class ProgramReadinessTest {

    @After
    fun restore() = Program.setActive(Program.seedDays)

    @Test
    fun aSuccessfulLoadReportsItself() {
        Program.setActive(Program.seedDays)
        assertEquals(Program.Readiness.LOADED, Program.readiness.value)
    }

    @Test
    fun aLoadThatAlreadyLandedIsNotDowngradedByALaterFailure() {
        Program.setActive(Program.seedDays)
        Program.markLoadFailed()
        assertEquals(
            "the program IS loaded; a later failed attempt does not un-load it",
            Program.Readiness.LOADED, Program.readiness.value
        )
    }
}
