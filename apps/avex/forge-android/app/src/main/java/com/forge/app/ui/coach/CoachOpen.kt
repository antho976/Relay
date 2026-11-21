package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.common.ChartPoint
import com.forge.app.ui.common.OpenGoalColumn
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenSectionHead
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.OpenStackedColumns
import com.forge.app.ui.common.OpenSwatchKey
import com.forge.app.ui.common.OpenTrendChart
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.TemporalAdjusters
import kotlin.math.ceil
import kotlin.math.roundToInt

/*
 * Coach, as redesigned 2026-10-21 (Relay design 7, "Coach final"): three tabs.
 *
 *   Brief   today's read, then this week's open calls, each with its evidence and an even
 *           Skip / Apply pair, and what was decided.
 *   Record  whether the calls held: held, on watch, missed and skipped per week, what is on watch
 *           now, and how earlier calls ended.
 *   Plan    the longer arc: the training block, the goals, and the one project being worked.
 */

enum class CoachTab(val label: String) { BRIEF("Brief"), RECORD("Record"), PLAN("Plan") }

private val SECTION = 52.dp
private const val WATCH_MS = 14L * 24 * 60 * 60 * 1000
private const val DAY_MS = 24L * 60 * 60 * 1000
private const val RECORD_WEEKS = 12

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Head(left: String, right: String? = null, rightInk: Boolean = false) {
    // A flow row, so a large font drops the right label under the left one instead of running into it.
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OpenLabel(left)
        if (right != null) OpenLabel(right, color = if (rightInk) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Serif(text: String, style: TextStyle = OpenType.figureMS) {
    Text(text, style = style, color = MaterialTheme.colorScheme.onBackground)
}

@Composable
private fun Body(text: String) {
    Text(text, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun Caption(text: String, ink: Boolean = false) {
    OpenLabel(text, style = OpenType.monoXS, color = if (ink) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
}

// ── Brief ──────────────────────────────────────────────────────────────────────────────────────

@Composable
internal fun CoachBriefTab(state: CoachViewModel.UiState, unit: WeightUnit, now: Long, actions: CoachActions) {
    val brief = state.brief
    Column {
        // Today: readiness as what it does to today's targets, the reason, then two readings.
        val forming = (brief?.sessionsToGo ?: 0) > 0
        val readiness = readinessLine(state)
        Head("Today", "Readiness")
        Spacer(Modifier.height(10.dp))
        if (forming && brief != null) {
            Serif("${brief.sessionsLogged} of ${brief.minSessions.coerceAtLeast(1)} sessions")
            Spacer(Modifier.height(8.dp))
            Body("Learning your baseline. ${brief.sessionsToGo} more ${if (brief.sessionsToGo == 1) "session" else "sessions"} and the coach makes its first call.")
        } else if (readiness != null) {
            Serif(readiness.value)
            readiness.sub?.let { Spacer(Modifier.height(8.dp)); Body(it) }
        }
        Spacer(Modifier.height(22.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
            FatigueFigure(state, Modifier.weight(1f))
            NextBriefFigure(state.daysToNextBrief, now, Modifier.weight(1f))
        }

        if (brief == null) return@Column
        val open = brief.decisions.filter { it.status == CoachRepository.STATUS_PROPOSED }
        val settled = brief.decisions.filter { it.status != CoachRepository.STATUS_PROPOSED && it.status != CoachRepository.STATUS_SHADOW }
        if (brief.decisions.isEmpty() && forming) return@Column

        Spacer(Modifier.height(SECTION))
        Head(if (open.isEmpty()) "This week" else "This week · ${open.size} open", "From Monday's brief")
        Spacer(Modifier.height(20.dp))
        if (open.isEmpty()) {
            Body(quietWeek(brief))
        }
        open.forEachIndexed { i, d ->
            if (i > 0) Spacer(Modifier.height(48.dp))
            CallBlock(d, state, unit, actions)
        }
        if (settled.isNotEmpty()) {
            Spacer(Modifier.height(40.dp))
            Head("Decided this week")
            Spacer(Modifier.height(14.dp))
            Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
                settled.forEach { DecidedRow(it, now, state, actions) }
            }
        }
    }
}

private fun quietWeek(brief: com.forge.app.data.repo.CoachBrief): String {
    brief.pass.holdReason?.takeIf { it.isNotBlank() }?.let { return recordHoldLine(it) }
    return "Nothing to change this week. Keep running the plan."
}

/** Fatigue against its deload line: "34/60", the meter with a tick at the line, the band under it. */
@Composable
private fun FatigueFigure(state: CoachViewModel.UiState, modifier: Modifier) {
    val watch = state.watch
    val score = watch?.fatigueScore
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OpenLabel("Fatigue")
        if (watch == null || score == null) {
            Text(
                buildAnnotatedString { append("0"); withStyle(OpenType.figureMS.toSpanStyle().copy(color = MaterialTheme.colorScheme.onSurfaceVariant)) { append("/${watch?.fatigueThreshold ?: 0}") } },
                style = OpenType.figureMS, color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            OpenMeter(0f)
            Caption(recoveryLine(state)?.value ?: "Forming")
            return@Column
        }
        val threshold = watch.fatigueThreshold.coerceAtLeast(1)
        Text(
            buildAnnotatedString {
                append("$score")
                withStyle(OpenType.figureMS.toSpanStyle().copy(color = MaterialTheme.colorScheme.onSurfaceVariant)) { append("/$threshold") }
            },
            style = OpenType.figureMS, color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.semantics { contentDescription = "Fatigue $score of $threshold before a deload" }
        )
        OpenMeter(score.toFloat() / threshold)
        Caption("Deload at $threshold" + (recoveryLine(state)?.value?.let { " · ${it.lowercase()}" } ?: ""))
    }
}

/** When the next weekly brief lands: the date, a meter across the week, and the days to go. */
@Composable
private fun NextBriefFigure(daysToNext: Int, now: Long, modifier: Modifier) {
    val days = if (daysToNext in 1..7) daysToNext else 7
    val date = Instant.ofEpochMilli(now).atZone(ZoneId.systemDefault()).toLocalDate().plusDays(days.toLong())
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OpenLabel("Next brief")
        Serif(date.format(DateTimeFormatter.ofPattern("EEE d", currentLocale())))
        OpenMeter((7 - days) / 7f)
        Caption(if (days == 1) "Tomorrow" else "In $days days")
    }
}

/**
 * One open call: the lift in mono with what kind of change it is, the change in serif, the reason,
 * the evidence it was read from, and an even Skip / Apply pair.
 */
@Composable
private fun CallBlock(d: CoachDecision, state: CoachViewModel.UiState, unit: WeightUnit, actions: CoachActions) {
    val copy = callCopy(d)
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenLabel(copy.subject, color = MaterialTheme.colorScheme.onBackground)
            callKind(d.type)?.let { KindTag(it) }
        }
        Serif(copy.change, OpenType.figureRow)
        if (copy.reason.isNotBlank()) Body(copy.reason)
        Evidence(d, state, unit)
        Spacer(Modifier.height(6.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            OpenSmallButton("Skip", filled = false, onClick = { actions.skip(d.id) })
            OpenSmallButton("Apply", filled = true, onClick = { actions.apply(d.id) })
        }
    }
}

@Composable
private fun KindTag(word: String) {
    Box(
        Modifier
            .border(1.dp, OpenColors.Rule, RoundedCornerShape(2.dp))
            .padding(horizontal = 6.dp, vertical = 2.dp)
    ) {
        Text(word.uppercase(), style = OpenType.tag.copy(fontFamily = OpenType.monoXS.fontFamily, fontWeight = androidx.compose.ui.text.font.FontWeight.Normal, letterSpacing = OpenType.monoXS.letterSpacing), color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** The reading a call was made from: a lift's e1RM over its recent sessions, or the recovery meter. */
@Composable
private fun Evidence(d: CoachDecision, state: CoachViewModel.UiState, unit: WeightUnit) {
    if (d.type == "deload") {
        val watch = state.watch ?: return
        val score = watch.fatigueScore ?: return
        Spacer(Modifier.height(6.dp))
        OpenMeter(score.toFloat() / watch.fatigueThreshold.coerceAtLeast(1))
        Caption("Fatigue $score of ${watch.fatigueThreshold} · " + watch.fatigueChecks.filter { it.fired }.take(2).joinToString(", ") { it.name.lowercase() })
        return
    }
    val series = state.e1rmBySlot[d.targetKey]?.takeLast(6) ?: return
    if (series.size < 2) return
    val n = series.size
    // The flat or falling stretch the call is about draws red; the climb before it stays quiet.
    val lastRise = (1 until n).lastOrNull { series[it] > series[it - 1] * 1.005 } ?: 0
    Spacer(Modifier.height(4.dp))
    OpenTrendChart(
        current = series.mapIndexed { i, v -> ChartPoint(i / (n - 1f), toDisplayWeight(v, unit).toFloat()) }.drop(lastRise.coerceAtMost(n - 2)),
        previous = series.mapIndexed { i, v -> ChartPoint(i / (n - 1f), toDisplayWeight(v, unit).toFloat()) }.take(lastRise + 1),
        previousColor = MaterialTheme.colorScheme.onSurfaceVariant,
        xLabels = listOf(1f to "Now"),
        height = 110.dp,
        smooth = false,
        prIndices = emptyList(),
        description = "${d.targetName} estimated one-rep max over its last $n sessions, " +
            "${toDisplayWeight(series.first(), unit).roundToInt()} to ${toDisplayWeight(series.last(), unit).roundToInt()} ${unitLabel(unit)}"
    )
    Caption("Est. 1RM, last $n sessions")
}

/** A call decided this week, with Undo while its window allows. */
@Composable
private fun DecidedRow(d: CoachDecision, now: Long, state: CoachViewModel.UiState, actions: CoachActions) {
    val copy = callCopy(d)
    val watching = d.outcome == "pending" && (d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED)
    val day = d.appliedAt?.let { Instant.ofEpochMilli(it).atZone(ZoneId.systemDefault()).dayOfWeek.getDisplayName(java.time.format.TextStyle.FULL, currentLocale()) }
    val status = when {
        d.status == CoachRepository.STATUS_SKIPPED -> "Skipped"
        d.status == "reverted" -> "Undone"
        watching && day != null -> "Applied $day · on watch"
        else -> coachDecisionStatusWord(d, now).replaceFirstChar { it.uppercase() }
    }
    val undoable = d.id !in state.undoRefused && d.status == CoachRepository.STATUS_APPLIED && d.undoData != null &&
        (d.undoExpiresAt == null || now <= d.undoExpiresAt!!)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(if (d.targetKey == "week") copy.change else "${copy.subject} · ${copy.change}", style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
            Caption(status)
        }
        if (undoable) OpenSmallButton("Undo", filled = false, onClick = { actions.undo(d.id) })
    }
}

// ── Record ─────────────────────────────────────────────────────────────────────────────────────

private fun weekStart(weekId: String): LocalDate? =
    runCatching { LocalDate.parse("$weekId-1", DateTimeFormatter.ISO_WEEK_DATE) }.getOrNull()

private fun applied(d: CoachDecision) = d.status == CoachRepository.STATUS_APPLIED || d.status == CoachRepository.STATUS_FOLDED || d.status == "reverted"
private fun held(d: CoachDecision) = applied(d) && d.outcome == "ok"
private fun missed(d: CoachDecision) = applied(d) && (d.outcome == "failed" || d.status == "reverted")
private fun onWatch(d: CoachDecision) = applied(d) && d.outcome == "pending" && d.status != "reverted"

@Composable
internal fun CoachRecordTab(state: CoachViewModel.UiState, now: Long, actions: CoachActions) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val weeks = state.timeline?.weeks.orEmpty()
        .map { it.copy(decisions = it.decisions.filterNot { d -> d.status == CoachRepository.STATUS_SHADOW }) }
        .mapNotNull { w -> weekStart(w.pass.weekId)?.let { it to w } }
        .sortedBy { it.first }
    val today = Instant.ofEpochMilli(now).atZone(ZoneId.systemDefault()).toLocalDate()
    val thisWeek = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    val from = thisWeek.minusWeeks((RECORD_WEEKS - 1).toLong())
    val inRange = weeks.filter { it.first >= from }
    val before = weeks.filter { it.first < from && it.first >= from.minusWeeks(RECORD_WEEKS.toLong()) }
    fun judged(ws: List<Pair<LocalDate, CoachRepository.CoachHistoryEntry>>) = ws.flatMap { it.second.decisions }.filter { held(it) || missed(it) }
    val judgedNow = judged(inRange)
    val heldNow = judgedNow.count(::held)
    val judgedBefore = judged(before)

    Column {
        Head("Calls that held", "$RECORD_WEEKS weeks")
        Spacer(Modifier.height(10.dp))
        if (judgedNow.isEmpty()) {
            Text(
                buildAnnotatedString { append("0"); withStyle(OpenType.figureLUnit.toSpanStyle().copy(color = muted)) { append(" of 0") } },
                style = OpenType.figureXL, color = muted
            )
            Spacer(Modifier.height(8.dp))
            Caption("Each change you apply gets two weeks to prove itself")
        } else {
            Text(
                buildAnnotatedString {
                    append("$heldNow")
                    withStyle(OpenType.figureLUnit.toSpanStyle().copy(color = muted)) { append(" of ${judgedNow.size}") }
                },
                style = OpenType.figureXL, color = MaterialTheme.colorScheme.onBackground
            )
            Spacer(Modifier.height(8.dp))
            val pct = heldNow * 100 / judgedNow.size
            val prev = judgedBefore.takeIf { it.isNotEmpty() }?.let { j -> j.count(::held) * 100 / j.size }
            Caption(
                listOfNotNull("$pct%", prev?.let { "${if (pct >= it) "↑" else "↓"} from $it% the $RECORD_WEEKS weeks before" }).joinToString(" · "),
                ink = true
            )
        }
        Spacer(Modifier.height(20.dp))
        val heldC = accent
        val watchC = accent.copy(alpha = 0.35f)
        val missedC = muted
        val skippedC = OpenColors.Previous
        OpenSwatchKey(listOf("Held" to heldC, "On watch" to watchC, "Missed" to missedC, "Skipped" to skippedC))
        Spacer(Modifier.height(10.dp))
        val stacks = (0 until RECORD_WEEKS).map { i ->
            val wk = from.plusWeeks(i.toLong())
            val ds = weeks.firstOrNull { it.first == wk }?.second?.decisions.orEmpty()
            listOf(
                ds.count(::held).toFloat() to heldC,
                ds.count(::onWatch).toFloat() to watchC,
                ds.count(::missed).toFloat() to missedC,
                ds.count { it.status == CoachRepository.STATUS_SKIPPED }.toFloat() to skippedC,
            )
        }
        val monthTicks = (0 until RECORD_WEEKS).mapNotNull { i ->
            val wk = from.plusWeeks(i.toLong())
            val f = (i + 0.5f) / RECORD_WEEKS
            when {
                i == RECORD_WEEKS - 1 -> f to "Now"
                i == 0 || wk.month != wk.minusWeeks(1).month -> f to wk.month.getDisplayName(java.time.format.TextStyle.SHORT, currentLocale())
                else -> null
            }
        }.let { t -> t.filterIndexed { idx, p -> idx == t.lastIndex || t.last().first - p.first > 0.12f } }
        OpenStackedColumns(stacks, xLabels = monthTicks, description = "Coach calls per week over $RECORD_WEEKS weeks: $heldNow held of ${judgedNow.size} judged")

        val watching = weeks.flatMap { it.second.decisions }.filter(::onWatch).sortedByDescending { it.appliedAt ?: 0 }
        if (watching.isNotEmpty()) {
            Spacer(Modifier.height(SECTION))
            Head("On watch", "14 days each")
            Spacer(Modifier.height(20.dp))
            Column(verticalArrangement = Arrangement.spacedBy(28.dp)) { watching.forEach { WatchRow(it, now) } }
        }

        val earlier = weeks.flatMap { it.second.decisions }.filter { held(it) || missed(it) || it.outcome == CoachDecision.OUTCOME_NOT_FOLLOWED }
            .sortedByDescending { it.appliedAt ?: 0 }
        Spacer(Modifier.height(SECTION))
        OpenSectionHead("Earlier · by week")
        Spacer(Modifier.height(20.dp))
        if (earlier.isEmpty()) {
            Body("Calls land here once their two weeks are up, stamped with how they went.")
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(24.dp)) { earlier.take(8).forEach { EarlierRow(it) } }
        }
    }
}

@Composable
private fun WatchRow(d: CoachDecision, now: Long) {
    val copy = callCopy(d)
    val since = d.appliedAt ?: now
    val day = (((now - since) / DAY_MS).toInt() + 1).coerceIn(1, 14)
    val accent = MaterialTheme.colorScheme.primary
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text(if (d.targetKey == "week") copy.change else "${copy.subject} · ${copy.change}", style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(12.dp))
            Caption("Day $day of 14")
        }
        if (copy.reason.isNotBlank()) Body(copy.reason)
        Row(
            Modifier.fillMaxWidth().semantics { contentDescription = "Day $day of its 14-day window" },
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            repeat(14) { i ->
                Box(Modifier.weight(1f).height(4.dp).background(if (i < day) accent else OpenColors.Track, RoundedCornerShape(1.dp)))
            }
        }
    }
}

@Composable
private fun EarlierRow(d: CoachDecision) {
    val copy = callCopy(d)
    val date = (d.appliedAt?.let { Instant.ofEpochMilli(it).atZone(ZoneId.systemDefault()).toLocalDate() }) ?: weekStart(d.weekId)
    val ok = held(d)
    val word = when {
        ok -> "Held"
        d.status == "reverted" -> "Undone"
        d.outcome == CoachDecision.OUTCOME_NOT_FOLLOWED -> "Not done"
        else -> "Missed"
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.width(34.dp)) {
            OpenLabel(date?.month?.getDisplayName(java.time.format.TextStyle.SHORT, currentLocale()) ?: "", style = OpenType.monoXS)
            Text("${date?.dayOfMonth ?: ""}", style = OpenType.figureS, color = MaterialTheme.colorScheme.onBackground)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(if (d.targetKey == "week") copy.change else "${copy.subject} · ${copy.change}", style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
            if (copy.reason.isNotBlank()) Text(copy.reason, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Box(
            Modifier
                .then(if (ok) Modifier.background(MaterialTheme.colorScheme.primary, RoundedCornerShape(2.dp)) else Modifier.border(1.dp, OpenColors.Rule, RoundedCornerShape(2.dp)))
                .padding(horizontal = 6.dp, vertical = 3.dp)
        ) {
            Text(
                word.uppercase(),
                style = OpenType.tag.copy(fontFamily = OpenType.monoXS.fontFamily, letterSpacing = OpenType.monoXS.letterSpacing, fontWeight = if (ok) androidx.compose.ui.text.font.FontWeight.Bold else androidx.compose.ui.text.font.FontWeight.Normal),
                color = if (ok) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

// ── Plan ───────────────────────────────────────────────────────────────────────────────────────

@Composable
internal fun CoachPlanTab(state: CoachViewModel.UiState, unit: WeightUnit, now: Long, actions: CoachActions, onOpenGoals: () -> Unit) {
    Column {
        // Three empty sections in a row read as three unrelated jargon words (Antho, 2026-10-21), so a
        // plan with nothing in it is the ladder they make instead: goal, then block, then focus.
        if (state.goals.isEmpty() && state.block == null && state.project == null && state.projectProposal == null) {
            PlanLadder(state, actions, onOpenGoals)
            return@Column
        }
        // The why first, then the weeks built on it, then the one weak point being worked.
        GoalsSection(state, unit, onOpenGoals)
        Spacer(Modifier.height(SECTION))
        BlockSection(state, actions)
        Spacer(Modifier.height(SECTION))
        ProjectSection(state, now, actions)
    }
}

/** A plan with nothing in it: the three steps it is made of, numbered, each with what it does. */
@Composable
private fun PlanLadder(state: CoachViewModel.UiState, actions: CoachActions, onOpenGoals: () -> Unit) {
    Head("How the coach plans")
    Spacer(Modifier.height(10.dp))
    Serif("Start with a goal")
    Spacer(Modifier.height(8.dp))
    Body("Each step builds on the one before. You only have to set the first.")
    Spacer(Modifier.height(28.dp))
    Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
        LadderStep(1, "Goal", "What you're after: a lift to reach, a muscle to grow, or your body weight.") {
            OpenSmallButton("Set a goal", filled = true, onClick = onOpenGoals)
        }
        LadderStep(2, "Block", "A few weeks of training built around that goal, ending in an easy week to recover.") {
            if (!state.blockBusy) OpenSmallButton("Start a block", filled = false, onClick = actions.startBlock)
        }
        LadderStep(3, "Focus", "When one muscle or lift falls behind, the coach proposes a few weeks on it. Nothing to do yet.")
    }
}

@Composable
private fun LadderStep(n: Int, title: String, text: String, action: (@Composable () -> Unit)? = null) {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(Modifier.size(28.dp).border(1.5.dp, OpenColors.Rule, androidx.compose.foundation.shape.CircleShape), contentAlignment = Alignment.Center) {
            Text("$n", style = OpenType.monoS, color = MaterialTheme.colorScheme.onBackground)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(title, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
            Body(text)
            if (action != null) {
                Spacer(Modifier.height(6.dp))
                action()
            }
        }
    }
}

private val DATE_SHORT = DateTimeFormatter.ofPattern("MMM d")

@Composable
private fun BlockSection(state: CoachViewModel.UiState, actions: CoachActions) {
    val block = state.block
    if (block == null) {
        Head("Training block", "Next few weeks")
        Spacer(Modifier.height(10.dp))
        Serif("No block running")
        Spacer(Modifier.height(8.dp))
        Body("Right now the coach adjusts week to week. A block plans a few weeks around your goal, then an easy week.")
        Spacer(Modifier.height(14.dp))
        if (!state.blockBusy) OpenSmallButton("Start a block", filled = true, onClick = actions.startBlock)
        return
    }
    val start = Instant.ofEpochMilli(block.startedAt).atZone(ZoneId.systemDefault()).toLocalDate()
    val end = start.plusWeeks(block.plannedWeeks.toLong()).minusDays(1)
    val phase = BlockPhase.fromCode(block.phase)
    Head("Training block", "${start.format(DATE_SHORT)} to ${end.format(DATE_SHORT)}")
    Spacer(Modifier.height(10.dp))
    // The phase is named once, lit on the bar below; in the title too it wrapped onto a line of its own.
    Serif("Week ${block.weekIndex} of ${block.plannedWeeks}")
    if (block.intent.isNotBlank()) {
        Spacer(Modifier.height(8.dp))
        Body(block.intent)
    }
    Spacer(Modifier.height(20.dp))
    // One bar for where the block is: its phases, the current one lit. (A row of week tiles over it
    // said the same thing twice; the week count is in the title.)
    val accent = MaterialTheme.colorScheme.primary
    val current = phase?.let { BlockPhase.entries.indexOf(it) } ?: -1
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        BlockPhase.entries.forEachIndexed { i, p ->
            Column(Modifier.weight(if (p == BlockPhase.DELOAD) 0.6f else 1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.fillMaxWidth().height(6.dp).background(when {
                    i == current -> accent
                    i < current -> OpenColors.Bar
                    else -> OpenColors.Unreached
                }, RoundedCornerShape(1.dp)))
                Caption(p.displayName, ink = i == current)
            }
        }
    }
    Spacer(Modifier.height(14.dp))
    if (!state.blockBusy) OpenSmallButton("End this block", filled = false, onClick = actions.endBlock)
}

@Composable
private fun GoalsSection(state: CoachViewModel.UiState, unit: WeightUnit, onOpenGoals: () -> Unit) {
    OpenSectionHead("Goals", action = "View all", actionLabel = "Open goals", onAction = onOpenGoals)
    Spacer(Modifier.height(20.dp))
    val goals = state.goals.take(3)
    if (goals.isEmpty()) {
        Body("Set a goal and the coach plans the block around it.")
        return
    }
    // The two-column grid: a third goal starts a second row rather than squeezing three across.
    Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
        goals.chunked(2).forEach { pair ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                pair.forEach { g ->
                    val target = g.target
                    val current = g.current
                    val isWeight = g.kind == CoachGoalKind.LIFT_1RM
                    fun fmt(v: Double) = if (isWeight) toDisplayWeight(v, unit).roundToInt().toString() else v.roundToInt().toString()
                    val name = when (g.kind.scope) {
                        CoachGoalKind.Scope.EXERCISE -> Program.exerciseDisplayName(g.goal.targetKey).substringAfterLast(' ')
                        CoachGoalKind.Scope.MUSCLE -> MuscleGroup.entries.firstOrNull { it.code == g.goal.targetKey }?.displayName ?: g.kind.displayName
                        else -> g.kind.displayName
                    }
                    OpenGoalColumn(
                        label = listOfNotNull(name, target?.let { fmt(it) }).joinToString(" "),
                        figure = current?.let { fmt(it) } ?: "0",
                        denominator = if (!isWeight && target != null) "/${fmt(target)}" else "",
                        fraction = if (current != null && target != null && target > 0) (current / target).toFloat() else 0f,
                        change = g.reading.takeIf { it.isNotBlank() },
                        changeMoved = g.onTrack == true || g.reachedNow,
                        description = "$name: ${g.reading}",
                        onClick = onOpenGoals,
                        modifier = Modifier.weight(1f),
                    )
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun ProjectSection(state: CoachViewModel.UiState, now: Long, actions: CoachActions) {
    val project = state.project
    if (project == null) {
        val proposal = state.projectProposal
        Head("Focus", "One weak point")
        Spacer(Modifier.height(10.dp))
        if (proposal == null) {
            Serif("Nothing to focus on")
            Spacer(Modifier.height(8.dp))
            Body("When one muscle or lift falls behind, the coach proposes a few weeks on it here.")
        } else {
            Serif(proposal.name, OpenType.figureRow)
            Spacer(Modifier.height(8.dp))
            Body(proposal.why)
            Spacer(Modifier.height(14.dp))
            OpenSmallButton("Start this focus", filled = true, onClick = { actions.startProject(proposal) })
        }
        return
    }
    val start = Instant.ofEpochMilli(project.startedAt).atZone(ZoneId.systemDefault()).toLocalDate()
    val week = (((now - project.startedAt) / (7 * DAY_MS)).toInt() + 1).coerceIn(1, project.weeks.coerceAtLeast(1))
    val finish = start.plusWeeks(project.weeks.toLong())
    Head("Focus", "Week $week of ${project.weeks}", rightInk = true)
    Spacer(Modifier.height(10.dp))
    Serif(project.name, OpenType.figureRow)
    Spacer(Modifier.height(8.dp))
    Body(project.why)
    Spacer(Modifier.height(20.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(28.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            OpenLabel("Finish line")
            Text(project.finishLine, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            OpenLabel("Ends")
            Serif(finish.format(DATE_SHORT), OpenType.figureM)
        }
    }
    Spacer(Modifier.height(18.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        repeat(project.weeks.coerceAtLeast(1)) { i ->
            Box(Modifier.weight(1f).height(4.dp).background(if (i < week) MaterialTheme.colorScheme.primary else OpenColors.Track, RoundedCornerShape(1.dp)))
        }
    }
    Spacer(Modifier.height(22.dp))
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        OpenLabel("Next step")
        Text(project.plan, style = OpenType.body, color = MaterialTheme.colorScheme.onBackground)
    }
}
