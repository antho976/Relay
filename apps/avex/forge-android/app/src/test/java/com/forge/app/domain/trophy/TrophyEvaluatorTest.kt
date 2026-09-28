package com.forge.app.domain.trophy

import com.forge.app.program.UnlockRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TrophyEvaluatorTest {

    private fun snap(
        goalsAchieved: Int = 0,
        finishedSessions: Int = 0,
        maxStreak: Int = 0
    ) = TrophyStatsSnapshot(
        totalLoggedExercises = 0, totalPrs = 0, brutalRatings = 0, swapsUsed = 0,
        fullTargetHits = 0, finishedSessions = finishedSessions, distinctDayKeysTrained = 0,
        maxBenchLb = 0.0, maxSquatLb = 0.0, maxSessionVolumeLb = 0.0,
        maxStreakEver = maxStreak,
        exerciseGoalsAchieved = goalsAchieved
    )

    @Test
    fun goalCrusher_unlocksAtOneAchievedGoal() {
        assertFalse("goal_crusher" in TrophyEvaluator.unlockedByRule(snap(goalsAchieved = 0)))
        assertTrue("goal_crusher" in TrophyEvaluator.unlockedByRule(snap(goalsAchieved = 1)))
    }

    @Test
    fun goalGetter_needsFive() {
        assertFalse("goals_5" in TrophyEvaluator.unlockedByRule(snap(goalsAchieved = 4)))
        assertTrue("goals_5" in TrophyEvaluator.unlockedByRule(snap(goalsAchieved = 5)))
    }

    @Test
    fun goalRule_progressFractionScales() {
        assertEquals(0.6f, TrophyEvaluator.progressFraction(UnlockRule.ExerciseGoalsAchievedAtLeast(5), snap(goalsAchieved = 3)), 0.001f)
    }

    @Test
    fun sessionDurationAtMost_progressAgreesWithUnlock() {
        // A sub-5-minute session never unlocks the trophy, so it must not fill the bar either.
        val rule = UnlockRule.SessionDurationAtMost(45)
        val tooShort = snap().copy(minFinishedSessionDurationMinutes = 2)
        assertFalse(TrophyEvaluator.isUnlocked(rule, tooShort))
        assertEquals(0f, TrophyEvaluator.progressFraction(rule, tooShort), 0f)
        val quick = snap().copy(minFinishedSessionDurationMinutes = 30)
        assertTrue(TrophyEvaluator.isUnlocked(rule, quick))
        assertEquals(1f, TrophyEvaluator.progressFraction(rule, quick), 0f)
    }

    @Test
    fun expandedMilestones_areWiredIntoTheCatalog() {
        val unlocked = TrophyEvaluator.unlockedByRule(snap(finishedSessions = 100, maxStreak = 30))
        assertTrue("workouts_50" in unlocked)
        assertTrue("workouts_100" in unlocked)
        assertTrue("streak_30" in unlocked)
    }
}
