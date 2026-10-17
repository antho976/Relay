package com.forge.app.domain.notify

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ComebackNudgeTest {

    private fun nudge(days: Int?, name: String? = "Push A", cardio: Int = 0, freestyle: Boolean = false) =
        ComebackNudge.build(days, name, cardio, freestyle)

    @Test
    fun speaksInTheFirstSecondAndFourthWeekOfALapseThenStops() {
        val spoken = (7..70).filter { nudge(it) != null }.map { it / 7 }.toSet()
        assertEquals(setOf(1, 2, 4), spoken)
    }

    @Test
    fun namesTheLastWorkoutAndHowLongAgo() {
        assertEquals(
            "Last workout: Push A, 9 days ago. Your plan is where you left it.",
            nudge(9)!!.body
        )
        assertEquals(
            "Last workout: Push A, 4 weeks ago. The first session back starts lighter.",
            nudge(30)!!.body
        )
    }

    @Test
    fun staysQuietForSomeoneWhoNeverTrainedOrDidCardio() {
        assertNull(nudge(null))
        assertNull(nudge(9, cardio = 40))
    }

    @Test
    fun freestyleNamesNoPlan() {
        assertEquals("Last workout 9 days ago. Log whatever you train, whenever.", nudge(9, name = null, freestyle = true)!!.body)
    }
}
