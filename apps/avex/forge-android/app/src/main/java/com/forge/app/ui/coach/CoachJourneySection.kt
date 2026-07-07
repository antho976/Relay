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
    timeline.weeks.takeIf { it.isNotEmpty() }?.let { weeks ->
        item("journey-record") {
            CoachSection(c, title = "The record", index = 2) {
                val now = remember { System.currentTimeMillis() }
                weeks.take(RECORD_WEEKS).forEach { w -> RecordWeekRow(w, now, c) }
                val more = weeks.size - RECORD_WEEKS
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
        CoachSection(c, title = "Earned autopilot", index = 3) {
            // The state reads at a glance (not buried in a muted caption), then a line saying what
            // filling a bar actually does — the concept isn't self-evident (§13).
            Text(
                if (on) "On" else "Off",
                style = MaterialTheme.typography.titleSmall,
                color = if (on) c.accent else c.onBg
            )
            Spacer(Modifier.height(2.dp))
            Text(
                if (on) "Each filled bar's change type now applies on its own."
                else "Fill a bar and that change can apply itself, once you turn this on in Settings.",
                style = MaterialTheme.typography.bodySmall,
                color = c.muted
            )
            Spacer(Modifier.height(12.dp))
            // Empty is data at zero (§12): every type shows its own "0 of N" bar filling toward autopilot.
            timeline.trust.forEach { t -> TrustRow(t, c) }
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

    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        // The marker rides the first text line, not the row's centre, so multi-line weeks stay tidy.
        CoachFlagDot(flagColor, Modifier.padding(top = 6.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    coachWeekLabel(week.pass.weekId) ?: week.pass.weekId,
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
