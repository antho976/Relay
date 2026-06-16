package com.forge.app.ui.common

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import com.forge.app.ui.theme.ForgeMotion

/**
 * A subtle left-to-right shimmer sweep for loading placeholders. Apply to a clipped, sized Box.
 *
 * Honours the system reduced-motion preference via [ForgeMotion.durationScale]: when animations are
 * off (`<= 0f`) it renders a flat, static placeholder tint instead of sweeping — matching how the
 * rest of the app's motion collapses under "Remove animations".
 */
@Composable
fun Modifier.forgeShimmer(): Modifier {
    val base = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.07f)
    val highlight = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.15f)

    // Reduced motion → a still, faint placeholder (no infinite animation).
    if (ForgeMotion.durationScale <= 0f) return this.background(base)

    val transition = rememberInfiniteTransition(label = "shimmer")
    val progress by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(durationMillis = 1100, easing = LinearEasing)),
        label = "shimmerSweep"
    )
    return this.drawBehind {
        val w = size.width
        val sweep = w * 1.5f
        val startX = -sweep + progress * (w + sweep)
        drawRect(
            Brush.linearGradient(
                colors = listOf(base, highlight, base),
                start = Offset(startX, 0f),
                end = Offset(startX + sweep, 0f)
            )
        )
    }
}
