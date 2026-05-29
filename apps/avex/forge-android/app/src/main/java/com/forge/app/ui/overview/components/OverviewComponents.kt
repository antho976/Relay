package com.forge.app.ui.overview.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.overview.state.OverviewRecentItem

@Composable
fun WeekDayBox(
    letter: String,
    trained: Boolean,
    isToday: Boolean,
    outlineColor: Color,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            letter,
            style = MaterialTheme.typography.labelSmall,
            color = if (isToday) onBg else onBg.copy(alpha = 0.75f),
            fontSize = 10.sp,
            fontWeight = if (isToday) FontWeight.SemiBold else FontWeight.Normal
        )
        val boxMod = Modifier.fillMaxWidth().aspectRatio(1f)
        when {
            trained -> Box(boxMod.background(onBg.copy(alpha = 0.85f), RoundedCornerShape(6.dp)))
            isToday -> {
                val dashColor = outlineColor.copy(alpha = 0.9f)
                Box(
                    boxMod.drawBehind {
                        drawRoundRect(
                            color = dashColor,
                            cornerRadius = CornerRadius(6.dp.toPx()),
                            style = Stroke(
                                width = 1.5.dp.toPx(),
                                pathEffect = PathEffect.dashPathEffect(
                                    floatArrayOf(4.dp.toPx(), 3.dp.toPx()), 0f
                                )
                            )
                        )
                    },
                    contentAlignment = Alignment.Center
                ) {
                    Text("NOW", style = MaterialTheme.typography.labelSmall, fontSize = 7.sp,
                        color = outlineColor.copy(alpha = 0.9f))
                }
            }
            else -> Box(boxMod.border(BorderStroke(1.dp, outlineColor.copy(alpha = 0.55f)), RoundedCornerShape(6.dp)))
        }
    }
}

@Composable
fun OverviewStat(value: String, label: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(value, style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onBackground, fontWeight = FontWeight.Normal)
        Text(label, style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun RecentRow(
    item: OverviewRecentItem,
    muted: Color,
    onBg: Color,
    outline: Color,
    onClick: () -> Unit = {}
) {
    Row(
        modifier = Modifier.fillMaxWidth().clickable { onClick() },
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(item.dayLabel, style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp)
                if (item.tag.isNotEmpty()) {
                    Box(
                        modifier = Modifier
                            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.4f)), RoundedCornerShape(3.dp))
                            .padding(horizontal = 6.dp, vertical = 2.dp)
                    ) {
                        Text(item.tag, style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.7f), fontSize = 8.sp, letterSpacing = 0.5.sp)
                    }
                }
            }
            Text(item.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
            if (item.subtitle.isNotEmpty()) {
                Text(item.subtitle, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp)
            }
        }

        if (item.isGym && item.volumeLb != null && item.volumeLb > 0) {
            Spacer(Modifier.width(12.dp))
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val volText = if (item.volumeLb >= 1000)
                    "%.1fk lb".format(item.volumeLb / 1000) else "${item.volumeLb.toInt()} lb"
                Text(volText, style = MaterialTheme.typography.bodySmall, color = onBg,
                    fontWeight = FontWeight.SemiBold, fontSize = 12.sp)
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
        }
    }
}
