package com.forge.app.ui.cardio

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.cardio.components.CardioEntryRow
import com.forge.app.ui.cardio.components.CardioPaceTrendSection
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet
import com.forge.app.ui.cardio.components.StepsByHourSection
import com.forge.app.ui.cardio.components.WatchImportsSection
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.state.CardioLens
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeHeroAction
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.common.statsEntrance
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale

/** Air between sections — §7's rhythm, applied once so no section carries its own leading spacer. */
private val SECTION_GAP = 28.dp

@Composable
fun CardioScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    // When set, "view all" opens the unified History page (where cardio + workouts merge);
    // null falls back to expanding the list inline.
    onOpenHistory: (() -> Unit)? = null,
    // Opens the full Goals screen — from the GOALS trim's header action / lines.
    onOpenGoals: () -> Unit = {},
    // Opens the weeks chart — the hero's `weeks →`. Replaced the swipe-pager overlay (2026-08-23).
    onOpenWeeks: () -> Unit = {},
    // Opens one week's own page — the hero's Mon–Sun strip, with this week's Monday.
    onOpenWeek: (Long) -> Unit = {},
    viewModel: CardioViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    // Re-check the Health Connect grants whenever the screen resumes — the user can connect steps/GPS
    // in Settings (or the HC app) and come back, and the placeholders should appear without a reload.
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                viewModel.refreshConnection()
                // …and re-anchor "this week". A retained ViewModel keeps the Monday it was built
                // with, while the labels and day cells below are drawn from a fresh LocalDate — so a
                // phone left on this tab over Sunday night showed the new week's dates above the old
                // week's totals.
                viewModel.refreshWeekAnchor()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Health Connect's per-route consent screen — returns the chosen session's route (or null if the
    // user declines). Launched from the session sheet's "Show GPS route" button.
    val routeLauncher = rememberLauncherForActivityResult(
        contract = androidx.health.connect.client.contracts.ExerciseRouteRequestContract()
    ) { route -> viewModel.onRouteConsented(route) }

    val zone = ZoneId.systemDefault()
    // Today comes from the ViewModel's anchor, not from a fresh clock read here (M-15). A
    // composition that reads the clock directly has nothing to recompose it, so a phone left on
    // this tab from Monday 23:59 into Tuesday kept Monday's dates in the week label and kept
    // Monday styled as today. The anchor moves on every day boundary and on any clock or timezone
    // change, and moving it is what re-emits this state. Zero is the pre-load state only.
    val today = remember(state.todayStartMs, zone) {
        state.todayStartMs.takeIf { it > 0L }
            ?.let { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
            ?: LocalDate.now(zone)
    }
    val weekNum = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val isoWeekEnd = isoWeekStart.plusDays(6)
    val weekLabel = remember(isoWeekStart) {
        val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
        "${isoWeekStart.format(fmt).uppercase()} – ${isoWeekEnd.format(fmt).uppercase()}"
    }
    val isoWeekStartMs = remember(isoWeekStart) {
        isoWeekStart.atStartOfDay(zone).toInstant().toEpochMilli()
    }
    val todayDow = today.dayOfWeek.value - 1

    // The session whose detail overlay is open (if any) — auto-closed if that entry vanishes (deleted).
    val sessionEntry = state.sessionDetailId?.let { id -> state.entries.firstOrNull { it.id == id } }
    LaunchedEffect(state.sessionDetailId, sessionEntry) {
        if (state.sessionDetailId != null && sessionEntry == null) viewModel.closeSessionDetail()
    }

    // System Back closes what is on top, in the same order the `when` below renders it.
    //
    // Both of these are full-screen contents swapped in by state rather than nav destinations, so
    // the back stack does not know they exist: Back popped the whole Cardio route — or left the app
    // from the tab root — while a half-typed log sheet was on screen. Every other overlay in this
    // app closes on Back; these were the ones that took the screen with them.
    BackHandler(enabled = state.sheetOpen || sessionEntry != null) {
        if (state.sheetOpen) viewModel.closeSheet() else viewModel.closeSessionDetail()
    }

    when {
        state.sheetOpen -> CardioLogSheet(
            onDismiss = viewModel::closeSheet,
            onSave = viewModel::saveEntry,
            onCreateCustom = viewModel::addCustomType,
            editing = state.editing,
            useMiles = state.useMiles,
            lastUsedType = state.lastCardioType
        )
        sessionEntry != null -> CardioSessionDetailSheet(
            entry = sessionEntry,
            allEntries = state.entries,
            useMiles = state.useMiles,
            route = state.sessionRoute, // Matched watch GPS track, once available/consented (else null).
            onShowRoute = state.sessionRouteConsentId?.let { id -> { routeLauncher.launch(id) } },
            wearable = state.sessionWearable, // That day's watch steps (null until loaded / when none).
            wearableConnected = state.stepsConnected, // Show an empty placeholder once connected.
            hr = state.sessionHr, // Matched watch workout's HR series (W5); null hides the section.
            watchStats = state.sessionWatch,
            onAdoptWatchStats = viewModel::adoptWatchStats,
            onEdit = { viewModel.editEntry(sessionEntry.id) },
            onDelete = { viewModel.deleteEntry(sessionEntry.id) },
            onBack = viewModel::closeSessionDetail
        )
        else -> CardioListContent(
            state = state,
            weekLabel = weekLabel,
            weekStartMs = isoWeekStartMs,
            today = today,
            todayDow = todayDow,
            zone = zone,
            onBack = onBack,
            onOpenLog = viewModel::openSheet,
            onOpenWeeks = onOpenWeeks,
            onOpenThisWeek = { onOpenWeek(isoWeekStartMs) },
            onOpenSession = viewModel::openSessionDetail,
            onRequestDelete = viewModel::deleteEntry,
            onSetLens = viewModel::setLens,
            onSeeAll = onOpenHistory ?: viewModel::toggleHistoryExpanded,
            seeAllExpands = onOpenHistory == null,
            onOpenGoals = onOpenGoals,
            onImportWatch = viewModel::importWatchWorkout,
            onDismissImports = viewModel::dismissWatchImports
        )
    }
}

/**
 * The overview (§3 Overview archetype). One hero, one primary action, then two lenses:
 *
 *   WEEK      what just happened — imports waiting, this week's split, its sessions, today's steps
 *   PROGRESS  where it is going — load over weeks, pace, records, goals
 *
 * The lenses replaced a full-screen week pager reached by tapping the hero (2026-08-23). That pager
 * redrew the hero's own marks one screen away, which is the §4.3 "one home" rule twice over; week
 * BROWSING moved to a ledger where the weeks are rows, not pages.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CardioListContent(
    state: CardioUiState,
    weekLabel: String,
    weekStartMs: Long,
    today: LocalDate,
    todayDow: Int,
    zone: ZoneId,
    onBack: (() -> Unit)?,
    onOpenLog: () -> Unit,
    onOpenWeeks: () -> Unit,
    onOpenThisWeek: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onRequestDelete: (Long) -> Unit,
    onSetLens: (CardioLens) -> Unit,
    onSeeAll: () -> Unit,
    seeAllExpands: Boolean,
    onOpenGoals: () -> Unit,
    onImportWatch: (com.forge.app.domain.health.WatchWorkout) -> Unit,
    onDismissImports: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    // The connect-a-wearable invite lives on the notifications page (2026-07-27) — a page never opens
    // with a resident strip above its own answer (§4.6).

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    if (onBack != null) IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 56.dp)
        ) {
            item("hero") {
                Column(Modifier.statsEntrance(0)) {
                    CardioHero(
                        weekLabel = weekLabel,
                        weekDays = state.cardioDaysThisWeek,
                        weekMinutes = state.weekMinutes,
                        weekDistanceKm = state.weekDistanceKm,
                        streakDays = state.cardioStreakDays,
                        weekTargetMin = state.weekTargetMin,
                        useMiles = state.useMiles,
                        days = state.weekDays,
                        todayDow = todayDow,
                        onBg = onBg, muted = muted, outline = outline, accent = accent,
                        onOpenWeeks = onOpenWeeks,
                        onOpenThisWeek = onOpenThisWeek
                    )
                }
            }

            // The primary action, above the fold (§3) — the old affordance was a small white `+` disc
            // riding a section header, which is not one of §2③'s three levels and named nothing.
            item("log") {
                Spacer(Modifier.height(24.dp))
                // The same hero button Home draws (Antho, 2026-08-23) — a hub tab's primary action
                // reads the same on every tab, and it was the one white capsule in an accent app.
                ForgeHeroAction(
                    text = "Log cardio",
                    onClick = onOpenLog,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp)
                        .statsEntrance(1)
                )
            }

            item("lenses") {
                Spacer(Modifier.height(24.dp))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp)
                        .statsEntrance(2),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    CardioLens.entries.forEach { lens ->
                        SegmentPill(
                            text = lens.label.uppercase(),
                            selected = state.lens == lens,
                            onClick = { onSetLens(lens) },
                            accent = accent, onBg = onBg, muted = muted, outline = outline
                        )
                    }
                }
            }

            when (state.lens) {
                CardioLens.WEEK -> weekLens(
                    state = state,
                    weekStartMs = weekStartMs,
                    today = today,
                    zone = zone,
                    onOpenSession = onOpenSession,
                    onRequestDelete = onRequestDelete,
                    onSeeAll = onSeeAll,
                    seeAllExpands = seeAllExpands,
                    onImportWatch = onImportWatch,
                    onDismissImports = onDismissImports,
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
                CardioLens.PROGRESS -> progressLens(
                    state = state,
                    onOpenSession = onOpenSession,
                    onOpenGoals = onOpenGoals,
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
            }
        }
    }
}

/**
 * WEEK — what just happened. Ordered by §4.8 (placement is rank): the watch sessions still waiting on
 * a decision, then this week's own split, then its sessions, then the passive steps read.
 */
private fun LazyListScope.weekLens(
    state: CardioUiState,
    weekStartMs: Long,
    today: LocalDate,
    zone: ZoneId,
    onOpenSession: (Long) -> Unit,
    onRequestDelete: (Long) -> Unit,
    onSeeAll: () -> Unit,
    seeAllExpands: Boolean,
    onImportWatch: (com.forge.app.domain.health.WatchWorkout) -> Unit,
    onDismissImports: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    // FROM YOUR WATCH (W5) — sessions the watch recorded that have no entry here yet. Row tap imports
    // (prefilled sheet); the header's `hide` dismisses the batch for good. Hidden when empty.
    if (state.importSuggestions.isNotEmpty()) {
        item("watch-imports") {
            Spacer(Modifier.height(SECTION_GAP))
            WatchImportsSection(
                suggestions = state.importSuggestions,
                useMiles = state.useMiles,
                onImport = onImportWatch,
                onDismiss = onDismissImports,
                onBg = onBg, muted = muted, accent = accent
            )
        }
    }

    // SESSIONS — this week's, newest first. The list follows the hero's week rather than showing an
    // all-time "recent" list beside a THIS WEEK hero, which said two different things at once.
    val weekEntries = state.entries.filter { it.date >= weekStartMs }
    item("sessions-header") {
        Spacer(Modifier.height(SECTION_GAP))
        EditorialHeader(
            label = "Sessions",
            muted = muted,
            accent = accent,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(8.dp))
    }
    if (weekEntries.isEmpty() && !state.isLoading) {
        item("sessions-empty") {
            // The zero state names the concrete last session rather than a status word (§12) — the
            // hero's all-zero bars already said the week is empty, so this adds the thing you'd ask next.
            val last = state.entries.firstOrNull { it.type != CardioType.REST.code }
            if (last != null) {
                LastSessionLine(
                    entry = last, today = today, zone = zone,
                    muted = muted, accent = accent,
                    onClick = { onOpenSession(last.id) }
                )
            } else {
                InlineEmptyHint(
                    text = "Your first session lands here.",
                    color = muted,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
                )
            }
        }
    }
    items(weekEntries, key = { it.id }) { entry ->
        CardioEntryRow(
            entry = entry,
            today = today,
            useMiles = state.useMiles,
            onRequestDelete = { onRequestDelete(entry.id) },
            onClick = { onOpenSession(entry.id) },
            modifier = forgeItemMotion()
        )
    }
    // §12 overflow — a trim with `view all →` beside it, never a bare link.
    if (state.entries.size > weekEntries.size) {
        item("see-all") {
            SeeAllRow(
                expands = seeAllExpands,
                expanded = state.historyExpanded,
                total = state.entries.size,
                onClick = onSeeAll,
                accent = accent
            )
        }
    }
    // The inline-expand fallback (no History route wired) grows the list in place with the older rows.
    if (seeAllExpands && state.historyExpanded) {
        items(state.entries.filter { it.date < weekStartMs }, key = { it.id }) { entry ->
            CardioEntryRow(
                entry = entry,
                today = today,
                useMiles = state.useMiles,
                onRequestDelete = { onRequestDelete(entry.id) },
                onClick = { onOpenSession(entry.id) },
                modifier = forgeItemMotion()
            )
        }
    }

    // STEPS — the hourly mark cardio owns (`design/MAP.md`), drawn for today. Replaced the hero's bare
    // `TODAY · N STEPS` text line: a data section leads with its mark, not a sentence (§12).
    if (state.todayWearable?.hasData == true || state.stepsConnected) {
        item("steps") {
            Spacer(Modifier.height(SECTION_GAP))
            StepsByHourSection(
                wearable = state.todayWearable,
                connected = state.stepsConnected,
                onBg = onBg, muted = muted, outline = outline, accent = accent
            )
        }
    }
}

/**
 * PROGRESS — where it is going. Pace leads (the live reading), records qualify it, goals (a target
 * ladder) sit last per §4.8.
 *
 * There is deliberately NO weekly-load chart here: it is the same mark the weeks page draws, and a
 * visual that only repeats another screen's answer is cut, not copied (§4.3). The hero's `weeks →`
 * is the way to it.
 */
private fun LazyListScope.progressLens(
    state: CardioUiState,
    onOpenSession: (Long) -> Unit,
    onOpenGoals: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    // PACE — a per-activity pace-over-time chart. It used to ride the week overlay's current page
    // alone; it is cross-week data, so a lens about progress is where it belonged all along.
    if (state.cardioPaceSeries.isNotEmpty()) {
        item("pace") {
            Spacer(Modifier.height(SECTION_GAP))
            CardioPaceTrendSection(
                series = state.cardioPaceSeries,
                useMiles = state.useMiles,
                onBg = onBg, muted = muted, outline = outline, accent = accent
            )
        }
    }

    if (state.cardioRecords.isNotEmpty()) {
        item("records") {
            Spacer(Modifier.height(SECTION_GAP))
            CardioRecordsSection(
                records = state.cardioRecords,
                useMiles = state.useMiles,
                onOpenSession = onOpenSession,
                onBg = onBg, muted = muted, accent = accent, outline = outline
            )
        }
    }

    // GOALS — the cardio-metric goals as the same open progress lines Home draws. Hidden when none
    // exist (Home owns the "set targets" nudge — no duplicate teaser here, §4.3).
    if (state.cardioGoals.isNotEmpty()) {
        item("goals") {
            Spacer(Modifier.height(SECTION_GAP))
            CardioGoalsSection(
                goals = state.cardioGoals,
                onOpenGoals = onOpenGoals,
                onBg = onBg, muted = muted, accent = accent, outline = outline
            )
        }
    }

    // Every mark in this lens needs history to exist. Below that, ONE line naming the concrete unlock
    // rather than four empty shells (§12 — collapse repetition).
    if (state.cardioPaceSeries.isEmpty() &&
        state.cardioRecords.isEmpty() && state.cardioGoals.isEmpty()
    ) {
        item("progress-empty") {
            Spacer(Modifier.height(SECTION_GAP))
            InlineEmptyHint(
                text = "Pace and load read out after your second session.",
                color = muted,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
        }
    }
}

/** The zero-week line: which session was last, and how long ago, as a tap into it. */
@Composable
private fun LastSessionLine(
    entry: CardioEntry,
    today: LocalDate,
    zone: ZoneId,
    muted: Color,
    accent: Color,
    onClick: () -> Unit
) {
    val customs = LocalCardioTypes.current
    val label = remember(entry, today, customs) {
        val date = java.time.Instant.ofEpochMilli(entry.date).atZone(zone).toLocalDate()
        val days = java.time.temporal.ChronoUnit.DAYS.between(date, today).toInt()
        val name = com.forge.app.domain.cardio.CardioActivity.resolve(entry.type, customs).displayName
        val ago = when (days) {
            0 -> "today"
            1 -> "yesterday"
            else -> "$days days ago"
        }
        "Last: $name, $ago"
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("Open your last session", onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Text("open →", style = MaterialTheme.typography.labelMedium, color = accent)
    }
}

@Composable
private fun SeeAllRow(
    expands: Boolean,
    expanded: Boolean,
    total: Int,
    onClick: () -> Unit,
    accent: Color
) {
    // Inline-expand mode toggles "show less"; navigate mode (and collapsed) reads "view all … →".
    val label = if (expands && expanded) "show less ↑" else "view all $total →"
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("View all sessions", onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 14.dp)
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = accent)
    }
}
