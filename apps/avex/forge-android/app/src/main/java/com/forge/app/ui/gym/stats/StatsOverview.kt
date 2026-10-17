package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.rowShape
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.components.musclesReadout
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.StatsUiState
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/**
 * The Stats hero + the always-on Records list. The hero mirrors the session-detail header — the
 * page's "face": THIS WEEK's three serif figures (volume · sessions · sets, with vs-last deltas) on
 * the left, the compact weekly muscle map on the right, and the one-line readiness status underneath
 * (2026-07-01 fusion — the old top-lift hero dissolved into the Strength lens, where it's simply the
 * first row).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ColumnScope.StatsHeroContent(state: StatsUiState, weightUnit: WeightUnit, c: StatsColors) {
    val cmp = state.weekComparison
    val cur = cmp?.current
    val prev = cmp?.previous
    // A delta needs a baseline. Against an empty last week every figure read "▲ n vs last", three
    // identical accent lines that said nothing, so a figure only carries its arrow once last week
    // had something to compare against.
    val volumeDelta = if (prev != null && prev.volumeLb > 0.0)
        toDisplayWeight(cmp.volumeDelta, weightUnit).roundToInt() else null
    val sessionsDelta = if (prev != null && prev.sessions > 0) cmp.sessionsDelta else null
    val setsDelta = if (prev != null && prev.sets > 0) cur!!.sets - prev.sets else null
    val anyDelta = listOfNotNull(volumeDelta, sessionsDelta, setsDelta).any { it != 0 }

    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                EditorialHeader(label = "This week", muted = c.muted, accent = c.accent, modifier = Modifier.weight(1f))
                // What the arrows are measured against, said once in the header rather than
                // repeated under every figure (Home's THIS WEEK carries its meta the same way).
                if (anyDelta) {
                    Text("VS LAST WEEK BY NOW", style = MaterialTheme.typography.labelSmall, color = c.muted.copy(alpha = 0.65f))
                }
            }
            Spacer(Modifier.height(14.dp))
            // The shared figure, so these read exactly like History's and Home's. Honest zeros on a
            // first run or a quiet week (§12), never a dash. Wraps rather than clips at large font.
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                EditorialFigure(
                    value = formatVolumeCompact(cur?.volumeLb ?: 0.0, weightUnit),
                    label = "Volume",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = volumeDelta,
                    deltaText = cmp?.takeIf { volumeDelta != null }?.let { formatVolumeCompact(kotlin.math.abs(it.volumeDelta), weightUnit, withUnit = false) }
                )
                EditorialFigure(
                    value = "${cur?.sessions ?: 0}",
                    label = if (cur?.sessions == 1) "Session" else "Sessions",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = sessionsDelta
                )
                EditorialFigure(
                    value = "${cur?.sets ?: 0}",
                    label = if (cur?.sets == 1) "Set" else "Sets",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = setsDelta
                )
            }
        }
        // The weekly muscle map as the header's face — same spot as the session screen's body figure.
        // Always drawn: at zero it's the faint silhouette (the section's own visual at zero), so the hero
        // carries a mark before the first log instead of a gap (§12).
        Spacer(Modifier.width(16.dp))
        // The map counts a rolling 7 days, while the figures beside it are the calendar week, so it
        // says so rather than sitting silently under "This week".
        val mapSets = state.weeklySetsByMuscle.associate { it.muscle to it.sets }
        Column(
            Modifier.width(108.dp).chartReadout("Last 7 days: ${musclesReadout(mapSets)}."),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            BodyHeatmap(
                setsByMuscle = mapSets,
                accent = c.accent,
                faint = c.outline.copy(alpha = 0.35f),
                silhouette = c.outline.copy(alpha = 0.25f),
                labelColor = c.muted,
                figureHeight = 104.dp,
                showLegend = false,
                showTitles = false,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(4.dp))
            Text("LAST 7 DAYS", style = MaterialTheme.typography.labelSmall, color = c.muted.copy(alpha = 0.65f))
        }
    }
    if (state.readinessPulse != null && state.readinessThreshold != null) {
        Spacer(Modifier.height(16.dp))
        ReadinessLine(state.readinessPulse, state.readinessThreshold, c)
    }
}

/** Which way the Records list is ordered. Newest leads: a fresh record is the one worth seeing. */
private enum class RecordSort(val label: String) { NEWEST("Newest"), HEAVIEST("Heaviest") }

/** A record set this recently carries the accent flag beside its date. */
private const val FRESH_DAYS = 14L

/** Weeks the "when they were set" strip covers, newest on the right. */
private const val RECORD_WEEKS = 12

/**
 * Records: the heaviest set standing on each lift.
 *
 * Rebuilt 2026-10-17 (Antho: "redo the record section"). It was a grey slab of every lift ordered
 * by load, so the same heavy lifts sat on top forever and a record set last Tuesday was buried in
 * the middle. Now it leads with a mark, the twelve weeks the standing records were set in, so a
 * run of new ones is visible at a glance; the list opens newest first with a fresh record flagged,
 * and the same rows can be ordered by load instead. Rows still tap through to the lift's trend.
 */
@Composable
internal fun ColumnScope.RecordsContent(
    records: List<PrRecord>,
    weightUnit: WeightUnit,
    c: StatsColors,
    onOpenLift: (String) -> Unit = {},
    /** Lifts whose Strength row can expand; a record outside it isn't tappable. Null = all are. */
    drillableIds: Set<String>? = null
) {
    val today = com.forge.app.ui.gym.stats.components.rememberToday()
    val zone = remember { java.time.ZoneId.systemDefault() }
    val locale = com.forge.app.ui.common.currentLocale()
    val fmt = remember(locale) { SimpleDateFormat("MMM d", locale) }
    var showAll by rememberSaveable { mutableStateOf(false) }
    var sort by rememberSaveable { mutableStateOf(RecordSort.NEWEST) }
    val sorted = remember(records, sort) {
        when (sort) {
            RecordSort.NEWEST -> records.sortedByDescending { it.sessionDate }
            RecordSort.HEAVIEST -> records.sortedByDescending { it.maxWeightLb }
        }
    }
    fun daysAgo(ms: Long): Long = java.time.temporal.ChronoUnit.DAYS.between(
        java.time.Instant.ofEpochMilli(ms).atZone(zone).toLocalDate(), today
    ).coerceAtLeast(0)

    RecordWeeksStrip(records, today, zone, c)
    Spacer(Modifier.height(14.dp))
    com.forge.app.ui.common.ForgeSlidingSegments(
        options = RecordSort.entries.map { it.label },
        selectedIndex = sort.ordinal,
        onSelect = { sort = RecordSort.entries[it] }
    )
    Spacer(Modifier.height(10.dp))
    val shown = if (showAll) sorted else sorted.take(LIFTS_SHOWN)
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        shown.forEachIndexed { i, r ->
            val ago = daysAgo(r.sessionDate)
            val fresh = ago <= FRESH_DAYS
            val whenText = when {
                ago == 0L -> "Today"
                ago == 1L -> "Yesterday"
                ago < FRESH_DAYS -> "$ago days ago"
                else -> fmt.format(Date(r.sessionDate))
            }
            Row(
                Modifier.fillMaxWidth()
                    .clip(rowShape(i, shown.size))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .then(
                        if (drillableIds == null || r.exerciseId in drillableIds)
                            Modifier.clickableLabeled("Show estimated 1RM trend for ${r.exerciseName}") { onOpenLift(r.exerciseId) }
                        else Modifier
                    )
                    .padding(horizontal = ROW_H, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(Modifier.weight(1f)) {
                    Text(r.exerciseName, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                    Spacer(Modifier.height(2.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        // The flag is the exception only: a record from the last two weeks.
                        if (fresh) {
                            androidx.compose.foundation.layout.Box(
                                Modifier.size(6.dp).clip(androidx.compose.foundation.shape.CircleShape).background(c.accent)
                            )
                            Spacer(Modifier.width(6.dp))
                        }
                        Text(
                            whenText,
                            style = MaterialTheme.typography.bodySmall,
                            color = if (fresh) c.onBg else c.muted
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(horizontalAlignment = Alignment.End) {
                    // onBg, not accent: a record is content, and accent body text fails contrast under
                    // most accents (§14). formatWeight keeps the real load ("7.5 kg", not "7 kg").
                    Text(
                        "${formatWeight(r.maxWeightLb, weightUnit)} × ${r.bestReps}",
                        style = MaterialTheme.typography.titleSmall,
                        color = c.onBg
                    )
                    r.relativeStrength?.let { rel ->
                        Text("${rel}× bodyweight", style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
            }
        }
    }
    if (records.size > LIFTS_SHOWN) {
        StatsShowAll(label = if (showAll) "top $LIFTS_SHOWN only" else "all ${records.size} records", c = c) { showAll = !showAll }
    }
}

/**
 * When the standing records were set: one bar per week for the last twelve, its height the number
 * of lifts whose current record dates from that week. Works at zero as twelve empty tracks.
 */
@Composable
private fun RecordWeeksStrip(
    records: List<PrRecord>,
    today: java.time.LocalDate,
    zone: java.time.ZoneId,
    c: StatsColors
) {
    val thisMonday = today.with(java.time.DayOfWeek.MONDAY)
    val counts = remember(records, thisMonday) {
        IntArray(RECORD_WEEKS).also { arr ->
            records.forEach { r ->
                val day = java.time.Instant.ofEpochMilli(r.sessionDate).atZone(zone).toLocalDate()
                val weeksBack = java.time.temporal.ChronoUnit.WEEKS.between(day.with(java.time.DayOfWeek.MONDAY), thisMonday).toInt()
                if (weeksBack in 0 until RECORD_WEEKS) arr[RECORD_WEEKS - 1 - weeksBack]++
            }
        }
    }
    val recent = records.count {
        java.time.temporal.ChronoUnit.DAYS.between(
            java.time.Instant.ofEpochMilli(it.sessionDate).atZone(zone).toLocalDate(), today
        ) in 0..29
    }
    val max = (counts.maxOrNull() ?: 0).coerceAtLeast(1)
    Row(verticalAlignment = Alignment.Bottom) {
        Text(
            "$recent",
            style = MaterialTheme.typography.headlineMedium,
            color = c.onBg
        )
        Spacer(Modifier.width(8.dp))
        Text(
            "set in the last 30 days, of ${records.size} ${if (records.size == 1) "lift" else "lifts"}",
            style = MaterialTheme.typography.bodySmall,
            color = c.muted,
            modifier = Modifier.padding(bottom = 6.dp)
        )
    }
    Spacer(Modifier.height(10.dp))
    Row(
        Modifier
            .fillMaxWidth()
            .height(40.dp)
            .chartReadout(
                "Records set per week over the last $RECORD_WEEKS weeks, oldest first: " +
                    counts.joinToString(", ") + ". This week: ${counts.last()}."
            ),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        counts.forEachIndexed { i, n ->
            androidx.compose.foundation.layout.Box(
                Modifier.weight(1f).fillMaxHeight(),
                contentAlignment = Alignment.BottomCenter
            ) {
                // The empty track is the zero shape; a week's records fill it from the floor.
                androidx.compose.foundation.layout.Box(
                    Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp))
                        .background(c.muted.copy(alpha = 0.35f))
                )
                if (n > 0) {
                    androidx.compose.foundation.layout.Box(
                        Modifier.fillMaxWidth()
                            .fillMaxHeight(n.toFloat() / max)
                            .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                            .background(if (i == counts.lastIndex) c.accent else c.accent.copy(alpha = 0.6f))
                    )
                }
            }
        }
    }
    Spacer(Modifier.height(4.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text("$RECORD_WEEKS WEEKS AGO", style = MaterialTheme.typography.labelSmall, color = c.muted)
        Text("THIS WEEK", style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}
