package com.forge.app.ui.overview.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatVolume
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.theme.LocalForgeSettings

@Composable
fun RecentRow(
    item: OverviewRecentItem,
    muted: Color,
    onBg: Color,
    outline: Color,
    onClick: () -> Unit = {}
) {
    val settings = LocalForgeSettings.current
    val weightUnit = settings.weightUnit
    Column(
        modifier = Modifier.fillMaxWidth().bounceClick { onClick() },
        verticalArrangement = Arrangement.spacedBy(3.dp)
    ) {
        // Meta row spans the full width: day label hard-left, chips hard-right — so the tag aligns
        // with "view all →" above it and sits directly over the volume column below.
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(item.dayLabel, style = MaterialTheme.typography.labelSmall,
                color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically) {
                // Status marker (deload/test/…) reads a touch stronger than the category tag.
                if (item.statusPill.isNotEmpty()) {
                    MiniChip(text = item.statusPill, color = muted, outline = outline)
                }
                if (item.tag.isNotEmpty()) {
                    MiniChip(text = item.tag, color = muted.copy(alpha = 0.7f), outline = outline)
                }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text(item.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
                if (item.subtitle.isNotEmpty()) {
                    Text(item.subtitle, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp)
                }
                item.topLift?.let { lift ->
                    Text(lift, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.75f),
                        fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }

            if (item.isGym && item.volumeLb != null && item.volumeLb > 0) {
                Spacer(Modifier.width(12.dp))
                Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    val volText = formatVolume(item.volumeLb, weightUnit)
                    // Trend vs this day-type's average (reuses vsAvgPct); a best session always reads up.
                    val trend = when {
                        item.isBest || (item.vsAvgPct != null && item.vsAvgPct > 0) -> "↑"
                        item.vsAvgPct != null && item.vsAvgPct < 0 -> "↓"
                        else -> null
                    }
                    Row(verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                        if (trend != null) {
                            Text(trend, style = MaterialTheme.typography.bodySmall,
                                color = if (trend == "↑") onBg else muted, fontSize = 11.sp)
                        }
                        Text(volText, style = MaterialTheme.typography.bodySmall, color = onBg,
                            fontWeight = FontWeight.SemiBold, fontSize = 12.sp)
                    }
                    when {
                        item.isBest -> Text("BEST", style = MaterialTheme.typography.labelSmall,
                            color = onBg.copy(alpha = 0.85f), fontSize = 9.sp, letterSpacing = 0.6.sp)
                        item.vsAvgPct != null -> {
                            val sign = if (item.vsAvgPct >= 0) "+" else ""
                            Text("$sign${item.vsAvgPct}% avg", style = MaterialTheme.typography.labelSmall,
                                color = muted, fontSize = 9.sp)
                        }
                    }
                    if (item.prCount > 0) {
                        Text("${item.prCount} PR", style = MaterialTheme.typography.labelSmall,
                            color = onBg.copy(alpha = 0.6f), fontSize = 9.sp)
                    }
                }
            } else if (!item.isGym && item.distanceKm != null && item.distanceKm > 0) {
                // Cardio's headline number sits where a gym row's volume does.
                Spacer(Modifier.width(12.dp))
                Text(formatDistance(item.distanceKm, settings.useMiles),
                    style = MaterialTheme.typography.bodySmall, color = onBg,
                    fontWeight = FontWeight.SemiBold, fontSize = 12.sp)
            }
        }
    }
}

/** The small bordered category/status chip used in a RECENT row. */
@Composable
private fun MiniChip(text: String, color: Color, outline: Color) {
    Box(
        modifier = Modifier
            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.35f)), RoundedCornerShape(4.dp))
            .padding(horizontal = 6.dp, vertical = 2.dp)
    ) {
        Text(text, style = MaterialTheme.typography.labelSmall,
            color = color, fontSize = 8.sp, letterSpacing = 0.5.sp,
            maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
