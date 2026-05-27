package com.forge.app.ui.overview

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

@Composable
internal fun HistoryRow(
    entry: HistoryViewModel.HistoryEntry,
    muted: Color,
    onBg: Color,
    outline: Color,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
            .padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            val zone = ZoneId.systemDefault()
            val date = Instant.ofEpochMilli(entry.timestampMs).atZone(zone).toLocalDate()
            val dateStr = date.format(DateTimeFormatter.ofPattern("d MMM", Locale.getDefault()))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(dateStr, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.55f), fontSize = 9.sp)
                if (entry.tag.isNotEmpty()) {
                    Box(
                        modifier = Modifier
                            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.3f)), RoundedCornerShape(2.dp))
                            .padding(horizontal = 5.dp, vertical = 1.dp)
                    ) {
                        Text(entry.tag, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f), fontSize = 7.sp, letterSpacing = 0.5.sp)
                    }
                }
            }
            Text(entry.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
            if (entry.subtitle.isNotEmpty()) {
                Text(entry.subtitle, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 10.sp)
            }
        }
        if (entry.isGym && entry.volumeLb != null && entry.volumeLb > 0) {
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val volText = if (entry.volumeLb >= 1000) "%.1fk".format(entry.volumeLb / 1000) else "${entry.volumeLb.toInt()}"
                Text("$volText lb", style = MaterialTheme.typography.bodySmall, color = onBg, fontWeight = FontWeight.SemiBold, fontSize = 11.sp)
                when {
                    entry.isBest -> Text("BEST", style = MaterialTheme.typography.labelSmall, color = onBg.copy(alpha = 0.85f), fontSize = 8.sp, letterSpacing = 0.6.sp)
                    entry.vsAvgPct != null -> {
                        val sign = if (entry.vsAvgPct >= 0) "+" else ""
                        Text("$sign${entry.vsAvgPct}% avg", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
                    }
                }
                if (entry.prCount > 0) {
                    Text("${entry.prCount} PR", style = MaterialTheme.typography.labelSmall, color = onBg.copy(alpha = 0.6f), fontSize = 8.sp)
                }
            }
        }
    }
}

@Composable
internal fun SummaryStat(value: String, label: String, muted: Color, onBg: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.Normal)
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}
