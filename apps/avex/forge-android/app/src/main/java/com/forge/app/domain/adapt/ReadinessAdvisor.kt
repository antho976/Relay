package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.mood.Mood

/**
 * System 6 of the adaptation engine: daily readiness autoregulation. A small, bounded
 * (±[AdaptThresholds.readinessMaxPercent]%) scale on today's weight suggestions, from the
 * last ~72 hours of signals — never a lurch, just a nudge. Pure + deterministic.
 *
 * Signals (each a named part of the reason):
 *  - most recent mood, if within 48h (STRONG +2 … DRAINED −3)
 *  - days since the last session (5+ → comeback caution −3; 2–4 → fresh +1)
 *  - unusually heavy session yesterday (volume > 1.25× recent median) −2
 *  - cardio rest flagged sore (−2) / sick (−4) in the last 48h
 *
 * Consumed by ProgressionAdvisor's chip path when the user hasn't made an explicit
 * intensity pick — an explicit LIGHT/HARD choice always outranks this (user intent wins).
 * Gates: ≥ [AdaptThresholds.readinessMinSessions] sessions and ≥
 * [AdaptThresholds.readinessMinMoods] recent moods; a net-zero score returns null (silent).
 *
 * Takes plain lists (not the full snapshot) so the day screen can feed it with three
 * cheap queries instead of the whole-history fan-out.
 */
object ReadinessAdvisor {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val HOUR_MS = 60L * 60 * 1000

    fun evaluate(
        sessions: List<Session>,
        moods: List<MoodEntry>,
        cardio: List<CardioEntry>,
        nowMs: Long,
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.ReadinessScale? {
        val finished = sessions.filter { it.finishedAt != null && !it.isUntracked }.sortedBy { it.startedAt }
        if (finished.size < t.readinessMinSessions) return null
        val recentMoods = moods.filter { it.recordedAt >= nowMs - t.readinessMoodWindowDays * DAY_MS }
        if (recentMoods.size < t.readinessMinMoods) return null

        var percent = 0
        val parts = mutableListOf<String>()

        // Most recent mood, only while it's fresh enough to describe "today".
        val latest = recentMoods.maxByOrNull { it.recordedAt }
            ?.takeIf { it.recordedAt >= nowMs - 48 * HOUR_MS }
        when (Mood.fromCode(latest?.mood)) {
            Mood.STRONG -> { percent += 2; parts += "felt strong last session" }
            Mood.GOOD -> { percent += 1; parts += "feeling good" }
            Mood.OFF -> { percent -= 2; parts += "felt off last session" }
            Mood.DRAINED -> { percent -= 3; parts += "drained last session" }
            else -> {}
        }

        // Recovery spacing.
        val lastSession = finished.maxBy { it.startedAt }
        val daysSince = ((nowMs - lastSession.startedAt) / DAY_MS).toInt()
        when {
            daysSince >= 5 -> { percent -= 3; parts += "first session back after $daysSince days — ease in" }
            daysSince in 2..4 -> { percent += 1; parts += "fresh after $daysSince rest days" }
        }

        // Unusually heavy session yesterday (acute load).
        val yesterday = finished.lastOrNull { it.startedAt in (nowMs - 36 * HOUR_MS)..(nowMs - 12 * HOUR_MS) }
        if (yesterday != null) {
            val volumes = finished.takeLast(8).mapNotNull { it.totalVolumeLb }.filter { it > 0 }
            val median = median(volumes)
            val vol = yesterday.totalVolumeLb ?: 0.0
            if (median != null && vol > median * 1.25) {
                percent -= 2
                parts += "heavy session yesterday"
            }
        }

        // The body already flagged recovery trouble.
        val recentCardio = cardio.filter { it.date >= nowMs - 48 * HOUR_MS }
        when {
            recentCardio.any { it.restReason == "sick" } -> { percent -= 4; parts += "sick recently" }
            recentCardio.any { it.restReason == "sore" } -> { percent -= 2; parts += "sore recently" }
        }

        val clamped = percent.coerceIn(-t.readinessMaxPercent, t.readinessMaxPercent)
        if (clamped == 0) return null
        return Recommendation.ReadinessScale(
            percent = clamped,
            reason = parts.joinToString(" · "),
            confidence = Confidence.MEDIUM
        )
    }

    private fun median(values: List<Double>): Double? {
        if (values.isEmpty()) return null
        val sorted = values.sorted()
        val mid = sorted.size / 2
        return if (sorted.size % 2 == 1) sorted[mid] else (sorted[mid - 1] + sorted[mid]) / 2
    }
}
