package com.forge.app.domain.units

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.Locale

class ClockTextTest {
    @Test fun `minutes then hours`() {
        assertEquals("0:45", clockText(45))
        assertEquals("10:05", clockText(605))
        assertEquals("1:05:12", clockText(3912))
    }

    @Test fun `ascii digits whatever the locale`() {
        val saved = Locale.getDefault()
        try {
            Locale.setDefault(Locale.forLanguageTag("ar-EG"))
            assertEquals("2:00", clockText(120))
        } finally {
            Locale.setDefault(saved)
        }
    }
}
