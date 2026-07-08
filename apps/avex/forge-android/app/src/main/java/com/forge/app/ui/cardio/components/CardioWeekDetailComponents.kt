package com.forge.app.ui.cardio.components

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
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/** One session in a week's timeline — the day, the time-of-day it happened, and a compact line. */
@Composable
internal fun SessionTimelineRow(
    entry: CardioEntry,
    useMiles: Boolean,
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
        cardioDetailParts(entry, useMiles = useMiles).joinToString(" · ")
    }
    Row(
        modifier = Modifier.fillMaxWidth().clickableLabeled("View session details", onClick = onClick).padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(dayLabel, style = MaterialTheme.typography.labelMedium, color = muted, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.width(32.dp))
        Icon(type.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(20.dp))
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
        // §8 level ③ — navigation reads as the mono accent `action →` idiom.
        Text("stats →", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary, fontSize = 10.sp)
    }
}

/**
 * The watch-only "steps through the day" graph. Shows the hourly bars when a wearable actually fed
 * data; once Avex holds the steps grant ([connected]) but that day has none synced yet, shows a quiet
 * "no data yet" placeholder so the feature reads as live. When NOT connected it renders nothing — the
 * invitation to connect lives in the dismissible banner, not here.
 */
@Composable
internal fun StepsByHourSection(
    wearable: CardioWearableDay?,
    connected: Boolean,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val hasData = wearable != null && wearable.hasData
    if (!hasData && !connected) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp)) {
        EditorialHeader(label = "Steps through the day", muted = muted, accent = accent)
        Spacer(Modifier.height(10.dp))
        if (hasData) {
            HourlyStepsBars(wearable!!, muted = muted, outline = outline, accent = accent)
        } else {
            // Connected but nothing synced yet — the section's own mark at zero (§12): ghost hour bars.
            VerticalBarRow(
                count = 24,
                trackHeight = 56.dp,
                modifier = Modifier.fillMaxWidth(),
                spacing = 2.dp,
                corner = 4.dp,
                bar = { BarGeom(height = 3.dp, fill = outline.copy(alpha = 0.25f)) }
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "No steps from your watch for this day yet.",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
            )
        }
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
            corner = 4.dp,
            bar = { i ->
                val steps = byHour[i]
                BarGeom(
                    height = (3 + (steps.toFloat() / max) * 52).dp,
                    fill = if (steps > 0) accent else outline.copy(alpha = 0.25f)
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
