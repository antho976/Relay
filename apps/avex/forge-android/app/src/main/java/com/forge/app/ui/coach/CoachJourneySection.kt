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
import com.forge.app.domain.coach.PersonalProfile
import com.forge.app.domain.coach.TypeTrust
import com.forge.app.ui.common.InlineEmptyHint

/**
 * The Journey lens: how far the coach has come, and what it has worked out about you. The
 * week-by-week record leads (§4.8, real rows outrank unlock meters), then the autopilot trust per
 * change type, then what it has learned.
 *
 * Trust lives here and only here. The Now lens used to carry an "Autopilot" countdown reading the
 * closest-to-earning type while this lens read types earned — one word, two numbers, one page
 * (`design/FAILURES.md`, *Mark echo*).
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
            // restates the baseline countdown the hero already carries (§4.3, one home). Drop those
            // weeks — the record is the changelog of real calls, not the wait for the first.
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
    if (timeline.trust.isNotEmpty()) {
        item("journey-trust") {
            val on = state.watch?.autopilot == true
            val trust = timeline.trust
            val earnedCount = trust.count { it.earned }
            CoachSection(c, title = "Earned autopilot", index = 3) {
                // The state leads on its own line so it can't be buried (the accent colour flags the
                // active ON state, §8); the right meta is a plain count (§8).
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
                // ONE caption saying what the state MEANS for the user (§11), not the mechanics of a bar.
                Text(
                    if (on) "A change applies on its own once its type's bar fills."
                    else "Autopilot is off; earned changes still wait for your tap.",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted
                )
                Spacer(Modifier.height(12.dp))
                // The bars draw at zero rather than collapsing to a line: each row names a real
                // change type and carries its own "0 of 3", which is a reading, not a ghost — and an
                // empty track is a container, not a value (`design/FAILURES.md`, *Empty by omission*).
                trust.forEach { t -> TrustRow(t, c) }
            }
        }
    }

    // ── Learned so far ────────────────────────────────────────────────────────
    // Moved off the Signals lens: what the coach has WORKED OUT belongs with its record, not with
    // the live readings it takes each week. Omitted when there is nothing learned — that is an
    // absent subject, and "The record" at zero already says no calls have landed (§4.3).
    val biases = state.watch?.learnedBiases.orEmpty()
    if (biases.isNotEmpty()) {
        item("journey-learned") {
            CoachSection(
                c, title = "Learned so far", index = 4,
                caption = "Carried into every regenerated plan."
            ) {
                biases.forEach { b ->
                    Column(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD)) {
                        Text(b.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                        Text(b.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
            }
        }
    }

    // ── Your numbers ──────────────────────────────────────────────────────────
    // The per-athlete constants that replaced the coach's defaults. These used to hang off the
    // bottom of the Project section under a hand-sized mono label, which made them a second section
    // wearing the first one's header.
    val profile = state.profile
    if (profile.hasPersonalData) {
        item("journey-numbers") {
            CoachSection(
                c, title = "Your numbers", index = 5,
                caption = profile.recoveryDays?.let {
                    "Best spacing: $it ${if (it == 1) "day" else "days"} between sessions."
                }
            ) {
                VolumeCapBars(profile, c)
            }
        }
    }
}

private const val RECORD_WEEKS = 8

/**
 * Weekly set caps the coach has measured for this athlete, as a ranked comparison — §2②'s shape for
 * exactly this data. The bar is the mark; the count beside each name is its reading.
 */
@Composable
private fun VolumeCapBars(profile: PersonalProfile.Profile, c: CoachColors) {
    val caps = profile.volumeCaps.entries.sortedByDescending { it.value }.take(VOLUME_CAPS_SHOWN)
    if (caps.isEmpty()) {
        CoachMeter(null, c)
        return
    }
    val max = caps.maxOf { it.value }.coerceAtLeast(1)
    caps.forEach { (muscle, cap) ->
        Column(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    muscle.displayName,
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    "$cap sets a week",
                    style = MaterialTheme.typography.labelSmall,
                    color = c.muted
                )
            }
            Spacer(Modifier.height(6.dp))
            CoachMeter(cap.toFloat() / max, c, height = 4.dp)
        }
    }
}

private const val VOLUME_CAPS_SHOWN = 5

/** One change type's trust bar, fully visible: label, streak state, segmented meter. */
@Composable
private fun TrustRow(t: TypeTrust, c: CoachColors) {
    Column(Modifier.fillMaxWidth().padding(bottom = COACH_BLOCK_GAP)) {
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
