package com.forge.app.ui.profile

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.Features
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.roundToInt

/**
 * Shared section scaffold (label + optional accent action + body) used only by the gamification
 * [StandingSection] now — the everyday stat sections sit openly on the page via the primitives in
 * [ProfileTiles]. Kept so re-enabling [Features.SHOW_GAMIFICATION] needs no rework.
 */
@Composable
internal fun ProfileBlock(
    label: String,
    muted: Color,
    accent: Color,
    outline: Color,
    action: String? = null,
    onAction: (() -> Unit)? = null,
    compact: Boolean = false,
    content: @Composable ColumnScope.() -> Unit
) {
    // Air + the mono header separate sections (§1) — no hairline strip.
    Spacer(Modifier.height(if (compact) 22.dp else 28.dp))
    Row(
        Modifier.fillMaxWidth().then(if (onAction != null) Modifier.bounceClick { onAction() } else Modifier),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
        if (action != null) Text(action, style = MaterialTheme.typography.labelSmall, color = accent)
    }
    Spacer(Modifier.height(if (compact) 8.dp else 12.dp))
    Column(content = content)
}

/**
 * ALL-TIME — lifetime tallies as big open serif figures (two-up, no boxes), the count figures each
 * carrying a small ↑/↓ "vs last week" badge. The cumulative lifted-volume curve is a separate
 * [LifetimeVolumeGraph] so the bodyweight figure can sit between the tallies and the curve.
 */
@Composable
internal fun AllTimeSection(
    sessions: Int,
    volumeLb: Double,
    prs: Int,
    sets: Int,
    xp: Long,
    /** Signed this-week-minus-last-week deltas for the workouts / sets / PRs figures. */
    workoutsDelta: Int,
    setsDelta: Int,
    prsDelta: Int,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    SectionHeader("ALL-TIME", muted)
    // At zero sessions the grid still renders — honest zeros ARE the empty state (§12), the
    // figures fill in from the first logged set.
    val specs = buildList {
        add(StatCellSpec("$sessions", "WORKOUTS", delta = workoutsDelta))
        add(StatCellSpec(formatVolume(volumeLb, weightUnit), "LIFETIME ${unitLabel(weightUnit).uppercase()}"))
        add(StatCellSpec("$prs", "PRs", delta = prsDelta))
        add(StatCellSpec(formatCount(sets), "SETS", delta = setsDelta))
        if (Features.SHOW_GAMIFICATION) add(StatCellSpec("$xp", "XP"))
    }
    StatCellGrid(specs, accent, muted, onBg)
}

/**
 * The cumulative lifetime-volume curve (session by session), drawn full-width as its own quiet chart.
 * Split from [AllTimeSection] so the bodyweight figure can sit above it and below the ALL-TIME
 * tallies. Draws nothing under two logged sessions.
 */
@Composable
internal fun LifetimeVolumeGraph(
    volumeSeriesLb: List<Double>,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    if (volumeSeriesLb.size < 2) return
    val weightUnit = LocalForgeSettings.current.weightUnit
    val series = volumeSeriesLb.map { toDisplayWeight(it, weightUnit) }
    Column(modifier) {
        ProfileSparkline(series, accent, Modifier.fillMaxWidth().height(72.dp))
        Spacer(Modifier.height(8.dp))
        ChartCaption(accent, "LIFETIME VOLUME · SESSION BY SESSION", muted)
    }
}

/** "1,240" → "1.2k"; small counts stay exact. Keeps a big lifetime sets figure from overflowing its column. */
private fun formatCount(n: Int): String = when {
    n >= 10_000 -> "${(n / 1000.0).roundToInt()}k"
    n >= 1_000 -> "${"%.1f".format(n / 1000.0)}k"
    else -> "$n"
}

/** STANDING · ESTIMATED, 90 DAYS — offline percentile estimate (never a live leaderboard). */
@Composable
internal fun StandingSection(
    standings: List<StandingMetric>,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    if (standings.isEmpty()) return
    ProfileBlock("STANDING · ESTIMATED, 90 DAYS", muted, accent, outline) {
        var play by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) { play = true }
        standings.forEachIndexed { i, m ->
            Row(
                Modifier.fillMaxWidth().padding(vertical = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Column(modifier = Modifier.width(96.dp)) {
                    Text(m.label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                    Text(m.valueText, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp)
                }
                val frac = ((100 - m.topPercent) / 100f).coerceIn(0f, 1f)
                val w by animateFloatAsState(
                    targetValue = if (play) frac else 0f,
                    animationSpec = tween(
                        durationMillis = ForgeMotion.scaledDuration(ForgeMotion.DurationEmphasized),
                        delayMillis = ForgeMotion.scaledDuration(i * 90),
                        easing = ForgeMotion.Standard
                    ),
                    label = "standing-bar"
                )
                Box(Modifier.weight(1f).height(2.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.25f))) {
                    Box(Modifier.fillMaxWidth(w).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent))
                }
                Text(
                    "TOP ${m.topPercent}%", style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp,
                    modifier = Modifier.width(58.dp)
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            "Modelled offline from your weekly sessions, volume, streak and best lift.",
            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f),
            fontStyle = FontStyle.Italic, fontSize = 10.sp
        )
    }
}

/** "412k" / "950" — compact lifetime volume (unit-less). */
internal fun formatVolume(lb: Double, unit: WeightUnit): String = formatVolumeCompact(lb, unit, withUnit = false)
