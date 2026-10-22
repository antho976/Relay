package com.forge.app.ui.onboarding

import com.forge.app.program.Equipment
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The footer's "plan so far" line, step by step, as the onboarding designs write it. */
class PlanSoFarTest {

    private val full = Equipment.entries.map { it.name }.toSet()
    private val answers = SoFarAnswers(
        planMode = PLAN_GENERATED, goal = "build_muscle", experience = "intermediate",
        daysPerWeek = 5, sessionMinutes = 60, equipment = full,
        dayNames = listOf("Push A", "Pull A", "Legs", "Upper", "Lower"), weekSets = 78
    )

    @Test fun modeNamesThePick() = assertEquals("Build me a plan", planSoFar(SoFarPage.MODE, answers))

    @Test fun nothingPickedSaysNothing() = assertNull(planSoFar(SoFarPage.MODE, SoFarAnswers()))

    @Test fun goal() = assertEquals("Plan · build muscle", planSoFar(SoFarPage.GOAL, answers))

    @Test fun goalBeforeItIsPickedKeepsTheMode() =
        assertEquals("Build me a plan", planSoFar(SoFarPage.GOAL, answers.copy(goal = "")))

    @Test fun days() = assertEquals("Build muscle · 5 days · 60 min", planSoFar(SoFarPage.DAYS, answers))

    @Test fun anyLengthIsLeftOut() =
        assertEquals("Build muscle · 5 days", planSoFar(SoFarPage.DAYS, answers.copy(sessionMinutes = 0)))

    @Test fun gym() = assertEquals("Build muscle · 5 days · 60 min · full gym", planSoFar(SoFarPage.GYM, answers))

    @Test fun aSmallGymCountsItsPieces() = assertEquals(
        "Build muscle · 5 days · 60 min · 4 pieces",
        planSoFar(SoFarPage.GEAR, answers.copy(equipment = full.take(4).toSet()))
    )

    @Test fun spotsAreCounted() = assertEquals(
        "Build muscle · 5 days · 60 min · full gym · 2 spots flagged",
        planSoFar(SoFarPage.SPOTS, answers.copy(problemAreas = setOf("knees", "shoulders")))
    )

    @Test fun week() = assertEquals("5 days · 78 sets · ~60 min each", planSoFar(SoFarPage.WEEK, answers))

    @Test fun readyNamesTheFirstDayAndItsWeekday() = assertEquals(
        "Ready · Push A is first, Monday",
        planSoFar(SoFarPage.EXTRAS, answers.copy(fixedDays = true, weekdays = setOf(0, 2, 4, 5, 6)))
    )

    @Test fun readyWithoutFixedDays() =
        assertEquals("Ready · Push A is first", planSoFar(SoFarPage.EXTRAS, answers.copy(fixedDays = false)))

    @Test fun readyForTheShortPaths() {
        assertEquals("Ready · you build the plan next", planSoFar(SoFarPage.EXTRAS, answers.copy(planMode = PLAN_CUSTOM)))
        assertEquals("Ready · log whatever you train", planSoFar(SoFarPage.EXTRAS, answers.copy(planMode = PLAN_FREESTYLE)))
    }
}
