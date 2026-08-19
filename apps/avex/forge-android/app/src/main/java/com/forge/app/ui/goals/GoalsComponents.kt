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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.program.ExerciseLibrary
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.experiment.CardMark
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.settings.SettingsIcons
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
 * One goal as an open progress line — title and figure on one baseline, a static bar underneath.
 * The SAME component renders Home's GOALS section and the Goals screen's rows, so the two read as
 * one surface.
 *
 * ## The bar is neutral now (2026-08-16)
 *
 * It used to fill in the accent for EVERY goal. On Home that meant three warm bars stacked under a
 * warm CTA, and the accent stopped flagging anything — which is the failure mode Antho named on the
 * old design ("goals are so bad looking, the blue dot on the first one doesn't look good"). An
 * in-progress goal now fills in `onBg`; only a REACHED one takes the accent, so colour marks the
 * exception rather than the majority.
 *
 * The reading also swaps to the word "REACHED" when achieved: the tint used to be the only channel
 * saying so, which fails for a monochrome or colour-blind reader.
 *
 * The title no longer clamps to one line. A goal name is user content, and a long lift name was
 * being truncated rather than wrapped.
 */
@Composable
internal fun GoalProgressLine(
    title: String,
    valueLine: String,
    fraction: Float,
    achieved: Boolean,
    @Suppress("UNUSED_PARAMETER") index: Int, // kept for call-site stability; the stagger it drove is gone
    onBg: Color, muted: Color, accent: Color, outline: Color,
    modifier: Modifier = Modifier,
    /**
     * The leading mark. Non-null on Home, where RECENT's rows carry one and a goals section without
     * one made the page read as two unrelated halves (Antho, 2026-08-16: "the overall feel of the
     * page feels disconnected from the recent section, it's the only one with icons"). Null on the
     * Goals screen itself, where every row is a goal and a column of identical glyphs says nothing.
     */
    icon: ImageVector? = null,
    onClick: () -> Unit
) {
    val frac = fraction.coerceIn(0f, 1f)
    val percent = (frac * 100).toInt()
    Row(
        modifier
            .fillMaxWidth()
            .bounceClick { onClick() }
            .semantics(mergeDescendants = true) {
                contentDescription = "$title, $percent percent of target"
            }
    ) {
        if (icon != null) {
            CardMark(icon, onBg)
            Spacer(Modifier.width(12.dp))
        }
        // The bar starts at the TEXT column, not at the mark: that shared left rail is what makes a
        // goal row and a RECENT row read as the same kind of object.
        Column(Modifier.weight(1f)) {
            Row(Modifier.fillMaxWidth()) {
                Text(
                    title, style = MaterialTheme.typography.bodyMedium, color = onBg,
                    modifier = Modifier.weight(1f).alignByBaseline()
                )
                Spacer(Modifier.width(12.dp))
                Text(
                    if (achieved) "REACHED" else valueLine,
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
                        .clip(RoundedCornerShape(50))
                        .background(if (achieved) accent else onBg)
                )
            }
        }
    }
}

/** A custom goal's glyph: its domain, since it has no implement. */
internal fun goalGlyph(metric: GoalMetric): ImageVector = when (metric) {
    GoalMetric.SESSIONS -> SettingsIcons.Session
    GoalMetric.VOLUME -> NavIcons.Stats
    GoalMetric.CARDIO_DISTANCE, GoalMetric.CARDIO_MINUTES -> NavIcons.Cardio
    GoalMetric.BODYWEIGHT -> ExerciseIcons.Bodyweight
}

/**
 * A lift target's glyph: the exercise's own equipment class, resolved through the library.
 *
 * Not a single shared barbell. Three pinned lifts would then stack three identical glyphs, which is
 * the "grey dot column" failure — a mark that is the same on every row carries no information and
 * becomes noise. Bench, squat and pull-ups now lead with a bench, a barbell and a bar. An exercise
 * the library does not know (a user's custom move) falls through to the pencil.
 */
internal fun liftGoalGlyph(exerciseId: String): ImageVector =
    ExerciseLibrary.byId(exerciseId)
        ?.let { ExerciseIcons.forEquipment(it.equipment) }
        ?: ExerciseIcons.Custom

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
