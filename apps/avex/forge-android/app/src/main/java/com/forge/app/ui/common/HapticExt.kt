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
        // "strong" (default). The set log and the countdown were CLOCK_TICK here as on medium, one
        // of the faintest constants there is, so Strong felt exactly like Medium in a loud gym.
        else -> when (type) {
            ForgeHapticType.SET_LOGGED, ForgeHapticType.COUNTDOWN_TICK -> HapticFeedbackConstants.VIRTUAL_KEY
            ForgeHapticType.PR_OR_FINISH -> HapticFeedbackConstants.LONG_PRESS
        }
    }
}

/**
 * The press acknowledgement a long-press gesture plays, or nothing when Feedback strength is Off.
 * Not an event haptic (§9 keeps those to set, PR and timer); this is the platform's "the hold
 * registered" tick, which Off has to silence like everything else.
 */
fun androidx.compose.ui.hapticfeedback.HapticFeedback.forgeLongPress(strength: String) {
    if (strength == "off") return
    performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
}
