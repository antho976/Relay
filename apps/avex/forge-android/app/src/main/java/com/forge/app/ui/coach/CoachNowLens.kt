package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.statsEntrance
import kotlin.math.ceil

/**
 * The Now lens, and the whole of it — one emitter, so section order and the entrance cascade cannot
 * disagree. They used to: the lens was assembled from four call sites that each numbered their own
 * `statsEntrance`, so six sections shared three stagger steps and one lens cascaded out of order.
 *
 * Ordered by what is live (§4.8, placement is rank): the week's call with its evidence and actions,
 * the changes still under watch, then what the training is FOR (goals, block, project), then the
 * pure countdowns last. The setup prompts used to open the lens, which meant a new user met three
 * things to configure before a single reading.
 */
internal fun LazyListScope.coachNowLens(
    state: CoachViewModel.UiState,
    c: CoachColors,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    onApplyAll: (String) -> Unit,
    onAddGoal: () -> Unit,
    onStartBlock: () -> Unit,
    onEndBlock: () -> Unit,
    onAcceptProject: () -> Unit,
    onCompleteProject: () -> Unit,
    onAbandonProject: () -> Unit,
    onOpenAcademy: () -> Unit
) {
    coachCallSection(state, c, index = 2, onApply, onSkip, onUndo, onApplyAll)
    coachWatchSection(state, c, index = 3)
    coachGoalsSection(state, c, index = 4, onAddGoal = onAddGoal)
    coachBlockSection(state.block, c, index = 5, onStart = onStartBlock, onEnd = onEndBlock)
    coachProjectSection(state, c, index = 6, onAcceptProject, onCompleteProject, onAbandonProject)
    coachComingUpSection(state, c, index = 7)
    coachAcademyLink(state, c, index = 8, onOpenAcademy)
}

/**
 * The week's call: every decision in the open with its evidence and its actions. Nothing folds
 * (§4.2); holds and errors speak once, in the hero.
 *
 * At zero it renders ONLY while the coach is still learning, where it names the gate the hero's
 * BASELINE figure counts toward — the caption belongs on the thing it gates (§4.3). Once the
 * baseline is in and a week simply produced nothing, the hero's verdict and aside already say so,
 * and a section here would be pure echo (`design/FAILURES.md`, *Mark echo*), so it stands down.
 */
private fun LazyListScope.coachCallSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    onApplyAll: (String) -> Unit
) {
    val brief = state.brief
    val decisions = brief?.decisions.orEmpty()
    val learning = brief == null || brief.sessionsToGo > 0
    if (decisions.isEmpty() && !learning) return

    item("coach-call") {
        CoachSection(c, title = "The call", index = index) {
            if (decisions.isEmpty()) {
                val needed = state.watch?.minSessions
                    ?: state.brief?.minSessions?.takeIf { it > 0 }
                    ?: AutoCoachPlanner.MIN_SESSIONS
                // The lens's ONE hint (§12): a call that does not exist yet has no shape of its own,
                // and the count itself is already drawn as the hero's fourth figure.
                InlineEmptyHint(
                    "The first call lands once the coach has $needed sessions to read.",
                    color = c.muted
                )
            } else {
                decisions.forEach { d -> DecisionRow(d, state, c, onApply, onSkip, onUndo) }
                val open = decisions.count { it.status == CoachRepository.STATUS_PROPOSED }
                if (open > 1 && brief != null) {
                    // §8: ONE filled capsule per section, grouped at its END. Per-decision actions
                    // stay level ② / ③ text so five open calls can never stack into a button wall.
                    ForgePrimaryCapsule(
                        label = "Apply all $open",
                        onClick = { onApplyAll(brief.pass.weekId) },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        }
    }
}

/** Last brief's changes, each with its two-week window drawn and its live verdict. */
private fun LazyListScope.coachWatchSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int
) {
    val previous = state.brief?.previousApplied.orEmpty()
    if (previous.isEmpty()) return

    item("coach-watching") {
        CoachSection(
            c, title = "Under watch", index = index,
            caption = "Two weeks to prove out; undo any time."
        ) {
            val now = remember { System.currentTimeMillis() }
            previous.forEach { d -> WatchedRow(d, now, c) }
        }
    }
}

/**
 * One decision, fully in the open: the summary with its status word, the coach's why as a quiet
 * aside, the evidence chart, and the actions. No folding, nothing behind a tap.
 */
@Composable
private fun DecisionRow(
    d: CoachDecision,
    state: CoachViewModel.UiState,
    c: CoachColors,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit
) {
    val now = remember { System.currentTimeMillis() }
    val status = coachDecisionStatusWord(d, now)
    val statusColor = if (d.status == CoachRepository.STATUS_PROPOSED) c.accent else c.muted

    Column(Modifier.fillMaxWidth().padding(bottom = COACH_BLOCK_GAP)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.Top
        ) {
            // §14: a decision summary names the user's own exercise, so it wraps rather than clips.
            Text(
                d.summary,
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            if (status.isNotEmpty()) {
                Text(status, style = MaterialTheme.typography.labelSmall, color = statusColor)
            }
        }
        Spacer(Modifier.height(2.dp))
        Text(
            d.reason,
            style = MaterialTheme.typography.bodySmall,
            color = c.muted,
            fontStyle = FontStyle.Italic
        )

        // The evidence: the lift's strength trend, or the recovery meter for a deload call.
        if (d.type == "deload") {
            state.watch?.fatigueScore?.let { score ->
                Spacer(Modifier.height(10.dp))
                CoachFatigueMeter(score, state.watch.fatigueThreshold, c)
            }
        } else {
            state.e1rmBySlot[d.targetKey]?.let { series ->
                Spacer(Modifier.height(8.dp))
                CoachChartLabel("${d.targetName} · estimated 1RM · last ${series.size} sessions", c)
                CoachSparkline(series, c.accent, c.bg)
            }
        }

        when {
            d.status == CoachRepository.STATUS_PROPOSED -> {
                Spacer(Modifier.height(4.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                    CoachAction("Apply →", c.accent, "Apply this change") { onApply(d.id) }
                    CoachAction("Skip", c.muted, "Skip this change") { onSkip(d.id) }
                }
            }
            d.status == CoachRepository.STATUS_APPLIED && d.undoData != null -> {
                Spacer(Modifier.height(4.dp))
                CoachAction("Undo", c.muted, "Undo this change") { onUndo(d.id) }
            }
        }
    }
}

/** One change under watch: its summary, the two-week window as a countdown bar, the verdict. */
@Composable
private fun WatchedRow(d: CoachDecision, now: Long, c: CoachColors) {
    val windowMs = 14L * 24 * 60 * 60 * 1000
    val decided = d.outcome == "ok" || d.outcome == "failed"
    val fraction =
        if (decided) 1f
        else d.appliedAt?.let { ((now - it).toFloat() / windowMs) }?.coerceIn(0f, 1f) ?: 0f
    val barColor = when {
        d.outcome == "failed" -> c.error
        d.outcome == "ok" -> c.accent
        else -> c.secondary
    }
    Column(Modifier.fillMaxWidth().padding(bottom = COACH_BLOCK_GAP)) {
        // §14: user content wraps. A change's summary carries an exercise name.
        Text(d.summary, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
        Spacer(Modifier.height(6.dp))
        CoachWatchBar(fraction, barColor, c)
        Spacer(Modifier.height(4.dp))
        Text(
            watchStatusLine(d, now),
            style = MaterialTheme.typography.labelSmall,
            color = if (d.outcome == "failed") c.error else c.muted
        )
    }
}

/** Status line for a change from the previous brief, with the live watch verdict. */
private fun watchStatusLine(d: CoachDecision, now: Long): String {
    val outcome = com.forge.app.domain.coach.CoachOutcome.label(d.status, d.outcome, d.appliedAt, now)
    return when (d.status) {
        CoachRepository.STATUS_FOLDED -> "absorbed into the baseline" + (outcome?.let { ", $it" } ?: "")
        "reverted" -> "undone"
        else -> "in effect" + (outcome?.let { ", $it" } ?: "")
    }
}

/**
 * The road ahead as small graphs: the next brief, the closest pending verdict, and the milestone
 * ladder as ONE rail plus the next step (§4.10).
 *
 * Autopilot used to have a bar here too, reading the closest-to-earning type ("0 of 3") while the
 * Journey lens read types earned ("0 of 4") — the same word, two numbers, one page. Trust lives on
 * Journey now and this section does not mention it (§4.3, one home).
 */
private fun LazyListScope.coachComingUpSection(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int
) {
    item("coach-coming") {
        CoachSection(c, title = "Coming up", index = index) {
            val brief = state.brief
            val now = remember { System.currentTimeMillis() }

            // The next weekly brief, counting down through the week. The countdown names its
            // landing day (§11) — the pass always runs with Monday.
            if (state.daysToNextBrief in 1..7) {
                CoachProgressRow(
                    label = "Next brief",
                    value = "${state.daysToNextBrief} day${if (state.daysToNextBrief == 1) "" else "s"}",
                    c = c,
                    fraction = (7 - state.daysToNextBrief) / 7f,
                    sub = "Lands Monday."
                )
            }

            // The closest pending verdict on an applied change — named, never a bare countdown (§11).
            val windowMs = 14L * 24 * 60 * 60 * 1000
            brief?.previousApplied
                ?.filter { it.outcome == "pending" && it.appliedAt != null }
                ?.minByOrNull { it.appliedAt!! }
                ?.let { d ->
                    val elapsed = (now - d.appliedAt!!).coerceIn(0, windowMs)
                    val daysLeft = ceil((windowMs - elapsed) / (24.0 * 60 * 60 * 1000)).toInt()
                    if (daysLeft > 0) CoachProgressRow(
                        label = "Next verdict",
                        value = "~$daysLeft day${if (daysLeft == 1) "" else "s"}",
                        c = c,
                        fraction = elapsed.toFloat() / windowMs,
                        sub = d.summary
                    )
                }

            // Milestones: the achievement ladder as ONE segmented rail (§4.10) with only the NEXT
            // step named beneath it — the reached ones are history the rail already counts.
            state.timeline?.milestones?.takeIf { it.isNotEmpty() }?.let { milestones ->
                val reached = milestones.count { it.reached }
                val next = milestones.firstOrNull { !it.reached }
                CoachProgressRow(
                    label = "Milestones",
                    value = "$reached of ${milestones.size}",
                    c = c,
                    segments = reached to milestones.size
                )
                if (next != null) {
                    // Under a NEXT eyebrow so it reads as a forward target, not a done thing (§11).
                    Column(Modifier.padding(bottom = COACH_ROW_PAD)) {
                        CoachChartLabel("Next", c)
                        Spacer(Modifier.height(4.dp))
                        Text(next.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                        Text(next.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
            }
        }
    }
}

/**
 * The knowledge half of the coach, one tap from its decisions (B3). It closes the lens rather than
 * sitting inside the goals section, where it was an unrelated destination stacked under an unrelated
 * action — two accent links in a row, the first thing a new user met on this page.
 */
private fun LazyListScope.coachAcademyLink(
    state: CoachViewModel.UiState,
    c: CoachColors,
    index: Int,
    onOpenAcademy: () -> Unit
) {
    item("coach-academy") {
        Column(
            Modifier.fillMaxWidth().statsEntrance(index)
                .padding(horizontal = COACH_GUTTER, vertical = 20.dp)
        ) {
            CoachAction(
                if (state.newLessons > 0) "Academy · ${state.newLessons} new →" else "Academy →",
                c.accent,
                "Open the Academy",
                onOpenAcademy
            )
        }
    }
}
