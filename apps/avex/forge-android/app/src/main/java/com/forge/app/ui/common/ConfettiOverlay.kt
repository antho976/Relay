package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.platform.LocalDensity
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.sin
import kotlin.random.Random

/**
 * Full-area confetti burst animation. Pass [modifier] to control the drawing area;
 * call sites typically use `Modifier.fillMaxSize()` or `Modifier.matchParentSize()`.
 *
 * Non-blocking: Canvas has no pointer-input handler so touches pass through.
 * Calls [onComplete] once the animation finishes.
 *
 * Particles accelerate downward (gravity) and flutter sideways (a sine sway) instead of
 * drifting at a constant velocity, the layout is re-seeded on every appearance so the burst
 * never reads as canned, and they start at/just-above the top edge so the opening beat is
 * populated immediately. [progress] is read inside the Canvas draw lambda so frames redraw
 * without recomposing.
 */
@Composable
fun ConfettiOverlay(
    modifier: Modifier = Modifier,
    durationMs: Int = ForgeMotion.DurationCelebration,
    onComplete: () -> Unit = {}
) {
    val density = LocalDensity.current.density
    // New random layout each time the overlay appears — never the same burst twice.
    val particles = remember { buildParticles(54, density, seed = Random.nextLong()) }
    val progress = remember { Animatable(0f) }

    LaunchedEffect(Unit) {
        progress.animateTo(
            targetValue = 1f,
            animationSpec = tween(durationMillis = durationMs, easing = LinearEasing)
        )
        onComplete()
    }

    Canvas(modifier = modifier) {
        val p = progress.value           // draw-phase read → redraw without recompose
        val fall = p * p                 // gravity: vertical travel accelerates over time
        particles.forEach { q ->
            val sway = q.sway * sin(p * q.swayFreq + q.phase)
            val x = (q.nx + q.dx * p + sway) * size.width
            val y = (q.ny + q.vy * fall) * size.height
            val alpha = if (p > 0.75f) ((1f - p) / 0.25f).coerceIn(0f, 1f) else 1f

            withTransform({
                translate(x, y)
                rotate(q.r0 + q.rs * p)
            }) {
                drawRect(
                    color = q.color.copy(alpha = alpha),
                    topLeft = Offset(-q.w / 2f, -q.h / 2f),
                    size = Size(q.w, q.h)
                )
            }
        }
    }
}

private data class Particle(
    val nx: Float,        // normalized start x  (0..1)
    val ny: Float,        // normalized start y  (-0.08..0.05 — at/just above the top edge)
    val dx: Float,        // steady x drift across the animation
    val vy: Float,        // y travel expressed as multiples of the canvas height
    val sway: Float,      // horizontal flutter amplitude (fraction of width)
    val swayFreq: Float,  // flutter frequency
    val phase: Float,     // flutter phase offset so particles don't sway in unison
    val w: Float,         // width  in px
    val h: Float,         // height in px
    val r0: Float,        // initial rotation  (degrees)
    val rs: Float,        // rotation over the full animation (degrees, ±300)
    val color: Color
)

private val palette = listOf(
    Color(0xFFFFC107),
    Color(0xFFE91E63),
    Color(0xFF00BCD4),
    Color(0xFF4CAF50),
    Color(0xFFFF5722),
    Color(0xFF9C27B0),
    Color(0xFF2196F3),
    Color(0xFFFFEB3B),
)

private fun buildParticles(count: Int, density: Float, seed: Long): List<Particle> {
    val rng = Random(seed)
    return List(count) { i ->
        Particle(
            nx = rng.nextFloat(),
            // Start at or just above the top edge so confetti is on screen from the first frame.
            ny = -0.08f + rng.nextFloat() * 0.13f,
            dx = (rng.nextFloat() - 0.5f) * 0.15f,
            vy = 1.2f + rng.nextFloat() * 0.8f,
            sway = 0.02f + rng.nextFloat() * 0.05f,
            swayFreq = 6f + rng.nextFloat() * 6f,
            phase = rng.nextFloat() * 6.2832f,
            w = (8f + rng.nextFloat() * 12f) * density,
            h = (4f + rng.nextFloat() * 8f) * density,
            r0 = rng.nextFloat() * 360f,
            rs = (rng.nextFloat() - 0.5f) * 600f,
            color = palette[i % palette.size]
        )
    }
}
