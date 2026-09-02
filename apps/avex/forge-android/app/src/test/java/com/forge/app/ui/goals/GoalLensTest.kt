package com.forge.app.ui.goals

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * M-31: the Goals lens can only be a lens the user can get back out of.
 *
 * The pills that set it are shown only while both categories have rows. Delete the last reached
 * goal with "Reached" selected and the pills go with it, so the saved choice outlived its own
 * control: the screen rendered "Nothing reached yet" over a list of live goals with nothing on it
 * to switch back with, until the route was recreated.
 */
class GoalLensTest {

    @Test
    fun theUsersChoiceGovernsWhileBothCategoriesHaveRows() {
        assertEquals(GoalLens.REACHED, effectiveGoalLens(GoalLens.REACHED, liveCount = 2, reachedCount = 1))
        assertEquals(GoalLens.LIVE, effectiveGoalLens(GoalLens.LIVE, liveCount = 2, reachedCount = 1))
    }

    @Test
    fun withNoChoiceMadeTheScreenOpensOnLiveWhenThereAreLiveGoals() {
        assertEquals(GoalLens.LIVE, effectiveGoalLens(null, liveCount = 2, reachedCount = 1))
    }

    @Test
    fun aChoiceForTheEmptyCategoryIsOverruledRatherThanHidingEverything() {
        // The audit's reproduction: Reached was selected, then the last reached goal was deleted.
        assertEquals(GoalLens.LIVE, effectiveGoalLens(GoalLens.REACHED, liveCount = 3, reachedCount = 0))
        // And its inverse, when the last live goal is the one that goes.
        assertEquals(GoalLens.REACHED, effectiveGoalLens(GoalLens.LIVE, liveCount = 0, reachedCount = 3))
    }

    @Test
    fun withNothingTrackedAtAllTheScreenReadsAsLive() {
        // "Nothing tracked yet" belongs to the live lens; "Nothing reached yet" would be a claim
        // about a category the account does not have.
        assertEquals(GoalLens.LIVE, effectiveGoalLens(null, liveCount = 0, reachedCount = 0))
        assertEquals(GoalLens.LIVE, effectiveGoalLens(GoalLens.REACHED, liveCount = 0, reachedCount = 0))
    }
}
