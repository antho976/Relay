package com.forge.app.ui.coach

import androidx.compose.foundation.background
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.ui.common.InlineEmptyHint

/**
 * The block card on the Now lens (Coach v3 C) — where the coach stops being purely reactive and
 * says what the next few weeks are FOR.
 *
 * §12: the zero state is drawn, not written. With no block running the section shows the phase rail
 * unlit, which is the same mark it uses when one IS running, so starting a block fills in a shape
 * the user has already seen.
 */
internal fun LazyListScope.coachBlockSection(
    block: TrainingBlock?,
    c: CoachColors,
    index: Int,
    onStart: () -> Unit,
    onEnd: () -> Unit
) {
    item("coach-block") {
        CoachSection(c, title = "Block", index = index) {
            PhaseRail(block, c)
            Spacer(Modifier.height(10.dp))
            if (block == null) {
                InlineEmptyHint(
                    "A block gives the next few weeks one intent, and schedules the deload instead of waiting for one.",
                    c.muted
                )
                CoachAction("Start a block →", c.accent, "Start a training block", onStart)
            } else {
                Text(
                    BlockPlanner.describe(block),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg
                )
                if (block.intent.isNotBlank()) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        block.intent,
                        style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
                        color = c.muted
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
 * The four phases as a segmented rail, with the live one filled — one mark that works at zero
 * (all segments unlit) and reads at a glance when running (§4.10's "rail, not a checklist").
 */
@Composable
private fun PhaseRail(block: TrainingBlock?, c: CoachColors) {
    val current = block?.let { BlockPhase.fromCode(it.phase) }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        BlockPhase.entries.forEachIndexed { i, phase ->
            val live = phase == current
            Column(Modifier.weight(1f)) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(RoundedCornerShape(50))
                        .background(if (live) c.accent else c.outline)
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    phase.displayName.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (live) c.onBg else c.muted,
                    fontSize = 8.sp,
                    letterSpacing = 0.8.sp
                )
            }
            if (i < BlockPhase.entries.lastIndex) Spacer(Modifier.width(6.dp))
        }
    }
}
