package com.forge.app.ui.cardio

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.cardio.components.CardioEmptyState
import com.forge.app.ui.cardio.components.CardioEntryRow
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet
import com.forge.app.ui.cardio.components.CardioWatchBanner
import com.forge.app.ui.cardio.components.CardioWeekDetailSheet
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.EditorialHeader
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
        "${isoWeekStart.format(fmt).uppercase()} — ${isoWeekEnd.format(fmt).uppercase()}"
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
            editing = state.editing,
            bodyweightLb = state.bodyweightLb,
            useMiles = state.useMiles,
            onHome = { viewModel.closeSheet(); goHome() }
        )
        sessionEntry != null -> CardioSessionDetailSheet(
            entry = sessionEntry,
            bodyweightLb = state.bodyweightLb,
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
            bodyweightLb = state.bodyweightLb,
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
            weekNum = weekNum,
            weekLabel = weekLabel,
            today = today,
            todayDow = todayDow,
            weekDays = state.cardioDaysThisWeek,
            onBack = onBack,
            onOpenLog = viewModel::openSheet,
            onOpenDetail = viewModel::openDetail,
            onOpenSession = viewModel::openSessionDetail,
            onRequestDelete = viewModel::requestDelete,
            onSeeAll = onOpenHistory ?: viewModel::toggleHistoryExpanded,
            seeAllExpands = onOpenHistory == null,
            onConnectWearable = onConnectWearable,
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
    weekNum: Int,
    weekLabel: String,
    today: LocalDate,
    todayDow: Int,
    weekDays: Int,
    onBack: (() -> Unit)?,
    onOpenLog: () -> Unit,
    onOpenDetail: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onRequestDelete: (Long) -> Unit,
    onSeeAll: () -> Unit,
    seeAllExpands: Boolean,
    onConnectWearable: () -> Unit,
    onDismissHint: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    val isEmpty = state.entries.isEmpty() && !state.isLoading
    // The connect-a-wearable hint only rides along once there's content (the first-run empty state has
    // its own copy), until the user dismisses it for good — and never once a watch is actually connected
    // (steps OR GPS granted), since by then the invite is moot and would just nag.
    val showWearableHint = !isEmpty && !state.wearableHintDismissed &&
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
                actions = {
                    Text(
                        "CARDIO",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 2.sp,
                        color = muted,
                        modifier = Modifier.padding(end = 16.dp)
                    )
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

            if (isEmpty) {
                // Dedicated first-run — its own intro + CTA, no empty hero/week scaffolding behind it.
                item("empty") {
                    CardioEmptyState(
                        onOpenLog = onOpenLog,
                        weekTargetMin = state.weekTargetMin,
                        onBg = onBg, muted = muted, outline = outline
                    )
                }
                return@LazyColumn
            }

            item("hero") {
                CardioHero(
                    weekDays = weekDays,
                    weekMinutes = state.weekMinutes,
                    weekNum = weekNum,
                    weekLabel = weekLabel,
                    onBg = onBg,
                    muted = muted,
                    onOpenDetail = onOpenDetail
                )
            }

            item("week-row") {
                WeekBoxRow(
                    days = state.weekDays,
                    todayDow = todayDow,
                    onBg = onBg,
                    muted = muted,
                    outline = outline,
                    onClick = onOpenDetail
                )
            }

            item("log-action") {
                Spacer(Modifier.height(8.dp))
                LogTodayRow(onOpenLog = onOpenLog, onBg = onBg, muted = muted, outline = outline)
                Spacer(Modifier.height(12.dp))
            }

            item("history-title") {
                Spacer(Modifier.height(8.dp))
                EditorialHairline(
                    outline = outline,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(14.dp))
                EditorialHeader(
                    label = "Recent sessions",
                    muted = muted,
                    accent = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(12.dp))
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
                HorizontalDivider(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    color = outline.copy(alpha = 0.18f)
                )
            }
            if (state.entries.size > HISTORY_PREVIEW) {
                item("see-all") {
                    SeeAllRow(
                        expands = seeAllExpands,
                        expanded = state.historyExpanded,
                        total = state.entries.size,
                        onClick = onSeeAll,
                        onBg = onBg,
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
    onBg: Color,
    muted: Color
) {
    // Inline-expand mode toggles "Show less"/"See all"; navigate mode always reads "See all … →".
    val label = when {
        !expands -> "See all $total sessions"
        expanded -> "Show less"
        else -> "See all $total sessions"
    }
    val trailing = when {
        !expands -> "→"
        expanded -> "↑"
        else -> "→"
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 16.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Text(trailing, style = MaterialTheme.typography.bodyMedium, color = muted)
    }
}
