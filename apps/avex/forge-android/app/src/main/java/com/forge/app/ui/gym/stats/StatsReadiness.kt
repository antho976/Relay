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
    // Words first, then the number with what it is a number OF. "FATIGUE 0 OF 5" in 9sp mono read
    // like a debug readout; nothing said that 5 is where a deload gets called.
    val (bandLabel, bandColor) = when (pulse.band) {
        PulseBand.FRESH -> "Fresh" to FRESH
        PulseBand.BUILDING -> "Fatigue building" to BUILDING
        PulseBand.DELOAD_SOON -> "Deload soon" to DELOAD
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(bandColor))
        Spacer(Modifier.width(8.dp))
        Text(
            "$bandLabel · fatigue ${pulse.score} of $threshold before a deload",
            style = MaterialTheme.typography.bodySmall,
            color = c.muted
        )
    }
}
