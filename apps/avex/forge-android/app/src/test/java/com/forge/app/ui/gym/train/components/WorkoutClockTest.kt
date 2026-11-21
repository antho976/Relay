package com.forge.app.ui.gym.train.components

import org.junit.Assert.assertEquals
import org.junit.Test

class WorkoutClockTest {
    @Test fun `minutes and seconds under the hour`() {
        assertEquals("0:00", workoutClock(0))
        assertEquals("5:07", workoutClock(307))
        assertEquals("59:59", workoutClock(3599))
    }

    @Test fun `hours past the hour`() {
        assertEquals("1:00:00", workoutClock(3600))
        assertEquals("1:15:12", workoutClock(4512))
    }

    @Test fun `a negative span reads as zero`() {
        assertEquals("0:00", workoutClock(-5))
    }
}
