package com.forge.app.ui.coach

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.ForgeRowPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.common.statsEntrance
import kotlin.math.ceil

/** Past weeks listed under the record rail: a few in Simple, more in Detailed. */
private const val WEEKS_LISTED_SIMPLE = 3
private const val WEEKS_LISTED_DETAILED = 6

/** Weeks the record rail draws, oldest on the left, this week on the right. */
private const val RAIL_WEEKS = 12

/** The fewest slots the rail divides its width into, so a short record keeps its spacing. */
private const val RAIL_MIN_SLOTS = 8

/** The outcome watcher's window: an applied change has two weeks to prove itself. */
private const val WATCH_WINDOW_MS = 14L * 24 * 60 * 60 * 1000

/**
 * THIS WEEK — the calls still asking for a decision, each a panel carrying the evidence it was made
 * from and its own Apply; then anything already decided this week, then last week's figures.
 */
internal fun LazyListScope.coachThisWeek(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    c: CoachColors,
    now: Long,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onUndo: (Long) -> Unit,
    onApplyAll: (String) -> Unit
) {
    val brief = state.brief ?: return
    val open = brief.decisions.filter { it.status == CoachRepository.STATUS_PROPOSED }
    val settled = brief.decisions.filter { it.status != CoachRepository.STATUS_PROPOSED }
    // While the baseline forms the Today panel already says everything this week has to say.
    if (brief.decisions.isEmpty() && brief.sessionsToGo > 0 && brief.review == null) return

    item("week-head") {
        CoachSectionHead(
            "This week",
            meta = when (open.size) {
                0 -> null
                1 -> "1 to decide"
                else -> "${open.size} to decide"
            },
            modifier = Modifier.statsEntrance(2)
        )
    }
    items(open, key = { "call-${it.id}" }) { d ->
        Column(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = COACH_GUTTER)
                .padding(bottom = 10.dp)
                .then(forgeItemMotion())
        ) {
            CoachCallTile(d, state, c, onApply, onSkip)
        }
    }
    // More than one open call: the sweep, as a text action, since each panel already carries its own.
    if (open.size > 1) {
        item("week-apply-all") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).padding(bottom = 6.dp)) {
                CoachAction("Apply all ${open.size} →", c.onBg, "Apply all ${open.size} changes") {
                    onApplyAll(brief.pass.weekId)
                }
            }
        }
    }
    if (settled.isNotEmpty()) {
        item("week-settled") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).then(forgeItemMotion())) {
                if (open.isNotEmpty()) Spacer(Modifier.height(6.dp))
                CoachRows(c, settled.map { d ->
                    @Composable { DecisionRow(d, now, c, onUndo, undoRefused = d.id in state.undoRefused) }
                })
            }
        }
    }
    // A week with nothing to decide still says so, in the coach's own words.
    if (brief.decisions.isEmpty() && brief.sessionsToGo == 0) {
        item("week-quiet") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER)) {
                CoachRows(c, listOf(@Composable {
                    Text(quietWeekLine(brief), style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                }))
            }
        }
    }
    // Last week's own figures close the section: what you actually did, against the plan.
    brief.review?.let { r ->
        item("week-figures") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).padding(horizontal = 4.dp)) {
                Spacer(Modifier.height(18.dp))
                // The review covers the week that ENDED: one caption says whose week it is.
                Text("Last week", style = MaterialTheme.typography.bodySmall, color = c.muted)
                Spacer(Modifier.height(6.dp))
                // A dense numeric row clamps its own scaling rather than truncating.
                val d = LocalDensity.current
                CompositionLocalProvider(LocalDensity provides Density(d.density, d.fontScale.coerceAtMost(1.3f))) {
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
}

/**
 * RECORD — whether the coach's calls have held.
 *
 * The answer leads ("5 of 6 held"), then twelve weeks as a rail of marks, one per week: a filled
 * disc for a change that held, a ring for one still proving out, red for one that missed or was
 * undone, a muted ring for a skipped call and a speck for a week with nothing to change. Under it,
 * the last few weeks in words, each change stamped with what became of it, with Undo while it can.
 */
internal fun LazyListScope.coachRecord(
    state: CoachViewModel.UiState,
    c: CoachColors,
    now: Long,
    onUndo: (Long) -> Unit
) {
    val brief = state.brief
    val record = state.timeline?.record
    val weeks = state.timeline?.weeks.orEmpty()
        .map(::withoutShadowCalls)
        .filterNot { w -> w.decisions.isEmpty() && AutoCoachPlanner.isLearningHold(w.pass.holdReason) }
    val past = weeks.filterNot { it.pass.weekId == brief?.pass?.weekId }
    val headline = record?.let(::recordLine)
    // Nothing proposed yet and no week on record: the Today panel's baseline says it all.
    if (headline == null && past.isEmpty()) return

    // A rail of one week only repeats what This week already says, so it waits for a second.
    val railWeeks = weeks.take(RAIL_WEEKS)
    val drawRail = railWeeks.size >= 2
    item("record") {
        CoachSection(
            "Record",
            meta = if (drawRail) "last ${railWeeks.size} weeks" else null,
            modifier = Modifier.statsEntrance(4)
        ) {
            Column(Modifier.padding(horizontal = 4.dp)) {
                headline?.let {
                    Text(it.value, style = MaterialTheme.typography.headlineSmall, color = c.onBg)
                    it.sub?.let { sub ->
                        Spacer(Modifier.height(2.dp))
                        Text(sub, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
                if (drawRail) {
                    if (headline != null) Spacer(Modifier.height(16.dp))
                    RecordRail(railWeeks.reversed().map { weekMark(it.decisions) }, c)
                }
            }
        }
    }

    val listed = past.take(if (state.advanced) WEEKS_LISTED_DETAILED else WEEKS_LISTED_SIMPLE)
    listed.forEach { week ->
        val label = coachWeekLabel(week.pass.weekId) ?: return@forEach
        item("record-${week.pass.weekId}") {
            Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).then(forgeItemMotion())) {
                Spacer(Modifier.height(18.dp))
                Text(label, style = MaterialTheme.typography.bodySmall, color = c.muted, modifier = Modifier.padding(horizontal = 4.dp))
                Spacer(Modifier.height(8.dp))
                if (week.decisions.isEmpty()) {
                    CoachRows(c, listOf(@Composable {
                        Text(
                            week.pass.holdReason?.takeIf { it.isNotBlank() }?.let(::recordHoldLine) ?: "No changes.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.muted
                        )
                    }))
                } else {
                    CoachRows(c, week.decisions.map { d ->
                        @Composable { DecisionRow(d, now, c, onUndo, undoRefused = d.id in state.undoRefused) }
                    })
                }
            }
        }
    }
    val more = past.size - listed.size
    if (more > 0) {
        item("record-more") {
            Text(
                "And $more more ${if (more == 1) "week" else "weeks"} on record.",
                style = MaterialTheme.typography.bodySmall,
                color = c.muted,
                modifier = Modifier.padding(horizontal = COACH_GUTTER + 4.dp).padding(top = 12.dp)
            )
        }
    }
}

/** What a week's mark on the record rail says. The worst outcome in the week wins. */
internal enum class WeekMark { HELD, PROVING, MISSED, DECLINED, OPEN, NONE }

internal fun weekMark(decisions: List<CoachDecision>): WeekMark {
    if (decisions.isEmpty()) return WeekMark.NONE
    val applied = { d: CoachDecision ->
        d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED
    }
    return when {
        decisions.any { it.outcome == "failed" || it.status == "reverted" } -> WeekMark.MISSED
        decisions.any { it.status == CoachRepository.STATUS_PROPOSED } -> WeekMark.OPEN
        decisions.any { applied(it) && it.outcome == "pending" } -> WeekMark.PROVING
        decisions.any { applied(it) } -> WeekMark.HELD
        decisions.all { it.status == CoachRepository.STATUS_SKIPPED } -> WeekMark.DECLINED
        else -> WeekMark.NONE
    }
}

private fun markWord(m: WeekMark) = when (m) {
    WeekMark.HELD -> "held"
    WeekMark.PROVING -> "proving out"
    WeekMark.MISSED -> "missed"
    WeekMark.DECLINED -> "skipped"
    WeekMark.OPEN -> "open"
    WeekMark.NONE -> "no change"
}

/**
 * The rail: one mark per week on a shared baseline, oldest left. Works at zero as a row of specks,
 * which is honest: weeks with nothing to change. A legend names only the marks actually drawn.
 */
@Composable
private fun RecordRail(marks: List<WeekMark>, c: CoachColors) {
    if (marks.isEmpty()) return
    Canvas(
        Modifier
            .fillMaxWidth()
            .height(20.dp)
            .semantics {
                contentDescription = "Last ${marks.size} weeks, oldest first: " +
                    marks.joinToString(", ") { markWord(it) }
            }
    ) {
        // A short record keeps a readable spacing rather than spreading four marks across the width.
        val slot = size.width / maxOf(marks.size, RAIL_MIN_SLOTS)
        val cy = size.height / 2f
        // The axis runs only between the weeks on record, broken around each mark so a ring reads
        // as a ring rather than a bead on a wire.
        val gap = 9.dp.toPx()
        for (i in 0 until marks.size - 1) {
            val x0 = slot * i + slot / 2f + gap
            val x1 = slot * (i + 1) + slot / 2f - gap
            if (x1 > x0) drawLine(c.outline.copy(alpha = 0.35f), Offset(x0, cy), Offset(x1, cy), 1.dp.toPx())
        }
        marks.forEachIndexed { i, m -> drawWeekMark(m, Offset(slot * i + slot / 2f, cy), c) }
    }
    Spacer(Modifier.height(8.dp))
    val shown = WeekMark.entries.filter { it in marks && it != WeekMark.NONE }
    if (shown.isNotEmpty()) {
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
            shown.forEach { m ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Canvas(Modifier.size(12.dp)) { drawWeekMark(m, Offset(size.width / 2f, size.height / 2f), c, scale = 0.8f) }
                    Spacer(Modifier.width(5.dp))
                    Text(markWord(m), style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
        }
    }
}

private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawWeekMark(
    m: WeekMark,
    center: Offset,
    c: CoachColors,
    scale: Float = 1f
) {
    val r = 6.dp.toPx() * scale
    val ring = Stroke(1.75.dp.toPx())
    when (m) {
        WeekMark.HELD -> drawCircle(c.accent, r, center)
        WeekMark.PROVING -> drawCircle(c.accent, r - ring.width / 2f, center, style = ring)
        WeekMark.OPEN -> drawCircle(c.onBg, r - ring.width / 2f, center, style = ring)
        WeekMark.MISSED -> drawCircle(c.error, r, center)
        WeekMark.DECLINED -> drawCircle(c.muted.copy(alpha = 0.65f), r - ring.width / 2f, center, style = ring)
        WeekMark.NONE -> drawCircle(c.muted.copy(alpha = 0.35f), 2.dp.toPx() * scale, center)
    }
}

/**
 * One decided change: what it was, what became of it, and while its window still runs, how much of
 * it is left. Undo rides the row itself, so a change is taken back from where it is read.
 */
@Composable
private fun DecisionRow(
    d: CoachDecision,
    now: Long,
    c: CoachColors,
    onUndo: (Long) -> Unit,
    undoRefused: Boolean = false
) {
    val copy = callCopy(d)
    // Non-null exactly when this change is still inside its two-week window.
    val watchingSince = d.appliedAt?.takeIf {
        d.outcome == "pending" &&
            (d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED)
    }
    // While the window runs, the bar and its line say how long is left, so the stamp stays the plain
    // lifecycle word rather than repeating the countdown.
    val word = if (watchingSince != null) "applied" else coachDecisionStatusWord(d, now)
    val failed = d.outcome == "failed" || d.status == "reverted"
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f)) {
            Text(copy.subject, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
            Text(copy.change, style = MaterialTheme.typography.bodySmall, color = c.muted)
        }
        Spacer(Modifier.width(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 3.dp)) {
            // The flag paints only for the exception: a change that missed or was undone.
            if (failed) {
                CoachFlagDot(c.error)
                Spacer(Modifier.width(6.dp))
            }
            CoachStamp(word, c)
        }
    }
    if (watchingSince != null) {
        val elapsed = (now - watchingSince).coerceIn(0, WATCH_WINDOW_MS)
        val daysLeft = ceil((WATCH_WINDOW_MS - elapsed) / (24.0 * 60 * 60 * 1000)).toInt()
        Spacer(Modifier.height(10.dp))
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
            if (daysLeft > 0) "$daysLeft ${if (daysLeft == 1) "day" else "days"} left to prove out" else "Verdict due",
            style = MaterialTheme.typography.bodySmall,
            color = c.muted
        )
    }
    // Undo only while the change is inside its undo window (stamped at apply time); rows applied
    // before that stamp existed carry null and keep the old behaviour. One the repository has just
    // declined is dead.
    val undoable = !undoRefused && d.status == CoachRepository.STATUS_APPLIED && d.undoData != null &&
        (d.undoExpiresAt == null || now <= d.undoExpiresAt!!)
    if (undoable) {
        Spacer(Modifier.height(10.dp))
        ForgeRowPill("Undo", Modifier.clickableLabeled("Undo this change") { onUndo(d.id) })
    }
}

/**
 * A week the coach was switched off for records its would-be proposals as inert SHADOW rows: never
 * proposed, never applied. They are observations, not calls, so the record does not list or count
 * them; a week with only those reads as a week with none.
 */
private fun withoutShadowCalls(w: CoachRepository.CoachHistoryEntry): CoachRepository.CoachHistoryEntry =
    w.copy(decisions = w.decisions.filterNot { it.status == CoachRepository.STATUS_SHADOW })

/** What a week with no calls has to say for itself. */
private fun quietWeekLine(brief: com.forge.app.data.repo.CoachBrief): String {
    if (brief.pass.status == CoachRepository.STATUS_ERROR) {
        return brief.pass.holdReason?.let(::recordHoldLine) ?: "Nothing was considered this week."
    }
    brief.pass.holdReason?.takeIf { it.isNotBlank() }?.let { return recordHoldLine(it) }
    return "Nothing to change. Keep running the plan."
}
