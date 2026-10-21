package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.cardio.cardioDetailParts
import com.forge.app.domain.cardio.distanceStanding
import com.forge.app.domain.cardio.formatInclinePct
import com.forge.app.domain.cardio.longestInWeeks
import com.forge.app.domain.cardio.paceStanding
import com.forge.app.domain.cardio.readsAsSpeed
import com.forge.app.domain.cardio.usualDistanceKm
import com.forge.app.domain.cardio.usualDurationMin
import com.forge.app.domain.health.HrPoint
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.domain.health.avgBpm
import com.forge.app.domain.health.maxBpm
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatElevation
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.cardio.CARDIO_GUTTER
import com.forge.app.ui.cardio.CardioAreaChart
import com.forge.app.ui.cardio.CardioBodyLine
import com.forge.app.ui.cardio.CardioChange
import com.forge.app.ui.cardio.CardioDangerButton
import com.forge.app.ui.cardio.CardioHead
import com.forge.app.ui.cardio.CardioQuietButton
import com.forge.app.ui.cardio.CardioReading
import com.forge.app.ui.cardio.CardioStripPlot
import com.forge.app.ui.cardio.CardioTopRow
import com.forge.app.ui.cardio.LocalCardioTypes
import com.forge.app.ui.cardio.cardioFigure
import com.forge.app.ui.cardio.distanceNumber
import com.forge.app.ui.cardio.formatCount
import com.forge.app.ui.cardio.ordinal
import com.forge.app.ui.cardio.sessionFigure
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle as JavaTextStyle
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

/** The air between sections on a session's page, as drawn. */
private val SECTION = 40.dp

/**
 * ONE cardio session (redesign 2026-10-21, "Session"): its date, the activity as the serif title,
 * Time / Distance / Pace with how each sits against your usual, the conditions line, how it ranks
 * against the same activity's other sessions as two strip plots, and the watch's heart rate. Then
 * what the watch measured (with Use watch stats), the route, that day's steps, the note, and Edit /
 * Delete. The ⋮ menu carries the same actions for a reach from the top.
 */
@Composable
fun CardioSessionDetailSheet(
    entry: CardioEntry,
    /** Every logged entry — the same-activity pool the standings read against. */
    allEntries: List<CardioEntry> = emptyList(),
    useMiles: Boolean = false,
    /** GPS track (watch-only); non-null once the matching session's route is available to draw. */
    route: List<RoutePoint>? = null,
    /** Non-null when a matching watch session has a route that needs Health Connect consent first. */
    onShowRoute: (() -> Unit)? = null,
    /** Watch steps for the session's day; null until loaded / when none. */
    wearable: CardioWearableDay? = null,
    /** Avex holds the steps grant — show that day's steps even before data syncs. */
    wearableConnected: Boolean = false,
    /** Downsampled HR series of the matched watch workout; two points or more draws the graph. */
    hr: List<HrPoint>? = null,
    /** AVG / MAX of the RAW series behind [hr]; null falls back to reading them off [hr]. */
    hrAvgBpm: Int? = null,
    hrMaxBpm: Int? = null,
    /** The matched watch workout's measured stats (W5). */
    watchStats: WatchWorkout? = null,
    /** Adopt the watch's measured duration/distance onto this entry — offered only when they differ. */
    onAdoptWatchStats: (() -> Unit)? = null,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onBack: () -> Unit,
    /** The zone the page reads dates in; the caller's, so a test can pin it. */
    zone: ZoneId = ZoneId.systemDefault(),
) {
    val activity = CardioActivity.resolve(entry.type, LocalCardioTypes.current)
    val locale = currentLocale()
    val use24h = LocalForgeSettings.current.timeFormat24h
    val dateLine = remember(entry.date, use24h, locale, zone) { sessionDateLine(entry.date, use24h, locale, zone) }
    val watchParts = if (!activity.isRest) watchStatParts(watchStats, entry, useMiles) else emptyList()
    val canAdopt = onAdoptWatchStats != null && watchParts.any { !it.endsWith("kcal") }
    var menu by remember { mutableStateOf(false) }

    LazyColumn(
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = CARDIO_GUTTER, end = CARDIO_GUTTER, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            CardioTopRow(onBack = onBack, trailing = {
                Box {
                    OpenTopBarIcon(OpenIcons.MoreVertical, "More for this session", { menu = true })
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text("Edit", style = OpenType.body) }, onClick = { menu = false; onEdit() })
                        if (canAdopt) {
                            DropdownMenuItem(text = { Text("Use watch stats", style = OpenType.body) }, onClick = { menu = false; onAdoptWatchStats?.invoke() })
                        }
                        if (route == null && onShowRoute != null && !activity.isRest) {
                            DropdownMenuItem(text = { Text("Show GPS route", style = OpenType.body) }, onClick = { menu = false; onShowRoute() })
                        }
                        DropdownMenuItem(
                            text = { Text("Delete", style = OpenType.body, color = MaterialTheme.colorScheme.error) },
                            onClick = { menu = false; onDelete() }
                        )
                    }
                }
            })
        }
        item("title") {
            Column {
                Spacer(Modifier.height(20.dp))
                CardioChange(dateLine, moved = false)
                Spacer(Modifier.height(8.dp))
                // A long custom name steps down a rung rather than running off the page.
                Text(
                    activity.displayName,
                    style = if (activity.displayName.length > 9) OpenType.titleM else OpenType.titleL,
                    color = MaterialTheme.colorScheme.onBackground
                )
            }
        }

        if (activity.isRest) {
            item("rest") {
                Column {
                    Spacer(Modifier.height(24.dp))
                    CardioBodyLine(CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day", color = MaterialTheme.colorScheme.onBackground)
                }
            }
        } else {
            item("figures") {
                Column {
                    Spacer(Modifier.height(24.dp))
                    SessionReadings(entry, allEntries, useMiles, zone)
                }
            }
            val tags = sessionTags(entry, useMiles)
            if (tags.isNotEmpty()) {
                item("tags") {
                    Column {
                        Spacer(Modifier.height(14.dp))
                        CardioChange(tags.joinToString(" · "), moved = false)
                    }
                }
            }
            item("standing") { StandingSection(entry, allEntries, activity, useMiles, locale, zone) }

            if (hr != null && hr.size >= 2) {
                item("heart-rate") {
                    Column {
                        Spacer(Modifier.height(SECTION))
                        HeartRateSection(hr, hrAvgBpm ?: hr.avgBpm(), hrMaxBpm ?: hr.maxBpm())
                    }
                }
            }
            if (watchParts.isNotEmpty()) {
                item("watch-stats") {
                    Column {
                        Spacer(Modifier.height(if (hr != null && hr.size >= 2) 20.dp else SECTION))
                        CardioBodyLine("Your watch measured ${watchParts.joinToString(" · ")}.")
                        if (canAdopt) {
                            Spacer(Modifier.height(4.dp))
                            CardioQuietButton("Use watch stats", onClick = { onAdoptWatchStats?.invoke() })
                        }
                    }
                }
            }
            if (route != null && route.size >= 2) {
                item("route") {
                    Column {
                        Spacer(Modifier.height(SECTION))
                        CardioHead("Route", "From your watch")
                        Spacer(Modifier.height(14.dp))
                        RouteThumbnail(route = route, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.fillMaxWidth().height(160.dp))
                    }
                }
            } else if (onShowRoute != null) {
                item("route-cta") {
                    Column {
                        Spacer(Modifier.height(SECTION))
                        CardioHead("Route", "From your watch")
                        Spacer(Modifier.height(8.dp))
                        CardioQuietButton("Show GPS route", onClick = onShowRoute)
                    }
                }
            }
            if (wearable?.hasData == true || wearableConnected) {
                item("steps") {
                    Column {
                        Spacer(Modifier.height(SECTION))
                        DaySteps(wearable, entry, zone, locale)
                    }
                }
            }
        }

        entry.note?.takeIf { it.isNotBlank() }?.let { note ->
            item("note") {
                Column {
                    Spacer(Modifier.height(SECTION))
                    OpenLabel("Note")
                    Spacer(Modifier.height(10.dp))
                    CardioBodyLine(note, color = MaterialTheme.colorScheme.onBackground)
                }
            }
        }

        item("actions") {
            Column {
                Spacer(Modifier.height(SECTION))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                    CardioQuietButton("Edit", onClick = onEdit)
                    CardioDangerButton("Delete", onClick = onDelete)
                }
            }
        }
    }
}

/** "Mon Oct 19 · 6:10 am", with the year when it is not this one. */
internal fun sessionDateLine(dateMs: Long, use24h: Boolean, locale: Locale, zone: ZoneId): String {
    val at = Instant.ofEpochMilli(dateMs).atZone(zone)
    val thisYear = Instant.now().atZone(zone).year
    val day = DateTimeFormatter.ofPattern(if (at.year == thisYear) "EEE MMM d" else "EEE MMM d, yyyy", locale)
    val clock = DateTimeFormatter.ofPattern(com.forge.app.domain.units.clockPattern(use24h), locale)
    return "${at.format(day)} · ${at.format(clock).lowercase(locale)}"
}

/** The three readings a session is judged on, each with how it sits against your usual. */
@Composable
private fun SessionReadings(entry: CardioEntry, all: List<CardioEntry>, useMiles: Boolean, zone: ZoneId) {
    val unit = distanceUnitLabel(useMiles)
    val usualMin = remember(entry, all) { usualDurationMin(entry, all) }
    val timeChange = usualMin?.let { u ->
        val d = entry.durationMin - u
        when {
            d > 0 -> "↑ $d vs usual" to true
            d < 0 -> "↓ ${-d} vs usual" to true
            else -> "Your usual" to false
        }
    }
    val distance = entry.distanceKm?.takeIf { it > 0.0 }
    val distChange = remember(entry, all, useMiles, zone) {
        if (distance == null) null else {
            val weeks = longestInWeeks(entry, all, zone)
            when {
                weeks == Int.MAX_VALUE -> "Longest yet" to true
                weeks != null -> "Longest in $weeks wk" to true
                else -> usualDistanceKm(entry, all)?.let { u ->
                    val d = toDisplayDistance(distance, useMiles) - toDisplayDistance(u, useMiles)
                    val t = (abs(d) * 10).roundToInt() / 10.0
                    if (t == 0.0) "Your usual" to false
                    else "${if (d > 0) "↑" else "↓"} ${String.format(Locale.US, "%.1f", t)} vs usual" to true
                }
            }
        }
    }
    val figure = sessionFigure(entry, useMiles)
    val speed = readsAsSpeed(entry.type)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
        CardioReading(
            label = "Time",
            figure = cardioFigure("${entry.durationMin}", " min", CardioOpenType.figureMUnit),
            change = timeChange?.first,
            moved = timeChange?.second ?: false,
            description = "${entry.durationMin} minutes${timeChange?.let { ", ${it.first}" } ?: ""}",
            modifier = Modifier.weight(1f)
        )
        if (distance != null) {
            val n = distanceNumber(distance, useMiles)
            CardioReading(
                label = "Distance",
                figure = cardioFigure(n, " $unit", CardioOpenType.figureMUnit),
                change = distChange?.first,
                moved = distChange?.second ?: false,
                description = "$n $unit${distChange?.let { ", ${it.first}" } ?: ""}",
                modifier = Modifier.weight(1f)
            )
        } else {
            Spacer(Modifier.weight(1f))
        }
        if (figure != null) {
            CardioReading(
                label = if (speed) "Speed" else "Pace",
                figure = cardioFigure(figure.first, "", CardioOpenType.figureMUnit),
                change = figure.second.trim(),
                moved = false,
                description = "${if (speed) "Speed" else "Pace"} ${figure.first} ${figure.second.trim()}",
                modifier = Modifier.weight(1f)
            )
        } else {
            Spacer(Modifier.weight(1f))
        }
    }
}

/** "runs", "rides", "swims" — how the standings name the activity's other sessions. */
private fun pluralOf(activity: CardioActivity): String = when (activity.code) {
    CardioType.RUN.code -> "runs"
    CardioType.WALK.code -> "walks"
    CardioType.CYCLE.code -> "rides"
    CardioType.SWIM.code -> "swims"
    CardioType.ROW.code -> "rows"
    CardioType.HIKE.code -> "hikes"
    CardioType.TREADMILL.code -> "treadmill runs"
    else -> "${activity.displayName.lowercase()} sessions"
}

/**
 * AGAINST YOUR RUNS — this session among every one of its activity, as two strip plots: a tick for
 * each, this one the red dot, the better end named on the right. Then the previous outing.
 */
@Composable
private fun StandingSection(
    entry: CardioEntry,
    all: List<CardioEntry>,
    activity: CardioActivity,
    useMiles: Boolean,
    locale: Locale,
    zone: ZoneId,
) {
    val dist = remember(entry, all) { distanceStanding(entry, all) }
    val pace = remember(entry, all) { paceStanding(entry, all) }
    val same = remember(entry, all) { all.filter { it.type == entry.type } }
    val previous = remember(entry, all) { same.filter { it.id != entry.id && it.date < entry.date }.maxByOrNull { it.date } }
    if (dist == null && pace == null && previous == null) return
    val plural = pluralOf(activity)
    val since = remember(same, locale, zone) {
        same.minOfOrNull { it.date }?.let { Instant.ofEpochMilli(it).atZone(zone).month.getDisplayName(JavaTextStyle.SHORT, locale) }
    }
    val unit = distanceUnitLabel(useMiles)
    Column {
        Spacer(Modifier.height(SECTION))
        CardioHead("Against your $plural", "${same.size} $plural${since?.let { " since $it" } ?: ""}")
        Spacer(Modifier.height(18.dp))
        Column(verticalArrangement = Arrangement.spacedBy(26.dp)) {
            if (dist != null) {
                val reading = "${distanceNumber(dist.mine, useMiles)} $unit · ${ordinal(dist.rank)} longest of ${dist.values.size}"
                StripRow("Distance", reading, dist.values, dist.mine, "Shorter", "Longer")
            }
            if (pace != null) {
                val fig = sessionFigure(entry, useMiles)
                val reading = "${fig?.first ?: ""}${fig?.second ?: ""} · ${ordinal(pace.rank)} fastest of ${pace.values.size}"
                // Lower seconds is faster, so the plot runs on the negative to put faster on the right.
                StripRow(if (readsAsSpeed(entry.type)) "Speed" else "Pace", reading, pace.values.map { -it }, -pace.mine, "Slower", "Faster")
            }
        }
        if (previous != null) {
            Spacer(Modifier.height(20.dp))
            val prevDate = Instant.ofEpochMilli(previous.date).atZone(zone).format(DateTimeFormatter.ofPattern("MMM d", locale))
            val parts = cardioDetailParts(previous, useMiles = useMiles).joinToString(" · ")
            CardioChange(if (parts.isBlank()) "Previous · $prevDate" else "Previous · $prevDate · $parts", moved = false)
        }
    }
}

@Composable
private fun StripRow(name: String, reading: String, values: List<Double>, mine: Double, low: String, high: String) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(name, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            CardioChange(reading, moved = true)
        }
        CardioStripPlot(values, mine, description = "$name: $reading")
        Row(Modifier.fillMaxWidth()) {
            CardioChange(low, moved = false, modifier = Modifier.weight(1f))
            CardioChange(high, moved = false)
        }
    }
}

/** The watch's heart rate over the session: the average as the figure, the peak beside it, the area. */
@Composable
private fun HeartRateSection(hr: List<HrPoint>, avg: Int?, peak: Int?) {
    val t0 = hr.first().timeMs
    val span = (hr.last().timeMs - t0).coerceAtLeast(1L)
    val minutes = (span / 60_000L).toInt()
    CardioHead("Heart rate", "From your watch")
    Spacer(Modifier.height(10.dp))
    Text(
        cardioFigure("${avg ?: 0}", " bpm", CardioOpenType.figureMUnit),
        style = OpenType.figureM,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = Modifier.clearAndSetSemantics { contentDescription = "Average heart rate ${avg ?: 0} bpm, peak ${peak ?: 0}" }
    )
    Spacer(Modifier.height(8.dp))
    CardioChange("Average · peak ${peak ?: 0}", moved = false)
    Spacer(Modifier.height(14.dp))
    CardioAreaChart(
        points = hr.map { ((it.timeMs - t0).toFloat() / span) to it.bpm.toFloat() },
        totalMinutes = minutes,
        description = "Heart rate over $minutes minutes, average ${avg ?: 0}, peak ${peak ?: 0}"
    )
}

/** That day's steps by hour from the watch, the session's own hours in red. */
@Composable
private fun DaySteps(wearable: CardioWearableDay?, entry: CardioEntry, zone: ZoneId, locale: Locale) {
    val accent = MaterialTheme.colorScheme.primary
    val start = Instant.ofEpochMilli(entry.date).atZone(zone)
    val startHour = start.hour
    val endHour = start.plusMinutes(entry.durationMin.toLong()).hour.let { if (it < startHour) 23 else it }
    val byHour = remember(wearable) {
        val raw = IntArray(24)
        wearable?.hourlySteps?.forEach { if (it.hour in 0..23) raw[it.hour] += it.steps }
        raw.toList()
    }
    val total = byHour.sum()
    CardioHead("Steps that day", formatCount(total, locale), rightInk = total > 0)
    Spacer(Modifier.height(14.dp))
    OpenColumnChart(
        values = byHour.map { it.toFloat() },
        colors = (0..23).map { h -> if (h in startHour..endHour) accent else OpenColors.Bar },
        height = 64.dp,
        columnLabels = (0..23).map { h -> if (h % 6 == 0) com.forge.app.ui.cardio.hourTick(h, LocalForgeSettings.current.timeFormat24h) else "" },
        gapFraction = 0.35f,
        description = "Steps that day by hour, $total in all"
    )
    if (total == 0) {
        Spacer(Modifier.height(6.dp))
        CardioChange("No steps from your watch for this day yet", moved = false)
    }
}

/**
 * The session's descriptive tags: effort · HR zone · intervals · the per-type field · weather. Each
 * is a word or a number that qualifies the session, so together they are one caption.
 */
private fun sessionTags(entry: CardioEntry, useMiles: Boolean): List<String> = buildList {
    CardioEffort.fromCode(entry.effort)?.let { add(it.displayName) }
    entry.hrZone?.let { add("Zone $it") }
    entry.intervalCount?.takeIf { it > 0 }?.let { add("$it intervals") }
    entry.laps?.takeIf { it > 0 }?.let { add("$it laps") }
    entry.inclinePct?.takeIf { it > 0 }?.let { add("${formatInclinePct(it)} incline") }
    entry.elevationM?.takeIf { it > 0 }?.let { add("${formatElevation(it, useMiles)} gain") }
    CardioCondition.decode(entry.conditions).forEach { add(it.displayName) }
}

/** What the watch measured that differs from the entry: its duration, its distance, and its kcal. */
private fun watchStatParts(w: WatchWorkout?, entry: CardioEntry, useMiles: Boolean): List<String> = w?.let {
    buildList {
        if (w.durationMin > 0 && w.durationMin != entry.durationMin) add("${w.durationMin} min")
        w.distanceKm?.takeIf { d -> entry.distanceKm == null || abs(d - entry.distanceKm!!) > 0.05 }
            ?.let { add(formatDistance(it, useMiles)) }
        w.kcal?.let { add("${it.toInt()} kcal") }
    }
}.orEmpty()
