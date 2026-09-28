package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.ui.common.statsEntrance

/**
 * THE BLOCK — which phase the next few weeks are in, and what they are for.
 *
 * It sits under the account and the live readings: the week's calls are what you came for, the
 * signals are what they were made from, and the block is the arc they both run inside.
 */
internal fun LazyListScope.coachBlock(
    state: CoachViewModel.UiState,
    c: CoachColors,
    onStartBlock: () -> Unit,
    onEndBlock: () -> Unit
) {
    item("block") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(2)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor("Block", c)
            Spacer(Modifier.height(18.dp))
            // The week is the reading, so it takes the figure; what the week is FOR sits under it.
            state.block?.let { block ->
                CoachFigure(
                    "Week ${block.weekIndex} of ${block.plannedWeeks}",
                    BlockPlanner.purpose(block),
                    c
                )
                Spacer(Modifier.height(16.dp))
            }
            // The rail is the zero shape: unlit with no block running, the same mark it uses when
            // one is, so starting a block fills in a shape already seen.
            PhaseRail(state.block, c)
            Spacer(Modifier.height(14.dp))
            state.block?.let { block ->
                if (block.intent.isNotBlank()) {
                    Text(
                        block.intent,
                        style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
                        color = c.muted
                    )
                }
                if (BlockPlanner.isTestWeek(block)) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Test week: take one heavy top set on your focus lift and log it honestly.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.onBg
                    )
                }
                Spacer(Modifier.height(6.dp))
            }
            // Inert while a start or end is in flight (M-13): the second of two quick taps used to
            // race the first for the singleton live row. Muted for that moment, so it reads as taken.
            CoachAction(
                if (state.block == null) "Start a block →" else "End the block →",
                if (state.block == null && !state.blockBusy) c.accent else c.muted,
                if (state.block == null) "Start a training block" else "End the training block"
            ) {
                if (!state.blockBusy) {
                    if (state.block == null) onStartBlock() else onEndBlock()
                }
            }
        }
    }
}

/**
 * WHAT UNLOCKS — the rungs still ahead, each with the real thing that opens it.
 *
 * Three of them while the coach is still building its baseline (there is no history to summarise
 * yet, so the page turns forward), one once it is calling.
 */
internal fun LazyListScope.coachUnlocks(
    state: CoachViewModel.UiState,
    c: CoachColors
) {
    val ahead = state.timeline?.milestones.orEmpty().filterNot { it.reached }
    if (ahead.isEmpty()) return
    val rungs = if ((state.brief?.sessionsToGo ?: 0) > 0) 3 else 1

    item("unlocks") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(4)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor(
                if (rungs > 1) "What unlocks" else "Next",
                c,
                meta = "${ahead.size} AHEAD"
            )
            ahead.take(rungs).forEach { rung ->
                Spacer(Modifier.height(14.dp))
                Text(rung.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                if (rung.detail.isNotBlank() && !rung.detail.equals(rung.label, true)) {
                    Text(rung.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
        }
    }
}

/**
 * The four phases on one thin rail: the phases already run in the 0.6 accent rung, the live one in
 * full accent, the ones ahead on the empty track. Labels in sentence case under their own stretch,
 * the live one in ink. It was four 6dp capsules with mono caps under each, which read as a tab bar
 * rather than as time passing. Works unlit at zero.
 */
@Composable
private fun PhaseRail(block: TrainingBlock?, c: CoachColors) {
    val current = block?.let { BlockPhase.fromCode(it.phase) }
    val currentIndex = current?.let { BlockPhase.entries.indexOf(it) } ?: -1
    Row(
        Modifier.fillMaxWidth().semantics {
            contentDescription = current?.let { "Training block phase: ${it.displayName}" }
                ?: "No training block running"
        },
        verticalAlignment = Alignment.Top
    ) {
        BlockPhase.entries.forEachIndexed { i, phase ->
            Column(Modifier.weight(1f)) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(3.dp)
                        .clip(RoundedCornerShape(50))
                        .drawBehind {
                            drawRect(
                                when {
                                    i == currentIndex -> c.accent
                                    i < currentIndex -> c.secondary
                                    else -> c.track
                                }
                            )
                        }
                )
                Spacer(Modifier.height(8.dp))
                // Four columns share ~73dp on a 360dp screen, so a long phase name WRAPS rather
                // than truncating.
                Text(
                    phase.displayName,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (i == currentIndex) c.onBg else c.muted
                )
            }
            if (i < BlockPhase.entries.lastIndex) Spacer(Modifier.width(4.dp))
        }
    }
}
