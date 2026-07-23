package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SessionHrAnalysisTest {

    /** 1 Hz trace from t=0: bpm follows [levels] minute by minute. */
    private fun trace(vararg levels: Int): List<HrPoint> =
        levels.flatMapIndexed { minute, bpm ->
            (0 until 60).map { s -> HrPoint(timeMs = (minute * 60L + s) * 1000L, bpm = bpm) }
        }

    @Test
    fun `too few samples yields no view`() {
        assertNull(buildSessionHrView(trace().take(5), emptyList(), emptyList()))
        assertNull(buildSessionHrView(emptyList(), emptyList(), emptyList()))
    }

    @Test
    fun `avg and max cover the whole trace`() {
        val view = buildSessionHrView(trace(100, 140), emptyList(), emptyList())!!
        assertEquals(120, view.avgBpm)
        assertEquals(140, view.maxBpm)
    }

    @Test
    fun `per-exercise averages split on the set timeline`() {
        // Minute 0-1 at 100 (bench sets), minutes 2-3 at 140 (squat sets).
        val samples = trace(100, 100, 140, 140)
        // Bench's last set closes exactly at minute 1's final sample, so Squat's span (which owns
        // the transition time to ITS first set) holds only 140-bpm samples.
        val sets = listOf(
            HrSetRef(completedAtMs = 110_000L, exerciseName = "Bench"),
            HrSetRef(completedAtMs = 119_000L, exerciseName = "Bench"),
            HrSetRef(completedAtMs = 230_000L, exerciseName = "Squat")
        )
        val view = buildSessionHrView(samples, sets, emptyList())!!
        assertEquals(listOf("Bench", "Squat"), view.perExercise.map { it.name })
        assertEquals(100, view.perExercise[0].avgBpm)
        assertEquals(140, view.perExercise[1].avgBpm)
        // One boundary — where Squat's span begins (Bench's last set + 1ms).
        assertEquals(1, view.exerciseBoundariesMs.size)
        assertEquals(3, view.setMarkersMs.size)
    }

    @Test
    fun `hrr60 averages the drop over the first minute of long rests`() {
        // Rest starts at t=60s (bpm 150) and one minute later bpm is 110 → drop 40.
        val samples = trace(150, 150, 110, 110)
        val rests = listOf(HrRestRef(endedAtMs = 180_000L, realizedSeconds = 120))
        val view = buildSessionHrView(samples, emptyList(), rests)!!
        assertEquals(40, view.avgHrr60)
    }

    @Test
    fun `short rests are excluded from hrr`() {
        val samples = trace(150, 110)
        val rests = listOf(HrRestRef(endedAtMs = 90_000L, realizedSeconds = 30))
        assertNull(buildSessionHrView(samples, emptyList(), rests)!!.avgHrr60)
    }
}
