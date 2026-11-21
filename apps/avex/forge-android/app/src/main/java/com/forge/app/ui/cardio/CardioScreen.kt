package com.forge.app.ui.cardio

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.cardio.components.CardioLogSheetHost
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet
import com.forge.app.ui.cardio.state.CardioLens
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

/**
 * The Cardio hub (redesign 2026-10-21, "pg-cardio"): the week as three readings over its day bars and
 * a meter toward the weekly mark, the red Log cardio bar (it carries what the watch picked up), then
 * the Week and Progress tabs.
 *
 * A session opens over the hub as its own page, and the log is a sheet over whatever is under it.
 */
@Composable
fun CardioScreen(
    // The hub's back arrow (to Home); null hides it.
    onBack: (() -> Unit)? = null,
    // The top-right history glyph and the Sessions "View all": the unified History page. Null falls
    // back to growing the list in place.
    onOpenHistory: (() -> Unit)? = null,
    // The full Goals screen, from the Progress tab's goals.
    onOpenGoals: () -> Unit = {},
    // The weeks chart, from the hero's "Weeks ›".
    onOpenWeeks: () -> Unit = {},
    // One week's own page (this week's Monday), from the hero's day bars.
    onOpenWeek: (Long) -> Unit = {},
    // Settings → Watch & health data, from the watch invite in the steps slot.
    onConnectWatch: () -> Unit = {},
    viewModel: CardioViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val watchInviteDismissed by viewModel.watchInviteDismissed.collectAsStateWithLifecycle()

    // Re-check the Health Connect grants whenever the screen resumes — the user can connect steps/GPS
    // in Settings (or the HC app) and come back — and re-anchor "this week" (M-15).
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                viewModel.refreshConnection()
                viewModel.refreshWeekAnchor()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Health Connect's per-route consent screen — returns the chosen session's route (or null).
    val routeLauncher = rememberLauncherForActivityResult(
        contract = androidx.health.connect.client.contracts.ExerciseRouteRequestContract()
    ) { route -> viewModel.onRouteConsented(route) }

    val zone = ZoneId.systemDefault()

    // The session whose page is open (if any) — auto-closed if that entry vanishes (deleted).
    val sessionEntry = state.sessionDetailId?.let { id -> state.entries.firstOrNull { it.id == id } }
    LaunchedEffect(state.sessionDetailId, sessionEntry) {
        if (state.sessionDetailId != null && sessionEntry == null) viewModel.closeSessionDetail()
    }
    // The session page is state, not a destination, so Back closes it rather than the whole route.
    BackHandler(enabled = sessionEntry != null && !state.sheetOpen) { viewModel.closeSessionDetail() }

    // The hub leaves composition while a session is up; its scroll position is held here.
    val hubState = rememberSaveableStateHolder()
    if (sessionEntry != null) {
        CardioSessionDetailSheet(
            entry = sessionEntry,
            allEntries = state.entries,
            useMiles = state.useMiles,
            route = state.sessionRoute,
            onShowRoute = state.sessionRouteConsentId?.let { id -> { routeLauncher.launch(id) } },
            wearable = state.sessionWearable,
            wearableConnected = state.stepsConnected,
            hr = state.sessionHr,
            hrAvgBpm = state.sessionHrAvgBpm,
            hrMaxBpm = state.sessionHrMaxBpm,
            watchStats = state.sessionWatch,
            onAdoptWatchStats = viewModel::adoptWatchStats,
            onEdit = { viewModel.editEntry(sessionEntry.id) },
            onDelete = { viewModel.deleteEntry(sessionEntry.id) },
            onBack = viewModel::closeSessionDetail
        )
    } else {
        hubState.SaveableStateProvider("hub") {
            CardioHub(
                state = state,
                zone = zone,
                actions = CardioHubActions(
                    onBack = onBack,
                    onOpenHistory = onOpenHistory ?: viewModel::toggleHistoryExpanded,
                    historyExpands = onOpenHistory == null,
                    onOpenGoals = onOpenGoals,
                    onOpenWeeks = onOpenWeeks,
                    onOpenWeek = onOpenWeek,
                    onOpenLog = viewModel::openSheet,
                    onOpenSession = viewModel::openSessionDetail,
                    onDeleteSession = viewModel::deleteEntry,
                    onSetLens = viewModel::setLens,
                    onConnectWatch = onConnectWatch,
                    onDismissInvite = viewModel::dismissWatchInvite,
                ),
                showWatchInvite = !state.stepsConnected && !watchInviteDismissed,
            )
        }
    }

    if (state.sheetOpen) {
        CardioLogSheetHost(
            onDismiss = viewModel::closeSheet,
            onSave = viewModel::saveEntry,
            onCreateCustom = viewModel::addCustomType,
            editing = state.editing,
            useMiles = state.useMiles,
            lastUsedType = state.lastCardioType,
            recentTypes = state.recentTypes,
            // A new log offers what the watch picked up; an edit is about the entry it opened on.
            watchSessions = if (state.editing == null) state.importSuggestions else emptyList(),
            onHideWatchSessions = viewModel::dismissWatchImports,
        )
    }
}

/** Everything the hub can do, gathered so the page can be drawn from a fake state in a test. */
internal class CardioHubActions(
    val onBack: (() -> Unit)? = null,
    val onOpenHistory: () -> Unit = {},
    /** True when "View all" grows the list in place (no History route wired). */
    val historyExpands: Boolean = false,
    val onOpenGoals: () -> Unit = {},
    val onOpenWeeks: () -> Unit = {},
    val onOpenWeek: (Long) -> Unit = {},
    val onOpenLog: () -> Unit = {},
    val onOpenSession: (Long) -> Unit = {},
    val onDeleteSession: (Long) -> Unit = {},
    val onSetLens: (CardioLens) -> Unit = {},
    val onConnectWatch: () -> Unit = {},
    val onDismissInvite: () -> Unit = {},
)

/** The hub page itself, drawn from [state] alone. */
@Composable
internal fun CardioHub(
    state: CardioUiState,
    zone: ZoneId,
    actions: CardioHubActions,
    showWatchInvite: Boolean,
) {
    // Today comes from the ViewModel's anchor, not from a clock read here (M-15).
    val today = remember(state.todayStartMs, zone) {
        state.todayStartMs.takeIf { it > 0L }
            ?.let { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
            ?: LocalDate.now(zone)
    }
    val weekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val weekStartMs = remember(weekStart, zone) { weekStart.atStartOfDay(zone).toInstant().toEpochMilli() }

    LazyColumn(
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = CARDIO_GUTTER, end = CARDIO_GUTTER, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            CardioTopRow(
                onBack = actions.onBack,
                actionIcon = OpenIcons.History,
                actionLabel = "Session history",
                onAction = actions.onOpenHistory
            )
        }
        item("title") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text("Cardio", style = OpenType.titleL, color = MaterialTheme.colorScheme.onBackground)
                Spacer(Modifier.height(28.dp))
            }
        }
        item("hero") {
            CardioHero(
                state = state,
                weekStart = weekStart,
                todayDow = today.dayOfWeek.value - 1,
                onOpenWeeks = actions.onOpenWeeks,
                onOpenThisWeek = { actions.onOpenWeek(weekStartMs) },
                onOpenLog = actions.onOpenLog,
            )
        }
        item("tabs") {
            Column {
                Spacer(Modifier.height(CARDIO_SECTION))
                OpenTabs(CardioLens.entries, state.lens, { it.label }, actions.onSetLens)
                Spacer(Modifier.height(22.dp))
            }
        }
        when (state.lens) {
            CardioLens.WEEK -> cardioWeekTab(state, today, weekStartMs, zone, actions, showWatchInvite)
            CardioLens.PROGRESS -> item("progress") { CardioProgressTab(state, zone, actions.onOpenGoals) }
        }
    }
}
