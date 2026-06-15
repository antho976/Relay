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
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import kotlin.math.roundToInt
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.data.repo.SignatureLift
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.emphasized

/** Shared section scaffold: hairline divider + small-caps label (+ optional accent action) + body. */
@Composable
internal fun ProfileBlock(
    label: String,
    muted: Color,
    accent: Color,
    outline: Color,
    action: String? = null,
    onAction: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    Spacer(Modifier.height(22.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.3f))
    Spacer(Modifier.height(14.dp))
    Row(
        Modifier.fillMaxWidth().then(if (onAction != null) Modifier.clickable { onAction() } else Modifier),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
        if (action != null) Text(action, style = MaterialTheme.typography.labelSmall, color = accent)
    }
    Spacer(Modifier.height(12.dp))
    Column(content = content)
}

/** THE LEDGER · ALL TIME — four lifetime tallies. */
@Composable
internal fun LedgerSection(
    sessions: Int,
    volumeLb: Double,
    prs: Int,
    xp: Long,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    ProfileBlock("THE LEDGER · ALL TIME", muted, accent, outline) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            LifetimeStat("$sessions", "WORKOUTS")
            LifetimeStat(formatVolume(volumeLb, useKg), "LIFETIME ${unitLabel(useKg).uppercase()}")
            LifetimeStat("$prs", "PRs")
            LifetimeStat("$xp", "XP")
        }
    }
}

/** STANDING · VS ATHLETES, 90 DAYS — offline percentile estimate. */
@Composable
internal fun StandingSection(
    standings: List<StandingMetric>,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    if (standings.isEmpty()) return
    ProfileBlock("STANDING · VS ATHLETES, 90 DAYS", muted, accent, outline) {
        // Each bar fills from 0 on first show, staggered by row for a cascade (reduced motion → instant).
        var play by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) { play = true }
        standings.forEachIndexed { i, m ->
            Row(
                Modifier.fillMaxWidth().padding(vertical = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Text(m.label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp,
                    modifier = Modifier.width(96.dp))
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
            "Estimated vs typical lifters — nothing ever leaves your phone.",
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
    ProfileBlock("SIGNATURE", muted, accent, outline) {
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

@Composable
private fun SignatureCell(value: String, label: String, onBg: Color, muted: Color, modifier: Modifier = Modifier) {
    Column(modifier.padding(end = 8.dp)) {
        Text(value, style = MaterialTheme.typography.titleMedium, color = emphasized(onBg), maxLines = 2)
        Spacer(Modifier.height(4.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}

@Composable
private fun SignatureDivider(outline: Color) {
    Box(Modifier.width(1.dp).height(34.dp).padding(end = 8.dp).background(outline.copy(alpha = 0.3f)))
}

@Composable
internal fun LifetimeStat(value: String, label: String) {
    Column(horizontalAlignment = Alignment.Start) {
        Text(value, style = MaterialTheme.typography.titleMedium, color = emphasized(MaterialTheme.colorScheme.onBackground))
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 8.sp)
    }
}

/** "412k" / "950" — compact lifetime volume (unit-less). */
internal fun formatVolume(lb: Double, useKg: Boolean): String = formatVolumeCompact(lb, useKg, withUnit = false)
