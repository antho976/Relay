package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.components.FatigueGauge
import com.forge.app.ui.gym.stats.state.PulseBand
import com.forge.app.ui.gym.stats.state.ReadinessPulse

private val FRESH = Color(0xFF3FB950)
private val BUILDING = Color(0xFFD29922)
private val DELOAD = Color(0xFFE5534B)

/**
 * Tier 4 — readiness/fatigue as a banded gauge with the learned deload threshold as a line, plus the
 * engine's drivers as the "why". The decision-gate visual: am I fresh, building, or due a deload.
 */
@Composable
internal fun ColumnScope.ReadinessContent(pulse: ReadinessPulse?, threshold: Int?, c: StatsColors) {
    if (pulse == null || threshold == null) return
    FatigueGauge(
        score = pulse.score,
        threshold = threshold,
        freshColor = FRESH.copy(alpha = 0.22f),
        buildingColor = BUILDING.copy(alpha = 0.22f),
        deloadColor = DELOAD.copy(alpha = 0.22f),
        markerColor = c.onBg,
        modifier = Modifier.fillMaxWidth().height(34.dp)
    )
    Spacer(Modifier.height(6.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text("Fresh", style = MaterialTheme.typography.labelSmall, color = FRESH)
        Text("Building", style = MaterialTheme.typography.labelSmall, color = BUILDING)
        Text("Deload", style = MaterialTheme.typography.labelSmall, color = DELOAD)
    }
    Spacer(Modifier.height(10.dp))
    val bandLabel = when (pulse.band) {
        PulseBand.FRESH -> "Fresh"
        PulseBand.BUILDING -> "Building"
        PulseBand.DELOAD_SOON -> "Deload soon"
    }
    Text(
        "$bandLabel · fatigue ${pulse.score} of $threshold",
        style = MaterialTheme.typography.bodyMedium,
        color = c.onBg
    )
    if (pulse.drivers.isNotEmpty()) {
        Spacer(Modifier.height(4.dp))
        Text(
            pulse.drivers.joinToString(" · "),
            style = MaterialTheme.typography.bodySmall,
            color = c.muted
        )
    }
}
