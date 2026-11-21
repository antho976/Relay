package com.forge.app.domain.coach

import com.forge.app.domain.coach.SessionAdaptor.TimedExercise
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The day screen's "I have N minutes today". */
class SessionTimeFitTest {

    // Canonical pricing: a compound set is 45s + 120s rest, an isolation set 45s + 90s rest.
    private val day = listOf(
        TimedExercise("squat", remainingSets = 4, restSeconds = 120, compound = true),   // 9:00
        TimedExercise("rdl", remainingSets = 3, restSeconds = 120, compound = true),     // 6:15
        TimedExercise("ext", remainingSets = 3, restSeconds = 90, compound = false),     // 5:15
        TimedExercise("curl", remainingSets = 3, restSeconds = 90, compound = false),    // 5:15
        TimedExercise("calf", remainingSets = 3, restSeconds = 90, compound = false)     // 5:15
    )

    @Test
    fun enoughTimeKeepsEverything() {
        val fit = SessionAdaptor.fitToTime(day, minutesAvailable = 60)
        assertEquals(day.map { it.id }, fit.keepIds)
        assertTrue(fit.dropIds.isEmpty())
    }

    @Test
    fun shortOnTimeKeepsTheCompoundsAndTheDaysOrder() {
        val fit = SessionAdaptor.fitToTime(day, minutesAvailable = 20)
        assertEquals(listOf("squat", "rdl"), fit.keepIds.take(2))
        assertTrue("20 minutes can't hold all five", fit.dropIds.isNotEmpty())
        assertTrue("dropped work is accessories", fit.dropIds.none { it == "squat" || it == "rdl" })
        val kept = day.filter { it.id in fit.keepIds }.sumOf { it.seconds }
        assertTrue("kept ${kept}s against 1200s", kept <= 20 * 60)
        // Survivors stay in the day's order.
        assertEquals(day.map { it.id }.filter { it in fit.keepIds }, fit.keepIds)
    }

    @Test
    fun laterSmallerWorkFillsTheGap() {
        // 25 min: squat + rdl = 15:15, then ext 5:15 → 20:30, curl would pass 25, calf too → both out.
        val fit = SessionAdaptor.fitToTime(day, minutesAvailable = 25)
        assertEquals(listOf("squat", "rdl", "ext"), fit.keepIds)
        assertEquals(listOf("curl", "calf"), fit.dropIds)
    }

    @Test
    fun startedWorkIsNeverCut() {
        val started = day.map { if (it.id == "calf") it.copy(started = true) else it }
        val fit = SessionAdaptor.fitToTime(started, minutesAvailable = 10)
        assertTrue("calf" in fit.keepIds)
    }

    @Test
    fun aSessionStaysASession() {
        val fit = SessionAdaptor.fitToTime(day, minutesAvailable = 1)
        assertEquals(SessionAdaptor.MIN_EXERCISES, fit.keepIds.size)
        assertEquals(listOf("squat", "rdl"), fit.keepIds)
    }

    @Test
    fun emptyDayIsFine() {
        val fit = SessionAdaptor.fitToTime(emptyList(), minutesAvailable = 30)
        assertTrue(fit.keepIds.isEmpty() && fit.dropIds.isEmpty())
    }
}
