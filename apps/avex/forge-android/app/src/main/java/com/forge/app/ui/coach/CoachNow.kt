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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.CoachRecord
import com.forge.app.ui.common.statsEntrance

/**
 * One reading in WHERE YOU STAND: the question on the left, the answer on the right, and the
 * evidence for that answer under both. Pure data so the wording is testable without Compose.
 */
internal data class StandingLine(val label: String, val value: String, val sub: String?)

/**
 * WHERE YOU STAND — the four answers a reader wants from the coach without opening its
 * instruments: how today looks, how the week's load looks, which way the lifts are going, and
 * whether the coach's changes have been worth taking.
 *
 * Each is one line with its reason under it, and each speaks only from real data. Below a gate the
 * line says how far along the gate is, never "not enough data". Under advanced tracking the Signals
 * region below draws recovery load and the lifts in full, so this region keeps only the two readings
 * nothing else on the page carries: readiness and the record.
 */
internal fun LazyListScope.coachNow(state: CoachViewModel.UiState, c: CoachColors) {
    val lines = standingLines(state)
    if (lines.isEmpty()) return
    item("now") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(1)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor("Where you stand", c)
            Spacer(Modifier.height(8.dp))
            lines.forEach { StandingRow(it, c) }
        }
    }
}

/** True when [coachNow] draws anything, so a deep link knows there is a reading to land on. */
internal fun hasStanding(state: CoachViewModel.UiState): Boolean = standingLines(state).isNotEmpty()

internal fun standingLines(state: CoachViewModel.UiState): List<StandingLine> = buildList {
    readinessLine(state)?.let(::add)
    if (!state.advanced) {
        recoveryLine(state)?.let(::add)
        liftsLine(state)?.let(::add)
    }
    state.timeline?.record?.let(::recordLine)?.let(::add)
}

/** Today's readiness, as the change it makes to the day's weight targets and why. */
internal fun readinessLine(state: CoachViewModel.UiState): StandingLine? {
    val logged = state.watch?.sessionsLogged ?: state.brief?.sessionsLogged ?: return null
    val gate = state.readinessGateSessions.coerceAtLeast(1)
    val r = state.readiness
    return when {
        r != null && r.percent != 0 -> StandingLine(
            label = "Readiness today",
            value = if (r.percent < 0) "Targets ${-r.percent}% lighter" else "Targets ${r.percent}% heavier",
            sub = r.reason.takeIf { it.isNotBlank() }?.replaceFirstChar { it.uppercase() }
        )
        logged >= gate -> StandingLine(
            label = "Readiness today",
            value = "As planned",
            sub = "Nothing today moves your targets."
        )
        else -> StandingLine(
            label = "Readiness today",
            value = "Forming",
            sub = "$logged of $gate sessions logged, first read after the $gate${ordinalSuffix(gate)}."
        )
    }
}

/**
 * Recovery load as one word, in the same bands the Stats pulse and the weekly review use, with the
 * checks that crossed their line as the reason. The meter and every check live under Signals.
 */
internal fun recoveryLine(state: CoachViewModel.UiState): StandingLine? {
    val watch = state.watch ?: return null
    val score = watch.fatigueScore
    val threshold = watch.fatigueThreshold
    val gate = watch.recoveryGateSessions.coerceAtLeast(1)
    val label = "Recovery load"
    return when {
        score != null -> {
            val band = when {
                score >= threshold -> "Deload soon"
                score >= threshold - 2 -> "Building"
                else -> "Fresh"
            }
            // The score leads so the band reads against its number: two checks can cross their
            // line while the total is still fresh, and a bare list of them read as a contradiction.
            val fired = watch.fatigueChecks.filter { it.fired }.map { it.name }
            val flagged = when {
                fired.isEmpty() -> null
                fired.size <= 2 -> fired.joinToString(", ")
                else -> fired.take(2).joinToString(", ") + " and ${fired.size - 2} more"
            }
            val sub = "$score of $threshold toward a deload" + (flagged?.let { " · $it" } ?: ".")
            StandingLine(label, band, sub)
        }
        // All gates met yet no score: the advisor mutes itself right after a deload.
        watch.sessionsLogged >= gate && watch.historyDays >= watch.recoveryWindowDays ->
            StandingLine(label, "Paused", "Resumes as training rebuilds after the deload.")
        watch.sessionsLogged < gate ->
            StandingLine(label, "Forming", "${watch.sessionsLogged} of $gate sessions logged.")
        else -> StandingLine(
            label,
            "Forming",
            "${watch.historyDays} of ${watch.recoveryWindowDays} days of training on record."
        )
    }
}

/** Which way the watched lifts are going, counted with the same call the Signals rows make. */
internal fun liftsLine(state: CoachViewModel.UiState): StandingLine? {
    val lifts = state.watch?.trackedLifts.orEmpty()
    if (lifts.isEmpty()) return null
    val (withTrend, forming) = lifts.partition { (state.e1rmBySlot[it.slotId]?.size ?: 0) >= 2 }
    if (withTrend.isEmpty()) {
        return StandingLine(
            "Lifts",
            "Forming",
            "${plural(forming.size, "lift")} building history, first read after two sessions."
        )
    }
    val moves = withTrend.map { it to liftMove(it, state.e1rmBySlot.getValue(it.slotId)) }
    val up = moves.count { it.second == LiftMove.UP }
    val flat = moves.count { it.second == LiftMove.FLAT }
    val down = moves.count { it.second == LiftMove.DOWN }
    val stalled = moves.filter { it.second == LiftMove.STALLED }.map { it.first.name }
    val rest = buildList {
        // A stall is the coach's own call, so it leads and names its lifts while they fit a line.
        when (stalled.size) {
            0 -> Unit
            1 -> add("${stalled[0]} stalled")
            2 -> add("${stalled[0]} and ${stalled[1]} stalled")
            else -> add("${stalled.size} stalled")
        }
        if (down > 0) add("$down slipping")
        if (flat > 0) add("$flat flat")
        if (forming.isNotEmpty()) add("${forming.size} still forming")
    }
    return StandingLine(
        "Lifts",
        "$up of ${withTrend.size} climbing",
        rest.takeIf { it.isNotEmpty() }?.joinToString(" · ")
    )
}

/**
 * Whether the coach's changes have been worth taking: every applied change against its verdict.
 * Absent until the coach has proposed anything, since the baseline entry above already says why.
 */
internal fun recordLine(record: CoachRecord): StandingLine? {
    val label = "Changes applied"
    if (record.applied == 0) {
        if (!record.anyProposed) return null
        return StandingLine(label, "None yet", "Each change you apply gets two weeks to prove itself.")
    }
    val rest = buildList {
        if (record.missed > 0) add("${record.missed} missed")
        if (record.watching > 0) add("${record.watching} still proving out")
        if (record.undone > 0) add("${record.undone} undone")
    }
    return StandingLine(
        label,
        "${record.held} of ${record.applied} held",
        when {
            rest.isNotEmpty() -> rest.joinToString(" · ")
            record.held == record.applied -> "Every one held through its two weeks."
            else -> null
        }
    )
}

@Composable
private fun StandingRow(line: StandingLine, c: CoachColors) {
    // One node for TalkBack: the question, the answer and the reason read as one sentence.
    Column(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {}
            .padding(vertical = COACH_ROW_PAD + 4.dp)
    ) {
        val label = @Composable { mod: Modifier ->
            Text(line.label, style = MaterialTheme.typography.bodyMedium, color = c.muted, modifier = mod)
        }
        val value = @Composable { mod: Modifier, align: TextAlign ->
            Text(
                line.value,
                style = MaterialTheme.typography.titleSmall,
                color = c.onBg,
                textAlign = align,
                modifier = mod
            )
        }
        // Side by side the answer sits flush right; past ~130% the pair stacks rather than
        // squeezing the answer onto three lines. Nothing on this page truncates.
        if (LocalDensity.current.fontScale > 1.3f) {
            label(Modifier)
            value(Modifier, TextAlign.Start)
        } else {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Top
            ) {
                label(Modifier.weight(1f))
                Spacer(Modifier.width(12.dp))
                value(Modifier.weight(1f, fill = false), TextAlign.End)
            }
        }
        if (line.sub != null) {
            Spacer(Modifier.height(2.dp))
            Text(line.sub, style = MaterialTheme.typography.bodySmall, color = c.muted)
        }
    }
}

private fun plural(n: Int, word: String) = "$n $word${if (n == 1) "" else "s"}"

private fun ordinalSuffix(n: Int): String = when {
    n % 100 in 11..13 -> "th"
    n % 10 == 1 -> "st"
    n % 10 == 2 -> "nd"
    n % 10 == 3 -> "rd"
    else -> "th"
}
