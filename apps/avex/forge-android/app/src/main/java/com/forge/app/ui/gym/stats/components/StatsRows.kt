package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.WeekActivityRow
import java.text.SimpleDateFormat
import java.time.LocalDate
import java.util.Date
import java.util.Locale

@Composable
internal fun RhythmRow(
    weekActivity: List<WeekActivityRow>,
    today: LocalDate,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    val dayLetters = listOf("M", "T", "W", "T", "F", "S", "S")
    val todayDow = today.dayOfWeek.value - 1
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        (0..6).forEach { i ->
            val row = weekActivity.getOrNull(i)
            val hasActivity = row?.sessionName != null || row?.cardioType != null
            val isToday = i == todayDow
            Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(if (isToday) "NOW" else dayLetters[i], style = MaterialTheme.typography.labelSmall,
                    fontSize = 7.sp, color = if (isToday) onBg else muted)
                Box(
                    modifier = Modifier.fillMaxWidth().aspectRatio(1f).background(
                        color = when {
                            hasActivity -> onBg.copy(alpha = 0.85f)
                            isToday -> outline.copy(alpha = 0.2f)
                            else -> outline.copy(alpha = 0.12f)
                        },
                        shape = RoundedCornerShape(4.dp)
                    )
                )
            }
        }
    }
}

@Composable
internal fun WeekDayRow(
    row: WeekActivityRow,
    today: LocalDate,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val rowDate = isoWeekStart.plusDays(row.dayOfWeek.toLong())
    val isFuture = rowDate.isAfter(today)
    Row(modifier = modifier, verticalAlignment = Alignment.Top) {
        Text(row.dayLabel, style = MaterialTheme.typography.labelSmall,
            color = if (isFuture) muted.copy(alpha = 0.25f) else muted,
            fontSize = 9.sp, modifier = Modifier.width(40.dp).padding(top = 2.dp))
        Column(modifier = Modifier.weight(1f)) {
            when {
                row.sessionName != null -> {
                    val displayName = if (row.hasPr) "${row.sessionName}  ● PR" else row.sessionName
                    Text(displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    val detail = buildList {
                        row.durationMin?.let { add("$it min") }
                        if (row.setCount > 0) add("${row.setCount} sets")
                    }.joinToString(" · ")
                    if (detail.isNotBlank()) {
                        Text(detail, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                    }
                }
                row.cardioType != null -> {
                    Text("Cardio · ${row.cardioType}", style = MaterialTheme.typography.bodyMedium, color = onBg)
                    val detail = buildList {
                        row.cardioDurationMin?.let { add("$it min") }
                        row.cardioDistanceKm?.let { add("${"%.1f".format(it)} km") }
                    }.joinToString(" · ")
                    if (detail.isNotBlank()) {
                        Text(detail, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                    }
                }
                isFuture -> Text("—", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.2f))
                else -> Text("rest", style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.6f), fontStyle = FontStyle.Italic)
            }
        }
        val tag = when {
            row.sessionName != null -> row.muscleWord
            row.cardioType != null -> "MOVE"
            else -> null
        }
        tag?.let {
            Text(it, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.padding(top = 2.dp))
        }
    }
}

@Composable
internal fun VsLastWeekRow(label: String, current: Int, previous: Int, muted: Color, onBg: Color, accent: Color) {
    val delta = current - previous
    val deltaText = when { delta > 0 -> "+$delta"; delta < 0 -> "$delta"; else -> "same" }
    val deltaColor = when { delta > 0 -> accent; delta < 0 -> muted.copy(alpha = 0.6f); else -> muted }
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, modifier = Modifier.weight(1f))
        Text(current.toString(), style = MaterialTheme.typography.bodySmall, color = onBg, modifier = Modifier.width(48.dp))
        Text(deltaText, style = MaterialTheme.typography.labelSmall, color = deltaColor, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
internal fun VolumeBarsSection(rows: List<MuscleVolume>, muted: Color, accent: Color, modifier: Modifier = Modifier) {
    val maxVol = rows.maxOfOrNull { it.volumeLb }?.coerceAtLeast(1.0) ?: 1.0
    val annotations = when (rows.size) {
        0 -> emptyList()
        1 -> listOf("biggest week")
        2 -> listOf("biggest week", "")
        else -> listOf("biggest week") + List(rows.size - 2) { "" } + listOf("falling behind")
    }
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        rows.forEachIndexed { i, row ->
            val fraction = (row.volumeLb / maxVol).toFloat().coerceIn(0.04f, 1f)
            val annotation = annotations.getOrElse(i) { "" }
            Column {
                Row(Modifier.fillMaxWidth(), Arrangement.SpaceBetween, Alignment.CenterVertically) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(row.muscle.displayName.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted)
                        if (annotation.isNotBlank()) {
                            Text(annotation, style = MaterialTheme.typography.labelSmall,
                                color = muted.copy(alpha = 0.5f), fontStyle = FontStyle.Italic)
                        }
                    }
                    Text("${formatVolume(row.volumeLb)} lb", style = MaterialTheme.typography.labelSmall, color = muted)
                }
                Spacer(Modifier.height(4.dp))
                Box(Modifier.fillMaxWidth(fraction).height(2.dp)
                    .background(if (i == 0) accent.copy(alpha = 0.75f) else muted.copy(alpha = 0.35f), RoundedCornerShape(1.dp)))
            }
        }
    }
}

@Composable
internal fun PrEntryRow(pr: PrEntry, muted: Color, onBg: Color, accent: Color, modifier: Modifier = Modifier) {
    val dateText = remember(pr.date) { SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(pr.date)).uppercase() }
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(dateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, modifier = Modifier.width(52.dp))
        Text(pr.exerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
        Text("${pr.weightText} × ${pr.reps}", style = MaterialTheme.typography.labelSmall, color = accent, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
internal fun HallOfFameRow(record: PrRecord, muted: Color, onBg: Color, accent: Color, modifier: Modifier = Modifier) {
    val dateText = remember(record.sessionDate) { SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(record.sessionDate)).uppercase() }
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Column(modifier = Modifier.weight(1f)) {
            Text(record.exerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(record.muscle.displayName.lowercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        }
        Text("${record.weightText} × ${record.bestReps}", style = MaterialTheme.typography.labelSmall,
            color = onBg.copy(alpha = 0.85f), fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(end = 12.dp))
        Text(dateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    }
}

@Composable
internal fun ExerciseFrequencySection(rows: List<ExerciseFrequency>, muted: Color, accent: Color, modifier: Modifier = Modifier) {
    val max = rows.maxOfOrNull { it.sessionCount }?.coerceAtLeast(1) ?: 1
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        rows.forEach { row ->
            val fraction = (row.sessionCount.toFloat() / max).coerceIn(0.04f, 1f)
            Column {
                Row(Modifier.fillMaxWidth(), Arrangement.SpaceBetween) {
                    Text(row.exerciseName, style = MaterialTheme.typography.labelSmall, color = muted)
                    Text("${row.sessionCount} sessions", style = MaterialTheme.typography.labelSmall, color = muted)
                }
                Spacer(Modifier.height(4.dp))
                Box(Modifier.fillMaxWidth(fraction).height(2.dp).background(muted.copy(alpha = 0.35f), RoundedCornerShape(1.dp)))
            }
        }
    }
}

@Composable
internal fun LifetimeStat(value: String, label: String, muted: Color, onBg: Color, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(value, style = MaterialTheme.typography.headlineSmall, color = onBg, fontWeight = FontWeight.Normal)
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}

internal fun numberWord(n: Int) = when (n) {
    0 -> "Zero"; 1 -> "One"; 2 -> "Two"; 3 -> "Three"; 4 -> "Four"; 5 -> "Five"
    6 -> "Six"; 7 -> "Seven"; 8 -> "Eight"; 9 -> "Nine"; 10 -> "Ten"
    11 -> "Eleven"; 12 -> "Twelve"; else -> n.toString()
}

internal fun formatVolume(lb: Double): String = "%,d".format(lb.toLong())

internal fun weekCommentary(sessions: Int, prs: Int): String = when {
    prs > 1 -> "$prs PRs this week."
    prs == 1 -> "One new PR."
    sessions >= 5 -> "Strong week."
    sessions >= 3 -> "Consistent week."
    sessions == 0 -> "Rest week."
    else -> ""
}

internal fun emptyWeekActivity(): List<WeekActivityRow> {
    val labels = listOf("MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN")
    return labels.mapIndexed { i, label -> WeekActivityRow(dayOfWeek = i, dayLabel = label) }
}
