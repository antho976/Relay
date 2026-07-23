package com.forge.wear.data

import android.content.Context
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager

/**
 * The wrist's three haptic moments (DESIGN.md §16) — timer-done strong buzz, set-logged tick,
 * PR double-tick. Nothing else vibrates.
 */
class WristHaptics(context: Context) {

    private val vibrator: Vibrator =
        (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as VibratorManager).defaultVibrator

    /** The one strong buzz: rest is over. */
    fun timerDone() {
        vibrate(VibrationEffect.createWaveform(longArrayOf(0, 350, 120, 350), -1))
    }

    /** Short confirmation tick: set logged. */
    fun setLogged() {
        vibrate(VibrationEffect.createOneShot(40, VibrationEffect.DEFAULT_AMPLITUDE))
    }

    /** Double-tick: PR. */
    fun pr() {
        vibrate(VibrationEffect.createWaveform(longArrayOf(0, 60, 80, 60), -1))
    }

    private fun vibrate(effect: VibrationEffect) {
        runCatching { vibrator.vibrate(effect) }
    }
}
