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
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        dayLetters.forEachIndexed { i, letter ->
            val mins = dailyMinutes.getOrElse(i) { 0 }
            val isToday = i == todayDow
            val hasActivity = mins > 0
            val isFuture = i > todayDow
            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(letter, fontSize = 8.sp, color = if (isToday) onBg else muted.copy(alpha = 0.45f))
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(40.dp)
                        .then(
                            if (isToday && !hasActivity) {
                                Modifier.drawBehind {
                                    val effect = PathEffect.dashPathEffect(floatArrayOf(5f, 4f), 0f)
                                    drawRoundRect(
                                        color = onBg.copy(alpha = 0.35f),
                                        style = Stroke(width = 1.dp.toPx(), pathEffect = effect),
                                        cornerRadius = CornerRadius(4.dp.toPx())
                                    )
                                }
                            } else {
                                Modifier.background(
                                    when {
                                        hasActivity -> onBg.copy(alpha = 0.14f)
                                        isFuture -> outline.copy(alpha = 0.06f)
                                        else -> outline.copy(alpha = 0.1f)
                                    },
                                    RoundedCornerShape(4.dp)
                                )
                            }
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    if (hasActivity) {
                        Text("${mins}m", fontSize = 9.sp, color = onBg, fontWeight = FontWeight.Medium)
                    } else if (isToday) {
                        Text("NOW", fontSize = 7.sp, color = muted.copy(alpha = 0.6f), letterSpacing = 0.5.sp)
                    }
                }
            }
        }
    }
    Spacer(Modifier.height(4.dp))
}

@Composable
internal fun LogTodayRow(onOpenLog: () -> Unit, onBg: Color, muted: Color, outline: Color) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpenLog)
            .padding(horizontal = 24.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Box(
            modifier = Modifier.size(44.dp).border(1.dp, onBg.copy(alpha = 0.55f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(Icons.Filled.Add, contentDescription = null, tint = onBg, modifier = Modifier.size(20.dp))
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("Log today's cardio", style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text("run · walk · treadmill · rest", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = null, tint = muted.copy(alpha = 0.5f), modifier = Modifier.size(14.dp))
    }
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
