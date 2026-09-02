package com.forge.app.ui.goals

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
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
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.program.ExerciseLibrary
import com.forge.app.domain.units.WeightUnit
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
import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import java.time.temporal.TemporalAdjusters

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
internal fun customGoalValueLine(g: ExtendedGoalRepository.Progress, weightUnit: WeightUnit, useMiles: Boolean): String =
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

// ─── The clock (2026-08-23) ─────────────────────────────────────────────────

/**
 * Days remaining in a goal's window, today included, or null for an all-time goal that has none.
 *
 * A weekly target is the one goal shape where the same reading means opposite things on different
 * days: "3.2 of 5 km" is comfortable on Tuesday and already lost on Sunday night. The window's end
 * was stored all along (`GoalPeriod` is what the repository aggregates over) and no surface has ever
 * drawn it, so every period goal has been rendering half of its own truth.
 */
internal fun periodDaysLeft(period: GoalPeriod, nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): Int? {
    val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    // The aggregate windows run Monday-start (`mondayStartMs`) and calendar month, so the last day
    // they still count a log on is the Sunday, and the month's own last date.
    val end = when (period) {
        GoalPeriod.WEEK -> today.with(TemporalAdjusters.nextOrSame(DayOfWeek.SUNDAY))
        GoalPeriod.MONTH -> today.withDayOfMonth(today.lengthOfMonth())
        GoalPeriod.ALL -> return null
    }
    return ChronoUnit.DAYS.between(today, end).toInt() + 1
}

/** The clock as words. Terse, because it sits under a bar as a caption, not as a sentence. */
private fun daysLeftCaption(days: Int): String = when {
    days <= 1 -> "Last day"
    else -> "$days days left"
}

/**
 * The mono caption under a goal's meter — the one line that says what the bar is measuring.
 *
 * Exactly one thing at a time, in priority order, and null for the many goals that need none:
 *  - a reached goal names itself, because the accent fill is otherwise the only channel saying so
 *    and that fails a monochrome or colour-blind reader (§5, §14);
 *  - a bodyweight goal names its baseline, because its bar measures travel from that weigh-in and
 *    the "now → target" reading beside it does not mention it at all;
 *  - a weekly or monthly goal names its clock (above).
 *
 * This is §2①'s "explains a mark → mono caption" slot. The right-hand reading stays a pure number
 * pair, which is what keeps a state word out of the row meta (§2①, §8).
 */
internal fun goalCaption(
    achieved: Boolean,
    metric: GoalMetric?,
    period: GoalPeriod?,
    baselineValue: Double?,
    weightUnit: WeightUnit,
    nowMs: Long,
): String? = when {
    achieved -> "Reached"
    metric == GoalMetric.BODYWEIGHT && baselineValue != null ->
        "From ${weightInputValue(baselineValue, weightUnit)} ${unitLabel(weightUnit)}"
    period != null -> periodDaysLeft(period, nowMs)?.let(::daysLeftCaption)
    else -> null
}

// ─── The shared goal line ───────────────────────────────────────────────────

/**
 * One goal as an open progress line: the name, its reading, the meter, and one caption saying what
 * the meter measures. The SAME component renders Home's GOALS section, Cardio's and the Profile's
 * trims, and the Goals screen's rows, so the four read as one surface.
 *
 * ## The bar is accent-filled (2026-08-24)
 *
 * Reverted to the accent for EVERY goal at Antho's request for the 0.9 release material —
 * a neutral bar reads as an unfilled control in the launch video. A reached goal still
 * names itself in words, so the reached/in-progress split survives without colour.
 *
 * ## Previously: the bar was neutral (2026-08-16)
 *
 * It used to fill in the accent for EVERY goal. On Home that meant three warm bars stacked under a
 * warm CTA, and the accent stopped flagging anything — which is the failure mode Antho named on the
 * old design ("goals are so bad looking, the blue dot on the first one doesn't look good"). An
 * in-progress goal fills in `onBg`; only a REACHED one takes the accent, so colour marks the
 * exception rather than the majority.
 *
 * ## The row grew a clock, and gave up a state word (2026-08-23)
 *
 * Every row used to answer one question — how far along — three separate ways: a percentage as bar
 * length, a number pair, and, on a finished goal, the word REACHED sitting in the row meta. That
 * word is the one thing §2① rules out of a row's right-hand slot ("a count or reading, never a state
 * word"), and it was competing with the reading it replaced, so a finished goal stopped showing its
 * own numbers at exactly the moment they were worth seeing.
 *
 * The meta is now always the reading. What the row gained instead is [caption]: a mono line under
 * the bar carrying the fact the bar cannot draw — the days left in a weekly window, the weigh-in a
 * cut is measured from, or the word for a goal that is done. Sparse by design; most rows have none
 * and stay two lines tall.
 *
 * The title does not clamp. A goal name is user content, and a long lift name wraps rather than
 * truncating (§14).
 */
@Composable
internal fun GoalProgressLine(
    title: String,
    valueLine: String,
    fraction: Float,
    achieved: Boolean,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    modifier: Modifier = Modifier,
    /**
     * The mono line under the meter, from [goalCaption]. Null on a row whose bar needs no gloss.
     */
    caption: String? = null,
    /**
     * The leading mark. Non-null on Home, where RECENT's rows carry one and a goals section without
     * one made the page read as two unrelated halves (Antho, 2026-08-16: "the overall feel of the
     * page feels disconnected from the recent section, it's the only one with icons"), and on the
     * Goals screen since it stopped sorting by kind — in one mixed ladder a barbell beside a pair of
     * running shoes is the fastest read of what kind of goal a row is. Null where every visible row
     * is the same kind and a column of identical glyphs would say nothing.
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
                // §14: the meter is a drawn mark, so the row reads its VALUE, not its shape — and
                // the caption is content here, not decoration, so it is spoken too.
                contentDescription = listOfNotNull(
                    title,
                    valueLine,
                    "$percent percent of target",
                    caption
                ).joinToString(", ")
            }
    ) {
        if (icon != null) {
            CardMark(icon, onBg, size = 38.dp, glyphSize = 20.dp)
            Spacer(Modifier.width(14.dp))
        }
        // The bar starts at the TEXT column, not at the mark: that shared left rail is what makes a
        // goal row and a RECENT row read as the same kind of object.
        Column(Modifier.weight(1f)) {
            // §14 "figure rows wrap rather than clip". The reading is mono and cannot shrink, so a
            // plain Row hands it a fixed appetite and squeezes the title into whatever is left —
            // at 200% scale that broke names mid-word ("Worko / uts · this / month"). FlowRow lets
            // the reading drop to its own line instead, and SpaceBetween keeps it hard right on the
            // one line it shares with a title that fits.
            FlowRow(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                itemVerticalAlignment = Alignment.CenterVertically
            ) {
                Text(title, style = MaterialTheme.typography.titleMedium, color = onBg)
                Text(
                    valueLine,
                    // §6: two mono labels rank by SIZE. This is the reading at labelLarge; the
                    // caption under the meter is labelMedium.
                    style = MaterialTheme.typography.labelLarge,
                    color = muted,
                    // Chrome and mono labels may clamp (§14) — this is a derived reading, not the
                    // user's own text, and the title beside it is what wraps.
                    maxLines = 1, overflow = TextOverflow.Ellipsis
                )
            }
            Spacer(Modifier.height(12.dp))
            GoalMeter(frac, accent, outline)
            if (caption != null) {
                Spacer(Modifier.height(8.dp))
                Text(
                    caption.uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    // The muted floor, measured 4.63:1 on Pearl (§14). Never below it.
                    color = if (achieved) accent else muted.copy(alpha = 0.65f)
                )
            }
        }
    }
}

/**
 * The meter itself. Full width on every row without exception: bar LENGTH is the only quantity
 * being compared down a ladder of goals, and a bar whose track changed width per row would make
 * that comparison a lie. Anything that varies per row is text, and goes above or below it.
 *
 * At 10dp it is a bar rather than a hairline (Antho, 2026-08-23: the whole section read too small
 * and too cramped). That thickness is also what lets the accent mean something on a reached goal —
 * at 6dp the one place this screen spends colour was a thread.
 *
 * The track sits at the outline 0.25 rung — §5 reserves that rung for data lines, and a meter track
 * is one. (It was drawn at 0.35, the rung for borders on unselected controls, which quietly made
 * every empty goal look like an interactive element it is not.)
 */
@Composable
private fun GoalMeter(frac: Float, accent: Color, outline: Color) {
    Box(
        Modifier.fillMaxWidth().height(10.dp)
            .clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.25f))
    ) {
        Box(
            Modifier.fillMaxWidth(frac).fillMaxHeight()
                .clip(RoundedCornerShape(50))
                .background(accent)
        )
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
    onBg: Color, muted: Color, accent: Color, outline: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    GoalProgressLine(
        title = g.name,
        valueLine = "${weightInputValue(g.currentBestLb, weightUnit)} / ${weightInputValue(g.targetLb, weightUnit)} ${unitLabel(weightUnit)}",
        fraction = g.fraction,
        achieved = g.achieved,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        modifier = modifier,
        // A lift target has no window and no baseline: it is done or it is not.
        caption = if (g.achieved) "Reached" else null,
        icon = liftGoalGlyph(g.exerciseId),
        onClick = onClick
    )
}

@Composable
internal fun CustomGoalRow(
    g: ExtendedGoalRepository.Progress,
    onBg: Color, muted: Color, accent: Color, outline: Color,
    /** Local midnight of today, from state — see the memo below. */
    todayStartMs: Long,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val settings = LocalForgeSettings.current
    // The caption is a date-grained reading ("3 days left"), and memoising it on the GOAL alone let
    // it outlive the day it was computed for: a screen left open across midnight kept yesterday's
    // count, and a goal whose numbers happened not to change across a week boundary kept the
    // expired window's copy entirely (M-32). The day is part of the key, so the boundary that
    // changes the answer is the boundary that recomputes it.
    val caption = remember(g, settings.weightUnit, todayStartMs) {
        goalCaption(
            achieved = g.achieved,
            metric = g.metric,
            period = g.period,
            baselineValue = g.baselineValue,
            weightUnit = settings.weightUnit,
            nowMs = System.currentTimeMillis(),
        )
    }
    GoalProgressLine(
        title = customGoalTitle(g),
        valueLine = customGoalValueLine(g, settings.weightUnit, settings.useMiles),
        fraction = g.fraction,
        achieved = g.achieved,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        modifier = modifier,
        caption = caption,
        icon = goalGlyph(g.metric),
        onClick = onClick
    )
}
