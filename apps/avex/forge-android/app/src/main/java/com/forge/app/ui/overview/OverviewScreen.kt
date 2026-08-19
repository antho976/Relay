package com.forge.app.ui.overview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.program.Program
import com.forge.app.ui.common.NotificationBell
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.monthsAgoPhrase
import com.forge.app.ui.common.statsEntrance
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
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.overview.components.HomeHero
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * # Home — the action-first rebuild (2026-08-16)
 *
 * ## What Antho asked for, in his words
 *
 * > "What do I do now, really small summary of important info that changed, and that's it."
 *
 * Plus: keep RECENT, and keep GOALS but make them readable at a glance. Everything else went.
 *
 * ## What that removed, and where it lives now
 *
 * The page carried seven sections to report one push session, and repeated its own volume figure
 * twice. Cut from Home: the WORKOUTS tile (absorbed by [WeekStrip], which says the same thing
 * legibly), the CARDIO tile (Cardio tab), VOLUME THIS WEEK (Stats), TODAY'S TARGETS (folded into the
 * hero's whisper line), the Academy read strip (Academy tab), MOVEMENT (Stats), ON THIS DAY, the
 * coach cards and the fatigue nudge (Coach tab; their signal survives compressed into the one
 * changed-line below the week), TROPHIES, and the "Nutrition · soon" footer — a page about absence
 * should not close on a promise of more absence.
 *
 * The test each survivor had to pass: **is something at stake?** A count of finished workouts reports
 * something already settled, which is why it could be read as "bland" with nothing wrong in it.
 *
 * ## Why there are almost no boxes left
 *
 * > "Boxes but not too seeable."
 *
 * The old page was five stacked grey rectangles at one fill value, which is the grammar of a settings
 * page. There are now none: the hero, the week, the goals and the recent rows all sit directly on the
 * warm ground, and the only fills left belong to things you can press.
 *
 * A warm accent bloom behind the hero was tried and cut the same day (Antho: "remove the accented
 * glow on the background"). It bought atmosphere without an asset, but a tinted wash across the top of
 * a page is decorative colour, and this design spends its accent on decisions only.
 *
 * ## The colour budget
 *
 * Ember appears in FOUR places and nowhere else: the CTA (large), today's cell in the week strip, a
 * reached goal, and the changed-line glyph. The old page scattered a muted navy across six
 * postage-stamp marks where it read as a dead pixel. Small in count, large in size.
 *
 * The CTA is the only one of the four that can go quiet: on a rest day it drops to an outline, because
 * a filled ember capsule is this app's single "do this now" signal (see [heroPlan]).
 */

/** The page gutter. */
private val GUTTER = 24.dp

/** Half the slack between a top-bar icon's 44dp target and its 20dp glyph — see the chrome row. */
private val GUTTER_SLACK = 12.dp

/** Home shows at most three of each list. Beyond that the section's `view all →` carries the rest. */
private const val HOME_LIST_CAP = 3

@Composable
fun OverviewScreen(
    onStartSession: (dayKey: String) -> Unit,
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
    val programEmpty by viewModel.programEmpty.collectAsStateWithLifecycle()
    val selectedItem by viewModel.selectedItem.collectAsStateWithLifecycle()
    val summaryLines by viewModel.sessionExerciseLines.collectAsStateWithLifecycle()

    // Keep the directive current across a day of glances: one that has gone stale past midnight
    // should be right on return.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                viewModel.refreshDirective()
                viewModel.refreshIdentity()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { viewModel.onMilestoneShown(it.id) }
    }

    val settings = LocalForgeSettings.current
    val weightUnit = settings.weightUnit
    val palette = surfacePalette()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    val today = LocalDate.now()
    val todayDow = today.dayOfWeek.value - 1
    val nextDay = Program.days.firstOrNull { it.key == state.nextUpDayKey }
    val resumeKey = state.activeSessionDayKey

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

    Scaffold(containerColor = Color.Transparent) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
        ) {
                Spacer(Modifier.height(8.dp))

                // ── Chrome ───────────────────────────────────────────────────────────
                // The name came off this row (2026-08-16). The hero's eyebrow already carries the
                // day and the hero itself carries the answer; a title above both was a third voice
                // competing for the top of the page. What is left is the bell and Profile, and an
                // empty start edge for the light to fall on.
                ChromeRow(
                    palette = palette,
                    muted = muted,
                    onOpenProfile = onOpenProfile,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(0)
                )

                Spacer(Modifier.height(18.dp))

                // ── 1. What do I do now ──────────────────────────────────────────────
                val hero = heroPlan(
                    state = state,
                    nextDay = nextDay,
                    freestyleMode = freestyleMode,
                    programEmpty = programEmpty,
                    resumeKey = resumeKey
                )

                HomeHero(
                    eyebrow = greetingNow(),
                    dateLabel = today.format(DateTimeFormatter.ofPattern("EEE d MMM")).uppercase(),
                    headline = hero.headline,
                    whisper = hero.whisper,
                    ctaText = hero.ctaText,
                    ctaLabel = hero.ctaLabel,
                    ctaFilled = hero.ctaFilled,
                    ctaHint = if (hero.allowSkipWarmup) "Hold to skip warmup" else null,
                    onLongCta = if (hero.allowSkipWarmup) {
                        { onStartSessionSkipWarmup(state.nextUpDayKey) }
                    } else null,
                    longCtaLabel = "Start, skipping warmup",
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(1)
                ) {
                    when (hero.action) {
                        HeroAction.BUILD_PLAN -> onBuildPlan()
                        HeroAction.LOG_FREESTYLE -> onLogFreestyle()
                        HeroAction.OPEN_CARDIO -> onGoToCardio()
                        HeroAction.OPEN_ACADEMY -> onOpenAcademy()
                        HeroAction.START_SESSION -> onStartSession(resumeKey ?: state.nextUpDayKey)
                    }
                }

                Spacer(Modifier.height(32.dp))

                // ── 2. The week, and the one thing that changed ───────────────────────
                // Plan-agnostic on purpose: it reads days trained, so a freestyle user with no
                // schedule gets exactly the same mark.
                WeekStrip(
                    trained = state.weekDaysTrained,
                    todayIndex = todayDow,
                    dayLabels = WEEK_INITIALS,
                    reading = "This week, ${state.weekDaysTrained.size} of 7 days trained",
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = GUTTER)
                        .clickableLabeled("View your program", onClick = onViewProgram)
                        .statsEntrance(2)
                )

                changedLine(state, weightUnit)?.let { line ->
                    Spacer(Modifier.height(22.dp))
                    ChangedLine(
                        text = line,
                        onBg = onBg,
                        modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(3)
                    )
                }

                // ── 3. Goals at a glance ─────────────────────────────────────────────
                Spacer(Modifier.height(32.dp))
                val goals = pinnedGoals(state, settings.pinnedGoalKeys)
                SectionAnchor(
                    label = "Goals",
                    muted = muted,
                    onBg = onBg,
                    action = if (goals.isEmpty()) null else "view all",
                    actionLabel = "Open goals",
                    onAction = onOpenGoals,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(4)
                )
                Spacer(Modifier.height(14.dp))
                if (goals.isEmpty()) {
                    // No ghost cards. An absent goal is an absent row, so zero is one tappable line
                    // rather than three empty rectangles that read as content failing to load.
                    PromptLine(
                        text = "Pin a goal",
                        label = "Pin a goal",
                        onClick = onOpenGoals,
                        modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(5)
                    )
                } else {
                    goals.forEachIndexed { i, goal ->
                        // The Goals screen's own row, not a Home-only fork: the preview and the full
                        // list have to read as one surface.
                        GoalProgressLine(
                            title = goal.title,
                            valueLine = goal.valueLine,
                            fraction = goal.fraction,
                            achieved = goal.achieved,
                            index = i,
                            onBg = onBg,
                            muted = muted,
                            accent = MaterialTheme.colorScheme.primary,
                            outline = MaterialTheme.colorScheme.outline,
                            modifier = Modifier
                                .padding(horizontal = GUTTER)
                                .statsEntrance(5 + i),
                            icon = goal.icon,
                            onClick = onOpenGoals
                        )
                        if (i != goals.lastIndex) Spacer(Modifier.height(18.dp))
                    }
                }

                // ── 4. Recent ────────────────────────────────────────────────────────
                Spacer(Modifier.height(32.dp))
                SectionAnchor(
                    label = "Recent",
                    muted = muted,
                    onBg = onBg,
                    action = if (state.recentItems.isEmpty()) null else "view all",
                    actionLabel = "View all sessions",
                    onAction = onViewAllHistory,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(8)
                )
                Spacer(Modifier.height(8.dp))
                if (state.recentItems.isEmpty()) {
                    // The old zero-state was a grey box holding a ghost bar rail — a loading
                    // skeleton that never resolves, on a screen where the local DB is instant and
                    // nothing ever loads. One quiet line is honest; bars-where-data-goes are not.
                    Text(
                        when {
                            freestyleMode -> "Your first logged workout lands here."
                            programEmpty -> "Build a plan above, then your first session lands here."
                            else -> "Your first session lands here."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted.copy(alpha = 0.65f),
                        modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(9)
                    )
                } else {
                    state.recentItems.take(HOME_LIST_CAP).forEachIndexed { i, item ->
                        RecentRow(
                            item = item,
                            palette = palette,
                            onBg = onBg,
                            weightUnit = weightUnit,
                            useMiles = settings.useMiles,
                            modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(9 + i),
                            onClick = {
                                if (item.isGym) onOpenSession(item.id)
                                else viewModel.selectRecentItem(item)
                            }
                        )
                    }
                }

            Spacer(Modifier.height(40.dp))
        }
    }
}

// ── Sections ──────────────────────────────────────────────────────────────────────────────────

/** Mon-first day initials for the week strip. */
private val WEEK_INITIALS = listOf("M", "T", "W", "T", "F", "S", "S")

/**
 * The chrome row: the bell and Profile, end-aligned, nothing else.
 *
 * The bell stays Home-only and the screen still does not print its own name.
 */
@Composable
private fun ChromeRow(
    palette: SurfacePalette,
    muted: Color,
    onOpenProfile: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically
    ) {
        NotificationBell()
        Spacer(Modifier.width(4.dp))
        Box(
            Modifier
                .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
                .offset(x = GUTTER_SLACK)
                .clickableLabeled("Profile", onClick = onOpenProfile),
            contentAlignment = Alignment.Center
        ) {
            Box(
                Modifier.size(36.dp).clip(CircleShape).background(palette.card),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    NavIcons.Profile,
                    contentDescription = null,
                    tint = muted,
                    modifier = Modifier.size(18.dp)
                )
            }
        }
    }
}

/**
 * The "really small summary of important info that changed" — one line, no header, no box.
 *
 * The accent glyph flags it as the exception on the page; the words stay on `onBg` so meaning never
 * rides on colour alone.
 */
@Composable
private fun ChangedLine(text: String, onBg: Color, modifier: Modifier = Modifier) {
    // Baseline-aligned, not top-aligned: a 13sp mono glyph beside 14sp sans sits visibly high on a
    // shared top edge, and the two read as one line only when they share a baseline.
    Row(modifier.fillMaxWidth()) {
        Text(
            "↑",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.alignByBaseline()
        )
        Spacer(Modifier.width(10.dp))
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = onBg,
            modifier = Modifier.alignByBaseline()
        )
    }
}

/** A zero-state that is a row, not a card: an accent `+` and the thing to do. */
@Composable
private fun PromptLine(
    text: String,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier
            .fillMaxWidth()
            .clickableLabeled(label, onClick = onClick)
            // Padding, not a fixed height: the 48dp target comes from the padding so the row still
            // grows with the font scale instead of clipping at 200%.
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

/** One RECENT row, bare on the page: leading glyph · title + date · figure + delta. */
@Composable
private fun RecentRow(
    item: OverviewRecentItem,
    palette: SurfacePalette,
    onBg: Color,
    weightUnit: com.forge.app.domain.units.WeightUnit,
    useMiles: Boolean,
    modifier: Modifier = Modifier,
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

    Box(modifier) {
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
}

// ── The hero's decision ───────────────────────────────────────────────────────────────────────

private enum class HeroAction { START_SESSION, BUILD_PLAN, LOG_FREESTYLE, OPEN_CARDIO, OPEN_ACADEMY }

private data class HeroPlan(
    val headline: String,
    val whisper: String?,
    val ctaText: String,
    val ctaLabel: String,
    val ctaFilled: Boolean,
    val action: HeroAction,
    val allowSkipWarmup: Boolean
)

/**
 * What the hero says and what its button does — the whole decision in one place.
 *
 * ## The bug this fixes
 *
 * The button used to branch on PLAN MODE (freestyle / no program / otherwise) and never looked at
 * the coach's verdict, so on a day the coach had already called it would render "Done for today"
 * over a filled "Start session →" (Antho, 2026-08-16). The page was contradicting itself in the two
 * largest elements on it.
 *
 * ## The rule
 *
 * The directive's [TodayDirective.Kind] decides, because the directive IS the day's answer. Resume
 * and no-program outrank it (an unfinished session and an absent plan are facts about the app, not
 * opinions about today); everything else follows the coach.
 *
 * On REST the action survives but goes OUTLINED. Removing it entirely would be wrong — you are
 * allowed to train on a rest day, and a page whose one control vanishes reads as broken — but a
 * filled ember capsule is the app's single "do this now" signal and a rest day has no such thing.
 * "Train anyway" is also the honest wording: it gives you the agency without the app pretending it
 * recommended this.
 *
 * No "view today's session" action on a Done day: the RECENT section directly below already leads
 * with that session, tagged TODAY, one tap away. A fact gets one home on a screen.
 */
private fun heroPlan(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    nextDay: com.forge.app.program.DayPlan?,
    freestyleMode: Boolean,
    programEmpty: Boolean,
    resumeKey: String?
): HeroPlan {
    val directive = state.directive
    val kind = directive?.kind
    val headline = when {
        programEmpty && !freestyleMode -> "No plan yet"
        directive != null -> directive.headline
        freestyleMode -> "Free session"
        else -> state.customDayName ?: nextDay?.defaultName ?: "Ready"
    }

    // An unfinished session outranks every opinion: it is a fact, and abandoning it silently is the
    // worst thing this screen could do.
    if (resumeKey != null) {
        return HeroPlan(
            headline = headline,
            whisper = "You have a session still open.",
            ctaText = "Resume session  →",
            ctaLabel = "Resume session",
            ctaFilled = true,
            action = HeroAction.START_SESSION,
            allowSkipWarmup = false
        )
    }
    if (programEmpty && !freestyleMode) {
        return HeroPlan(
            headline = headline,
            whisper = "Build your own plan and Avex will guide each session.",
            ctaText = "Build a plan  →",
            ctaLabel = "Build your plan",
            ctaFilled = true,
            action = HeroAction.BUILD_PLAN,
            allowSkipWarmup = false
        )
    }

    // The whisper names today's lifts only when today IS a training day. On a rest or recovery day
    // the reason the coach gave is the more useful line, and listing lifts you are being told not to
    // do is the same contradiction in miniature.
    val trainingWhisper = todaysLiftsLine(state, nextDay) ?: directive?.reason
    val restWhisper = directive?.reason

    return when (kind) {
        com.forge.app.domain.coach.TodayDirective.Kind.REST -> HeroPlan(
            headline = headline,
            whisper = restWhisper,
            ctaText = if (freestyleMode) "Log anyway  →" else "Train anyway  →",
            ctaLabel = if (freestyleMode) "Log a workout anyway" else "Train anyway",
            ctaFilled = false,
            action = if (freestyleMode) HeroAction.LOG_FREESTYLE else HeroAction.START_SESSION,
            allowSkipWarmup = false
        )
        com.forge.app.domain.coach.TodayDirective.Kind.CARDIO -> HeroPlan(
            headline = headline,
            // The directive's secondary slot is where the concrete suggestion lives ("a 20-minute
            // walk would serve recovery better"), so it beats the generic reason here.
            whisper = directive?.secondary ?: restWhisper,
            ctaText = "Log cardio  →",
            ctaLabel = "Open cardio",
            ctaFilled = true,
            action = HeroAction.OPEN_CARDIO,
            allowSkipWarmup = false
        )
        com.forge.app.domain.coach.TodayDirective.Kind.LEARN -> HeroPlan(
            headline = headline,
            whisper = restWhisper,
            ctaText = "Open Academy  →",
            ctaLabel = "Open the Academy",
            ctaFilled = true,
            action = HeroAction.OPEN_ACADEMY,
            allowSkipWarmup = false
        )
        // TRAIN, or no directive yet.
        else -> HeroPlan(
            headline = headline,
            whisper = if (freestyleMode) {
                directive?.reason ?: lastTrainedLine(state.recentItems)
                    ?: "Log whatever you trained, whenever you want."
            } else trainingWhisper,
            ctaText = if (freestyleMode) "Start logging  →" else "Start session  →",
            ctaLabel = if (freestyleMode) "Start logging" else "Start session",
            ctaFilled = true,
            action = if (freestyleMode) HeroAction.LOG_FREESTYLE else HeroAction.START_SESSION,
            allowSkipWarmup = !freestyleMode
        )
    }
}

// ── Derivations ───────────────────────────────────────────────────────────────────────────────

private data class GoalLineData(
    val key: String,
    val title: String,
    val valueLine: String,
    val fraction: Float,
    val achieved: Boolean,
    /** The leading mark, so a goal row and a RECENT row read as the same kind of object. */
    val icon: androidx.compose.ui.graphics.vector.ImageVector
)

/**
 * The three goals Home shows.
 *
 * Antho's call: the user pins them. [pinnedKeys] is that choice, in the order it was made. When
 * nothing is pinned yet the section falls back to closest-first so an existing user's goals do not
 * vanish the moment this ships — the fallback is capped at the same three, so the section never
 * changes shape between the two.
 */
@Composable
private fun pinnedGoals(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    pinnedKeys: List<String>
): List<GoalLineData> {
    val settings = LocalForgeSettings.current
    val lift = state.goals.map { g ->
        GoalLineData(
            key = liftPinKey(g.exerciseId),
            title = g.name,
            valueLine = "${com.forge.app.domain.units.weightInputValue(g.currentBestLb, settings.weightUnit)} / " +
                "${com.forge.app.domain.units.weightInputValue(g.targetLb, settings.weightUnit)} " +
                com.forge.app.domain.units.unitLabel(settings.weightUnit),
            fraction = g.fraction,
            achieved = g.achieved,
            icon = liftGoalGlyph(g.exerciseId)
        )
    }
    val custom = state.customGoals.map { g ->
        GoalLineData(
            key = customPinKey(g.id),
            title = customGoalTitle(g),
            valueLine = customGoalValueLine(g, settings.weightUnit, settings.useMiles),
            fraction = g.fraction,
            achieved = g.achieved,
            icon = goalGlyph(g.metric)
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

/**
 * Today's lifts as one line: "Bench Press, Incline Press, Dips".
 *
 * Prefers the coach's own per-exercise targets (what you will actually be asked to do today) and
 * falls back to the plan day's exercise list. Three names, because a fourth wraps on a 393dp screen
 * and the line stops being a whisper.
 */
private fun todaysLiftsLine(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    nextDay: com.forge.app.program.DayPlan?
): String? {
    val fromBrief = state.brief?.targets?.map { it.name }.orEmpty()
    val names = fromBrief.ifEmpty { nextDay?.exercises?.map { it.name }.orEmpty() }
    if (names.isEmpty()) return null
    val head = names.take(3).joinToString(", ")
    val rest = names.size - 3
    return if (rest > 0) "$head, +$rest more" else head
}

/** The freestyle hero's whisper: there is no plan to name, so it names what you last trained. */
private fun lastTrainedLine(recent: List<OverviewRecentItem>): String? {
    val last = recent.firstOrNull() ?: return null
    val day = last.dayLabel.takeIf { it.isNotBlank() }?.lowercase()
    return if (day != null) "Last: ${last.title}, $day" else "Last: ${last.title}"
}

/**
 * The one changed-line, in priority order.
 *
 * This is what is left of four cut sections. A PR, a best session, a streak, a memory and a fatigue
 * build were each a card or a strip of their own; they are all the same KIND of fact — something
 * moved since you last looked — so they share one line and the most important one wins. Null when
 * nothing has actually changed, and then the line does not render at all: a slot that always says
 * something says nothing.
 */
private fun changedLine(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    unit: com.forge.app.domain.units.WeightUnit
): String? {
    val last = state.recentItems.firstOrNull()
    return when {
        last != null && last.prCount > 0 ->
            "${last.prCount} PR${if (last.prCount > 1) "s" else ""} in your last session"
        last != null && last.isBest -> "Last session was your best ${last.title} yet"
        state.streakDays >= 2 -> "Streak at ${state.streakDays} days"
        state.onThisDayMemory != null -> state.onThisDayMemory.let { m ->
            "${monthsAgoPhrase(m.monthsAgo).replaceFirstChar { it.uppercase() }} today you trained " +
                "${m.dayName}, ${formatVolumeCompact(m.totalVolumeLb, unit)} moved"
        }
        state.coachFatigue != null -> state.coachFatigue.let { f ->
            "Fatigue ${f.score} of ${f.threshold}" + (f.topDriver?.let { ", $it" } ?: "") +
                ". Easing up helps."
        }
        else -> null
    }
}

/** Time-of-day greeting. Dry and factual — it names the part of the day, nothing more. */
private fun greetingNow(): String = when (java.time.LocalTime.now().hour) {
    in 0..4 -> "Late night"
    in 5..11 -> "Good morning"
    in 12..17 -> "Good afternoon"
    else -> "Good evening"
}
