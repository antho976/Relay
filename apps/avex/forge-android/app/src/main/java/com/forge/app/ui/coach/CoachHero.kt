package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure

/**
 * The page's hero, in Home's TODAY idiom: one mono eyebrow carrying the screen's identity and the
 * human week, then ONE serif line — the coach's live verdict — with its context aside, and the
 * week's figures underneath.
 *
 * The figure row renders from session one (§3, §12). While the coach is still learning there is no
 * verdict to state, so the eyebrow and the figures ARE the hero and the fourth figure counts the
 * baseline down; the serif line and the aside stay out (a status is never a verdict, and an
 * anticipation aside just restates the count — both settled, `design/SETTLED.md`). What the baseline
 * MEANS is said once, on the section it gates ("The call"), never here as well (§4.3, one home).
 */
@Composable
internal fun CoachHero(state: CoachViewModel.UiState, weightUnit: WeightUnit, c: CoachColors) {
    val brief = state.brief
    val watch = state.watch

    Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER)) {
        // Identity lives in the eyebrow (mono, like Home's TODAY) so the serif line stays free
        // for the verdict — never a screen name stacked over a conclusion (§3).
        val week = brief?.pass?.weekId?.let(::coachWeekLabel)
        Eyebrow(if (week != null) "COACH · ${week.uppercase()}" else "COACH", c)

        // Both reads failed: a load error, not a fresh account.
        if (brief == null && watch == null) {
            Verdict("Notes unavailable", c.onBg)
            SubLine("Couldn't read the coach's notes right now. Come back in a bit.", c)
            return@Column
        }

        // Pre-baseline the coach has nothing to conclude, so the figures carry the hero alone.
        val learning = brief == null || brief.sessionsToGo > 0
        if (!learning) {
            // The weekly pass can have run earlier in the week, before the baseline was complete.
            // Its stored hold reason is stale then; say nothing rather than something untrue.
            val staleLearning = brief.pass.status == CoachRepository.STATUS_HOLD &&
                AutoCoachPlanner.isLearningHold(brief.pass.holdReason)

            val decisions = brief.decisions
            val open = decisions.count { it.status == CoachRepository.STATUS_PROPOSED }
            val applied = decisions.count {
                it.status == CoachRepository.STATUS_APPLIED || it.status == CoachRepository.STATUS_FOLDED
            }
            val hasOpenDeload = decisions.any {
                it.type == "deload" && it.status == CoachRepository.STATUS_PROPOSED
            }

            // Serif verdict — a headline, so no terminal period (§11). It renders ONLY when it
            // carries a decision or result (§3).
            val verdict = when {
                // §11: a verdict states what it means for the user, not internal state ("pass").
                brief.pass.status == CoachRepository.STATUS_ERROR -> "No call this week"
                staleLearning -> null
                hasOpenDeload -> "Deload week"
                open == 1 -> "One proposal"
                open > 1 -> "$open proposals"
                applied > 0 -> "$applied change${if (applied == 1) "" else "s"} applied"
                else -> "No changes"
            }
            if (verdict != null) {
                Verdict(verdict, if (brief.pass.status == CoachRepository.STATUS_ERROR) c.error else c.onBg)
            } else {
                Spacer(Modifier.height(6.dp))
            }
            // The context line carries the coach's own read for the week and nothing else.
            val sub = when {
                // Stored reasons are machine prose — translated at the seam (§11), never raw.
                brief.pass.status == CoachRepository.STATUS_ERROR ->
                    brief.pass.holdReason?.let(::recordHoldLine) ?: "Nothing was considered this week."
                staleLearning -> null
                // A quiet week needs no restatement — drop the line rather than echo the verdict (§4.3).
                else -> brief.review?.focusLine ?: brief.pass.holdReason?.let(::recordHoldLine)
            }
            if (sub != null) SubLine(sub, c)
        }

        Spacer(Modifier.height(if (learning) 14.dp else 20.dp))
        CoachFigures(state, weightUnit, learning, c)
    }
}

/**
 * The week in figures — equal columns so long values can never collide, honest zeros throughout
 * (§12). The first three come off the weekly review; the fourth is whichever reading is live: the
 * baseline countdown while the coach is learning you, then the recovery score once it reads, then
 * the week's cardio load. A failed snapshot leaves only the baseline, which is still a real reading.
 */
@Composable
private fun CoachFigures(
    state: CoachViewModel.UiState,
    weightUnit: WeightUnit,
    learning: Boolean,
    c: CoachColors
) {
    val review = state.brief?.review
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        if (review != null) {
            // While the baseline is filling, BASELINE below is already a count of sessions — showing
            // last week's session count beside it puts the same number on the row twice
            // (`design/FAILURES.md`, *Mark echo*). It returns the moment the coach is live.
            if (!learning) {
                Figure("${review.sessionsLastWeek} of ${review.sessionsTarget}", "Sessions", c)
            }
            Figure(
                formatVolumeCompact(review.volumeLastWeekLb, weightUnit),
                "Volume",
                c,
                delta = review.volumeDeltaPct
            )
            Figure("${review.prsLastWeek}", "PRs", c)
        }
        val fatigue = review?.fatigueScore
        when {
            learning -> {
                val logged = state.watch?.sessionsLogged ?: state.brief?.sessionsLogged ?: 0
                val needed = (state.watch?.minSessions ?: state.brief?.minSessions ?: 0)
                    .coerceAtLeast(1)
                Figure("$logged of $needed", "Baseline", c)
            }
            fatigue != null -> Figure("$fatigue", "Fatigue", c)
            review != null && review.cardioMinutesLastWeek > 0 ->
                Figure("${review.cardioMinutesLastWeek}", "Cardio min", c)
        }
    }
}

@Composable
private fun RowScope.Figure(value: String, label: String, c: CoachColors, delta: Int? = null) {
    EditorialFigure(
        value = value,
        label = label,
        onBg = c.onBg, muted = c.muted, accent = c.accent,
        delta = delta,
        modifier = Modifier.weight(1f)
    )
}

/** Home's TODAY lockup: a mono eyebrow sitting tight over the serif line. */
@Composable
private fun Eyebrow(text: String, c: CoachColors) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = c.muted)
    Spacer(Modifier.height(2.dp))
}

@Composable
private fun Verdict(text: String, color: androidx.compose.ui.graphics.Color) {
    Text(text, style = MaterialTheme.typography.headlineLarge, color = color)
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun SubLine(text: String, c: CoachColors) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = c.muted,
        fontStyle = FontStyle.Italic
    )
}
