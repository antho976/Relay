package com.forge.app.ui.overview

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateIntAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.semantics.Role
import com.forge.app.Features
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.overview.state.MilestoneEvent
import com.forge.app.ui.overview.state.OnThisDayMemory
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.overview.components.OverviewStat
import com.forge.app.ui.overview.components.RecentRow
import com.forge.app.ui.overview.components.TrophiesTile
import com.forge.app.ui.overview.components.WeekDayBox
import java.time.LocalDate

/**
 * A dismissible info strip (e.g. the auto-resolved orphan-session notice, E8). The close affordance
 * is a 48 dp touch target — the [Icon] itself stays 16 dp, but its hit area meets the a11y minimum.
 */
@Composable
private fun DismissibleNotice(text: String, onBg: Color, muted: Color, onDismiss: () -> Unit) {
    Spacer(Modifier.height(16.dp))
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(muted.copy(alpha = 0.10f))
            .padding(start = 14.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text, style = MaterialTheme.typography.bodySmall, color = onBg,
            modifier = Modifier.weight(1f).padding(vertical = 12.dp)
        )
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(CircleShape)
                .clickable(onClick = onDismiss),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                Icons.Default.Close, contentDescription = "Dismiss",
                tint = muted.copy(alpha = 0.7f),
                modifier = Modifier.size(16.dp)
            )
        }
    }
}

/** Top-bar icon with a ≥44dp tappable area + spoken label, while the glyph stays visually small. */
@Composable
private fun TopBarIconButton(icon: ImageVector, label: String, tint: Color, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
            .clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(20.dp))
    }
}

/** The shared "COACH" home-section scaffold (divider + label + a tappable headline/body). At most one
 *  of three states fills it — learning countdown, fatigue nudge, or persistent entry — so they share
 *  one treatment and can't drift apart. Emits into the calling column in order. */
@Composable
private fun CoachHomeBlock(headline: String, body: String, clickLabel: String, onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    Spacer(Modifier.height(20.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.3f))
    Spacer(Modifier.height(16.dp))
    Text("COACH", style = MaterialTheme.typography.labelMedium, color = muted)
    Spacer(Modifier.height(10.dp))
    Column(
        Modifier.fillMaxWidth().clickableLabeled(clickLabel) { onClick() }.padding(vertical = 2.dp)
    ) {
        Text(headline, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Spacer(Modifier.height(2.dp))
        Text(body, style = MaterialTheme.typography.bodySmall, color = muted)
    }
}

@Composable
fun OverviewScreen(
    onStartSession: (dayKey: String) -> Unit,
    onStartSessionSkipWarmup: (dayKey: String) -> Unit = onStartSession,
    onViewProgram: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    onOpenNotes: () -> Unit = {},
    onGoToNutrition: () -> Unit = {},
    onGoToSettings: () -> Unit = {},
    onOpenCoachBrief: () -> Unit = {},
    onOpenCoachLab: () -> Unit = {},
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
    val coachBanner by viewModel.coachBanner.collectAsStateWithLifecycle()
    val orphanNotice by viewModel.orphanNotice.collectAsStateWithLifecycle()
    val selectedItem by viewModel.selectedItem.collectAsStateWithLifecycle()
    val summaryLines by viewModel.sessionExerciseLines.collectAsStateWithLifecycle()
    var showDayEdit by remember { mutableStateOf(false) }
    // A milestone "fires and vanishes" otherwise (it's marked shown immediately) — capture it into a
    // transient banner so the user actually sees it (#Overview pendingMilestone).
    var milestoneToast by remember { mutableStateOf<MilestoneEvent?>(null) }

    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { event ->
            milestoneToast = event
            viewModel.onMilestoneShown(event.id) // persist so it won't recompute/re-fire
        }
    }
    // Auto-dismiss the celebratory banner after a few seconds (it's a toast, not a task).
    LaunchedEffect(milestoneToast) {
        if (milestoneToast != null) { kotlinx.coroutines.delay(6000); milestoneToast = null }
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

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    if (showDayEdit) {
        DayEditSheet(
            initialDayKey = state.nextUpDayKey,
            onSelectAsToday = { dayKey -> viewModel.setPlanNextDay(dayKey) },
            onDismiss = { showDayEdit = false }
        )
    }

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
                .graphicsLayer {
                    alpha = entry.value
                    translationY = (1f - entry.value) * 24.dp.toPx()
                }
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            Spacer(Modifier.height(16.dp))

            // ── Top bar ──────────────────────────────────────────────────────
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(5.dp).background(accent, CircleShape))
                    Spacer(Modifier.width(6.dp))
                    Text("Forge", style = MaterialTheme.typography.bodyMedium,
                        fontStyle = FontStyle.Italic, color = onBg)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    // ≥44dp touch target (a11y) — the glyph stays small, the tappable area doesn't.
                    // Cardio, Stats and Profile now live in the bottom bar; Settings stays here.
                    TopBarIconButton(Icons.Default.Settings, "Settings", muted.copy(alpha = 0.7f)) { onGoToSettings() }
                }
            }

            // Milestone banner — the celebratory moment that used to fire and vanish silently.
            milestoneToast?.let { ms ->
                Spacer(Modifier.height(12.dp))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.14f))
                        .clickableLabeled("Dismiss") { milestoneToast = null }
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text("✦", color = accent, style = MaterialTheme.typography.titleSmall)
                    Text(ms.message, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
                }
            }

            // ── Coach: a new Week Brief is ready (dismissible; lives in Settings too) ──
            if (coachEnabled) coachBanner?.let { banner ->
                Spacer(Modifier.height(16.dp))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.12f))
                        .clickableLabeled("Open the week brief") { viewModel.dismissCoachBanner(); onOpenCoachBrief() }
                        .padding(start = 14.dp, end = 8.dp, top = 12.dp, bottom = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column(Modifier.weight(1f)) {
                        Text("COACH BRIEF", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                        // banner.text is the brief's one-line summary (summaryFor) — drop its redundant
                        // "Coach · " prefix so it reads as a clean preview line under the eyebrow.
                        Text(banner.text.removePrefix("Coach · "), style = MaterialTheme.typography.bodyMedium, color = onBg)
                    }
                    Text("→", style = MaterialTheme.typography.bodyLarge, color = accent)
                    Icon(
                        Icons.Default.Close, contentDescription = "Dismiss",
                        tint = muted.copy(alpha = 0.7f),
                        modifier = Modifier
                            .padding(start = 8.dp)
                            .size(16.dp)
                            .clickableLabeled("Dismiss") { viewModel.dismissCoachBanner() }
                    )
                }
            }

            // ── Orphan-session notice: a zombie session was auto-resolved (E8) ──
            orphanNotice?.let { notice ->
                DismissibleNotice(notice, onBg, muted) { viewModel.dismissOrphanNotice() }
            }

            // ── Resume reminder: an unfinished workout is waiting (slides + fades in) ──
            val resumeBannerKey = state.activeSessionDayKey
            // Hold the last real key so the row keeps its name through the exit fade after a finish.
            var lastResumeKey by remember { mutableStateOf<String?>(null) }
            if (resumeBannerKey != null) lastResumeKey = resumeBannerKey
            AnimatedVisibility(
                visible = resumeBannerKey != null,
                enter = fadeIn(tween(300)) + slideInVertically(tween(300)) { it / 3 },
                exit = fadeOut(tween(200)) + slideOutVertically(tween(200)) { it / 3 }
            ) {
                val activeKey = (resumeBannerKey ?: lastResumeKey).orEmpty()
                Column {
                    Spacer(Modifier.height(16.dp))
                    val activeName = Program.days.firstOrNull { it.key == activeKey }?.defaultName ?: "Workout"
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(10.dp))
                            .background(accent.copy(alpha = 0.12f))
                            // Gate on the LIVE key, not the held one: during the exit fade after a
                            // finish, resumeBannerKey is null, so a stray tap can't re-enter the
                            // just-completed session.
                            .clickableLabeled("Resume your workout") {
                                resumeBannerKey?.let { viewModel.onSessionStarting(); onStartSession(it) }
                            }
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Column {
                            Text("IN PROGRESS", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                            Text("$activeName · tap to resume", style = MaterialTheme.typography.bodyMedium, color = onBg)
                        }
                        Text("→", style = MaterialTheme.typography.bodyLarge, color = accent)
                    }
                }
            }

            // ── Comeback nudge: a multi-day gap since the last session (>4d) ──
            state.daysSinceLastSession?.let { gap ->
                if (gap > 4 && state.activeSessionDayKey == null) {
                    Spacer(Modifier.height(16.dp))
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(10.dp))
                            .background(accent.copy(alpha = 0.10f))
                            .padding(horizontal = 14.dp, vertical = 12.dp)
                    ) {
                        Text("WELCOME BACK", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                        Text("It's been $gap days — start light today and rebuild momentum.",
                            style = MaterialTheme.typography.bodyMedium, color = onBg)
                    }
                }
            }

            // ── Recovery snapshot: resting HR elevated vs your baseline (Health Connect) ──
            state.recoveryAlert?.let { alert ->
                Spacer(Modifier.height(16.dp))
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.10f))
                        .padding(horizontal = 14.dp, vertical = 12.dp)
                ) {
                    Text("RECOVERY", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                    Text(
                        "Resting HR is ${alert.deltaBpm} bpm above your baseline " +
                            "(${alert.restingBpm} vs ${alert.baselineBpm}) — a sign you're not fully recovered. " +
                            "An easier day and an earlier night will help.",
                        style = MaterialTheme.typography.bodyMedium, color = onBg
                    )
                }
            }

            // ── Cardio-load nudge: a big cardio block in the last day or two ──
            // Dismissible for the session (rememberSaveable) — a closed nudge re-appears next launch
            // only while the heavy-cardio window still holds, not forever.
            var cardioNudgeDismissed by rememberSaveable { mutableStateOf(false) }
            if (state.heavyCardioRecent && state.activeSessionDayKey == null && !cardioNudgeDismissed) {
                Spacer(Modifier.height(16.dp))
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.10f))
                        .padding(horizontal = 14.dp, vertical = 12.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("RECENT CARDIO", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                        Icon(
                            Icons.Default.Close, contentDescription = "Dismiss",
                            tint = muted.copy(alpha = 0.7f),
                            modifier = Modifier.size(16.dp).clickableLabeled("Dismiss") { cardioNudgeDismissed = true }
                        )
                    }
                    state.recentCardioLead?.let { lead ->
                        Text(lead, style = MaterialTheme.typography.bodyLarge, color = onBg)
                        Spacer(Modifier.height(4.dp))
                    }
                    Text(
                        "Enough to compete with lifting recovery — keep today's heavy work a touch submaximal, or fuel up well before you go after it.",
                        style = MaterialTheme.typography.bodyMedium, color = muted
                    )
                }
            }

            Spacer(Modifier.height(20.dp))

            if (freestyleMode || programEmpty) {
                // No fixed plan to surface — lead with a CTA instead of a next-up day card.
                // freestyle → "log what you did"; empty plan → "build a plan" (with logging as a fallback).
                Text("TODAY", style = MaterialTheme.typography.labelSmall, fontSize = 13.sp, color = muted)
                Spacer(Modifier.height(2.dp))
                Text(if (freestyleMode) "Open workout" else "No plan yet",
                    style = MaterialTheme.typography.displayLarge, color = onBg)
                Spacer(Modifier.height(10.dp))
                Text(
                    if (freestyleMode) "No fixed plan — just log whatever you trained, whenever you want."
                    else "Build your own plan, or just log what you do each session.",
                    style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic), color = muted
                )
                Spacer(Modifier.height(20.dp))
                if (programEmpty && !freestyleMode) {
                    // Same shape as the Start-session row: a primary action + a bordered-pill secondary
                    // (the "skip warmup" style), here "Build a plan" + "log a workout".
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Button(
                            onClick = onBuildPlan,
                            shape = RoundedCornerShape(50),
                            colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = Color.Black),
                            contentPadding = PaddingValues(horizontal = 32.dp, vertical = 18.dp)
                        ) {
                            Text("Build a plan →", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold)
                        }
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(50))
                                .border(0.5.dp, muted.copy(alpha = 0.4f), RoundedCornerShape(50))
                                .clickableLabeled("Log a workout") { onLogFreestyle() }
                                .padding(horizontal = 18.dp, vertical = 12.dp)
                        ) {
                            Text("log a workout", style = MaterialTheme.typography.bodySmall, color = muted)
                        }
                    }
                } else {
                    Button(
                        onClick = onLogFreestyle,
                        shape = RoundedCornerShape(50),
                        colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = Color.Black),
                        contentPadding = PaddingValues(horizontal = 32.dp, vertical = 18.dp)
                    ) {
                        Text("Log a workout →", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold)
                    }
                }
            } else {
                // ── Next workout ─────────────────────────────────────────────────
                // If you've already trained today, the next session is tomorrow — say so.
                val trainedToday = todayDow in state.weekDaysTrained
                Text(if (trainedToday) "TOMORROW" else "TODAY",
                    style = MaterialTheme.typography.labelSmall, fontSize = 13.sp, color = muted)
                Spacer(Modifier.height(2.dp))
                Text(
                    state.customDayName ?: nextDay?.defaultName ?: "Ready",
                    style = MaterialTheme.typography.displayLarge,
                    color = onBg,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = if (nextDay != null) Modifier.clickableLabeled("Edit or swap this day") { showDayEdit = true } else Modifier
                )
                if (nextDay != null) {
                    Spacer(Modifier.height(10.dp))
                    Text(
                        "${nextDay.subtitle} · ${nextDay.exercises.size} exercises",
                        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Tap the day name to edit or swap it",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.7f)
                    )
                }

                Spacer(Modifier.height(20.dp))

                // ── Start / resume session + skip warmup ─────────────────────────
                val resumeKey = state.activeSessionDayKey
                val ctaDayKey = resumeKey ?: state.nextUpDayKey
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(
                        onClick = { viewModel.onSessionStarting(); onStartSession(ctaDayKey) },
                        shape = RoundedCornerShape(50),
                        colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = Color.Black),
                        contentPadding = PaddingValues(horizontal = 32.dp, vertical = 18.dp)
                    ) {
                        Text(
                            if (resumeKey != null) "Resume session →" else "Start session →",
                            style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold
                        )
                    }
                    // Skipping the warmup only applies to a fresh start, not a resume.
                    if (resumeKey == null) {
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(50))
                                .border(0.5.dp, muted.copy(alpha = 0.4f), RoundedCornerShape(50))
                                .clickableLabeled("Start, skipping warmup") { val d = state.nextUpDayKey; viewModel.onSessionStarting(); onStartSessionSkipWarmup(d) }
                                .padding(horizontal = 18.dp, vertical = 12.dp)
                        ) {
                            Text("skip warmup", style = MaterialTheme.typography.bodySmall, color = muted)
                        }
                    }
                }
            }

            Spacer(Modifier.height(28.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(20.dp))

            // ── This week ────────────────────────────────────────────────────
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically) {
                Text("THIS WEEK", style = MaterialTheme.typography.labelMedium, color = muted)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("${state.workoutsThisWeek} of ${state.weeklyWorkoutTarget} target", style = MaterialTheme.typography.labelSmall, color = muted)
                    Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f))
                    Text("view program →", style = MaterialTheme.typography.labelMedium,
                        color = accent,
                        modifier = Modifier.clickableLabeled("View your program") { onViewProgram() }.padding(vertical = 2.dp))
                }
            }
            Spacer(Modifier.height(12.dp))

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                listOf("M", "T", "W", "T", "F", "S", "S").forEachIndexed { i, letter ->
                    WeekDayBox(letter = letter, trained = i in state.weekDaysTrained,
                        isToday = i == todayDow, outlineColor = outline, modifier = Modifier.weight(1f))
                }
            }
            Spacer(Modifier.height(16.dp))

            val useKg = LocalForgeSettings.current.useKg
            val animWorkouts by animateIntAsState(state.workoutsThisWeek.coerceAtLeast(0), label = "workouts")
            val animVolume by animateIntAsState(toDisplayWeight(state.volumeThisWeekLb, useKg).coerceAtLeast(0.0).toInt(), label = "volume")
            val animCardio by animateIntAsState(state.cardioMinutesThisWeek.coerceAtLeast(0), label = "cardio")
            Row(modifier = Modifier.fillMaxWidth()) {
                OverviewStat(value = "$animWorkouts", label = "WORKOUTS", modifier = Modifier.weight(1f))
                OverviewStat(value = "$animVolume", label = if (useKg) "KG" else "LB", modifier = Modifier.weight(1f))
                OverviewStat(
                    value = if (state.cardioWeeklyTargetMin > 0) "$animCardio/${state.cardioWeeklyTargetMin}" else "$animCardio",
                    label = "CARDIO MIN", modifier = Modifier.weight(1f)
                )
            }

            // ── Coach (adaptation engine: actionable advice only) ────────────
            if (coachEnabled && state.coach.isNotEmpty()) {
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.3f))
                Spacer(Modifier.height(16.dp))
                Text("COACH", style = MaterialTheme.typography.labelMedium, color = muted)
                Spacer(Modifier.height(10.dp))
                // Each coach card cascades in (fade + settle-up), so the section reveals as a
                // sequence rather than one block. Reduced-motion-safe via the shared motion kit.
                state.coach.forEachIndexed { index, item ->
                    Column(Modifier.fillMaxWidth().statsEntrance(index).padding(bottom = 14.dp)) {
                        Text(item.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Spacer(Modifier.height(2.dp))
                        Text(item.body, style = MaterialTheme.typography.bodySmall, color = muted)
                        Spacer(Modifier.height(6.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                            if (item.applyLabel != null) {
                                Text(
                                    "${item.applyLabel} →",
                                    style = MaterialTheme.typography.labelSmall, color = accent,
                                    modifier = Modifier
                                        .clickableLabeled("Apply this suggestion") { viewModel.applyCoach(item) }
                                        .padding(vertical = 2.dp)
                                )
                            }
                            Text(
                                "dismiss",
                                style = MaterialTheme.typography.labelSmall,
                                color = muted.copy(alpha = 0.7f),
                                modifier = Modifier
                                    .clickableLabeled("Dismiss this suggestion") { viewModel.dismissCoach(item) }
                                    .padding(vertical = 2.dp)
                            )
                        }
                    }
                }
            }

            // ── Coach (home entry) — at most one shows: a learning countdown (CD-1), a fatigue nudge
            // (Tier 3), or a persistent "quiet but reachable" entry (D3). All share CoachHomeBlock. ──
            if (coachEnabled) {
                state.coachLearning?.let { hint ->
                    CoachHomeBlock(
                        "Still learning your training.",
                        "${hint.sessionsToGo} more session${if (hint.sessionsToGo == 1) "" else "s"} and it starts " +
                            "calling weekly adjustments. See what it's tracking →",
                        clickLabel = "Open Coach Lab", onClick = onOpenCoachLab
                    )
                }
                state.coachFatigue?.let { f ->
                    CoachHomeBlock(
                        "Recovery signals building.",
                        "Fatigue ${f.score} of ${f.threshold}${f.topDriver?.let { " · $it" } ?: ""} — " +
                            "not a deload yet, but easing up helps. See what it's tracking →",
                        clickLabel = "Open Coach Lab", onClick = onOpenCoachLab
                    )
                }
                if (state.coach.isEmpty() && state.coachLearning == null && state.coachFatigue == null) {
                    CoachHomeBlock(
                        "Your coach.",
                        "See your weekly brief and what it's tracking →",
                        clickLabel = "Open the week brief", onClick = onOpenCoachBrief
                    )
                }
            }

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(16.dp))

            // ── Recent ───────────────────────────────────────────────────────
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically) {
                Text("RECENT", style = MaterialTheme.typography.labelMedium, color = muted)
                Text("view all →", style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 10.sp,
                    modifier = Modifier.clickableLabeled("View all sessions") { onViewAllHistory() }.padding(vertical = 2.dp))
            }
            Spacer(Modifier.height(10.dp))
            if (state.recentItems.isEmpty()) {
                InlineEmptyHint("No workouts yet — tap \"Start session\" above to log your first, and it'll show up here.",
                    color = muted)
            } else {
                state.recentItems.forEach { item ->
                    RecentRow(item = item, muted = muted, onBg = onBg, outline = outline,
                        // Gym sessions open the full detail page; cardio keeps the lightweight summary sheet.
                        onClick = { if (item.isGym) onOpenSession(item.id) else viewModel.selectRecentItem(item) })
                    Spacer(Modifier.height(14.dp))
                }
            }

            // ── On this day: a session from N months ago near today's date (#106) ──
            state.onThisDayMemory?.let { memory ->
                Spacer(Modifier.height(16.dp))
                OnThisDayCard(memory = memory, onBg = onBg, muted = muted, accent = accent, outline = outline)
            }

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(12.dp))

            // ── Trophies (gamification only) ─────────────────────────────────
            // "Search notes" and the cardio "Moved today?" nudge have been pulled from the home
            // page (UI only). Their handlers (onOpenNotes / onGoToCardio) stay wired and all backing
            // state is untouched — so these surfaces can be re-added without re-plumbing.
            // Trophies (gamification) are parked behind Features.SHOW_GAMIFICATION.
            if (Features.SHOW_GAMIFICATION) {
                TrophiesTile(unlocked = state.trophiesUnlocked, total = Trophies.all.size,
                    onClick = onGoToTrophies, onBg = onBg, muted = muted, outline = outline,
                    modifier = Modifier.fillMaxWidth())
            }

            // "Up next" trophy — the locked trophy closest to unlocking (a near-miss retention hook, #136).
            if (Features.SHOW_GAMIFICATION) state.topNearMiss?.let { nudge ->
                Spacer(Modifier.height(6.dp))
                Text(
                    "▲ NEXT TROPHY · $nudge",
                    style = MaterialTheme.typography.labelSmall,
                    color = accent.copy(alpha = 0.9f),
                    fontSize = 10.sp,
                    modifier = Modifier
                        .clickableLabeled("Trophy almost unlocked — $nudge") { onGoToTrophies() }
                        .padding(vertical = 2.dp)
                )
            }

            Spacer(Modifier.height(8.dp))
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Text("Nutrition · soon", style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f), fontSize = 10.sp,
                    modifier = Modifier.clickableLabeled("Open Nutrition") { onGoToNutrition() }.padding(4.dp))
            }

            Spacer(Modifier.height(24.dp))
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
    val useKg = LocalForgeSettings.current.useKg
    val agoLabel = com.forge.app.ui.common.monthsAgoPhrase(memory.monthsAgo).uppercase()
    val vol = com.forge.app.domain.units.formatVolumeCompact(memory.totalVolumeLb, useKg)
    val prText = if (memory.prCount > 0) " · ${memory.prCount} PR${if (memory.prCount > 1) "s" else ""}" else ""
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(10.dp))
            .padding(horizontal = 14.dp, vertical = 12.dp)
    ) {
        Text("ON THIS DAY · $agoLabel", style = MaterialTheme.typography.labelSmall,
            color = accent, fontSize = 10.sp)
        Spacer(Modifier.height(2.dp))
        Text("You trained ${memory.dayName} — $vol moved$prText",
            style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}
