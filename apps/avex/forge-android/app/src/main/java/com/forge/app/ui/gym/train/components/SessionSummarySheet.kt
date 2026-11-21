package com.forge.app.ui.gym.train.components

import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.common.ForgePrimaryCapsule

import androidx.compose.foundation.layout.height

import androidx.compose.foundation.layout.Spacer

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.notify.PrMilestone
import com.forge.app.domain.units.formatVolume
import com.forge.app.service.ForgeNotifications
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.gym.train.state.SessionSummary

/**
 * The end-of-session summary. Stripped to five reads: any new PRs (hidden when none), the general
 * stats + time, a recap of what you worked (muscle map + exercise list), and the coach's corner —
 * what he saw and the effort data he'd like more of. [onDismiss] keeps its tags/mood params for the
 * shared event signature, but both are now always empty; the mid-session journal flows straight
 * through so it's still persisted on COMPLETE.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionSummarySheet(
    summary: SessionSummary,
    onDismiss: (mood: Mood?, tags: List<String>, journal: String) -> Unit
) {
    // Block swipe-to-dismiss: the only way out is the COMPLETE button, otherwise the sheet can be
    // swiped away while the summary is still "open", leaving FINISH dead.
    val sheetState = rememberModalBottomSheetState(
        skipPartiallyExpanded = true,
        confirmValueChange = { it != SheetValue.Hidden }
    )

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background
    val accent = MaterialTheme.colorScheme.primary
    val weightUnit = LocalForgeSettings.current.weightUnit
    val context = LocalContext.current
    val hapticStrength = LocalForgeSettings.current.hapticStrength

    // A PR is the peak of a session — celebrate it once with confetti + a single haptic. Both are
    // rememberSaveable so a rotation while the sheet is open doesn't replay them.
    val hasPr = summary.prCount > 0
    var showConfetti by rememberSaveable { mutableStateOf(hasPr) }
    val view = androidx.compose.ui.platform.LocalView.current
    var prHapticFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!prHapticFired && hasPr) {
            // Through the strength setting: Light and Medium played the same LongPress as Strong.
            view.forgeHaptic(com.forge.app.ui.common.ForgeHapticType.PR_OR_FINISH, hapticStrength)
            prHapticFired = true
        }
    }
    // PR-milestone push (#13): a finished session crossing a lifetime-PR round number drops a
    // celebratory notification in the shade. Own channel; fired once, only on a genuine milestone.
    var prMilestoneFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!prMilestoneFired) {
            PrMilestone.check(summary.lifetimePrCount, summary.prCount)?.let { nudge ->
                ForgeNotifications.postPrMilestone(context, nudge.title, nudge.body)
            }
            prMilestoneFired = true
        }
    }

    ModalBottomSheet(
        onDismissRequest = {},
        sheetState = sheetState,
        containerColor = bg
    ) {
        Box {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 24.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                // Header: what it was, in the redesign's grammar (mono label → serif name).
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    com.forge.app.ui.common.OpenLabel("Workout complete")
                    Text(summary.displayName, style = com.forge.app.ui.theme.OpenType.figureM, color = onBg)
                }

                // The session's figures, two by two: serif figures over mono labels. New PRs lead
                // when there are any (the celebration), else volume does.
                val figures = buildList<@Composable (Modifier) -> Unit> {
                    if (hasPr) add { m -> CountUpStat(summary.prCount.toDouble(), if (summary.prCount == 1) "New PR" else "New PRs", m) { it.toInt().toString() } }
                    add { m -> CountUpStat(summary.totalVolumeLb, "Volume", m) { formatVolume(it, weightUnit) } }
                    add { m -> FlatStat("${summary.durationMinutes} min", "Time", m) }
                    add { m -> FlatStat("${summary.setCount}", if (summary.setCount == 1) "Set" else "Sets", m) }
                    add { m ->
                        FlatStat(
                            "${summary.exercisesLogged}",
                            (if (summary.exercisesLogged == 1) "Exercise" else "Exercises") +
                                (if (summary.exercisesSkipped > 0) " · ${summary.exercisesSkipped} skipped" else ""),
                            m
                        )
                    }
                }
                Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
                    figures.chunked(2).forEach { pair ->
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                            pair.forEach { it(Modifier.weight(1f)) }
                            if (pair.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                }

                // Recap — what you worked on: the muscle map + the exercise list.
                if (summary.setsByMuscle.isNotEmpty() || summary.highlights.isNotEmpty()) {
                    Spacer(Modifier.height(12.dp))
                    com.forge.app.ui.common.OpenLabel("What you worked")
                    if (summary.setsByMuscle.isNotEmpty()) {
                        BodyHeatmap(
                            setsByMuscle = summary.setsByMuscle,
                            accent = accent,
                            faint = outline.copy(alpha = 0.35f),
                            silhouette = outline.copy(alpha = 0.25f),
                            labelColor = muted,
                            modifier = Modifier.fillMaxWidth().statsEntrance(0)
                        )
                    }
                    summary.highlights.forEach { h -> HighlightRow(h) }
                }

                // Coach — what he read from this session, and the effort data he'd like more of.
                CoachReadSection(
                    coachOpinion = summary.coachOpinion,
                    setsWithRpe = summary.setsWithRpe,
                    totalSets = summary.setCount,
                    exercisesRated = summary.exercisesRated,
                    exercisesLogged = summary.exercisesLogged,
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                // Complete button — only way to dismiss. Tags/mood are gone; the mid-session journal
                // flows straight through so it's still persisted on finish.
                // The sheet's one do-it-now action: the red primary, as on every page.
                com.forge.app.ui.common.OpenPrimaryButton(
                    "Complete",
                    onClick = { onDismiss(null, emptyList(), summary.initialJournal) },
                    icon = com.forge.app.ui.theme.OpenIcons.Check,
                    modifier = Modifier.padding(top = 12.dp, bottom = 16.dp)
                )
            }

            // PR confetti: fires once when the sheet opens on a session with a new PR.
            if (showConfetti) {
                ConfettiOverlay(
                    modifier = Modifier.matchParentSize(),
                    onComplete = { showConfetti = false }
                )
            }
        }
    }
}
