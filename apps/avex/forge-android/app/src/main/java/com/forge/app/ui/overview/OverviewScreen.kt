package com.forge.app.ui.overview

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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.theme.emphasized
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.overview.components.CardioTile
import com.forge.app.ui.overview.components.OverviewStat
import com.forge.app.ui.overview.components.RecentRow
import com.forge.app.ui.overview.components.StatsTile
import com.forge.app.ui.overview.components.TrophiesTile
import com.forge.app.ui.overview.components.WeekDayBox
import java.time.LocalDate
import java.time.format.TextStyle
import java.time.temporal.WeekFields
import java.util.Locale

@Composable
fun OverviewScreen(
    onStartSession: (dayKey: String) -> Unit,
    onStartSessionSkipWarmup: (dayKey: String) -> Unit = onStartSession,
    onViewProgram: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    onGoToStats: () -> Unit = {},
    onGoToNutrition: () -> Unit = {},
    onGoToSettings: () -> Unit = {},
    onOpenCoachBrief: () -> Unit = {},
    onOpenProfile: () -> Unit = {},
    onOpenSession: (Long) -> Unit = {},
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val coachBanner by viewModel.coachBanner.collectAsStateWithLifecycle()
    val selectedItem by viewModel.selectedItem.collectAsStateWithLifecycle()
    val summaryLines by viewModel.sessionExerciseLines.collectAsStateWithLifecycle()
    var showDayEdit by remember { mutableStateOf(false) }
    var showHistory by remember { mutableStateOf(false) }

    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { event -> viewModel.onMilestoneShown(event.id) }
    }

    // Content rises + fades in on open — a premium reveal instead of snapping in.
    val entry = remember { Animatable(0f) }
    LaunchedEffect(Unit) { entry.animateTo(1f, animationSpec = tween(450)) }

    val today = LocalDate.now()
    val todayDow = today.dayOfWeek.value - 1
    val weekNumber = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val dayName = today.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()
    val weekStart = today.minusDays((today.dayOfWeek.value - 1).toLong())
    val weekEnd = weekStart.plusDays(6)
    val weekRangeText = buildString {
        append(weekStart.month.getDisplayName(TextStyle.SHORT, Locale.getDefault()))
        append(" ${weekStart.dayOfMonth}")
        append(" – ")
        append(weekEnd.month.getDisplayName(TextStyle.SHORT, Locale.getDefault()))
        append(" ${weekEnd.dayOfMonth}")
    }

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

    if (showHistory) {
        HistorySheet(
            onDismiss = { showHistory = false },
            onOpenSession = { id -> showHistory = false; onOpenSession(id) }
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
                    Column(horizontalAlignment = Alignment.End) {
                        Text("$dayName · WK $weekNumber", style = MaterialTheme.typography.labelSmall,
                            fontSize = 13.sp, color = muted)
                        Text(weekRangeText, style = MaterialTheme.typography.labelSmall,
                            fontSize = 9.sp, color = muted.copy(alpha = 0.8f))
                    }
                    Spacer(Modifier.width(8.dp))
                    Icon(
                        Icons.Default.Settings, contentDescription = "Settings",
                        tint = muted.copy(alpha = 0.7f),
                        modifier = Modifier.size(16.dp).clickable { onGoToSettings() }
                    )
                    Spacer(Modifier.width(10.dp))
                    Icon(
                        Icons.Default.AccountCircle, contentDescription = "You",
                        tint = muted.copy(alpha = 0.7f),
                        modifier = Modifier.size(18.dp).clickable { onOpenProfile() }
                    )
                }
            }

            // ── Coach: a new Week Brief is ready (dismissible; lives in Settings too) ──
            coachBanner?.let { banner ->
                Spacer(Modifier.height(16.dp))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.12f))
                        .clickable { viewModel.dismissCoachBanner(); onOpenCoachBrief() }
                        .padding(start = 14.dp, end = 8.dp, top = 12.dp, bottom = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column(Modifier.weight(1f)) {
                        Text("NEW BRIEF", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                        Text(banner.text, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    }
                    Text("→", style = MaterialTheme.typography.bodyLarge, color = accent)
                    Icon(
                        Icons.Default.Close, contentDescription = "Dismiss",
                        tint = muted.copy(alpha = 0.7f),
                        modifier = Modifier
                            .padding(start = 8.dp)
                            .size(16.dp)
                            .clickable { viewModel.dismissCoachBanner() }
                    )
                }
            }

            // ── Resume reminder: an unfinished workout is waiting ────────────
            state.activeSessionDayKey?.let { activeKey ->
                Spacer(Modifier.height(16.dp))
                val activeName = Program.days.firstOrNull { it.key == activeKey }?.defaultName ?: "Workout"
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.12f))
                        .clickable { viewModel.onSessionStarting(); onStartSession(activeKey) }
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

            Spacer(Modifier.height(20.dp))

            // ── Next workout ─────────────────────────────────────────────────
            // If you've already trained today, the next session is tomorrow — say so.
            val trainedToday = todayDow in state.weekDaysTrained
            Text(if (trainedToday) "TOMORROW" else "TODAY",
                style = MaterialTheme.typography.labelSmall, fontSize = 13.sp, color = emphasized(muted))
            Spacer(Modifier.height(2.dp))
            Text(
                state.customDayName ?: nextDay?.defaultName ?: "Ready",
                style = MaterialTheme.typography.displayLarge,
                color = emphasized(onBg),
                modifier = if (nextDay != null) Modifier.clickable { showDayEdit = true } else Modifier
            )
            if (nextDay != null) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "${nextDay.subtitle} · ${nextDay.exercises.size} exercises",
                    style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                    color = muted
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
                            .clickable { val d = state.nextUpDayKey; viewModel.onSessionStarting(); onStartSessionSkipWarmup(d) }
                            .padding(horizontal = 18.dp, vertical = 12.dp)
                    ) {
                        Text("skip warmup", style = MaterialTheme.typography.bodySmall, color = muted)
                    }
                }
            }

            Spacer(Modifier.height(28.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(20.dp))

            // ── This week ────────────────────────────────────────────────────
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically) {
                Text("THIS WEEK", style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("${state.workoutsThisWeek} of 6 target", style = MaterialTheme.typography.labelSmall, color = muted)
                    Text("·", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.5f))
                    Text("view program →", style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 10.sp,
                        modifier = Modifier.clickable { onViewProgram() }.padding(vertical = 2.dp))
                }
            }
            Spacer(Modifier.height(12.dp))

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                listOf("M", "T", "W", "T", "F", "S", "S").forEachIndexed { i, letter ->
                    WeekDayBox(letter = letter, trained = i in state.weekDaysTrained,
                        isToday = i == todayDow, outlineColor = outline, modifier = Modifier.weight(1f))
                }
            }
            // Streak hook — the chain you don't want to break. Encouraging, not guilt-y
            // (the underlying streak is vacation-aware + gap-bridged, so rest never punishes you).
            if (state.streakDays >= 2) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "🔥 ${state.streakDays}-day streak — keep it alive",
                    style = MaterialTheme.typography.labelMedium,
                    color = accent
                )
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
            if (state.coach.isNotEmpty()) {
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.3f))
                Spacer(Modifier.height(16.dp))
                Text("COACH", style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
                Spacer(Modifier.height(10.dp))
                state.coach.forEach { item ->
                    Column(Modifier.fillMaxWidth().padding(bottom = 14.dp)) {
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
                                        .clickable { viewModel.applyCoach(item) }
                                        .padding(vertical = 2.dp)
                                )
                            }
                            Text(
                                "dismiss",
                                style = MaterialTheme.typography.labelSmall,
                                color = muted.copy(alpha = 0.7f),
                                modifier = Modifier
                                    .clickable { viewModel.dismissCoach(item) }
                                    .padding(vertical = 2.dp)
                            )
                        }
                    }
                }
            }

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(16.dp))

            // ── Recent ───────────────────────────────────────────────────────
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically) {
                Text("RECENT", style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
                Text("view all →", style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 10.sp,
                    modifier = Modifier.clickable { showHistory = true }.padding(vertical = 2.dp))
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

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(12.dp))

            // ── Cardio · Stats · Trophies ────────────────────────────────────
            CardioTile(cardioWeekDays = state.cardioWeekDays, totalMin = state.cardioMinutesThisWeek,
                totalKm = state.cardioDistanceKm, onClick = onGoToCardio,
                onBg = onBg, muted = muted, outline = outline)
            Spacer(Modifier.height(8.dp))
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                StatsTile(totalSessions = state.totalFinishedSessions, streakDays = state.streakDays,
                    onClick = onGoToStats, onBg = onBg, muted = muted, outline = outline, modifier = Modifier.weight(1f))
                TrophiesTile(unlocked = state.trophiesUnlocked, total = Trophies.all.size,
                    onClick = onGoToTrophies, onBg = onBg, muted = muted, outline = outline, modifier = Modifier.weight(1f))
            }

            Spacer(Modifier.height(8.dp))
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Text("Nutrition · soon", style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f), fontSize = 10.sp,
                    modifier = Modifier.clickable { onGoToNutrition() }.padding(4.dp))
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}
