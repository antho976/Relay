/*
 * THESIS: The coaching relationship as one running account, newest first, where an open proposal
 * and a five-week-old outcome are the same kind of object. It refuses the three-lens dashboard
 * that split "what changed", "why", and "did it work" across three taps and left no screen able
 * to answer how training is going.
 *
 * OWN-WORLD: Avex's warm near-black ground and its three type voices, plus this surface's own
 * vocabulary — a spine running the left margin as the time axis, a node per entry carrying its
 * lifecycle, and a stamp carrying its outcome. Every passive entry is bare; a filled body is spent
 * only on an entry still asking the user for something, so the fills on screen ARE the decisions
 * owed, and a week that asks nothing has none.
 *
 * STORY: The user sees what the coach is asking for now, understands it from evidence attached to
 * the ask itself, decides, and can keep scrolling into everything it has ever done and whether it
 * worked.
 *
 * FIRST VIEWPORT: THIS WEEK on the spine, then the call as a filled tile — the lift at the title
 * rung, the change itself at the serif rung, the reason under it, the evidence at full width, and
 * Apply paired with Skip. No count, no inbox, no lens pills.
 *
 * FORM: The Ledger, first on the ordered grounded list; surface seed key cacfe66a.
 *
 * FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the
 * verdict, DESIGN.md, and every shipping raster carrying its provenance.
 */
package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.forge.app.domain.units.WeightUnit
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * Where a deep link into the Coach page lands.
 *
 * The page used to be three lenses and these were the lens ids. There are no lenses now, so an
 * old "what it's watching" or "learning timeline" link resolves to a scroll position on the one
 * column instead of a tab — the content it pointed at is on screen either way.
 */
enum class CoachEntryPoint { ACCOUNT, WHERE_YOU_STAND }

/**
 * The Coach page as one running account.
 *
 * Reading order is the account's own: this week's calls, each carrying the evidence it was made
 * from and its own Apply; then every week before it, stamped with what became of it; then the
 * live reading the coach works from; then the longer arcs; then the standing balance of what it
 * has learned. One column, no lenses, nothing folded behind a tap.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoachScreen(
    // Null when hosted as a hub pager page (no redundant back arrow); a real callback as a route.
    onBack: (() -> Unit)? = null,
    entryPoint: CoachEntryPoint = CoachEntryPoint.ACCOUNT,
    // Lands on Settings → Recovery; the unconnected inputs carry it as a Connect pill.
    onConnectHealth: (() -> Unit)? = null,
    viewModel: CoachViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit
    val c = rememberCoachColors()
    val listState = rememberLazyListState()
    // One clock for the whole page, so every countdown and stamp on it agrees.
    val now = remember(state.brief, state.timeline) { System.currentTimeMillis() }

    // A deep link that used to open the Signals lens scrolls to the reading it meant, once, after
    // the first read lands. Everything else opens at the top of the account.
    LaunchedEffect(state.loading, entryPoint) {
        if (!state.loading && entryPoint == CoachEntryPoint.WHERE_YOU_STAND) {
            listState.scrollToItem(accountItemCount(state))
        }
    }

    Scaffold(
        topBar = {
            // The top bar never names the screen — just, on a routed entry, the back arrow. Hosted
            // as a hub pager page there is no back arrow and no action, so the bar has NO content
            // and is not drawn at all: an empty app bar above the account is a void to scroll past.
            if (onBack != null) {
                TopAppBar(
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(
                                Icons.AutoMirrored.Filled.ArrowBack,
                                contentDescription = "Back",
                                tint = c.muted
                            )
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            // No spinners: the local read is instant, so the page appears with its entrance cascade.
            state.loading -> Box(Modifier.fillMaxSize().padding(inner))
            state.freestyle -> CoachMessage(
                "Freestyle logging leaves the coach no plan to watch. Build or generate one and it " +
                    "starts from your first session.",
                c,
                Modifier.padding(inner)
            )
            // Both reads failed: a load error, not a fresh account.
            state.brief == null && state.watch == null -> CoachMessage(
                "Couldn't read the coach's notes right now. Come back in a bit.",
                c,
                Modifier.padding(inner)
            )
            else -> CoachLedger(
                state = state,
                weightUnit = weightUnit,
                now = now,
                actions = CoachActions(
                    apply = viewModel::apply,
                    skip = viewModel::skip,
                    undo = viewModel::undo,
                    applyAll = viewModel::applyAll,
                    startBlock = viewModel::startBlock,
                    endBlock = viewModel::endBlock,
                    connectHealth = onConnectHealth
                ),
                listState = listState,
                modifier = Modifier.padding(inner)
            )
        }
    }
}

/**
 * Everything the page can do, gathered so the column takes one parameter instead of many.
 *
 * Goals and projects are deliberately absent: their engine, repositories and ViewModel actions all
 * remain, but nothing on the Coach page renders them any more (Antho, 2026-08-20). `GoalPickerDialog`
 * and `CoachViewModel.addGoal` / `startProject` are kept for whichever surface picks them up next.
 */
internal data class CoachActions(
    val apply: (Long) -> Unit = {},
    val skip: (Long) -> Unit = {},
    val undo: (Long) -> Unit = {},
    val applyAll: (String) -> Unit = {},
    val startBlock: () -> Unit = {},
    val endBlock: () -> Unit = {},
    val connectHealth: (() -> Unit)? = null
)

/**
 * The ledger column itself, free of the ViewModel so the golden screenshots render the same thing
 * the app does rather than a test's own copy of it.
 */
@Composable
internal fun CoachLedger(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    now: Long,
    actions: CoachActions,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState()
) {
    val c = rememberCoachColors()
    val statusBar = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        // Hosted as a hub pager page there is no app bar to hold the status bar off the content,
        // so the account was opening hard against the clock. It reserves that space itself.
        contentPadding = PaddingValues(top = statusBar + 20.dp, bottom = 64.dp)
    ) {
        coachAccount(
            state = state,
            weightUnit = weightUnit,
            c = c,
            now = now,
            onApply = actions.apply,
            onSkip = actions.skip,
            onUndo = actions.undo,
            onApplyAll = actions.applyAll
        )
        coachStand(
            state = state,
            weightUnit = weightUnit,
            c = c,
            onConnectHealth = actions.connectHealth
        )
        coachBlock(
            state = state,
            c = c,
            onStartBlock = actions.startBlock,
            onEndBlock = actions.endBlock
        )
        coachInputs(
            state = state,
            c = c,
            onConnectHealth = actions.connectHealth
        )
        coachUnlocks(state = state, c = c)
        coachLearned(state = state, c = c)
    }
}

/** A whole-page state that has nothing to account for, said once and centred. */
@Composable
private fun CoachMessage(text: String, c: CoachColors, modifier: Modifier = Modifier) {
    Box(
        modifier.fillMaxSize().padding(horizontal = COACH_GUTTER),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = c.muted,
            textAlign = TextAlign.Center
        )
    }
}
