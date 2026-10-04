package com.tally.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.runtime.mutableFloatStateOf

/**
 * Motion tokens. [durationScale] mirrors the system animator scale (0 when Remove animations is
 * on); MainActivity keeps it live. Compose applies the platform scale itself, so specs carry their
 * nominal duration and only collapse to a cut when the scale is 0.
 */
object TallyMotion {

    private val scaleState = mutableFloatStateOf(1f)
    var durationScale: Float
        get() = scaleState.floatValue
        set(value) { scaleState.floatValue = value }

    val animationsOff: Boolean get() = durationScale <= 0f

    const val Fast = 150
    const val Standard = 240
    const val Emphasized = 320
    const val Draw = 900

    val Decelerate: Easing = CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f)
    val Accelerate: Easing = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f)
    val StandardEasing: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)
    /** An even ease-out for fills drawing in: moves at once, settles softly. */
    val DrawDecelerate: Easing = CubicBezierEasing(0.39f, 0.575f, 0.565f, 1f)

    private fun nominal(ms: Int) = if (animationsOff) 0 else ms

    fun <T> enter(ms: Int = Standard): FiniteAnimationSpec<T> = tween(nominal(ms), easing = Decelerate)
    fun <T> exit(ms: Int = Standard): FiniteAnimationSpec<T> = tween(nominal(ms), easing = Accelerate)
    fun <T> standard(ms: Int = Standard): FiniteAnimationSpec<T> = tween(nominal(ms), easing = StandardEasing)
    fun <T> draw(ms: Int = Draw): FiniteAnimationSpec<T> = tween(nominal(ms), easing = DrawDecelerate)

    fun <T> snappy(): FiniteAnimationSpec<T> =
        if (animationsOff) snap() else spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)

    fun <T> bouncy(): FiniteAnimationSpec<T> =
        if (animationsOff) snap() else spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMediumLow)
}
