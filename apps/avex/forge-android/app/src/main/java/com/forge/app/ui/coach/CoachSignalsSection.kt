package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.RecoverySignal
import com.forge.app.data.repo.TrackedLift
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.InlineEmptyHint
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The Signals lens: what the coach reads, drawn as data (§4.4), ordered by what's live
 * (§4.10) — the lifts on watch with their real deltas and trends first, then recovery load
 * with its live drivers, each recovery input as a plain-language row with its chart inline,
 * and the learned biases.
 */
internal fun LazyListScope.coachSignalsLens(
    state: CoachViewModel.UiState,
    useKg: Boolean,
    c: CoachColors,
    onConnectHealth: (() -> Unit)? = null
) {
    val watch = state.watch

    if (watch == null) {
        item("signals-error") {
            CoachSection(c, title = "Signals", index = 2) {
                InlineEmptyHint(
                    "Couldn't read the coach's inputs right now. Log a workout and check back.",
                    color = c.muted
                )
            }
        }
        return
    }

    // ── Lifts on watch ────────────────────────────────────────────────────────
    // Renders only once a lift is tracked. Every row carries its real trend: the delta since
    // the window's first session plus the sparkline. Lifts still short of two sessions collapse
    // into ONE ghost row (§12) — never a column of "forming", never an n-of-m jargon bar.
    if (watch.trackedLifts.isNotEmpty()) {
        item("signals-lifts") {
            CoachSection(c, title = "Lifts on watch", index = 2) {
                val (withTrend, forming) = watch.trackedLifts
                    .partition { (state.e1rmBySlot[it.slotId]?.size ?: 0) >= 2 }
                withTrend.take(LIFTS_SHOWN).forEach { lift ->
                    val series = state.e1rmBySlot.getValue(lift.slotId)
                    val (word, color) = liftTrendWord(lift, series, c)
                    LiftTrendRow(
                        name = lift.name,
                        statusWord = word,
                        statusColor = color,
                        // The read plus its depth (§4.11): the live e1RM and how many
                        // sessions of history stand behind the trend.
                        valueText = "${formatWeight(series.last(), useKg)} · " +
                            "${lift.bouts} session${if (lift.bouts == 1) "" else "s"}",
                        series = series,
                        c = c
                    )
                }
                val more = withTrend.size - LIFTS_SHOWN
                if (more > 0) {
                    Text(
                        "And $more more.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted
                    )
                }
                // The ghost spark rides the row only when real sparklines sit above it to rhyme
                // with (§12) — an all-forming group drops the mark so a lone flat line never
                // reads as a broken element.
                if (forming.isNotEmpty()) {
                    FormingLiftsRow(forming, showGhost = withTrend.isNotEmpty(), c)
                }
            }
        }
    }

    // ── Recovery load ─────────────────────────────────────────────────────────
    item("signals-fatigue") {
        val gate = watch.recoveryGateSessions.coerceAtLeast(1)
        // All gates met yet no score: the advisor mutes itself right after a deload.
        val muted = watch.fatigueScore == null &&
            watch.sessionsLogged >= gate && watch.historyDays >= watch.recoveryWindowDays
        CoachSection(
            c, title = "Recovery load", index = 3,
            caption = if (muted) "Muted after a deload. The score resumes as training rebuilds." else null
        ) {
            // The meter: the live fatigue score, else progress toward the advisor's REAL gates
            // (session count then a full window of history — never the coach's 4-session activation
            // gate), else a muted track when a recent deload silenced it.
            val score = watch.fatigueScore
            when {
                score != null -> CoachFatigueMeter(score, watch.fatigueThreshold, c)
                muted -> Box(
                    Modifier
                        .fillMaxWidth()
                        .height(5.dp)
                        .clip(RoundedCornerShape(50))
                        .background(c.outline.copy(alpha = 0.25f))
                )
                watch.sessionsLogged < gate -> CoachProgressRow(
                    label = "Building a baseline",
                    value = "${watch.sessionsLogged} of $gate sessions",
                    c = c,
                    segments = watch.sessionsLogged.coerceAtMost(gate) to gate
                )
                else -> CoachProgressRow(
                    label = "Building a baseline",
                    value = "${watch.historyDays} of ${watch.recoveryWindowDays} days trained",
                    c = c,
                    fraction = watch.historyDays.toFloat() /
                        watch.recoveryWindowDays.coerceAtLeast(1)
                )
            }
            // The instrument panel under the meter — every check the coach tracks, with its live
            // reading, drawn from session one (§4.9). Fired checks carry their points; pre-baseline
            // all read quiet. "No data" checks (disconnected sleep/HR) are left to "What it reads".
            val panel = watch.fatigueChecks.filter { it.reading != "no data" }
            if (panel.isNotEmpty()) {
                // §4.9: this panel of named checks is DATA, not §12 repetition — each renders its OWN
                // reading from session one (below its gate that reading is progress toward it, e.g.
                // "0 of 6 rated sets"), never collapsed to a countless "still building" line. Ordered
                // live-first (§4.10): a check that has woken surfaces above ones still short of a gate.
                val (dormant, live) = panel.partition { it.gated }
                Spacer(Modifier.height(12.dp))
                live.forEach { FatigueCheckRow(it, c) }
                dormant.forEach { FatigueCheckRow(it, c) }
                // One muted line so the readings aren't unexplained (§4.9) — what they build toward.
                if (!muted) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "A deload is called when enough cross their line.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted
                    )
                }
            }
        }
    }

    // ── The inputs it reads ───────────────────────────────────────────────────
    // One row per input in plain language — its name and the repo's own count ("12 in the last
    // two weeks"). Inputs with data draw their chart directly under their row; an unconnected
    // Health Connect input carries the action that connects it (§11).
    item("signals-inputs") {
        CoachSection(c, title = "What it reads", index = 4) {
            watch.recoverySignals.forEach { sig ->
                // The recovery panel above already carries the rest-day reading — one home (§4.3).
                if (sig.label == "Rest-day flags") return@forEach
                SignalRow(sig, c, onConnectHealth)
                // The labels are verbatim from CoachRepository.coachLab(); match them to hang
                // each chart under the input it belongs to.
                when {
                    sig.label == "Sleep" && state.health.sleepHours.isNotEmpty() -> {
                        Spacer(Modifier.height(6.dp))
                        CoachSleepBars(state.health.sleepHours, state.health.sleepFloorHours, c)
                        Spacer(Modifier.height(12.dp))
                    }
                    sig.label.contains("heart", ignoreCase = true) && state.health.restingHr.size >= 2 -> {
                        Spacer(Modifier.height(6.dp))
                        CoachHrLine(state.health.restingHr, state.health.hrBaseline, c)
                        Spacer(Modifier.height(12.dp))
                    }
                }
            }
        }
    }

    // ── Learned so far ────────────────────────────────────────────────────────
    // A learned-biases list has no zero-shape, so this is the lens's one allowed hint (§12) —
    // and when it shows, it REPLACES the caption (never both on one section).
    item("signals-learned") {
        val learnedEmpty = watch.learnedBiases.isEmpty()
        CoachSection(
            c, title = "Learned so far", index = 5,
            caption = if (learnedEmpty) null else "Carried into every regenerated plan."
        ) {
            if (learnedEmpty) {
                InlineEmptyHint(
                    "Nothing yet. The changes you apply and keep are what teach it.",
                    color = c.muted
                )
            } else {
                watch.learnedBiases.forEach { b ->
                    Column(Modifier.padding(bottom = 8.dp)) {
                        Text(b.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                        Text(b.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
            }
        }
    }
}

private const val LIFTS_SHOWN = 8

/** One fatigue check on the instrument panel: the check's name and its live reading (no dot —
 *  the name color + the "+N" already carry fired-ness, §8). Fired checks carry their points. */
@Composable
private fun FatigueCheckRow(check: DeloadAdvisor.FatigueCheck, c: CoachColors) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            check.name,
            style = MaterialTheme.typography.bodySmall,
            color = if (check.fired) c.onBg else c.muted,
            modifier = Modifier.weight(1f)
        )
        Text(
            if (check.fired) "${check.reading} · +${check.points}" else check.reading,
            style = MaterialTheme.typography.labelSmall,
            color = if (check.fired) c.onBg else c.muted
        )
    }
}

/** One recovery input: plain name + the repo's own count as the right-hand meta (no dot — the
 *  label color + meta already show connected/not, §8). An unconnected Health Connect input swaps
 *  its meta for the connect action. */
@Composable
private fun SignalRow(sig: RecoverySignal, c: CoachColors, onConnectHealth: (() -> Unit)?) {
    val connectable = !sig.active && onConnectHealth != null &&
        (sig.label == "Sleep" || sig.label.contains("heart", ignoreCase = true))
    Row(
        Modifier
            .fillMaxWidth()
            .then(
                // §8: the whole row is the tap target; the pill below is drawn, not clickable.
                if (connectable) Modifier.clickableLabeled("Connect Health Connect for ${sig.label}") {
                    onConnectHealth?.invoke()
                } else Modifier
            )
            .padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            sig.label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (sig.active) c.onBg else c.muted,
            modifier = Modifier.weight(1f)
        )
        if (connectable) {
            // §8: per-row do-it-now = right-aligned compact OUTLINED pill (a bare accent link
            // is too dim against a muted accent), border only, onBg text, sentence case.
            Text(
                "Connect",
                style = MaterialTheme.typography.labelMedium,
                color = c.onBg,
                modifier = Modifier
                    .border(1.dp, c.outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                    .padding(horizontal = 12.dp, vertical = 4.dp)
            )
        } else {
            Text(sig.detail, style = MaterialTheme.typography.labelSmall, color = c.muted)
        }
    }
}

/**
 * The lift's actual movement over its drawn window — "↑ 4%", "↓ 2%", "flat" — with stalling
 * (the coach's own stall call) as the one worded exception. The delta answers "what trend".
 */
private fun liftTrendWord(
    lift: TrackedLift,
    series: List<Double>,
    c: CoachColors
): Pair<String, androidx.compose.ui.graphics.Color> {
    val first = series.first()
    val pct = if (first > 0) (series.last() - first) / first * 100 else 0.0
    return when {
        lift.stalling -> "stalling" to c.error
        pct >= 0.5 -> "↑ ${pct.roundToInt()}%" to c.accent
        pct <= -0.5 -> "↓ ${abs(pct).roundToInt()}%" to c.muted
        else -> "flat" to c.muted
    }
}

/**
 * The lifts still short of two logged sessions, collapsed to ONE row (§12). A single lift is
 * named (the title IS the name); many just count — a wrapped name-cram reads worse than none
 * (§11). The ghost spark rides along only when live sparklines sit above ([showGhost]).
 */
@Composable
private fun FormingLiftsRow(forming: List<TrackedLift>, showGhost: Boolean, c: CoachColors) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                if (forming.size == 1) forming.first().name
                else "${forming.size} lifts building history",
                style = MaterialTheme.typography.bodyMedium,
                color = c.muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                "first read after two sessions",
                style = MaterialTheme.typography.labelSmall,
                color = c.muted
            )
        }
        if (showGhost) {
            Spacer(Modifier.width(12.dp))
            CoachGhostSpark(c, modifier = Modifier.width(110.dp), height = 32.dp)
        }
    }
}
