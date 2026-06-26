package com.forge.app.ui.profile

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.HorizontalDivider
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
import com.forge.app.Features
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.stats.components.LineChart
import kotlin.math.roundToInt
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.data.repo.SignatureLift
import com.forge.app.ui.theme.ForgeMotion

/**
 * Shared section scaffold: hairline divider + small-caps label (+ optional accent action) + body.
 * [compact] tightens the vertical rhythm (used by the denser Ledger / Signature blocks).
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
    Spacer(Modifier.height(if (compact) 14.dp else 22.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.3f))
    Spacer(Modifier.height(if (compact) 8.dp else 14.dp))
    Row(
        Modifier.fillMaxWidth().then(if (onAction != null) Modifier.clickable { onAction() } else Modifier),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
        if (action != null) Text(action, style = MaterialTheme.typography.labelSmall, color = accent)
    }
    Spacer(Modifier.height(if (compact) 8.dp else 12.dp))
    Column(content = content)
}

/** ALL-TIME — lifetime tallies + a cumulative-volume curve (XP cell only when gamification is enabled). */
@Composable
internal fun LedgerSection(
    sessions: Int,
    volumeLb: Double,
    prs: Int,
    xp: Long,
    muted: Color,
    accent: Color,
    outline: Color,
    longestStreakDays: Int = 0,
    /** Cumulative lifted volume (lb) bucketed by month, oldest → newest. Empty until ≥2 months exist. */
    volumeSeriesLb: List<Double> = emptyList()
) {
    val useKg = LocalForgeSettings.current.useKg
    ProfileBlock("ALL-TIME", muted, accent, outline, compact = true) {
        if (sessions == 0) {
            // Bare zeros on a stranger's first open read as "empty/broken" — name what fills them.
            InlineEmptyHint(
                if (Features.SHOW_GAMIFICATION)
                    "Finish your first workout — your lifetime workouts, volume, PRs, and XP start tallying here."
                else
                    "Finish your first workout — your lifetime workouts, volume and PRs start tallying here.",
                muted
            )
        } else {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                LifetimeStat("$sessions", "WORKOUTS", compact = true)
                LifetimeStat(formatVolume(volumeLb, useKg), "LIFETIME ${unitLabel(useKg).uppercase()}", compact = true)
                LifetimeStat("$prs", "PRs", compact = true)
                if (Features.SHOW_GAMIFICATION) LifetimeStat("$xp", "XP", compact = true)
            }
            // Cumulative-volume curve: total lifted weight (in the display unit) climbing month by
            // month over your whole history. Needs ≥2 points to draw a line.
            if (volumeSeriesLb.size >= 2) {
                Spacer(Modifier.height(14.dp))
                val series = volumeSeriesLb.map { toDisplayWeight(it, useKg) }
                LineChart(
                    values = series,
                    lineColor = accent,
                    minValue = 0.0,
                    maxValue = series.last(),
                    modifier = Modifier.fillMaxWidth().height(56.dp)
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    "Total ${unitLabel(useKg)} lifted, all time",
                    style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                )
            }
            if (longestStreakDays > 1) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "Longest streak ever · $longestStreakDays days",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                )
            }
        }
    }
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
    // "ESTIMATED" (not "VS ATHLETES") in the header itself so a stranger can't misread these
    // percentiles as a real online ranking before they reach the disclaimer below.
    ProfileBlock("STANDING · ESTIMATED, 90 DAYS", muted, accent, outline) {
        // Each bar fills from 0 on first show, staggered by row for a cascade (reduced motion → instant).
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
                    // Your actual value behind the percentile, so the bar isn't an abstract figure.
                    Text(m.valueText, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp)
                }
                // Fill = how far above the bottom of the pack you are (lower topPercent = fuller).
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
                Box(Modifier.weight(1f).height(2.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.2f))) {
                    Box(Modifier.fillMaxWidth(w).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent.copy(alpha = 0.8f)))
                }
                Text("TOP ${m.topPercent}%", style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp,
                    modifier = Modifier.width(58.dp))
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            "Estimated from your sessions per week, weekly volume, streak and best lift over the last 90 days — " +
                "modelled, not a live leaderboard. Your data never leaves your device.",
            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f),
            fontStyle = FontStyle.Italic, fontSize = 10.sp
        )
    }
}

/** SIGNATURE — top lift · most-logged day · usual hour. */
@Composable
internal fun SignatureSection(
    topLift: SignatureLift?,
    mostLoggedDay: String?,
    usualHour: String?,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    ProfileBlock("SIGNATURE", muted, accent, outline, compact = true) {
        if (topLift == null && mostLoggedDay == null && usualHour == null) {
            // Three "—" cells look like a rendering error to a stranger — explain what they become.
            InlineEmptyHint(
                "Your signature — your go-to lift, the day you train most, and your usual hour — takes shape after a few logged sessions.",
                muted
            )
        } else {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                SignatureCell(
                    value = topLift?.name ?: "—",
                    label = if (topLift != null) "TOP LIFT · ${toDisplayWeight(topLift.weightLb, useKg).roundToInt()} ${unitLabel(useKg).uppercase()}" else "TOP LIFT",
                    onBg = onBg, muted = muted, modifier = Modifier.weight(1f)
                )
                SignatureDivider(outline)
                SignatureCell(value = mostLoggedDay ?: "—", label = "MOST LOGGED", onBg = onBg, muted = muted, modifier = Modifier.weight(1f))
                SignatureDivider(outline)
                SignatureCell(value = usualHour ?: "—", label = "USUAL HOUR", onBg = onBg, muted = muted, modifier = Modifier.weight(1f))
            }
        }
    }
}

/** All-time cardio totals — only rendered once at least one non-rest session has been logged. */
@Composable
internal fun CardioTotalsSection(
    sessions: Int,
    minutes: Int,
    distanceKm: Double,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useMiles = com.forge.app.ui.theme.LocalForgeSettings.current.useMiles
    val timeLabel = if (minutes >= 60) "${minutes / 60}h ${minutes % 60}m" else "$minutes min"
    val distLabel = if (distanceKm > 0) com.forge.app.domain.units.formatDistance(distanceKm, useMiles) else "—"
    ProfileBlock("CARDIO", muted, accent, outline) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            LifetimeStat("$sessions", "SESSIONS")
            LifetimeStat(timeLabel, "TIME")
            LifetimeStat(distLabel, "DISTANCE")
        }
    }
}

@Composable
private fun SignatureCell(value: String, label: String, onBg: Color, muted: Color, modifier: Modifier = Modifier) {
    Column(modifier.padding(end = 8.dp)) {
        // One line only — long lift names ellipsize rather than wrap to a second row, so the
        // SIGNATURE block stays a single line tall.
        Text(value, style = MaterialTheme.typography.titleSmall, color = onBg, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(3.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}

@Composable
private fun SignatureDivider(outline: Color) {
    Box(Modifier.width(1.dp).height(28.dp).padding(end = 8.dp).background(outline.copy(alpha = 0.3f)))
}

/** A lifetime tally cell. [compact] drops the value to titleSmall for the denser Ledger row. */
@Composable
internal fun LifetimeStat(value: String, label: String, compact: Boolean = false) {
    Column(horizontalAlignment = Alignment.Start) {
        Text(
            value,
            style = if (compact) MaterialTheme.typography.titleSmall else MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground
        )
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 8.sp)
    }
}

/** "412k" / "950" — compact lifetime volume (unit-less). */
internal fun formatVolume(lb: Double, useKg: Boolean): String = formatVolumeCompact(lb, useKg, withUnit = false)
