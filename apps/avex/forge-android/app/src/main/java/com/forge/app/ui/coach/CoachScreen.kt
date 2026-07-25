package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.gym.session.SegmentRow
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings

/** The three reads the Coach page offers, the Stats lens-pill pattern applied to coaching. */
// Lens labels stay ONE short word — "This week" didn't fit the pill row (Antho 2026-07-05).
enum class CoachLens(val label: String) {
    NOW("Now"),
    SIGNALS("Signals"),
    JOURNEY("Journey")
}

/**
 * The Coach page, rebuilt as one editorial screen in the Stats mold: a hero verdict with the
 * week's figures, lens pills, then one interactive deep-dive list per lens. Now holds the
 * proposals, the changes under watch, the countdowns and the milestone ladder; Signals holds
 * what the coach reads, drawn as data; Journey holds trust and the week-by-week record. The
 * old Week Brief, Coach lab and learning timeline screens all live here now.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoachScreen(
    // Null when hosted as a hub pager page (no redundant back arrow); a real callback as a route.
    onBack: (() -> Unit)? = null,
    initialLens: CoachLens = CoachLens.NOW,
    // Lands on Settings → Recovery; the Signals lens's unconnected inputs carry it as "connect →".
    onConnectHealth: (() -> Unit)? = null,
    /** Opens the Academy (Coach v3 B3) — the knowledge layer beside the decisions. */
    onOpenAcademy: (() -> Unit)? = null,
    viewModel: CoachViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit
    val c = rememberCoachColors()
    var lens by rememberSaveable { mutableStateOf(initialLens) }
    /** A2: the add-a-goal tiny input, opened from the Goals section's action line. */
    var showGoalPicker by rememberSaveable { mutableStateOf(false) }

    if (showGoalPicker) {
        GoalPickerDialog(
            onPick = { kind, targetKey, target -> viewModel.addGoal(kind, targetKey, target) },
            onDismiss = { showGoalPicker = false }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: the top bar never names the screen — just the wordmark (home button) and,
                // on a routed entry, the back arrow. The hero verdict is the screen's identity.
                title = { ForgeWordmark() },
                navigationIcon = {
                    if (onBack != null) IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = c.muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            // No spinners (§13): the page appears with its entrance cascade once the read lands.
            state.loading -> Box(Modifier.fillMaxSize().padding(inner))
            state.freestyle -> Box(
                Modifier.fillMaxSize().padding(inner).padding(horizontal = COACH_GUTTER),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "Freestyle logging leaves the coach no plan to watch. Build or generate one and it starts from your first session.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.muted,
                    textAlign = TextAlign.Center
                )
            }
            else -> LazyColumn(
                modifier = Modifier.fillMaxSize().padding(inner),
                contentPadding = PaddingValues(top = 4.dp, bottom = 56.dp)
            ) {
                // ── Hero: the verdict and the week's figures ──────────────────
                item("coach-hero") {
                    Column(Modifier.fillMaxWidth().statsEntrance(0).padding(vertical = 10.dp)) {
                        CoachHero(state, weightUnit, c)
                    }
                }

                // ── Lens pills ────────────────────────────────────────────────
                item("coach-lens") {
                    Column(
                        Modifier.fillMaxWidth().statsEntrance(1)
                            .padding(horizontal = COACH_GUTTER, vertical = 6.dp)
                    ) {
                        SegmentRow(
                            items = CoachLens.entries,
                            isSelected = { it == lens },
                            label = { it.label },
                            onSelect = { lens = it },
                            onBg = c.onBg, muted = c.muted, accent = c.accent, outline = c.outline
                        )
                    }
                }

                when (lens) {
                    CoachLens.NOW -> {
                        // A2: what you're chasing sits above the week's mechanics — "toward what?"
                        // outranks "what changed" (§4.8, lead with the live).
                        coachGoalsSection(
                            state = state,
                            c = c,
                            index = 2,
                            onAddGoal = { showGoalPicker = true },
                            onArchiveGoal = viewModel::archiveGoal,
                            onOpenAcademy = { onOpenAcademy?.invoke() }
                        )
                        // C: what the next few weeks are for, above the week's own mechanics.
                        coachBlockSection(
                            block = state.block,
                            c = c,
                            index = 3,
                            onStart = viewModel::startBlock,
                            onEnd = viewModel::endBlock
                        )
                        // D: the one lever the coach is hunting, plus what it has measured about you.
                        coachProjectSection(
                            state = state,
                            c = c,
                            index = 4,
                            onAccept = viewModel::acceptProject,
                            onComplete = viewModel::completeProject,
                            onAbandon = viewModel::abandonProject
                        )
                        coachNowLens(
                            state = state,
                            c = c,
                            onApply = viewModel::apply,
                            onSkip = viewModel::skip,
                            onUndo = viewModel::undo,
                            onApplyAll = viewModel::applyAll
                        )
                    }
                    CoachLens.SIGNALS -> coachSignalsLens(state, weightUnit, c, onConnectHealth)
                    CoachLens.JOURNEY -> coachJourneyLens(state, c)
                }
            }
        }
    }
}
