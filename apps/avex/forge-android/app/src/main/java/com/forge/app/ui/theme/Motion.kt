package com.forge.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import kotlin.math.roundToInt

/**
 * Central motion tokens. Every animated surface pulls its duration / easing / spring from
 * here so motion reads as one coherent system instead of a dozen independently-tuned magic
 * numbers. Easings follow the Material 3 "emphasized" family — incoming content decelerates
 * into place, outgoing content accelerates away.
 *
 * Reduced motion: [durationScale] mirrors the system "Remove animations" preference
 * (Settings.Global.ANIMATOR_DURATION_SCALE, set once in MainActivity). Because every spec below
 * is derived through [scaled] / [reduceMotion], honoring that preference is automatic — tweens
 * collapse to 0 ms and springs snap instantly. No call site has to check it.
 */
object ForgeMotion {

    /** 1f = normal speed · 0f = the user disabled animations. Set once at app start. */
    @Volatile var durationScale: Float = 1f

    private val reduceMotion: Boolean get() = durationScale <= 0f

    /**
     * True when the user has turned animations off system-wide.
     *
     * Every spec below already collapses on its own, so nothing that merely *plays* an animation
     * needs this. It exists for the one thing a duration cannot express: motion that REPEATS on a
     * timer with no input, like the Academy's rotating poke. A zero-length tween would turn that
     * into a silent hard cut every few seconds rather than into stillness, so the caller has to
     * stop the timer itself and offer the manual control instead.
     */
    val animationsOff: Boolean get() = reduceMotion
    private fun scaled(durationMs: Int): Int = (durationMs * durationScale).roundToInt()

    /** Scale a one-off raw duration (for animations not built from the helpers below — e.g. a
     *  literal tween in a Canvas) by the reduced-motion preference; 0 when animations are off. */
    fun scaledDuration(durationMs: Int): Int = scaled(durationMs)

    // ── Durations (ms) ──────────────────────────────────────────────────────────
    const val DurationFast = 150          // micro: press, tiny fades
    const val DurationStandard = 240      // default enter/exit, tab content
    const val DurationEmphasized = 320    // page-level pushes
    const val DurationDraw = 900          // chart draw-in reveals (sparklines, bars, lines)
    const val DurationCelebration = 2200  // confetti & one-shot flourishes

    // ── Easings ─────────────────────────────────────────────────────────────────
    /** Incoming content settles into place. */
    val Decelerate: Easing = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f)
    /**
     * A gentle ease-out (~easeOutSine): starts moving immediately at a moderate pace and
     * decelerates evenly to a soft stop. Unlike [Decelerate] — which front-loads almost all the
     * motion into the first few percent so a reveal reads as a sharp "snap" — this draws smoothly
     * across the whole duration. Used for chart draw-ins (sparklines, bars, line reveals).
     */
    val DrawDecelerate: Easing = CubicBezierEasing(0.39f, 0.575f, 0.565f, 1f)
    /** Outgoing content speeds up as it leaves. */
    val Accelerate: Easing = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f)
    /** Symmetric easing for in-place changes (no enter/exit direction). */
    val Standard: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    // ── Springs ───────────────────────────────────────────────────────────────--
    /** Lands with a subtle bounce — set-logged "thunk", PR pop, bubble pop-in. */
    fun <T> bouncy(): FiniteAnimationSpec<T> =
        if (reduceMotion) snap()
        else spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMediumLow)

    /** Quick, no overshoot — pill slide, content size, value follows. */
    fun <T> snappy(): FiniteAnimationSpec<T> =
        if (reduceMotion) snap()
        else spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)

    // ── Tween helpers ─────────────────────────────────────────────────────────--
    fun <T> enterTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(scaled(durationMs), easing = Decelerate)

    fun <T> exitTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(scaled(durationMs), easing = Accelerate)

    fun <T> standardTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(scaled(durationMs), easing = Standard)

    /** A long, even chart draw-in: starts instantly, decelerates gently to a soft stop. Slower and
     *  smoother than [enterTween] so a line/bar reveal glides in rather than snapping. */
    fun <T> drawTween(durationMs: Int = DurationDraw): FiniteAnimationSpec<T> =
        tween(scaled(durationMs), easing = DrawDecelerate)
}
