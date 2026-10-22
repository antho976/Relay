package com.forge.app.ui.cardio

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.WHO_WEEKLY_ACTIVITY_MIN
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenPrimaryButton
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.CardioOpenColors
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle as JavaTextStyle
import java.time.temporal.ChronoUnit

/**
 * The hub's hero: THIS WEEK · dates with Weeks ›, the week's three readings (minutes of the mark,
 * distance against last week, days with the week streak), the seven day bars, the meter toward the
 * weekly mark, and the red Log cardio bar. All honest zeros on a fresh week.
 */
@Composable
internal fun CardioHero(
    state: CardioUiState,
    weekStart: LocalDate,
    todayDow: Int,
    onOpenWeeks: () -> Unit,
    onOpenThisWeek: () -> Unit,
    onOpenLog: () -> Unit,
) {
    val locale = currentLocale()
    val personal = state.weekTargetMin > 0
    val target = if (personal) state.weekTargetMin else WHO_WEEKLY_ACTIVITY_MIN
    val unit = distanceUnitLabel(state.useMiles)
    val range = remember(weekStart, locale) { weekRangeLabel(weekStart, weekStart.plusDays(6), locale) }
    if (state.entries.isEmpty() && !state.isLoading) {
        CardioFirstWeek(range, target, personal, todayDow, onOpenWeeks, onOpenLog)
        return
    }
    Column {
        CardioLinkHead(
            label = "This week · $range",
            action = "Weeks",
            actionLabel = "Browse your weeks",
            onAction = onOpenWeeks
        )
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
            CardioReading(
                label = "Minutes",
                figure = cardioFigure("${state.weekMinutes}", "", CardioOpenType.figureMUnit),
                change = "Of $target",
                moved = false,
                description = "${state.weekMinutes} of $target minutes this week",
                modifier = Modifier.weight(1f)
            )
            val (change, moved) = distanceVsLast(state.weekDistanceKm, state.lastWeekToDateKm, state.useMiles)
            val dist = distanceNumber(state.weekDistanceKm, state.useMiles)
            CardioReading(
                label = "Distance",
                figure = cardioFigure(dist, " $unit", CardioOpenType.figureMUnit),
                change = change,
                moved = moved,
                description = "$dist $unit this week, $change",
                modifier = Modifier.weight(1f)
            )
            val streak = state.cardioStreakWeeks
            CardioReading(
                label = "Days",
                figure = cardioFigure("${state.cardioDaysThisWeek}", "", CardioOpenType.figureMUnit),
                change = "Streak $streak wk",
                moved = false,
                description = "${state.cardioDaysThisWeek} days this week, a $streak week streak",
                modifier = Modifier.weight(1f)
            )
        }
        Spacer(Modifier.height(20.dp))
        CardioDayBars(state, todayDow, onOpenThisWeek)
        Spacer(Modifier.height(14.dp))
        OpenMeter(state.weekMinutes.toFloat() / target)
        Spacer(Modifier.height(8.dp))
        CardioChange(weekMeterCaption(state.weekMinutes, target, personal), moved = false)
        Spacer(Modifier.height(28.dp))
        CardioLogBar(watchCount = state.importSuggestions.size, onClick = onOpenLog)
    }
}

/**
 * The hero before anything is logged. Three zeros and a bare meter read as a dead page (Antho,
 * 2026-10-21), so a new account sees the week it is about to fill instead: what the mark asks of a
 * day, seven empty day columns with today ringed in red, and the first log as the action.
 */
@Composable
private fun CardioFirstWeek(
    range: String,
    target: Int,
    personal: Boolean,
    todayDow: Int,
    onOpenWeeks: () -> Unit,
    onOpenLog: () -> Unit,
) {
    val perDay = (target + 6) / 7
    Column {
        CardioLinkHead(label = "This week · $range", action = "Weeks", actionLabel = "Browse your weeks", onAction = onOpenWeeks)
        Spacer(Modifier.height(14.dp))
        Text("Your first week", style = OpenType.figureM, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(8.dp))
        CardioBodyLine(
            if (personal) "Your target is $target minutes. About $perDay a day gets you there."
            else "$target minutes a week is the WHO mark. About $perDay a day gets you there."
        )
        Spacer(Modifier.height(20.dp))
        CardioEmptyWeek(todayDow)
        Spacer(Modifier.height(28.dp))
        OpenPrimaryButton("Log your first session", onClick = onOpenLog, icon = OpenIcons.Plus)
    }
}

/** Seven empty day columns waiting to fill, today's outlined in red, each with its letter. */
@Composable
private fun CardioEmptyWeek(todayDow: Int) {
    val accent = MaterialTheme.colorScheme.primary
    val letters = listOf("M", "T", "W", "T", "F", "S", "S")
    Row(
        Modifier.fillMaxWidth().semantics(mergeDescendants = true) { contentDescription = "This week, nothing logged yet" },
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        letters.forEachIndexed { i, letter ->
            val today = i == todayDow
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(56.dp)
                        .clip(CardioCorner)
                        .background(if (today) MaterialTheme.colorScheme.primaryContainer else OpenColors.Track.copy(alpha = 0.35f))
                        .then(if (today) Modifier.border(1.dp, accent, CardioCorner) else Modifier)
                )
                OpenLabel(letter, style = OpenType.monoXS, color = if (today) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** The week's seven day bars: red for a day trained, a low quiet stub for a logged rest day. */
@Composable
private fun CardioDayBars(state: CardioUiState, todayDow: Int, onOpenThisWeek: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val days = state.weekDays.ifEmpty { List(7) { com.forge.app.ui.cardio.state.CardioDayCell() } }
    val letters = listOf("M", "T", "W", "T", "F", "S", "S")
    val reading = remember(days) {
        "This week, ${days.count { it.minutes > 0 }} of 7 days trained, ${days.sumOf { it.minutes }} minutes. Opens this week."
    }
    // Only trained days draw: an untrained day, today's included, is its letter over bare ground.
    Box(Modifier.fillMaxWidth().clickableLabeled("Open this week", onClick = onOpenThisWeek)) {
        OpenColumnChart(
            values = days.map { it.minutes.toFloat() },
            colors = days.map { accent },
            // An untrained week is its letters over the meter: no tall empty plot above them.
            height = if (days.any { it.minutes > 0 }) 96.dp else 28.dp,
            columnLabels = letters,
            gapFraction = 0.5f,
            description = reading
        )
    }
}

/**
 * The red Log cardio bar. When the watch picked up sessions with no entry yet it carries a small
 * "N from watch" chip on its right (the sheet then opens with them on top); otherwise a +.
 */
@Composable
internal fun CardioLogBar(watchCount: Int, onClick: () -> Unit) {
    if (watchCount <= 0) {
        OpenPrimaryButton("Log cardio", onClick = onClick, icon = OpenIcons.Plus)
        return
    }
    val ink = openOnAccentLarge()
    val label = "Log cardio, $watchCount from your watch"
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label }
            .clip(CardioCorner)
            .background(MaterialTheme.colorScheme.primary)
            .padding(start = 20.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("Log cardio", style = OpenType.button, color = ink, modifier = Modifier.weight(1f).padding(vertical = 14.dp))
        Row(
            Modifier
                .heightIn(min = 32.dp)
                .clip(CardioCorner)
                .background(CardioOpenColors.ChipWash)
                .padding(horizontal = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(OpenIcons.Navigation, contentDescription = null, tint = ink, modifier = Modifier.size(14.dp))
            Text("$watchCount from watch", style = CardioOpenType.chip, color = ink)
        }
    }
}

/**
 * WEEK — today on the watch (steps by hour against the usual by now), then this week's sessions.
 * Without a watch the steps slot is the invite to connect one, until it is hidden.
 */
internal fun LazyListScope.cardioWeekTab(
    state: CardioUiState,
    today: LocalDate,
    weekStartMs: Long,
    zone: ZoneId,
    actions: CardioHubActions,
    showWatchInvite: Boolean,
) {
    val hasSteps = state.stepsConnected || state.todayWearable?.hasData == true
    if (hasSteps || showWatchInvite) {
        item("watch") {
            Column {
                Spacer(Modifier.height(10.dp))
                OpenLabel("Today on your watch")
                Spacer(Modifier.height(18.dp))
                if (hasSteps) CardioStepsToday(state) else CardioWatchInvite(actions.onConnectWatch, actions.onDismissInvite)
                Spacer(Modifier.height(CARDIO_SECTION))
            }
        }
    } else {
        item("air") { Spacer(Modifier.height(10.dp)) }
    }

    val weekEntries = state.entries.filter { it.date >= weekStartMs }
    item("sessions-head") {
        Column {
            CardioLinkHead(
                label = "Sessions",
                action = if (actions.historyExpands && state.historyExpanded) "Show less" else "View all",
                actionLabel = "All sessions",
                onAction = actions.onOpenHistory
            )
            Spacer(Modifier.height(4.dp))
        }
    }
    if (weekEntries.isEmpty() && !state.isLoading) {
        item("sessions-empty") {
            val last = state.entries.firstOrNull { it.type != CardioType.REST.code }
            Column {
                Spacer(Modifier.height(14.dp))
                if (last == null) CardioFirstIdeas(actions.onOpenLog)
                else CardioBodyLine("0 sessions this week.")
                if (last != null) {
                    Spacer(Modifier.height(20.dp))
                    OpenLabel("Last time", style = OpenType.monoXS)
                    Spacer(Modifier.height(8.dp))
                    CardioSessionRow(last, today, zone, state.useMiles, actions)
                }
            }
        }
    }
    items(weekEntries, key = { it.id }) { entry ->
        Column {
            Spacer(Modifier.height(14.dp))
            CardioSessionRow(entry, today, zone, state.useMiles, actions)
            Spacer(Modifier.height(10.dp))
        }
    }
    // The inline fallback (no History route): the older rows grow in place under this week's.
    if (actions.historyExpands && state.historyExpanded) {
        val older = state.entries.filter { it.date < weekStartMs }
        items(older, key = { "o${it.id}" }) { entry ->
            Column {
                Spacer(Modifier.height(14.dp))
                CardioSessionRow(entry, today, zone, state.useMiles, actions)
                Spacer(Modifier.height(10.dp))
            }
        }
    }
}

/**
 * One session: its date column, the activity over "30 min · 7.1 mi", and its pace (or a ride's
 * speed) in serif on the right. Tap opens it; a long press deletes it, with Undo.
 */
@Composable
internal fun CardioSessionRow(
    entry: CardioEntry,
    today: LocalDate,
    zone: ZoneId,
    useMiles: Boolean,
    actions: CardioHubActions,
) {
    val activity = CardioActivity.resolve(entry.type, LocalCardioTypes.current)
    val locale = currentLocale()
    val date = remember(entry.date, zone) { Instant.ofEpochMilli(entry.date).atZone(zone).toLocalDate() }
    // A row from an earlier week leads with its month, so "19" is never ambiguous.
    val sameWeek = ChronoUnit.DAYS.between(date, today) < 7 && !date.isAfter(today) &&
        date.dayOfWeek.value <= today.dayOfWeek.value
    val top = if (sameWeek) date.dayOfWeek.getDisplayName(JavaTextStyle.SHORT, locale)
    else date.month.getDisplayName(JavaTextStyle.SHORT, locale)
    val summary = sessionSummary(entry, activity.isRest, useMiles)
    val figure = if (activity.isRest) null else sessionFigure(entry, useMiles)
    Row(
        Modifier
            .fillMaxWidth()
            .bounceCombinedClick(
                onClickLabel = "Open ${activity.displayName}",
                onLongClickLabel = "Delete this session",
                onLongClick = { actions.onDeleteSession(entry.id) },
                role = Role.Button,
                onClick = { actions.onOpenSession(entry.id) }
            )
            .semantics(mergeDescendants = true) {},
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        CardioDateColumn(top, "${date.dayOfMonth}")
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(activity.displayName, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
            Text(summary, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (figure != null) CardioRowFigure(figure.first, figure.second)
    }
}

/**
 * Before the first session: three easy ways in, each a row that opens the log, so the list starts
 * with something to do rather than a sentence about nothing.
 */
@Composable
private fun CardioFirstIdeas(onOpenLog: () -> Unit) {
    val ideas = listOf(
        Triple(CardioType.WALK, "20 min", "The easiest way in"),
        Triple(CardioType.RUN, "25 min", "Easy pace, you can still talk"),
        Triple(CardioType.CYCLE, "30 min", "Outside or on a bike at the gym"),
    )
    Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
        ideas.forEach { (type, length, why) ->
            Row(
                Modifier.fillMaxWidth().clickableLabeled("Log a ${type.displayName.lowercase()}", onClick = onOpenLog),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    Modifier.size(36.dp).background(MaterialTheme.colorScheme.surfaceContainerHigh, CardioCorner),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(type.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text("${type.displayName} · $length", style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
                    Text(why, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Icon(OpenIcons.Plus, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
            }
        }
    }
}

/** Steps today by hour, the hour now in red, against the usual by this hour. */
@Composable
private fun CardioStepsToday(state: CardioUiState) {
    val locale = currentLocale()
    val use24h = LocalForgeSettings.current.timeFormat24h
    val accent = MaterialTheme.colorScheme.primary
    val wearable = state.todayWearable
    val total = wearable?.totalSteps ?: 0
    val nowHour = state.stepsHour.takeIf { it in 0..23 } ?: 23
    val first = 6
    val last = 21
    val byHour = remember(wearable) {
        val raw = IntArray(24)
        wearable?.hourlySteps?.forEach { if (it.hour in 0..23) raw[it.hour] += it.steps }
        (first..last).map { h ->
            when (h) {
                first -> (0..first).sumOf { raw[it] }
                last -> (last..23).sumOf { raw[it] }
                else -> raw[h]
            }
        }
    }
    val reading = buildString {
        append(formatCount(total, locale))
        state.usualStepsByNow?.let { append(" · usual ${formatCount(it, locale)} by now") }
    }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Steps", style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            OpenLabel(reading, style = OpenType.monoXS, color = MaterialTheme.colorScheme.onBackground)
        }
        OpenColumnChart(
            values = byHour.map { it.toFloat() },
            colors = (first..last).map { h -> if (h == nowHour.coerceIn(first, last)) accent else OpenColors.Bar },
            height = 64.dp,
            columnLabels = (first..last).map { h -> if ((h - first) % 3 == 0) hourTick(h, use24h) else "" },
            gapFraction = 0.35f,
            description = "Steps today by hour, $total so far" +
                (state.usualStepsByNow?.let { ", usually $it by now" } ?: "")
        )
        if (total == 0) CardioChange("Your watch has sent no steps today yet", moved = false)
    }
}

/** The steps slot without a watch: what connecting one gives, Connect, and a quiet way to hide it. */
@Composable
private fun CardioWatchInvite(onConnect: () -> Unit, onDismiss: () -> Unit) {
    Column {
        CardioBodyLine("Connect a watch or ring through Health Connect to see today's steps here and bring its workouts into Cardio.")
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenSmallButton("Connect", filled = true, onClick = onConnect)
            OpenSmallButton("Not now", filled = false, onClick = onDismiss)
        }
    }
}
