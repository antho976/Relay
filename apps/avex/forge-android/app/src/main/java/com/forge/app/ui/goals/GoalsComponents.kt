package com.forge.app.ui.goals

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings

// ─── Metric metadata / formatting (shared by rows and the editor flow) ──────

/** The metrics offered in the "add a goal" chooser, in display order. */
internal val customGoalMetrics = listOf(
    GoalMetric.SESSIONS,
    GoalMetric.VOLUME,
    GoalMetric.CARDIO_DISTANCE,
    GoalMetric.CARDIO_MINUTES,
    GoalMetric.BODYWEIGHT,
)

internal fun metricDisplayName(metric: GoalMetric): String = when (metric) {
    GoalMetric.CARDIO_DISTANCE -> "Cardio distance"
    GoalMetric.CARDIO_MINUTES -> "Cardio time"
    GoalMetric.SESSIONS -> "Workouts"
    GoalMetric.VOLUME -> "Training volume"
    GoalMetric.BODYWEIGHT -> "Bodyweight"
}

private fun periodText(period: GoalPeriod): String = when (period) {
    GoalPeriod.WEEK -> "this week"
    GoalPeriod.MONTH -> "this month"
    GoalPeriod.ALL -> "all-time"
}

/** The row title: the user's own name if set, else a generated one from the parameters. */
internal fun customGoalTitle(g: ExtendedGoalRepository.Progress): String {
    if (g.label.isNotBlank()) return g.label
    return if (g.metric == GoalMetric.BODYWEIGHT) "Bodyweight goal"
    else "${metricDisplayName(g.metric)} · ${periodText(g.period)}"
}

/** "current / target unit" (or "now → goal" for a bodyweight level). Reused by the Home goal lines. */
internal fun customGoalValueLine(g: ExtendedGoalRepository.Progress, weightUnit: com.forge.app.domain.units.WeightUnit, useMiles: Boolean): String =
    when (g.metric) {
        GoalMetric.CARDIO_DISTANCE ->
            "${distanceInputValue(g.currentValue, useMiles)} / ${distanceInputValue(g.targetValue, useMiles)} ${distanceUnitLabel(useMiles)}"
        GoalMetric.CARDIO_MINUTES ->
            "${g.currentValue.toInt()} / ${g.targetValue.toInt()} min"
        GoalMetric.SESSIONS ->
            "${g.currentValue.toInt()} / ${g.targetValue.toInt()} workouts"
        GoalMetric.VOLUME ->
            "${weightInputValue(g.currentValue, weightUnit)} / ${weightInputValue(g.targetValue, weightUnit)} ${unitLabel(weightUnit)}"
        GoalMetric.BODYWEIGHT ->
            "${weightInputValue(g.currentValue, weightUnit)} → ${weightInputValue(g.targetValue, weightUnit)} ${unitLabel(weightUnit)}"
    }

// ─── The shared goal line ───────────────────────────────────────────────────

/**
 * One goal as an open progress line — title and figure on one baseline, a static bar underneath
 * (the List archetype bans draw-in theatrics, §3; overview hosts animate at the section level).
 * The SAME component renders Home's GOALS teaser and the Goals screen's rows, so the preview and
 * the full list read as one surface. Achieved goals tint title + figure with the accent — the
 * tint alone flags the state (§8: state is never rendered twice).
 */
@Composable
internal fun GoalProgressLine(
    title: String,
    valueLine: String,
    fraction: Float,
    achieved: Boolean,
    @Suppress("UNUSED_PARAMETER") index: Int, // kept for call-site stability; the stagger it drove is gone
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val frac = fraction.coerceIn(0f, 1f)
    Column(Modifier.fillMaxWidth().bounceClick { onClick() }) {
        Row(Modifier.fillMaxWidth()) {
            Text(
                title, style = MaterialTheme.typography.bodyMedium,
                color = if (achieved) accent else onBg,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).alignByBaseline()
            )
            Spacer(Modifier.width(12.dp))
            Text(
                valueLine,
                style = MaterialTheme.typography.labelMedium,
                color = if (achieved) accent else muted,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.alignByBaseline()
            )
        }
        Spacer(Modifier.height(9.dp))
        Box(
            Modifier.fillMaxWidth().height(5.dp)
                .clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.35f))
        ) {
            Box(
                Modifier.fillMaxWidth(frac).fillMaxHeight()
                    .clip(RoundedCornerShape(50)).background(accent)
            )
        }
    }
}

// ─── Rows (Goals screen) ────────────────────────────────────────────────────

@Composable
internal fun LiftGoalRow(
    g: GoalRepository.GoalProgress,
    index: Int,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    GoalProgressLine(
        title = g.name,
        valueLine = "${weightInputValue(g.currentBestLb, weightUnit)} / ${weightInputValue(g.targetLb, weightUnit)} ${unitLabel(weightUnit)}",
        fraction = g.fraction,
        achieved = g.achieved,
        index = index,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        onClick = onClick
    )
}

@Composable
internal fun CustomGoalRow(
    g: ExtendedGoalRepository.Progress,
    index: Int,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val settings = LocalForgeSettings.current
    GoalProgressLine(
        title = customGoalTitle(g),
        valueLine = customGoalValueLine(g, settings.weightUnit, settings.useMiles),
        fraction = g.fraction,
        achieved = g.achieved,
        index = index,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        onClick = onClick
    )
}
