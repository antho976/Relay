package com.forge.app.ui.coach

import com.forge.app.data.repo.CoachRecord


/**
 * One reading the Today panel draws: what is being read, the answer, and the evidence for that
 * answer under it. Pure data so the wording is testable without Compose.
 */
internal data class StandingLine(val label: String, val value: String, val sub: String?)

/** Today's readiness, as the change it makes to the day's weight targets and why. */
internal fun readinessLine(state: CoachViewModel.UiState): StandingLine? {
    val logged = state.watch?.sessionsLogged ?: state.brief?.sessionsLogged ?: return null
    val gate = state.readinessGateSessions.coerceAtLeast(1)
    val r = state.readiness
    // The day's targets carry readiness AND the comeback ramp; the line states both, composed the
    // way the day screen and the brief compose them.
    val composed = (1 + (r?.percent ?: 0) / 100.0) * state.rampScale
    val percent = kotlin.math.round((composed - 1.0) * 100).toInt()
    val reason = r?.reason?.takeIf { it.isNotBlank() } ?: state.rampNote
    return when {
        percent != 0 -> StandingLine(
            label = "Readiness today",
            value = if (percent < 0) "Targets ${-percent}% lighter" else "Targets $percent% heavier",
            sub = reason?.replaceFirstChar { it.uppercase() }
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
 * checks that crossed their line as the reason. The meter draws under it on the Today panel.
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

private fun ordinalSuffix(n: Int): String = when {
    n % 100 in 11..13 -> "th"
    n % 10 == 1 -> "st"
    n % 10 == 2 -> "nd"
    n % 10 == 3 -> "rd"
    else -> "th"
}
