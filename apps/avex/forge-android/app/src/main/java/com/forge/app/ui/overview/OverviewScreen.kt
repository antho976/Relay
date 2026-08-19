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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ChevronRight
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
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.Program
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
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

@Composable
private fun HomeSectionHeader(title: String, action: String? = null, onAction: (() -> Unit)? = null) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onBackground
        )
        if (action != null && onAction != null) {
            Box(
                modifier = Modifier
                    .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                    .clickableLabeled(action, onClick = onAction)
                    .padding(horizontal = 8.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    action,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun HomeWeekStrip(trainedDays: Set<Int>, todayIndex: Int) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        listOf("M", "T", "W", "T", "F", "S", "S").forEachIndexed { index, day ->
            val trained = index in trainedDays
            val today = index == todayIndex
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    day,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (today) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
                )
                Box(
                    modifier = Modifier
                        .size(34.dp)
                        .clip(CircleShape)
                        .then(
                            when {
                                trained -> Modifier.background(MaterialTheme.colorScheme.primary)
                                today -> Modifier.border(1.5.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                else -> Modifier.border(1.dp, MaterialTheme.colorScheme.outline, CircleShape)
                            }
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    if (trained) {
                        Icon(
                            Icons.Default.Check,
                            contentDescription = "$day trained",
                            tint = MaterialTheme.colorScheme.onPrimary,
                            modifier = Modifier.size(18.dp)
                        )
                    } else if (today) {
                        Box(Modifier.size(5.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
                    }
                }
            }
        }
    }
}

@Composable
private fun CompactRecentRow(item: OverviewRecentItem, onClick: () -> Unit) {
    val settings = LocalForgeSettings.current
    val metric = when {
        item.isGym && item.volumeLb != null && item.volumeLb > 0 -> formatVolume(item.volumeLb, settings.weightUnit)
        !item.isGym && item.distanceKm != null && item.distanceKm > 0 -> formatDistance(item.distanceKm, settings.useMiles)
        item.durationMin != null && item.durationMin > 0 -> "${item.durationMin} min"
        else -> ""
    }
    val detail = listOfNotNull(item.dayLabel.takeIf { it.isNotBlank() }, item.topLift ?: item.subtitle.takeIf { it.isNotBlank() })
        .joinToString(" · ")

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 54.dp)
            .bounceCombinedClick(onClickLabel = "Open ${item.title}", onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                item.title,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onBackground,
                overflow = TextOverflow.Ellipsis
            )
            if (detail.isNotBlank()) {
                Text(
                    detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
        if (metric.isNotBlank()) {
            Spacer(Modifier.width(12.dp))
            Text(
                metric,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Spacer(Modifier.width(8.dp))
        Icon(
            Icons.Default.ChevronRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(20.dp)
        )
    }
}

@Composable
private fun HomeGoalPreview(
    title: String?,
    valueLine: String?,
    fraction: Float,
    onClick: () -> Unit
) {
    HomeSectionHeader(
        title = "Goals",
        action = if (title != null) "View all" else null,
        onAction = if (title != null) onClick else null
    )
    Spacer(Modifier.height(4.dp))
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .bounceCombinedClick(onClickLabel = "Open goals", onClick = onClick)
            .padding(vertical = 8.dp),
        verticalArrangement = Arrangement.Center
    ) {
        if (title == null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "Set targets, track your lifts",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground
                )
                Text(
                    "→",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.primary
                )
            }
            return@Column
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                title,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.weight(1f)
            )
            valueLine?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        Box(
            Modifier
                .fillMaxWidth()
                .height(4.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(MaterialTheme.colorScheme.outline)
        ) {
            Box(
                Modifier
                    .fillMaxWidth(fraction.coerceIn(0f, 1f))
                    .height(4.dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(MaterialTheme.colorScheme.primary)
            )
        }
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
                    TopBarIconButton(
                        NavIcons.Profile,
                        "Profile",
                        MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.offset(x = GUTTER_SLACK),
                        onClick = onOpenProfile
                    )
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
                HomeWeekStrip(state.weekDaysTrained, todayDow)
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

                Spacer(Modifier.height(24.dp))
                val liftGoal = state.goals.firstOrNull()
                val customGoal = if (liftGoal == null) state.customGoals.firstOrNull() else null
                val goalTitle = liftGoal?.name ?: customGoal?.let(::customGoalTitle)
                val goalValue = when {
                    liftGoal != null -> {
                        val unit = LocalForgeSettings.current.weightUnit
                        "${weightInputValue(liftGoal.currentBestLb, unit)} / " +
                            "${weightInputValue(liftGoal.targetLb, unit)} ${unitLabel(unit)}"
                    }
                    customGoal != null -> customGoalValueLine(
                        customGoal,
                        LocalForgeSettings.current.weightUnit,
                        LocalForgeSettings.current.useMiles
                    )
                    else -> null
                }
                HomeGoalPreview(
                    title = goalTitle,
                    valueLine = goalValue,
                    fraction = liftGoal?.fraction ?: customGoal?.fraction ?: 0f,
                    onClick = onOpenGoals
                )

                Spacer(Modifier.height(24.dp))
                HomeSectionHeader("Recent workouts", "View all", onViewAllHistory)
                Spacer(Modifier.height(4.dp))
                if (state.recentItems.isEmpty()) {
                    InlineEmptyHint(
                        when {
                            freestyleMode -> "No workouts yet. Log one above and it will show here."
                            programEmpty -> "No workouts yet. Build your plan, then start training."
                            else -> "No workouts yet. Start a session and it will show here."
                        },
                        color = muted
                    )
                } else {
                    state.recentItems.forEachIndexed { index, item ->
                        CompactRecentRow(item) {
                            if (item.isGym) onOpenSession(item.id) else viewModel.selectRecentItem(item)
                        }
                        if (index < state.recentItems.lastIndex) {
                            Box(
                                Modifier
                                    .fillMaxWidth()
                                    .height(1.dp)
                                    .background(MaterialTheme.colorScheme.outline)
                            )
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
