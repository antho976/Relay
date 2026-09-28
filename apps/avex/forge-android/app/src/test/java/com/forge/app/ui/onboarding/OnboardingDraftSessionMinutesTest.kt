package com.forge.app.ui.onboarding

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The session-length answer survives a killed app mid-onboarding, tri-state intact. */
// Robolectric: the round-trips go through org.json, which the plain JVM only has as android.jar
// stubs that throw, so these tests could never pass outside it.
@RunWith(RobolectricTestRunner::class)
class OnboardingDraftSessionMinutesTest {

    private fun draft(sessionMinutes: Int?) = OnboardingDraft(
        step = 3, planMode = PLAN_GENERATED, name = "", useKg = false, useMilesChoice = false,
        distanceTouched = false, goal = "build_muscle", experience = "intermediate", bodyweightInput = "",
        sex = null, daysPerWeek = 4, equipment = emptySet(), frozenIds = null, plateWeightLb = 15.0,
        problemAreas = emptySet(), cadence = "", everyN = 4, previewSeed = 1L, appLock = false,
        coachChoice = null, sessionMinutes = sessionMinutes
    )

    @Test
    fun anAnswerRoundTrips() {
        assertEquals(45, OnboardingDraft.fromJson(draft(45).toJson())!!.sessionMinutes)
    }

    @Test
    fun anExplicitAnyRoundTrips() {
        assertEquals(0, OnboardingDraft.fromJson(draft(0).toJson())!!.sessionMinutes)
    }

    @Test
    fun unansweredStaysUnanswered() {
        assertNull(OnboardingDraft.fromJson(draft(null).toJson())!!.sessionMinutes)
    }
}
