package com.tally.app.ui.home

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class HomeLogicTest {

    private lateinit var saved: Locale

    @Before fun english() {
        saved = Locale.getDefault()
        Locale.setDefault(Locale.CANADA)
    }

    @After fun restore() = Locale.setDefault(saved)

    @Test fun oneAccountReadsSingular() {
        assertEquals("NET ACROSS 1 ACCOUNT", netAcrossLabel(1))
        assertEquals("NET ACROSS 4 ACCOUNTS", netAcrossLabel(4))
    }

    @Test fun weekDescriptionNamesEachDaySoFarInFull() {
        // Sunday 11 Oct 2026 starts the week; today is the Wednesday, so four days are read.
        val week = listOf(4_200L, 9_800L, 2_100L, 3_400L, 0L, 0L, 0L)
        val text = weekDescription(LocalDate.of(2026, 10, 11), week, todayInWeek = 3) { "$" + it / 100 }
        assertEquals("This week: Sunday $42, Monday $98, Tuesday $21, Wednesday $34", text)
    }

    @Test fun weekDescriptionTellsSaturdayFromSundayAndTuesdayFromThursday() {
        val text = weekDescription(LocalDate.of(2026, 10, 11), List(7) { 0L }, todayInWeek = 6) { "$" + it }
        assertEquals(
            "This week: Sunday $0, Monday $0, Tuesday $0, Wednesday $0, Thursday $0, Friday $0, Saturday $0",
            text,
        )
    }
}
