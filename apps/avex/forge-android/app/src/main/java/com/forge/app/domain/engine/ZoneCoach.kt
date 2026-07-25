package com.forge.app.domain.engine

/**
 * Live zone coaching (Engine E-C) — a pure state machine over the watch's HR stream.
 *
 * The classic failure of easy cardio is that it isn't easy: zone 2 creeps into zone 3 and the
 * session quietly stops being the thing it was for. This catches that and says so ONCE, quietly.
 *
 * **Hysteresis is the whole design.** A naive "you're out of zone" check fires every few seconds
 * at the boundary and turns a calm 40 minutes into a nagging watch. Drifting is only called after
 * the reading has been out for a sustained stretch, and it only re-fires after coming back.
 */
object ZoneCoach {

    /** Seconds out of band before the coach says anything. */
    const val DRIFT_SECONDS = 45

    /** Seconds back inside before the state resets and a new alert becomes possible. */
    const val RECOVER_SECONDS = 30

    enum class State { IN_ZONE, DRIFTING_HIGH, DRIFTING_LOW, UNKNOWN }

    /**
     * @param state where the athlete is against the target band.
     * @param alert non-null exactly once per drift episode — the thing to buzz or say.
     * @param secondsOutOfBand carried between updates; the caller passes the previous reading back.
     */
    data class Reading(
        val state: State,
        val alert: String? = null,
        val secondsOutOfBand: Int = 0,
        val secondsInBand: Int = 0,
        val alerted: Boolean = false
    ) {
        companion object {
            val START = Reading(State.UNKNOWN)
        }
    }

    /**
     * Advance the state machine.
     *
     * @param bpm the current heart rate, or null when the stream drops out — a dropped stream is
     *   not a drift, so the state holds rather than alarming.
     * @param band the prescription's target band, or null when the athlete has no zones at all,
     *   in which case this stays silent forever (rung one runs as a plain timer).
     */
    fun update(previous: Reading, bpm: Int?, band: IntRange?, elapsedSeconds: Int): Reading {
        if (band == null || bpm == null) return previous.copy(alert = null)

        val inBand = bpm in band
        if (inBand) {
            val inFor = previous.secondsInBand + elapsedSeconds
            // Coming back resets the episode, so the NEXT genuine drift can speak again.
            return if (inFor >= RECOVER_SECONDS) {
                Reading(State.IN_ZONE, null, secondsOutOfBand = 0, secondsInBand = inFor, alerted = false)
            } else {
                previous.copy(alert = null, secondsInBand = inFor)
            }
        }

        val outFor = previous.secondsOutOfBand + elapsedSeconds
        val high = bpm > band.last
        val state = if (high) State.DRIFTING_HIGH else State.DRIFTING_LOW
        val shouldAlert = outFor >= DRIFT_SECONDS && !previous.alerted
        return Reading(
            state = state,
            alert = if (shouldAlert) {
                if (high) "Ease off — you've drifted above the zone." else "Pick it up a little."
            } else null,
            secondsOutOfBand = outFor,
            secondsInBand = 0,
            alerted = previous.alerted || shouldAlert
        )
    }

    /** The interval structure a prescription describes, as a sequence the timer can run. */
    fun segmentsFor(prescription: ConditioningPlanner.Prescription): List<Segment> {
        if (prescription.structure != ConditioningPlanner.Structure.INTERVALS) {
            return listOf(Segment("Steady", prescription.minutes * 60, prescription.zone))
        }
        val out = mutableListOf<Segment>()
        if (prescription.warmUpMinutes > 0) out += Segment("Warm-up", prescription.warmUpMinutes * 60, 2)
        val workSeconds = 60
        val restSeconds = 60
        repeat(prescription.intervals) { i ->
            out += Segment("Interval ${i + 1}", workSeconds, prescription.zone)
            if (i < prescription.intervals - 1) out += Segment("Recover", restSeconds, 1)
        }
        if (prescription.coolDownMinutes > 0) out += Segment("Cool-down", prescription.coolDownMinutes * 60, 1)
        return out
    }

    /** One phase of a structured session. */
    data class Segment(val label: String, val seconds: Int, val zone: Int)
}
