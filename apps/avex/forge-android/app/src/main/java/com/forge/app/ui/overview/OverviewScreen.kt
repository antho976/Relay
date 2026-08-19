package com.forge.app.ui.overview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.itemsIndexed
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
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.Features
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.ui.common.NotificationBell
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.monthsAgoPhrase
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.experiment.CardLink
import com.forge.app.ui.experiment.PeekCardRow
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.experiment.SurfaceCard
import com.forge.app.ui.experiment.SurfaceListRow
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.surfacePalette
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.nav.NavIcons
import com.forge.app.ui.overview.components.HomeGoalCard
import com.forge.app.ui.overview.components.HomeGoalGhost
import com.forge.app.ui.overview.components.HomeHeroCard
import com.forge.app.ui.overview.components.HomeNoteCard
import com.forge.app.ui.overview.components.HomeRecentZero
import com.forge.app.ui.overview.components.HomeTileCard
import com.forge.app.ui.overview.components.HomeTrophiesCard
import com.forge.app.ui.overview.components.HomeVolumeCard
import com.forge.app.ui.overview.components.TileMeter
import com.forge.app.ui.overview.components.TileWeekRail
import com.forge.app.ui.overview.state.OverviewRecentItem
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import kotlin.math.roundToInt

/**
 * # Home — design/surface-experiment (2026-08-15)
 *
 * A card-led rebuild of the open-editorial Home, kept only to be looked at and judged. The
 * open-editorial original is at `.design-backups/editorial-2026-08/src/overview/`; one command
 * restores it.
 *
 * The rhythm follows the reference dashboard: header row → hero card → two-up tiles → a horizontal
 * strip that peeks → a list. What it deliberately breaks is recorded on the primitives it uses
 * (`ui/experiment/SurfaceKit.kt`) rather than repeated here.
 *
 * Two structural notes worth arguing with:
 *
 * 1. **The directive is no longer the serif hero.** A number is. See `HomeHeroCard`'s KDoc.
 * 2. **Sections still separate by air, not by lines** (§7 survives). What changed is that a
 *    section's CONTENT sits on a fill. The mono anchors and the 28dp breaks are untouched, which
 *    is why the page still scans as this app rather than as a generic dashboard.
 */

/** The page gutter (§7). Sections apply it themselves so the goals strip can break out full-bleed. */
private val GUTTER = 24.dp

/** Half the slack between a top-bar icon's 44dp target and its 20dp glyph — see the header row. */
private val GUTTER_SLACK = 12.dp

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
    val identity by viewModel.identity.collectAsStateWithLifecycle()

    // Keep the movement line, the directive and now the header avatar current across a day of
    // glances (W6): steps taken while away, a directive that has gone stale past midnight, and a
    // photo changed on Profile should all be right on return.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                viewModel.refreshMovement()
                viewModel.refreshDirective()
                viewModel.refreshIdentity()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    // Firing a milestone persists it AND queues it in the notifications feed behind the bell.
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
            Spacer(Modifier.height(16.dp))

            // ── 1. Header row: greeting · avatar · one action ─────────────────────
            HeaderRow(
                name = identity.name,
                palette = palette,
                onBg = onBg,
                muted = muted,
                onOpenProfile = onOpenProfile,
                modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(0)
            )

            Spacer(Modifier.height(20.dp))

            // ── 2. Hero card ─────────────────────────────────────────────────────
            val resumeKey = state.activeSessionDayKey
            val noPlan = freestyleMode || programEmpty

            HomeHeroCard(
                palette = palette,
                // The greeting moved off the header row into the eyebrow (Antho, 2026-08-15). It is
                // context for the day's answer, not a second identity line above the one the header
                // already carries.
                eyebrow = greetingNow(),
                dateLabel = today.format(DateTimeFormatter.ofPattern("EEE d MMM")),
                headline = when {
                    freestyleMode -> "Open workout"
                    programEmpty -> "No plan yet"
                    else -> state.directive?.headline
                        ?: state.customDayName ?: nextDay?.defaultName ?: "Ready"
                },
                reason = when {
                    freestyleMode -> "No fixed plan. Log whatever you trained, whenever you want."
                    programEmpty -> "Build your own plan and Avex will guide each session."
                    else -> state.directive?.reason
                        ?: nextDay?.let { "${it.subtitle} · ${it.exercises.size} exercises" }
                },
                secondary = if (noPlan) null else state.directive?.secondary,
                ctaText = when {
                    programEmpty && !freestyleMode -> "Build a plan →"
                    freestyleMode -> "Log a workout →"
                    resumeKey != null -> "Resume session →"
                    else -> "Start session →"
                },
                ctaLabel = when {
                    programEmpty && !freestyleMode -> "Build your plan"
                    freestyleMode -> "Log a workout"
                    resumeKey != null -> "Resume session"
                    else -> "Start session"
                },
                ctaHint = if (!noPlan && resumeKey == null) "Hold to skip warmup" else null,
                onLongCta = if (!noPlan && resumeKey == null) {
                    { onStartSessionSkipWarmup(state.nextUpDayKey) }
                } else null,
                longCtaLabel = "Start, skipping warmup",
                onBg = onBg,
                muted = muted,
                modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(1)
            ) {
                when {
                    programEmpty && !freestyleMode -> onBuildPlan()
                    freestyleMode -> onLogFreestyle()
                    else -> onStartSession(resumeKey ?: state.nextUpDayKey)
                }
            }

            // Today's targets + the cold-start lesson keep their place under the hero rather than
            // inside it: three target lines make the hero card tall enough to push the two-up row
            // off the fold, and §3 wants the primary action above it.
            if (!noPlan) {
                val targets = state.brief?.targets?.filter { it.targetWeightLb != null }?.take(3).orEmpty()
                if (targets.isNotEmpty() || state.coldStartLesson != null) {
                    Spacer(Modifier.height(10.dp))
                    SurfaceCard(
                        palette,
                        Modifier.padding(horizontal = GUTTER).statsEntrance(2)
                    ) {
                        if (targets.isNotEmpty()) {
                            Text(
                                "TODAY'S TARGETS",
                                style = MaterialTheme.typography.labelMedium,
                                color = muted
                            )
                            Spacer(Modifier.height(8.dp))
                            targets.forEach { t ->
                                Text(
                                    "${t.name} · ${t.setsText}×${t.repsText} @ " +
                                        com.forge.app.domain.units.formatWeight(t.targetWeightLb!!, weightUnit),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = palette.mutedOnCard,
                                    modifier = Modifier.padding(vertical = 3.dp)
                                )
                            }
                        }
                        state.coldStartLesson?.let { lesson ->
                            if (targets.isNotEmpty()) Spacer(Modifier.height(8.dp))
                            CardLink(
                                text = "Read: ${lesson.title}",
                                label = "Open the lesson: ${lesson.title}",
                                onBg = onBg,
                                onClick = onOpenAcademy
                            )
                        }
                    }
                }
            }

            Spacer(Modifier.height(10.dp))

            // ── 3. Two-up tile row ───────────────────────────────────────────────
            // IntrinsicSize.Min (not a fixed dp) equalises the two cards while still letting them
            // grow with the font scale — a fixed height here would be exactly §14's clipping trap.
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = GUTTER)
                    .height(IntrinsicSize.Min)
                    .statsEntrance(3),
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                val target = state.weeklyWorkoutTarget
                HomeTileCard(
                    palette = palette,
                    icon = SettingsIcons.Session,
                    hue = palette.hues[0],
                    label = "Workouts",
                    figure = "${state.workoutsThisWeek}",
                    suffix = if (target > 0) "OF $target" else null,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                    onClick = if (!noPlan) onViewProgram else null,
                    clickLabel = "View your program"
                ) {
                    TileWeekRail(
                        trained = state.weekDaysTrained,
                        todayIndex = todayDow,
                        hue = palette.hues[0],
                        muted = muted,
                        reading = "This week, ${state.workoutsThisWeek} of 7 days trained"
                    )
                }

                val cardioTarget = state.cardioWeeklyTargetMin
                HomeTileCard(
                    palette = palette,
                    icon = NavIcons.Cardio,
                    hue = palette.hues[1],
                    label = "Cardio",
                    figure = "${state.cardioMinutesThisWeek}",
                    suffix = if (cardioTarget > 0) "OF $cardioTarget MIN" else "MIN",
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                    onClick = onGoToCardio,
                    clickLabel = "Open cardio"
                ) {
                    TileMeter(
                        fraction = if (cardioTarget > 0) {
                            state.cardioMinutesThisWeek.toFloat() / cardioTarget
                        } else if (state.cardioMinutesThisWeek > 0) 1f else 0f,
                        hue = palette.hues[1],
                        muted = muted,
                        reading = if (cardioTarget > 0) {
                            "${state.cardioMinutesThisWeek} of $cardioTarget cardio minutes this week"
                        } else "${state.cardioMinutesThisWeek} cardio minutes this week"
                    )
                }
            }

            // ── Volume this week — a reading, not the headline ───────────────────
            // Rendered only once there is volume history. At a true zero the card would be a serif
            // "0" over a dead chart, alone on a surface: §12's honest zero belongs on a figure with
            // siblings, which is what the WORKOUTS and CARDIO tiles above already provide.
            val volumeNow = state.volumeThisWeekLb
            val volumePrev = state.volumeLastWeekLb
            val volumeSeries = state.weeklyVolumeSeriesLb.let { raw ->
                // The curve's last point IS the figure above it, so the two can never disagree on
                // screen even if the weekly-stats query and the session sweep ever diverge.
                if (raw.isEmpty()) raw else raw.toMutableList().apply { this[lastIndex] = volumeNow }
            }
            if (volumeSeries.any { it > 0.0 }) {
                Spacer(Modifier.height(10.dp))
                HomeVolumeCard(
                    palette = palette,
                    figure = formatVolumeCompact(volumeNow, weightUnit, withUnit = false),
                    unit = unitLabel(weightUnit).uppercase(),
                    // No comparable week yet → no badge. A fabricated "0%" would claim a comparison
                    // the data cannot make (§12: honest zeros, never fake readings).
                    deltaPercent = if (volumePrev > 0.0) {
                        (((volumeNow - volumePrev) / volumePrev) * 100).roundToInt()
                    } else null,
                    series = volumeSeries,
                    seriesReading = weeklySeriesReading(volumeSeries, weightUnit),
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = GUTTER).statsEntrance(4)
                )
            }

            // Movement (W6) — rendered only when steps are connected: honest zero when connected,
            // hidden when the grant was never given (§12 stale/denied).
            movement?.let { m ->
                Spacer(Modifier.height(10.dp))
                val target = m.typicalSteps ?: m.steps
                HomeTileCard(
                    palette = palette,
                    icon = NavIcons.Stats,
                    hue = palette.hues[2],
                    label = "Movement today",
                    figure = "%,d".format(m.steps),
                    suffix = m.typicalSteps?.let { "TYPICAL ~%,d".format(it) } ?: "STEPS",
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = GUTTER).statsEntrance(4)
                ) {
                    TileMeter(
                        fraction = if (target <= 0) 0f else m.steps.toFloat() / target,
                        hue = palette.hues[2],
                        muted = muted,
                        reading = m.typicalSteps?.let {
                            "${m.steps} steps today, typical day about $it"
                        } ?: "${m.steps} steps today"
                    )
                }
            }

            // ── 4. Horizontal section: goals, peeking the next card ──────────────
            Spacer(Modifier.height(28.dp))
            val goalCount = state.goals.size + state.customGoals.size
            SectionAnchor(
                label = "Goals",
                muted = muted,
                onBg = onBg,
                action = when {
                    goalCount == 0 -> null
                    goalCount > 3 -> "all $goalCount"
                    else -> "view all"
                },
                actionLabel = "Open goals",
                onAction = onOpenGoals,
                modifier = Modifier.padding(horizontal = GUTTER)
            )
            Spacer(Modifier.height(10.dp))
            GoalStrip(
                state = state,
                palette = palette,
                onBg = onBg,
                muted = muted,
                onOpenGoals = onOpenGoals,
                modifier = Modifier.statsEntrance(5)
            )

            // ── 5. List section: recent sessions ─────────────────────────────────
            Spacer(Modifier.height(28.dp))
            SectionAnchor(
                label = "Recent",
                muted = muted,
                onBg = onBg,
                action = if (state.recentItems.isEmpty()) null else "view all",
                actionLabel = "View all sessions",
                onAction = onViewAllHistory,
                modifier = Modifier.padding(horizontal = GUTTER)
            )
            Spacer(Modifier.height(10.dp))
            SurfaceCard(
                palette,
                Modifier.padding(horizontal = GUTTER).statsEntrance(6),
                padding = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
            ) {
                if (state.recentItems.isEmpty()) {
                    HomeRecentZero(
                        palette = palette,
                        todayIndex = todayDow,
                        hint = when {
                            freestyleMode -> "Your first logged workout lands here."
                            programEmpty -> "Build your plan above, then your first session lands here."
                            else -> "Your first session lands here."
                        },
                        muted = muted
                    )
                } else {
                    state.recentItems.forEach { item ->
                        RecentSurfaceRow(
                            item = item,
                            palette = palette,
                            onBg = onBg,
                            muted = muted,
                            weightUnit = weightUnit,
                            useMiles = settings.useMiles,
                            onClick = {
                                if (item.isGym) onOpenSession(item.id) else viewModel.selectRecentItem(item)
                            }
                        )
                    }
                }
            }

            // ON THIS DAY — a session from N months ago near today's date (#106).
            state.onThisDayMemory?.let { memory ->
                Spacer(Modifier.height(10.dp))
                val vol = formatVolumeCompact(memory.totalVolumeLb, weightUnit)
                val prs = if (memory.prCount > 0) {
                    " · ${memory.prCount} PR${if (memory.prCount > 1) "s" else ""}"
                } else ""
                HomeNoteCard(
                    palette = palette,
                    eyebrow = "On this day · ${monthsAgoPhrase(memory.monthsAgo)}",
                    body = "You trained ${memory.dayName} · $vol moved$prs",
                    onBg = onBg,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(7)
                )
            }

            // ── Coach ────────────────────────────────────────────────────────────
            if (coachEnabled && !freestyleMode && state.coach.isNotEmpty()) {
                Spacer(Modifier.height(28.dp))
                SectionAnchor("Coach", muted, onBg, modifier = Modifier.padding(horizontal = GUTTER))
                Spacer(Modifier.height(10.dp))
                state.coach.forEachIndexed { index, item ->
                    SurfaceCard(
                        palette,
                        Modifier.padding(horizontal = GUTTER).statsEntrance(8 + index)
                    ) {
                        Text(item.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Spacer(Modifier.height(4.dp))
                        Text(item.body, style = MaterialTheme.typography.bodySmall, color = palette.mutedOnCard)
                        Spacer(Modifier.height(8.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                            item.applyLabel?.let { label ->
                                CardLink(label, "Apply this suggestion", onBg) { viewModel.applyCoach(item) }
                            }
                            Text(
                                "dismiss",
                                style = MaterialTheme.typography.labelMedium,
                                color = palette.mutedOnCard,
                                modifier = Modifier
                                    .clickableLabeled("Dismiss this suggestion") { viewModel.dismissCoach(item) }
                                    .padding(vertical = 4.dp)
                            )
                        }
                    }
                    if (index != state.coach.lastIndex) Spacer(Modifier.height(10.dp))
                }
            }

            if (coachEnabled && !freestyleMode) {
                state.coachFatigue?.let { f ->
                    Spacer(Modifier.height(28.dp))
                    SectionAnchor("Coach", muted, onBg, modifier = Modifier.padding(horizontal = GUTTER))
                    Spacer(Modifier.height(10.dp))
                    HomeNoteCard(
                        palette = palette,
                        eyebrow = "Recovery",
                        body = "Fatigue ${f.score} of ${f.threshold}" +
                            (f.topDriver?.let { " · $it" } ?: "") +
                            ". Not a deload yet, but easing up helps.",
                        onBg = onBg,
                        modifier = Modifier.padding(horizontal = GUTTER),
                        onClick = onOpenCoachLab,
                        clickLabel = "Open Coach Lab"
                    )
                }
            }

            // ── Trophies (gamification only) ─────────────────────────────────────
            if (Features.SHOW_GAMIFICATION) {
                Spacer(Modifier.height(28.dp))
                HomeTrophiesCard(
                    palette = palette,
                    icon = NavIcons.Academy,
                    unlocked = state.trophiesUnlocked,
                    total = Trophies.all.size,
                    nearMiss = state.topNearMiss,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.padding(horizontal = GUTTER).statsEntrance(11),
                    onClick = onGoToTrophies
                )
            }

            Spacer(Modifier.height(12.dp))
            Row(
                Modifier.fillMaxWidth().padding(horizontal = GUTTER),
                horizontalArrangement = Arrangement.End
            ) {
                Text(
                    "Nutrition · soon",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.65f),
                    modifier = Modifier
                        .clickableLabeled("Open Nutrition") { onGoToNutrition() }
                        .padding(4.dp)
                )
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}

// ── Sections ──────────────────────────────────────────────────────────────────────────────────

/**
 * The header row: who you are on the start, chrome on the end.
 *
 * §4.6 survives intact — the bell is still Home-only, the row still carries exactly one other
 * action (Profile), and the screen still does not print its own name.
 *
 * The greeting moved into the hero card's eyebrow and the avatar went back to the Profile glyph
 * (Antho, 2026-08-15). The avatar was showing the COVER photo, which is a background image rather
 * than a portrait — the app has no concept of a face, so rendering the cover in a 36dp circle
 * claimed one it does not have. A glyph is honest about that.
 */
@Composable
private fun HeaderRow(
    name: String,
    palette: SurfacePalette,
    onBg: Color,
    muted: Color,
    onOpenProfile: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            name.ifBlank { "Athlete" },
            style = MaterialTheme.typography.titleLarge,
            color = onBg,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
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
}

/** The goals strip: real cards, or the ghost strip that runs off the edge the same way (§12). */
@Composable
private fun GoalStrip(
    state: com.forge.app.ui.overview.state.OverviewUiState,
    palette: SurfacePalette,
    onBg: Color,
    muted: Color,
    onOpenGoals: () -> Unit,
    modifier: Modifier = Modifier
) {
    val settings = LocalForgeSettings.current
    // In-progress goals lead (closest first), then reached ones — a full slate of achieved goals
    // must not bury the target actually being worked toward.
    val lift = state.goals.map { g ->
        GoalCardData(
            title = g.name,
            valueLine = "${com.forge.app.domain.units.weightInputValue(g.currentBestLb, settings.weightUnit)} / " +
                "${com.forge.app.domain.units.weightInputValue(g.targetLb, settings.weightUnit)} " +
                unitLabel(settings.weightUnit),
            fraction = g.fraction,
            achieved = g.achieved
        )
    }
    val custom = state.customGoals.map { g ->
        GoalCardData(
            title = customGoalTitle(g),
            valueLine = customGoalValueLine(g, settings.weightUnit, settings.useMiles),
            fraction = g.fraction,
            achieved = g.achieved
        )
    }
    val preview = (lift + custom)
        .sortedWith(compareBy<GoalCardData> { it.achieved }.thenByDescending { it.fraction })
        .take(6)

    PeekCardRow(modifier, gutter = GUTTER) {
        if (preview.isEmpty()) {
            items(3) { i -> HomeGoalGhost(palette, i, muted, onBg, onOpenGoals) }
        } else {
            itemsIndexed(preview) { i, goal ->
                HomeGoalCard(
                    palette = palette,
                    title = goal.title,
                    valueLine = goal.valueLine,
                    fraction = goal.fraction,
                    achieved = goal.achieved,
                    hue = palette.hues[i % palette.hues.size],
                    onBg = onBg,
                    muted = muted,
                    onClick = onOpenGoals
                )
            }
        }
    }
}

private data class GoalCardData(
    val title: String,
    val valueLine: String,
    val fraction: Float,
    val achieved: Boolean
)

/** One RECENT row: leading glyph · title + date · volume + trend. */
@Composable
private fun RecentSurfaceRow(
    item: OverviewRecentItem,
    palette: SurfacePalette,
    onBg: Color,
    muted: Color,
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
    // Colour as data: a session above its day-type average reads positive, below reads negative.
    // The word carries it too ("BEST", "+8% avg"), so the tint is never the only channel.
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
        hue = if (item.isGym) palette.hues[0] else palette.hues[1],
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

// ── Small helpers ─────────────────────────────────────────────────────────────────────────────

/** Time-of-day greeting. Dry and factual — it names the part of the day, nothing more (§11). */
private fun greetingNow(): String = when (java.time.LocalTime.now().hour) {
    in 0..4 -> "Late night"
    in 5..11 -> "Good morning"
    in 12..17 -> "Good afternoon"
    else -> "Good evening"
}

/** What TalkBack speaks for the hero curve — the reading, not the shape (§14). */
private fun weeklySeriesReading(
    series: List<Double>,
    unit: com.forge.app.domain.units.WeightUnit
): String {
    if (series.isEmpty()) return "No weekly volume yet"
    val now = toDisplayWeight(series.last(), unit).roundToInt()
    val peak = toDisplayWeight(series.max(), unit).roundToInt()
    return "Weekly volume over ${series.size} weeks, now $now ${unitLabel(unit)}, best $peak"
}
