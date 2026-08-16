package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
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
import com.forge.app.ui.experiment.CardEyebrow
import com.forge.app.ui.experiment.CardMark
import com.forge.app.ui.experiment.DeltaBadge
import com.forge.app.ui.experiment.HeroFigure
import com.forge.app.ui.experiment.PeekCardRow
import com.forge.app.ui.experiment.peekCardWidth
import com.forge.app.ui.experiment.SurfaceCard
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.SurfaceSparkline
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * design/surface-experiment (2026-08-15) — Profile's card sections.
 *
 * The same language as Home, applied under the cover photo. The cover itself ([ProfileHeaderCard])
 * is UNTOUCHED: it is the one part of the shipped design that already has the warmth this branch is
 * chasing, so the cards are built to sit under it rather than compete with it — which is why the
 * first card starts a full 28dp below the cover's dissolve and nothing here uses a photo, a
 * gradient, or a second elevation.
 *
 * Everything is experiment-scoped. The shipped sections (`AllTimeSection`, `BodyMetricsSection`,
 * `LifetimeVolumeGraph`, `ProfileTiles`) are all still here, untouched and uncalled.
 */

/** A body card's minimum height, so the strip keeps one baseline while still growing at 200%. */
private val BODY_CARD_MIN_HEIGHT = 148.dp

// ── Hero ──────────────────────────────────────────────────────────────────────────────────────

/**
 * PROFILE'S HERO CARD — lifetime volume as the serif figure over its cumulative curve.
 *
 * **It carries no delta badge, and that is deliberate.** The reference dashboard pairs every hero
 * figure with a percentage, but a lifetime cumulative total only ever rises, so a "+2% this week"
 * beside it would be a number with no decision attached — §2① says cut those. The honest
 * week-over-week reading lives on the two-up row below, where the figures actually move.
 *
 * This is the first place the fintech reference stops translating, and it is worth noticing: that
 * layout assumes a balance that can fall.
 */
@Composable
internal fun ProfileHeroCard(
    palette: SurfacePalette,
    totalVolumeLb: Double,
    series: List<Double>,
    totalSets: Int,
    sinceLabel: String,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val displaySeries = remember(series, weightUnit) { series.map { toDisplayWeight(it, weightUnit) } }
    SurfaceCard(palette, modifier, padding = PaddingValues(18.dp)) {
        // No "since <month>" here: the cover directly above already carries SINCE JUL 2026, and a
        // fact belongs to one place on a screen (§4.3 "one home"). Antho caught it written twice.
        CardEyebrow("Lifetime volume", muted)
        Spacer(Modifier.height(10.dp))
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
        // Under two sessions there is no curve, and one point drawn flat would read as broken (§12).
        if (displaySeries.size >= 2) {
            Spacer(Modifier.height(12.dp))
            SurfaceSparkline(
                values = displaySeries,
                color = palette.hues[0],
                reading = "Lifetime volume over ${displaySeries.size} sessions, " +
                    "now ${displaySeries.last().roundToInt()} ${unitLabel(weightUnit)}",
                modifier = Modifier.fillMaxWidth().height(56.dp)
            )
        }
        Spacer(Modifier.height(14.dp))
        Text(
            "${compactCount(totalSets)} SETS LOGGED",
            style = MaterialTheme.typography.labelMedium,
            color = palette.mutedOnCard
        )
    }
}

// ── Two-up ────────────────────────────────────────────────────────────────────────────────────

/**
 * The two lifetime tallies whose week-over-week movement is actually interesting, each with a
 * two-bar this-week / last-week comparison as its mark (§2②: ranked comparison → thin bars).
 *
 * Both bars empty is a real reading, so the mark still draws on a brand-new profile.
 */
@Composable
internal fun ProfileTwoUp(
    palette: SurfacePalette,
    totalSessions: Int,
    workoutsThisWeek: Int,
    workoutsLastWeek: Int,
    totalPrs: Int,
    prsThisWeek: Int,
    prsLastWeek: Int,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    Row(
        modifier.fillMaxWidth().height(androidx.compose.foundation.layout.IntrinsicSize.Min),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        ProfileStatCard(
            palette, SettingsIcons.Session, palette.hues[0], "Workouts", "$totalSessions",
            workoutsThisWeek, workoutsLastWeek, "workouts", onBg, muted,
            Modifier.weight(1f).fillMaxHeight()
        )
        ProfileStatCard(
            palette, NavIcons.Stats, palette.hues[1], "PRs", "$totalPrs",
            prsThisWeek, prsLastWeek, "PRs", onBg, muted,
            Modifier.weight(1f).fillMaxHeight()
        )
    }
}

@Composable
private fun ProfileStatCard(
    palette: SurfacePalette,
    icon: ImageVector,
    hue: Color,
    label: String,
    figure: String,
    thisWeek: Int,
    lastWeek: Int,
    noun: String,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    SurfaceCard(palette, modifier, padding = PaddingValues(16.dp)) {
        CardMark(icon, hue)
        Spacer(Modifier.height(12.dp))
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(Modifier.height(4.dp))
        Text(figure, style = MaterialTheme.typography.headlineMedium, color = onBg, maxLines = 1)
        Spacer(Modifier.height(3.dp))
        Text("ALL TIME", style = MaterialTheme.typography.labelSmall, color = palette.mutedOnCard)
        if (thisWeek > 0 || lastWeek > 0) {
            Spacer(Modifier.height(14.dp))
            WeekComparisonBars(thisWeek, lastWeek, hue, muted, noun, palette)
        }
    }
}

/**
 * Two thin bars: this week over last week, scaled to whichever is larger. The reading below names
 * both counts, so the comparison never depends on reading a bar length — or on colour.
 */
@Composable
private fun WeekComparisonBars(
    thisWeek: Int,
    lastWeek: Int,
    hue: Color,
    muted: Color,
    noun: String,
    palette: SurfacePalette
) {
    // Nothing either week → draw NOTHING. Two empty tracks over "0 THIS WK · 0 LAST" was the
    // first thing Antho flagged (2026-08-15): §12 says an all-ghost group drops its mark, because
    // a pair of flat lines reads as broken rather than as empty. The card still answers honestly
    // above — a real 0 under ALL TIME — and gains its bars the moment there is a week to compare.
    if (thisWeek == 0 && lastWeek == 0) return
    val peak = maxOf(thisWeek, lastWeek, 1)
    Column(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {
                contentDescription = "$thisWeek $noun this week, $lastWeek last week"
            },
        verticalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        ComparisonBar(thisWeek.toFloat() / peak, hue, muted)
        ComparisonBar(lastWeek.toFloat() / peak, muted.copy(alpha = 0.35f), muted)
        Spacer(Modifier.height(3.dp))
        Text(
            "$thisWeek THIS WK · $lastWeek LAST",
            style = MaterialTheme.typography.labelSmall,
            color = palette.mutedOnCard,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
    }
}

@Composable
private fun ComparisonBar(fraction: Float, color: Color, muted: Color) {
    Box(
        Modifier
            .fillMaxWidth()
            .height(4.dp)
            .clip(RoundedCornerShape(50))
            .background(muted.copy(alpha = 0.25f))
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
}

// ── The body strip ────────────────────────────────────────────────────────────────────────────

/** One body metric, flattened for the strip. A null [figure] means nothing has been logged yet. */
private data class BodyCard(
    val label: String,
    val figure: String?,
    val unit: String?,
    val deltaValue: Double?,
    val series: List<Double>,
    val action: String,
    val icon: ImageVector,
    val onOpen: () -> Unit
)

/**
 * BODY as a horizontal strip of cards — the section this direction suits best.
 *
 * The shipped `BodyMetricsSection` is a rail of rows, which reads well but forces every metric to
 * share one 108dp mark slot. A card per metric gives each its own figure, delta and sparkline at a
 * size worth drawing, and the peek makes a fifth metric free to add. The cost: BODY is no longer
 * scannable in one glance — you have to swipe to know whether MUSCLE has data.
 *
 * Zero is drawn by the cards themselves: a metric with no readings still shows its name and its
 * action, exactly as the shipped rows do (§12 — the rows ARE the zero-shape, here the cards are).
 */
@Composable
internal fun ProfileBodyStrip(
    palette: SurfacePalette,
    bodyweight: List<BodyweightEntry>,
    bodyFat: List<BodyFatEntry>,
    onLogWeight: () -> Unit,
    onLogBodyFat: () -> Unit,
    onOpenMeasurements: () -> Unit,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier,
    measurementsVm: BodyMeasurementsViewModel = hiltViewModel(),
    leanMassVm: LeanMassViewModel = hiltViewModel()
) {
    val measurements by measurementsVm.state.collectAsStateWithLifecycle()
    val leanMass by leanMassVm.state.collectAsStateWithLifecycle()
    val settings = LocalForgeSettings.current
    val weightUnit = settings.weightUnit

    val cards = buildList {
        add(weightCard(bodyweight, weightUnit, onLogWeight))
        add(bodyFatCard(bodyFat, onLogBodyFat))
        // The MUSCLE card exists only for a connected watch (or leftover data after a disconnect):
        // an HC-only metric never shows an unconnected ghost here, and Recovery owns the connect
        // flow (§12 stale/denied — hidden when the grant was never given).
        if (leanMass.connected || leanMass.entries.isNotEmpty()) {
            add(leanMassCard(leanMass.entries, weightUnit) { leanMassVm.syncNow() })
        }
        add(sizesCard(measurements, onOpenMeasurements))
    }

    PeekCardRow(modifier) {
        itemsIndexed(cards) { i, card ->
            BodyMetricCard(
                palette = palette,
                card = card,
                hue = palette.hues[i % palette.hues.size],
                onBg = onBg,
                muted = muted
            )
        }
    }
}

@Composable
private fun BodyMetricCard(
    palette: SurfacePalette,
    card: BodyCard,
    hue: Color,
    onBg: Color,
    muted: Color
) {
    SurfaceCard(
        palette,
        Modifier.width(peekCardWidth()),
        onClick = card.onOpen,
        clickLabel = "${card.action} ${card.label.lowercase()}",
        padding = PaddingValues(14.dp),
        minHeight = BODY_CARD_MIN_HEIGHT
    ) {
        CardMark(card.icon, hue)
        Spacer(Modifier.height(10.dp))
        Text(
            card.label,
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(Modifier.height(4.dp))
        if (card.figure != null) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(card.figure, style = MaterialTheme.typography.headlineSmall, color = onBg, maxLines = 1)
                card.unit?.let {
                    Spacer(Modifier.width(3.dp))
                    Text(
                        it,
                        style = MaterialTheme.typography.labelSmall,
                        color = palette.mutedOnCard,
                        modifier = Modifier.padding(bottom = 3.dp)
                    )
                }
            }
            card.deltaValue?.let { d ->
                Spacer(Modifier.height(4.dp))
                Text(
                    "${if (d > 0) "↑" else "↓"} ${"%.1f".format(abs(d))}",
                    style = MaterialTheme.typography.labelSmall,
                    // Direction only: gaining or losing weight is not a verdict, so this stays on
                    // the muted rung rather than reaching for positive/negative (§11).
                    color = palette.mutedOnCard,
                    maxLines = 1
                )
            }
        } else {
            // Zero: name the action, not the absence. The card is the tap target, so the word is
            // drawn rather than being a second nested control (§2③).
            Text(
                "${card.action} your first",
                style = MaterialTheme.typography.bodySmall,
                color = onBg,
                maxLines = 2
            )
        }
        Spacer(Modifier.height(12.dp))
        if (card.series.size >= 2) {
            SurfaceSparkline(
                values = card.series,
                color = hue,
                reading = "${card.label}, ${card.series.size} readings, latest ${card.figure ?: "none"}",
                modifier = Modifier.fillMaxWidth().height(26.dp)
            )
        } else {
            // A metric with no trend draws NO mark at all — the shipped rows made the same call
            // 2026-07-25: an empty track is decoration, not an empty state. The slot keeps its
            // height so the strip's cards stay level.
            Spacer(Modifier.height(26.dp))
        }
    }
}

// ── Card builders ─────────────────────────────────────────────────────────────────────────────

/** The comparison window for the "vs about a month ago" delta, matching the shipped body rows. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

private fun weightCard(
    entries: List<BodyweightEntry>,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onLog: () -> Unit
): BodyCard {
    val display = entries.map { toDisplayWeight(it.weightLb, weightUnit) }
    return BodyCard(
        label = "WEIGHT",
        figure = display.lastOrNull()?.roundToInt()?.toString(),
        unit = unitLabel(weightUnit).uppercase(),
        deltaValue = windowDelta(entries.map { it.recordedAt }, display),
        series = display,
        action = "Log",
        icon = SettingsIcons.Units,
        onOpen = onLog
    )
}

private fun bodyFatCard(entries: List<BodyFatEntry>, onLog: () -> Unit): BodyCard {
    val values = entries.map { it.percent }
    return BodyCard(
        label = "BODY FAT",
        figure = values.lastOrNull()?.let { "%.1f".format(it) },
        unit = "%",
        deltaValue = windowDelta(entries.map { it.recordedAt }, values),
        series = values,
        action = "Log",
        icon = SettingsIcons.Likes,
        onOpen = onLog
    )
}

private fun leanMassCard(
    entries: List<LeanMassEntry>,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    onSync: () -> Unit
): BodyCard {
    val display = entries.map { toDisplayWeight(it.weightLb, weightUnit) }
    return BodyCard(
        label = "MUSCLE",
        figure = display.lastOrNull()?.let { "%.1f".format(it) },
        unit = unitLabel(weightUnit).uppercase(),
        deltaValue = windowDelta(entries.map { it.recordedAt }, display),
        series = display,
        action = "Sync",
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
private fun sizesCard(state: BodyMeasurementsUiState, onOpen: () -> Unit): BodyCard {
    val tracked = state.series.filter { it.entries.isNotEmpty() }
    val featured = tracked.firstOrNull { it.type == BodyMeasurementType.WAIST }
        ?: tracked.maxByOrNull { it.entries.last().recordedAt }
    val display = featured?.entries.orEmpty().map { toDisplayLength(it.valueCm, state.useCm) }
    val delta = featured?.entries.orEmpty().let { e ->
        if (e.size < 2) null
        else (toDisplayLength(e.last().valueCm, state.useCm) - toDisplayLength(e.first().valueCm, state.useCm))
            .takeIf { abs(it) >= 0.05 }
    }
    return BodyCard(
        label = featured?.type?.label?.uppercase() ?: "SIZES",
        figure = featured?.let { lengthInputValue(it.entries.last().valueCm, state.useCm) },
        unit = if (featured == null) null else lengthUnitLabel(state.useCm).uppercase(),
        deltaValue = delta,
        series = display,
        action = "Open",
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

// ── Year grid ─────────────────────────────────────────────────────────────────────────────────

/**
 * THIS YEAR inside a card. The grid itself is the shipped [YearConsistencySection], untouched — the
 * card is only a container around it, which is the cleanest single test of §1 on this page: the
 * exact same mark, once bare and once boxed.
 */
@Composable
internal fun ProfileYearCard(
    palette: SurfacePalette,
    activityByDay: Map<Long, Int>,
    muted: Color,
    modifier: Modifier = Modifier
) {
    SurfaceCard(palette, modifier, padding = PaddingValues(16.dp)) {
        YearConsistencySection(activityByDay, muted, palette.hues[0])
    }
}

/** A card carrying one delta-bearing figure — used for the gamification sections when re-enabled. */
@Composable
internal fun ProfileFigureCard(
    palette: SurfacePalette,
    label: String,
    figure: String,
    deltaPercent: Int?,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    SurfaceCard(palette, modifier) {
        CardEyebrow(label, muted)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(figure, style = MaterialTheme.typography.headlineMedium, color = onBg, maxLines = 1)
            Spacer(Modifier.weight(1f))
            if (deltaPercent != null) DeltaBadge(deltaPercent, palette, muted)
        }
    }
}
