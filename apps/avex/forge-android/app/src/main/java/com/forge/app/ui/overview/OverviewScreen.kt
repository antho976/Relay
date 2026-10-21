package com.forge.app.ui.overview

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.core.time.userWeekDayIndex
import com.forge.app.ui.goals.customGoalValueLine
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.CoachHomeSummary
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.goal.customPinKey
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.Program
import com.forge.app.ui.coach.callCopy
import com.forge.app.ui.common.OpenDeltaBars
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenNotificationBell
import com.forge.app.ui.common.OpenPrTag
import com.forge.app.ui.common.OpenPrimaryButton
import com.forge.app.ui.common.OpenSectionHead
import com.forge.app.ui.common.OpenSlotLabel
import com.forge.app.ui.common.OpenSparkline
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.goalCaption
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.overview.state.OverviewUiState
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.domain.coach.TodayDirective
import com.forge.app.domain.goal.GoalMetric
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

/*
 * Home, as redesigned 2026-10-21 ("05 Open panel", Relay design 7, frame c5).
 *
 * Top to bottom: the date and three glyphs; TODAY with the session in serif, its spec line, the
 * COACH line and the week's dots; one red Start; two readings the user picks (Sleep and Yesterday by
 * default); the goal scoreboard; the last three sessions. No boxes around any of it: sections hold
 * by the 24dp gutter and ~52dp of air, and red is spent only on today, the action and what moved.
 */

/** Home shows at most three goals and three recent sessions. */
private const val HOME_LIST_CAP = 3

/** The air between sections, as drawn. */
private val SECTION_GAP = 52.dp

// Two orders, one per Settings → Format → Week starts. The dots are indexed in the user's week
// (StatsRepository.weekDaysTrained, userWeekDayIndex), so the letters rotate with them.
private val WEEK_INITIALS_MONDAY = listOf("M", "T", "W", "T", "F", "S", "S")
private val WEEK_INITIALS_SUNDAY = listOf("S", "M", "T", "W", "T", "F", "S")
private val WEEK_NAMES_MONDAY = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
private val WEEK_NAMES_SUNDAY = listOf("Sunday") + WEEK_NAMES_MONDAY.dropLast(1)

/** "WED 21 OCT" in the top bar, in the phone's own locale. */
private val HOME_DATE_FORMAT: java.time.format.DateTimeFormatter =
    java.time.format.DateTimeFormatter.ofPattern("EEE d MMM")

@Composable
fun OverviewScreen(
    onStartSession: (dayKey: String) -> Unit,
    /** Opens the Academy on the cold-start lesson the directive is carrying (B3). */
    onOpenAcademy: () -> Unit = {},
    onStartSessionSkipWarmup: (dayKey: String) -> Unit = onStartSession,
    onViewProgram: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    onOpenNotes: () -> Unit = {},
    onGoToNutrition: () -> Unit = {},
    onOpenCoachBrief: () -> Unit = {},
    onOpenCoachLab: () -> Unit = {},
    onOpenGoals: () -> Unit = {},
    onOpenProfile: () -> Unit = {},
    onOpenSettings: () -> Unit = {},
    onOpenSession: (Long) -> Unit = {},
    onViewAllHistory: () -> Unit = {},
    onLogFreestyle: () -> Unit = {},
    onBuildPlan: () -> Unit = {},
    /** A body reading's tap: the full body view on Stats. */
    onOpenBody: () -> Unit = {},
    /** A watch slot's "Connect": Settings → Wearable. */
    onConnectWatch: () -> Unit = {},
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val freestyleMode by viewModel.freestyleMode.collectAsStateWithLifecycle()
    val programEmpty by viewModel.programEmpty.collectAsStateWithLifecycle()
    val selectedItem by viewModel.selectedItem.collectAsStateWithLifecycle()
    val summaryLines by viewModel.sessionExerciseLines.collectAsStateWithLifecycle()
    val spec by viewModel.spec.collectAsStateWithLifecycle()
    val coach by viewModel.coach.collectAsStateWithLifecycle()
    val slots by viewModel.slots.collectAsStateWithLifecycle()

    // Keep the readings current across a day of glances (W6): a night's sleep that synced, steps
    // taken while away, and today's answer, which is date- and session-sensitive (B2).
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                viewModel.refreshBody()
                viewModel.refreshDirective()
                viewModel.refreshCoach()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    // Firing a milestone persists it and queues it under the bell, where it waits to be read.
    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { viewModel.onMilestoneShown(it.id) }
    }

    // Rise + fade on the first show after a cold launch only; swiping back to Home just shows it.
    var entrancePlayed by rememberSaveable { mutableStateOf(false) }
    val entry = remember { Animatable(if (entrancePlayed) 1f else 0f) }
    LaunchedEffect(Unit) {
        if (!entrancePlayed) {
            entry.animateTo(1f, animationSpec = ForgeMotion.enterTween(450))
            entrancePlayed = true
        }
    }

    var pickingSlot by remember { mutableStateOf<Boolean?>(null) }

    val settings = LocalForgeSettings.current
    val today = LocalDate.now()
    val todayDow = userWeekDayIndex(today, settings.firstDayMonday)
    val nextDay = Program.days.firstOrNull { it.key == state.nextUpDayKey }

    if (selectedItem != null) {
        val item = selectedItem!!
        SummarySheet(
            title = item.displayTitle(com.forge.app.ui.cardio.LocalCardioTypes.current),
            dateMs = item.timestampMs,
            tag = item.tag,
            durationMin = item.durationMin,
            volumeLb = item.volumeLb,
            prCount = item.prCount,
            vsAvgPct = item.vsAvgPct,
            isBest = item.isBest,
            isGym = item.isGym,
            distanceKm = item.distanceKm,
            exerciseLines = summaryLines,
            onDismiss = { viewModel.clearSelectedItem() }
        )
    }
    pickingSlot?.let { left ->
        HomeSlotSheet(
            left = left,
            slots = slots,
            onPick = { viewModel.setSlot(left, it); pickingSlot = null },
            onDismiss = { pickingSlot = null }
        )
    }

    // The ground under Home is the app's own Pearl gradient (ForgeTheme); the Scaffold stays clear.
    Scaffold(containerColor = Color.Transparent) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .graphicsLayer {
                    alpha = entry.value
                    translationY = (1f - entry.value) * 18.dp.toPx()
                }
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            Spacer(Modifier.height(4.dp))
            TopRow(today, onOpenProfile = onOpenProfile, onOpenSettings = onOpenSettings)

            val directive = state.directive
            val resumeKey = state.activeSessionDayKey
            val sessionDayKey = resumeKey ?: directive?.dayKey ?: state.nextUpDayKey
            val trainedToday = todayDow in state.weekDaysTrained
            val headline = when {
                freestyleMode -> "Open workout"
                programEmpty -> "No plan yet"
                else -> directive?.headline ?: state.customDayName ?: nextDay?.defaultName ?: "Ready"
            }
            val actionText = when {
                resumeKey != null -> "Resume session"
                freestyleMode -> "Log workout"
                programEmpty -> "Build plan"
                directive?.kind == TodayDirective.Kind.CARDIO -> "Log cardio"
                directive?.kind == TodayDirective.Kind.LEARN -> "Open Academy"
                directive?.kind == TodayDirective.Kind.REST -> "Train anyway"
                else -> "Start session"
            }
            val action: () -> Unit = when {
                resumeKey != null -> ({ onStartSession(resumeKey) })
                freestyleMode -> onLogFreestyle
                programEmpty -> onBuildPlan
                directive?.kind == TodayDirective.Kind.CARDIO -> onGoToCardio
                directive?.kind == TodayDirective.Kind.LEARN -> onOpenAcademy
                else -> ({ onStartSession(sessionDayKey) })
            }
            val restDay = resumeKey == null && !freestyleMode && !programEmpty &&
                directive?.kind == TodayDirective.Kind.REST
            val startsNewGymSession = resumeKey == null && !freestyleMode && !programEmpty &&
                directive?.kind != TodayDirective.Kind.CARDIO &&
                directive?.kind != TodayDirective.Kind.LEARN
            // The spec line describes a gym session; a rest, cardio or lesson day has none to describe.
            val specLine = when {
                freestyleMode -> "No fixed plan · log what you trained"
                programEmpty -> "Build your plan once"
                restDay || !startsNewGymSession && resumeKey == null -> directive?.secondary
                else -> spec?.let { "${it.exercises} exercises · ${it.sets} sets · ~${it.minutes} min" }
            }

            Spacer(Modifier.height(32.dp))
            OpenLabel(if (directive == null && trainedToday) "Tomorrow" else "Today")
            Spacer(Modifier.height(8.dp))
            HeroLine(headline)
            if (specLine != null) {
                Spacer(Modifier.height(8.dp))
                OpenLabel(specLine, style = OpenType.monoSTight)
            }
            coach?.let { summary ->
                Spacer(Modifier.height(36.dp))
                CoachRow(summary, onOpenCoachBrief)
            }
            Spacer(Modifier.height(26.dp))
            WeekRow(
                state = state,
                todayIndex = todayDow,
                initials = if (settings.firstDayMonday) WEEK_INITIALS_MONDAY else WEEK_INITIALS_SUNDAY,
                names = if (settings.firstDayMonday) WEEK_NAMES_MONDAY else WEEK_NAMES_SUNDAY,
                freestyle = freestyleMode || programEmpty,
            )
            Spacer(Modifier.height(38.dp))
            OpenPrimaryButton(
                text = actionText,
                onClick = action,
                // A rest day still offers the session, outlined: a filled button under "Rest today"
                // would tell you to do what the headline just said not to.
                emphasized = !restDay,
                // The hold stays as a shortcut for people who know it; TalkBack lists it.
                onLongClick = if (startsNewGymSession) ({ onStartSessionSkipWarmup(sessionDayKey) }) else null,
                onLongClickLabel = if (startsNewGymSession) "Start, skipping warmup" else null,
            )

            Spacer(Modifier.height(SECTION_GAP))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                SlotReadingView(
                    slot = slots.left,
                    reading = slots.readings[slots.left],
                    watchInvite = slots.watchInvite,
                    onChange = { pickingSlot = true },
                    onConnectWatch = onConnectWatch,
                    onOpen = slotTap(slots.left, onOpenBody, onViewAllHistory),
                    modifier = Modifier.weight(1f)
                )
                SlotReadingView(
                    slot = slots.right,
                    reading = slots.readings[slots.right],
                    watchInvite = slots.watchInvite,
                    onChange = { pickingSlot = false },
                    onConnectWatch = onConnectWatch,
                    onOpen = slotTap(slots.right, onOpenBody, onViewAllHistory),
                    modifier = Modifier.weight(1f)
                )
            }

            if (HOME_GOALS_TILE !in settings.hiddenOverviewTiles) {
                val goals = scoreboardGoals(state, settings.pinnedGoalKeys)
                Spacer(Modifier.height(SECTION_GAP))
                OpenSectionHead(
                    "Goals",
                    action = "View all",
                    actionLabel = "Open goals",
                    onAction = onOpenGoals
                )
                Spacer(Modifier.height(20.dp))
                // Nothing before the first read (todayStartMs is 0 until then), so a returning user
                // never sees the empty prompt flash while their goals load.
                if (state.todayStartMs != 0L) {
                    if (goals.isEmpty()) {
                        EmptyLine("Pin up to three goals to see them here.", "Pin a goal", onOpenGoals)
                    } else {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                            goals.forEach { GoalColumn(it, onOpenGoals, Modifier.weight(1f)) }
                            // Fewer than three keep their column width, so a lone goal is not a banner.
                            repeat(HOME_LIST_CAP - goals.size) { Spacer(Modifier.weight(1f)) }
                        }
                    }
                }
            }

            Spacer(Modifier.height(SECTION_GAP))
            OpenSectionHead(
                "Recent",
                action = if (state.recentItems.isEmpty()) null else "View all",
                actionLabel = "View all sessions",
                onAction = onViewAllHistory
            )
            Spacer(Modifier.height(22.dp))
            if (state.todayStartMs != 0L) {
                if (state.recentItems.isEmpty()) {
                    Text(
                        when {
                            freestyleMode -> "Your first logged workout lands here."
                            programEmpty -> "Build a plan above, then your first session lands here."
                            else -> "Your first session lands here."
                        },
                        style = OpenType.body,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(26.dp)) {
                        state.recentItems.take(HOME_LIST_CAP).forEach { item ->
                            RecentRow(item) {
                                if (item.isGym) onOpenSession(item.id) else viewModel.selectRecentItem(item)
                            }
                        }
                    }
                }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

@Composable
private fun TopRow(today: LocalDate, onOpenProfile: () -> Unit, onOpenSettings: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().height(48.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(today.format(HOME_DATE_FORMAT), style = OpenType.monoM)
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenNotificationBell()
            OpenTopBarIcon(OpenIcons.User, "Profile", onOpenProfile)
            OpenTopBarIcon(OpenIcons.Settings, "Settings", onOpenSettings)
        }
    }
}

private val HERO_STEPS = listOf(OpenType.hero, OpenType.title, OpenType.heroS)

/** The serif hero, its scaling clamped at 1.3× so the biggest font setting keeps Start on screen. */
@Composable
private fun HeroLine(text: String) {
    val density = LocalDensity.current
    CompositionLocalProvider(LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))) {
        // "Pull B" is what the design drew at 76; a longer answer ("Done for today") steps down the
        // serif rungs until it fits one line, and only wraps once it is at the smallest.
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            val measurer = androidx.compose.ui.text.rememberTextMeasurer()
            val width = constraints.maxWidth
            val style = remember(text, width) {
                HERO_STEPS.firstOrNull { st ->
                    measurer.measure(text, st, maxLines = 1, softWrap = false).size.width <= width
                } ?: HERO_STEPS.last()
            }
            Text(
                text,
                style = style,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}

/**
 * COACH on the left, the week's open call and what is on watch on the right, as one tappable row
 * into the brief: "Barbell Row · Add load · Squat on watch".
 */
@Composable
private fun CoachRow(summary: CoachHomeSummary, onOpen: () -> Unit) {
    val line = coachLine(summary)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .touchSlop(vertical = 12.dp)
            .clickableLabeled("Open the coach's brief", onClick = onOpen)
            .padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel("Coach")
        Row(
            Modifier.weight(1f),
            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                line,
                style = OpenType.body,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false)
            )
            Icon(OpenIcons.ChevronRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        }
    }
}

/** The open call first, then what is on watch; "Holding steady" when the week asks nothing. */
internal fun coachLine(summary: CoachHomeSummary): String {
    val parts = mutableListOf<String>()
    summary.open.firstOrNull()?.let { d ->
        val c = callCopy(d)
        parts += if (d.targetKey == "week") c.change else "${c.subject} · ${c.change}"
        if (summary.open.size > 1) parts += "${summary.open.size - 1} more"
    }
    when (summary.watching.size) {
        0 -> Unit
        1 -> parts += "${summary.watching.first().targetName} on watch"
        else -> parts += "${summary.watching.size} on watch"
    }
    return parts.joinToString(" · ").ifBlank { "Holding steady" }
}

/**
 * THIS WEEK with the count, then seven dots: red with a dark letter for a day trained, a red ring on
 * a wash for today, a moon for a planned rest day, an outlined letter for the rest.
 */
@Composable
private fun WeekRow(
    state: OverviewUiState,
    todayIndex: Int,
    initials: List<String>,
    names: List<String>,
    freestyle: Boolean,
) {
    val days = state.weekDaysTrained.size
    val count = if (!freestyle && state.weeklyTrainingDays > 0) "$days / ${state.weeklyTrainingDays}" else if (days == 1) "1 day" else "$days days"
    val ink = MaterialTheme.colorScheme.onBackground
    val reading = "This week, $days of " + (if (!freestyle && state.weeklyTrainingDays > 0) "${state.weeklyTrainingDays} days trained" else "7 days trained")
    val label = buildAnnotatedString {
        append("THIS WEEK  ")
        withStyle(SpanStyle(color = ink)) { append(count.uppercase()) }
    }
    val dots: @Composable (dot: androidx.compose.ui.unit.Dp, gap: androidx.compose.ui.unit.Dp, modifier: Modifier) -> Unit = { dot, gap, m ->
        Row(horizontalArrangement = Arrangement.spacedBy(gap), modifier = m.clearAndSetSemantics { }) {
            for (i in 0 until 7) {
                WeekDot(
                    letter = initials[i],
                    trained = i in state.weekDaysTrained,
                    today = i == todayIndex,
                    rest = i in state.weekRestDays && i !in state.weekDaysTrained,
                    name = names[i],
                    size = dot,
                )
            }
        }
    }
    // The design's row is 412 wide. A narrower phone (an S21 at 384dp) or a larger font left the
    // count touching the first dot, so the dots close up first, then shrink, and only when even that
    // will not leave 16dp of air does the label take its own line above them.
    androidx.compose.foundation.layout.BoxWithConstraints(
        Modifier.fillMaxWidth().semantics(mergeDescendants = true) { contentDescription = reading }
    ) {
        val measurer = androidx.compose.ui.text.rememberTextMeasurer()
        val density = LocalDensity.current
        val labelWidth = with(density) { measurer.measure(label, OpenType.monoS).size.width.toDp() }
        val width = maxWidth
        val room = width - labelWidth - 16.dp
        val full = 24.dp * 7 + 7.dp * 6
        when {
            room >= full -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(label, style = OpenType.monoS, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.clearAndSetSemantics { })
                dots(24.dp, 7.dp, Modifier)
            }
            (room - 4.dp * 6) / 7 >= 20.dp -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(label, style = OpenType.monoS, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.clearAndSetSemantics { })
                dots(minOf(24.dp, (room - 4.dp * 6) / 7), 4.dp, Modifier)
            }
            else -> Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(label, style = OpenType.monoS, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.clearAndSetSemantics { })
                dots(24.dp, ((width - 24.dp * 7) / 6).coerceAtMost(16.dp), Modifier)
            }
        }
    }
}

private val DotLetter = OpenType.tag.copy(fontWeight = FontWeight.Bold, letterSpacing = 0.sp)

@Composable
private fun WeekDot(letter: String, trained: Boolean, today: Boolean, rest: Boolean, name: String, size: androidx.compose.ui.unit.Dp = 24.dp) {
    val accent = MaterialTheme.colorScheme.primary
    val base = Modifier.size(size)
    val modifier = when {
        trained -> base.background(accent, CircleShape)
        today -> base.background(MaterialTheme.colorScheme.primaryContainer, CircleShape).border(1.5.dp, accent, CircleShape)
        rest -> base.border(1.5.dp, OpenColors.Track, CircleShape)
        else -> base.border(1.5.dp, OpenColors.Rule, CircleShape)
    }
    Box(modifier, contentAlignment = Alignment.Center) {
        if (rest && !today) {
            Icon(OpenIcons.Moon, contentDescription = "$name, rest", tint = OpenColors.Glyph, modifier = Modifier.size(12.dp))
        } else {
            Text(
                letter,
                style = DotLetter,
                textAlign = TextAlign.Center,
                color = when {
                    trained -> MaterialTheme.colorScheme.onPrimary
                    today -> MaterialTheme.colorScheme.onBackground
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                }
            )
        }
    }
}

/** The unit an empty reading's zero wears, so "0h" reads as no sleep yet rather than a stray digit. */
@Composable
private fun emptyUnit(slot: HomeSlot): String = when (slot) {
    HomeSlot.SLEEP -> "h"
    HomeSlot.RESTING_HR -> " bpm"
    HomeSlot.HRV -> " ms"
    HomeSlot.STEPS -> ""
    HomeSlot.BODY_WEIGHT -> " " + com.forge.app.domain.units.unitLabel(LocalForgeSettings.current.weightUnit)
    HomeSlot.LAST_SESSION -> " min"
}

/** What tapping a reading's figure opens: the body view for the watch and weight, the log for a session. */
private fun slotTap(slot: HomeSlot, onOpenBody: () -> Unit, onOpenHistory: () -> Unit): () -> Unit =
    if (slot == HomeSlot.LAST_SESSION) onOpenHistory else onOpenBody

/**
 * One reading in its slot: the label with ▾ (tap to swap the reading), the serif figure with its unit,
 * the mark, and the change line. A watch reading with nothing to show says why, once.
 */
@Composable
private fun SlotReadingView(
    slot: HomeSlot,
    reading: SlotReading?,
    watchInvite: Boolean,
    onChange: () -> Unit,
    onConnectWatch: () -> Unit,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val ink = MaterialTheme.colorScheme.onBackground
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OpenSlotLabel(reading?.label ?: slot.label, onClick = onChange)
        if (reading == null) {
            // Nothing to read yet: a zero in the figure's place and a bare baseline where the mark
            // goes, so the empty slot keeps the same shape as the one beside it.
            FigureLine(figureWithUnit("0", emptyUnit(slot), muted), muted)
            OpenDeltaBars(emptyList(), Modifier.fillMaxWidth().height(30.dp))
            when {
                slot.needsWatch && watchInvite ->
                    OpenLabel(
                        "Connect a watch",
                        style = OpenType.monoSTight,
                        color = ink,
                        modifier = Modifier.touchSlop(vertical = 14.dp).clickableLabeled("Connect a watch", onClick = onConnectWatch).padding(vertical = 14.dp)
                    )
                slot == HomeSlot.LAST_SESSION -> OpenLabel("No sessions yet", style = OpenType.monoSTight)
                slot == HomeSlot.BODY_WEIGHT -> OpenLabel("No weigh-ins yet", style = OpenType.monoSTight)
                slot == HomeSlot.STEPS -> OpenLabel("None counted yet", style = OpenType.monoSTight)
                else -> OpenLabel("Nothing synced yet", style = OpenType.monoSTight)
            }
            return@Column
        }
        Column(
            Modifier
                .clickableLabeled(reading.description, onClick = onOpen)
                .semantics(mergeDescendants = true) { contentDescription = reading.description },
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            FigureLine(figureWithUnit(reading.figure, reading.unit, muted), ink)
            when {
                reading.bars != null -> OpenDeltaBars(reading.bars, Modifier.fillMaxWidth().height(30.dp), barWidth = 22.dp)
                reading.spark.size < 2 -> OpenDeltaBars(emptyList(), Modifier.fillMaxWidth().height(30.dp))
                reading.spark.size >= 2 -> OpenSparkline(reading.spark, Modifier.fillMaxWidth().height(30.dp))
                else -> Spacer(Modifier.height(30.dp))
            }
            reading.change?.let {
                OpenLabel(it, style = OpenType.monoSTight, color = if (reading.changeMoved) ink else muted, modifier = Modifier.clearAndSetSemantics { })
            }
        }
    }
}

/** A reading's serif figure. One call for the real and the empty reading, so both rows line up. */
@Composable
private fun FigureLine(text: AnnotatedString, color: Color) {
    Text(
        text,
        style = OpenType.figureL,
        color = color,
        maxLines = 1,
        softWrap = false,
        overflow = TextOverflow.Clip,
        modifier = Modifier.clearAndSetSemantics { }
    )
}

/** "7.4h": the figure at its size, the unit after it at half size in the muted voice. */
private fun figureWithUnit(figure: String, unit: String, unitColor: Color): AnnotatedString =
    buildAnnotatedString {
        append(figure)
        if (unit.isNotEmpty()) {
            withStyle(OpenType.figureLUnit.toSpanStyle().copy(color = unitColor)) { append(unit) }
        }
    }

/** One goal on the scoreboard. */
internal data class ScoreGoal(
    val key: String,
    val label: String,
    val figure: String,
    /** The muted "/8" after a count figure, or empty. */
    val denominator: String,
    val fraction: Float,
    val change: String?,
    val changeMoved: Boolean,
    val description: String,
)

/**
 * The pinned goals (or, with none pinned, the reached ones last and the closest first), as the
 * scoreboard reads them: "BENCH 225" over "205", the meter, "↑ 10 in 30d".
 */
@Composable
private fun scoreboardGoals(state: OverviewUiState, pinnedKeys: List<String>): List<ScoreGoal> {
    val settings = LocalForgeSettings.current
    val now = remember(state.customGoals, state.todayStartMs) { System.currentTimeMillis() }
    val unit = settings.weightUnit
    val lift = state.goals.map { g ->
        val delta = state.liftGoalChange30dLb[g.exerciseId]
        val change = when {
            g.achieved -> "Reached"
            delta == null -> null
            delta >= 0.5 -> "↑ ${weightInputValue(delta, unit)} in 30d"
            else -> "Flat over 30d"
        }
        val target = weightInputValue(g.targetLb, unit)
        val current = weightInputValue(g.currentBestLb, unit)
        ScoreGoal(
            key = liftPinKey(g.exerciseId),
            label = "${goalShortName(g.name)} $target",
            figure = current,
            denominator = "",
            fraction = g.fraction,
            change = change,
            changeMoved = g.achieved || (delta != null && delta >= 0.5),
            description = "${g.name}, $current of $target" + (change?.let { ", ${it.replace("↑", "up")}" } ?: ""),
        )
    }
    val custom = state.customGoals.map { g ->
        val countable = g.metric == GoalMetric.SESSIONS || g.metric == GoalMetric.CARDIO_MINUTES
        val valueLine = customGoalValueLine(g, unit, settings.useMiles)
        val caption = goalCaption(g.achieved, g.metric, g.period, g.baselineValue, unit, now)
        ScoreGoal(
            key = customPinKey(g.id),
            label = customGoalTitle(g),
            figure = if (countable) "${g.currentValue.toInt()}" else valueLine.substringBefore(" ").trim(),
            denominator = if (countable) "/${g.targetValue.toInt()}" else "",
            fraction = g.fraction,
            change = caption,
            changeMoved = g.achieved,
            description = "${customGoalTitle(g)}, $valueLine" + (caption?.let { ", $it" } ?: ""),
        )
    }
    val all = lift + custom
    val pinned = pinnedKeys.mapNotNull { key -> all.firstOrNull { it.key == key } }
    return if (pinned.isNotEmpty()) pinned.take(HOME_LIST_CAP)
    else all.sortedWith(compareBy<ScoreGoal> { it.fraction >= 1f }.thenByDescending { it.fraction }).take(HOME_LIST_CAP)
}

/** "Barbell Bench Press" → "Bench": the scoreboard column is a third of the page wide. */
internal fun goalShortName(name: String): String {
    val words = name.trim().split(' ').filter { it.isNotBlank() }
    val dropped = words.filterNot { it.lowercase() in setOf("barbell", "dumbbell", "back", "press", "machine", "cable") }
    return (dropped.firstOrNull() ?: words.firstOrNull() ?: name)
}

@Composable
private fun GoalColumn(goal: ScoreGoal, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    com.forge.app.ui.common.OpenGoalColumn(
        label = goal.label,
        figure = goal.figure,
        denominator = goal.denominator,
        fraction = goal.fraction,
        change = goal.change,
        changeMoved = goal.changeMoved,
        description = goal.description,
        onClick = onOpen,
        modifier = modifier,
    )
}

@Composable
private fun EmptyLine(text: String, action: String, onClick: () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(text, style = OpenType.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
        OpenLabel(
            action,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.touchSlop(vertical = 14.dp).clickableLabeled(action, onClick = onClick).padding(vertical = 14.dp)
        )
    }
}

/**
 * A recent session: its date in a narrow column (MON over a serif 19), the title and "48 min ·
 * Bench Press 205×5" with a PR tag when it set one, and the kind of session as a glyph on a quiet
 * square tile at the right (Antho kept the old row's mark, 2026-10-21; the redesign squares its
 * corners and drops its colour).
 */
@Composable
private fun RecentRow(item: OverviewRecentItem, onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val ink = MaterialTheme.colorScheme.onBackground
    val title = item.displayTitle(com.forge.app.ui.cardio.LocalCardioTypes.current)
    val date = Instant.ofEpochMilli(item.timestampMs).atZone(ZoneId.systemDefault()).toLocalDate()
    val dow = date.dayOfWeek.getDisplayName(java.time.format.TextStyle.SHORT, com.forge.app.ui.common.currentLocale())
    val sub = listOfNotNull(
        item.durationMin?.let { "$it min" },
        if (item.isGym) item.topLift else item.distanceKm?.let {
            com.forge.app.domain.units.formatDistance(it, LocalForgeSettings.current.useMiles)
        }
    ).joinToString(" · ")
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("Open $title", onClick = onClick),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.width(34.dp)) {
            OpenLabel(dow, style = OpenType.monoXS)
            Text("${date.dayOfMonth}", style = OpenType.figureS, color = ink)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = OpenType.rowTitle, color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(sub, style = OpenType.bodyS, color = muted, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                if (item.prCount > 0) OpenPrTag()
            }
        }
        Box(
            Modifier
                .size(32.dp)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh, androidx.compose.foundation.shape.RoundedCornerShape(4.dp)),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                if (item.isGym) OpenIcons.Dumbbell else OpenIcons.Activity,
                contentDescription = null,
                tint = muted,
                modifier = Modifier.size(18.dp)
            )
        }
    }
}
