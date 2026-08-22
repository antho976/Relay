package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.toDisplayLength
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.experiment.HeroFigure
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.SurfaceSparkline
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * # Profile's open sections (2026-08-22)
 *
 * The page dropped its cards. Every section here used to be a `SurfaceCard` — one fill, one
 * hairline border, 16dp of inner padding — and now sits directly on the page in the app's own
 * editorial language (`ui/common/Editorial.kt`), which is what Home and the live session already
 * speak. **The icons stayed; the boxes went** (Antho, 2026-08-22).
 *
 * ## What the fill was actually doing, and what replaced it
 *
 * A card fill does three jobs at once: it groups, it separates, and it raises. Removing it means
 * paying for each separately, or the page collapses into an undifferentiated column.
 *
 * - **Grouping** is now the section anchor plus the space around it — more air above a heading than
 *   below it, so a gap reads as a break rather than as a mistake.
 * - **Separating** is [EditorialHairline] between rows inside a section. A rule is drawn only where
 *   two readings genuinely need dividing, never as decoration.
 * - **Raising** is not replaced, and does not need to be: nothing on this page was ever above
 *   anything else. The elevation was inherited from a dashboard reference, not earned here.
 *
 * ## The two things de-boxing bought
 *
 * The BODY strip was a swipeable row of cards, and its own doc admitted the cost: *"BODY is no
 * longer scannable in one glance — you have to swipe to know whether MUSCLE has data."* As rows,
 * every metric is visible at once, which is the whole job of that section.
 *
 * The stat cards each carried an `ALL TIME` caption under their figure, because a card has no
 * header to inherit from. Under a real `ALL TIME` anchor the caption is the same fact written
 * twice, so it is gone (§4.3, one home).
 *
 * ## What the fill's removal cost
 *
 * The on-card contrast compensation goes with it. [SurfacePalette.mutedOnCard] exists because 0.65
 * muted measures 4.39:1 on the card fill and fails AA; on the page it measures 4.54:1 and passes.
 * Every caption here is back on plain `muted` — one rung darker than it was, and correct.
 */

/** The icon column. One width for every row on the page, so labels align down the whole screen. */
private val ROW_ICON = 18.dp
private val ROW_ICON_GAP = 12.dp

/** A row's minimum height — the 48dp touch target the tappable BODY rows owe Material. */
private val ROW_MIN_HEIGHT = 48.dp

// ── All time ──────────────────────────────────────────────────────────────────────────────────

/**
 * ALL TIME — the lifetime figure over its curve, then the two tallies whose week-over-week movement
 * means something, each with a two-bar this-week / last-week comparison.
 *
 * **The hero carries no delta, deliberately.** A lifetime cumulative total only ever rises, so a
 * "+2% this week" beside it would be a number with no decision attached (§2①). The honest
 * week-over-week reading lives on the rows below, where the figures actually move.
 *
 * The volume's name sits BELOW its figure, not above it. An eyebrow over a heading is banned
 * outright by the craft floor, and the section anchor already introduces the group — so the caption
 * is what the number turned out to be, not a label announcing it in advance.
 */
@Composable
internal fun ProfileAllTime(
    palette: SurfacePalette,
    totalVolumeLb: Double,
    series: List<Double>,
    totalSets: Int,
    totalSessions: Int,
    workoutsThisWeek: Int,
    workoutsLastWeek: Int,
    totalPrs: Int,
    prsThisWeek: Int,
    prsLastWeek: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val displaySeries = remember(series, weightUnit) { series.map { toDisplayWeight(it, weightUnit) } }

    Column(modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.Bottom) {
            HeroFigure(formatVolumeCompact(totalVolumeLb, weightUnit, withUnit = false), onBg)
            Spacer(Modifier.width(6.dp))
            Text(
                unitLabel(weightUnit).uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                modifier = Modifier.padding(bottom = 8.dp)
            )
        }
        Spacer(Modifier.height(4.dp))
        Text(
            "LIFETIME VOLUME · ${compactCount(totalSets)} SETS",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp
        )
        // Under two sessions there is no curve, and one point drawn flat would read as broken (§12).
        if (displaySeries.size >= 2) {
            Spacer(Modifier.height(16.dp))
            SurfaceSparkline(
                values = displaySeries,
                color = palette.hues[0],
                reading = "Lifetime volume over ${displaySeries.size} sessions, " +
                    "now ${displaySeries.last().roundToInt()} ${unitLabel(weightUnit)}",
                modifier = Modifier.fillMaxWidth().height(60.dp)
            )
        }

        Spacer(Modifier.height(22.dp))
        EditorialHairline(outline)
        AllTimeRow(
            SettingsIcons.Session, "WORKOUTS", "$totalSessions",
            workoutsThisWeek, workoutsLastWeek, "workouts", palette.hues[0], onBg, muted
        )
        EditorialHairline(outline)
        AllTimeRow(
            NavIcons.Stats, "PRS", "$totalPrs",
            prsThisWeek, prsLastWeek, "PRs", palette.hues[1], onBg, muted
        )
    }
}

/**
 * One tally: the glyph and its name on the left, the lifetime figure right-aligned, and — only when
 * there is a week worth comparing — the two-bar comparison indented under the name.
 *
 * The bars carry their own counts as text on the right, so the comparison never depends on reading
 * a bar length, or on colour (§14).
 */
@Composable
private fun AllTimeRow(
    icon: ImageVector,
    label: String,
    figure: String,
    thisWeek: Int,
    lastWeek: Int,
    noun: String,
    hue: Color,
    onBg: Color,
    muted: Color
) {
    Column(Modifier.fillMaxWidth().padding(vertical = 14.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            // Decorative (§14): the label beside it already says the word.
            Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(ROW_ICON))
            Spacer(Modifier.width(ROW_ICON_GAP))
            Text(
                label,
                style = MaterialTheme.typography.labelLarge,
                color = muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(12.dp))
            Text(figure, style = MaterialTheme.typography.headlineSmall, color = onBg, maxLines = 1)
        }
        // Nothing either week → draw NOTHING. Two empty tracks over "0 THIS WK · 0 LAST" was the
        // first thing Antho flagged (2026-08-15): §12 says an all-ghost group drops its mark,
        // because a pair of flat lines reads as broken rather than as empty. The row still answers
        // honestly above — a real 0 — and gains its bars the moment there is a week to compare.
        if (thisWeek == 0 && lastWeek == 0) return@Column
        Spacer(Modifier.height(12.dp))
        Column(
            Modifier
                .fillMaxWidth()
                .padding(start = ROW_ICON + ROW_ICON_GAP)
                .semantics(mergeDescendants = true) {
                    contentDescription = "$thisWeek $noun this week, $lastWeek last week"
                },
            verticalArrangement = Arrangement.spacedBy(7.dp)
        ) {
            val peak = maxOf(thisWeek, lastWeek, 1)
            ComparisonBar("THIS WK", thisWeek, thisWeek.toFloat() / peak, hue, onBg, muted)
            ComparisonBar("LAST WK", lastWeek, lastWeek.toFloat() / peak, muted.copy(alpha = 0.4f), onBg, muted)
        }
    }
}

/** `THIS WK ▬▬▬▬▬▬ 4` — a named track with its count, so the bar is a second channel, never the only one. */
@Composable
private fun ComparisonBar(
    label: String,
    count: Int,
    fraction: Float,
    color: Color,
    onBg: Color,
    muted: Color
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            maxLines = 1,
            modifier = Modifier.width(56.dp)
        )
        Box(
            Modifier
                .weight(1f)
                .height(4.dp)
                .clip(RoundedCornerShape(50))
                .background(muted.copy(alpha = 0.18f))
        ) {
            val f = fraction.coerceIn(0f, 1f)
            if (f > 0f) {
                Box(
                    Modifier
                        .fillMaxWidth(f)
                        .fillMaxHeight()
                        .clip(RoundedCornerShape(50))
                        .background(color)
                )
            }
        }
        Spacer(Modifier.width(10.dp))
        Text(
            "$count",
            style = MaterialTheme.typography.labelSmall,
            color = onBg,
            maxLines = 1,
            textAlign = TextAlign.End,
            modifier = Modifier.width(24.dp)
        )
    }
}

// ── Body ──────────────────────────────────────────────────────────────────────────────────────

/** One body metric, flattened for a row. A null [figure] means nothing has been logged yet. */
private data class BodyMetric(
    val label: String,
    val figure: String?,
    val unit: String?,
    val deltaValue: Double?,
    val series: List<Double>,
    /** The verb TalkBack announces for the row's tap: "Log weight", "Open sizes". */
    val action: String,
    /**
     * What the row says when it has nothing to show. Written per metric, not composed from
     * [action]: the cards derived it as "$action your first" and SIZES came out "Open your first",
     * which is not a sentence. A zero state is the one line a new user is guaranteed to read, so it
     * names its own action in the app's own words (craft floor — controls name their action).
     */
    val zeroLabel: String,
    val icon: ImageVector,
    val onOpen: () -> Unit
)

/**
 * BODY as a stack of open rows — glyph, name, trend, reading — separated by hairlines.
 *
 * This replaces the swipeable card strip (2026-08-22). The strip gave each metric its own figure,
 * delta and sparkline at a generous size, and charged the section its scanability: you had to swipe
 * to learn whether MUSCLE had data at all. Rows give that back. The sparkline survives the move at
 * a smaller size, in the slot between the name and the figure, which is the trade the shipped
 * `BodyMetricsSection` made too — and the reason it read well.
 *
 * Zero is drawn by the rows themselves: a metric with no readings still shows its name and its
 * action, exactly as the cards did (§12 — the rows ARE the zero-shape).
 */
@Composable
internal fun ProfileBodyRows(
    palette: SurfacePalette,
    bodyweight: List<BodyweightEntry>,
    bodyFat: List<BodyFatEntry>,
    onLogWeight: () -> Unit,
    onLogBodyFat: () -> Unit,
    onOpenMeasurements: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier,
    measurementsVm: BodyMeasurementsViewModel = hiltViewModel(),
    leanMassVm: LeanMassViewModel = hiltViewModel()
) {
    val measurements by measurementsVm.state.collectAsStateWithLifecycle()
    val leanMass by leanMassVm.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit

    val metrics = buildList {
        add(weightMetric(bodyweight, weightUnit, onLogWeight))
        add(bodyFatMetric(bodyFat, onLogBodyFat))
        // The MUSCLE row exists only for a connected watch (or leftover data after a disconnect):
        // an HC-only metric never shows an unconnected ghost here, and Recovery owns the connect
        // flow (§12 stale/denied — hidden when the grant was never given).
        if (leanMass.connected || leanMass.entries.isNotEmpty()) {
            add(leanMassMetric(leanMass.entries, weightUnit) { leanMassVm.syncNow() })
        }
        add(sizesMetric(measurements, onOpenMeasurements))
    }

    Column(modifier.fillMaxWidth()) {
        metrics.forEachIndexed { i, metric ->
            EditorialHairline(outline)
            BodyMetricRow(
                metric = metric,
                hue = palette.hues[i % palette.hues.size],
                onBg = onBg,
                muted = muted
            )
        }
        EditorialHairline(outline)
    }
}

@Composable
private fun BodyMetricRow(metric: BodyMetric, hue: Color, onBg: Color, muted: Color) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = ROW_MIN_HEIGHT)
            .bounceCombinedClick(onClickLabel = "${metric.action} ${metric.label.lowercase()}") { metric.onOpen() }
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(metric.icon, contentDescription = null, tint = muted, modifier = Modifier.size(ROW_ICON))
        Spacer(Modifier.width(ROW_ICON_GAP))
        // widthIn, NOT width. A fixed 84dp column aligned the labels beautifully at 1.0 and clipped
        // "BODY FAT" to "BODY F…" at the 1.3 font scale — measured on device. The minimum keeps the
        // alignment for every label that fits it; anything longer takes the width it needs and the
        // trend beside it gives up the difference, because a name that cannot be read is worth less
        // than a sparkline that is 20dp narrower.
        Text(
            metric.label,
            style = MaterialTheme.typography.labelLarge,
            color = muted,
            maxLines = 1,
            modifier = Modifier.widthIn(min = 84.dp)
        )
        Spacer(Modifier.width(12.dp))
        // The trend takes whatever the row has left. A metric with no trend draws NO mark at all —
        // the shipped rows made the same call 2026-07-25: an empty track is decoration, not an
        // empty state.
        if (metric.series.size >= 2) {
            SurfaceSparkline(
                values = metric.series,
                color = hue,
                reading = "${metric.label}, ${metric.series.size} readings, latest ${metric.figure ?: "none"}",
                modifier = Modifier.weight(1f).height(24.dp)
            )
        } else {
            Spacer(Modifier.weight(1f))
        }
        Spacer(Modifier.width(12.dp))
        if (metric.figure != null) {
            Column(horizontalAlignment = Alignment.End) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(metric.figure, style = MaterialTheme.typography.titleLarge, color = onBg, maxLines = 1)
                    metric.unit?.let {
                        Spacer(Modifier.width(3.dp))
                        Text(
                            it,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp,
                            modifier = Modifier.padding(bottom = 2.dp)
                        )
                    }
                }
                metric.deltaValue?.let { d ->
                    Spacer(Modifier.height(2.dp))
                    Text(
                        "${if (d > 0) "↑" else "↓"} ${"%.1f".format(abs(d))}",
                        style = MaterialTheme.typography.labelSmall,
                        // Direction only: gaining or losing weight is not a verdict, so this stays
                        // on the muted rung rather than reaching for positive/negative (§11).
                        color = muted,
                        fontSize = 9.sp,
                        maxLines = 1
                    )
                }
            }
        } else {
            // Zero: name the action, not the absence. The row is the tap target, so the word is
            // drawn rather than being a second nested control (§2③).
            Text(
                metric.zeroLabel,
                style = MaterialTheme.typography.bodySmall,
                color = onBg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.End
            )
        }
    }
}

// ── Metric builders ───────────────────────────────────────────────────────────────────────────

/** The comparison window for the "vs about a month ago" delta, matching the shipped body rows. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

private fun weightMetric(
    entries: List<BodyweightEntry>,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onLog: () -> Unit
): BodyMetric {
    val display = entries.map { toDisplayWeight(it.weightLb, weightUnit) }
    return BodyMetric(
        label = "WEIGHT",
        figure = display.lastOrNull()?.roundToInt()?.toString(),
        unit = unitLabel(weightUnit).uppercase(),
        deltaValue = windowDelta(entries.map { it.recordedAt }, display),
        series = display,
        action = "Log",
        zeroLabel = "Log your first",
        icon = SettingsIcons.Units,
        onOpen = onLog
    )
}

private fun bodyFatMetric(entries: List<BodyFatEntry>, onLog: () -> Unit): BodyMetric {
    val values = entries.map { it.percent }
    return BodyMetric(
        label = "BODY FAT",
        figure = values.lastOrNull()?.let { "%.1f".format(it) },
        unit = "%",
        deltaValue = windowDelta(entries.map { it.recordedAt }, values),
        series = values,
        action = "Log",
        zeroLabel = "Log your first",
        icon = SettingsIcons.Likes,
        onOpen = onLog
    )
}

private fun leanMassMetric(
    entries: List<LeanMassEntry>,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onSync: () -> Unit
): BodyMetric {
    val display = entries.map { toDisplayWeight(it.weightLb, weightUnit) }
    return BodyMetric(
        label = "MUSCLE",
        figure = display.lastOrNull()?.let { "%.1f".format(it) },
        unit = unitLabel(weightUnit).uppercase(),
        deltaValue = windowDelta(entries.map { it.recordedAt }, display),
        series = display,
        action = "Sync",
        zeroLabel = "Sync from your watch",
        icon = SettingsIcons.Recovery,
        onOpen = onSync
    )
}

/**
 * SIZES — the featured circumference, waist by convention, falling back to whichever site was
 * logged most recently. Same rule as the shipped row: a named site with a trend answers "which"
 * by saying it, where a coverage count ("5 of 5") saturates in a fortnight and then reports the
 * same fact forever.
 */
private fun sizesMetric(state: BodyMeasurementsUiState, onOpen: () -> Unit): BodyMetric {
    val tracked = state.series.filter { it.entries.isNotEmpty() }
    val featured = tracked.firstOrNull { it.type == BodyMeasurementType.WAIST }
        ?: tracked.maxByOrNull { it.entries.last().recordedAt }
    val display = featured?.entries.orEmpty().map { toDisplayLength(it.valueCm, state.useCm) }
    val delta = featured?.entries.orEmpty().let { e ->
        if (e.size < 2) null
        else (toDisplayLength(e.last().valueCm, state.useCm) - toDisplayLength(e.first().valueCm, state.useCm))
            .takeIf { abs(it) >= 0.05 }
    }
    return BodyMetric(
        label = featured?.type?.label?.uppercase() ?: "SIZES",
        figure = featured?.let { lengthInputValue(it.entries.last().valueCm, state.useCm) },
        unit = if (featured == null) null else lengthUnitLabel(state.useCm).uppercase(),
        deltaValue = delta,
        series = display,
        action = "Open",
        zeroLabel = "Measure your first",
        icon = SettingsIcons.Program,
        onOpen = onOpen
    )
}

/** Signed change across roughly the last 30 days, dropping wobble that would round to "0.0". */
private fun windowDelta(timestamps: List<Long>, values: List<Double>): Double? {
    if (values.size < 2 || timestamps.size != values.size) return null
    val cutoff = timestamps.last() - DELTA_WINDOW_MS
    val refIdx = timestamps.indexOfFirst { it >= cutoff }
        .coerceAtMost(values.lastIndex - 1)
        .coerceAtLeast(0)
    return (values.last() - values[refIdx]).takeIf { abs(it) >= 0.05 }
}

/** "1,240" → "1.2k"; small counts stay exact (§11: k-abbreviate at 10,000, keep 1k readable here). */
private fun compactCount(n: Int): String = when {
    n >= 10_000 -> "${(n / 1000.0).roundToInt()}k"
    n >= 1_000 -> "${"%.1f".format(n / 1000.0)}k"
    else -> "$n"
}
