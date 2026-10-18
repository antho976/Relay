package com.quietsoftware.relay.ui.threads

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate

/** Shares, refusals and days read as the desktop's do. */
class FormatTest {
    @Test
    fun aSliverIsUnderOnePercentNotNothing() {
        assertEquals("<1%", share(49))
        assertEquals("1%", share(50))
        assertEquals("0%", share(0))
        assertEquals("52%", share(5_200))
    }

    @Test
    fun aRefusedToolSaysTheEnginesSentence() {
        assertEquals("No such entry", refusalWords("""{"error": "Rejected money.not_found: No such entry"}"""))
        assertEquals("", refusalWords(""))
    }

    @Test
    fun todayIsLowercaseInsideASentence() {
        assertEquals("today", inlineDate(LocalDate.now().toString()))
        assertEquals("yesterday", inlineDate(LocalDate.now().minusDays(1).toString()))
    }
}
