package com.forge.app.ui.cardio.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
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
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.cardioDetailParts
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/** One session in a week's timeline — the day, the time-of-day it happened, and a compact line. */
@Composable
internal fun SessionTimelineRow(
    entry: CardioEntry,
    bodyweightLb: Double?,
    zone: ZoneId,
    onBg: Color,
    muted: Color,
    onClick: () -> Unit
) {
    val type = CardioType.fromCode(entry.type)
    val dayLabel = remember(entry.date) {
        Instant.ofEpochMilli(entry.date).atZone(zone).dayOfWeek
            .getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase().take(3)
    }
    val timeLabel = remember(entry.date, entry.durationMin) {
        sessionTimeLabel(entry.date, if (type.isRest) 0 else entry.durationMin, zone)
    }
    val detail = if (type.isRest) {
        CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day"
    } else {
        cardioDetailParts(entry, type, bodyweightLb).joinToString(" · ")
    }
    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(dayLabel, style = MaterialTheme.typography.labelMedium, color = onBg.copy(alpha = 0.85f), fontSize = 10.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.width(32.dp))
        Icon(type.icon, contentDescription = null, tint = onBg.copy(alpha = 0.9f), modifier = Modifier.size(20.dp))
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(type.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                if (timeLabel.isNotBlank()) {
                    Text(timeLabel, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                }
            }
            if (detail.isNotBlank()) {
                Text(detail, style = MaterialTheme.typography.labelSmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 10.sp)
            }
        }
        Text("stats ›", style = MaterialTheme.typography.labelSmall, color = onBg.copy(alpha = 0.7f), fontSize = 10.sp)
    }
}

/**
 * The watch-only "steps through the day" graph — rendered ONLY when a wearable actually fed data.
 * The invitation to connect a wearable now lives in the dismissible banner, so there's no inline
 * placeholder here. Stays dormant until the Health Connect steps read is wired in.
 */
@Composable
internal fun StepsByHourSection(
    wearable: CardioWearableDay?,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    if (wearable == null || !wearable.hasData) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp)) {
        Text("STEPS THROUGH THE DAY", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(10.dp))
        HourlyStepsBars(wearable, muted = muted, outline = outline, accent = accent)
    }
}

/** 24 thin bars (one per hour) of an active-steps day; the busiest hour is labelled. */
@Composable
private fun HourlyStepsBars(
    wearable: CardioWearableDay,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val byHour = remember(wearable) { (0..23).map { h -> wearable.hourlySteps.firstOrNull { it.hour == h }?.steps ?: 0 } }
    val actualMax = byHour.maxOrNull() ?: 0
    val max = actualMax.coerceAtLeast(1)
    val peakHour = byHour.indexOf(actualMax).coerceAtLeast(0)
    Column {
        VerticalBarRow(
            count = 24,
            trackHeight = 56.dp,
            modifier = Modifier.fillMaxWidth(),
            spacing = 2.dp,
            corner = 2.dp,
            bar = { i ->
                val steps = byHour[i]
                BarGeom(
                    height = (3 + (steps.toFloat() / max) * 52).dp,
                    fill = if (steps > 0) accent.copy(alpha = 0.7f) else outline.copy(alpha = 0.25f)
                )
            }
        )
        Spacer(Modifier.height(8.dp))
        Text(
            "${"%,d".format(wearable.totalSteps)} steps · busiest ${hourRangeLabel(peakHour)}",
            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
        )
    }
}

/** "6:30 AM – 7:10 AM" for a timed session, "6:30 AM" when there's no duration, "" for rest. */
private fun sessionTimeLabel(startMs: Long, durationMin: Int, zone: ZoneId): String {
    val fmt = DateTimeFormatter.ofPattern("h:mm a", Locale.getDefault())
    val start = Instant.ofEpochMilli(startMs).atZone(zone)
    val startStr = start.format(fmt)
    if (durationMin <= 0) return startStr
    val end = start.plusMinutes(durationMin.toLong())
    return "$startStr – ${end.format(fmt)}"
}

/** "4–5am" style label for the peak step hour. */
private fun hourRangeLabel(hour: Int): String {
    fun fmt(h: Int): String {
        val period = if (h < 12) "am" else "pm"
        val h12 = when (h % 12) { 0 -> 12; else -> h % 12 }
        return "$h12$period"
    }
    return "${fmt(hour)}–${fmt((hour + 1) % 24)}"
}
