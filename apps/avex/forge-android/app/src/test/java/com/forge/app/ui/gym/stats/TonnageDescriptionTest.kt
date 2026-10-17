package com.forge.app.ui.gym.stats

import org.junit.Assert.assertEquals
import org.junit.Test

class TonnageDescriptionTest {

    @Test fun `reads the span, the range and the last week`() {
        assertEquals(
            "Weekly volume over 3 weeks, from 9000 to 12000 lb, last week 12000 lb.",
            tonnageDescription(listOf(10_000.0, 9_000.0, 12_000.0), deloadWeeks = 0, unit = "lb")
        )
    }

    @Test fun `counts deload weeks`() {
        assertEquals(
            "Weekly volume over 2 weeks, from 4000 to 8000 kg, last week 8000 kg, 1 deload week.",
            tonnageDescription(listOf(4_000.0, 8_000.0), deloadWeeks = 1, unit = "kg")
        )
        assert(tonnageDescription(listOf(1.0, 2.0), deloadWeeks = 2, unit = "lb").endsWith("2 deload weeks."))
    }
}
