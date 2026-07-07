package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure

/**
 * The page's hero, in Home's TODAY idiom: one mono eyebrow carrying the screen's identity and
 * the human week, then ONE serif line — the coach's live verdict — with its context aside and
 * the week's figures. The verdict answers "what did my coach decide" before anything else is read.
 */
@Composable
internal fun CoachHero(state: CoachViewModel.UiState, useKg: Boolean, c: CoachColors) {
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

        // Pre-baseline: a countdown, not a verdict.
        if (brief == null || brief.sessionsToGo > 0) {
            LearningHero(brief, watch, c)
            return@Column
        }

        // The weekly pass can have run earlier in the week, before the baseline was complete.
        // Its stored hold reason is stale then; say what's actually true instead.
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
        // carries a decision or result; status/anticipation ("Ready to coach") is never a
        // verdict (§3) — those states run eyebrow + aside and let the figures be the hero.
        val verdict = when {
            brief.pass.status == CoachRepository.STATUS_ERROR -> "The pass failed"
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
        // The context line carries the coach's own read for the week (its "focus" cue) and
        // nothing else. A status/anticipation aside is banned (§3): "You've logged enough…"
        // just restated the Next-brief countdown below, so the stale-learning hero drops the
        // line entirely and lets the figures be the hero.
        val sub = when {
            brief.pass.status == CoachRepository.STATUS_ERROR ->
                brief.pass.holdReason ?: "Nothing was considered this week."
            staleLearning -> null
            // A quiet week needs no restatement — drop the line rather than echo the verdict (§4.4).
            else -> brief.review?.focusLine ?: brief.pass.holdReason
        }
        if (sub != null) SubLine(sub, c)

        // The week in figures. Equal columns so long values can never collide.
        brief.review?.let { r ->
            Spacer(Modifier.height(20.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                EditorialFigure(
                    value = "${r.sessionsLastWeek} of ${r.sessionsTarget}",
                    label = "Sessions",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    modifier = Modifier.weight(1f)
                )
                EditorialFigure(
                    value = formatVolumeCompact(r.volumeLastWeekLb, useKg),
                    label = "Volume",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    delta = r.volumeDeltaPct,
                    modifier = Modifier.weight(1f)
                )
                EditorialFigure(
                    value = "${r.prsLastWeek}",
                    label = "PRs",
                    onBg = c.onBg, muted = c.muted, accent = c.accent,
                    modifier = Modifier.weight(1f)
                )
                // A 4th figure when there's one to show: the recovery score once it reads, else the
                // week's cardio load (real data the coach also weighs) so the hero isn't bare at three.
                val fatigue = r.fatigueScore
                when {
                    fatigue != null -> EditorialFigure(
                        value = "$fatigue",
                        label = "Fatigue",
                        onBg = c.onBg, muted = c.muted, accent = c.accent,
                        modifier = Modifier.weight(1f)
                    )
                    r.cardioMinutesLastWeek > 0 -> EditorialFigure(
                        value = "${r.cardioMinutesLastWeek}",
                        label = "Cardio min",
                        onBg = c.onBg, muted = c.muted, accent = c.accent,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }

    }
}

/** Home's TODAY lockup: a mono eyebrow sitting tight over the serif line. */
@Composable
private fun Eyebrow(text: String, c: CoachColors) {
    Text(text, style = MaterialTheme.typography.labelSmall, fontSize = 13.sp, color = c.muted)
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

/** The pre-baseline hero: a countdown with the concrete signals that sharpen the picture. */
@Composable
private fun LearningHero(
    brief: com.forge.app.data.repo.CoachBrief?,
    watch: com.forge.app.data.repo.CoachWatch?,
    c: CoachColors
) {
    val logged = watch?.sessionsLogged ?: brief?.sessionsLogged ?: 0
    val needed = (watch?.minSessions ?: brief?.minSessions ?: 0).coerceAtLeast(1)
    val toGo = (needed - logged).coerceAtLeast(0)

    // Learning is a status, not a verdict (§3): no serif headline, and no anticipation aside
    // (§3/§14) — the progress bar and the signal dots below ARE the hero, the first dot naming
    // the countdown the aside used to spell out.
    Spacer(Modifier.height(10.dp))
    TrustProgressBar(
        streak = logged.coerceAtMost(needed),
        required = needed,
        earned = toGo == 0,
        modifier = Modifier.fillMaxWidth()
    )

    if (watch != null) {
        Spacer(Modifier.height(14.dp))
        val effortActive = watch.recoverySignals.firstOrNull { it.label.startsWith("Effort") }?.active == true
        val healthActive = watch.recoverySignals.any {
            (it.label.startsWith("Sleep") || it.label.contains("heart")) && it.active
        }
        CoachDotRow(
            if (toGo > 0) "$toGo more session${if (toGo == 1) "" else "s"} for a baseline" else "Baseline reached",
            active = toGo == 0, c = c
        )
        CoachDotRow(
            if (effortActive) "Session effort ratings are coming in" else "Rate how sessions feel at the finish",
            active = effortActive, c = c
        )
        CoachDotRow(
            if (healthActive) "Health Connect is feeding sleep and heart rate" else "Connect Health Connect for sleep and heart rate",
            active = healthActive, c = c
        )
    }
}
