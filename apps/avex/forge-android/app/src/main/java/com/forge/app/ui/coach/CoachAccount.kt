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
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.CompositionLocalProvider
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.common.forgeItemMotion
import kotlin.math.ceil

/** How many past weeks the account draws inline before it says how many it did not. */
private const val RECORD_WEEKS = 6

/** The outcome watcher's window: an applied change has two weeks to prove itself. */
private const val WATCH_WINDOW_MS = 14L * 24 * 60 * 60 * 1000

/**
 * THE ACCOUNT — this week's calls and every week before them, on one spine.
 *
 * There are no lenses here. A proposal, an applied change still proving itself, and a call from
 * five weeks ago that failed are all the same kind of object: an entry with a node on the time
 * axis and a stamp saying what became of it. That is the whole structure, and it is what lets the
 * two real scenes share one screen — on Monday you act on what is at the top, and idly you keep
 * scrolling into your own history.
 *
 * Nothing is deleted or moved when it resolves. An open call loses its body and becomes a stamped
 * line in place, which is the page's one authored moment.
 */
internal fun LazyListScope.coachAccount(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    c: CoachColors,
    now: Long,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    onApplyAll: (String) -> Unit
) {
    val brief = state.brief
    val open = brief?.decisions.orEmpty().filter { it.status == CoachRepository.STATUS_PROPOSED }
    val settled = brief?.decisions.orEmpty().filter { it.status != CoachRepository.STATUS_PROPOSED }

    // ── This week ────────────────────────────────────────────────────────────
    item("acct-anchor-now") {
        Column(Modifier.fillMaxWidth().ledgerSpine(c, top = false).statsEntrance(0)) {
            Spacer(Modifier.height(4.dp))
            CoachAnchor(
                "This week",
                c,
                meta = nextBriefMeta(state.daysToNextBrief),
                modifier = Modifier.padding(horizontal = COACH_GUTTER)
            )
            Spacer(Modifier.height(14.dp))
        }
    }

    // The open calls, each a full entry with its own evidence and its own commit.
    items(open, key = { "call-${it.id}" }) { d ->
        Column(
            Modifier
                .fillMaxWidth()
                .ledgerSpine(c, node = EntryNode.OPEN, nodeY = NODE_ON_TILE)
                .padding(horizontal = COACH_GUTTER)
                .padding(bottom = 12.dp)
                .then(forgeItemMotion())
        ) {
            CoachCallTile(d, state, c, onApply, onSkip)
        }
    }

    // With more than one open call the page offers the sweep, as a link rather than a second
    // filled control: the tiles already carry the primary action, one apiece.
    if (open.size > 1 && brief != null) {
        item("acct-apply-all") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .ledgerSpine(c)
                    .padding(horizontal = COACH_GUTTER)
                    .padding(bottom = 12.dp)
            ) {
                CoachAction(
                    "Apply all ${open.size} →",
                    c.accent,
                    "Apply all ${open.size} changes"
                ) { onApplyAll(brief.pass.weekId) }
            }
        }
    }

    // Anything already decided this week, stamped in place beside the open ones.
    items(settled, key = { "settled-${it.id}" }) { d ->
        Column(
            Modifier
                .fillMaxWidth()
                .ledgerSpine(c, node = entryNode(d))
                .padding(horizontal = COACH_GUTTER)
                .then(forgeItemMotion())
        ) {
            DecisionEntry(d, now, c, onUndo)
        }
    }

    // A week with nothing to decide still says so, in the coach's own words.
    if (brief != null && brief.decisions.isEmpty()) {
        item("acct-quiet") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .ledgerSpine(c, node = EntryNode.PLAIN)
                    .padding(horizontal = COACH_GUTTER)
                    .padding(bottom = 18.dp)
            ) {
                if (brief.sessionsToGo > 0) BaselineEntry(brief, c)
                else Text(
                    quietWeekLine(brief),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg
                )
            }
        }
    }

    // The week's own figures close it: what you actually did, against what was planned.
    brief?.review?.let { r ->
        item("acct-figures") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .ledgerSpine(c)
                    .padding(horizontal = COACH_GUTTER)
                    .padding(top = 2.dp, bottom = 26.dp)
            ) {
                // A dense numeric row clamps its own scaling rather than truncating: a readable
                // "4 of 4" at 130% beats an unreadable "4 …" at 200%.
                val d = LocalDensity.current
                CompositionLocalProvider(
                    LocalDensity provides Density(d.density, d.fontScale.coerceAtMost(1.3f))
                ) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        EditorialFigure(
                            value = "${r.sessionsLastWeek} of ${r.sessionsTarget}",
                            label = "Sessions",
                            onBg = c.onBg, muted = c.muted, accent = c.accent,
                            modifier = Modifier.weight(1f)
                        )
                        EditorialFigure(
                            value = formatVolumeCompact(r.volumeLastWeekLb, weightUnit),
                            label = "Volume",
                            onBg = c.onBg, muted = c.muted, accent = c.accent,
                            delta = r.volumeDeltaPct,
                            // Volume carries the longest value AND the only delta badge; on equal
                            // thirds the badge wrapped into two lines and collided with the PRs
                            // figure beside it, reading as one run-on number.
                            modifier = Modifier.weight(1.5f)
                        )
                        EditorialFigure(
                            value = "${r.prsLastWeek}",
                            label = "PRs",
                            onBg = c.onBg, muted = c.muted, accent = c.accent,
                            modifier = Modifier.weight(0.7f)
                        )
                    }
                }
            }
        }
    }


    // ── The weeks before ─────────────────────────────────────────────────────
    // A pre-baseline learning hold is not a record of anything: no calls, and its one line only
    // restates the countdown the top of the page already carries. The account is the changelog of
    // real calls, not the wait for the first one.
    val past = state.timeline?.weeks
        .orEmpty()
        .filterNot { it.pass.weekId == brief?.pass?.weekId }
        .filterNot { w ->
            w.decisions.isEmpty() && AutoCoachPlanner.isLearningHold(w.pass.holdReason)
        }
    val shown = past.take(RECORD_WEEKS)

    shown.forEachIndexed { wi, week ->
        val label = coachWeekLabel(week.pass.weekId) ?: return@forEachIndexed
        val last = wi == shown.lastIndex
        item("acct-week-${week.pass.weekId}") {
            Column(Modifier.fillMaxWidth().ledgerSpine(c)) {
                Spacer(Modifier.height(8.dp))
                CoachAnchor(
                    label,
                    c,
                    meta = week.decisions.size.takeIf { it > 0 }
                        ?.let { "$it call${if (it == 1) "" else "s"}" },
                    modifier = Modifier.padding(horizontal = COACH_GUTTER)
                )
                Spacer(Modifier.height(12.dp))
            }
        }
        if (week.decisions.isEmpty()) {
            item("acct-week-${week.pass.weekId}-held") {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .ledgerSpine(c, node = EntryNode.PLAIN, bottom = !last)
                        .padding(horizontal = COACH_GUTTER)
                        .padding(bottom = 18.dp)
                ) {
                    Text(
                        week.pass.holdReason?.takeIf { it.isNotBlank() }?.let(::recordHoldLine)
                            ?: "No changes.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.muted
                    )
                }
            }
        } else {
            week.decisions.forEachIndexed { di, d ->
                item("acct-past-${d.id}") {
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .ledgerSpine(
                                c,
                                node = entryNode(d),
                                bottom = !(last && di == week.decisions.lastIndex)
                            )
                            .padding(horizontal = COACH_GUTTER)
                    ) {
                        DecisionEntry(d, now, c, onUndo)
                    }
                }
            }
        }
    }

    val more = past.size - shown.size
    if (more > 0) {
        item("acct-more") {
            Text(
                "And $more more week${if (more == 1) "" else "s"}.",
                style = MaterialTheme.typography.bodySmall,
                color = c.muted,
                modifier = Modifier.padding(horizontal = COACH_GUTTER).padding(bottom = 8.dp)
            )
        }
    }
}

/**
 * One resolved entry: what the change was, what became of it, and — while its window is still
 * running — how much of that window is left. Undo rides the entry itself, so a change can be
 * taken back from the same place it is read.
 */
@Composable
private fun DecisionEntry(
    d: CoachDecision,
    now: Long,
    c: CoachColors,
    onUndo: (Long) -> Unit
) {
    val copy = callCopy(d)
    // Non-null exactly when this change is still inside its two-week window.
    val watchingSince = d.appliedAt?.takeIf {
        d.outcome == "pending" &&
            (d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED)
    }
    val watching = watchingSince != null
    // While the window runs, the bar and its line below say how long is left, so the stamp stays
    // the plain lifecycle word rather than repeating the countdown out to the right.
    val word = if (watching) "applied" else coachDecisionStatusWord(d, now)
    Column(Modifier.fillMaxWidth().padding(bottom = 18.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
            Text(
                "${copy.subject} · ${copy.change}",
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(12.dp))
            CoachStamp(word, c)
        }
        // A change still inside its window draws that window, so "applied" carries a reading
        // rather than standing alone as a status word.
        if (watchingSince != null) {
            val elapsed = (now - watchingSince).coerceIn(0, WATCH_WINDOW_MS)
            val daysLeft = ceil((WATCH_WINDOW_MS - elapsed) / (24.0 * 60 * 60 * 1000)).toInt()
            Spacer(Modifier.height(8.dp))
            CoachWatchBar(
                elapsed.toFloat() / WATCH_WINDOW_MS,
                c.secondary,
                c,
                modifier = Modifier.semantics {
                    contentDescription = "Proving out, $daysLeft days left of the two-week window"
                }
            )
            Spacer(Modifier.height(6.dp))
            Text(
                (if (daysLeft > 0) "$daysLeft day${if (daysLeft == 1) "" else "s"} left to prove out"
                else "Verdict due").uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = c.muted
            )
        }
        // A per-row action is a drawn outlined pill, not a bare word. As muted text it sat dimmer
        // than the entry it acts on and read as a caption; accent would have been unreadable on
        // four of the five accent choices.
        // Undo is offered only while the change is still inside its undo window (stamped at apply
        // time). Rows applied before that stamp existed carry null and keep the old behaviour.
        val undoable = d.status == CoachRepository.STATUS_APPLIED && d.undoData != null &&
            (d.undoExpiresAt == null || now <= d.undoExpiresAt!!)
        if (undoable) {
            Spacer(Modifier.height(10.dp))
            ForgeRowPill(
                "Undo",
                Modifier.clickableLabeled("Undo this change") { onUndo(d.id) }
            )
        }
    }
}

/**
 * The account's first entry on a new account: the baseline the coach is still gathering.
 *
 * This is the only thing a first-run user has above the fold, so it is drawn at the size of the
 * fact rather than as a caption. A count, the meter it fills, and what happens when it fills.
 */
@Composable
private fun BaselineEntry(brief: com.forge.app.data.repo.CoachBrief, c: CoachColors) {
    val logged = brief.sessionsLogged
    val needed = brief.minSessions.coerceAtLeast(1)
    val toGo = brief.sessionsToGo
    Text("Baseline still forming", style = MaterialTheme.typography.titleMedium, color = c.onBg)
    Spacer(Modifier.height(10.dp))
    Text(
        "$logged of $needed",
        style = MaterialTheme.typography.headlineMedium,
        color = c.onBg
    )
    Spacer(Modifier.height(12.dp))
    TrustProgressBar(
        streak = logged.coerceAtMost(needed),
        required = needed,
        earned = toGo == 0,
        modifier = Modifier.fillMaxWidth().semantics {
            contentDescription = "$logged of $needed baseline sessions logged"
        }
    )
    Spacer(Modifier.height(10.dp))
    Text(
        if (toGo > 0) "$toGo more session${if (toGo == 1) "" else "s"} and the coach makes its first call."
        else "Your first call lands with the next brief.",
        style = MaterialTheme.typography.bodySmall,
        color = c.muted
    )
}

/**
 * How many items [coachAccount] emits, so a deep link can scroll past the account to the reading
 * below it. It mirrors the emission above exactly — change one and change the other.
 */
internal fun accountItemCount(state: CoachViewModel.UiState): Int {
    val brief = state.brief
    val decisions = brief?.decisions.orEmpty()
    val open = decisions.count { it.status == CoachRepository.STATUS_PROPOSED }
    val settled = decisions.size - open
    var n = 1 // the "This week" anchor
    n += open
    if (open > 1 && brief != null) n += 1 // apply-all
    n += settled
    if (brief != null && decisions.isEmpty()) n += 1 // the quiet / baseline entry
    if (brief?.review != null) n += 1 // the week's figures

    val past = state.timeline?.weeks
        .orEmpty()
        .filterNot { it.pass.weekId == brief?.pass?.weekId }
        .filterNot { w -> w.decisions.isEmpty() && AutoCoachPlanner.isLearningHold(w.pass.holdReason) }
    val shown = past.take(RECORD_WEEKS)
    shown.forEach { week ->
        if (coachWeekLabel(week.pass.weekId) == null) return@forEach
        n += 1 // the week's anchor
        n += if (week.decisions.isEmpty()) 1 else week.decisions.size
    }
    if (past.size > shown.size) n += 1 // the "and N more weeks" line
    return n
}

/** The lifecycle a node draws. The stamp beside it carries the outcome, so neither repeats. */
private fun entryNode(d: CoachDecision): EntryNode = when {
    d.status == CoachRepository.STATUS_PROPOSED -> EntryNode.OPEN
    d.outcome == "failed" || d.status == "reverted" -> EntryNode.FAILED
    d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED ->
        EntryNode.APPLIED
    d.status == CoachRepository.STATUS_SKIPPED -> EntryNode.DECLINED
    else -> EntryNode.PLAIN
}

/** What a week with no calls has to say for itself. */
private fun quietWeekLine(brief: com.forge.app.data.repo.CoachBrief): String {
    if (brief.pass.status == CoachRepository.STATUS_ERROR) {
        return brief.pass.holdReason?.let(::recordHoldLine) ?: "Nothing was considered this week."
    }
    brief.pass.holdReason?.takeIf { it.isNotBlank() }?.let { return recordHoldLine(it) }
    return "Nothing to change. Keep running the plan."
}

/** The countdown to the next pass, worded forward and naming the day it lands. */
private fun nextBriefMeta(days: Int): String? = when {
    days <= 0 || days > 7 -> null
    days == 1 -> "next brief tomorrow"
    else -> "next brief in $days days"
}
