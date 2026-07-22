package com.forge.app.ui.gym.stats

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.adapt.E1rm
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.LineChart
import com.forge.app.ui.gym.stats.components.ScatterChart
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.components.staggeredProgress
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.StrengthCurve
import com.forge.app.ui.theme.ForgeMotion
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/** A per-lift strength curve only earns its chart once it has this many plotted sets. */
private const val MIN_POINTS_FOR_LIFT_CURVE = 6

/**
 * The Strength lens — the session-detail pattern applied to the whole history: one tappable row per
 * lift with a comparison bar against your strongest lift, expanding inline into that lift's e1RM
 * trend, its recent PRs and its load-rep strength curve. The standalone PR-timeline and
 * strength-curve cards dissolved into this drill-down (2026-07-01 fusion).
 */
@Composable
internal fun ColumnScope.E1rmComparisonList(
    lifts: List<E1rmLift>,
    prs: List<PrEntry>,
    curves: List<StrengthCurve>,
    weightUnit: WeightUnit,
    c: StatsColors,
    /** A lift to open pre-expanded — set when a record tap deep-links here. */
    focusLift: String? = null,
    /** Bumps on every record tap so re-tapping the same [focusLift] re-expands its row. */
    focusNonce: Int = 0
) {
    val maxE1 = lifts.maxOf { it.currentE1rm }.coerceAtLeast(1.0)
    // The comparison bars glide in on first appearance, staggered down the list — the same motion
    // as the session screen's per-exercise bars.
    val progress = rememberDrawProgress(Unit, ForgeMotion.drawTween())
    lifts.forEachIndexed { i, lift ->
        E1rmDrillRow(
            lift = lift,
            frac = (lift.currentE1rm / maxE1).toFloat(),
            barProgress = staggeredProgress(progress, i, lifts.size),
            prsForLift = prs.filter { it.exerciseName == lift.exerciseName },
            curve = curves.firstOrNull { it.exerciseId == lift.exerciseId },
            weightUnit = weightUnit,
            c = c,
            isFocused = lift.exerciseId == focusLift,
            focusNonce = focusNonce
        )
    }
}

/**
 * One lift row, session-detail style: name + e1RM + expand caret over a comparison bar; tapping
 * opens the lift's full read inline. Lifts with a single logged session aren't expandable (there's
 * nothing to chart) and say so with a quiet tag instead of a caret.
 */
@Composable
private fun E1rmDrillRow(
    lift: E1rmLift,
    frac: Float,
    barProgress: Float,
    prsForLift: List<PrEntry>,
    curve: StrengthCurve?,
    weightUnit: WeightUnit,
    c: StatsColors,
    isFocused: Boolean,
    focusNonce: Int
) {
    val display = remember(lift.history, weightUnit) { lift.history.map { toDisplayWeight(it, weightUnit) } }
    val expandable = display.size >= 2
    var expanded by rememberSaveable(lift.exerciseId) { mutableStateOf(isFocused && expandable) }
    // A record tap deep-links here and focuses this row; focusNonce changes on every such tap, so
    // re-tapping the already-focused lift re-expands it even after the user manually collapsed it.
    // The nonce is CONSUMED (tracked in a saveable) rather than re-applied on every first
    // composition — otherwise returning to Stats or rotating would restore the old nonce, replay
    // the effect, and pop open a row the user had deliberately collapsed.
    var consumedNonce by rememberSaveable(lift.exerciseId) { mutableStateOf(if (isFocused) focusNonce else -1) }
    LaunchedEffect(focusNonce) {
        if (isFocused && expandable && focusNonce > consumedNonce) {
            consumedNonce = focusNonce
            expanded = true
        }
    }
    val current = toDisplayWeight(lift.currentE1rm, weightUnit).roundToInt()
    val unit = unitLabel(weightUnit)

    Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(vertical = 2.dp)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                // Open rows get an accent wash so it's obvious which one is expanded.
                .background(if (expanded) c.accent.copy(alpha = 0.10f) else androidx.compose.ui.graphics.Color.Transparent)
                .then(
                    if (expandable) Modifier.clickable(
                        onClickLabel = if (expanded) "Collapse ${lift.exerciseName}" else "Expand ${lift.exerciseName}",
                        role = Role.Button
                    ) { expanded = !expanded } else Modifier
                )
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    lift.exerciseName, style = MaterialTheme.typography.bodyMedium,
                    color = if (expanded) c.accent else c.onBg,
                    fontWeight = if (expanded) FontWeight.SemiBold else FontWeight.Normal,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f)
                )
                Text(
                    "$current $unit",
                    style = MaterialTheme.typography.labelMedium,
                    color = if (expanded) c.accent else c.muted
                )
                if (expandable) {
                    Text(if (expanded) "▾" else "▸", style = MaterialTheme.typography.labelMedium, color = c.accent)
                } else {
                    Text("1 SESSION", style = MaterialTheme.typography.labelSmall, color = c.muted.copy(alpha = 0.7f), fontSize = 8.sp)
                }
            }
            Box(
                modifier = Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(50))
                    .background(c.outline.copy(alpha = 0.18f))
            ) {
                Box(
                    modifier = Modifier.fillMaxWidth((frac * barProgress).coerceIn(0f, 1f)).fillMaxHeight()
                        .clip(RoundedCornerShape(50)).background(c.accent)
                )
            }
        }
        AnimatedVisibility(
            visible = expanded && expandable,
            enter = expandVertically() + fadeIn(),
            exit = shrinkVertically() + fadeOut()
        ) {
            Box(Modifier.padding(horizontal = 8.dp)) {
                LiftDetailBody(lift, display, prsForLift, curve, weightUnit, c)
            }
        }
    }
}

/** The expanded lift read: e1RM trend, the lift's recent PRs, and its load-rep curve. */
@Composable
private fun LiftDetailBody(
    lift: E1rmLift,
    display: List<Double>,
    prsForLift: List<PrEntry>,
    curve: StrengthCurve?,
    weightUnit: WeightUnit,
    c: StatsColors
) {
    val unit = unitLabel(weightUnit)
    val lo = remember(display) { display.minOrNull() ?: 0.0 }
    val hi = remember(display) { display.maxOrNull() ?: 1.0 }
    val fmt = remember { SimpleDateFormat("MMM d", Locale.getDefault()) }
    Column(Modifier.fillMaxWidth().padding(bottom = 6.dp)) {
        // ── e1RM trend ────────────────────────────────────────────────────
        LineChart(
            values = display,
            lineColor = c.accent,
            trendColor = c.muted,
            minValue = lo,
            maxValue = hi,
            modifier = Modifier.fillMaxWidth().height(STATS_CHART_H)
        )
        Spacer(Modifier.height(6.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("${lo.roundToInt()} $unit", style = MaterialTheme.typography.labelSmall, color = c.muted)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                lift.monthlyPct?.let { pct ->
                    Text(
                        "%+.1f%%/mo".format(pct),
                        style = MaterialTheme.typography.labelSmall,
                        color = if (pct >= 0) c.accent else c.muted
                    )
                }
                if (lift.stalling) Text("stalling", style = MaterialTheme.typography.labelSmall, color = c.muted)
            }
            Text("${hi.roundToInt()} $unit", style = MaterialTheme.typography.labelSmall, color = c.muted)
        }

        // ── This lift's recent PRs ────────────────────────────────────────
        val recentPrs = remember(prsForLift) { prsForLift.sortedByDescending { it.date }.take(3) }
        if (recentPrs.isNotEmpty()) {
            Spacer(Modifier.height(12.dp))
            Text("RECENT PRS", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Spacer(Modifier.height(4.dp))
            recentPrs.forEach { pr ->
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        "${toDisplayWeight(pr.weightLb, weightUnit).roundToInt()} $unit × ${pr.reps}",
                        style = MaterialTheme.typography.bodySmall, color = c.onBg
                    )
                    Text(fmt.format(Date(pr.date)), style = MaterialTheme.typography.labelSmall, color = c.muted)
                }
            }
        }

        // ── Load-rep strength curve (needs enough plotted sets to mean anything) ──
        if (curve != null && curve.points.size >= MIN_POINTS_FOR_LIFT_CURVE) {
            Spacer(Modifier.height(12.dp))
            Text("STRENGTH CURVE", style = MaterialTheme.typography.labelSmall, color = c.muted, fontSize = 9.sp, letterSpacing = 1.sp)
            Spacer(Modifier.height(6.dp))
            LiftCurveChart(curve, weightUnit, c)
        }
    }
}

/** Every working set as weight × reps with the fitted Epley curve — scoped to one lift. */
@Composable
private fun LiftCurveChart(curve: StrengthCurve, weightUnit: WeightUnit, c: StatsColors) {
    val unit = unitLabel(weightUnit)
    val pts = remember(curve, weightUnit) {
        curve.points.map { Offset(it.reps.toFloat(), toDisplayWeight(it.weightLb, weightUnit).toFloat()) }
    }
    val e1 = toDisplayWeight(curve.e1rmLb, weightUnit).toFloat()
    val maxReps = curve.points.maxOf { it.reps }.coerceAtLeast(2)
    // Fitted curve from the e1RM via the shared Epley inverse — never diverges from E1rm.epley.
    val overlay = remember(curve, weightUnit) {
        (maxReps downTo 1).map { r -> Offset(r.toFloat(), E1rm.epleyInverse(e1.toDouble(), r).toFloat()) }
    }
    val minY = minOf(pts.minOf { it.y }, overlay.minOf { it.y })
    val maxY = maxOf(e1, pts.maxOf { it.y })
    ScatterChart(
        points = pts,
        overlay = overlay,
        pointColor = c.muted,
        lineColor = c.accent,
        gridColor = c.outline.copy(alpha = 0.12f),
        minX = 1f, maxX = maxReps.toFloat(),
        minY = minY * 0.95f, maxY = maxY * 1.05f,
        highlightOverlayEnd = true,
        modifier = Modifier.fillMaxWidth().height(STATS_CHART_H)
    )
    Spacer(Modifier.height(4.dp))
    Text(
        "every set · weight × reps · ● = projected 1-rep max (≈ ${e1.roundToInt()} $unit)",
        style = MaterialTheme.typography.labelSmall, color = c.muted
    )
}
