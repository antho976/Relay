package com.forge.app.ui.cardio

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale

@Composable
internal fun CardioHero(
    weekMinutes: Int,
    weekNum: Int,
    weekLabel: String,
    weekEntries: List<CardioEntry>,
    zone: ZoneId,
    onBg: Color,
    muted: Color
) {
    val description = remember(weekEntries) { buildWeekDescription(weekEntries, zone) }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 8.dp)
    ) {
        Text("WEEK $weekNum · $weekLabel", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                if (weekMinutes > 0) "$weekMinutes" else "—",
                style = MaterialTheme.typography.displayLarge,
                color = onBg
            )
            Text(
                " minutes.",
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(start = 4.dp, bottom = 6.dp)
            )
        }
        if (description.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Text(description, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        }
        Spacer(Modifier.height(16.dp))
    }
}

@Composable
internal fun WeekBoxRow(
    dailyMinutes: List<Int>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    val dayLetters = listOf("M", "T", "W", "T", "F", "S", "S")
    val maxMin = (dailyMinutes.maxOrNull() ?: 0).coerceAtLeast(1)
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        dayLetters.forEachIndexed { i, letter ->
            val mins = dailyMinutes.getOrElse(i) { 0 }
            val isToday = i == todayDow
            val hasActivity = mins > 0
            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                if (hasActivity) {
                    Text("${mins}m", fontSize = 9.sp, color = onBg, fontWeight = FontWeight.SemiBold)
                }
                // Proportional bar in a fixed 48dp track — clearly readable, white-on-dark.
                Box(
                    modifier = Modifier.fillMaxWidth().height(48.dp),
                    contentAlignment = Alignment.BottomCenter
                ) {
                    val frac = (mins.toFloat() / maxMin).coerceIn(0f, 1f)
                    val barHeight = if (hasActivity) (8 + 40 * frac).dp else 4.dp
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(barHeight)
                            .clip(RoundedCornerShape(4.dp))
                            .then(
                                if (isToday && !hasActivity) {
                                    Modifier.drawBehind {
                                        drawRoundRect(
                                            color = onBg.copy(alpha = 0.6f),
                                            style = Stroke(
                                                width = 1.5.dp.toPx(),
                                                pathEffect = PathEffect.dashPathEffect(floatArrayOf(5f, 4f), 0f)
                                            ),
                                            cornerRadius = CornerRadius(4.dp.toPx())
                                        )
                                    }
                                } else {
                                    Modifier.background(
                                        when {
                                            hasActivity -> onBg
                                            else -> outline.copy(alpha = 0.35f)
                                        }
                                    )
                                }
                            )
                    )
                }
                Text(
                    letter,
                    fontSize = 9.sp,
                    color = if (isToday) onBg else muted,
                    fontWeight = if (isToday) FontWeight.Bold else FontWeight.Normal
                )
            }
        }
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
internal fun LogTodayRow(onOpenLog: () -> Unit, onBg: Color, muted: Color, outline: Color) {
    // Prominent full-width CTA so logging never feels hidden.
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .clip(RoundedCornerShape(16.dp))
            .border(1.dp, onBg, RoundedCornerShape(16.dp))
            .clickable(onClick = onOpenLog)
            .padding(horizontal = 16.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(
            modifier = Modifier.size(40.dp).background(onBg, CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(Icons.Filled.Add, contentDescription = null, tint = MaterialTheme.colorScheme.background, modifier = Modifier.size(20.dp))
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("Log today's cardio", style = MaterialTheme.typography.bodyLarge, color = onBg, fontWeight = FontWeight.SemiBold)
            Text("run · walk · treadmill · rest", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = null, tint = onBg, modifier = Modifier.size(16.dp))
    }
    Spacer(Modifier.height(8.dp))
}

internal fun buildWeekDescription(entries: List<CardioEntry>, zone: ZoneId): String {
    if (entries.isEmpty()) return ""
    if (entries.size == 1) {
        val e = entries.first()
        val typeName = CardioType.fromCode(e.type).displayName.lowercase()
        val dayName = Instant.ofEpochMilli(e.date)
            .atZone(zone).dayOfWeek
            .getDisplayName(TextStyle.FULL, Locale.getDefault())
            .lowercase()
        return "One $typeName on $dayName."
    }
    return "${entries.size} sessions this week."
}
