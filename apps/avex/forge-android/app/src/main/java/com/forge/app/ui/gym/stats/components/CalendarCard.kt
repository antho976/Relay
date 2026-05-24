package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.gym.stats.state.MonthCalendarData
import com.forge.app.ui.gym.stats.state.SessionDaySummary
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale

/**
 * Monthly training calendar for the Stats tab (#54).
 * Shows the current month with dots on days that had a session.
 * Tapping a session day reveals a mini summary below the grid.
 */
@Composable
fun CalendarCard(
    data: MonthCalendarData,
    modifier: Modifier = Modifier,
    onMarkRestDay: (dateKey: String, type: String) -> Unit = { _, _ -> },
    onClearRestDay: (dateKey: String) -> Unit = {}
) {
    var selectedDay by remember { mutableStateOf<Int?>(null) }
    var showRestPicker by remember { mutableStateOf<Int?>(null) }
    val today = LocalDate.now(ZoneId.systemDefault())
    val firstDayOfMonth = data.yearMonth.atDay(1)
    val daysInMonth = data.yearMonth.lengthOfMonth()

    // Monday-first offset: Monday=0 … Sunday=6
    val startOffset = (firstDayOfMonth.dayOfWeek.value - 1 + 7) % 7

    val monthLabel = data.yearMonth.month.getDisplayName(TextStyle.FULL, Locale.getDefault())
        .replaceFirstChar { it.uppercase() } + " ${data.yearMonth.year}"

    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                "CALENDAR",
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                monthLabel,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface
            )

            // Day-of-week headers
            Row(modifier = Modifier.fillMaxWidth()) {
                listOf("M", "T", "W", "T", "F", "S", "S").forEach { label ->
                    Text(
                        text = label,
                        modifier = Modifier.weight(1f),
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            // Calendar grid — total cells = offset + daysInMonth, rounded up to multiple of 7
            val totalCells = startOffset + daysInMonth
            val cells = (0 until totalCells).toList()

            LazyVerticalGrid(
                columns = GridCells.Fixed(7),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(((totalCells / 7 + if (totalCells % 7 != 0) 1 else 0) * 40).dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
                userScrollEnabled = false
            ) {
                items(cells) { cellIndex ->
                    val dayOfMonth = cellIndex - startOffset + 1
                    if (cellIndex < startOffset || dayOfMonth > daysInMonth) {
                        Box(Modifier.aspectRatio(1f))
                        return@items
                    }
                    val hasSession = dayOfMonth in data.sessionDays
                    val isToday = data.yearMonth.atDay(dayOfMonth) == today
                    val isSelected = selectedDay == dayOfMonth
                    val hasRestDay = dayOfMonth in data.restDays
                    DayCell(
                        day = dayOfMonth,
                        hasSession = hasSession,
                        hasRestDay = hasRestDay,
                        restDayType = data.restDays[dayOfMonth],
                        isToday = isToday,
                        isSelected = isSelected,
                        onClick = {
                            if (hasSession) selectedDay = if (isSelected) null else dayOfMonth
                            else showRestPicker = dayOfMonth
                        }
                    )
                }
            }

            // Session detail for selected day
            selectedDay?.let { day ->
                data.sessionDays[day]?.let { summary ->
                    SessionDayDetail(day = day, month = monthLabel, summary = summary)
                }
            }
        }
    }

    // Rest day picker sheet
    showRestPicker?.let { day ->
        val dateKey = "${data.yearMonth}-${day.toString().padStart(2, '0')}"
        val existing = data.restDays[day]
        AlertDialog(
            onDismissRequest = { showRestPicker = null },
            title = { Text("Mark day $day") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("planned" to "Planned rest 🛋️", "sick" to "Sick day 🤧", "travel" to "Travel day ✈️").forEach { (type, label) ->
                        Row(
                            modifier = Modifier.fillMaxWidth().clickable {
                                onMarkRestDay(dateKey, type)
                                showRestPicker = null
                            }.padding(8.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) { Text(label) }
                    }
                    if (existing != null) {
                        Row(
                            modifier = Modifier.fillMaxWidth().clickable {
                                onClearRestDay(dateKey)
                                showRestPicker = null
                            }.padding(8.dp)
                        ) { Text("Clear marker", color = MaterialTheme.colorScheme.error) }
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { showRestPicker = null }) { Text("Cancel") } }
        )
    }
}

@Composable
private fun DayCell(
    day: Int,
    hasSession: Boolean,
    hasRestDay: Boolean = false,
    restDayType: String? = null,
    isToday: Boolean,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    val accent = MaterialTheme.colorScheme.primary
    val restColor = MaterialTheme.colorScheme.secondary
    val bg = when {
        isSelected -> accent.copy(alpha = 0.22f)
        hasSession -> accent.copy(alpha = 0.10f)
        hasRestDay -> restColor.copy(alpha = 0.10f)
        else -> MaterialTheme.colorScheme.surface
    }
    val textColor = when {
        isSelected || hasSession -> accent
        hasRestDay -> restColor
        isToday -> MaterialTheme.colorScheme.onSurface
        else -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f)
    }
    Box(
        modifier = Modifier
            .aspectRatio(1f)
            .clip(RoundedCornerShape(6.dp))
            .background(bg)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = "$day",
                style = MaterialTheme.typography.labelMedium,
                fontWeight = if (isToday) FontWeight.Black else FontWeight.Normal,
                color = textColor
            )
            when {
                hasSession -> {
                    Spacer(Modifier.height(1.dp))
                    Box(Modifier.size(4.dp).clip(CircleShape).background(accent))
                }
                hasRestDay -> {
                    val emoji = when (restDayType) { "sick" -> "🤧" "travel" -> "✈" else -> "🛋" }
                    Text(emoji, style = MaterialTheme.typography.labelSmall.copy(fontSize = androidx.compose.ui.unit.TextUnit(8f, androidx.compose.ui.unit.TextUnitType.Sp)))
                }
            }
        }
    }
}

@Composable
private fun SessionDayDetail(day: Int, month: String, summary: SessionDaySummary) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.primary.copy(alpha = 0.08f),
        shape = RoundedCornerShape(8.dp)
    ) {
        Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                summary.dayName,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.primary
            )
            val detail = buildString {
                if (summary.totalVolumeLb > 0) append("${summary.totalVolumeLb.toInt()} lb")
                if (summary.prCount > 0) {
                    if (isNotEmpty()) append(" · ")
                    append("${summary.prCount} PR${if (summary.prCount > 1) "s" else ""}")
                }
            }
            if (detail.isNotEmpty()) {
                Text(
                    detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}
