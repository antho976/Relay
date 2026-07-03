package com.forge.app.ui.goals

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.profile.ProgressRing
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.LocalForgeSettings

// ─── Metric metadata / formatting (shared by rows and the add-flow dialogs) ──

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

/** "current / target unit" (or "now → goal" for a bodyweight level). Reused by the Profile goal box. */
internal fun customGoalValueLine(g: ExtendedGoalRepository.Progress, useKg: Boolean, useMiles: Boolean): String =
    when (g.metric) {
        GoalMetric.CARDIO_DISTANCE ->
            "${distanceInputValue(g.currentValue, useMiles)} / ${distanceInputValue(g.targetValue, useMiles)} ${distanceUnitLabel(useMiles)}"
        GoalMetric.CARDIO_MINUTES ->
            "${g.currentValue.toInt()} / ${g.targetValue.toInt()} min"
        GoalMetric.SESSIONS ->
            "${g.currentValue.toInt()} / ${g.targetValue.toInt()} workouts"
        GoalMetric.VOLUME ->
            "${weightInputValue(g.currentValue, useKg)} / ${weightInputValue(g.targetValue, useKg)} ${unitLabel(useKg)}"
        GoalMetric.BODYWEIGHT ->
            "${weightInputValue(g.currentValue, useKg)} → ${weightInputValue(g.targetValue, useKg)} ${unitLabel(useKg)}"
    }

// ─── Rows ───────────────────────────────────────────────────────────────────

@Composable
internal fun LiftGoalRow(
    g: GoalRepository.GoalProgress,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val useKg = LocalForgeSettings.current.useKg
    GoalRow(
        title = g.name,
        valueLine = "${weightInputValue(g.currentBestLb, useKg)} / ${weightInputValue(g.targetLb, useKg)} ${unitLabel(useKg)}",
        fraction = g.fraction,
        achieved = g.achieved,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        onClick = onClick
    )
}

@Composable
internal fun CustomGoalRow(
    g: ExtendedGoalRepository.Progress,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    val settings = LocalForgeSettings.current
    GoalRow(
        title = customGoalTitle(g),
        valueLine = customGoalValueLine(g, settings.useKg, settings.useMiles),
        fraction = g.fraction,
        achieved = g.achieved,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        onClick = onClick
    )
}

/**
 * One goal as a ring-led row — the same open progress-ring language as the Profile's GOALS teaser,
 * so the preview and the full screen read as one surface. Tap anywhere to edit.
 */
@Composable
private fun GoalRow(
    title: String,
    valueLine: String,
    fraction: Float,
    achieved: Boolean,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    onClick: () -> Unit
) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically
    ) {
        ProgressRing(
            fraction = fraction.coerceIn(0f, 1f),
            color = if (achieved) accent else onBg,
            trackColor = outline.copy(alpha = 0.25f),
            stroke = 4.dp,
            animated = true,
            modifier = Modifier.size(46.dp)
        ) {
            if (achieved) Text("✓", style = MaterialTheme.typography.titleSmall, color = accent)
            else Text(
                "${(fraction * 100).toInt()}%",
                style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 9.sp
            )
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = onBg, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(3.dp))
            Text(valueLine, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Spacer(Modifier.width(10.dp))
        Text(if (achieved) "reached" else "edit", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f))
    }
}
