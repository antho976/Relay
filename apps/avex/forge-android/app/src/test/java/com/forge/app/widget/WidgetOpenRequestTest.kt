package com.forge.app.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * M-30: the widget's primary action, on the two paths it used to fail on.
 *
 * A repeated tap assigned the same sticky day string, which is no state change at all, so the nav
 * host's value-keyed effect never ran again. And a cold tap for a custom builder day was judged
 * against the seed split — the program `Program` reports until the database one is loaded into it —
 * rejected, and never retried, because the string it was carried on did not change afterwards.
 */
class WidgetOpenRequestTest {

    private val loadedProgram = listOf("upper-a", "lower-a", "upper-b")
    private val seedProgram = listOf("push", "pull", "legs")

    @Test
    fun theSameDayTappedTwiceIsTwoDifferentEvents() {
        val first = WidgetOpenRequest(id = 1L, dayKey = "upper-a")
        val second = WidgetOpenRequest(id = 2L, dayKey = "upper-a")

        assertNotEquals("the second tap must not look like the first", first, second)
        assertEquals(first.dayKey, second.dayKey)
    }

    @Test
    fun aProgramDayOpensThatDay() {
        assertEquals(
            WidgetDestination.GymDay("upper-a"),
            widgetDestinationFor("upper-a", loadedProgram)
        )
    }

    @Test
    fun aCustomDayIsOnlyRecognisedOnceTheRealProgramIsLoaded() {
        // The cold-launch race, both halves of it.
        assertEquals(
            "against the seed split it is not a day at all",
            WidgetDestination.Unrecognised,
            widgetDestinationFor("upper-a", seedProgram)
        )
        assertEquals(
            WidgetDestination.GymDay("upper-a"),
            widgetDestinationFor("upper-a", loadedProgram)
        )
    }

    @Test
    fun aCardioTapSelectsTheHubPageWhateverTheProgramHolds() {
        assertEquals(WidgetDestination.CardioTab, widgetDestinationFor("cardio", loadedProgram))
        assertEquals(WidgetDestination.CardioTab, widgetDestinationFor("cardio", emptyList()))
    }

    @Test
    fun aProgramDayWins_soADayNamedLikeTheCardioKeyIsStillOpenedAsADay() {
        assertEquals(
            WidgetDestination.GymDay("cardio-conditioning"),
            widgetDestinationFor("cardio-conditioning", listOf("cardio-conditioning"))
        )
    }

    @Test
    fun aDayTheProgramNoLongerHasIsRejectedRatherThanLeftPending() {
        assertEquals(WidgetDestination.Unrecognised, widgetDestinationFor("deleted-day", loadedProgram))
        assertEquals(WidgetDestination.Unrecognised, widgetDestinationFor("", loadedProgram))
    }
}
