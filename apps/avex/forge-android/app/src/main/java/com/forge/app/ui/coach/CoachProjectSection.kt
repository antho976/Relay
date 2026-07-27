package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachProject

/**
 * The project section (Coach v3 D) — "what should I improve?", answered before it is asked.
 *
 * One project at a time is the discipline, so this shows exactly one thing: the running project, or
 * the one the coach would propose. It used to stack four paragraphs (name, why, plan, finish line)
 * with no mark at all, which is the prose budget (§4.3) spent four times over. Redrawn: the WHY is
 * the reading and becomes the section's one caption (§4.9), the FINISH LINE becomes a meter — a
 * project is a value against a target like any other (§2②) — and the PLAN, the only part the user
 * acts on, is the content.
 */
internal fun LazyListScope.coachProjectSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onAccept: () -> Unit,
    onComplete: () -> Unit,
    onAbandon: () -> Unit
) {
    val project = state.project
    val proposal = state.projectProposal
    // No project and nothing to propose is not a zero state, it is an absent subject: there is
    // nothing to draw at zero because the concept has no instance yet.
    if (project == null && proposal == null) return

    item("coach-project") {
        CoachSection(
            c,
            title = "Project",
            index = index,
            caption = project?.why ?: proposal?.why
        ) {
            if (project != null) {
                ProjectBody(
                    name = project.name,
                    plan = project.plan,
                    fraction = projectFraction(project),
                    meta = projectWeekLabel(project),
                    c = c
                )
                Row {
                    CoachAction("Done →", c.accent, "Mark this project finished", onComplete)
                    Spacer(Modifier.width(16.dp))
                    CoachAction("Drop it →", c.muted, "Drop this project", onAbandon)
                }
            } else if (proposal != null) {
                ProjectBody(
                    name = proposal.name,
                    plan = proposal.plan,
                    // Nothing has started, so the meter draws its track alone rather than a zero
                    // fill that would read as "no progress" on work not yet begun (§12).
                    fraction = null,
                    meta = "${proposal.weeks} weeks",
                    c = c
                )
                CoachAction("Start this →", c.accent, "Start this project", onAccept)
            }
        }
    }
}

@Composable
private fun ProjectBody(
    name: String,
    plan: String,
    fraction: Float?,
    meta: String,
    c: CoachColors
) {
    Column(Modifier.fillMaxWidth()) {
        Text(name, style = MaterialTheme.typography.titleSmall, color = c.onBg)
        Spacer(Modifier.height(8.dp))
        CoachMeter(fraction, c)
        Spacer(Modifier.height(4.dp))
        CoachChartLabel(meta, c)
        Spacer(Modifier.height(8.dp))
        Text(plan, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
        Spacer(Modifier.height(4.dp))
    }
}

/** How far through its planned weeks the project has run. */
@Composable
private fun projectFraction(project: CoachProject): Float {
    val now = remember { System.currentTimeMillis() }
    val span = project.weeks.coerceAtLeast(1) * WEEK_MS
    return ((now - project.startedAt).toFloat() / span).coerceIn(0f, 1f)
}

/** "Week 2 of 4" — the meter's reading, in weeks the user counts, never a raw timestamp (§11). */
@Composable
private fun projectWeekLabel(project: CoachProject): String {
    val now = remember { System.currentTimeMillis() }
    val weeks = project.weeks.coerceAtLeast(1)
    val elapsed = ((now - project.startedAt) / WEEK_MS).toInt() + 1
    return "Week ${elapsed.coerceIn(1, weeks)} of $weeks"
}

private const val WEEK_MS = 7L * 24 * 60 * 60 * 1000
