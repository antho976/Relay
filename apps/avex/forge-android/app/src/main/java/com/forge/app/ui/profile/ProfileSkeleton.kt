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
 * gamification-off layout — an identity card, a 2-up stat-tile grid, then a couple of section cards —
 * so the populated content swaps in without a jump. (The old skeleton drew a centred rank emblem +
 * rank line that never appear when gamification is off, which read as a stray grey blob.)
 */
@Composable
internal fun ProfileSkeleton(modifier: Modifier = Modifier) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(8.dp))

        // Identity hero card.
        ShimmerBlock(Modifier.fillMaxWidth().height(100.dp))
        Spacer(Modifier.height(28.dp))

        // ALL-TIME label + a 2×2 tile grid.
        ShimmerBar(96.dp, 10.dp)
        Spacer(Modifier.height(12.dp))
        repeat(2) { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                ShimmerBlock(Modifier.weight(1f).height(104.dp))
                ShimmerBlock(Modifier.weight(1f).height(104.dp))
            }
            if (row == 0) Spacer(Modifier.height(12.dp))
        }
        Spacer(Modifier.height(28.dp))

        // A couple of section cards (signature · goals · gallery).
        repeat(2) {
            ShimmerBar(96.dp, 10.dp)
            Spacer(Modifier.height(12.dp))
            ShimmerBlock(Modifier.fillMaxWidth().height(72.dp))
            Spacer(Modifier.height(28.dp))
        }
    }
}

/** A rounded card-shaped shimmer block. */
@Composable
private fun ShimmerBlock(modifier: Modifier) {
    Box(modifier.clip(RoundedCornerShape(16.dp)).forgeShimmer())
}

/** A single rounded shimmer bar at a fixed size (for labels). */
@Composable
private fun ShimmerBar(width: Dp, height: Dp) {
    Box(Modifier.width(width).height(height).clip(RoundedCornerShape(50)).forgeShimmer())
}
