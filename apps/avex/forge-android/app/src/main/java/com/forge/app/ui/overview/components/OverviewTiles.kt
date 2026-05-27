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
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun CardioTile(
    cardioWeekDays: Set<Int>,
    totalMin: Int,
    totalKm: Double,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.35f)), RoundedCornerShape(10.dp))
            .clip(RoundedCornerShape(10.dp))
            .clickable { onClick() }
            .padding(horizontal = 16.dp, vertical = 14.dp)
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("CARDIO", style = MaterialTheme.typography.labelSmall, letterSpacing = 1.sp, color = muted, fontSize = 9.sp)
                Text("→", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.5f))
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                listOf("M", "T", "W", "T", "F", "S", "S").forEachIndexed { i, letter ->
                    val active = i in cardioWeekDays
                    Column(
                        modifier = Modifier.weight(1f),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        Box(
                            modifier = Modifier.fillMaxWidth().height(36.dp).then(
                                if (active) Modifier.background(onBg.copy(alpha = 0.82f), RoundedCornerShape(5.dp))
                                else Modifier.border(BorderStroke(0.5.dp, outline.copy(alpha = 0.45f)), RoundedCornerShape(5.dp))
                            )
                        )
                        Text(letter, style = MaterialTheme.typography.labelSmall, fontSize = 8.sp,
                            color = if (active) onBg else muted.copy(alpha = 0.4f))
                    }
                }
            }
            if (totalMin > 0) {
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.Bottom) {
                    Text("$totalMin min", style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.Normal)
                    if (totalKm > 0) {
                        Text("· ${"%.1f".format(totalKm)} km", style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp)
                    }
                }
            } else {
                Text("no cardio logged this week", style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.4f), fontStyle = FontStyle.Italic, fontSize = 11.sp)
            }
        }
    }
}

@Composable
fun StatsTile(
    totalSessions: Int,
    streakDays: Int,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(108.dp)
            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.35f)), RoundedCornerShape(10.dp))
            .clip(RoundedCornerShape(10.dp))
            .clickable { onClick() }
            .padding(horizontal = 14.dp, vertical = 12.dp)
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("STATS", style = MaterialTheme.typography.labelSmall, letterSpacing = 1.sp, color = muted, fontSize = 9.sp)
            Spacer(Modifier.height(4.dp))
            Text("$totalSessions", style = MaterialTheme.typography.headlineSmall, color = onBg, fontWeight = FontWeight.Normal)
            Text("sessions total", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp)
            if (streakDays > 0) {
                Spacer(Modifier.height(2.dp))
                Text("$streakDays day streak", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
            }
        }
        Text("→", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.5f), modifier = Modifier.align(Alignment.TopEnd))
    }
}

@Composable
fun TrophiesTile(
    unlocked: Int,
    total: Int,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(108.dp)
            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.35f)), RoundedCornerShape(10.dp))
            .clip(RoundedCornerShape(10.dp))
            .clickable { onClick() }
            .padding(horizontal = 14.dp, vertical = 12.dp)
    ) {
        Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("TROPHIES", style = MaterialTheme.typography.labelSmall, letterSpacing = 1.sp, color = muted, fontSize = 9.sp)
            Spacer(Modifier.height(4.dp))
            Text("$unlocked", style = MaterialTheme.typography.headlineSmall, color = onBg, fontWeight = FontWeight.Normal)
            Text("of $total", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp)
            Spacer(Modifier.height(6.dp))
            val progress = if (total > 0) (unlocked.toFloat() / total).coerceIn(0f, 1f) else 0f
            Box(Modifier.fillMaxWidth().height(2.dp).background(outline.copy(alpha = 0.2f), RoundedCornerShape(1.dp))) {
                Box(Modifier.fillMaxWidth(progress).height(2.dp).background(onBg.copy(alpha = 0.7f), RoundedCornerShape(1.dp)))
            }
        }
        Text("→", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.5f), modifier = Modifier.align(Alignment.TopEnd))
    }
}
