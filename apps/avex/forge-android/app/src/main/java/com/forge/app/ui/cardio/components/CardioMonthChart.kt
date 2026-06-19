package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale

/**
 * A compact six-month bar chart of active cardio minutes — the longer-arc companion to the
 * week row. Rest entries are excluded (they aren't "minutes moved"). Renders nothing until at
 * least one month has activity, so a brand-new log doesn't show six empty bars.
 */
@Composable
internal fun CardioMonthChart(
    entries: List<CardioEntry>,
    zone: ZoneId,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val today = LocalDate.now(zone)
    val months = remember(today.month, today.year) {
        (5 downTo 0).map { YearMonth.from(today).minusMonths(it.toLong()) }
    }
    val byMonth = remember(entries) {
        entries.asSequence()
            .filter { it.type != CardioType.REST.code }
            .groupBy { YearMonth.from(Instant.ofEpochMilli(it.date).atZone(zone).toLocalDate()) }
            .mapValues { (_, list) -> list.sumOf { it.durationMin } }
    }
    val data = months.map { it to (byMonth[it] ?: 0) }
    if (data.all { it.second == 0 }) return

    val max = (data.maxOf { it.second }).coerceAtLeast(1)

    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp)) {
        Text("LAST 6 MONTHS · MINUTES", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        Spacer(Modifier.height(10.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            data.forEach { (month, minutes) ->
                val frac = minutes.toFloat() / max
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        if (minutes > 0) "$minutes" else "",
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp
                    )
                    Spacer(Modifier.height(2.dp))
                    Box(
                        Modifier
                            .fillMaxWidth(0.7f)
                            .height((4 + frac * 56).dp)
                            .clip(RoundedCornerShape(3.dp))
                            .background(if (minutes > 0) accent.copy(alpha = 0.7f) else outline.copy(alpha = 0.3f))
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        month.month.getDisplayName(TextStyle.SHORT, Locale.getDefault()).take(1),
                        style = MaterialTheme.typography.labelSmall,
                        color = if (month == YearMonth.from(today)) onBg else muted.copy(alpha = 0.7f),
                        fontSize = 9.sp
                    )
                }
            }
        }
    }
}
