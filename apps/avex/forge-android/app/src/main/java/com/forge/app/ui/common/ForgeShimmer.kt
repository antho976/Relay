package com.forge.app.ui.common

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
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
    val scheme = MaterialTheme.colorScheme.onSurface
    val base = scheme.copy(alpha = 0.07f)
    // Remembered, not rebuilt on every draw (P-16): the sweep needs new coordinates each frame, not
    // new colours, and a skeleton of eighteen placeholders allocated eighteen of these per frame.
    val stops = remember(scheme) { listOf(base, scheme.copy(alpha = 0.15f), base) }

    // Reduced motion → a still, faint placeholder (no infinite animation).
    if (ForgeMotion.durationScale <= 0f) return this.background(base)

    // ONE clock for the whole skeleton when a host provides it (P-16). Every placeholder used to
    // own an infinite transition, so Profile drove about 1,080 independent animated-state updates a
    // second while loading and Recap about 960 — doubled again on a 120 Hz display — for a sweep
    // that reads better in phase anyway. A placeholder outside a host keeps its own clock, so this
    // stays a drop-in modifier.
    val shared = LocalShimmerPhase.current
    val phase: State<Float> = if (shared != null) shared else rememberShimmerPhase()
    val progress by phase
    return this.drawBehind {
        val w = size.width
        val sweep = w * 1.5f
        val startX = -sweep + progress * (w + sweep)
        drawRect(
            Brush.linearGradient(
                colors = stops,
                start = Offset(startX, 0f),
                end = Offset(startX + sweep, 0f)
            )
        )
    }
}

/** One sweep, in ms. Shared so a host's clock and a lone placeholder's cannot drift apart. */
private const val SHIMMER_SWEEP_MS = 1100

/** One shimmer clock, for a host or for a placeholder that has no host above it. */
@Composable
private fun rememberShimmerPhase(): State<Float> =
    rememberInfiniteTransition(label = "shimmer").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(durationMillis = SHIMMER_SWEEP_MS, easing = LinearEasing)),
        label = "shimmerSweep"
    )

/**
 * The shimmer phase every placeholder under a [ForgeShimmerHost] reads. Null outside one, which is
 * how [forgeShimmer] stays usable on its own.
 */
val LocalShimmerPhase = androidx.compose.runtime.compositionLocalOf<State<Float>?> { null }

/**
 * Runs ONE shimmer clock for everything inside it (P-16).
 *
 * Wrap a loading skeleton in this and its placeholders sweep together off a single animation
 * instead of one each. Under "Remove animations" no clock is started at all — the placeholders
 * inside render flat, as they already did.
 */
@Composable
fun ForgeShimmerHost(content: @Composable () -> Unit) {
    if (ForgeMotion.durationScale <= 0f) {
        content()
        return
    }
    CompositionLocalProvider(LocalShimmerPhase provides rememberShimmerPhase()) { content() }
}
