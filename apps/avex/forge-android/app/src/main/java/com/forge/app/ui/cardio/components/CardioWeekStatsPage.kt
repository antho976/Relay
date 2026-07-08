package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioWeekAggregate
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHairline
import java.time.ZoneId
import java.util.Locale

/**
 * One page of the swipeable week stats — the per-day bar graph, a tile grid of the week's numbers,
 * an activity-type breakdown graph, the goal/streak (current week only), the optional wearable steps
 * graph, and that week's session list. One [CardioWeekAggregate] in, a scrollable page out.
 */
@Composable
internal fun CardioWeekStatsPage(
    agg: CardioWeekAggregate,
    weekEntries: List<CardioEntry>,
    useMiles: Boolean,
    isCurrentWeek: Boolean,
    todayDow: Int,
    weekTargetMin: Int,
    cardioStreakDays: Int,
    wearable: CardioWearableDay?,
    /** Avex holds the steps grant — show the steps section (with a placeholder) even before data syncs. */
    wearableConnected: Boolean,
    bodyweightLb: Double?,
    zone: ZoneId,
    onOpenSession: (Long) -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val ordered = remember(weekEntries) { weekEntries.sortedBy { it.date } }
    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 40.dp)
    ) {
        // ── Per-day bar graph ───────────────────────────────────────────────
        PerDayBars(
            perDayMinutes = agg.perDayMinutes,
            todayDow = if (isCurrentWeek) todayDow else -1,
            onBg = onBg, muted = muted, outline = outline
        )

        // ── Open figures ── serif number + tiny mono label, no card shell ─────
        val avgPace = pacePerUnit(agg.minutes, agg.distanceKm, useMiles)
        val distUnit = distanceUnitLabel(useMiles)
        val figures = buildList {
            add("${agg.days}" to if (agg.days == 1) "day" else "days")
            add("${agg.sessions}" to "sessions")
            add("${agg.minutes}" to "minutes")
            if (agg.distanceKm > 0) add(String.format(Locale.US, "%.1f", toDisplayDistance(agg.distanceKm, useMiles)) to distUnit)
            if (avgPace != null) add(avgPace to "/$distUnit avg")
        }
        EditorialHairline(outline = outline, modifier = Modifier.padding(horizontal = 24.dp))
        Spacer(Modifier.height(14.dp))
        figures.chunked(3).forEach { rowFigs ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(0.dp)
            ) {
                rowFigs.forEachIndexed { idx, (value, label) ->
                    EditorialFigure(
                        value = value,
                        label = label,
                        onBg = onBg, muted = muted, accent = accent,
                        modifier = Modifier.weight(1f)
                    )
                    if (idx < rowFigs.lastIndex) {
                        // Even 16dp on both sides so a long figure never touches the rule.
                        Spacer(Modifier.width(16.dp))
                        Box(Modifier.width(1.dp).height(40.dp).background(outline.copy(alpha = 0.25f)).align(Alignment.CenterVertically))
                        Spacer(Modifier.width(16.dp))
                    }
                }
                repeat(3 - rowFigs.size) { Spacer(Modifier.weight(1f)) }
            }
            Spacer(Modifier.height(14.dp))
        }
        EditorialHairline(outline = outline, modifier = Modifier.padding(horizontal = 24.dp))

        // ── Activity breakdown graph ────────────────────────────────────────
        if (agg.minutesByType.isNotEmpty()) {
            Spacer(Modifier.height(12.dp))
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                Text("BY ACTIVITY", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(10.dp))
                val maxMin = agg.minutesByType.first().second.coerceAtLeast(1)
                agg.minutesByType.forEach { (type, min) ->
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text(type.displayName, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp, modifier = Modifier.width(72.dp))
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(8.dp)
                                .clip(RoundedCornerShape(4.dp))
                                .background(outline.copy(alpha = 0.15f))
                        ) {
                            Box(
                                Modifier.fillMaxWidth(min.toFloat() / maxMin).height(8.dp)
                                    .clip(RoundedCornerShape(4.dp)).background(accent.copy(alpha = 0.7f))
                            )
                        }
                        Text("$min", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, modifier = Modifier.width(36.dp))
                    }
                    Spacer(Modifier.height(8.dp))
                }
            }
        }

        // ── Goal + streak (current week only) ───────────────────────────────
        if (isCurrentWeek && (weekTargetMin > 0 || cardioStreakDays >= 2)) {
            Spacer(Modifier.height(4.dp))
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                if (cardioStreakDays >= 2) {
                    Text("$cardioStreakDays-day cardio streak", style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.height(8.dp))
                }
                if (weekTargetMin > 0) {
                    val frac = (agg.minutes.toFloat() / weekTargetMin).coerceIn(0f, 1f)
                    Box(Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(muted.copy(alpha = 0.2f))) {
                        Box(Modifier.fillMaxWidth(frac).height(6.dp).clip(RoundedCornerShape(3.dp)).background(onBg))
                    }
                    Spacer(Modifier.height(4.dp))
                    Text(
                        if (agg.minutes >= weekTargetMin) "Weekly goal hit — ${agg.minutes} / $weekTargetMin min"
                        else "${agg.minutes} / $weekTargetMin min this week",
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                    )
                }
            }
        }

        // ── Wearable steps (data, or a placeholder once connected) ──────────
        StepsByHourSection(wearable = wearable, connected = wearableConnected, onBg = onBg, muted = muted, outline = outline, accent = accent)

        // ── That week's sessions ────────────────────────────────────────────
        Spacer(Modifier.height(12.dp))
        if (ordered.isEmpty()) {
            Text(
                "Nothing logged this week.",
                style = MaterialTheme.typography.bodyMedium, color = muted,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
            )
        } else {
            Text("SESSIONS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp, modifier = Modifier.padding(horizontal = 24.dp))
            Spacer(Modifier.height(4.dp))
            ordered.forEach { entry ->
                SessionTimelineRow(
                    entry = entry, bodyweightLb = bodyweightLb, useMiles = useMiles, zone = zone,
                    onBg = onBg, muted = muted, onClick = { onOpenSession(entry.id) }
                )
                HorizontalDivider(modifier = Modifier.padding(horizontal = 24.dp), color = outline.copy(alpha = 0.12f))
            }
        }
    }
}

@Composable
private fun PerDayBars(perDayMinutes: List<Int>, todayDow: Int, onBg: Color, muted: Color, outline: Color) {
    val letters = listOf("M", "T", "W", "T", "F", "S", "S")
    val maxMin = (perDayMinutes.maxOrNull() ?: 0).coerceAtLeast(1)
    VerticalBarRow(
        count = 7,
        trackHeight = 56.dp,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
        bar = { i ->
            val mins = perDayMinutes.getOrElse(i) { 0 }
            val frac = (mins.toFloat() / maxMin).coerceIn(0f, 1f)
            if (mins > 0) BarGeom(height = (8 + 48 * frac).dp, fill = onBg)
            else BarGeom(height = 4.dp, fill = outline.copy(alpha = 0.3f))
        },
        top = { i ->
            val mins = perDayMinutes.getOrElse(i) { 0 }
            if (mins > 0) Text("${mins}m", fontSize = 9.sp, color = onBg, fontWeight = FontWeight.SemiBold)
        },
        bottom = { i ->
            Text(
                letters[i], fontSize = 9.sp,
                color = if (i == todayDow) onBg else muted,
                fontWeight = if (i == todayDow) FontWeight.Bold else FontWeight.Normal
            )
        }
    )
}

