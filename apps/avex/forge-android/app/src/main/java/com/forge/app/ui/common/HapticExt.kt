package com.forge.app.ui.common

import android.view.HapticFeedbackConstants
import android.view.View

enum class ForgeHapticType { SET_LOGGED, PR_OR_FINISH, COUNTDOWN_TICK }

/** Dispatches haptic feedback respecting the user's strength preference (#118). */
fun View.forgeHaptic(type: ForgeHapticType, strength: String) {
    val constant = forgeHapticConstant(type, strength) ?: return
    performHapticFeedback(constant)
}

/**
 * The [HapticFeedbackConstants] value a [type] plays at [strength], or null when Feedback strength
 * is Off. Split from the [View] extension so the "Off means nothing plays" contract is checkable
 * on the JVM; every event haptic in the app goes through here, never straight to the platform.
 */
fun forgeHapticConstant(type: ForgeHapticType, strength: String): Int? {
    if (strength == "off") return null
    return when (strength) {
        "light" -> HapticFeedbackConstants.TEXT_HANDLE_MOVE
        "medium" -> when (type) {
            ForgeHapticType.SET_LOGGED, ForgeHapticType.COUNTDOWN_TICK -> HapticFeedbackConstants.CLOCK_TICK
            ForgeHapticType.PR_OR_FINISH -> HapticFeedbackConstants.VIRTUAL_KEY
        }
        else -> when (type) { // "strong" (default)
            ForgeHapticType.SET_LOGGED, ForgeHapticType.COUNTDOWN_TICK -> HapticFeedbackConstants.CLOCK_TICK
            ForgeHapticType.PR_OR_FINISH -> HapticFeedbackConstants.LONG_PRESS
        }
    }
}
