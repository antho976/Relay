package com.forge.app.widget

import com.forge.app.program.Program
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

    // ── Readiness, not a timer ───────────────────────────────────────────────

    /**
     * The gap the four-second poll left. It waited, then judged the key against whatever `Program`
     * happened to hold — which on a slow cold start is still the seed split — and discarded the tap.
     * A load in flight and a day the program does not have became the same answer.
     */
    @Test
    fun aTapIsNotAnsweredUntilTheProgramHasActuallyLoaded() {
        assertEquals(
            "still loading: wait, do not judge",
            WidgetRouting.Wait,
            widgetRoutingFor(Program.Readiness.PENDING, "upper-a", seedProgram)
        )
        assertEquals(
            "loaded: now the key means something",
            WidgetRouting.Decided(WidgetDestination.GymDay("upper-a")),
            widgetRoutingFor(Program.Readiness.LOADED, "upper-a", loadedProgram)
        )
    }

    /**
     * A load that FAILED is not a verdict on the day either — the program is still the seed split.
     * Consuming the tap here is the same silent loss, reached by a different route, so the request
     * is kept for a load that does succeed.
     */
    @Test
    fun aFailedLoadKeepsTheTapRatherThanRejectingIt() {
        assertEquals(
            WidgetRouting.Wait,
            widgetRoutingFor(Program.Readiness.FAILED, "upper-a", seedProgram)
        )
    }

    /** And a loaded program that genuinely lacks the day DOES answer — the request is spent. */
    @Test
    fun aLoadedProgramWithoutTheDayIsAnAnswer() {
        assertEquals(
            WidgetRouting.Decided(WidgetDestination.Unrecognised),
            widgetRoutingFor(Program.Readiness.LOADED, "deleted-day", loadedProgram)
        )
    }
}
