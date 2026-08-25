package com.forge.shared.weight

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The weight unit as it travels over the wear protocol — a protocol-owned enum so :shared never
 * depends on :app's domain types. :app maps its `WeightUnit` onto this at the publisher boundary.
 */
@Serializable
enum class ProtocolWeightUnit {
    // Explicit wire strings: the phone APK and the watch APK are minified by two
    // independent R8 runs, and an enum travels as its constant name. A @SerialName is a
    // source-level literal neither run can touch.
    @SerialName("LB") LB,
    @SerialName("KG") KG,
    @SerialName("ST") ST
}

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
