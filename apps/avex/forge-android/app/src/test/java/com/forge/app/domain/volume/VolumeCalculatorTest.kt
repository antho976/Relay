package com.forge.app.domain.volume

import com.forge.app.data.db.entities.LoggedSet
import org.junit.Assert.assertEquals
import org.junit.Test

class VolumeCalculatorTest {

    private fun set(weightLb: Double?, reps: Int) = LoggedSet(
        loggedExerciseId = 1L,
        setIndex = 0,
        weightText = weightLb?.toString() ?: "BW",
        weightLb = weightLb,
        reps = reps,
        completedAt = 0L
    )

    @Test
    fun emptySessionHasZeroVolume() {
        assertEquals(0.0, VolumeCalculator.sessionVolumeLb(emptyList()), 0.0)
    }

    @Test
    fun volumeIsSumOfWeightTimesReps() {
        val sets = listOf(set(100.0, 5), set(50.0, 10))
        assertEquals(1000.0, VolumeCalculator.sessionVolumeLb(sets), 0.0)
    }

    @Test
    fun bodyweightSetsContributeZero() {
        val sets = listOf(set(null, 12), set(40.0, 10))
        assertEquals(400.0, VolumeCalculator.sessionVolumeLb(sets), 0.0)
    }
}
