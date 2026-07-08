package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.components.BandedBar
import com.forge.app.ui.gym.stats.state.E1rmLift

// Bodyweight-relative strength tiers — recovered verbatim from the old StrengthStandardsCard so the
// "where you stand" read can't drift. Generic ×bodyweight bands (a simplification: real standards
// differ per lift, but honest for DB/machine work with no published tables). Sex-aware.
private val TIER_CUTOFFS_MALE = listOf(0.4, 0.7, 1.1, 1.5)
private val TIER_CUTOFFS_FEMALE = listOf(0.3, 0.5, 0.8, 1.1)
private val TIER_FULL = listOf("Untrained", "Novice", "Intermediate", "Advanced", "Elite")
private const val MIN_SESSIONS_FOR_TIER = 3

private fun tierCutoffs(sex: String) = if (sex == "female") TIER_CUTOFFS_FEMALE else TIER_CUTOFFS_MALE
private fun tierIndex(ratio: Double, sex: String): Int {
    tierCutoffs(sex).forEachIndexed { i, cut -> if (ratio < cut) return i }
    return tierCutoffs(sex).size
}

// The bodyweight chart moved to the Profile's BODYWEIGHT section 2026-07-01 (your body lives on
// your profile, not in Stats). This file keeps only the strength-standards read, which the
// STRENGTH tab renders.

/**
 * Tier 5b — relative strength: each main lift's e1RM ÷ bodyweight as a marker sitting on banded tier
 * zones (Untrained → Elite). "Where do I rank" at a glance. A tier only locks once a lift has a few
 * sessions behind its e1RM, so one fluke set can't read as Advanced.
 */
@Composable
internal fun ColumnScope.StrengthStandardsContent(
    lifts: List<E1rmLift>,
    bodyweightLb: Double?,
    sex: String,
    c: StatsColors
) {
    val bw = bodyweightLb ?: 0.0
    if (bw <= 0.0) {
        Text(
            "Log your bodyweight to see where you stand.",
            style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic
        )
        return
    }
    val rated = lifts.filter { it.currentE1rm > 0 && it.history.size >= MIN_SESSIONS_FOR_TIER }.take(5)
    if (rated.isEmpty()) {
        Text(
            "Still calibrating. A few more sessions of your main lifts and your tier locks in.",
            style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic
        )
        return
    }
    rated.forEach { lift -> StrengthStandardRow(lift, bw, sex, c) }
}

@Composable
private fun StrengthStandardRow(lift: E1rmLift, bw: Double, sex: String, c: StatsColors) {
    val ratio = lift.currentE1rm / bw
    val idx = tierIndex(ratio, sex)
    val cutoffs = tierCutoffs(sex)
    val maxRatio = (cutoffs.last() * 1.3).toFloat()
    val zoneEdges = cutoffs.map { (it / maxRatio).toFloat() }
    val zoneColors = (0..4).map { lerp(c.outline.copy(alpha = 0.30f), c.accent, it / 4f) }

    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(lift.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
            Text(
                "%.2f× BW · %s".format(ratio, TIER_FULL[idx]),
                style = MaterialTheme.typography.labelMedium, color = c.onBg, fontWeight = FontWeight.SemiBold
            )
        }
        Spacer(Modifier.height(6.dp))
        BandedBar(
            markerFraction = (ratio / maxRatio).toFloat(),
            zoneEdges = zoneEdges,
            zoneColors = zoneColors,
            markerColor = c.onBg,
            modifier = Modifier.fillMaxWidth().height(12.dp)
        )
    }
}
