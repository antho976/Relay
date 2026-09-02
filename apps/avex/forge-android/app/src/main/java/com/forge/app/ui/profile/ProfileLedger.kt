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
import com.forge.app.ui.common.sparklineSeries
import com.forge.app.Features
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.EditorialFigure
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
 * ALL-TIME — the lifetime tallies as one open row of serif figures (the shared [EditorialFigure],
 * §8), each carrying a small ↑/↓ "vs last week" badge. Counts only: lifted volume is the
 * [LifetimeVolumeGraph]'s own reading below, so the number appears once on the page (§4.3).
 */
@Composable
internal fun AllTimeSection(
    sessions: Int,
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
    SectionHeader("ALL-TIME", muted)
    // At zero sessions the row still renders — honest zeros ARE the empty state (§12), the
    // figures fill in from the first logged set.
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
        EditorialFigure("$sessions", "workouts", onBg, muted, accent, Modifier.weight(1f), delta = workoutsDelta)
        EditorialFigure(formatCount(sets), "sets", onBg, muted, accent, Modifier.weight(1f), delta = setsDelta)
        EditorialFigure("$prs", "PRs", onBg, muted, accent, Modifier.weight(1f), delta = prsDelta)
        if (Features.SHOW_GAMIFICATION) {
            EditorialFigure("$xp", "xp", onBg, muted, accent, Modifier.weight(1f))
        }
    }
}

/**
 * LIFETIME VOLUME — every pound you've moved, as the section's serif reading over the cumulative
 * session-by-session curve. Sits between the bodyweight cluster and the year grid; it owns the
 * lifetime-volume number outright, so ALL-TIME above stays counts alone (§4.3). Draws nothing under
 * two logged sessions — one session has no curve, and the total then is just that session's.
 */
@Composable
internal fun LifetimeVolumeGraph(
    volumeSeriesLb: List<Double>,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    if (volumeSeriesLb.size < 2) return
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Reduced to what a phone-width chart can actually show, and remembered (P-13): this is one
    // point per finished session with no cap, and the reveal below re-walked every one of them to
    // rebuild both paths on each of its ~54 frames. The headline figure above still reads the raw
    // series, so the number is exact however the curve under it is plotted.
    val series = remember(volumeSeriesLb, weightUnit) {
        sparklineSeries(volumeSeriesLb).map { toDisplayWeight(it, weightUnit) }
    }
    Column(modifier) {
        SectionHeader("LIFETIME VOLUME", muted)
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                formatVolume(volumeSeriesLb.last(), weightUnit),
                style = MaterialTheme.typography.headlineMedium, color = onBg
            )
            Spacer(Modifier.width(4.dp))
            Text(
                unitLabel(weightUnit).uppercase(),
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp,
                modifier = Modifier.padding(bottom = 4.dp)
            )
        }
        Spacer(Modifier.height(10.dp))
        ProfileSparkline(series, accent, Modifier.fillMaxWidth().height(72.dp))
        Spacer(Modifier.height(8.dp))
        ChartCaption(accent, "SESSION BY SESSION", muted)
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
                    Text(m.label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted)
                    Text(m.valueText, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp)
                }
                val frac = ((100 - m.topPercent) / 100f).coerceIn(0f, 1f)
                val w by animateFloatAsState(
                    targetValue = if (play) frac else 0f,
                    animationSpec = tween(
                        durationMillis = ForgeMotion.nominalDuration(ForgeMotion.DurationEmphasized),
                        delayMillis = ForgeMotion.nominalDuration(i * 90),
                        easing = ForgeMotion.Standard
                    ),
                    label = "standing-bar"
                )
                Box(Modifier.weight(1f).height(2.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.25f))) {
                    Box(Modifier.fillMaxWidth(w).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent))
                }
                Text(
                    "TOP ${m.topPercent}%", style = MaterialTheme.typography.labelSmall, color = onBg,
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
