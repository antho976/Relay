package com.forge.app.ui.overview

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import com.forge.app.ui.common.NotificationBell
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.overview.state.OverviewRecentItem
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.formatVolume
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.Program
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.experiment.SurfaceListRow
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.WeekStrip
import com.forge.app.ui.experiment.surfacePalette
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.goals.customPinKey
import com.forge.app.ui.goals.goalGlyph
import com.forge.app.ui.goals.liftGoalGlyph
import com.forge.app.ui.goals.liftPinKey
import com.forge.app.ui.settings.SettingsIcons
import java.time.LocalDate

@Composable
private fun HomePrimaryAction(
    text: String,
    label: String,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .heightIn(min = 52.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(MaterialTheme.colorScheme.primary)
            .bounceCombinedClick(
                onClickLabel = label,
                onLongClickLabel = if (onLongClick != null) "Start, skipping warmup" else null,
                onLongClick = onLongClick,
                onClick = onClick
            )
            .padding(horizontal = 20.dp, vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onPrimary,
        )
    }
}

@Composable
private fun HomePlanAction(text: String, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .heightIn(min = 52.dp)
            .clip(RoundedCornerShape(14.dp))
            .border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(14.dp))
            .clickableLabeled(text, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onBackground,
        )
    }
}

/** Home shows at most three goals and three recent sessions, matching the second backup. */
private const val HOME_LIST_CAP = 3

private val WEEK_INITIALS = listOf("M", "T", "W", "T", "F", "S", "S")

@Composable
private fun PromptLine(text: String, label: String, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickableLabeled(label, onClick = onClick)
            .padding(vertical = 14.dp)
    ) {
        Text(
            "+",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.alignByBaseline()
        )
        Spacer(Modifier.width(10.dp))
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.alignByBaseline()
        )
    }
}

@Composable
private fun RecentRow(
    item: OverviewRecentItem,
    palette: SurfacePalette,
    onBg: Color,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    useMiles: Boolean,
    onClick: () -> Unit
) {
    val figure = when {
        item.isGym && (item.volumeLb ?: 0.0) > 0.0 -> formatVolume(item.volumeLb!!, weightUnit)
        !item.isGym && (item.distanceKm ?: 0.0) > 0.0 ->
            com.forge.app.domain.units.formatDistance(item.distanceKm!!, useMiles)
        else -> null
    }
    val delta = when {
        item.isBest -> "BEST"
        item.vsAvgPct != null -> "${if (item.vsAvgPct >= 0) "+" else ""}${item.vsAvgPct}% avg"
        item.prCount > 0 -> "${item.prCount} PR"
        else -> null
    }
    val deltaColor = when {
        item.isBest -> palette.positive
        item.vsAvgPct != null && item.vsAvgPct > 0 -> palette.positive
        item.vsAvgPct != null && item.vsAvgPct < 0 -> palette.negative
        else -> palette.mutedOnCard
    }
    val sub = listOfNotNull(
        item.dayLabel.takeIf { it.isNotBlank() },
        item.statusPill.takeIf { it.isNotBlank() },
        item.topLift
    ).joinToString(" · ")

    SurfaceListRow(
        icon = if (item.isGym) SettingsIcons.Session else NavIcons.Cardio,
        hue = palette.hues[0],
        label = item.title,
        sub = sub,
        figure = figure,
        delta = delta,
        deltaColor = deltaColor,
        onBg = onBg,
        muted = palette.mutedOnCard,
        onClick = onClick,
        clickLabel = "Open ${item.title}"
    )
}

private data class GoalLineData(
    val key: String,
    val title: String,
    val valueLine: String,
    val fraction: Float,
    val achieved: Boolean,
    val icon: ImageVector
)

@Composable
private fun pinnedGoals(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    pinnedKeys: List<String>
): List<GoalLineData> {
    val settings = LocalForgeSettings.current
    val lift = state.goals.map { goal ->
        GoalLineData(
            key = liftPinKey(goal.exerciseId),
            title = goal.name,
            valueLine = "${com.forge.app.domain.units.weightInputValue(goal.currentBestLb, settings.weightUnit)} / " +
                "${com.forge.app.domain.units.weightInputValue(goal.targetLb, settings.weightUnit)} " +
                com.forge.app.domain.units.unitLabel(settings.weightUnit),
            fraction = goal.fraction,
            achieved = goal.achieved,
            icon = liftGoalGlyph(goal.exerciseId)
        )
    }
    val custom = state.customGoals.map { goal ->
        GoalLineData(
            key = customPinKey(goal.id),
            title = customGoalTitle(goal),
            valueLine = customGoalValueLine(goal, settings.weightUnit, settings.useMiles),
            fraction = goal.fraction,
            achieved = goal.achieved,
            icon = goalGlyph(goal.metric)
        )
    }
    val all = lift + custom
    val pinned = pinnedKeys.mapNotNull { key -> all.firstOrNull { it.key == key } }
    return if (pinned.isNotEmpty()) {
        pinned.take(HOME_LIST_CAP)
    } else {
        all.sortedWith(compareBy<GoalLineData> { it.achieved }.thenByDescending { it.fraction })
            .take(HOME_LIST_CAP)
    }
}

/** Half the slack between a top-bar icon's 48dp touch target and its 20dp glyph. */
private val GUTTER_SLACK = 14.dp

/** Top-bar icon with a 48dp tappable area + spoken label, while the glyph stays visually small. */
@Composable
private fun TopBarIconButton(
    icon: ImageVector,
    label: String,
    tint: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
            .clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(20.dp))
    }
}

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
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val freestyleMode by viewModel.freestyleMode.collectAsStateWithLifecycle()
    val coachEnabled by viewModel.coachEnabled.collectAsStateWithLifecycle()
    val programEmpty by viewModel.programEmpty.collectAsStateWithLifecycle()
    val selectedItem by viewModel.selectedItem.collectAsStateWithLifecycle()
    val summaryLines by viewModel.sessionExerciseLines.collectAsStateWithLifecycle()
    val movement by viewModel.movement.collectAsStateWithLifecycle()

    // Keep the movement line current across a day of glances (W6) — steps taken while away should
    // show on return, same resume-refresh rule as the cardio hero's TODAY line (GYMAP-64).
    val movementLifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(movementLifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                viewModel.refreshMovement()
                // The answer is date- and session-sensitive: a directive that still says "Push day"
                // after you trained, or after midnight, is worse than none (B2).
                viewModel.refreshDirective()
            }
        }
        movementLifecycleOwner.lifecycle.addObserver(observer)
        onDispose { movementLifecycleOwner.lifecycle.removeObserver(observer) }
    }
    // Firing a milestone persists it (so it can't recompute/re-fire) AND queues it in the
    // notifications feed. It used to flash here as a six-second toast, which meant a milestone earned
    // while the app was closed was gone before it was ever read; it now waits under the bell.
    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { viewModel.onMilestoneShown(it.id) }
    }

    // Rise + fade entrance, but ONLY on the first show after a cold launch — not every time you
    // swipe back to Home (Home is a pager page that gets disposed/recomposed as you swipe away and
    // back). The "played" flag is rememberSaveable so it survives the page leaving the viewport and
    // returning; after the first play, Home settles in like any other swipable page.
    var entrancePlayed by rememberSaveable { mutableStateOf(false) }
    val entry = remember { Animatable(if (entrancePlayed) 1f else 0f) }
    LaunchedEffect(Unit) {
        if (!entrancePlayed) {
            entry.animateTo(1f, animationSpec = tween(450))
            entrancePlayed = true
        }
    }

    val today = LocalDate.now()
    val todayDow = today.dayOfWeek.value - 1

    val nextDay = Program.days.firstOrNull { it.key == state.nextUpDayKey }

    val baseColors = MaterialTheme.colorScheme
    val settings = LocalForgeSettings.current
    val palette = surfacePalette()
    val onBg = baseColors.onBackground
    val muted = baseColors.onSurfaceVariant
    val accent = baseColors.primary
    val outline = baseColors.outline

    if (selectedItem != null) {
        val item = selectedItem!!
        SummarySheet(
            title = item.title,
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

    val homeColors = baseColors.copy(
        background = Color.Black,
        surface = Color.Black
    )

    MaterialTheme(colorScheme = homeColors) {
        Scaffold(containerColor = MaterialTheme.colorScheme.background) { inner ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(inner)
                    .graphicsLayer {
                        alpha = entry.value
                        translationY = (1f - entry.value) * 18.dp.toPx()
                    }
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp)
            ) {
                Spacer(Modifier.height(8.dp))

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    NotificationBell(
                        modifier = Modifier
                            .offset(x = (-GUTTER_SLACK))
                            .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        TopBarIconButton(
                            NavIcons.Profile,
                            "Profile",
                            MaterialTheme.colorScheme.onSurfaceVariant,
                            onClick = onOpenProfile
                        )
                        TopBarIconButton(
                            Icons.Default.Settings,
                            "Settings",
                            MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.offset(x = GUTTER_SLACK),
                            onClick = onOpenSettings
                        )
                    }
                }

                val directive = state.directive
                val resumeKey = state.activeSessionDayKey
                val sessionDayKey = resumeKey ?: directive?.dayKey ?: state.nextUpDayKey
                val trainedToday = todayDow in state.weekDaysTrained
                val headline = when {
                    freestyleMode -> "Open workout"
                    programEmpty -> "No plan yet"
                    else -> directive?.headline ?: state.customDayName ?: nextDay?.defaultName ?: "Ready"
                }
                val reason = when {
                    freestyleMode -> "No fixed plan. Log what you trained and keep moving."
                    programEmpty -> "Build your plan once, then Home will always show what comes next."
                    directive != null -> directive.reason
                    nextDay != null -> "${nextDay.subtitle} · ${nextDay.exercises.size} exercises"
                    else -> "Your next session will appear here."
                }
                val actionText = when {
                    resumeKey != null -> "Resume session"
                    freestyleMode -> "Log workout"
                    programEmpty -> "Build plan"
                    directive?.kind == com.forge.app.domain.coach.TodayDirective.Kind.CARDIO -> "Log cardio"
                    directive?.kind == com.forge.app.domain.coach.TodayDirective.Kind.LEARN -> "Open Academy"
                    directive?.kind == com.forge.app.domain.coach.TodayDirective.Kind.REST -> "Train anyway"
                    else -> "Start session"
                }
                val action: () -> Unit = when {
                    resumeKey != null -> ({ onStartSession(resumeKey) })
                    freestyleMode -> onLogFreestyle
                    programEmpty -> onBuildPlan
                    directive?.kind == com.forge.app.domain.coach.TodayDirective.Kind.CARDIO -> onGoToCardio
                    directive?.kind == com.forge.app.domain.coach.TodayDirective.Kind.LEARN -> onOpenAcademy
                    else -> ({ onStartSession(sessionDayKey) })
                }
                val startsNewGymSession = resumeKey == null && !freestyleMode && !programEmpty &&
                    directive?.kind != com.forge.app.domain.coach.TodayDirective.Kind.CARDIO &&
                    directive?.kind != com.forge.app.domain.coach.TodayDirective.Kind.LEARN
                val planAction = if (programEmpty || freestyleMode) onBuildPlan else onViewProgram

                Spacer(Modifier.height(10.dp))
                Text(
                    if (directive == null && trainedToday) "TOMORROW" else "TODAY",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(5.dp))
                Text(
                    headline,
                    style = MaterialTheme.typography.headlineLarge.copy(
                        fontFamily = FontFamily.SansSerif,
                        fontWeight = FontWeight.SemiBold,
                        letterSpacing = (-0.5).sp
                    ),
                    color = MaterialTheme.colorScheme.onBackground,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    reason,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                directive?.secondary?.let { secondary ->
                    Spacer(Modifier.height(3.dp))
                    Text(
                        secondary,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                state.brief?.targets
                    ?.filter { it.targetWeightLb != null }
                    ?.take(2)
                    ?.takeIf { it.isNotEmpty() }
                    ?.let { targets ->
                        Spacer(Modifier.height(10.dp))
                        targets.forEach { target ->
                            Text(
                                "${target.name} · ${target.setsText}×${target.repsText} @ " +
                                    com.forge.app.domain.units.formatWeight(
                                        target.targetWeightLb!!,
                                        LocalForgeSettings.current.weightUnit
                                    ),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onBackground,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                    }

                state.coldStartLesson?.let { lesson ->
                    Spacer(Modifier.height(6.dp))
                    Box(
                        modifier = Modifier
                            .heightIn(min = 48.dp)
                            .clickableLabeled("Open the lesson: ${lesson.title}", onClick = onOpenAcademy)
                            .padding(end = 12.dp),
                        contentAlignment = Alignment.CenterStart
                    ) {
                        Text(
                            "Read ${lesson.title}",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }
                }

                Spacer(Modifier.height(18.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    HomePrimaryAction(
                        text = actionText,
                        label = actionText,
                        modifier = Modifier.weight(1f),
                        onLongClick = if (startsNewGymSession) {
                            { onStartSessionSkipWarmup(sessionDayKey) }
                        } else null,
                        onClick = action
                    )
                    if (!programEmpty) {
                        HomePlanAction("Plan", planAction)
                    }
                }
                if (startsNewGymSession) {
                    Spacer(Modifier.height(5.dp))
                    Text(
                        "Hold start to skip warmup",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                Spacer(Modifier.height(24.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        "This week",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onBackground
                    )
                    Text(
                        if (!freestyleMode && !programEmpty) {
                            "${state.workoutsThisWeek} / ${state.weeklyWorkoutTarget} target"
                        } else {
                            "${state.workoutsThisWeek} workouts"
                        },
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Spacer(Modifier.height(12.dp))
                WeekStrip(
                    trained = state.weekDaysTrained,
                    todayIndex = todayDow,
                    dayLabels = WEEK_INITIALS,
                    reading = "This week, ${state.weekDaysTrained.size} of 7 days trained",
                    modifier = Modifier.fillMaxWidth()
                )
                Spacer(Modifier.height(10.dp))

                val weeklyFacts = buildList {
                    add("${state.workoutsThisWeek} workout${if (state.workoutsThisWeek == 1) "" else "s"}")
                    if (state.volumeThisWeekLb > 0) {
                        add(formatVolume(state.volumeThisWeekLb, LocalForgeSettings.current.weightUnit))
                    }
                    if (state.cardioMinutesThisWeek > 0) add("${state.cardioMinutesThisWeek} cardio min")
                }
                Text(
                    weeklyFacts.joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                movement?.let { currentMovement ->
                    Spacer(Modifier.height(14.dp))
                    MovementLine(currentMovement, onBg = onBg, muted = muted, outline = outline, accent = accent)
                }

                Spacer(Modifier.height(32.dp))
                val goals = pinnedGoals(state, settings.pinnedGoalKeys)
                SectionAnchor(
                    label = "Goals",
                    muted = muted,
                    onBg = onBg,
                    action = if (goals.isEmpty()) null else "view all",
                    actionLabel = "Open goals",
                    onAction = onOpenGoals
                )
                Spacer(Modifier.height(14.dp))
                if (goals.isEmpty()) {
                    PromptLine("Pin a goal", "Pin a goal", onOpenGoals)
                } else {
                    goals.forEachIndexed { index, goal ->
                        GoalProgressLine(
                            title = goal.title,
                            valueLine = goal.valueLine,
                            fraction = goal.fraction,
                            achieved = goal.achieved,
                            index = index,
                            onBg = onBg,
                            muted = muted,
                            accent = accent,
                            outline = outline,
                            icon = goal.icon,
                            onClick = onOpenGoals
                        )
                        if (index != goals.lastIndex) Spacer(Modifier.height(18.dp))
                    }
                }

                Spacer(Modifier.height(32.dp))
                SectionAnchor(
                    label = "Recent",
                    muted = muted,
                    onBg = onBg,
                    action = if (state.recentItems.isEmpty()) null else "view all",
                    actionLabel = "View all sessions",
                    onAction = onViewAllHistory
                )
                Spacer(Modifier.height(8.dp))
                if (state.recentItems.isEmpty()) {
                    Text(
                        when {
                            freestyleMode -> "Your first logged workout lands here."
                            programEmpty -> "Build a plan above, then your first session lands here."
                            else -> "Your first session lands here."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted.copy(alpha = 0.65f)
                    )
                } else {
                    state.recentItems.take(HOME_LIST_CAP).forEach { item ->
                        RecentRow(
                            item = item,
                            palette = palette,
                            onBg = onBg,
                            weightUnit = settings.weightUnit,
                            useMiles = settings.useMiles
                        ) {
                            if (item.isGym) onOpenSession(item.id) else viewModel.selectRecentItem(item)
                        }
                    }
                }

                state.onThisDayMemory?.let { memory ->
                    Spacer(Modifier.height(18.dp))
                    OnThisDayCard(memory = memory, onBg = onBg, muted = muted, accent = accent, outline = outline)
                }

                Spacer(Modifier.height(28.dp))
            }
        }
    }
}

/**
 * "On this day" nostalgia card (#106) — surfaces a session from 1/3/6/12 months ago that lands near
 * today's date. Display-only; tapping through to the session is a separate roadmap item.
 */
@Composable
private fun OnThisDayCard(
    memory: OnThisDayMemory,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val agoLabel = com.forge.app.ui.common.monthsAgoPhrase(memory.monthsAgo).uppercase()
    val vol = com.forge.app.domain.units.formatVolumeCompact(memory.totalVolumeLb, weightUnit)
    val prText = if (memory.prCount > 0) " · ${memory.prCount} PR${if (memory.prCount > 1) "s" else ""}" else ""
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
            .padding(horizontal = 14.dp, vertical = 12.dp)
    ) {
        Text("ON THIS DAY · $agoLabel", style = MaterialTheme.typography.labelSmall,
            color = muted, fontSize = 10.sp)
        Spacer(Modifier.height(2.dp))
        Text("You trained ${memory.dayName} · $vol moved$prText",
            style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}

/**
 * The Home movement line (W6): today's watch steps against a typical day (14-day median), the
 * quiet daily-movement read between sessions. One thin bar (fill `primary` on an outline track,
 * §5) + one mono reading — honest at zero, hidden entirely when steps aren't connected.
 */
@Composable
private fun MovementLine(
    movement: OverviewViewModel.TodayMovement,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val reading = buildString {
        append("%,d".format(movement.steps))
        append(" STEPS")
        movement.typicalSteps?.let { append(" · TYPICAL ~%,d".format(it)) }
    }
    Column(Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("MOVEMENT", style = MaterialTheme.typography.labelMedium, color = muted)
            Text(
                reading,
                style = MaterialTheme.typography.labelSmall,
                color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp
            )
        }
        Spacer(Modifier.height(8.dp))
        // Today against the typical-day mark; with no baseline yet the bar shows today against
        // itself (full when any steps, empty at zero) — a real reading either way, never fake.
        val target = movement.typicalSteps ?: movement.steps
        val fraction = if (target <= 0) 0f else (movement.steps.toFloat() / target).coerceIn(0f, 1f)
        Box(
            Modifier
                .fillMaxWidth()
                .height(4.dp)
                .background(outline.copy(alpha = 0.3f), RoundedCornerShape(2.dp))
        ) {
            if (fraction > 0f) {
                Box(
                    Modifier
                        .fillMaxWidth(fraction)
                        .height(4.dp)
                        .background(accent, RoundedCornerShape(2.dp))
                )
            }
        }
    }
}
