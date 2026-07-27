package com.forge.app.data.health

import androidx.health.connect.client.records.ExerciseSessionRecord
import com.forge.app.domain.cardio.CardioType

/**
 * Maps Avex activities to Health Connect [ExerciseSessionRecord] exercise types (W0 write-back).
 * Cardio writes its REAL type so Samsung Health / Google Fit label the session correctly; an
 * unknown or custom activity falls back to the generic workout type rather than lying.
 */
object HcExerciseTypes {

    /** Gym sessions are always strength training. */
    const val STRENGTH = ExerciseSessionRecord.EXERCISE_TYPE_STRENGTH_TRAINING

    /** The HC exercise type for a cardio entry's stored type code (built-in or `custom_`). */
    fun forCardioCode(code: String): Int = when (CardioType.entries.firstOrNull { it.code == code }) {
        CardioType.RUN -> ExerciseSessionRecord.EXERCISE_TYPE_RUNNING
        CardioType.WALK -> ExerciseSessionRecord.EXERCISE_TYPE_WALKING
        CardioType.TREADMILL -> ExerciseSessionRecord.EXERCISE_TYPE_RUNNING_TREADMILL
        CardioType.CYCLE -> ExerciseSessionRecord.EXERCISE_TYPE_BIKING
        CardioType.SWIM -> ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_POOL
        CardioType.ROW -> ExerciseSessionRecord.EXERCISE_TYPE_ROWING_MACHINE
        CardioType.HIKE -> ExerciseSessionRecord.EXERCISE_TYPE_HIKING
        CardioType.ELLIPTICAL -> ExerciseSessionRecord.EXERCISE_TYPE_ELLIPTICAL
        CardioType.HIIT -> ExerciseSessionRecord.EXERCISE_TYPE_HIGH_INTENSITY_INTERVAL_TRAINING
        CardioType.YOGA -> ExerciseSessionRecord.EXERCISE_TYPE_YOGA
        else -> ExerciseSessionRecord.EXERCISE_TYPE_OTHER_WORKOUT
    }

    /**
     * The Avex cardio code for a watch session's HC exercise type (W5 import prefill) — the reverse
     * of [forCardioCode], widened to the near-miss HC types a watch actually records (open-water
     * swim, road vs stationary bike). Anything unrecognised imports honestly as Other.
     */
    fun toCardioCode(hcType: Int): String = when (hcType) {
        ExerciseSessionRecord.EXERCISE_TYPE_RUNNING -> CardioType.RUN.code
        ExerciseSessionRecord.EXERCISE_TYPE_RUNNING_TREADMILL -> CardioType.TREADMILL.code
        ExerciseSessionRecord.EXERCISE_TYPE_WALKING -> CardioType.WALK.code
        ExerciseSessionRecord.EXERCISE_TYPE_BIKING,
        ExerciseSessionRecord.EXERCISE_TYPE_BIKING_STATIONARY -> CardioType.CYCLE.code
        ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_POOL,
        ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_OPEN_WATER -> CardioType.SWIM.code
        ExerciseSessionRecord.EXERCISE_TYPE_ROWING,
        ExerciseSessionRecord.EXERCISE_TYPE_ROWING_MACHINE -> CardioType.ROW.code
        ExerciseSessionRecord.EXERCISE_TYPE_HIKING -> CardioType.HIKE.code
        ExerciseSessionRecord.EXERCISE_TYPE_ELLIPTICAL -> CardioType.ELLIPTICAL.code
        ExerciseSessionRecord.EXERCISE_TYPE_HIGH_INTENSITY_INTERVAL_TRAINING -> CardioType.HIIT.code
        ExerciseSessionRecord.EXERCISE_TYPE_YOGA -> CardioType.YOGA.code
        else -> CardioType.OTHER.code
    }
}
