package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.state.PulseBand
import com.forge.app.ui.gym.stats.state.ReadinessPulse
import com.forge.app.ui.theme.ForgeError
import com.forge.app.ui.theme.ForgeSuccess
import com.forge.app.ui.theme.ForgeWarning

// §5 reserved true-state colors — never off-palette literals.
private val FRESH = ForgeSuccess
private val BUILDING = ForgeWarning
private val DELOAD = ForgeError

/**
 * Readiness demoted to a one-line STATUS under the Stats hero (2026-07-01 fusion) — a band-colored
 * dot plus "BUILDING · FATIGUE 34 OF 60". It's a state, not a chart; the full why lives with the
 * coach. The old banded FatigueGauge card is gone.
 */
@Composable
internal fun ReadinessLine(pulse: ReadinessPulse?, threshold: Int?, c: StatsColors) {
    if (pulse == null || threshold == null) return
    val (bandLabel, bandColor) = when (pulse.band) {
        PulseBand.FRESH -> "FRESH" to FRESH
        PulseBand.BUILDING -> "BUILDING" to BUILDING
        PulseBand.DELOAD_SOON -> "DELOAD SOON" to DELOAD
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(7.dp).clip(CircleShape).background(bandColor))
        Spacer(Modifier.width(7.dp))
        Text(
            "$bandLabel · FATIGUE ${pulse.score} OF $threshold",
            style = MaterialTheme.typography.labelSmall,
            color = c.muted,
            fontSize = 9.sp,
            letterSpacing = 1.sp
        )
    }
}
