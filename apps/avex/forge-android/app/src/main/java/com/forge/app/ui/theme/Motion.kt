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
 * Reduced motion: [durationScale] mirrors the system animator duration scale
 * (Settings.Global.ANIMATOR_DURATION_SCALE — 0 when "Remove animations" is on; MainActivity
 * writes it at start and keeps it live). For everything Compose animates it is a GATE, not a
 * multiplier: Compose already applies that same platform scale exactly once through its
 * `MotionDurationScale`, so every spec below carries its NOMINAL duration and only collapses
 * (tweens to 0 ms, springs to a snap) when the scale is 0. Multiplying here as well applied the
 * setting twice — 2x became 4x, 10x became 100x. The scale is used as a factor ONLY by
 * [scaledDuration], for motion Compose does not clock (raw `delay()`s between animations,
 * animated drawables, ValueAnimators). No call site has to check the preference itself.
 */
object ForgeMotion {

    /**
     * The system animator duration scale: 1f = normal · 0f = the user disabled animations.
     * Written by MainActivity at start and whenever the setting changes.
     *
     * Compose snapshot state, not a plain volatile field (M-35). Toggling "Remove animations" while
     * Avex is in the background updated the field and invalidated nothing, so the paths Compose
     * cannot stop by itself — the Academy's seven-second poke rotator, its promoted/chapter split,
     * an animated WebP mid-play in onboarding — kept running until some unrelated recomposition
     * happened along. A read inside composition now subscribes, so the setting takes effect where
     * it is consulted; reads from ordinary code are unchanged.
     */
    var durationScale: Float
        get() = durationScaleState.floatValue
        set(value) { durationScaleState.floatValue = value }

    private val durationScaleState = androidx.compose.runtime.mutableFloatStateOf(1f)

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

    /** A Compose spec duration: the nominal value, or 0 when animations are off. Deliberately NOT
     *  multiplied by [durationScale] — Compose applies the platform scale itself. */
    private fun nominal(durationMs: Int): Int = if (reduceMotion) 0 else durationMs

    /**
     * The duration to hand a Compose animation built at a call site (a literal tween in a Canvas,
     * an `infiniteRepeatable` period, a `delayMillis`): the NOMINAL value, or 0 when animations
     * are off. Compose scales it by the platform animator scale on its own clock, so scaling it
     * here too would square the setting.
     */
    fun nominalDuration(durationMs: Int): Int = nominal(durationMs)

    /**
     * Scale a raw duration that Compose does NOT clock — a kotlinx `delay()` between animations,
     * an animated drawable, a ValueAnimator — by the platform animator scale so it keeps pace
     * with the Compose motion around it; 0 when animations are off. Never feed this into a
     * Compose spec: Compose would scale it a second time (use [nominalDuration] there).
     */
    fun scaledDuration(durationMs: Int): Int = (durationMs * durationScale).roundToInt()

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
    // Nominal durations: Compose applies the animator scale once via MotionDurationScale.
    fun <T> enterTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(nominal(durationMs), easing = Decelerate)

    fun <T> exitTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(nominal(durationMs), easing = Accelerate)

    fun <T> standardTween(durationMs: Int = DurationStandard): FiniteAnimationSpec<T> =
        tween(nominal(durationMs), easing = Standard)

    /** A long, even chart draw-in: starts instantly, decelerates gently to a soft stop. Slower and
     *  smoother than [enterTween] so a line/bar reveal glides in rather than snapping. */
    fun <T> drawTween(durationMs: Int = DurationDraw): FiniteAnimationSpec<T> =
        tween(nominal(durationMs), easing = DrawDecelerate)
}
