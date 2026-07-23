package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import kotlin.math.abs
import kotlin.math.roundToInt

/** The comparison window for the little "vs a month ago" delta beside a body reading. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

/** The trend-mark box shared by every body row, so the sparklines and pips right-align across rows. */
private val markMod = Modifier.width(88.dp).height(30.dp)

/**
 * BODY (Antho 2026-07-13) — bodyweight, body fat and measurements merged from three separate Profile
 * sections into ONE compact stack, so the "your body" cluster reads as a single section. One mono
 * "BODY" header, then a compact row per metric: mono label + current reading + ~30-day delta + a
 * trend mark on the right, with "+ log" for the two metrics logged here and a whole-row "open →"
 * into the Measurements screen.
 *
 * Empty is drawn (§12): an empty metric shows a ghost flat line beside its live siblings ("still
 * forming"); when all three are empty the whole section collapses to a single hint rather than three
 * ghost rows. Measurements reads its own [BodyMeasurementsViewModel] so ProfileViewModel is untouched.
 */
@Composable
internal fun BodyMetricsSection(
    bodyweight: List<BodyweightEntry>,
    bodyweightGoalLb: Double?,
    bodyFat: List<BodyFatEntry>,
    onLogWeight: () -> Unit,
    onLogBodyFat: () -> Unit,
    onOpenMeasurements: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    measurementsVm: BodyMeasurementsViewModel = hiltViewModel(),
    leanMassVm: LeanMassViewModel = hiltViewModel()
) {
    val measurements by measurementsVm.state.collectAsStateWithLifecycle()
    val leanMass by leanMassVm.state.collectAsStateWithLifecycle()

    // The MUSCLE row exists only for a connected watch (or leftover data after a disconnect) — an
    // HC-only metric never shows an unconnected ghost row here; Recovery owns the connect flow.
    val showMuscle = leanMass.connected || leanMass.entries.isNotEmpty()

    SectionHeader("BODY", muted)
    if (bodyweight.isEmpty() && bodyFat.isEmpty() && !measurements.anyData && leanMass.entries.isEmpty()) {
        // All at zero → one hint, never a stack of identical ghost rows (§12 collapse-repetition).
        InlineEmptyHint("Log a weigh-in, body fat or a measurement to track your body here.", muted)
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        WeightRow(bodyweight, bodyweightGoalLb, onLogWeight, onBg, muted, accent)
        BodyFatRow(bodyFat, onLogBodyFat, onBg, muted, accent)
        if (showMuscle) {
            LeanMassRow(leanMass.entries, onSync = { leanMassVm.syncNow() }, onBg, muted, accent)
        }
        SizesRow(measurements, onOpenMeasurements, muted, accent)
    }
}

/** Direction-only trend arrow + magnitude; both tones muted (up isn't a good/bad verdict, §11). */
private data class MetricDelta(val up: Boolean, val value: String)

/**
 * BODYWEIGHT row — the current weight as a serif figure, a ~30-day delta off the 7-day average (so a
 * noisy final weigh-in can't flip the arrow), and the smoothed trend as a spark that keeps the dashed
 * goal line on-canvas when a bodyweight goal is set.
 */
@Composable
private fun WeightRow(
    entries: List<BodyweightEntry>,
    goalLb: Double?,
    onLog: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val unit = unitLabel(weightUnit)
    val display = remember(entries, weightUnit) { entries.map { toDisplayWeight(it.weightLb, weightUnit) } }
    val ma = remember(entries, display) { sevenDayMovingAverage(entries, display) }
    val delta = remember(entries, ma) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            ma.last() - ma[refIdx.coerceAtLeast(0)]
        }
    }
    val goalDisplay = goalLb?.let { toDisplayWeight(it, weightUnit) }
    BodyMetricRow(
        label = "WEIGHT",
        figure = if (entries.isEmpty()) null else display.last().roundToInt().toString(),
        unit = unit.uppercase(),
        delta = delta.asMetricDelta(),
        action = "+ log",
        onAction = onLog,
        rowTapNav = false,
        onBg = onBg, muted = muted, accent = accent
    ) {
        if (ma.size >= 2) ProfileSparkline(ma, accent, markMod, reference = goalDisplay)
        else GhostSpark(muted)
    }
}

/**
 * BODY FAT row (GYMAP-62) — the current reading as a serif figure with a ~30-day delta in points, and
 * the raw readings as the spark directly (body fat is logged sparsely, so a smoothing average would
 * just trace the same points).
 */
@Composable
private fun BodyFatRow(
    entries: List<BodyFatEntry>,
    onLog: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val values = remember(entries) { entries.map { it.percent } }
    val delta = remember(entries, values) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            values.last() - values[refIdx.coerceAtLeast(0)]
        }
    }
    BodyMetricRow(
        label = "BODY FAT",
        figure = if (entries.isEmpty()) null else "%.1f".format(values.last()),
        unit = "%",
        delta = delta.asMetricDelta(),
        action = "+ log",
        onAction = onLog,
        rowTapNav = false,
        onBg = onBg, muted = muted, accent = accent
    ) {
        if (values.size >= 2) ProfileSparkline(values, accent, markMod)
        else GhostSpark(muted)
    }
}

/**
 * MUSCLE row (W6) — the watch's BIA lean-mass reading as a serif figure with a ~30-day delta, the
 * raw readings as the spark (measured sparsely, like body fat). Import-only: the row's `sync →`
 * pulls the latest Health Connect reading; there is no manual log for a watch-authored metric.
 */
@Composable
private fun LeanMassRow(
    entries: List<com.forge.app.data.db.entities.LeanMassEntry>,
    onSync: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val display = remember(entries, weightUnit) { entries.map { toDisplayWeight(it.weightLb, weightUnit) } }
    val delta = remember(entries, display) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            display.last() - display[refIdx.coerceAtLeast(0)]
        }
    }
    BodyMetricRow(
        label = "MUSCLE",
        figure = if (entries.isEmpty()) null else "%.1f".format(display.last()),
        unit = unitLabel(weightUnit).uppercase(),
        delta = delta.asMetricDelta(),
        action = "sync →",
        onAction = onSync,
        rowTapNav = false,
        onBg = onBg, muted = muted, accent = accent
    ) {
        if (display.size >= 2) ProfileSparkline(display, accent, markMod)
        else GhostSpark(muted)
    }
}

/**
 * MEASUREMENTS row — a trim of the destination (§4.2): the coverage pips show which of the five
 * circumferences are tracked (a mark that works at zero, all hollow), and the whole row taps into the
 * full [BodyMeasurementsScreen] where the values, trends and logging live.
 */
@Composable
private fun SizesRow(
    state: BodyMeasurementsUiState,
    onOpen: () -> Unit,
    muted: Color,
    accent: Color
) {
    BodyMetricRow(
        label = "SIZES",
        figure = null,
        unit = null,
        delta = null,
        action = "open →",
        onAction = onOpen,
        rowTapNav = true,
        onBg = muted, muted = muted, accent = accent
    ) {
        MeasurementPips(state, muted, accent)
    }
}

/**
 * The shared compact row skeleton: fixed mono label column (so figures align across rows) + the
 * reading cluster + a flexible gap + the trend mark + the action. A nav row ([rowTapNav]) taps the
 * whole surface and draws its action passively (no nested tap, §8); a log row makes only its "+ log"
 * tappable.
 */
@Composable
private fun BodyMetricRow(
    label: String,
    figure: String?,
    unit: String?,
    delta: MetricDelta?,
    action: String,
    onAction: () -> Unit,
    rowTapNav: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    mark: @Composable () -> Unit
) {
    val rowMod = Modifier.fillMaxWidth().let { if (rowTapNav) it.bounceClick(onClick = onAction) else it }
    Row(rowMod, verticalAlignment = Alignment.CenterVertically) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            maxLines = 1,
            modifier = Modifier.width(84.dp)
        )
        if (figure != null) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(figure, style = MaterialTheme.typography.headlineSmall, color = onBg)
                if (unit != null) {
                    Spacer(Modifier.width(3.dp))
                    Text(
                        unit,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 9.sp,
                        modifier = Modifier.padding(bottom = 3.dp)
                    )
                }
                delta?.let {
                    Spacer(Modifier.width(6.dp))
                    Text(
                        "${if (it.up) "↑" else "↓"} ${it.value}",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 9.sp,
                        modifier = Modifier.padding(bottom = 3.dp)
                    )
                }
            }
        }
        Spacer(Modifier.weight(1f))
        mark()
        Spacer(Modifier.width(12.dp))
        if (rowTapNav) {
            Text(action, style = MaterialTheme.typography.labelMedium, color = accent)
        } else {
            Text(
                action,
                style = MaterialTheme.typography.labelMedium,
                color = accent,
                modifier = Modifier.bounceClick(onClick = onAction).padding(vertical = 6.dp, horizontal = 4.dp)
            )
        }
    }
}

/** Five coverage pips (filled = that circumference has a reading), right-aligned in the mark box. */
@Composable
private fun MeasurementPips(state: BodyMeasurementsUiState, muted: Color, accent: Color) {
    Row(
        markMod,
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
        verticalAlignment = Alignment.CenterVertically
    ) {
        state.series.forEach { series ->
            val filled = series.entries.isNotEmpty()
            Canvas(Modifier.size(9.dp)) {
                val r = size.minDimension / 2f
                if (filled) drawCircle(accent, radius = r)
                else drawCircle(muted.copy(alpha = 0.55f), radius = r - 0.75.dp.toPx(), style = Stroke(width = 1.5.dp.toPx()))
            }
        }
    }
}

/** A flat baseline in the mark box — the "no readings yet" ghost for a metric shown beside live siblings (§12). */
@Composable
private fun GhostSpark(muted: Color) {
    Canvas(markMod) {
        val y = size.height / 2f
        drawLine(
            color = muted.copy(alpha = 0.3f),
            start = Offset(0f, y),
            end = Offset(size.width, y),
            strokeWidth = 1.5.dp.toPx(),
            cap = StrokeCap.Round
        )
    }
}

/** Fold a raw signed change into a display delta, dropping sub-0.05 wobble that would round to "0.0". */
private fun Double?.asMetricDelta(): MetricDelta? =
    this?.takeIf { abs(it) >= 0.05 }?.let { MetricDelta(it > 0, "%.1f".format(abs(it))) }

/**
 * Trailing 7-calendar-day moving average aligned index-for-index with [display] (and [entries]):
 * each point averages the display weights of every weigh-in whose day is within the 6 days before it
 * (inclusive). One row per calendar day (unique date_key), so a window holds at most 7 values.
 */
private fun sevenDayMovingAverage(entries: List<BodyweightEntry>, display: List<Double>): List<Double> {
    if (entries.isEmpty()) return emptyList()
    val days = entries.map { LocalDate.parse(it.dateKey).toEpochDay() }
    return display.indices.map { i ->
        val floor = days[i] - 6
        var sum = 0.0
        var n = 0
        // entries are chronological, so walk back from i while still inside the trailing window.
        var j = i
        while (j >= 0 && days[j] >= floor) { sum += display[j]; n++; j-- }
        sum / n
    }
}
