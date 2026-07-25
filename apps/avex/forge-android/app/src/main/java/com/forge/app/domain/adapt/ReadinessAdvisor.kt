package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset

/**
 * System 6 of the adaptation engine: daily readiness autoregulation. A small, bounded
 * (±[AdaptThresholds.readinessMaxPercent]%) scale on today's weight suggestions, from the
 * last ~72 hours of signals — never a lurch, just a nudge. Pure + deterministic.
 *
 * Signals (each a named part of the reason):
 *  - days since the last session (5+ → comeback caution −3; 2–4 → fresh +1)
 *  - unusually heavy session yesterday (volume > 1.25× recent median) −2
 *  - cardio rest flagged sore (−2) / sick (−4) in the last 48h
 *
 * Consumed by ProgressionAdvisor's chip path when the user hasn't made an explicit
 * intensity pick — an explicit LIGHT/HARD choice always outranks this (user intent wins).
 * Gate: ≥ [AdaptThresholds.readinessMinSessions] sessions; a net-zero score returns null
 * (silent).
 *
 * Takes plain lists (not the full snapshot) so the day screen can feed it with two
 * cheap queries instead of the whole-history fan-out.
 */
object ReadinessAdvisor {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val HOUR_MS = 60L * 60 * 1000

    fun evaluate(
        sessions: List<Session>,
        cardio: List<CardioEntry>,
        nowMs: Long,
        zoneId: ZoneId = ZoneOffset.UTC,
        /** Holiday/vacation predicate (#135): such days don't count as "time off". */
        onVacation: (LocalDate) -> Boolean = { false },
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.ReadinessScale? {
        val finished = sessions.filter { it.finishedAt != null && !it.isUntracked }.sortedBy { it.startedAt }
        if (finished.size < t.readinessMinSessions) return null

        var percent = 0
        val parts = mutableListOf<String>()

        // Recovery spacing — vacation days don't count toward "time off", so coming back
        // from a planned holiday doesn't trigger the comeback caution (#135).
        val lastSession = finished.maxBy { it.startedAt }
        val lastDate = Instant.ofEpochMilli(lastSession.startedAt).atZone(zoneId).toLocalDate()
        val today = Instant.ofEpochMilli(nowMs).atZone(zoneId).toLocalDate()
        var daysSince = 0
        var gapDay = lastDate.plusDays(1)
        while (!gapDay.isAfter(today)) {
            if (!onVacation(gapDay)) daysSince++
            gapDay = gapDay.plusDays(1)
        }
        when {
            // DESIGN §11: join with a comma, never an em dash.
            daysSince >= 5 -> { percent -= 3; parts += "first session back after $daysSince days, ease in" }
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

        // Acute cardio load (beyond the sore/sick flags): a big block of active cardio in the last day
        // competes with lifting recovery, so shave a point. restReason == null ⇒ an active (non-rest) row.
        val cardioLoadMin = cardio
            .filter { it.date >= nowMs - DAY_MS && it.restReason == null }
            .sumOf { it.durationMin }
        if (cardioLoadMin >= t.readinessCardioLoadMinutes) {
            percent -= t.readinessCardioLoadPenalty
            parts += "heavy cardio in the last day"
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
