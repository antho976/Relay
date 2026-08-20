package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.coach.GoalPortfolio
import com.forge.app.program.Program
import com.forge.app.ui.common.InlineEmptyHint

/**
 * THE PLAN — the one section that answers "toward what?" (Coach v3 A2/C/D, merged 2026-08-20).
 *
 * It used to be three: Goals, Block and Project, each a full §7 section with its own 15sp anchor.
 * Three anchors for one question is what pushed the Now lens past §3's 2–3 viewport budget and
 * flattened its rank — six sections in one frame means none of them leads (§4.8). They fold here
 * into ONE section with the phase rail as its mark and mono micro-labels ranking the groups by
 * SIZE (§6): the 15sp anchor leads, the 9sp group eyebrows step under it.
 *
 * §12: the rail is the zero-shape — unlit with no block running, the same mark it uses when one
 * IS, so starting a block fills in a shape the user has already seen.
 */
internal fun LazyListScope.coachPlanSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onAddGoal: () -> Unit,
    onArchiveGoal: (Long) -> Unit,
    onStartBlock: () -> Unit,
    onEndBlock: () -> Unit,
    onAcceptProject: () -> Unit,
    onCompleteProject: () -> Unit,
    onAbandonProject: () -> Unit
) {
    item("coach-plan") {
        CoachSection(c, title = "The plan", index = index) {
            // ── The block: the mark, and what the next few weeks are FOR ──────
            GroupHeader(
                label = "Block",
                action = if (state.block == null) "Start" else "End",
                contentDescription = if (state.block == null) "Start a training block"
                else "End the training block",
                accent = state.block == null,
                c = c,
                onAction = if (state.block == null) onStartBlock else onEndBlock
            )
            Spacer(Modifier.height(8.dp))
            PhaseRail(state.block, c)
            Spacer(Modifier.height(10.dp))
            BlockLines(state.block, c)

            // ── Goals: what you're chasing, each row its own reading ──────────
            Spacer(Modifier.height(18.dp))
            GroupHeader(
                label = "Goals",
                action = "Add",
                contentDescription = "Add a coach goal",
                accent = true,
                c = c,
                onAction = onAddGoal
            )
            Spacer(Modifier.height(8.dp))
            if (state.goals.isEmpty()) {
                // §12: a goal list has no shape until it has a goal, so this is the lens's one
                // allowed hint, and it names the concrete next step.
                InlineEmptyHint("Pick what you're chasing and every call names the goal it serves.", c.muted)
            } else {
                state.goals.forEach { g -> GoalRow(g, c, onArchiveGoal) }
                // Conflicts speak once, under the rows they're about.
                state.goalConflicts.firstOrNull()?.let { conflict ->
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "${conflict.explanation} ${conflict.proposal}",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted,
                        fontStyle = FontStyle.Italic
                    )
                }
            }

            // ── The project: ONE lever at a time, or nothing ──────────────────
            // "Your numbers" used to close this group; it is what the coach LEARNED about you,
            // so it lives with the rest of the learning on Signals now (§4.3, one home).
            val project = state.project
            val proposal = state.projectProposal
            if (project != null || proposal != null) {
                Spacer(Modifier.height(18.dp))
                GroupHeader(
                    label = "Project",
                    action = if (project != null) "Done" else "Start",
                    contentDescription = if (project != null) "Mark this project finished"
                    else "Start this project",
                    accent = true,
                    c = c,
                    onAction = if (project != null) onCompleteProject else onAcceptProject
                )
                Spacer(Modifier.height(8.dp))
                ProjectBody(
                    name = project?.name ?: proposal!!.name,
                    plan = project?.plan ?: proposal!!.plan,
                    finishLine = project?.finishLine ?: proposal!!.finishLine,
                    c = c
                )
                // Drop stays a plain muted link: it is the destructive sidekick, and §8 keeps a
                // destructive action visually quieter than the one it sits beside.
                if (project != null) {
                    Spacer(Modifier.height(8.dp))
                    CoachAction("Drop it →", c.muted, "Drop this project", onAbandonProject)
                }
            }
        }
    }
}

/**
 * One group inside The plan: its 9sp mono eyebrow with the group's action right-aligned on the
 * same line.
 *
 * The actions used to be `action →` links sitting under each group's prose, at whatever left
 * position the text above them ended — three accent links dribbling down the column, which is
 * exactly the scatter §5 warns about ("never many at postage-stamp size"). Pinning them to the
 * eyebrow's right edge makes one aligned column of actions instead of three interruptions, and
 * the eyebrow says which group each one belongs to (2026-08-20).
 */
@Composable
private fun GroupHeader(
    label: String,
    action: String,
    contentDescription: String,
    accent: Boolean,
    c: CoachColors,
    onAction: () -> Unit
) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        CoachChartLabel(label, c)
        CoachAction("$action →", if (accent) c.accent else c.muted, contentDescription, onAction)
    }
}

/** The running block in words, or the invitation to start one. */
@Composable
private fun BlockLines(
    block: TrainingBlock?,
    c: CoachColors
) {
    if (block == null) {
        // No prose hint here. §12 allows ONE InlineEmptyHint per lens and reserves it for content
        // with no zero-shape — the unlit rail directly above IS this group's drawn zero-state, so
        // the action alone carries it and the lens's one hint goes to Goals, which has no shape at
        // all. Merging three sections into one is what put two hints in a row; this is the cut.
        return
    }
    Text(BlockPlanner.describe(block), style = MaterialTheme.typography.bodyMedium, color = c.onBg)
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
}

/**
 * The project in a title, ONE line and ONE caption.
 *
 * It used to stack four: name, why, plan and finish line — four sentences of near-equal weight,
 * which §4.3 caps at "max ONE muted caption per section" and calls a sentences-only section. On a
 * real device that block read as an undifferentiated wall of text. The `plan` survives because it
 * is the only line that says what to DO; the finish line survives as the caption because a project
 * without one is a complaint. The `why` is cut, not folded (§4.2) — the coach proposing it IS the
 * why, and the reading it was built from lives on Signals.
 */
@Composable
private fun ProjectBody(
    name: String,
    plan: String,
    finishLine: String,
    c: CoachColors
) {
    Text(name, style = MaterialTheme.typography.titleSmall, color = c.onBg)
    Spacer(Modifier.height(6.dp))
    Text(plan, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
    Spacer(Modifier.height(4.dp))
    Text(
        finishLine,
        style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
        color = c.muted
    )
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
            Column(Modifier.weight(1f)) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(RoundedCornerShape(50))
                        .background(if (phase == current) c.accent else c.outline)
                )
                Spacer(Modifier.height(4.dp))
                // The shared 9sp caption idiom rather than a hand-dialed fontSize (§6): four
                // columns share ~73dp on a 360dp screen, so the 10sp labelSmall default clips
                // "ACCUMULATE". The label no longer changes colour with the live phase — the
                // filled bar directly above it already carries that state (§8, never twice).
                CoachChartLabel(phase.displayName, c)
            }
            if (i < BlockPhase.entries.lastIndex) Spacer(Modifier.width(6.dp))
        }
    }
}

/** One goal: the reading in its own unit, with its trajectory as right-meta. */
@Composable
private fun GoalRow(
    state: GoalPortfolio.GoalState,
    c: CoachColors,
    onArchive: (Long) -> Unit
) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // The dot flags the exception only (§8): off track earns a mark, on track is the norm.
        CoachFlagDot(if (state.onTrack == false) c.muted else null)
        Spacer(Modifier.width(7.dp))
        Column(Modifier.weight(1f)) {
            Text(
                goalTitle(state),
                style = MaterialTheme.typography.titleSmall,
                color = c.onBg,
                // §14: user content never clamps to one line — a long goal title must wrap,
                // not truncate, at large font scales. Two lines keeps the row sane.
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
            Text(state.reading, style = MaterialTheme.typography.bodySmall, color = c.muted)
        }
        Spacer(Modifier.width(12.dp))
        // Right meta is a reading, never a state word (§8): the ETA, or nothing.
        state.etaWeeks?.let { weeks ->
            Text(
                if (weeks == 0) "reached" else "~$weeks wk",
                style = MaterialTheme.typography.labelMedium,
                color = c.muted
            )
        }
    }
}

/** "Bench press · 315 lb" — the goal in the user's vocabulary, never the enum's. */
private fun goalTitle(state: GoalPortfolio.GoalState): String {
    val kind = state.kind
    val subject = when (kind.scope) {
        CoachGoalKind.Scope.EXERCISE ->
            Program.exercise(state.goal.targetKey)?.name ?: state.goal.targetKey
        CoachGoalKind.Scope.MUSCLE ->
            com.forge.app.program.MuscleGroup.entries
                .firstOrNull { it.code == state.goal.targetKey }?.displayName ?: state.goal.targetKey
        CoachGoalKind.Scope.BALANCE_PAIR -> when (state.goal.targetKey) {
            "push_pull" -> "Push and pull"
            "quad_ham" -> "Quads and hamstrings"
            else -> state.goal.targetKey
        }
        CoachGoalKind.Scope.NONE -> null
    }
    return subject?.takeIf { it.isNotBlank() }?.let { "$it · ${kind.displayName.lowercase()}" }
        ?: kind.displayName
}
