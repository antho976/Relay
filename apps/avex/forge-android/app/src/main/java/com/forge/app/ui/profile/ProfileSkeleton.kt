package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.forgeShimmer

/**
 * Shimmer placeholder shown while [ProfileViewModel] loads the "You" hub. Matches the real screen's
 * rhythm (label · name · rank line · hero emblem · section blocks) so the populated content swaps in
 * without a layout jump — and replaces the old empty-default flash (owner finding #8).
 */
@Composable
internal fun ProfileSkeleton(modifier: Modifier = Modifier) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(8.dp))
        ShimmerBar(120.dp, 10.dp)              // "ATHLETE PROFILE" label
        Spacer(Modifier.height(12.dp))
        ShimmerBar(210.dp, 36.dp)              // name
        Spacer(Modifier.height(10.dp))
        ShimmerBar(170.dp, 14.dp)              // "Rank … — …" line
        Spacer(Modifier.height(28.dp))

        // Hero rank emblem + the slim six-dot rail beneath it.
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Box(Modifier.size(104.dp).clip(CircleShape).forgeShimmer())
        }
        Spacer(Modifier.height(14.dp))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            ShimmerBar(190.dp, 8.dp)
        }
        Spacer(Modifier.height(32.dp))

        // A few section blocks (ledger · standing · signature · trophy case).
        repeat(4) {
            ShimmerBar(96.dp, 10.dp)           // section label
            Spacer(Modifier.height(12.dp))
            Box(Modifier.fillMaxWidth().height(52.dp).clip(RoundedCornerShape(12.dp)).forgeShimmer())
            Spacer(Modifier.height(28.dp))
        }
    }
}

/** A single rounded shimmer bar at a fixed size. */
@Composable
private fun ShimmerBar(width: Dp, height: Dp) {
    Box(Modifier.width(width).height(height).clip(RoundedCornerShape(50)).forgeShimmer())
}
