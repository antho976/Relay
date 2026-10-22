package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.ChartKeyMark
import com.forge.app.ui.common.ChartPoint
import com.forge.app.ui.common.OpenBarList
import com.forge.app.ui.common.OpenChartKey
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenPrTag
import com.forge.app.ui.common.OpenRangeSwitch
import com.forge.app.ui.common.OpenSectionHead
import com.forge.app.ui.common.OpenSlotLabel
import com.forge.app.ui.common.OpenSparkline
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.OpenTrendChart
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.gym.stats.state.StatsUiState
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The page's tabs (2026-10-21 redesign). The days grid that was a fifth lens now lives on Profile;
 * a stored "DAYS" choice falls back to Strength.
 */
enum class StatsLens(val label: String) {
    STRENGTH("Strength"),
    VOLUME("Volume"),
    EFFORT("Effort"),
    BODY("Body"),
}

/**
 * A lens another screen asks Stats to open on (Home's body reading). Stats is a pager page that is
 * often already composed when the request is made, so a nav argument would not reach it.
 */
object StatsLensRequests {
    val pending = kotlinx.coroutines.flow.MutableStateFlow<StatsLens?>(null)

    /** A lift another screen asks Stats to show (a session's "trend →"): Strength, on that lift. */
    val pendingLift = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
}

private val GUTTER = 24.dp
private val SECTION = 52.dp

/**
 * Stats, as redesigned 2026-10-21 (Relay design 7, "Stats final"): a serif title, four tabs, one
 * range for the whole page, then each tab as open readings and graphs. Every figure is the range's,
 * and every change line compares it with the same span before ("8 more than the 12 before").
 */
@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    onBack: (() -> Unit)? = null,
    /** Opens a gym session's detail. */
    onOpenSession: (Long) -> Unit = {},
    /** Opens a cardio session's detail. */
    onOpenCardioSession: (Long) -> Unit = {},
    /** The Body tab's connect line: Settings → Wearable. */
    onConnectWatch: () -> Unit = {},
    /** The Body tab's weigh-in link: the profile. */
    onOpenProfile: () -> Unit = {},
    onOpenHistory: () -> Unit = {},
    onOpenNotes: () -> Unit = {},
    onOpenRecap: () -> Unit = {},
    /** Swipes to the Coach tab; null when it isn't shown (coach off, or freestyle). */
    onOpenCoach: (() -> Unit)? = null,
    /** A lift's own page. */
    onOpenLift: (String) -> Unit = {},
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val body by viewModel.body.collectAsStateWithLifecycle()
    val unit = LocalForgeSettings.current.weightUnit
    val listState = rememberLazyListState()

    // Reopen on the last tab the user viewed; unknown stored names (the retired lenses) fall back.
    val lastLensName by viewModel.lastStatsTabName.collectAsStateWithLifecycle()
    var lens by rememberSaveable { mutableStateOf(StatsLens.STRENGTH) }
    var restoredLens by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(lastLensName) {
        if (!restoredLens && lastLensName != null) {
            StatsLens.entries.firstOrNull { it.name == lastLensName }?.let { lens = it }
            restoredLens = true
        }
    }
    LaunchedEffect(restoredLens) {
        if (restoredLens) snapshotFlow { lens }.collect { viewModel.saveStatsTab(it.name) }
    }
    var range by rememberSaveable { mutableStateOf(StatsRange.W12) }
    var liftId by rememberSaveable { mutableStateOf<String?>(null) }

    val requestedLens by StatsLensRequests.pending.collectAsStateWithLifecycle()
    LaunchedEffect(requestedLens) {
        requestedLens?.let {
            lens = it; restoredLens = true
            StatsLensRequests.pending.value = null
            listState.scrollToItem(0)
        }
    }
    val requestedLift by StatsLensRequests.pendingLift.collectAsStateWithLifecycle()
    LaunchedEffect(requestedLift) {
        requestedLift?.let { id ->
            liftId = id; lens = StatsLens.STRENGTH; restoredLens = true
            StatsLensRequests.pendingLift.value = null
            listState.scrollToItem(0)
        }
    }
    // Health Connect has no observer, so the Body tab reads it when opened and on every resume.
    LaunchedEffect(lens) { if (lens == StatsLens.BODY) viewModel.refreshBody() }
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME && lens == StatsLens.BODY) viewModel.refreshBody()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    val now = remember(state) { endOfToday(System.currentTimeMillis()) }
    val window = remember(range, state.statSets, now) {
        statsWindow(range, now, state.statSets.firstOrNull()?.sessionStartedAt)
    }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = GUTTER, end = GUTTER, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                if (onBack != null) OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
                Spacer(Modifier.weight(1f))
                OpenTopBarIcon(OpenIcons.History, "Session history", onOpenHistory)
            }
        }
        item("title") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text("Stats", style = OpenType.titleL, color = MaterialTheme.colorScheme.onBackground)
                Spacer(Modifier.height(18.dp))
                OpenTabs(StatsLens.entries, lens, { it.label }, { lens = it })
                Spacer(Modifier.height(12.dp))
                OpenRangeSwitch(StatsRange.entries, range, { it.label }, { range = it })
                Spacer(Modifier.height(22.dp))
            }
        }
        if (state.loadError) {
            item("error") {
                Text(
                    "Couldn't load your stats just now. Swipe away and back. If it keeps happening, restart the app.",
                    style = OpenType.body, color = MaterialTheme.colorScheme.error
                )
            }
        } else if (!state.isLoading) {
            when (lens) {
                StatsLens.STRENGTH -> item("strength") {
                    StrengthTab(state, window, range, unit, liftId, { liftId = it }, onOpenLift)
                }
                StatsLens.VOLUME -> item("volume") { VolumeTab(state, window, range, unit) }
                StatsLens.EFFORT -> item("effort") { EffortTab(state, window, range) }
                StatsLens.BODY -> item("body") { BodyTab(state, body, window, range, unit, onConnectWatch, onOpenProfile) }
            }
            item("look-back") {
                Column {
                    Spacer(Modifier.height(SECTION))
                    OpenSectionHead("Look back")
                    Spacer(Modifier.height(12.dp))
                    LookBackLink("Session history", onOpenHistory)
                    LookBackLink("Notes", onOpenNotes)
                    LookBackLink("Weekly recap", onOpenRecap)
                }
            }
        }
    }
}

@Composable
private fun LookBackLink(text: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickableLabeled(text, onClick = onClick).padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(text, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
        Icon(OpenIcons.ChevronRight, contentDescription = null, tint = OpenColors.Glyph, modifier = Modifier.size(16.dp))
    }
}

// ── Shared pieces ──────────────────────────────────────────────────────────────────────────────

/** A weight in the user's unit, rounded to whole units for a reading. */
internal fun w(lb: Double, unit: WeightUnit): String = toDisplayWeight(lb, unit).roundToInt().toString()

/** "228" with " lb" at half size in the muted voice. */
@Composable
internal fun figure(value: String, unitText: String, style: TextStyle, unitStyle: TextStyle): AnnotatedString {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    return buildAnnotatedString {
        append(value)
        if (unitText.isNotEmpty()) withStyle(unitStyle.toSpanStyle().copy(color = muted)) { append(unitText) }
    }
}

@Composable
private fun Head(left: String, right: String? = null, leftContent: (@Composable () -> Unit)? = null) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        if (leftContent != null) leftContent() else OpenLabel(left)
        if (right != null) OpenLabel(right)
    }
}

@Composable
private fun Change(text: String?, moved: Boolean = true) {
    if (text == null) return
    OpenLabel(
        text,
        style = OpenType.monoSTight,
        color = if (moved) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
    )
}

@Composable
private fun Empty(text: String) {
    Text(text, style = OpenType.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

private val DATE_SHORT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMM d")

private fun dateOf(t: Long): String = Instant.ofEpochMilli(t).atZone(ZoneId.systemDefault()).toLocalDate().format(DATE_SHORT)

// ── Strength ───────────────────────────────────────────────────────────────────────────────────

@Composable
private fun StrengthTab(
    state: StatsUiState,
    window: StatsWindow,
    range: StatsRange,
    unit: WeightUnit,
    selectedId: String?,
    onSelect: (String) -> Unit,
    onOpenLift: (String) -> Unit,
) {
    val sessions = remember(state.statSets) { liftSessions(state.statSets) }
    val names = remember(state.e1rmLifts) { state.e1rmLifts.associate { it.exerciseId to it.exerciseName } }
    val trends = remember(sessions, window, names) {
        sessions.mapNotNull { (id, s) -> liftTrend(id, s, window)?.copy(name = names[id] ?: com.forge.app.program.Program.exerciseDisplayName(id)) }
            .filter { it.current.isNotEmpty() }
            .sortedByDescending { it.now }
    }
    if (trends.isEmpty()) {
        Empty("Log a weighted set and your strength shows here: each lift's estimated one-rep max over time.")
        return
    }
    // The lift on the graph: the one asked for, else the one trained most in the range.
    // The lift on the graph: the one asked for, else the strongest lift trained at least three times
    // in the range (a main lift, not whichever accessory was logged most), else the most trained.
    val lift = trends.firstOrNull { it.exerciseId == selectedId }
        ?: trends.firstOrNull { it.current.size >= 3 }
        ?: trends.maxBy { it.current.size }
    var picking by remember { mutableStateOf(false) }
    val unitLabel = unitLabel(unit)
    val fmt: (Double) -> String = { "${w(it, unit)} $unitLabel" }
    val fmtN: (Double) -> String = { w(it, unit) }

    Column {
        Head("", "Est. 1RM") {
            OpenSlotLabel(lift.name, onClick = { picking = true })
        }
        Spacer(Modifier.height(10.dp))
        Text(figure(w(lift.now, unit), " $unitLabel", OpenType.figureXL, OpenType.figureXLUnit), style = OpenType.figureXL, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(8.dp))
        Change(listOfNotNull(liftChangeLine(lift, range, fmt), versusBefore(lift.delta, lift.previousDelta, range, fmtN)).joinToString(" · ").ifBlank { null }, lift.flatWeeks == null)
        Spacer(Modifier.height(20.dp))
        LiftGraph(lift, state, window, range, unit)
        Spacer(Modifier.height(SECTION))
        Head("Every lift", "Est. 1RM · ${range.span}")
        Spacer(Modifier.height(22.dp))
        var showAll by rememberSaveable { mutableStateOf(false) }
        Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
            (if (showAll) trends else trends.take(6)).forEach { t ->
                LiftRow(t, range, unit, onClick = { onOpenLift(t.exerciseId) })
            }
            if (trends.size > 6) {
                OpenLabel(
                    if (showAll) "Show fewer" else "Show all ${trends.size}",
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.touchSlop(vertical = 14.dp).clickableLabeled(if (showAll) "Show fewer lifts" else "Show every lift") { showAll = !showAll }.padding(vertical = 14.dp)
                )
            }
        }
    }
    if (picking) {
        LiftPickerSheet(trends, lift.exerciseId, unit, onPick = { onSelect(it); picking = false }, onDismiss = { picking = false })
    }
}

/** The lift's graph and its key: this range in red over the span before, with its PRs, deloads and coach changes. */
@Composable
internal fun LiftGraph(lift: LiftTrend, state: StatsUiState, window: StatsWindow, range: StatsRange, unit: WeightUnit) {
    val current = lift.current.map { ChartPoint(window.fraction(it.t), it.e1rm.toFloat()) }
    val previous = lift.previous.map { ChartPoint(window.previousFraction(it.t), it.e1rm.toFloat()) }
    val prs = lift.current.indices.filter { lift.current[it].pr }
    val coach = state.coachChanges[lift.exerciseId].orEmpty().filter { it in window }.map { window.fraction(it) }
    val bands = state.sessionVolumes.filter { it.deloadMarkedHere && it.startedAt in window }.map {
        val f = window.fraction(it.startedAt); f to (f + 7f / ((window.length / 86_400_000f).coerceAtLeast(7f)))
    }
    val unitLabel = unitLabel(unit)
    OpenTrendChart(
        current = current,
        previous = previous,
        xLabels = monthTicks(window),
        prIndices = prs,
        coachMarks = coach,
        bands = bands,
        readout = { i ->
            val p = lift.current[i]
            val was = lift.previous.minByOrNull { abs(window.previousFraction(it.t) - window.fraction(p.t)) }
            buildAnnotatedString {
                append("${dateOf(p.t)} · ${w(p.e1rm, unit)} $unitLabel")
                if (was != null) withStyle(SpanStyle(color = OpenColors.Glyph)) { append(" · was ${w(was.e1rm, unit)}") }
            }
        },
        description = "${lift.name} estimated one-rep max over ${range.span}, ${w(lift.now, unit)} $unitLabel now" +
            (if (prs.isNotEmpty()) ", ${prs.size} personal records" else "")
    )
    Spacer(Modifier.height(14.dp))
    // The key names only the marks this chart draws, so it stays one line at 384dp.
    OpenChartKey(
        listOfNotNull(
            if (range.before != null && previous.isNotEmpty()) ChartKeyMark.PREVIOUS to "${range.span} before" else null,
            if (prs.isNotEmpty()) ChartKeyMark.PR to "PR" else null,
            if (coach.isNotEmpty()) ChartKeyMark.COACH to "Coach change" else null,
            if (bands.isNotEmpty()) ChartKeyMark.DELOAD to "Deload" else null,
        )
    )
}

@Composable
private fun LiftRow(t: LiftTrend, range: StatsRange, unit: WeightUnit, onClick: () -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val change = liftChangeLine(t, range) { "${w(it, unit)} ${unitLabel(unit)}" }
    Row(
        Modifier.fillMaxWidth().clickableLabeled("Open ${t.name}", onClick = onClick),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(t.name, style = OpenType.rowTitle, color = ink, modifier = Modifier.weight(1f, fill = false))
                if (t.prInRange) OpenPrTag()
            }
            if (change != null) OpenLabel(change, style = OpenType.monoXS, color = if (t.flatWeeks == null) ink else MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(w(t.now, unit), style = OpenType.figureRow, color = ink)
        Icon(OpenIcons.ChevronRight, contentDescription = null, tint = OpenColors.Glyph, modifier = Modifier.size(16.dp))
    }
}

// ── Volume ─────────────────────────────────────────────────────────────────────────────────────

private const val RANGE_LOW = 10
private const val RANGE_HIGH = 20

@Composable
private fun VolumeTab(state: StatsUiState, window: StatsWindow, range: StatsRange, unit: WeightUnit) {
    val muscles = remember(state.statSets, window) { weeklySetsPerMuscle(state.statSets, window) }
    val weeks = remember(state.sessionVolumes, window) { weeklyVolumes(state.sessionVolumes, window) }
    Column {
        Head("Muscles in range", "$RANGE_LOW to $RANGE_HIGH sets a week")
        Spacer(Modifier.height(10.dp))
        if (muscles.isEmpty()) {
            Empty("Sets you log land here, by the muscle they work.")
        } else {
            val inRange = muscles.count { it.second in RANGE_LOW..RANGE_HIGH }
            val under = muscles.filter { it.second < RANGE_LOW }.map { it.first.displayName.lowercase() }
            Text(figure("$inRange", "/${muscles.size}", OpenType.figureXL, OpenType.figureXL), style = OpenType.figureXL, color = MaterialTheme.colorScheme.onBackground)
            Spacer(Modifier.height(8.dp))
            val total = muscles.sumOf { it.second }
            Change(
                listOfNotNull(
                    when (under.size) {
                        0 -> "All in range"
                        1 -> "${under[0].replaceFirstChar { it.uppercase() }} under"
                        2 -> "${under[0].replaceFirstChar { it.uppercase() }} and ${under[1]} under"
                        else -> "${under.size} under"
                    },
                    "$total sets a week"
                ).joinToString(" · ")
            )
            Spacer(Modifier.height(20.dp))
            OpenBarList(
                rows = muscles.map { it.first.displayName to it.second.toFloat() },
                lit = muscles.indices.filter { muscles[it].second < RANGE_LOW }.toSet(),
                description = "Average weekly sets per muscle over ${range.span}: " +
                    muscles.joinToString { "${it.first.displayName} ${it.second}" }
            )
        }
        Spacer(Modifier.height(SECTION))
        val unitLabel = unitLabel(unit)
        val thisWeek = weeks.lastOrNull()?.takeIf { it.current }
        Head("Weekly volume · $unitLabel", thisWeek?.let { "${short(toDisplayWeight(it.volumeLb, unit))} so far" })
        Spacer(Modifier.height(10.dp))
        val finished = weeks.filterNot { it.current }
        if (finished.isEmpty() || finished.all { it.volumeLb == 0.0 }) {
            Empty("Finish a week of training and its volume shows here.")
            return@Column
        }
        val avg = finished.map { it.volumeLb }.average()
        val prevAvg = window.prevStart?.let { averageWeek(state.sessionVolumes, it, window.start) }
        val avgText = short(toDisplayWeight(avg, unit))
        Text(
            figure(avgText.trimEnd('k'), if (avgText.endsWith("k")) "k" else "", OpenType.figureL, OpenType.figureLUnit),
            style = OpenType.figureL, color = MaterialTheme.colorScheme.onBackground
        )
        Spacer(Modifier.height(8.dp))
        val pct = prevAvg?.takeIf { it > 0 }?.let { ((avg - it) / it * 100).roundToInt() }
        Change(listOfNotNull("Avg week", pct?.let { "${if (it < 0) "↓" else "↑"} ${abs(it)}% on ${range.before}" }).joinToString(" · "))
        Spacer(Modifier.height(16.dp))
        val accent = MaterialTheme.colorScheme.primary
        OpenColumnChart(
            values = weeks.map { toDisplayWeight(it.volumeLb, unit).toFloat() },
            colors = weeks.map { if (it.current) accent else OpenColors.BarQuiet },
            xLabels = monthTicks(window),
            description = "Weekly volume over ${range.span}, averaging $avgText $unitLabel a week"
        )
        Spacer(Modifier.height(10.dp))
        val notes = listOfNotNull(
            if (weeks.any { it.deload }) "The short weeks were deloads" else null,
            if (thisWeek != null) "this week is still going" else null
        )
        if (notes.isNotEmpty()) {
            Text(notes.joinToString(" · ").replaceFirstChar { it.uppercase() }, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** 61_400 → "61.4k", 950 → "950". */
private fun short(v: Double): String = when {
    v >= 100_000 -> "${(v / 1000).roundToInt()}k"
    v >= 1000 -> "%.1fk".format(v / 1000).replace(".0k", "k")
    else -> v.roundToInt().toString()
}

// ── Effort ─────────────────────────────────────────────────────────────────────────────────────

@Composable
private fun EffortTab(state: StatsUiState, window: StatsWindow, range: StatsRange) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    Column {
        val today = LocalDate.now().toEpochDay()
        val fromDay = Instant.ofEpochMilli(window.start).atZone(ZoneId.systemDefault()).toLocalDate().toEpochDay()
        val model = remember(state.dailyActivity, fromDay, today) { banister(state.dailyActivity, fromDay, today) }
        Head("Fitness vs fatigue", state.readinessPulse?.let { p -> state.readinessThreshold?.let { "Fatigue ${p.score} of $it" } })
        Spacer(Modifier.height(10.dp))
        if (model == null) {
            Empty("A few sessions in, this reads whether you're building, fresh or carrying fatigue.")
        } else {
            Text(formWord(model), style = OpenType.figureMS, color = MaterialTheme.colorScheme.onBackground)
            Spacer(Modifier.height(8.dp))
            Change(formLine(model))
            Spacer(Modifier.height(16.dp))
            OpenChartKeyLines(listOf("Fitness" to muted, "Fatigue" to accent))
            Spacer(Modifier.height(8.dp))
            // One point a week, ending today: the daily model swings with every session and read as noise.
            val idx = model.days.indices.reversed().filter { (model.days.size - 1 - it) % 7 == 0 }.reversed()
            val first = model.days.first()
            val span = (model.days.last() - first).coerceAtLeast(1).toFloat()
            OpenTrendChart(
                current = idx.map { ChartPoint((model.days[it] - first) / span, model.fatigue[it]) },
                previous = idx.map { ChartPoint((model.days[it] - first) / span, model.fitness[it]) },
                previousColor = muted,
                xLabels = monthTicks(window),
                height = 170.dp,
                description = "Fitness and fatigue over ${range.span}. Today reads ${formWord(model)}."
            )
        }
        Spacer(Modifier.height(SECTION))
        val effort = remember(state.statSets, window) { effortRead(state.statSets, window.start, window.end) }
        val before = remember(state.statSets, window) { window.prevStart?.let { effortRead(state.statSets, it, window.start - 1) } }
        Head("Effort", "${effort.rated} rated ${if (effort.rated == 1) "set" else "sets"}")
        Spacer(Modifier.height(10.dp))
        if (effort.rated == 0) {
            Empty("Rate how hard a set was while you log it, and how close you train to failure shows here.")
            return@Column
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.width(120.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(figure("${effort.inZonePct ?: 0}", "%", OpenType.figureXL, OpenType.figureXLUnit), style = OpenType.figureXL, color = MaterialTheme.colorScheme.onBackground)
                // No-break spaces keep "1 to 3 reps left" on one line in the narrow figure column.
                Text("of sets with 1\u00A0to\u00A03\u00A0reps\u00A0left", style = OpenType.bodyS, color = muted)
            }
            OpenColumnChart(
                values = effort.buckets.map { it.toFloat() },
                colors = listOf(OpenColors.Bar, accent, accent, accent, OpenColors.Bar),
                columnLabels = listOf("4+", "3", "2", "1", "0"),
                valueLabels = true,
                height = 110.dp,
                modifier = Modifier.weight(1f),
                description = "Rated sets by reps left: " + listOf("4 or more", "3", "2", "1", "none").zip(effort.buckets).joinToString { "${it.first} ${it.second}" }
            )
        }
        Spacer(Modifier.height(12.dp))
        val prevPct = before?.inZonePct
        if (prevPct != null && range.before != null) {
            val now = effort.inZonePct ?: 0
            Change("${if (now >= prevPct) "↑" else "↓"} from $prevPct% ${range.before}", moved = false)
        }
    }
}

/** A line key: a short bar in each series' colour with its word. */
@Composable
private fun OpenChartKeyLines(items: List<Pair<String, Color>>) {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
        items.forEach { (word, color) ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                androidx.compose.foundation.Canvas(Modifier.size(8.dp)) { drawRect(color) }
                Text(word, style = OpenType.chartLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

// ── Body ───────────────────────────────────────────────────────────────────────────────────────

@Composable
private fun BodyTab(
    state: StatsUiState,
    body: StatsBodyUi?,
    window: StatsWindow,
    range: StatsRange,
    unit: WeightUnit,
    onConnectWatch: () -> Unit,
    onOpenProfile: () -> Unit,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val trend = remember(state.bodyweightPoints, window) { weightTrend(state.bodyweightPoints, window) }
    val unitLabel = unitLabel(unit)
    Column {
        Head("Body weight", "From your scale")
        Spacer(Modifier.height(10.dp))
        if (trend == null) {
            Text(figure("0", " $unitLabel", OpenType.figureXL, OpenType.figureXLUnit), style = OpenType.figureXL, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            OpenLabel("No weigh-ins yet · log one", color = ink, style = OpenType.monoSTight, modifier = Modifier.touchSlop(vertical = 14.dp).clickableLabeled("Log a weigh-in", onClick = onOpenProfile).padding(vertical = 14.dp))
        } else {
            val shown = toDisplayWeight(trend.latestLb, unit)
            Text(figure("%.1f".format(shown).removeSuffix(".0"), " $unitLabel", OpenType.figureXL, OpenType.figureXLUnit), style = OpenType.figureXL, color = ink)
            Spacer(Modifier.height(8.dp))
            val d = trend.deltaLb
            Change(
                d?.let {
                    val dd = toDisplayWeight(abs(it), unit)
                    "${if (it < 0) "↓" else "↑"} ${"%.1f".format(dd)} $unitLabel in ${range.span}" + if (dd < 2.0) " · steady" else ""
                },
                moved = d != null
            )
            Spacer(Modifier.height(16.dp))
            OpenTrendChart(
                current = trend.current.map { ChartPoint(window.fraction(it.recordedAt), toDisplayWeight(it.weightLb, unit).toFloat()) },
                previous = trend.previous.map { ChartPoint(window.previousFraction(it.recordedAt), toDisplayWeight(it.weightLb, unit).toFloat()) },
                xLabels = monthTicks(window),
                height = 150.dp,
                readout = { i ->
                    val p = trend.current[i]
                    AnnotatedString("${dateOf(p.recordedAt)} · ${"%.1f".format(toDisplayWeight(p.weightLb, unit))} $unitLabel")
                },
                description = "Body weight over ${range.span}, ${"%.1f".format(shown)} $unitLabel now"
            )
        }
        Spacer(Modifier.height(SECTION))
        when {
            body == null -> Unit
            else -> {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                    WatchReading("Sleep · avg", body.sleepMin, { "%.1f".format(it / 60) }, "h", { d -> "${arrow(d)} ${"%.1f".format(abs(d) / 60)}h vs before" }, Modifier.weight(1f))
                    WatchReading("Resting HR", body.restingHr, { "${it.roundToInt()}" }, "", { d -> "${arrow(d)} ${abs(d).roundToInt()} bpm vs before" }, Modifier.weight(1f), latest = true)
                }
                Spacer(Modifier.height(44.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                    WatchReading("HRV", body.hrvMs, { "${it.roundToInt()}" }, "", { d -> "${arrow(d)} ${abs(d).roundToInt()} ms vs before" }, Modifier.weight(1f), latest = true)
                    WatchReading("Steps · avg", body.steps, { short(it).trimEnd('k') }, "", { d -> "${arrow(d)} ${short(abs(d))} vs before" }, Modifier.weight(1f), kUnit = true)
                }
                // Not connected: the four readings stay, at zero, with one way to fill them.
                if (!body.connected && body.available) {
                    Spacer(Modifier.height(20.dp))
                    OpenLabel("Connect a watch", color = ink, style = OpenType.monoSTight, modifier = Modifier.touchSlop(vertical = 14.dp).clickableLabeled("Connect a watch", onClick = onConnectWatch).padding(vertical = 14.dp))
                }
            }
        }
    }
}

private fun arrow(d: Double) = if (d < 0) "↓" else "↑"

/**
 * One watch signal over its last two weeks: the average (or latest) as the figure, the days as a
 * plain red line, and the last week against the one before.
 */
@Composable
private fun WatchReading(
    label: String,
    series: BodySeries?,
    fmt: (Double) -> String,
    unitText: String,
    changeText: (Double) -> String,
    modifier: Modifier,
    latest: Boolean = false,
    kUnit: Boolean = false,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OpenLabel(label)
        val days = series?.days.orEmpty()
        val recent = days.takeLast(7).filterNotNull()
        val before = days.dropLast(7).filterNotNull()
        val value = if (latest) series?.latest ?: recent.lastOrNull() else recent.takeIf { it.isNotEmpty() }?.average()
        if (value == null) {
            Text(figure("0", unitText, OpenType.figureL, OpenType.figureLUnit), style = OpenType.figureL, color = muted)
            com.forge.app.ui.common.OpenDeltaBars(emptyList(), Modifier.fillMaxWidth().height(26.dp))
            OpenLabel("No data yet", style = OpenType.monoSTight)
            return@Column
        }
        val text = fmt(value)
        Text(
            figure(text, if (kUnit && value >= 1000) "k" else unitText, OpenType.figureL, OpenType.figureLUnit),
            style = OpenType.figureL, color = ink
        )
        OpenSparkline(days.filterNotNull().map { it.toFloat() }, Modifier.fillMaxWidth().height(26.dp), plain = true)
        val delta = if (recent.isNotEmpty() && before.size >= 3) recent.average() - before.average() else null
        val small = delta != null && abs(delta) < (value * 0.03)
        Change(
            when {
                delta == null -> null
                small -> "About your usual"
                else -> changeText(delta)
            },
            moved = delta != null && !small
        )
    }
}
