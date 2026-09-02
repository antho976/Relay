package com.forge.app.ui.overview

import androidx.compose.animation.core.Animatable
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
import androidx.compose.ui.text.font.FontStyle
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.common.NotificationBell
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.overview.components.HeroHeadline
import com.forge.app.ui.overview.state.OverviewRecentItem
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.formatVolume
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.Program
import com.forge.app.ui.common.ForgeHeroAction
import com.forge.app.ui.experiment.CellShape
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.experiment.SurfaceListRow
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.WeekStrip
import com.forge.app.ui.experiment.surfacePalette
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.goals.goalCaption
import com.forge.app.ui.goals.customPinKey
import com.forge.app.ui.goals.goalGlyph
import com.forge.app.ui.goals.liftGoalGlyph
import com.forge.app.ui.goals.liftPinKey
import java.time.LocalDate

/**
 * Home's buttons take the page's own corner, not the app's pill.
 *
 * They were `RoundedCornerShape(14.dp)`, a radius that exists nowhere in `Shape.kt` (§7 allows
 * 4/8/12/16/24 and `RoundedCornerShape(50)`) — so they matched neither the app nor the page. Pills
 * were the first correction and the wrong one: Home states a geometry of its own and states it
 * twice, in `CellShape`'s rounded-square week cells and in the RECENT rows' 10dp leading marks
 * ("a squared cell ... lines up with the rounded squares the RECENT rows' leading marks already
 * use, so the page shares one geometry" — `SurfaceKit`). The CTA row sits directly above the week
 * strip, so a pill there put the one shape that disagreed with the page immediately above the
 * element that defines it (Antho, 2026-08-22).
 *
 * This is [CellShape] itself rather than a matching literal, so the two can never drift apart.
 * The cost is real and accepted: Home's capsules are now the only non-pill buttons in the app.
 * 56dp is `SurfaceCta`'s height.
 */
private val HomeCapsuleShape = CellShape

@Composable
private fun HomePrimaryAction(
    text: String,
    label: String,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit
) {
    // The drawing moved to `ui/common` when Cardio needed the same button (§2⑥) — this stays as the
    // name Home's call site reads by, and as the home of the skip-warmup hold label.
    ForgeHeroAction(
        text = text,
        onClick = onClick,
        modifier = modifier,
        label = label,
        onLongClick = onLongClick,
        onLongClickLabel = if (onLongClick != null) "Start, skipping warmup" else null
    )
}

@Composable
private fun HomePlanAction(text: String, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .heightIn(min = 56.dp)
            .clip(HomeCapsuleShape)
            .border(1.dp, MaterialTheme.colorScheme.outline, HomeCapsuleShape)
            // Bounce, not a bare clickable: this capsule sits shoulder to shoulder with the primary
            // and a control that does not answer the press reads as the disabled one (§9).
            .bounceCombinedClick(onClickLabel = text, onClick = onClick)
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
            .bounceCombinedClick(onClickLabel = label, onClick = onClick)
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
    // A custom cardio activity's name lives in the user's definitions, provided at the nav root;
    // the mapper only had its storage code.
    val title = item.displayTitle(com.forge.app.ui.cardio.LocalCardioTypes.current)

    SurfaceListRow(
        icon = if (item.isGym) SettingsIcons.Session else NavIcons.Cardio,
        hue = palette.hues[0],
        label = title,
        sub = sub,
        figure = figure,
        delta = delta,
        deltaColor = deltaColor,
        onBg = onBg,
        muted = palette.mutedOnCard,
        onClick = onClick,
        clickLabel = "Open $title"
    )
}

private data class GoalLineData(
    val key: String,
    val title: String,
    val valueLine: String,
    val fraction: Float,
    val achieved: Boolean,
    val icon: ImageVector,
    /** The mono line under the meter — the clock on a period goal, the baseline on a cut. */
    val caption: String?
)

@Composable
private fun pinnedGoals(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    pinnedKeys: List<String>
): List<GoalLineData> {
    val settings = LocalForgeSettings.current
    // One timestamp for the whole trim, so three captions on one page can't disagree about the day
    // — keyed on the day itself as well as on the goals, because a period boundary changes what the
    // caption should say without necessarily changing any goal's numbers (M-32).
    val now = remember(state.customGoals, state.todayStartMs) { System.currentTimeMillis() }
    val lift = state.goals.map { goal ->
        GoalLineData(
            key = liftPinKey(goal.exerciseId),
            title = goal.name,
            valueLine = "${com.forge.app.domain.units.weightInputValue(goal.currentBestLb, settings.weightUnit)} / " +
                "${com.forge.app.domain.units.weightInputValue(goal.targetLb, settings.weightUnit)} " +
                com.forge.app.domain.units.unitLabel(settings.weightUnit),
            fraction = goal.fraction,
            achieved = goal.achieved,
            icon = liftGoalGlyph(goal.exerciseId),
            // A lift target has neither a window nor a baseline: it is done or it is not.
            caption = if (goal.achieved) "Reached" else null
        )
    }
    val custom = state.customGoals.map { goal ->
        GoalLineData(
            key = customPinKey(goal.id),
            title = customGoalTitle(goal),
            valueLine = customGoalValueLine(goal, settings.weightUnit, settings.useMiles),
            fraction = goal.fraction,
            achieved = goal.achieved,
            icon = goalGlyph(goal.metric),
            caption = goalCaption(
                achieved = goal.achieved,
                metric = goal.metric,
                period = goal.period,
                baselineValue = goal.baselineValue,
                weightUnit = settings.weightUnit,
                nowMs = now,
            )
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

/**
 * How far the profile and settings glyphs lean toward each other inside their own targets.
 *
 * The pair still measures 48dp per button, because that is the touch target and §14 does not
 * negotiate it — which also fixes the seam between them at 48dp apart and the whitespace between
 * two 20dp glyphs at 28dp. Moving the BOXES closer is therefore not available; moving the glyphs
 * within them is, and costs nothing, because a touch target has never been required to sit
 * concentric with the mark it answers for. Each glyph gives up 6dp of margin on its inner side and
 * keeps 8dp — you would have to tap nearly a glyph's width past one icon to reach the other.
 */
private val PAIR_LEAN = 6.dp

/**
 * Top-bar icon with a 48dp tappable area + spoken label, while the glyph stays visually small.
 *
 * [nudge] slides the glyph inside that area without moving the area — see [PAIR_LEAN].
 */
@Composable
private fun TopBarIconButton(
    icon: ImageVector,
    label: String,
    tint: Color,
    modifier: Modifier = Modifier,
    nudge: Dp = 0.dp,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
            .clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            icon, contentDescription = label, tint = tint,
            modifier = Modifier.size(20.dp).offset(x = nudge)
        )
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
            entry.animateTo(1f, animationSpec = ForgeMotion.enterTween(450))
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

    // The ground under Home is the APP's ground.
    //
    // `ForgeTheme` paints the Pearl gradient (`#17120E → #0A0806`, §5) behind every screen and
    // `HubScreen`'s Scaffold is deliberately transparent so each pager page sits on it. Home used
    // to copy the scheme to pure black and then paint that opaque, which made the one screen you
    // swipe through most the one screen whose ground changed under you: flat cold black on Home,
    // warm gradient one swipe either side. It also undid the 2026-08-16 warm repalette on the exact
    // page that repalette was for, and it flattened the AMOLED/Pearl distinction to nothing.
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
                // §7's page gutter. Home sat at 20 and Stats sits at 16, so the content rail
                // shifted under you as you swiped between two pages of the same pager.
                .padding(horizontal = 24.dp)
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
                // Profile + settings read as one cluster, so they are spaced like one (Antho,
                // 2026-08-24, twice: "make these two closer", then "a bit closer").
                //
                // Two moves, because the first one ran out. The slack offset used to be on SETTINGS
                // alone, to sit its glyph on the page's optical right rail rather than 14dp inside
                // it — correct for settings, and it dragged that button 14dp further from the icon
                // beside it, opening a 42dp hole. Giving the offset to the Row slides both onto the
                // rail at once and hands those 14dp back, which is as far as moving buttons goes:
                // 48dp targets sit 48dp apart, leaving 28dp between two 20dp glyphs.
                //
                // The rest comes from the glyphs leaning inside their own targets — see
                // [PAIR_LEAN]. The Row takes the lean back on the right so SETTINGS does not walk
                // off the rail to pay for it, which leaves the whole 12dp on the gap: 16dp between
                // the glyphs, split evenly either side of the seam.
                Row(
                    modifier = Modifier.offset(x = GUTTER_SLACK + PAIR_LEAN),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    TopBarIconButton(
                        NavIcons.Profile,
                        "Profile",
                        MaterialTheme.colorScheme.onSurfaceVariant,
                        nudge = PAIR_LEAN,
                        onClick = onOpenProfile
                    )
                    TopBarIconButton(
                        Icons.Default.Settings,
                        "Settings",
                        MaterialTheme.colorScheme.onSurfaceVariant,
                        nudge = -PAIR_LEAN,
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

            Spacer(Modifier.height(12.dp))
            Text(
                if (directive == null && trainedToday) "TOMORROW" else "TODAY",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(8.dp))
            // The serif rung, through the shared clamp. This was `headlineLarge` overridden to
            // SansSerif with a hand-set letterSpacing at the call site, so the page's biggest
            // element opted out of the three-voice system (§6) -- every other overview in the app
            // leads serif -- and out of the 1.3x hero clamp, which meant at 200% font scale it
            // pushed the CTA off the fold. Same size, correct voice, survives the biggest font.
            HeroHeadline(
                headline,
                MaterialTheme.colorScheme.onBackground,
                MaterialTheme.typography.headlineLarge
            )
            Spacer(Modifier.height(8.dp))
            Text(
                reason,
                style = MaterialTheme.typography.bodySmall,
                fontStyle = FontStyle.Italic,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            directive?.secondary?.let { secondary ->
                Spacer(Modifier.height(8.dp))
                Text(
                    secondary,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.height(20.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
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
                Spacer(Modifier.height(8.dp))
                Text(
                    "Hold start to skip warmup",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.height(28.dp))
            // The same anchor GOALS and RECENT use. This one was a sans `titleMedium` in sentence
            // case carrying its own right meta, so a three-section page had three section-header
            // treatments in three type voices -- exactly the kind of break you feel as "something
            // is off" without ever looking straight at it.
            SectionAnchor(
                label = "This week",
                muted = muted,
                onBg = onBg,
                // Days, not sessions. The strip below draws DAYS trained and the denominator is
                // the program's training-day count, but this line counted SESSIONS — so three
                // sessions logged on one Monday rendered "4 / 7 target" directly above a strip with
                // a single cell lit: the section's header disagreeing with the section's own mark,
                // in the same breath (Antho, 2026-08-24). Nothing in the page said which of the two
                // numbers to believe, which is why it read as wrong without reading as broken.
                // The session count is not restated anywhere on Home — a finished tally is settled,
                // and Home carries what is at stake (`design/SETTLED.md`, 2026-08-16).
                meta = run {
                    val days = state.weekDaysTrained.size
                    when {
                        !freestyleMode && !programEmpty -> "$days / ${state.weeklyTrainingDays} days"
                        days == 1 -> "1 day"
                        else -> "$days days"
                    }
                }
            )
            Spacer(Modifier.height(12.dp))
            WeekStrip(
                trained = state.weekDaysTrained,
                todayIndex = todayDow,
                dayLabels = WEEK_INITIALS,
                reading = "This week, ${state.weekDaysTrained.size} of 7 days trained",
                modifier = Modifier.fillMaxWidth()
            )

            movement?.let { currentMovement ->
                Spacer(Modifier.height(16.dp))
                MovementLine(currentMovement, onBg = onBg, muted = muted, outline = outline, accent = accent)
            }

            Spacer(Modifier.height(28.dp))
            val goals = pinnedGoals(state, settings.pinnedGoalKeys)
            SectionAnchor(
                label = "Goals",
                muted = muted,
                onBg = onBg,
                action = if (goals.isEmpty()) null else "view all",
                actionLabel = "Open goals",
                onAction = onOpenGoals
            )
            Spacer(Modifier.height(12.dp))
            if (goals.isEmpty()) {
                PromptLine("Pin a goal", "Pin a goal", onOpenGoals)
            } else {
                goals.forEachIndexed { index, goal ->
                    GoalProgressLine(
                        title = goal.title,
                        valueLine = goal.valueLine,
                        fraction = goal.fraction,
                        achieved = goal.achieved,
                        onBg = onBg,
                        muted = muted,
                        accent = accent,
                        outline = outline,
                        caption = goal.caption,
                        icon = goal.icon,
                        onClick = onOpenGoals
                    )
                    if (index != goals.lastIndex) Spacer(Modifier.height(16.dp))
                }
            }

            Spacer(Modifier.height(28.dp))
            SectionAnchor(
                label = "Recent",
                muted = muted,
                onBg = onBg,
                action = if (state.recentItems.isEmpty()) null else "view all",
                actionLabel = "View all sessions",
                onAction = onViewAllHistory
            )
            Spacer(Modifier.height(12.dp))
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

            Spacer(Modifier.height(28.dp))
        }
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
            // MOVEMENT is labelMedium (11) and its reading is labelSmall (10): §6 ranks two mono
            // labels by SIZE, which the scale already does. The 9sp + hand-set tracking on top of
            // labelSmall was a third, undeclared rung that also stopped following the font setting.
            Text(reading, style = MaterialTheme.typography.labelSmall, color = muted)
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
