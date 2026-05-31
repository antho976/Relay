package com.forge.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween

/**
 * Central motion tokens. Every animated surface pulls its duration / easing / spring from
 * here so motion reads as one coherent system instead of a dozen independently-tuned magic
 * numbers. Easings follow the Material 3 "emphasized" family — incoming content decelerates
 * into place, outgoing content accelerates away.
 */
object ForgeMotion {

    // ── Durations (ms) ──────────────────────────────────────────────────────────
    const val DurationFast = 150          // micro: press, tiny fades
    const val DurationStandard = 240      // default enter/exit, tab content
    const val DurationEmphasized = 320    // page-level pushes
    const val DurationCelebration = 2200  // confetti & one-shot flourishes

    // ── Easings ─────────────────────────────────────────────────────────────────
    /** Incoming content settles into place. */
    val Decelerate: Easing = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f)
    /** Outgoing content speeds up as it leaves. */
    val Accelerate: Easing = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f)
    /** Symmetric easing for in-place changes (no enter/exit direction). */
    val Standard: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    // ── Springs ───────────────────────────────────────────────────────────────--
    /** Lands with a subtle bounce — set-logged "thunk", PR pop, bubble pop-in. */
    fun <T> bouncy(): SpringSpec<T> = spring(
        dampingRatio = 0.55f,
        stiffness = Spring.StiffnessMediumLow
    )

    /** Quick, no overshoot — pill slide, content size, value follows. */
    fun <T> snappy(): SpringSpec<T> = spring(
        dampingRatio = Spring.DampingRatioNoBouncy,
        stiffness = Spring.StiffnessMedium
    )

    // ── Tween helpers ─────────────────────────────────────────────────────────--
    fun <T> enterTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(durationMs, easing = Decelerate)

    fun <T> exitTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(durationMs, easing = Accelerate)

    fun <T> standardTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(durationMs, easing = Standard)
}
