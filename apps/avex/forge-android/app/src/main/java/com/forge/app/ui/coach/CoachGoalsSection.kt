package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.coach.GoalPortfolio
import com.forge.app.program.Program
import com.forge.app.ui.common.InlineEmptyHint

/**
 * GOALS on the Now lens (Coach v3 A2) — what the coach is actually working toward, above the
 * week's mechanics, because "am I making progress, toward what?" outranks "what changed this week".
 *
 * Data-led per §4.3: each row is a reading (the number, in the goal's own unit) with its trajectory
 * as right-meta. No mechanics narration; the conflict line is the one caption the section spends.
 */
internal fun LazyListScope.coachGoalsSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onAddGoal: () -> Unit,
    onArchiveGoal: (Long) -> Unit,
    onOpenAcademy: () -> Unit = {}
) {
    item("coach-goals") {
        CoachSection(c, title = "Goals", index = index) {
            if (state.goals.isEmpty()) {
                // §12: nothing to draw at zero here — a goal list has no shape until it has a goal,
                // so this is the lens's one allowed hint, and it names the concrete next step.
                InlineEmptyHint("Pick what you're chasing and every call names the goal it serves.", c.muted)
                Spacer(Modifier.height(4.dp))
            } else {
                state.goals.forEach { g ->
                    GoalRow(g, c, onArchiveGoal)
                }
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
            CoachAction("Add a goal →", c.accent, "Add a coach goal", onAddGoal)
            // The knowledge half of the coach lives one tap from its decisions (B3).
            CoachAction(
                if (state.newLessons > 0) "Academy · ${state.newLessons} new →" else "Academy →",
                c.accent,
                "Open the Academy",
                onOpenAcademy
            )
        }
    }
}

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
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                state.reading,
                style = MaterialTheme.typography.bodySmall,
                color = c.muted
            )
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
