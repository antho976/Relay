package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.RecoverySignal
import com.forge.app.data.repo.TrackedLift
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance
import kotlin.math.abs
import kotlin.math.roundToInt

private const val LIFTS_SHOWN = 8

/**
 * WHERE YOU STAND — the coach's live reading, in one place.
 *
 * This is what the old Signals lens was for, minus its two failures. The instrument panel of
 * every dormant check is gone: a check that has not fired is not a reading the user needs, and a
 * column of them was the machine showing its work rather than the coach saying anything. The
 * signal registry with its unbuilt slots is gone too — a roadmap is not a reading.
 *
 * What survives is what the coach actually reads right now: recovery load with the checks that
 * crossed their line, the lifts it is watching with their real trends, and the inputs feeding it.
 * The evidence for any specific CALL lives on that call, not here.
 */
internal fun LazyListScope.coachStand(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    c: CoachColors,
    onConnectHealth: (() -> Unit)?
) {
    val watch = state.watch ?: return

    item("stand") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(1)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor("Signals", c)
            Spacer(Modifier.height(18.dp))

            // ── Recovery load ────────────────────────────────────────────────
            val gate = watch.recoveryGateSessions.coerceAtLeast(1)
            val score = watch.fatigueScore
            // All gates met yet no score: the advisor mutes itself right after a deload.
            val muted = score == null &&
                watch.sessionsLogged >= gate && watch.historyDays >= watch.recoveryWindowDays
            StandGroup("Recovery load", c)
            Spacer(Modifier.height(10.dp))
            when {
                score != null -> {
                    CoachFatigueMeter(score, watch.fatigueThreshold, c)
                    val fired = watch.fatigueChecks.filter { it.fired }
                    if (fired.isNotEmpty()) {
                        Spacer(Modifier.height(12.dp))
                        // Only what crossed its line: the exceptions are the reading, and a row of
                        // quiet checks would be a column of "fine" saying nothing.
                        fired.forEach { check ->
                            Row(
                                Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
                                verticalAlignment = Alignment.Top
                            ) {
                                Text(
                                    check.name,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = c.onBg,
                                    modifier = Modifier.weight(1f)
                                )
                                Spacer(Modifier.width(12.dp))
                                Text(
                                    check.reading.uppercase(),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = c.muted
                                )
                            }
                        }
                    }
                }
                // A recent deload silenced the score: an honest empty track, and the caption says
                // why rather than leaving a dead bar on the page.
                muted -> {
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .height(10.dp)
                            .clip(RoundedCornerShape(5.dp))
                            .drawBehind { drawRect(c.outline.copy(alpha = 0.25f)) }
                            .semantics { contentDescription = "Recovery load paused after a deload" }
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Paused after a deload. The score resumes as training rebuilds.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.muted
                    )
                }
                // Below a gate, the reading is progress toward the gate — never "not enough data".
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
                    fraction = watch.historyDays.toFloat() / watch.recoveryWindowDays.coerceAtLeast(1)
                )
            }

            // ── Lifts on watch ───────────────────────────────────────────────
            if (watch.trackedLifts.isNotEmpty()) {
                Spacer(Modifier.height(28.dp))
                StandGroup("Lifts on watch", c)
                Spacer(Modifier.height(10.dp))
                val (withTrend, forming) = watch.trackedLifts
                    .partition { (state.e1rmBySlot[it.slotId]?.size ?: 0) >= 2 }
                withTrend.take(LIFTS_SHOWN).forEach { lift ->
                    val series = state.e1rmBySlot.getValue(lift.slotId)
                    val word = liftTrendWord(lift, series)
                    LiftTrendRow(
                        name = lift.name,
                        statusWord = word,
                        // Measured on the near-black ground, error red is 3.67:1 and accent is
                        // accent-dependent, so neither may colour text this small. The ↑ / ↓ glyph
                        // carries the direction and the SPARKLINE goes red for a stall — a data
                        // mark only needs 3:1, which error clears.
                        statusColor = if (lift.stalling || word.startsWith("↑")) c.onBg else c.muted,
                        markColor = if (lift.stalling) c.error else c.accent,
                        valueText = "${formatWeight(series.last(), weightUnit)} · " +
                            "${lift.bouts} session${if (lift.bouts == 1) "" else "s"}",
                        series = series,
                        c = c
                    )
                }
                val more = withTrend.size - LIFTS_SHOWN
                if (more > 0) {
                    Text("And $more more.", style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
                // Lifts short of two sessions collapse to ONE line naming the real unlock, never a
                // column of identical "forming" rows. The ghost spark rides along only when live
                // sparklines sit above it to rhyme with; alone, a flat line reads as broken.
                if (forming.isNotEmpty()) {
                    FormingLiftsRow(forming, showGhost = withTrend.isNotEmpty(), c)
                }
            }

        }
    }
}

/**
 * WHAT IT READS — the inputs feeding the coach, and whether each one is connected.
 *
 * Its own region rather than a group inside SIGNALS: the block sits between the two now, so a
 * 13sp group label here would read as belonging to the block rather than to the readings.
 */
internal fun LazyListScope.coachInputs(
    state: CoachViewModel.UiState,
    c: CoachColors,
    onConnectHealth: (() -> Unit)?
) {
    val watch = state.watch ?: return
    if (watch.recoverySignals.isEmpty()) return
    item("inputs") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(3)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor("What it reads", c)
            Spacer(Modifier.height(12.dp))
            watch.recoverySignals.forEach { sig ->
                SignalRow(sig, c, onConnectHealth)
                // Each input's own chart hangs under the input it belongs to.
                when {
                    sig.label == "Sleep" && state.health.sleepHours.isNotEmpty() -> {
                        Spacer(Modifier.height(6.dp))
                        CoachSleepBars(state.health.sleepHours, state.health.sleepFloorHours, c)
                        Spacer(Modifier.height(14.dp))
                    }
                    sig.label.contains("heart", ignoreCase = true) &&
                        state.health.restingHr.size >= 2 -> {
                        Spacer(Modifier.height(6.dp))
                        CoachHrLine(state.health.restingHr, state.health.hrBaseline, c)
                        state.health.hrvWindowAvg?.let { hrv ->
                            Spacer(Modifier.height(6.dp))
                            Text(
                                buildString {
                                    append("HRV $hrv MS")
                                    state.health.hrvBaseline?.let { append(" · BASELINE $it MS") }
                                },
                                style = MaterialTheme.typography.labelSmall,
                                color = c.muted
                            )
                        }
                        Spacer(Modifier.height(14.dp))
                    }
                }
            }
        }
    }
}

/**
 * A group inside SIGNALS. Ranked below the 15sp anchor by SIZE, not by tracking or colour: the
 * anchor names the region, these name the readings inside it.
 */
@Composable
private fun StandGroup(label: String, c: CoachColors) {
    Text(label.uppercase(), style = MaterialTheme.typography.labelLarge, color = c.muted)
}

/**
 * One watched lift: its name and its real movement on the left, the trend drawn on the right when
 * there are two points to draw.
 */
@Composable
private fun LiftTrendRow(
    name: String,
    statusWord: String,
    statusColor: Color,
    markColor: Color,
    valueText: String?,
    series: List<Double>,
    c: CoachColors
) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
            // A row's sub-reading is sentence-shaped, so it takes the sans voice; mono here would
            // be the machine voice set as prose.
            Row {
                Text(statusWord, style = MaterialTheme.typography.bodySmall, color = statusColor)
                if (valueText != null) {
                    Text(" · $valueText", style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
        }
        if (series.size >= 2) {
            Spacer(Modifier.width(12.dp))
            CoachSparkline(
                series,
                markColor,
                c.bg,
                modifier = Modifier
                    .width(110.dp)
                    .semantics { contentDescription = "$name trend, $statusWord" },
                height = 32.dp
            )
        }
    }
}

/**
 * The lift's actual movement over its drawn window — "↑ 4%", "↓ 2%", "flat" — with the coach's own
 * stall call as the one worded exception.
 */
private fun liftTrendWord(lift: TrackedLift, series: List<Double>): String {
    val first = series.first()
    val pct = if (first > 0) (series.last() - first) / first * 100 else 0.0
    return when {
        lift.stalling -> "stalling"
        pct >= 0.5 -> "↑ ${pct.roundToInt()}%"
        pct <= -0.5 -> "↓ ${abs(pct).roundToInt()}%"
        else -> "flat"
    }
}

/** Lifts still short of two logged sessions, collapsed to ONE row naming the concrete unlock. */
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
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                "first read after two sessions",
                style = MaterialTheme.typography.bodySmall,
                color = c.muted
            )
        }
        if (showGhost) {
            Spacer(Modifier.width(12.dp))
            CoachGhostSpark(c, modifier = Modifier.width(110.dp), height = 32.dp)
        }
    }
}

/**
 * One recovery input: its name and the repository's own count. An unconnected Health Connect
 * input swaps that count for the action that connects it — the whole row is the tap target and
 * the pill is drawn, never independently clickable.
 */
@Composable
private fun SignalRow(sig: RecoverySignal, c: CoachColors, onConnectHealth: (() -> Unit)?) {
    val connectable = !sig.active && onConnectHealth != null &&
        (sig.label == "Sleep" || sig.label.contains("heart", ignoreCase = true))
    Row(
        Modifier
            .fillMaxWidth()
            .then(
                if (connectable) Modifier.clickableLabeled("Connect Health Connect for ${sig.label}") {
                    onConnectHealth?.invoke()
                } else Modifier
            )
            // ONE height for every input row. A pill row is taller than a text row, so mixing
            // them left the four rows on an uneven rhythm with their readings drifting apart;
            // this also buys the 48dp target the connectable rows need anyway.
            .heightIn(min = 48.dp)
            .padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // The one list on this page that leads with a glyph: four near-identical rows on the
        // emptiest screen a new account sees, where the icon is how the eye finds "sleep".
        Icon(
            CoachIcons.forSignal(sig.label),
            contentDescription = null,
            tint = if (sig.active) c.onBg else c.muted,
            modifier = Modifier.size(18.dp)
        )
        Spacer(Modifier.width(12.dp))
        Text(
            sig.label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (sig.active) c.onBg else c.muted,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(12.dp))
        if (connectable) ForgeRowPill("Connect")
        else Text(signalReading(sig), style = MaterialTheme.typography.bodySmall, color = c.muted)
    }
}

/**
 * The repository writes a different sentence for each input's zero ("none yet", "none flagged",
 * "not connected"), which put three ways of saying nothing in one four-row list. Zero is one word.
 */
private fun signalReading(sig: RecoverySignal): String =
    if (sig.active) sig.detail else "none"

/** The lens's one hint, for the state where the coach could not read its own inputs at all. */
@Composable
internal fun CoachStandUnavailable(c: CoachColors) {
    InlineEmptyHint(
        "Couldn't read the coach's inputs right now. Log a workout and check back.",
        color = c.muted
    )
}
