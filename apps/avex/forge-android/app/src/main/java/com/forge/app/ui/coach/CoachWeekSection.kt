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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.ui.common.ForgePrimaryCapsule
import kotlin.math.ceil

/**
 * The Now lens, ordered by what's live (§4.10): the week's call with the evidence and the
 * actions in the open, the changes under watch from the last brief, then ONE road-ahead
 * section — the countdown bars with the milestone rail folded in (§4.12). Nothing folds
 * (§4.2); holds and errors speak once, in the hero. The deep dives live in Signals; trust
 * and the week-by-week record live in Journey.
 */
internal fun LazyListScope.coachNowLens(
    state: CoachViewModel.UiState,
    c: CoachColors,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    onApplyAll: (String) -> Unit
) {
    val brief = state.brief

    // ── The call — only when the week actually produced decisions ────────────
    if (brief != null && brief.decisions.isNotEmpty()) {
        item("coach-call") {
            val open = brief.decisions.count { it.status == CoachRepository.STATUS_PROPOSED }
            CoachSection(c, title = "The call", index = 2) {
                brief.decisions.forEach { d ->
                    // With exactly one open proposal the section's own capsule IS its Apply, so
                    // the row drops its link rather than offering the same action twice (§8).
                    DecisionRow(d, state, c, onApply, onSkip, onUndo, inlineApply = open != 1)
                }
                // §3's "primary action above fold" + §8 ① — the ONE filled capsule on this page,
                // and the screen's single large-format element. The accent used to be spent only
                // on postage-stamp dots and links, which §5 calls a dead pixel, not energy.
                if (open > 0) {
                    Spacer(Modifier.height(4.dp))
                    val single = brief.decisions.firstOrNull {
                        it.status == CoachRepository.STATUS_PROPOSED
                    }
                    ForgePrimaryCapsule(
                        label = if (open == 1) "Apply this change" else "Apply all $open",
                        onClick = {
                            if (open == 1 && single != null) onApply(single.id)
                            else onApplyAll(brief.pass.weekId)
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        }
    }

    // ── Under watch: last brief's changes, each with its window drawn ─────────
    if (brief != null && brief.previousApplied.isNotEmpty()) {
        item("coach-watching") {
            CoachSection(
                c, title = "Under watch", index = 3,
                caption = "Two weeks to prove out; undo any time."
            ) {
                val now = remember { System.currentTimeMillis() }
                brief.previousApplied.forEach { d -> WatchedRow(d, now, c) }
            }
        }
    }

}

/**
 * One decision, fully in the open: the summary with its status word, the coach's why as a
 * quiet aside, the evidence chart, and the actions. No folding, nothing behind a tap.
 */
@Composable
private fun DecisionRow(
    d: CoachDecision,
    state: CoachViewModel.UiState,
    c: CoachColors,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    inlineApply: Boolean
) {
    val now = remember { System.currentTimeMillis() }
    val status = coachDecisionStatusWord(d, now)
    val statusColor = if (d.status == CoachRepository.STATUS_PROPOSED) c.accent else c.muted

    Column(Modifier.fillMaxWidth().padding(bottom = 16.dp)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                d.summary,
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
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
                    if (inlineApply) {
                        CoachAction("Apply →", c.accent, "Apply this change") { onApply(d.id) }
                    }
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
    Column(Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        Text(
            d.summary, style = MaterialTheme.typography.bodyMedium, color = c.onBg,
            maxLines = 1, overflow = TextOverflow.Ellipsis
        )
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
 * The road ahead as small graphs: every countdown the coach knows about drawn as a progress
 * bar — the next brief, pending verdicts, the change type closest to autopilot, milestones.
 */
internal fun LazyListScope.coachComingUpSection(
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

            // The Autopilot countdown that sat here is gone (2026-08-20): Journey's "Earned
            // autopilot" section already owns that fact with a bar per change type, and §4.3
            // gives a fact ONE home — never repeated across lenses.

            // Milestones: the achievement ladder as ONE segmented rail (§4.12), sharing the
            // countdowns' bar language, with only the NEXT step named beneath it — the reached
            // ones are history the rail already counts, the 9-row checklist is gone.
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
                    // The next milestone under a NEXT eyebrow so it reads as a forward target,
                    // not a done thing (§11) — no dot, the eyebrow does the framing.
                    Column(Modifier.padding(bottom = 6.dp)) {
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
