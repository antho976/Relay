package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.forgeShimmer

/**
 * Shimmer placeholder shown while [ProfileViewModel] loads the "You" hub. It mirrors the real
 * gamification-off layout — the full-bleed cover, the bodyweight figure near the top, the open 2×2
 * figure grid + volume curve, a stack of goal progress lines and the full-bleed gallery filmstrip —
 * so the populated content swaps in without a jump.
 */
@Composable
internal fun ProfileSkeleton(modifier: Modifier = Modifier, topInset: Dp = 0.dp) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        // Full-bleed identity cover banner (mirrors the real profile-photo header, incl. the area
        // behind the status bar so the load→loaded swap doesn't jump).
        Box(Modifier.fillMaxWidth().height(240.dp + topInset).forgeShimmer())

        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp)) {
            Spacer(Modifier.height(28.dp))

            // ALL-TIME label + open 2×2 figure grid.
            ShimmerBar(96.dp, 10.dp)
            Spacer(Modifier.height(14.dp))
            repeat(2) { row ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    repeat(2) {
                        Column(Modifier.weight(1f)) {
                            ShimmerBar(72.dp, 26.dp)
                            Spacer(Modifier.height(6.dp))
                            ShimmerBar(44.dp, 8.dp)
                        }
                    }
                }
                if (row == 0) Spacer(Modifier.height(18.dp))
            }
            Spacer(Modifier.height(28.dp))

            // BODYWEIGHT label + big figure + unit (sits between the tallies and the curve).
            ShimmerBar(96.dp, 10.dp)
            Spacer(Modifier.height(14.dp))
            ShimmerBar(90.dp, 34.dp)
            Spacer(Modifier.height(6.dp))
            ShimmerBar(56.dp, 8.dp)
            Spacer(Modifier.height(28.dp))

            // Lifetime-volume curve.
            ShimmerBlock(Modifier.fillMaxWidth().height(72.dp))
            Spacer(Modifier.height(28.dp))

            ShimmerBar(96.dp, 10.dp)
            Spacer(Modifier.height(14.dp))
        }

        // GALLERY filmstrip — full-bleed like the real strip.
        Row(
            Modifier.fillMaxWidth().padding(start = 20.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            repeat(3) {
                Box(Modifier.width(132.dp).height(176.dp).clip(RoundedCornerShape(16.dp)).forgeShimmer())
            }
        }
        Spacer(Modifier.height(40.dp))
    }
}

/** A rounded block-shaped shimmer (charts, photo cells). */
@Composable
private fun ShimmerBlock(modifier: Modifier) {
    Box(modifier.clip(RoundedCornerShape(12.dp)).forgeShimmer())
}

/** A single rounded shimmer bar at a fixed size (labels and figures). */
@Composable
private fun ShimmerBar(width: Dp, height: Dp) {
    Box(Modifier.width(width).height(height).clip(RoundedCornerShape(50)).forgeShimmer())
}
