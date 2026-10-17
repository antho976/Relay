package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.RecoverySignal
import com.forge.app.data.repo.TrackedLift
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.GROUP_INNER
import com.forge.app.ui.common.GROUP_OUTER
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance
import kotlin.math.abs
import kotlin.math.roundToInt

/** Lift tiles drawn before the grid offers the rest. Three rows of two. */
private const val LIFTS_SHOWN = 6

/**
 * YOUR LIFTS (Detailed) — every lift the coach watches, as a grid of tiles: the name, its trend
 * as a sparkline, today's estimated max and which way it is moving. A stall is the coach's own call,
 * so stalled lifts lead and their line turns red. Lifts short of two sessions collapse to one line.
 */
internal fun LazyListScope.coachLifts(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    c: CoachColors
) {
    val lifts = state.watch?.trackedLifts.orEmpty()
    if (lifts.isEmpty()) return
    val (withTrend, forming) = lifts.partition { (state.e1rmBySlot[it.slotId]?.size ?: 0) >= 2 }
    item("lifts") {
        var showAll by rememberSaveable { mutableStateOf(false) }
        CoachSection("Your lifts", meta = liftsLine(state)?.value, modifier = Modifier.statsEntrance(3)) {
            if (withTrend.isNotEmpty()) {
                Text(
                    "Estimated 1RM over each lift's last sessions.",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted,
                    modifier = Modifier.padding(horizontal = 4.dp)
                )
                Spacer(Modifier.height(10.dp))
                val ordered = withTrend.sortedBy { if (it.stalling) 0 else 1 }
                val shown = if (showAll) ordered else ordered.take(LIFTS_SHOWN)
                // One column past ~130%: two tiles side by side squeeze a name to a sliver.
                val cols = if (LocalDensity.current.fontScale > 1.3f) 1 else 2
                LiftGrid(shown, cols, state, weightUnit, c)
                if (ordered.size > LIFTS_SHOWN) {
                    CoachAction(
                        if (showAll) "Show fewer" else "All ${ordered.size} lifts →",
                        c.onBg,
                        if (showAll) "Show fewer lifts" else "Show all ${ordered.size} lifts"
                    ) { showAll = !showAll }
                }
            }
            // Lifts short of two sessions collapse to ONE line naming the real unlock.
            if (forming.isNotEmpty()) {
                Spacer(Modifier.height(if (withTrend.isEmpty()) 0.dp else 6.dp))
                Text(
                    (if (forming.size == 1) forming.first().name else "${forming.size} lifts building history") +
                        " · first read after two sessions",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted,
                    modifier = Modifier.padding(horizontal = 4.dp)
                )
            }
        }
    }
}

@Composable
private fun LiftGrid(
    lifts: List<TrackedLift>,
    cols: Int,
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    c: CoachColors
) {
    val rows = lifts.chunked(cols)
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        rows.forEachIndexed { r, rowLifts ->
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(GROUP_SEAM)
            ) {
                rowLifts.forEachIndexed { i, lift ->
                    val first = r == 0
                    val last = r == rows.lastIndex
                    fun corner(top: Boolean, start: Boolean): Dp {
                        val edgeRow = if (top) first else last
                        val edgeCol = if (start) i == 0 else i == cols - 1 || (last && i == rowLifts.lastIndex && rowLifts.size < cols)
                        return if (edgeRow && edgeCol) GROUP_OUTER else GROUP_INNER
                    }
                    val shape = RoundedCornerShape(
                        topStart = corner(top = true, start = true),
                        topEnd = corner(top = true, start = false),
                        bottomStart = corner(top = false, start = true),
                        bottomEnd = corner(top = false, start = false)
                    )
                    LiftTile(
                        lift,
                        state.e1rmBySlot.getValue(lift.slotId),
                        weightUnit,
                        c,
                        Modifier.weight(1f).fillMaxHeight().clip(shape).background(c.tile)
                    )
                }
                repeat(cols - rowLifts.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
private fun LiftTile(
    lift: TrackedLift,
    series: List<Double>,
    weightUnit: WeightUnit,
    c: CoachColors,
    modifier: Modifier
) {
    val word = liftTrendWord(lift, series)
    val figure = formatWeight(series.last(), weightUnit)
    Column(
        modifier
            .semantics(mergeDescendants = true) {
                contentDescription = "${lift.name}, estimated max $figure, $word over ${lift.bouts} sessions"
            }
            .padding(horizontal = 14.dp, vertical = 12.dp)
    ) {
        // A lift's name is user content and wraps rather than clamping (§14).
        Text(lift.name, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
        Spacer(Modifier.weight(1f, fill = true))
        Spacer(Modifier.height(10.dp))
        CoachSparkline(
            series,
            if (lift.stalling) c.error else c.accent,
            c.tile,
            modifier = Modifier.fillMaxWidth(),
            height = 30.dp
        )
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                figure,
                style = MaterialTheme.typography.titleSmall.copy(fontFeatureSettings = "tnum"),
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(8.dp))
            // Error red and most accents fail AA as small text, so the word stays ink or muted and
            // the line above carries the colour.
            Text(
                word,
                style = MaterialTheme.typography.bodySmall,
                color = if (lift.stalling || word.startsWith("↑")) c.onBg else c.muted
            )
        }
    }
}

/**
 * The lift's actual movement over its drawn window ("↑ 4%", "↓ 2%", "flat") with the coach's own
 * stall call as the one worded exception.
 */
private fun liftTrendWord(lift: TrackedLift, series: List<Double>): String {
    val pct = liftMovePct(series)
    return when (liftMove(lift, series)) {
        LiftMove.STALLED -> "stalling"
        LiftMove.UP -> "↑ ${pct.roundToInt()}%"
        LiftMove.DOWN -> "↓ ${abs(pct).roundToInt()}%"
        LiftMove.FLAT -> "flat"
    }
}

/** Which way a watched lift is going. The coach's stall call outranks the raw movement. */
internal enum class LiftMove { UP, FLAT, DOWN, STALLED }

/**
 * One classification for both readings of a lift, the tile and the one-line summary, so the two can
 * never disagree about the same lift. Half a percent either way is flat.
 */
internal fun liftMove(lift: TrackedLift, series: List<Double>): LiftMove {
    val pct = liftMovePct(series)
    return when {
        lift.stalling -> LiftMove.STALLED
        pct >= 0.5 -> LiftMove.UP
        pct <= -0.5 -> LiftMove.DOWN
        else -> LiftMove.FLAT
    }
}

private fun liftMovePct(series: List<Double>): Double {
    val first = series.firstOrNull() ?: return 0.0
    return if (first > 0) (series.last() - first) / first * 100 else 0.0
}

/**
 * WHAT IT READS (Detailed) — the inputs feeding the coach, each a row with its count, or the
 * Connect that would feed it. A connected input's own chart hangs inside its row.
 */
internal fun LazyListScope.coachInputs(
    state: CoachViewModel.UiState,
    c: CoachColors,
    onConnectHealth: (() -> Unit)?
) {
    val watch = state.watch ?: return
    if (watch.recoverySignals.isEmpty()) return
    item("inputs") {
        CoachSection("What it reads", modifier = Modifier.statsEntrance(6)) {
            CoachRows(c, watch.recoverySignals.map { sig ->
                @Composable {
                    // Health Connect can be connected yet have nothing inside the recent window: the
                    // row then reads "none" rather than offering Connect for something connected.
                    val hasStaleData = !sig.active && when {
                        sig.label == "Sleep" -> state.health.sleepSynced
                        sig.label.contains("heart", ignoreCase = true) -> state.health.restingHrSynced
                        else -> false
                    }
                    SignalRow(sig, c, onConnectHealth, connected = hasStaleData)
                    when {
                        sig.label == "Sleep" && sig.active && state.health.sleepHours.isNotEmpty() -> {
                            Spacer(Modifier.height(8.dp))
                            CoachSleepBars(state.health.sleepHours, state.health.sleepFloorHours, c)
                        }
                        sig.label.contains("heart", ignoreCase = true) && sig.active &&
                            state.health.restingHr.size >= 2 -> {
                            Spacer(Modifier.height(8.dp))
                            CoachHrLine(state.health.restingHr, state.health.hrBaseline, c)
                            state.health.hrvWindowAvg?.let { hrv ->
                                Spacer(Modifier.height(6.dp))
                                Text(
                                    buildString {
                                        append("HRV $hrv ms")
                                        state.health.hrvBaseline?.let { append(" · baseline $it ms") }
                                    },
                                    style = MaterialTheme.typography.bodySmall,
                                    color = c.muted
                                )
                            }
                        }
                    }
                }
            })
        }
    }
}

/**
 * One recovery input: a muted glyph, its name and the repository's own count. An unconnected Health
 * Connect input swaps that count for a drawn Connect pill, and the whole row is the tap target.
 */
@Composable
private fun SignalRow(
    sig: RecoverySignal,
    c: CoachColors,
    onConnectHealth: (() -> Unit)?,
    connected: Boolean = false
) {
    val connectable = !sig.active && !connected && onConnectHealth != null &&
        (sig.label == "Sleep" || sig.label.contains("heart", ignoreCase = true))
    Row(
        Modifier
            .fillMaxWidth()
            .then(
                if (connectable) Modifier.clickableLabeled("Connect Health Connect for ${sig.label}") {
                    onConnectHealth?.invoke()
                } else Modifier
            )
            .heightIn(min = 36.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
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
        else Text(if (sig.active) sig.detail else "none", style = MaterialTheme.typography.bodySmall, color = c.muted)
    }
}
