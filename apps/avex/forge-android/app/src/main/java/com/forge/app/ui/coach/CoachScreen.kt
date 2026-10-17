/*
 * The Coach page, rebuilt 2026-10-17 (Antho: "needs a tear down and a remake ... I want a full
 * redesign"). Direction: a Today panel, then sections, on the app's grouped-surface kit. The surface
 * brief under .impeccable/surfaces/ holds the direction contract.
 *
 * It replaced the ledger: one grey column on a spine where this week's call, last month's outcome,
 * the readings and the block were all the same weight of text. Now the page answers in the order a
 * person asks: what today means for my targets, what the coach wants this week and the trend it
 * read, and whether its calls have held. Detailed adds the instruments behind those answers.
 */
package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.repeatOnLifecycle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * Where a deep link into the Coach page lands. Both open at the top now: the Today panel IS the
 * live reading the old "what it's watching" link pointed at. Kept so the routes still resolve.
 */
enum class CoachEntryPoint { ACCOUNT, WHERE_YOU_STAND }

/**
 * The Coach page.
 *
 * Simple (the default) is Today, This week, the Record and what unlocks next: everything the Monday
 * ritual needs. Detailed adds the instruments: each lift's trend, the training block, the inputs
 * the coach reads and what it has learned. The switch sits under the title and writes the same
 * preference as Settings → Coach → Advanced tracking; the coach behaves the same either way.
 */
@Composable
fun CoachScreen(
    // The top-left back arrow: to Home as a hub page, pop as a route.
    onBack: (() -> Unit)? = null,
    @Suppress("UNUSED_PARAMETER") entryPoint: CoachEntryPoint = CoachEntryPoint.ACCOUNT,
    // Lands on Settings → Recovery; an unconnected input carries it as a Connect pill.
    onConnectHealth: (() -> Unit)? = null,
    viewModel: CoachViewModel = hiltViewModel(),
    isVisible: Boolean = true
) {
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(viewModel, lifecycle, isVisible) {
        if (isVisible) lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.RESUMED) {
            viewModel.refreshWhileVisible()
        }
    }
    val state by viewModel.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit
    val c = rememberCoachColors()
    // One clock for the whole page, so every countdown and stamp on it agrees.
    val now = remember(state.brief, state.timeline) { System.currentTimeMillis() }

    Scaffold(
        topBar = { ForgeTopBar(onBack = onBack) },
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
            else -> CoachPage(
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
                    connectHealth = onConnectHealth,
                    setAdvanced = viewModel::setAdvanced
                ),
                modifier = Modifier.padding(inner)
            )
        }
    }
}

/**
 * Everything the page can do, gathered so the column takes one parameter instead of many.
 *
 * Goals and projects are deliberately absent: their engine, repositories and ViewModel actions all
 * remain, but nothing on the Coach page renders them (Antho, 2026-08-20).
 */
internal data class CoachActions(
    val apply: (Long) -> Unit = {},
    val skip: (Long) -> Unit = {},
    val undo: (Long) -> Unit = {},
    val applyAll: (String) -> Unit = {},
    val startBlock: () -> Unit = {},
    val endBlock: () -> Unit = {},
    val connectHealth: (() -> Unit)? = null,
    val setAdvanced: (Boolean) -> Unit = {}
)

/**
 * The page column, free of the ViewModel so the golden screenshots render exactly what the app does.
 */
@Composable
internal fun CoachPage(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    now: Long,
    actions: CoachActions,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState()
) {
    val c = rememberCoachColors()
    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(top = 4.dp, bottom = 64.dp)
    ) {
        item("head") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(0)) {
                Text(
                    "Coach",
                    style = MaterialTheme.typography.headlineLarge,
                    color = c.onBg,
                    modifier = Modifier.semantics { heading() }
                )
                Spacer(Modifier.height(16.dp))
                ForgeSlidingSegments(
                    options = listOf("Simple", "Detailed"),
                    selectedIndex = if (state.advanced) 1 else 0,
                    onSelect = { actions.setAdvanced(it == 1) }
                )
            }
        }
        coachToday(state = state, c = c)
        coachThisWeek(
            state = state,
            weightUnit = weightUnit,
            c = c,
            now = now,
            onApply = actions.apply,
            onSkip = actions.skip,
            onUndo = actions.undo,
            onApplyAll = actions.applyAll
        )
        if (state.advanced) {
            coachLifts(state = state, weightUnit = weightUnit, c = c)
            coachBlock(state = state, c = c, onStartBlock = actions.startBlock, onEndBlock = actions.endBlock)
        }
        coachRecord(state = state, c = c, now = now, onUndo = actions.undo)
        coachUnlocks(state = state, c = c)
        if (state.advanced) {
            coachInputs(state = state, c = c, onConnectHealth = actions.connectHealth)
            coachLearned(state = state, c = c)
        }
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
