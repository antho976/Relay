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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.coach.TypeTrust
import com.forge.app.ui.common.InlineEmptyHint

/**
 * The Journey lens: how far the coach has come. The week-by-week record as a changelog —
 * newest first, one row per pass, the week's calls and their outcomes underneath — then the
 * autopilot trust per change type. The record leads (§4.10): real rows outrank unlock meters.
 * The milestone ladder lives on the Now lens.
 */
internal fun LazyListScope.coachJourneyLens(
    state: CoachViewModel.UiState,
    c: CoachColors
) {
    val timeline = state.timeline

    if (timeline == null) {
        item("journey-empty") {
            CoachSection(c, title = "Journey", index = 2) {
                InlineEmptyHint("No coaching history yet. Log a few weeks and this fills in.", color = c.muted)
            }
        }
        return
    }

    // ── The record ────────────────────────────────────────────────────────────
    // The coach's changelog: one row per weekly pass, newest first. The dot carries the week's
    // outcome, the lines under it name the calls. Reads the same at two weeks as at twenty.
    item("journey-record") {
        CoachSection(c, title = "The record", index = 2) {
            val now = remember { System.currentTimeMillis() }
            // A pre-baseline learning hold isn't a record of anything: no calls, and its one line just
            // restates the baseline countdown the hero eyebrow + Now lens already carry (§4.3, one home).
            // Drop those weeks — the record is the changelog of real calls, not the wait for the first.
            val recordWeeks = remember(timeline.weeks) {
                timeline.weeks.filterNot { w ->
                    w.decisions.isEmpty() && AutoCoachPlanner.isLearningHold(w.pass.holdReason)
                }
            }
            if (recordWeeks.isEmpty()) {
                // The one hint this lens is allowed (§12); no restating the countdown.
                InlineEmptyHint("No calls yet. Each week's changes land here once coaching starts.", color = c.muted)
            } else {
                recordWeeks.take(RECORD_WEEKS).forEach { w -> RecordWeekRow(w, now, c) }
                val more = recordWeeks.size - RECORD_WEEKS
                if (more > 0) {
                    Text(
                        "And $more more week${if (more == 1) "" else "s"}.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted
                    )
                }
            }
        }
    }

    // ── Earned autopilot ──────────────────────────────────────────────────────
    item("journey-trust") {
        val on = state.watch?.autopilot == true
        val trust = timeline.trust
        val earnedCount = trust.count { it.earned }
        val anyProgress = trust.any { it.streak > 0 || it.earned }
        CoachSection(c, title = "Earned autopilot", index = 3) {
            // The state leads on its own line so it can't be buried (the accent colour flags the active
            // ON state, §8); the right meta is a plain count of how many types have earned it (§8).
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    if (on) "On" else "Off",
                    style = MaterialTheme.typography.titleSmall,
                    color = if (on) c.accent else c.onBg,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    "$earnedCount of ${trust.size} earned",
                    style = MaterialTheme.typography.labelSmall,
                    color = c.muted
                )
            }
            Spacer(Modifier.height(2.dp))
            // ONE caption saying what the state MEANS for the user (§11), not the mechanics of a
            // bar — and §12: a hint REPLACES the caption, never both. With no type started, the
            // hint below says everything this line did, so at zero the caption stands down.
            if (anyProgress) {
                Text(
                    if (on) "A change applies on its own once its type's bar fills."
                    else "Autopilot is off; earned changes still wait for your tap.",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted
                )
            }
            Spacer(Modifier.height(12.dp))
            if (anyProgress) {
                // Each type carries a distinct reading, so the bars earn their list (§4.10).
                trust.forEach { t -> TrustRow(t, c) }
            } else {
                // §12: an all-ghost group drops the mark — a lone flat bar reads as broken, and a meter
                // on "types earned of total" is the wrong gate (autopilot is earned per type, by that
                // type's OWN streak). Collapse to ONE line naming the real per-type unlock.
                InlineEmptyHint(
                    "No type has earned autopilot yet. Fill a change type's bar and it earns it on its own.",
                    color = c.muted
                )
            }
        }
    }
}

private const val RECORD_WEEKS = 8

/** One change type's trust bar, fully visible: label, streak state, segmented meter. */
@Composable
private fun TrustRow(t: TypeTrust, c: CoachColors) {
    Column(Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        Row(Modifier.fillMaxWidth()) {
            Text(
                t.label,
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            Text(
                if (t.earned) "auto" else "${t.streak} of ${t.required}",
                style = MaterialTheme.typography.labelSmall,
                color = if (t.earned) c.accent else c.muted
            )
        }
        Spacer(Modifier.height(6.dp))
        TrustProgressBar(streak = t.streak, required = t.required, earned = t.earned, modifier = Modifier.fillMaxWidth())
    }
}

/**
 * One week of the record: the human week and what happened underneath — each call with its
 * lifecycle word, or (for a held/failed week) the humanized reason. A leading dot flags only an
 * outcome the eye should catch (§8): red for a failed/reverted week, accent for a confirmed win;
 * the neutral majority (held, learning, plain calls) reserve the gutter with no dot.
 */
@Composable
private fun RecordWeekRow(week: CoachRepository.CoachHistoryEntry, now: Long, c: CoachColors) {
    // §11: machine ids never render — an unparseable week id drops the row, never shows raw.
    val weekLabel = coachWeekLabel(week.pass.weekId) ?: return
    val flagColor = when {
        week.pass.status == CoachRepository.STATUS_ERROR ||
            week.decisions.any { it.outcome == "failed" || it.status == "reverted" } -> c.error
        week.decisions.any { it.outcome == "ok" } -> c.accent
        else -> null
    }
    // Right meta is a COUNT only (§11): the "N calls" tally. "held"/"failed" are state words the
    // dot and the reason line below already carry, so they never float on the right.
    val callCount = week.decisions.size.takeIf { it > 0 }
        ?.let { "$it call${if (it == 1) "" else "s"}" }

    Row(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD)) {
        // The marker rides the first text line, not the row's centre, so multi-line weeks stay tidy.
        CoachFlagDot(flagColor, Modifier.padding(top = 6.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    weekLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg,
                    modifier = Modifier.weight(1f)
                )
                if (callCount != null) {
                    Text(callCount, style = MaterialTheme.typography.labelSmall, color = c.muted)
                }
            }
            week.decisions.forEach { d ->
                val word = coachDecisionStatusWord(d, now)
                Text(
                    if (word.isEmpty()) d.summary else "${d.summary} · $word",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted
                )
            }
            // A held/failed week has no calls to list — its humanized reason says what happened,
            // machine prose translated at the seam (§11): no em dashes, no "(s)", no paragraph.
            if (week.decisions.isEmpty()) {
                week.pass.holdReason?.takeIf { it.isNotBlank() }?.let { reason ->
                    Text(recordHoldLine(reason), style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
        }
    }
}
