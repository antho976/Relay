package com.forge.app.ui.common

import android.view.HapticFeedbackConstants
import android.view.View

enum class ForgeHapticType { SET_LOGGED, PR_OR_FINISH, COUNTDOWN_TICK }

/** Dispatches haptic feedback respecting the user's strength preference (#118). */
fun View.forgeHaptic(type: ForgeHapticType, strength: String) {
    if (strength == "off") return
    val constant = when (strength) {
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
    performHapticFeedback(constant)
}
