package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner

/**
 * The block section on the Now lens (Coach v3 C) — where the coach stops being purely reactive and
 * says what the next few weeks are FOR.
 *
 * §12: the zero state is drawn, not written. With no block running the section shows the phase rail
 * unlit — the same mark it uses when one IS running — so starting a block fills in a shape the user
 * has already seen. The rail is also the explanation: the two-line "what a block is" narration that
 * used to sit here was mechanics narration, which §4.3 says is cut rather than trimmed.
 */
internal fun LazyListScope.coachBlockSection(
    block: TrainingBlock?,
    c: CoachColors,
    index: Int,
    onStart: () -> Unit,
    onEnd: () -> Unit
) {
    item("coach-block") {
        CoachSection(
            c,
            title = "Block",
            index = index,
            // The section's ONE caption, and only while there is no block to read (§4.3). It names
            // the arc, which is what the unlit rail cannot say on its own.
            caption = if (block == null) "Four phases: accumulate, intensify, peak, deload." else null
        ) {
            PhaseRail(block, c)
            Spacer(Modifier.height(8.dp))
            if (block == null) {
                CoachAction("Start a block →", c.accent, "Start a training block", onStart)
            } else {
                val phase = BlockPhase.fromCode(block.phase)
                if (phase != null) {
                    CoachChartLabel(phase.displayName, c)
                    Spacer(Modifier.height(6.dp))
                }
                Text(
                    BlockPlanner.describe(block),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg
                )
                if (block.intent.isNotBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        block.intent,
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted,
                        fontStyle = FontStyle.Italic
                    )
                }
                if (BlockPlanner.isTestWeek(block)) {
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Test week: take one heavy top set on your focus lift and log it honestly.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.onBg
                    )
                }
                CoachAction("End this block →", c.muted, "End the training block", onEnd)
            }
        }
    }
}

/**
 * The four phases as a segmented rail, read as PROGRESS rather than a single lit box: the phases
 * already served take the weaker accent rung, the live one takes full accent, the rest stay track.
 * One mark that works at zero (all segments unlit) and reads at a glance when running (§4.10).
 */
@Composable
private fun PhaseRail(block: TrainingBlock?, c: CoachColors) {
    val current = block?.let { BlockPhase.fromCode(it.phase) }
    val currentIndex = current?.ordinal ?: -1
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        BlockPhase.entries.forEachIndexed { i, _ ->
            Box(
                Modifier
                    .weight(1f)
                    .height(4.dp)
                    .clip(RoundedCornerShape(50))
                    .background(
                        when {
                            i == currentIndex -> c.accent
                            i < currentIndex -> c.secondary
                            // The unlit rail IS the mark here, so it takes §12's empty rung rather
                            // than the bar-track one, which is invisible on near-black
                            // (`design/FAILURES.md`, *Invisible ghost*).
                            else -> c.muted.copy(alpha = 0.30f)
                        }
                    )
            )
            if (i < BlockPhase.entries.lastIndex) Spacer(Modifier.width(6.dp))
        }
    }
}
