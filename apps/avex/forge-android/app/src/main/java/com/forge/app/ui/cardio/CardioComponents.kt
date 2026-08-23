package com.forge.app.ui.cardio

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioActivityRecord
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.toDisplayDistance
import com.forge.app.ui.cardio.components.BarGeom
import com.forge.app.ui.cardio.components.MeterBar
import com.forge.app.ui.cardio.components.RankedBarRow
import com.forge.app.ui.cardio.components.VerticalBarRow
import com.forge.app.ui.cardio.state.CardioDayCell
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The cardio hero — a THIS WEEK eyebrow with a `weeks →` action into the ledger, the week's open
 * serif figures (days · minutes · distance, streak once it exists), the Mon–Sun bar row and the
 * minutes meter. All honest zeros on a fresh week or a fresh install, so the empty screen is this
 * same screen at zero (§12).
 *
 * The hero itself is passive (2026-08-23): it used to be ONE page-wide tap target opening a
 * full-screen week pager, which put the page's richest content behind a gesture nothing announced.
 * Two named ways in replaced it — `weeks →` to the weeks chart, and the Mon–Sun strip, which opens
 * THIS week in full (Antho, 2026-08-23). The strip is a single tap target: seven day-wide ones would
 * each be under the 48dp minimum and would all lead to the same place anyway (§2③).
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
    onOpenWeeks: () -> Unit,
    /** Tapping the Mon–Sun strip opens this week's own page. */
    onOpenThisWeek: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
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
                color = muted, letterSpacing = 1.sp
            )
            // §2③ — navigation is the mono accent `action →`, with its own touch target.
            Text(
                "weeks →",
                style = MaterialTheme.typography.labelMedium,
                color = accent,
                modifier = Modifier
                    .clickableLabeled("Browse earlier weeks", onClick = onOpenWeeks)
                    .padding(horizontal = 4.dp, vertical = 8.dp)
            )
        }
        Spacer(Modifier.height(10.dp))
        // Figures wrap rather than clip at large font scales (§14) — four of them never fit one line
        // at 200%, and a clipped streak is worse than a wrapped one.
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            EditorialFigure(
                value = "$weekDays",
                label = if (weekDays == 1) "day" else "days",
                onBg = onBg, muted = muted, accent = accent,
                modifier = Modifier.weight(1f)
            )
            EditorialFigure(
                value = "$weekMinutes",
                label = "minutes",
                onBg = onBg, muted = muted, accent = accent,
                modifier = Modifier.weight(1f)
            )
            EditorialFigure(
                value = String.format(Locale.US, "%.1f", toDisplayDistance(weekDistanceKm, useMiles)),
                label = distanceUnitLabel(useMiles),
                onBg = onBg, muted = muted, accent = accent,
                modifier = Modifier.weight(1f)
            )
            if (streakDays >= 2) {
                EditorialFigure(
                    value = "$streakDays",
                    label = "day streak",
                    onBg = onBg, muted = muted, accent = accent,
                    modifier = Modifier.weight(1f)
                )
            } else {
                Spacer(Modifier.weight(1f))
            }
        }
        Spacer(Modifier.height(18.dp))
        WeekBoxRow(
            days = days,
            todayDow = todayDow,
            onBg = onBg, muted = muted, outline = outline, accent = accent,
            onClick = onOpenThisWeek
        )
        Spacer(Modifier.height(16.dp))
        // A personal minutes target fills its goal meter; without one, the WHO 150-min/week reference
        // takes its place (GYMAP-42) so the week always reads against a baseline, never empty space.
        val hasGoal = weekTargetMin > 0
        val target = if (hasGoal) weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
        MeterBar(
            fraction = weekMinutes.toFloat() / target,
            caption = when {
                hasGoal && weekMinutes >= target -> "Goal hit · $target min"
                hasGoal -> "Goal $target min"
                weekMinutes >= target -> "WHO 150 min · met"
                else -> "WHO 150 min"
            },
            muted = muted, outline = outline, accent = accent,
            contentDescription = "$weekMinutes of $target minutes this week"
        )
    }
}

/**
 * BY ACTIVITY — this week's minutes split by activity, ranked (§2②: ranked comparison = thin bars).
 * The mark carries the split; the type names are its labels, not its content. Hidden at zero, where
 * the hero's own all-zero bars already say the week is empty (§12 — no second empty shell).
 */
@Composable
internal fun CardioByActivitySection(
    minutesByType: List<Pair<CardioType, Int>>,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    if (minutesByType.isEmpty()) return
    val leader = minutesByType.first().second.coerceAtLeast(1)
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        EditorialHeader(label = "By activity", muted = muted, accent = accent)
        Spacer(Modifier.height(12.dp))
        // >4 uniform rows is the checklist look (§4.10) — the tail collapses into one honest line.
        val shown = minutesByType.take(4)
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            shown.forEach { (type, min) ->
                RankedBarRow(
                    label = type.displayName,
                    value = "$min min",
                    fraction = min.toFloat() / leader,
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
            }
        }
        val restMinutes = minutesByType.drop(4).sumOf { it.second }
        if (restMinutes > 0) {
            Spacer(Modifier.height(10.dp))
            Text(
                "${minutesByType.size - 4} more · $restMinutes min".uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted, letterSpacing = 1.sp
            )
        }
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
        val preview = remember(goals) {
            goals
                .sortedWith(compareBy<ExtendedGoalRepository.Progress> { it.achieved }.thenByDescending { it.fraction })
                .take(3)
        }
        Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
            preview.forEachIndexed { i, g ->
                GoalProgressLine(
                    title = customGoalTitle(g),
                    valueLine = customGoalValueLine(g, settings.weightUnit, settings.useMiles),
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

/**
 * RECORDS — per-activity all-time bests (GYMAP-34), drawn as the same ranked bars BY ACTIVITY uses
 * so the PROGRESS lens reads as one visual language (§4.10). Each row's bar is that activity's
 * longest distance against the longest you have run in any activity; the fastest pace rides the row
 * as its reading. A row opens the record-setting session.
 *
 * The distance used to be accent-coloured body text, which fails AA on four of the five accents
 * (§14). It is onBg text now, and the accent is spent on the bar — where a colour can carry meaning
 * without being read.
 */
@Composable
internal fun CardioRecordsSection(
    records: List<CardioActivityRecord>,
    useMiles: Boolean,
    onOpenSession: (Long) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    if (records.isEmpty()) return
    val customs = LocalCardioTypes.current
    val fmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.getDefault()) }
    val leader = remember(records) {
        records.maxOfOrNull { it.longestEntry.distanceKm ?: 0.0 }?.coerceAtLeast(0.1) ?: 0.1
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        EditorialHeader(label = "Records", muted = muted, accent = accent)
        Spacer(Modifier.height(12.dp))
        Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            records.take(4).forEach { r ->
                val name = CardioActivity.resolve(r.typeCode, customs).displayName
                val longestKm = r.longestEntry.distanceKm ?: 0.0
                val pace = pacePerUnit(r.fastestEntry.durationMin, r.fastestEntry.distanceKm, useMiles)
                Column(
                    // The WHOLE row is the tap target (§2③) — never a nested one.
                    Modifier
                        .fillMaxWidth()
                        .clickableLabeled("Show your longest $name") { onOpenSession(r.longestEntry.id) }
                        .padding(vertical = 2.dp)
                ) {
                    RankedBarRow(
                        label = name,
                        value = formatDistance(longestKm, useMiles),
                        fraction = (longestKm / leader).toFloat(),
                        onBg = onBg, muted = muted, outline = outline, accent = accent
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        buildString {
                            append(fmt.format(Date(r.longestEntry.date)).uppercase())
                            if (pace != null) append(" · BEST ${pace.uppercase()} /${distanceUnitLabel(useMiles).uppercase()}")
                        },
                        style = MaterialTheme.typography.labelSmall,
                        color = muted, letterSpacing = 0.5.sp
                    )
                }
            }
        }
    }
}

/**
 * The Mon–Sun bar row — accent bars scale with each day's minutes; a rest day reads as a low muted
 * stub, today-so-far as a dashed slot, untouched days as ghost track marks. The whole strip is one
 * tap target ([onClick]) opening this week's page; passing null leaves it passive.
 */
@Composable
internal fun WeekBoxRow(
    days: List<CardioDayCell>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color,
    onClick: (() -> Unit)? = null
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
    val reading = remember(days) {
        val total = days.sumOf { it.minutes }
        val active = days.count { it.minutes > 0 }
        "This week, $active of 7 days trained, $total minutes"
    }
    VerticalBarRow(
        count = 7,
        trackHeight = 48.dp,
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (onClick != null) Modifier.clickableLabeled("Open this week", onClick = onClick)
                else Modifier
            )
            // The strip's own value, so TalkBack reads the week rather than "button" (§14).
            .semantics(mergeDescendants = true) { contentDescription = reading }
            // Padding, not bar height, carries the ≥48dp touch target (§14) — the bars are 48dp of
            // track but a ghost week draws only 4dp of it.
            .padding(vertical = 4.dp),
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
                hasActivity -> Text("${mins}m", style = MaterialTheme.typography.labelSmall, color = onBg, fontWeight = FontWeight.SemiBold)
                isRest -> Text("rest", style = MaterialTheme.typography.labelSmall, color = muted)
            }
        },
        bottom = { i ->
            Text(
                dayLetters[i],
                style = MaterialTheme.typography.labelSmall,
                color = if (i == todayDow) onBg else muted,
                fontWeight = if (i == todayDow) FontWeight.Bold else FontWeight.Normal
            )
        }
    )
}
