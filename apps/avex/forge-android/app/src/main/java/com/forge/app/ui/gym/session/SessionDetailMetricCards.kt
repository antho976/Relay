package com.forge.app.ui.gym.session

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.common.staggeredProgress
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings

// ─── Open section anchor + style toggle ────────────────────────────────────────

/**
 * Open editorial section for a metric: small-caps mono label on the left, the bars/line style
 * toggle right-aligned, then content directly on the page — no card shell, no fill, no border,
 * no hairline (§1: air separates sections). The accent wash on tappable expand/collapse rows
 * inside [content] remains.
 */
@Composable
private fun MetricCardShell(
    title: String,
    style: SessionChartStyle,
    onStyle: (SessionChartStyle) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    content: @Composable () -> Unit
) {
    // One Column root: call sites wrap this in a Box (statsEntrance), where loose siblings would
    // stack on top of each other instead of flowing vertically.
    Column(Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                title.uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                letterSpacing = 1.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
            StyleToggle(style, onStyle, onBg, muted, accent, outline)
        }
        Spacer(Modifier.height(12.dp))
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            content()
        }
    }
}

/** The per-card bars/line switch — reuses the shared [SegmentRow]. */
@Composable
private fun StyleToggle(
    style: SessionChartStyle,
    onStyle: (SessionChartStyle) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    SegmentRow(
        items = SessionChartStyle.entries,
        isSelected = { it == style },
        label = { it.label },
        onSelect = onStyle,
        onBg = onBg, muted = muted, accent = accent, outline = outline
    )
}

// ─── Weight / Volume / Reps: tappable per-exercise comparison ───────────────────

/**
 * The per-exercise comparison for one metric (Weight/Volume/Reps), with its own bars/line toggle.
 * BARS draws a tappable row per exercise; LINE draws the session-shape sparkline first, then the same
 * tappable rows without bars. Tapping a row expands that exercise's full detail inline.
 */
@Composable
internal fun MetricExerciseCard(
    exercises: List<ExerciseDetail>,
    metric: SessionMetric,
    style: SessionChartStyle,
    onStyle: (SessionChartStyle) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val values = exercises.map { it.metricValue(metric) }
    val rawMax = values.maxOrNull() ?: 0.0
    // Hoisted above the card so the remember runs unconditionally regardless of the empty-state guard.
    // The slow draw curve fills the comparison bars in gently rather than snapping them to width.
    val progress = rememberDrawProgress(metric, ForgeMotion.drawTween())
    MetricCardShell("${metric.label.uppercase()} PER EXERCISE", style, onStyle, onBg, muted, accent, outline) {
        // The Weight metric is meaningless for a bodyweight-only session — say so instead of empty bars.
        if (rawMax <= 0.0) {
            Text(
                "No ${metric.label.lowercase()} logged for this session.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
            )
            return@MetricCardShell
        }
        // Bars always compare the exercises; the bars/line toggle now only restyles the per-set chart
        // inside an expanded row. The first row opens by default to hint that rows are tappable.
        exercises.forEachIndexed { i, ex ->
            ExerciseDrillRow(
                ex = ex,
                value = values[i],
                rawMax = rawMax,
                metric = metric,
                style = style,
                defaultExpanded = i == 0,
                barProgress = staggeredProgress(progress, i, exercises.size),
                onBg = onBg, muted = muted, accent = accent, outline = outline
            )
        }
    }
}

/** One exercise row in a metric card: name + value + comparison bar; taps open to the full detail. */
@Composable
private fun ExerciseDrillRow(
    ex: ExerciseDetail,
    value: Double,
    rawMax: Double,
    metric: SessionMetric,
    style: SessionChartStyle,
    defaultExpanded: Boolean,
    barProgress: Float,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Keyed by name only (NOT metric): Weight/Volume/Reps share one composable tree, so adding metric
    // to the key would reset the row on every metric switch and re-pop the first row open. Starts at
    // [defaultExpanded]; rememberSaveable keeps it closed once collapsed, reopening only on a fresh visit.
    var expanded by rememberSaveable(ex.name) { mutableStateOf(defaultExpanded) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                // Open rows get an accent wash so it's obvious which one is expanded (§5 primaryContainer rung).
                .background(if (expanded) accent.copy(alpha = 0.15f) else Color.Transparent)
                .clickable(
                    onClickLabel = if (expanded) "Collapse ${ex.name}" else "Expand ${ex.name}",
                    role = Role.Button
                ) { expanded = !expanded }
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    ex.name, style = MaterialTheme.typography.bodyMedium,
                    color = if (expanded) accent else onBg,
                    fontWeight = if (expanded) FontWeight.SemiBold else FontWeight.Normal,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f)
                )
                Text(
                    formatMetricValue(value, metric, weightUnit),
                    style = MaterialTheme.typography.labelMedium,
                    color = if (expanded) accent else muted
                )
                Text(if (expanded) "▾" else "▸", style = MaterialTheme.typography.labelMedium, color = accent)
            }
            val frac = (value / rawMax).toFloat() * barProgress
            Box(
                modifier = Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(50))
                    .background(outline.copy(alpha = 0.25f))
            ) {
                Box(
                    modifier = Modifier.fillMaxWidth(frac.coerceIn(0f, 1f)).fillMaxHeight()
                        .clip(RoundedCornerShape(50)).background(accent)
                )
            }
        }
        if (expanded) {
            Box(Modifier.padding(horizontal = 8.dp)) {
                ExerciseDetailBody(ex, metric, style, onBg, muted, accent, outline)
            }
        }
    }
}

// ─── RPE: one merged chart with an exercise selector ────────────────────────────

/**
 * RPE for the whole session, merged into one card: pick an exercise from the dropdown and see its
 * per-set RPE (in this card's bars/line style). Only exercises that actually logged an RPE appear.
 */
@Composable
internal fun RpeExerciseCard(
    exercises: List<ExerciseDetail>,
    style: SessionChartStyle,
    onStyle: (SessionChartStyle) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    // Hoisted above the card and memoized so the filter/map don't re-run each recomposition and the
    // selection state is created unconditionally (independent of the empty-state guard).
    val withRpe = remember(exercises) { exercises.filter { it.avgRpe > 0.0 } }
    val names = remember(withRpe) { withRpe.map { it.name } }
    // Track the selection by name so it survives a reorder; fall back to the first if it vanishes.
    var selectedName by rememberSaveable { mutableStateOf(withRpe.firstOrNull()?.name.orEmpty()) }
    MetricCardShell("RPE", style, onStyle, onBg, muted, accent, outline) {
        if (withRpe.isEmpty()) {
            Text(
                "No RPE logged for this session.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
            )
            return@MetricCardShell
        }
        val idx = withRpe.indexOfFirst { it.name == selectedName }.let { if (it >= 0) it else 0 }
        val ex = withRpe[idx]
        ExercisePicker(
            items = names,
            selectedIndex = idx,
            onSelect = { selectedName = withRpe[it].name },
            onBg = onBg, muted = muted, outline = outline
        )
        val rated = ex.sets.count { it.rpe != null }
        Text(
            "Avg RPE ${rpeLabel(ex.avgRpe)} · $rated ${if (rated == 1) "set" else "sets"} rated",
            style = MaterialTheme.typography.labelMedium, color = muted
        )
        PerExerciseSetChart(ex, SessionMetric.RPE, style, accent, muted, outline)
    }
}

/** A compact dropdown of exercise names (matches the cardio activity picker's look). */
@Composable
private fun ExercisePicker(
    items: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
                .clickable(onClickLabel = "Choose exercise") { open = true }
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                items.getOrElse(selectedIndex) { items.first() },
                style = MaterialTheme.typography.bodyMedium, color = onBg,
                maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f)
            )
            Text("▾", style = MaterialTheme.typography.labelMedium, color = muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            items.forEachIndexed { i, name ->
                DropdownMenuItem(
                    text = { Text(name, color = onBg) },
                    onClick = { onSelect(i); open = false }
                )
            }
        }
    }
}
