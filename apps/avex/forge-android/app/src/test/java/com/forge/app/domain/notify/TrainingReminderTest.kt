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
    fun restDay_staysQuiet() {
        assertNull(TrainingReminder.build(trainedToday = false, dayName = null, streakDays = 5))
        assertNull(TrainingReminder.build(trainedToday = false, dayName = "", streakDays = 5))
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
        assertEquals("Don't break your streak", n!!.title)
        assertTrue(n.body.contains("4-day streak"))
        assertTrue(n.body.contains("Legs"))
    }

    @Test
    fun streakOfOne_isNotYetAStreak() {
        // 1 day isn't a streak worth a protection message — stay plain.
        val n = TrainingReminder.build(trainedToday = false, dayName = "Pull", streakDays = 1)
        assertEquals("Time to train", n!!.title)
    }
}
