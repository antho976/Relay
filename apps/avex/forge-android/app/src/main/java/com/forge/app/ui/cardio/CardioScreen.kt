package com.forge.app.ui.cardio

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.cardio.components.CardioEntryRow
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet
import com.forge.app.ui.cardio.components.CardioWatchBanner
import com.forge.app.ui.cardio.components.CardioWeekDetailSheet
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.state.CardioUiState
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale

/** Most-recent entries shown inline; the rest sit behind "See all" (the full history is still one tap away). */
private const val HISTORY_PREVIEW = 5

@Composable
fun CardioScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    // When set, the "See all" row opens the unified History page (where cardio + workouts merge);
    // null falls back to expanding the list inline.
    onOpenHistory: (() -> Unit)? = null,
    // Tapping the "connect a watch/ring" banner — opens Settings → Recovery to grant the steps/GPS read.
    onConnectWearable: () -> Unit = {},
    // Opens the full Goals screen — from the GOALS trim's header action / lines.
    onOpenGoals: () -> Unit = {},
    viewModel: CardioViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val goHome = com.forge.app.ui.common.LocalGoHome.current

    // Re-check the Health Connect grants whenever the screen resumes — the user can connect steps/GPS
    // in Settings (or the HC app) and come back, and the banner should vanish + the placeholders appear
    // without a manual reload.
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) viewModel.refreshConnection()
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
    val today = LocalDate.now(zone)
    val weekNum = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val isoWeekEnd = isoWeekStart.plusDays(6)
    val weekLabel = remember(weekNum) {
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

    when {
        state.sheetOpen -> CardioLogSheet(
            onDismiss = viewModel::closeSheet,
            onSave = viewModel::saveEntry,
            onCreateCustom = viewModel::addCustomType,
            editing = state.editing,
            useMiles = state.useMiles,
            lastUsedType = state.lastCardioType,
            onHome = { viewModel.closeSheet(); goHome() }
        )
        sessionEntry != null -> CardioSessionDetailSheet(
            entry = sessionEntry,
            allEntries = state.entries,
            useMiles = state.useMiles,
            route = state.sessionRoute, // Matched watch GPS track, once available/consented (else null).
            onShowRoute = state.sessionRouteConsentId?.let { id -> { routeLauncher.launch(id) } },
            wearable = state.sessionWearable, // That day's watch steps (null until loaded / when none).
            wearableConnected = state.stepsConnected, // Show an empty placeholder once connected.
            onEdit = { viewModel.editEntry(sessionEntry.id) },
            onDelete = { viewModel.requestDelete(sessionEntry.id) },
            onBack = viewModel::closeSessionDetail,
            onHome = { viewModel.closeSessionDetail(); goHome() }
        )
        state.detailOpen -> CardioWeekDetailSheet(
            allEntries = state.entries,
            currentWeekStartMs = isoWeekStartMs,
            useMiles = state.useMiles,
            weekTargetMin = state.weekTargetMin,
            cardioStreakDays = state.cardioStreakDays,
            wearable = state.weekWearable, // Today's watch steps on the current-week page (null when none).
            wearableConnected = state.stepsConnected, // Show an empty placeholder once connected.
            todayDow = todayDow,
            zone = zone,
            onOpenSession = viewModel::openSessionDetail,
            onBack = viewModel::closeDetail,
            onHome = { viewModel.closeDetail(); goHome() }
        )
        else -> CardioListContent(
            state = state,
            weekLabel = weekLabel,
            today = today,
            todayDow = todayDow,
            onBack = onBack,
            onOpenLog = viewModel::openSheet,
            onOpenDetail = viewModel::openDetail,
            onOpenSession = viewModel::openSessionDetail,
            onRequestDelete = viewModel::requestDelete,
            onSeeAll = onOpenHistory ?: viewModel::toggleHistoryExpanded,
            seeAllExpands = onOpenHistory == null,
            onConnectWearable = onConnectWearable,
            onOpenGoals = onOpenGoals,
            onDismissHint = viewModel::dismissWearableHint
        )
    }

    if (state.pendingDeleteId != null) {
        AlertDialog(
            onDismissRequest = viewModel::cancelDelete,
            title = { Text("Delete entry?") },
            text = { Text("This can't be undone.") },
            confirmButton = { TextButton(onClick = viewModel::confirmDelete) { Text("Delete") } },
            dismissButton = { TextButton(onClick = viewModel::cancelDelete) { Text("Cancel") } }
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CardioListContent(
    state: CardioUiState,
    weekLabel: String,
    today: LocalDate,
    todayDow: Int,
    onBack: (() -> Unit)?,
    onOpenLog: () -> Unit,
    onOpenDetail: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onRequestDelete: (Long) -> Unit,
    onSeeAll: () -> Unit,
    seeAllExpands: Boolean,
    onConnectWearable: () -> Unit,
    onOpenGoals: () -> Unit,
    onDismissHint: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    // The connect-a-wearable invite rides along until dismissed for good — and never once a watch is
    // actually connected (steps OR GPS granted), since by then it would just nag.
    val showWearableHint = !state.wearableHintDismissed &&
        !(state.stepsConnected || state.routesConnected)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { com.forge.app.ui.common.ForgeWordmark() },
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
            if (showWearableHint) {
                item("watch-hint") {
                    CardioWatchBanner(
                        onConnect = onConnectWearable,
                        onDismiss = onDismissHint,
                        onBg = onBg, muted = muted, outline = outline
                    )
                }
            }

            item("hero") {
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
                    onOpenDetail = onOpenDetail
                )
            }

            // GOALS — the cardio-metric goals as the same open progress lines Home draws, so a
            // distance/minutes target reads identically wherever it shows. Hidden when none exist
            // (Home owns the "set targets" nudge — no duplicate teaser here, §4.3).
            if (state.cardioGoals.isNotEmpty()) {
                item("goals") {
                    Spacer(Modifier.height(28.dp))
                    CardioGoalsSection(
                        goals = state.cardioGoals,
                        onOpenGoals = onOpenGoals,
                        onBg = onBg, muted = muted, accent = accent, outline = outline
                    )
                }
            }

            item("history-title") {
                Spacer(Modifier.height(28.dp))
                // The log affordance — the old white + circle, trimmed, riding the section anchor.
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    EditorialHeader(
                        label = "Recent sessions",
                        muted = muted,
                        accent = accent,
                        modifier = Modifier.weight(1f)
                    )
                    Box(
                        modifier = Modifier.size(44.dp).bounceClick(onClick = onOpenLog),
                        contentAlignment = Alignment.Center
                    ) {
                        Box(
                            modifier = Modifier.size(28.dp).background(onBg, CircleShape),
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                "+",
                                style = MaterialTheme.typography.titleMedium,
                                color = MaterialTheme.colorScheme.background,
                                modifier = Modifier.semantics { contentDescription = "Log cardio" }
                            )
                        }
                    }
                }
                Spacer(Modifier.height(6.dp))
            }
            if (state.entries.isEmpty() && !state.isLoading) {
                item("history-empty") {
                    InlineEmptyHint(
                        text = "Your first session lands here.",
                        color = muted,
                        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
                    )
                }
            }
            // When "See all" routes to History (the merged page), the home list stays capped at 5;
            // only the inline-expand fallback grows the list in place.
            val shown = if (seeAllExpands && state.historyExpanded) state.entries else state.entries.take(HISTORY_PREVIEW)
            items(shown, key = { it.id }) { entry ->
                CardioEntryRow(
                    entry = entry,
                    today = today,
                    useMiles = state.useMiles,
                    onRequestDelete = { onRequestDelete(entry.id) },
                    onClick = { onOpenSession(entry.id) },
                    modifier = forgeItemMotion()
                )
            }
            if (state.entries.size > HISTORY_PREVIEW) {
                item("see-all") {
                    SeeAllRow(
                        expands = seeAllExpands,
                        expanded = state.historyExpanded,
                        total = state.entries.size,
                        onClick = onSeeAll,
                        accent = accent,
                        muted = muted
                    )
                }
            }
        }
    }
}

@Composable
private fun SeeAllRow(
    expands: Boolean,
    expanded: Boolean,
    total: Int,
    onClick: () -> Unit,
    accent: Color,
    muted: Color
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
