package com.forge.shared.weight

import kotlinx.serialization.Serializable

/**
 * The weight unit as it travels over the wear protocol — a protocol-owned enum so :shared never
 * depends on :app's domain types. :app maps its `WeightUnit` onto this at the publisher boundary.
 */
@Serializable
enum class ProtocolWeightUnit { LB, KG, ST }

/**
 * ONE table for the ± / bezel weight-adjust step, shared by the phone's stepper pills and the
 * watch's rotary adjust so the two surfaces can never drift (extracted from SetInputRow, W1).
 * Steps are in the DISPLAY unit; plate-loaded exercises step by half a plate regardless of unit.
 * Stones step by half a stone so a single-decimal field stays clean; kg 2.5, lb 5.
 */
object WeightSteps {

    const val PLATE_STEP = 0.5
    const val REP_STEP = 1

    fun weightStep(unit: ProtocolWeightUnit, isPlates: Boolean): Double = when {
        isPlates -> PLATE_STEP
        unit == ProtocolWeightUnit.KG -> 2.5
        unit == ProtocolWeightUnit.ST -> 0.5
        else -> 5.0
    }
}
