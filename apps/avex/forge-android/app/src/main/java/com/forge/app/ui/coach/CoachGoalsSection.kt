package com.forge.app.ui.coach

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
import androidx.compose.ui.unit.dp
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.coach.GoalPortfolio
import com.forge.app.program.Program

/**
 * What the coach is working toward, on the Now lens (Coach v3 A2).
 *
 * A goal is one value against a target, so §2② fixes the mark without a decision: a meter. Each row
 * is its meter with the reading underneath, in the goal's own unit, and the ETA as right-meta — a
 * reading, never a state word (§8). At zero the TRACK still draws, which is the section's own
 * vocabulary at zero rather than a paragraph (§12).
 *
 * It is called "Working toward", not "Goals", because the app has a separate Goals screen backed by
 * a different store; until those merge, two sections called Goals would be two truths with one name.
 */
internal fun LazyListScope.coachGoalsSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onAddGoal: () -> Unit
) {
    item("coach-goals") {
        CoachSection(
            c,
            title = "Working toward",
            index = index,
            // The section's ONE caption (§4.3), and it only speaks when there is nothing to read.
            caption = if (state.goals.isEmpty())
                "Pick what you're chasing and every call names the goal it serves." else null
        ) {
            if (state.goals.isEmpty()) {
                CoachMeter(null, c)
                Spacer(Modifier.height(10.dp))
            } else {
                state.goals.forEach { GoalRow(it, c) }
                // Conflicts speak once, under the rows they are about — the section's one caption
                // when there ARE goals, so it never stacks with the empty-state line.
                state.goalConflicts.firstOrNull()?.let { conflict ->
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "${conflict.explanation} ${conflict.proposal}",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted,
                        fontStyle = FontStyle.Italic
                    )
                }
                Spacer(Modifier.height(6.dp))
            }
            CoachAction("Add a goal →", c.accent, "Add a coach goal", onAddGoal)
        }
    }
}

@Composable
private fun GoalRow(state: GoalPortfolio.GoalState, c: CoachColors) {
    Column(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
            // §14: a goal's subject is an exercise or muscle NAME — user content, never clamped.
            Text(
                goalTitle(state),
                style = MaterialTheme.typography.titleSmall,
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            // Right meta is a reading, never a state word (§8). A reached goal says so with a full
            // meter and its own reading line, so nothing floats on the right.
            state.etaWeeks?.takeIf { it > 0 }?.let { weeks ->
                Spacer(Modifier.width(12.dp))
                Text(
                    "~$weeks wk",
                    style = MaterialTheme.typography.labelMedium,
                    color = c.muted
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        CoachMeter(goalFraction(state), c)
        Spacer(Modifier.height(4.dp))
        Text(state.reading, style = MaterialTheme.typography.bodySmall, color = c.muted)
    }
}

/**
 * Progress toward the target. Null when the goal carries no number to measure against — the meter
 * then draws its track alone rather than a fake zero, since "no target" is not "no progress".
 */
private fun goalFraction(s: GoalPortfolio.GoalState): Float? {
    if (s.reachedNow) return 1f
    val current = s.current ?: return null
    val target = s.target?.takeIf { it > 0.0 } ?: return null
    return (current / target).coerceIn(0.0, 1.0).toFloat()
}

/** "Bench press · 1rm" — the goal in the user's vocabulary, never the enum's (§11). */
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
