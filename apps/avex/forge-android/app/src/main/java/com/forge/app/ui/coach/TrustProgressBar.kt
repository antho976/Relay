package com.forge.app.ui.coach

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeMotion

/**
 * A segmented earned-trust meter (Tier 6 narrative surface). The continuous fill animates to the
 * current streak's share of [required]; thin dividers in the background colour split it into the
 * milestone segments a type must clear to auto-apply. When [earned] the bar fills fully in the
 * primary colour — the "milestone reached" state — and the fill animation plays the moment it does.
 */
@Composable
fun TrustProgressBar(
    streak: Int,
    required: Int,
    earned: Boolean,
    modifier: Modifier = Modifier
) {
    val fraction = if (required <= 0) 0f else streak.coerceIn(0, required).toFloat() / required
    val animated by animateFloatAsState(
        targetValue = if (earned) 1f else fraction,
        // Routed through ForgeMotion so the system "Remove animations" preference collapses it to a snap.
        animationSpec = ForgeMotion.drawTween(),
        label = "trustFill"
    )
    val accent = if (earned)
        androidx.compose.material3.MaterialTheme.colorScheme.primary
    else
        androidx.compose.material3.MaterialTheme.colorScheme.secondary
    // With a fill riding it the track is a groove and takes the §5 bar-track rung. With NOTHING on
    // it — a change type that has never earned a streak — the track IS the mark, and `outline @0.25`
    // measures ~1.08:1 on near-black, i.e. invisible (`design/FAILURES.md`, *Invisible ghost*). A
    // solid empty bar takes `muted @0.30` instead, low enough not to out-shout a real reading.
    val empty = !earned && streak <= 0
    val track =
        if (empty) androidx.compose.material3.MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.30f)
        else androidx.compose.material3.MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)
    val segmentGap = androidx.compose.material3.MaterialTheme.colorScheme.background

    // Milestone pop: the bar gives a one-shot vertical pulse the moment a type's trust is earned
    // (full). Fires only on the false→true edge — prevEarned seeds to the current value so a bar that's
    // already earned when the screen (re)opens doesn't pop, and re-entering composition (scroll / nav
    // back) doesn't replay it. Reduced-motion collapses it (guarded on durationScale).
    val pulse = remember { Animatable(1f) }
    var prevEarned by remember { mutableStateOf(earned) }
    LaunchedEffect(earned) {
        if (earned && !prevEarned && ForgeMotion.durationScale > 0f) {
            pulse.snapTo(1f)
            pulse.animateTo(1.6f, ForgeMotion.standardTween(ForgeMotion.DurationFast))
            pulse.animateTo(1f, ForgeMotion.standardTween(ForgeMotion.DurationEmphasized))
        }
        prevEarned = earned
    }

    Box(
        modifier
            .height(8.dp)
            .graphicsLayer { scaleY = pulse.value }
            .clip(RoundedCornerShape(4.dp))
            .background(track)
    ) {
        // Continuous, animated fill behind the segment dividers.
        Box(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth(animated)
                .background(accent)
        )
        // Milestone dividers: required − 1 thin gaps painted in the background colour, so the fill
        // reads as discrete segments without a separate per-segment animation.
        if (required > 1) {
            Row(Modifier.fillMaxSize()) {
                repeat(required) { i ->
                    Box(Modifier.weight(1f).fillMaxHeight())
                    if (i < required - 1) {
                        Box(Modifier.width(2.dp).fillMaxHeight().background(segmentGap))
                    }
                }
            }
        }
    }
}
