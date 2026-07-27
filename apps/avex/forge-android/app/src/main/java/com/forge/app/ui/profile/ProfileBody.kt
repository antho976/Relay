package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.toDisplayLength
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.bounceClick
import androidx.compose.ui.text.TextStyle
import com.forge.app.ui.theme.MonoSectionAnchor
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import kotlin.math.abs
import kotlin.math.roundToInt

/** The comparison window for the little "vs a month ago" delta beside a body reading. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

/**
 * The trend-mark box shared by every body row, so the marks right-align on one rail across rows.
 *
 * 108dp, widened from 72dp on device: at 72 a 30-day sparkline had no room to show shape, and the
 * empty-state baseline read as a detached dash floating mid-row rather than as the lane a chart will
 * fill. §12 asks for "visual mass at the data's REAL size" — this box was sized for the label beside
 * it, not for the data inside it.
 */
private val markMod = Modifier.width(108.dp).height(26.dp)

/**
 * BODY (Antho 2026-07-13) — bodyweight, body fat and measurements merged from three separate Profile
 * sections into ONE compact stack, so the "your body" cluster reads as a single section. One mono
 * "BODY" header, then one row per metric built on TWO rails (rebuilt 2026-07-24, Antho): the mono
 * metric name with its serif reading + ~30-day delta glued beneath it on the left, the trend mark and
 * a compact outlined action pill on the right. The whole row is the tap target — weigh-in and body-fat
 * rows open their log sheet, MUSCLE syncs, SIZES opens the Measurements screen — so the section never
 * stacks a column of bare accent text links (§8).
 *
 * Empty is drawn (§12), and the ROWS are the zero-shape: a metric with no readings still shows its
 * name and its action pill, so a brand-new profile gets an actionable section instead of a written
 * hint. A metric with no data draws NO mark at all — the slot keeps its width so the marks and pills
 * still share one right edge, but nothing is painted in it. The empty tracks that used to sit there
 * were struck out 2026-07-25 (Antho): a container with nothing in it is decoration, not an empty
 * state, and a row that has genuinely nothing to show is more honest saying so with silence.
 * Measurements reads its own [BodyMeasurementsViewModel] so ProfileViewModel is untouched.
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

    // ── The label layer, rebuilt from scratch 2026-07-26 (Antho) ─────────────────────────────────
    // TWO roles and nothing in between. An ANCHOR (`BODY`, `SIZES`) names a group at the 15sp mono
    // anchor rung; a metric NAME (`WEIGHT`, `BODY FAT`, `WAIST`) names one reading at 11sp. Nothing
    // is a hybrid — SIZES used to be a 15sp label INSIDE a row with the site name beneath it, which
    // is what made this section unreadable: a header nested in a list item belongs to neither level.
    //
    // The GROUPING is done by the rhythm, not by the type. Every gap here was equal before (~28dp
    // above the anchor, ~28dp below it, ~30dp between rows), so the anchor owned nothing and the
    // section read as five equidistant floating labels. Now the rows sit as tight to their anchor as
    // the pill allows and the ONE big break falls before the next anchor.
    //
    // The floor is the ACTION PILL, not the text: each row is as tall as its pill, so a label can
    // never sit closer to the next label than the pill's own height (~18dp between caps at
    // `spacedBy(0)`). Tightening past that means shrinking `ForgeRowPill`, which is a §8 component
    // shared with Settings — out of scope here, and the next lever if this still reads loose.
    SectionHeader("BODY", muted)
    // NO all-empty collapse to a written hint (removed 2026-07-24, Antho): `InlineEmptyHint` is the
    // last resort for a section with no zero-shape (§12), and this one HAS a zero-shape — the rows
    // themselves. Every row is a named metric with a real action pill even before its first reading.
    // 8dp is the FLOOR, set by the pill and not by the type. At 0 the labels sat 18.7dp apart — the
    // tightest this section can be — but each row is as tall as its `Log` pill, so consecutive rows
    // abutted and the two pills' borders touched and read as one merged shape. 8dp is the least that
    // separates them; the labels land ~26dp apart as a consequence, which is the pill's price.
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        WeightRow(bodyweight, bodyweightGoalLb, onLogWeight, onBg, muted, accent)
        BodyFatRow(bodyFat, onLogBodyFat, onBg, muted, accent)
        if (showMuscle) {
            LeanMassRow(leanMass.entries, onSync = { leanMassVm.syncNow() }, onBg, muted, accent)
        }
    }
    // The break. This is the only large gap in the section, so it is the only thing that reads as a
    // division — §7's 28dp above a section anchor, spent once and deliberately.
    Spacer(Modifier.height(28.dp))
    // SIZES is an ANCHOR, not a metric name: it is a group of five circumferences, and the row below
    // names the one being shown. Lifting it out of the row is what lets both levels keep one style.
    SectionHeader("SIZES", muted)
    SizesRow(measurements, onOpenMeasurements, onBg, muted, accent)
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
        action = "Log",
        onOpen = onLog,
        onBg = onBg, muted = muted
    ) {
        if (ma.size >= 2) ProfileSparkline(ma, accent, markMod, reference = goalDisplay)
        else Spacer(markMod)
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
        action = "Log",
        onOpen = onLog,
        onBg = onBg, muted = muted
    ) {
        if (values.size >= 2) ProfileSparkline(values, accent, markMod)
        else Spacer(markMod)
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
        action = "Sync",
        onOpen = onSync,
        onBg = onBg, muted = muted
    ) {
        if (display.size >= 2) ProfileSparkline(display, accent, markMod)
        else Spacer(markMod)
    }
}

/**
 * SIZES row — a trim of the destination (§4.2): ONE named circumference read exactly like its
 * siblings (serif value, unit, change, spark), with the whole row tapping into
 * [BodyMeasurementsScreen] where every site, its trend and its logging live.
 *
 * It used to be five anonymous coverage pips and an "n OF 5" count. That was wrong twice over
 * (2026-07-25, Antho): the pips could not say WHICH site they stood for, and coverage is not a
 * reading at all — it is onboarding state. You do not measure yourself once, so "5 OF 5" saturates
 * within a week or two and then reports the same fact forever, occupying the row where its siblings
 * carry a live number. A named site with a trend never goes stale and answers "which" by saying it.
 *
 * The featured site is the WAIST by convention — the circumference people actually track — falling
 * back to whichever site was logged most recently when the waist has no readings, so the row shows
 * the thing you last cared about rather than nothing.
 */
@Composable
private fun SizesRow(
    state: BodyMeasurementsUiState,
    onOpen: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val featured = remember(state.series) {
        val tracked = state.series.filter { it.entries.isNotEmpty() }
        tracked.firstOrNull { it.type == BodyMeasurementType.WAIST }
            ?: tracked.maxByOrNull { it.entries.last().recordedAt }
    }
    val useCm = state.useCm
    val display = remember(featured, useCm) {
        featured?.entries.orEmpty().map { toDisplayLength(it.valueCm, useCm) }
    }
    // Measured from the FIRST reading and gated on the DISPLAY magnitude, matching the Measurements
    // screen: a circumference is logged sparsely, so "since last" spans an unknown gap, and a wobble
    // that rounds to "0.0 in" is not a change.
    val delta = remember(featured, display) {
        val e = featured?.entries.orEmpty()
        if (e.size < 2) null else (e.last().valueCm - e.first().valueCm)
            .takeIf { toDisplayLength(abs(it), useCm) >= 0.05 }
    }
    BodyMetricRow(
        // The row is the SITE, named at the metric rung exactly like WEIGHT and BODY FAT — the group
        // name now lives on the `SIZES` anchor above it (2026-07-26). With nothing tracked there is
        // no site to name, so it falls back to the group's own word rather than an empty label.
        label = featured?.type?.label?.uppercase() ?: "SIZES",
        figure = featured?.let { lengthInputValue(it.entries.last().valueCm, useCm) },
        unit = if (featured == null) null else lengthUnitLabel(useCm).uppercase(),
        delta = delta?.let { d -> MetricDelta(up = d > 0, value = lengthInputValue(abs(d), useCm)) },
        action = "Open",
        onOpen = onOpen,
        onBg = onBg, muted = muted
    ) {
        if (display.size >= 2) ProfileSparkline(display, accent, markMod) else Spacer(markMod)
    }
}

/**
 * The shared row skeleton — TWO rails and nothing between them. Left: the mono metric name with its
 * serif reading glued directly beneath (unit + ~30-day delta riding the baseline), so a figure never
 * floats mid-row detached from what it measures. Right: the trend mark, then the row's action as a
 * compact outlined [ForgeRowPill]. The WHOLE row is the tap target and the pill is drawn only, never
 * separately clickable (§8) — so there is no nested tap and no column of bare accent text links.
 */
@Composable
private fun BodyMetricRow(
    label: String,
    figure: String?,
    unit: String?,
    delta: MetricDelta?,
    action: String,
    onOpen: () -> Unit,
    onBg: Color,
    muted: Color,
    mark: @Composable () -> Unit
) {
    Row(
        Modifier.fillMaxWidth().bounceClick(onClick = onOpen),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            // labelMedium — 11sp, a 22px cap on this device. It went to 13sp first and read too
            // heavy once the section anchor moved up to its own 15sp rung: with the anchor clearly
            // leading, the row labels no longer need size to be legible, and at 13 they crowded the
            // serif figures they are supposed to introduce.
            Text(label, style = MaterialTheme.typography.labelMedium, color = muted, maxLines = 1)
            if (figure != null) {
                Spacer(Modifier.height(3.dp))
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(figure, style = MaterialTheme.typography.headlineMedium, color = onBg)
                    if (unit != null) {
                        Spacer(Modifier.width(4.dp))
                        Text(
                            unit,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted, fontSize = 9.sp,
                            modifier = Modifier.padding(bottom = 4.dp)
                        )
                    }
                    delta?.let {
                        Spacer(Modifier.width(8.dp))
                        Text(
                            "${if (it.up) "↑" else "↓"} ${it.value}",
                            style = MaterialTheme.typography.labelSmall,
                            color = muted, fontSize = 10.sp,
                            modifier = Modifier.padding(bottom = 4.dp)
                        )
                    }
                }
            }
        }
        Spacer(Modifier.width(12.dp))
        mark()
        Spacer(Modifier.width(12.dp))
        // Fixed-width pill slot. "Open" is wider than "Log", and with a free-width pill the row's
        // mark got pushed left by exactly that difference — so the trend marks did NOT share a right
        // edge across rows, which is the misalignment you can see between the tracks and the pips.
        Box(Modifier.widthIn(min = 60.dp), contentAlignment = Alignment.CenterEnd) {
            ForgeRowPill(action)
        }
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
