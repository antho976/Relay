package com.forge.app.domain.notify

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TrainingReminderTest {

    @Test
    fun trainedToday_staysQuiet() {
        assertNull(TrainingReminder.build(trainedToday = true, dayName = "Push", streakDays = 5))
    }

    @Test
    fun noScheduleOrProgram_staysQuiet() {
        // dayName null and NOT a scheduled rest day (no program / sequence mode) → stay silent.
        assertNull(TrainingReminder.build(trainedToday = false, dayName = null, streakDays = 5))
        assertNull(TrainingReminder.build(trainedToday = false, dayName = "", streakDays = 5))
    }

    @Test
    fun scheduledRestDay_offersAGentleRecoveryNote() {
        val n = TrainingReminder.build(
            trainedToday = false, dayName = null, streakDays = 5, isScheduledRestDay = true
        )
        assertEquals("Rest day", n!!.title)
        assertEquals("Nothing scheduled today.", n.body)
    }

    @Test
    fun aRestDayNamesWhatComesNext() {
        val n = TrainingReminder.build(
            trainedToday = false, dayName = null, streakDays = 0, isScheduledRestDay = true,
            nextDayName = "Pull A", nextWhen = "Thursday"
        )
        assertEquals("Nothing scheduled today. Next: Pull A, Thursday.", n!!.body)
    }

    @Test
    fun restDayFlagIgnoredOnceTrainedToday() {
        assertNull(TrainingReminder.build(
            trainedToday = true, dayName = null, streakDays = 5, isScheduledRestDay = true
        ))
    }

    @Test
    fun scheduledDay_noStreak_plainNudge() {
        val n = TrainingReminder.build(trainedToday = false, dayName = "Upper A", streakDays = 0)
        assertEquals("Time to train", n!!.title)
        assertTrue(n.body.contains("Upper A"))
    }

    @Test
    fun scheduledDay_withStreak_emphasizesTheStreak() {
        val n = TrainingReminder.build(trainedToday = false, dayName = "Legs", streakDays = 4)
        assertEquals("4-day streak", n!!.title)
        assertTrue(n.body.contains("Legs"))
    }

    @Test
    fun streakOfOne_isNotYetAStreak() {
        // 1 day isn't a streak worth a protection message — stay plain.
        val n = TrainingReminder.build(trainedToday = false, dayName = "Pull", streakDays = 1)
        assertEquals("Time to train", n!!.title)
    }

    @Test
    fun freestyle_noDay_stillNudgesWithPlanAgnosticCopy() {
        // No plan to name a day, but the user opted in — nudge with generic copy, no day name.
        val n = TrainingReminder.build(
            trainedToday = false, dayName = null, streakDays = 0, noFixedPlan = true
        )
        assertEquals("Time to train", n!!.title)
        assertTrue(n.body.contains("log whatever", ignoreCase = true))
    }

    @Test
    fun freestyle_withStreak_protectsTheStreakWithoutADayName() {
        val n = TrainingReminder.build(
            trainedToday = false, dayName = null, streakDays = 4, noFixedPlan = true
        )
        assertEquals("4-day streak", n!!.title)
        assertTrue(n.body.contains("log whatever", ignoreCase = true))
    }

    @Test
    fun freestyle_trainedToday_staysQuiet() {
        assertNull(TrainingReminder.build(
            trainedToday = true, dayName = null, streakDays = 4, noFixedPlan = true
        ))
    }

    /** The first nudge after a break names it, the way Home does, instead of reading like any day. */
    @Test
    fun aLongBreakIsNamedInWeeks() {
        val n = TrainingReminder.build(trainedToday = false, dayName = "Push A", streakDays = 0, daysSinceLast = 23)
        assertEquals("Push A today", n!!.title)
        assertTrue(n.body, n.body.contains("First session in 3 weeks"))
    }

    @Test
    fun aFewDaysOffAreCountedNotDramatised() {
        val n = TrainingReminder.build(trainedToday = false, dayName = "Push A", streakDays = 0, daysSinceLast = 4)
        assertEquals("Push A today. Your last session was 4 days ago.", n!!.body)
    }

    /** §11: no emoji, no exclamation marks, no em dashes in anything the reminder can say. */
    @Test
    fun noVariantBreaksTheVoiceRules() {
        val variants = listOfNotNull(
            TrainingReminder.build(false, "Legs", 4),
            TrainingReminder.build(false, "Legs", 0),
            TrainingReminder.build(false, "Legs", 0, daysSinceLast = 30),
            TrainingReminder.build(false, null, 4, noFixedPlan = true),
            TrainingReminder.build(false, null, 0, noFixedPlan = true, daysSinceLast = 30),
            TrainingReminder.build(false, null, 0, isScheduledRestDay = true, nextDayName = "Pull", nextWhen = "tomorrow")
        )
        variants.forEach { n ->
            val text = n.title + " " + n.body
            assertTrue(text, '—' !in text && '!' !in text && text.none { Character.isSurrogate(it) })
        }
    }
}
