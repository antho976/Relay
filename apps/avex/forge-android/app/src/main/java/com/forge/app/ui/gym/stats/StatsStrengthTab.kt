package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.components.EntranceItem
import com.forge.app.ui.gym.stats.components.HallOfFameRow
import com.forge.app.ui.gym.stats.components.PatternRadarCard
import com.forge.app.ui.gym.stats.components.StrengthOverlayCard
import com.forge.app.ui.gym.stats.components.TimeToPrCard
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.StatsUiState

/**
 * The Strength tab: e1RM hero → lift ledger → plateau ladder → PR timeline → movement
 * radar → rep maxes → overlay → all-time bests. Engine signals (plateau flags, PR
 * recency, pattern radar) ride the same StatsUiState as everything else.
 */
internal fun LazyListScope.strengthTab(state: StatsUiState, c: StatsColors, onLiftClick: (E1rmLift) -> Unit = {}, onShareTopLift: (() -> Unit)? = null) {
    // Cold start: zero weighted history → one intentional editorial screen, no gated crumbs.
    if (state.e1rmLifts.isEmpty() && state.hallOfFame.isEmpty()) {
        item("strength-cold") { EntranceItem(0) { StrengthColdStart(c.onBg, c.muted, c.outline) } }
        return
    }
    var idx = 0
    val plateausById = state.plateauFlags.associateBy { it.exerciseId }
    val prDays = state.prRecency?.byExercise.orEmpty()

    state.e1rmLifts.firstOrNull()?.let { top ->
        val i = idx++
        item("e1rm-chart") { EntranceItem(i) { E1rmChartCard(top, c.onBg, c.muted, c.accent, c.outline, onShare = onShareTopLift) } }
    }
    if (state.e1rmLifts.isNotEmpty()) {
        val i = idx++
        item("big-lifts") {
            EntranceItem(i) { E1rmCard(state.e1rmLifts, prDays, plateausById, c.onBg, c.muted, c.accent, c.outline, onLiftClick = onLiftClick) }
        }
    }
    if (state.plateauFlags.isNotEmpty()) {
        val i = idx++
        item("plateaus") { EntranceItem(i) { PlateauSection(state.plateauFlags, c.onBg, c.muted, c.outline) } }
    }
    if (state.recentPrs.isNotEmpty()) {
        sectionTitle("pr-title", "Records broken", c)
        // Celebrate PRs set in the current ISO week (Monday-anchored, like the rest of the app).
        val weekStartMs = com.forge.app.core.time.mondayStartMs(System.currentTimeMillis())
        val prsThisWeek = state.recentPrs.count { it.date >= weekStartMs }
        if (prsThisWeek > 0) {
            item("pr-thisweek") {
                EntranceItem(0) {
                    Text(
                        "▲ $prsThisWeek PR${if (prsThisWeek > 1) "s" else ""} this week",
                        style = MaterialTheme.typography.labelLarge, color = c.accent,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 4.dp)
                    )
                }
            }
        }
        state.prRecency?.let { rec ->
            val i = idx++
            item("pr-drought") { EntranceItem(i) { PrDroughtHeader(rec.daysSinceLast, c.onBg, c.muted) } }
        }
        val lastPr = state.recentPrs.lastIndex
        itemsIndexed(state.recentPrs, key = { _, pr -> "pr-${pr.date}-${pr.exerciseName}" }) { rowIdx, pr ->
            EntranceItem(rowIdx.coerceAtMost(8)) {
                PrTimelineRow(
                    pr = pr, isFirst = rowIdx == 0, isLast = rowIdx == lastPr,
                    onBg = c.onBg, muted = c.muted, accent = c.accent, outline = c.outline,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)
                )
            }
        }
        item("pr-end") {
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                Spacer(Modifier.height(8.dp))
                HorizontalDivider(color = c.outline.copy(alpha = 0.25f))
                Spacer(Modifier.height(20.dp))
            }
        }
    }
    if (state.patternRadar.isNotEmpty()) {
        val i = idx++
        item("radar") { EntranceItem(i) { PatternRadarCard(state.patternRadar) } }
    }
    state.repMaxes?.let { rm ->
        if (rm.entries.isNotEmpty()) {
            val i = idx++
            item("repmax") { EntranceItem(i) { RepMaxCard(rm, c.onBg, c.muted, c.outline) } }
        }
    }
    val withHistory = state.exerciseHistory.entries.filter { it.value.size >= 2 }.sortedByDescending { it.value.size }
    if (withHistory.size >= 2) {
        val a = withHistory[0]; val b = withHistory[1]
        cardItem("overlay") {
            StrengthOverlayCard(
                history1 = Program.exerciseDisplayName(a.key) to a.value,
                history2 = Program.exerciseDisplayName(b.key) to b.value
            )
        }
    }
    sectionTitle("hof-title", "All-time bests", c)
    if (state.hallOfFame.isEmpty()) {
        item("hof-empty") {
            Text("No records yet. Finish a session to set your first PR.",
                style = MaterialTheme.typography.bodySmall, color = c.muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp))
        }
    } else {
        itemsIndexed(state.hallOfFame.take(10), key = { _, r -> "hof-${r.exerciseId}" }) { rowIdx, record ->
            EntranceItem(rowIdx.coerceAtMost(8)) {
                HallOfFameRow(record = record, muted = c.muted, onBg = c.onBg, accent = c.accent,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp))
            }
        }
    }
    if (state.timeToPr.isNotEmpty()) {
        cardItem("time-to-pr") { TimeToPrCard(state.timeToPr) }
    }
}
