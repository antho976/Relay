package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.coach.LifeEvents
import com.forge.app.domain.mood.Mood
import com.forge.app.program.MuscleGroup
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset

/**
 * System 6, rebuilt (Coach v3 B1): daily readiness from everything the app knows about today.
 *
 * V1 read two things — session spacing and cardio rest flags. V2 reads the morning check-in, last
 * night's sleep, resting HR against your own baseline, post-session mood, acute load, off-gym
 * movement, bodyweight flux and life events, and it names every part it used. The output is still
 * a small bounded scale on today's targets: readiness *shapes* the session, it never cancels it.
 *
 * **One signal, one computation** (plan M6). Illness now arrives through [LifeEvents] rather than a
 * second cardio-flag deduction, and generic soreness is replaced by per-muscle gates from the
 * check-in. Nothing double-counts.
 *
 * Gates hold: below [AdaptThresholds.readinessMinSessions] sessions it stays silent, and a net-zero
 * score returns null rather than emitting a confident zero. Every input is optional — a user who
 * logs nothing but sessions gets exactly v1's behavior.
 */
object ReadinessAdvisor {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val HOUR_MS = 60L * 60 * 1000

    /**
     * A readiness read with its parts named, plus the muscles today should tread lightly on.
     * [scale] is null when the advisor is silent (below gates, or a net-zero score).
     */
    data class Readiness(
        val scale: Recommendation.ReadinessScale?,
        val soreMuscles: Set<MuscleGroup>,
        val lifeEvents: LifeEvents.State
    )

    /**
     * The full read. Every parameter past [nowMs] is optional so existing callers keep working and
     * a device with no health data, no check-ins and no moods behaves exactly as it did.
     */
    fun assess(
        sessions: List<Session>,
        cardio: List<CardioEntry>,
        nowMs: Long,
        zoneId: ZoneId = ZoneOffset.UTC,
        onVacation: (LocalDate) -> Boolean = { false },
        moods: List<MoodEntry> = emptyList(),
        checkins: List<CheckinEntry> = emptyList(),
        health: HealthSnap = HealthSnap(),
        bodyweightFlux: Boolean = false,
        lifeEvents: LifeEvents.State = LifeEvents.State.NONE,
        t: AdaptThresholds = AdaptThresholds()
    ): Readiness {
        val finished = sessions.filter { it.finishedAt != null && !it.isUntracked }.sortedBy { it.startedAt }
        if (finished.size < t.readinessMinSessions) {
            return Readiness(null, lifeEvents.soreMuscles, lifeEvents)
        }

        var percent = 0
        val parts = mutableListOf<String>()

        // ── Life first: illness and time away outrank every other reading ─────────
        if (lifeEvents.sick) {
            percent -= t.readinessSickPenalty
            parts += "you flagged being unwell"
        }
        lifeEvents.layoff?.let { layoff ->
            if (layoff.returning) {
                percent -= t.readinessComebackPenalty
                parts += "first week back after ${layoff.days} days"
            }
        }

        // ── Recovery spacing (v1 rule, kept) ──────────────────────────────────────
        // Vacation days don't count as "time off", so a planned holiday doesn't read as a
        // comeback — the layoff ramp above handles real breaks (#135).
        val lastSession = finished.maxBy { it.startedAt }
        val lastDate = Instant.ofEpochMilli(lastSession.startedAt).atZone(zoneId).toLocalDate()
        val today = Instant.ofEpochMilli(nowMs).atZone(zoneId).toLocalDate()
        var daysSince = 0
        var gapDay = lastDate.plusDays(1)
        while (!gapDay.isAfter(today)) {
            if (!onVacation(gapDay)) daysSince++
            gapDay = gapDay.plusDays(1)
        }
        // Suppressed while a layoff already spoke — one home for "you've been away" (§4.3).
        if (lifeEvents.layoff == null) {
            when {
                // DESIGN §11: join with a comma, never an em dash.
                daysSince >= 5 -> { percent -= 3; parts += "first session back after $daysSince days, ease in" }
                daysSince in 2..4 -> { percent += 1; parts += "fresh after $daysSince rest days" }
            }
        }

        // ── Acute load: an unusually heavy session yesterday ──────────────────────
        // The CALENDAR day before today, in the caller's zone — the same reading `daysSince` above
        // is built from. This was a fixed 12-36 h window, which is not yesterday from any hour of
        // the day: an 07:00 session checked at 20:00 is 37 h back and was missed entirely, a 22:00
        // session checked at 06:00 is 8 h back and was missed too, and an 11:00 session checked at
        // 23:00 the SAME day is 12 h back — so today's session was deducted for and labelled
        // "heavy session yesterday". Evening lifters got the deduction least often and morning
        // lifters got it against the wrong day.
        val yesterdayDate = today.minusDays(1)
        val yesterday = finished.lastOrNull {
            Instant.ofEpochMilli(it.startedAt).atZone(zoneId).toLocalDate() == yesterdayDate
        }
        if (yesterday != null) {
            val volumes = finished.takeLast(8).mapNotNull { it.totalVolumeLb }.filter { it > 0 }
            val median = median(volumes)
            val vol = yesterday.totalVolumeLb ?: 0.0
            if (median != null && vol > median * 1.25) {
                percent -= 2
                parts += "heavy session yesterday"
            }
        }

        // ── This morning's check-in ───────────────────────────────────────────────
        val checkin = checkins
            .filter { it.hasAnswers && nowMs - it.recordedAt <= t.readinessCheckinHours * HOUR_MS }
            .maxByOrNull { it.recordedAt }
        if (checkin != null) {
            checkin.sleepQuality?.let { q ->
                when {
                    q <= 2 -> { percent -= 2; parts += "slept badly" }
                    q >= 5 -> { percent += 1; parts += "slept well" }
                }
            }
            checkin.soreness?.let { s ->
                if (s >= 4) { percent -= 2; parts += "sore" }
            }
            checkin.stress?.let { s ->
                if (s >= 4) { percent -= 1; parts += "stressed" }
            }
            checkin.motivation?.let { m ->
                when {
                    m <= 2 -> { percent -= 1; parts += "low drive" }
                    m >= 5 -> { percent += 1; parts += "ready to go" }
                }
            }
        }

        // ── Last night's sleep, measured (Health Connect) ─────────────────────────
        // Only when the check-in didn't already answer for sleep — one home per fact (§4.3).
        if (checkin?.sleepQuality == null) {
            lastNightSleepMin(health, nowMs)?.let { minutes ->
                when {
                    minutes <= t.readinessShortSleepMinutes -> {
                        percent -= 2
                        parts += "short night (${minutes / 60}h)"
                    }
                    minutes >= t.readinessGoodSleepMinutes -> {
                        percent += 1
                        parts += "long night (${minutes / 60}h)"
                    }
                }
            }
        }

        // ── Resting HR against your own baseline ──────────────────────────────────
        restingHrDelta(health, nowMs, t)?.let { delta ->
            if (delta >= t.readinessRestingHrDeltaBpm) {
                percent -= 2
                parts += "resting HR up $delta bpm"
            }
        }

        // ── Overnight HRV against your own baseline (F) ───────────────────────────
        // Noisy per night and meaningful as a trend, so this asks the same question as resting HR:
        // is today's reading meaningfully below YOUR recent normal? Never an absolute number, and
        // never louder than what you said in the check-in.
        hrvDrop(health, nowMs, t)?.let { dropPct ->
            if (dropPct >= t.readinessHrvDropPercent) {
                percent -= 1
                parts += "HRV down $dropPct%"
            }
        }

        // ── How the last session actually felt ────────────────────────────────────
        val lastMood = moods
            .filter { it.recordedAt >= nowMs - t.readinessMoodHours * HOUR_MS }
            .maxByOrNull { it.recordedAt }
            ?.let { Mood.fromCode(it.mood) }
        when (lastMood) {
            Mood.DRAINED, Mood.OFF -> {
                percent -= t.readinessLowMoodPenalty
                parts += "last session felt ${lastMood.displayName.lowercase()}"
            }
            Mood.STRONG -> {
                percent += t.readinessStrongMoodBonus
                parts += "last session felt strong"
            }
            else -> Unit
        }

        // ── Conditioning interference ─────────────────────────────────────────────
        // The single interference read (M6): the old sick/sore cardio deductions are gone (life
        // events own illness), leaving active cardio load as the one thing cardio says here.
        // ONE formula for what cardio costs lifting (Engine E-A): readiness consumes the Engine's
        // pure ConditioningLoad rather than re-deriving effort × zone × minutes here. Two formulas
        // would eventually disagree, and that would surface as a coach contradicting its own hub.
        val interference = com.forge.app.domain.engine.ConditioningLoad.interferencePenalty(cardio, nowMs)
        if (interference > 0) {
            percent -= interference
            parts += "cardio load yesterday"
        }

        // ── Off-gym movement (Health Connect steps) ───────────────────────────────
        yesterdaySteps(health, nowMs, zoneId)?.let { steps ->
            if (steps >= t.readinessHighStepDay) {
                percent -= 1
                parts += "long day on your feet"
            }
        }

        // ── Bodyweight flux ───────────────────────────────────────────────────────
        if (bodyweightFlux) {
            percent -= 1
            parts += "weight swinging"
        }

        val clamped = percent.coerceIn(-t.readinessMaxPercent, t.readinessMaxPercent)
        val scale = if (clamped == 0) null else Recommendation.ReadinessScale(
            percent = clamped,
            reason = parts.joinToString(" · "),
            confidence = Confidence.MEDIUM,
            lessonId = LESSON_READINESS
        )
        // Soreness gates come from the check-in's muscle picker; a generic "sore" tap with no
        // muscles named shapes the score above but gates nothing — a gate needs a target.
        return Readiness(scale, lifeEvents.soreMuscles, lifeEvents)
    }

    /** The Academy lesson behind a readiness score — what it's built from and what it isn't. */
    const val LESSON_READINESS = "coach.readiness_built_from"

    /**
     * V1's entry point, kept so the day screen and the wear publisher keep working unchanged.
     * Delegates to [assess] and returns just the scale.
     */
    fun evaluate(
        sessions: List<Session>,
        cardio: List<CardioEntry>,
        nowMs: Long,
        zoneId: ZoneId = ZoneOffset.UTC,
        onVacation: (LocalDate) -> Boolean = { false },
        moods: List<MoodEntry> = emptyList(),
        checkins: List<CheckinEntry> = emptyList(),
        health: HealthSnap = HealthSnap(),
        lifeEvents: LifeEvents.State = LifeEvents.State.NONE,
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.ReadinessScale? = assess(
        sessions = sessions,
        cardio = cardio,
        nowMs = nowMs,
        zoneId = zoneId,
        onVacation = onVacation,
        moods = moods,
        checkins = checkins,
        health = health,
        lifeEvents = lifeEvents,
        t = t
    ).scale

    /** Minutes slept in the night that ended most recently, when it's recent enough to matter. */
    private fun lastNightSleepMin(health: HealthSnap, nowMs: Long): Int? =
        health.sleepNights
            .filter { it.endedAtMs <= nowMs && nowMs - it.endedAtMs <= 20 * HOUR_MS }
            .maxByOrNull { it.endedAtMs }
            ?.durationMin

    /** How far today's HRV sits below the prior fortnight's average, as a percent. */
    private fun hrvDrop(health: HealthSnap, nowMs: Long, t: AdaptThresholds): Int? {
        val recent = health.hrv.filter { it.timeMs >= nowMs - 2 * DAY_MS }
        val baseline = health.hrv.filter { it.timeMs in (nowMs - 16 * DAY_MS) until (nowMs - 2 * DAY_MS) }
        if (recent.isEmpty() || baseline.size < t.readinessMinRestingHrSamples) return null
        val today = recent.map { it.rmssdMs }.average()
        val normal = baseline.map { it.rmssdMs }.average()
        if (normal <= 0) return null
        return (((normal - today) / normal) * 100).toInt()
    }

    /** Today's resting HR minus the prior-fortnight average, or null below the sample gate. */
    private fun restingHrDelta(health: HealthSnap, nowMs: Long, t: AdaptThresholds): Int? {
        val recent = health.restingHr.filter { it.timeMs >= nowMs - 2 * DAY_MS }
        val baselineWindow = health.restingHr.filter {
            it.timeMs in (nowMs - 16 * DAY_MS) until (nowMs - 2 * DAY_MS)
        }
        if (recent.isEmpty() || baselineWindow.size < t.readinessMinRestingHrSamples) return null
        val today = recent.map { it.bpm }.average()
        val baseline = baselineWindow.map { it.bpm }.average()
        if (baseline <= 0) return null
        return (today - baseline).toInt()
    }

    /**
     * Yesterday's step count, when steps are being synced.
     *
     * The window was `nowMs - dayStartMs in 0..2 days` followed by `maxByOrNull { dayStartMs }`,
     * which admitted TODAY's bucket and then picked it, because today's is always the latest. So
     * "long day on your feet" was reading the day in progress: at 07:00 it saw a few hundred steps
     * and never fired, and when it did fire it was deducting for a day the athlete had not finished
     * living, to advise them about training in it. Yesterday is a calendar day, so ask for the
     * calendar day.
     */
    private fun yesterdaySteps(health: HealthSnap, nowMs: Long, zoneId: ZoneId): Int? {
        val yesterday = Instant.ofEpochMilli(nowMs).atZone(zoneId).toLocalDate().minusDays(1)
        return health.dailySteps
            .filter { Instant.ofEpochMilli(it.dayStartMs).atZone(zoneId).toLocalDate() == yesterday }
            .maxByOrNull { it.dayStartMs }
            ?.steps
    }

    private fun median(values: List<Double>): Double? {
        if (values.isEmpty()) return null
        val sorted = values.sorted()
        val mid = sorted.size / 2
        return if (sorted.size % 2 == 1) sorted[mid] else (sorted[mid - 1] + sorted[mid]) / 2
    }
}
