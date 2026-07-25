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
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.coach.PersonalProfile
import com.forge.app.ui.common.InlineEmptyHint

/**
 * The project card (Coach v3 D) — "what should I improve?", answered before it's asked, and the
 * personal numbers the coach has learned about this athlete.
 *
 * One project at a time is the discipline, so this section shows exactly one thing: the running
 * project, or the one the coach would propose, or nothing at all when neither exists.
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
    if (project == null && proposal == null && !state.profile.hasPersonalData) return

    item("coach-project") {
        CoachSection(c, title = "Project", index = index) {
            when {
                project != null -> {
                    Text(project.name, style = MaterialTheme.typography.titleSmall, color = c.onBg)
                    Spacer(Modifier.height(4.dp))
                    Text(project.why, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    Spacer(Modifier.height(6.dp))
                    Text(project.plan, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        project.finishLine,
                        style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
                        color = c.muted
                    )
                    Row {
                        CoachAction("Done →", c.accent, "Mark this project finished", onComplete)
                        Spacer(Modifier.width(16.dp))
                        CoachAction("Drop it →", c.muted, "Drop this project", onAbandon)
                    }
                }

                proposal != null -> {
                    Text(proposal.name, style = MaterialTheme.typography.titleSmall, color = c.onBg)
                    Spacer(Modifier.height(4.dp))
                    Text(proposal.why, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    Spacer(Modifier.height(6.dp))
                    Text(proposal.plan, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        proposal.finishLine,
                        style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
                        color = c.muted
                    )
                    CoachAction("Start this →", c.accent, "Start this project", onAccept)
                }

                else -> InlineEmptyHint("Nothing stands out as the biggest lever right now.", c.muted)
            }

            // What the coach has measured about YOU — the numbers that replaced the defaults.
            if (state.profile.hasPersonalData) {
                Spacer(Modifier.height(14.dp))
                ProfileReadout(state.profile, c)
            }
        }
    }
}

@Composable
private fun ProfileReadout(profile: PersonalProfile.Profile, c: CoachColors) {
    Column(Modifier.fillMaxWidth()) {
        Text(
            "YOUR NUMBERS",
            style = MaterialTheme.typography.labelSmall,
            color = c.muted,
            fontSize = 9.sp,
            letterSpacing = 1.5.sp
        )
        Spacer(Modifier.height(6.dp))
        profile.recoveryDays?.let { days ->
            Text(
                "Best spacing: $days ${if (days == 1) "day" else "days"} between sessions",
                style = MaterialTheme.typography.bodySmall,
                color = c.onBg,
                modifier = Modifier.padding(vertical = 2.dp)
            )
        }
        profile.volumeCaps.entries.take(3).forEach { (muscle, cap) ->
            Text(
                "${muscle.displayName}: up to $cap sets a week",
                style = MaterialTheme.typography.bodySmall,
                color = c.onBg,
                modifier = Modifier.padding(vertical = 2.dp)
            )
        }
    }
}
