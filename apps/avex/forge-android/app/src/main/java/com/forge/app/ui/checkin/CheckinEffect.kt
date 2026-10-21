package com.forge.app.ui.checkin

import com.forge.app.domain.adapt.AdaptThresholds

/*
 * What the check-in does to today, shown live as the sliders move (2026-10-21 redesign, "pg-notif
 * ck"). The same arithmetic ReadinessAdvisor applies to this morning's answers and to the unwell
 * flag, bounded the same way, so the line under the sliders says what saving will actually do to
 * today's targets. The advisor also reads your watch, your last session and yesterday's load; this
 * line speaks only for the answers on the sheet.
 */

/** The share of today's readiness these answers make, in percent of today's targets. */
internal fun checkinEffectPercent(
    sleep: Int?,
    soreness: Int?,
    stress: Int?,
    drive: Int?,
    sick: Boolean,
    t: AdaptThresholds = AdaptThresholds(),
): Int {
    var p = 0
    if (sick) p -= t.readinessSickPenalty
    sleep?.let { q -> if (q <= 2) p -= 2 else if (q >= 5) p += 1 }
    soreness?.let { s -> if (s >= 4) p -= 2 }
    stress?.let { s -> if (s >= 4) p -= 1 }
    drive?.let { m -> if (m <= 2) p -= 1 else if (m >= 5) p += 1 }
    return p.coerceIn(-t.readinessMaxPercent, t.readinessMaxPercent)
}

/** The live line: "Today: targets as planned", "Today: targets ease 3%", "Today: targets up 2%". */
internal fun checkinEffectLine(percent: Int, sick: Boolean): String = when {
    sick -> "Today: nothing pushed, targets ease ${-percent}%"
    percent < 0 -> "Today: targets ease ${-percent}%"
    percent > 0 -> "Today: targets up $percent%"
    else -> "Today: targets as planned"
}

/** Each question's five answers, named, so the slider says what it holds. */
internal enum class CheckinQuestion(val label: String, val low: String, val high: String, val answers: List<String>) {
    SLEEP("Sleep", "Rough", "Great", listOf("Rough", "Poor", "OK", "Good", "Great")),
    SORENESS("Soreness", "None", "Very", listOf("None", "A bit", "Some", "Sore", "Very")),
    STRESS("Stress", "Calm", "Very", listOf("Calm", "Low", "Some", "High", "Very")),
    DRIVE("Drive", "Flat", "Eager", listOf("Flat", "Low", "OK", "Keen", "Eager")),
}
