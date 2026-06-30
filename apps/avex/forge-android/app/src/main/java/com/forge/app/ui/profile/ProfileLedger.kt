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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.Features
import com.forge.app.data.repo.SignatureLift
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.roundToInt

/**
 * Shared section scaffold (label + optional accent action + body) used only by the gamification
 * [StandingSection] now — the everyday stat sections moved to the card/tile primitives in
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

/** ALL-TIME — lifetime tallies as a 2-up tile grid; the volume tile carries a cumulative sparkline. */
@Composable
internal fun AllTimeTiles(
    sessions: Int,
    volumeLb: Double,
    prs: Int,
    xp: Long,
    longestStreakDays: Int,
    /** Cumulative lifted volume (lb), oldest → newest; powers the volume tile's sparkline. */
    volumeSeriesLb: List<Double>,
    onBg: Color,
    muted: Color,
    accent: Color,
    @Suppress("UNUSED_PARAMETER") outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    SectionHeader("ALL-TIME", muted, accent)
    if (sessions == 0) {
        // Bare zeros read as "empty/broken" on a stranger's first open — name what fills them.
        ProfileCard {
            InlineEmptyHint(
                if (Features.SHOW_GAMIFICATION)
                    "Finish your first workout — your lifetime workouts, volume, PRs, and XP start tallying here."
                else
                    "Finish your first workout — your lifetime workouts, volume and PRs start tallying here.",
                muted
            )
        }
        return
    }
    val series = if (volumeSeriesLb.size >= 2) volumeSeriesLb.map { toDisplayWeight(it, useKg) } else null
    val specs = buildList {
        add(StatTileSpec("$sessions", "WORKOUTS"))
        add(StatTileSpec(formatVolume(volumeLb, useKg), "LIFETIME ${unitLabel(useKg).uppercase()}", sparkline = series))
        add(StatTileSpec("$prs", "PRs"))
        add(StatTileSpec("$longestStreakDays", "BEST STREAK", caption = if (longestStreakDays > 1) "days" else null))
        if (Features.SHOW_GAMIFICATION) add(StatTileSpec("$xp", "XP"))
    }
    StatTileGrid(specs, accent, muted, onBg)
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
                Box(Modifier.weight(1f).height(2.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.2f))) {
                    Box(Modifier.fillMaxWidth(w).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent.copy(alpha = 0.8f)))
                }
                Text(
                    "TOP ${m.topPercent}%", style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp,
                    modifier = Modifier.width(58.dp)
                )
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

/** SIGNATURE — top lift · most-logged day · usual hour, three cells on one card. */
@Composable
internal fun SignatureCard(
    topLift: SignatureLift?,
    mostLoggedDay: String?,
    usualHour: String?,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    SectionHeader("SIGNATURE", muted, accent)
    ProfileCard {
        if (topLift == null && mostLoggedDay == null && usualHour == null) {
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

/** All-time cardio totals — three cells on a card, only shown once a non-rest session exists. */
@Composable
internal fun CardioCard(
    sessions: Int,
    minutes: Int,
    distanceKm: Double,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useMiles = LocalForgeSettings.current.useMiles
    val timeLabel = if (minutes >= 60) "${minutes / 60}h ${minutes % 60}m" else "$minutes min"
    val distLabel = if (distanceKm > 0) com.forge.app.domain.units.formatDistance(distanceKm, useMiles) else "—"
    SectionHeader("CARDIO", muted, accent)
    ProfileCard {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
            SignatureCell("$sessions", "SESSIONS", onBg, muted, Modifier.weight(1f))
            SignatureDivider(outline)
            SignatureCell(timeLabel, "TIME", onBg, muted, Modifier.weight(1f))
            SignatureDivider(outline)
            SignatureCell(distLabel, "DISTANCE", onBg, muted, Modifier.weight(1f))
        }
    }
}

@Composable
private fun SignatureCell(value: String, label: String, onBg: Color, muted: Color, modifier: Modifier = Modifier) {
    Column(modifier.padding(end = 8.dp)) {
        Text(value, style = MaterialTheme.typography.titleSmall, color = onBg, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.height(3.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}

@Composable
private fun SignatureDivider(outline: Color) {
    Box(Modifier.width(1.dp).height(28.dp).padding(end = 8.dp).background(outline.copy(alpha = 0.3f)))
}

/** "412k" / "950" — compact lifetime volume (unit-less). */
internal fun formatVolume(lb: Double, useKg: Boolean): String = formatVolumeCompact(lb, useKg, withUnit = false)
