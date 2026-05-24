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
import kotlin.random.Random

/**
 * Full-area confetti burst animation. Pass [modifier] to control the drawing area;
 * call sites typically use `Modifier.fillMaxSize()` or `Modifier.matchParentSize()`.
 *
 * Non-blocking: Canvas has no pointer-input handler so touches pass through.
 * Calls [onComplete] once the animation finishes (~2.5 s).
 */
@Composable
fun ConfettiOverlay(
    modifier: Modifier = Modifier,
    durationMs: Int = 2500,
    onComplete: () -> Unit = {}
) {
    val density = LocalDensity.current.density
    val particles = remember { buildParticles(42, density) }
    val progress = remember { Animatable(0f) }

    LaunchedEffect(Unit) {
        progress.animateTo(
            targetValue = 1f,
            animationSpec = tween(durationMillis = durationMs, easing = LinearEasing)
        )
        onComplete()
    }

    val p = progress.value

    Canvas(modifier = modifier) {
        particles.forEach { q ->
            val x = (q.nx + q.dx * p) * size.width
            val y = (q.ny + q.vy * p) * size.height
            val alpha = if (p > 0.7f) ((1f - p) / 0.3f).coerceIn(0f, 1f) else 1f

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
    val nx: Float,  // normalized start x  (0..1)
    val ny: Float,  // normalized start y  (-0.25..0, above the visible area)
    val dx: Float,  // x drift across the full animation (-0.2..0.2)
    val vy: Float,  // y travel expressed as multiples of the canvas height (1.3..2.0)
    val w: Float,   // width  in px
    val h: Float,   // height in px
    val r0: Float,  // initial rotation  (degrees)
    val rs: Float,  // rotation over the full animation (degrees, ±300)
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

private fun buildParticles(count: Int, density: Float): List<Particle> {
    val rng = Random(seed = 42)
    return List(count) { i ->
        Particle(
            nx = rng.nextFloat(),
            ny = -rng.nextFloat() * 0.25f,
            dx = (rng.nextFloat() - 0.5f) * 0.25f,
            vy = 1.3f + rng.nextFloat() * 0.7f,
            w = (8f + rng.nextFloat() * 12f) * density,
            h = (4f + rng.nextFloat() * 8f) * density,
            r0 = rng.nextFloat() * 360f,
            rs = (rng.nextFloat() - 0.5f) * 600f,
            color = palette[i % palette.size]
        )
    }
}
