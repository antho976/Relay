package com.forge.app.ui.cardio

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.cardio.components.BarGeom
import com.forge.app.ui.cardio.components.VerticalBarRow
import com.forge.app.ui.cardio.state.CardioDayCell
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.theme.LocalForgeSettings
import java.util.Locale

/**
 * The cardio overview hero — a THIS WEEK eyebrow with a "week stats →" action, the week's open
 * serif figures (days · minutes · distance, streak once it exists), and the Mon–Sun bar row. All
 * honest zeros on a fresh week or a fresh install, so the empty screen is this same screen at
 * zero (§12). The figures and bars tap through to the swipeable week-stats overlay.
 */
@Composable
internal fun CardioHero(
    weekLabel: String,
    weekDays: Int,
    weekMinutes: Int,
    weekDistanceKm: Double,
    streakDays: Int,
    weekTargetMin: Int,
    useMiles: Boolean,
    days: List<CardioDayCell>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    onOpenDetail: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled(label = "Week stats", onClick = onOpenDetail)
            .padding(horizontal = 24.dp)
            .padding(top = 12.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "THIS WEEK · $weekLabel",
                style = MaterialTheme.typography.labelSmall,
                color = muted, fontSize = 9.sp, letterSpacing = 1.sp
            )
            Text(
                "week stats →",
                style = MaterialTheme.typography.labelMedium,
                color = accent
            )
        }
        Spacer(Modifier.height(14.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            EditorialFigure(
                value = "$weekDays",
                label = if (weekDays == 1) "day" else "days",
                onBg = onBg, muted = muted, accent = accent
            )
            EditorialFigure(
                value = "$weekMinutes",
                label = "minutes",
                onBg = onBg, muted = muted, accent = accent
            )
            EditorialFigure(
                value = String.format(Locale.US, "%.1f", toDisplayDistance(weekDistanceKm, useMiles)),
                label = distanceUnitLabel(useMiles),
                onBg = onBg, muted = muted, accent = accent
            )
            if (streakDays >= 2) {
                EditorialFigure(
                    value = "$streakDays",
                    label = "day streak",
                    onBg = onBg, muted = muted, accent = accent
                )
            }
        }
        Spacer(Modifier.height(18.dp))
        WeekBoxRow(days = days, todayDow = todayDow, onBg = onBg, muted = muted, outline = outline, accent = accent)
        if (weekTargetMin > 0) {
            Spacer(Modifier.height(16.dp))
            GoalMeter(
                weekMinutes = weekMinutes, weekTargetMin = weekTargetMin,
                muted = muted, outline = outline, accent = accent
            )
        }
    }
}

/** The weekly-goal progress meter — a thin accent bar filling toward the minutes target. */
@Composable
private fun GoalMeter(weekMinutes: Int, weekTargetMin: Int, muted: Color, outline: Color, accent: Color) {
    val frac = (weekMinutes.toFloat() / weekTargetMin).coerceIn(0f, 1f)
    Column(Modifier.fillMaxWidth()) {
        Box(
            Modifier.fillMaxWidth().height(4.dp)
                .clip(RoundedCornerShape(4.dp))
                .background(outline.copy(alpha = 0.25f))
        ) {
            if (frac > 0f) {
                Box(
                    Modifier.fillMaxWidth(frac).height(4.dp)
                        .clip(RoundedCornerShape(4.dp)).background(accent)
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            if (weekMinutes >= weekTargetMin) "GOAL HIT · $weekTargetMin MIN" else "GOAL $weekTargetMin MIN",
            style = MaterialTheme.typography.labelSmall,
            color = muted, fontSize = 9.sp, letterSpacing = 1.sp
        )
    }
}

/**
 * GOALS — the cardio-metric custom goals (distance / minutes) as the same open progress lines Home
 * draws ([GoalProgressLine] is the shared component), in-progress-closest first, capped at three.
 * The header action and every line open the full Goals screen.
 */
@Composable
internal fun CardioGoalsSection(
    goals: List<ExtendedGoalRepository.Progress>,
    onOpenGoals: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val settings = LocalForgeSettings.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        EditorialHeader(
            label = "Goals",
            muted = muted,
            accent = accent,
            action = if (goals.size > 3) "all ${goals.size} →" else "view all →",
            onAction = onOpenGoals
        )
        Spacer(Modifier.height(12.dp))
        // In-progress goals lead (closest-first), then achieved — same ordering as Home's trim.
        val preview = goals
            .sortedWith(compareBy<ExtendedGoalRepository.Progress> { it.achieved }.thenByDescending { it.fraction })
            .take(3)
        Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
            preview.forEachIndexed { i, g ->
                GoalProgressLine(
                    title = customGoalTitle(g),
                    valueLine = customGoalValueLine(g, settings.useKg, settings.useMiles),
                    fraction = g.fraction,
                    achieved = g.achieved,
                    index = i,
                    onBg = onBg, muted = muted, accent = accent, outline = outline,
                    onClick = onOpenGoals
                )
            }
        }
    }
}

/** The Mon–Sun bar row — accent bars scale with each day's minutes; a rest day reads as a low
 *  muted stub, today-so-far as a dashed slot, untouched days as ghost track marks. */
@Composable
internal fun WeekBoxRow(
    days: List<CardioDayCell>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val dayLetters = listOf("M", "T", "W", "T", "F", "S", "S")
    val maxMin = (days.maxOfOrNull { it.minutes } ?: 0).coerceAtLeast(1)
    // (minutes, hasActivity, isRest) for a day. A rest day reads distinctly from an untrained one.
    fun cellAt(i: Int): Triple<Int, Boolean, Boolean> {
        val cell = days.getOrNull(i)
        val mins = cell?.minutes ?: 0
        val hasActivity = mins > 0
        return Triple(mins, hasActivity, !hasActivity && (cell?.isRest ?: false))
    }
    VerticalBarRow(
        count = 7,
        trackHeight = 48.dp,
        modifier = Modifier.fillMaxWidth(),
        bar = { i ->
            val (mins, hasActivity, isRest) = cellAt(i)
            val frac = (mins.toFloat() / maxMin).coerceIn(0f, 1f)
            when {
                hasActivity -> BarGeom(height = (8 + 40 * frac).dp, fill = accent)
                isRest -> BarGeom(height = 12.dp, fill = muted.copy(alpha = 0.65f))
                i == todayDow -> BarGeom(height = 4.dp, dashedOutline = muted.copy(alpha = 0.65f))
                else -> BarGeom(height = 4.dp, fill = outline.copy(alpha = 0.35f))
            }
        },
        top = { i ->
            val (mins, hasActivity, isRest) = cellAt(i)
            when {
                hasActivity -> Text("${mins}m", fontSize = 9.sp, color = onBg, fontWeight = FontWeight.SemiBold)
                isRest -> Text("rest", fontSize = 8.sp, color = muted)
            }
        },
        bottom = { i ->
            Text(
                dayLetters[i],
                fontSize = 9.sp,
                color = if (i == todayDow) onBg else muted,
                fontWeight = if (i == todayDow) FontWeight.Bold else FontWeight.Normal
            )
        }
    )
}
