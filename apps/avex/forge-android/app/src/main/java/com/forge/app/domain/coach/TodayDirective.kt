package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.domain.adapt.countsForProgression
import com.forge.app.program.MuscleGroup

/**
 * ONE answer to "what do I do today?" (Coach v3 B2) — the flagship Decision-Zero surface.
 *
 * Not a menu and not a dashboard: a directive, with the veto one tap away. Train (which day, why),
 * rest (why, and what to do instead), or cardio. Everything else on the Overview is context for
 * this one line.
 *
 * Pure, like every advisor, and honest about what it doesn't know. Every degraded mode is declared
 * rather than discovered:
 *  - **no block yet** (Phase C) → computed from spacing, readiness and the schedule alone;
 *  - **sequence-mode schedule** → next-up-relative only; it never claims a weekday it can't see;
 *  - **freestyle** → no program to point at, so it answers from spacing and readiness and prepares
 *    nothing — the one cohort that most needs an answer still gets one;
 *  - **cold start** → below the data gates it teaches instead of prescribing (the Academy track),
 *    so the card degrades from personalised to principled, never to silence;
 *  - **cardio** stays a suggestion until Engine E-B can actually prescribe one.
 */
object TodayDirective {

    enum class Kind {
        /** Train the named program day. */
        TRAIN,

        /** Deliberate rest, with something better to do than nothing. */
        REST,

        /** Move, but not in the gym. Suggestion-only until Engine E-B. */
        CARDIO,

        /** No data yet: the curriculum carries the day. */
        LEARN
    }

    /**
     * @param headline the answer itself, in the app's voice ("Pull day", "Rest today").
     * @param reason why — one sentence, always present.
     * @param dayKey the program day to open when [kind] is TRAIN.
     * @param secondary an optional second slot ("then 20 min easy cardio"), so a dual-discipline
     *   day stays one answer instead of becoming a menu.
     * @param soreMuscles muscles today should tread lightly on, carried through to the brief.
     * @param lessonId the Academy lesson behind this directive, when one applies.
     */
    data class Directive(
        val kind: Kind,
        val headline: String,
        val reason: String,
        val dayKey: String? = null,
        val secondary: String? = null,
        val soreMuscles: Set<MuscleGroup> = emptySet(),
        val lessonId: String? = null,
        /** True when the coach is answering without a program (freestyle) — the UI preps nothing. */
        val freestyle: Boolean = false
    )

    /**
     * @param nextUpDayKey what the schedule says is next (from `WeeklySchedule.resolveNextUp`).
     * @param trainedToday a session already finished today.
     * @param weekdayMode the schedule can name weekdays; false = sequence mode's degraded read.
     * @param sessionsThisWeek finished sessions since the week started, for the weekly budget.
     * @param weeklyTarget the athlete's own sessions-per-week goal, when they set one.
     */
    fun compute(
        s: AdaptationSnapshot,
        readiness: Recommendation.ReadinessScale?,
        life: LifeEvents.State,
        nextUpDayKey: String?,
        dayName: (String) -> String,
        trainedToday: Boolean,
        weekdayMode: Boolean,
        sessionsThisWeek: Int,
        weeklyTarget: Int? = null,
        freestyle: Boolean = false,
        t: AdaptThresholds = AdaptThresholds()
    ): Directive {
        // ── Life first. Illness and a live layoff outrank every schedule. ─────────
        if (life.sick) {
            return Directive(
                kind = Kind.REST,
                headline = "Rest today",
                reason = "You flagged being unwell. Training through it costs more than it buys.",
                soreMuscles = life.soreMuscles
            )
        }

        val finished = s.sessions.filter { it.finishedAt != null && !it.isUntracked }

        // ── Cold start: the curriculum IS the directive until the data gates open ─
        if (finished.size < t.readinessMinSessions && !trainedToday) {
            return Directive(
                kind = if (nextUpDayKey != null) Kind.TRAIN else Kind.LEARN,
                headline = nextUpDayKey?.let(dayName) ?: "Start with the basics",
                reason = if (nextUpDayKey != null) {
                    "Your first sessions set the baseline everything else is measured against."
                } else {
                    "Nothing logged yet. Learn the movements first; the numbers come after."
                },
                dayKey = nextUpDayKey,
                lessonId = LESSON_HOW_THE_COACH_WORKS,
                freestyle = freestyle
            )
        }

        if (trainedToday) {
            return Directive(
                kind = Kind.REST,
                headline = "Done for today",
                reason = "You've trained. Eat, sleep, and let it land.",
                soreMuscles = life.soreMuscles
            )
        }

        // ── Coming back from a real break ─────────────────────────────────────────
        life.layoff?.let { layoff ->
            if (layoff.away || layoff.returning) {
                return Directive(
                    kind = Kind.TRAIN,
                    headline = nextUpDayKey?.let(dayName) ?: "Ease back in",
                    reason = if (layoff.away) {
                        "It's been ${layoff.days} days. Start light: this one is about showing up."
                    } else {
                        "First week back after ${layoff.days} days off, so loads hold about 10% under."
                    },
                    dayKey = nextUpDayKey,
                    soreMuscles = life.soreMuscles,
                    freestyle = freestyle
                )
            }
        }

        // CALENDAR days, not elapsed 24-hour blocks. The rules below read `daysSinceLast < 1` as
        // "trained today" and the copy at the bottom says "It's been N days since your last
        // session" — both of which an elapsed-hours count gets wrong for anyone who trains in the
        // evening. Two users who both last trained YESTERDAY used to get opposite directives purely
        // on 06:30 vs 20:00: the 20:00 one was told "Rest today", the 06:30 one "train what's next".
        val daysSinceLast = finished.maxOfOrNull { it.startedAt }
            ?.let {
                java.time.temporal.ChronoUnit.DAYS.between(
                    java.time.Instant.ofEpochMilli(it).atZone(s.zoneId).toLocalDate(),
                    java.time.Instant.ofEpochMilli(s.nowMs).atZone(s.zoneId).toLocalDate()
                ).toInt()
            } ?: Int.MAX_VALUE

        // ── Recovery spacing: trained yesterday and readiness is poor → rest ──────
        val readinessLow = (readiness?.percent ?: 0) <= -t.readinessRestThreshold
        if (daysSinceLast < 1 && readinessLow) {
            return Directive(
                kind = Kind.CARDIO,
                headline = "Take it easy today",
                reason = readiness?.reason?.let { "Readiness is low: $it." }
                    ?: "Readiness is low today.",
                secondary = "A 20-minute walk would serve recovery better than a session.",
                soreMuscles = life.soreMuscles,
                lessonId = LESSON_READINESS
            )
        }

        // ── The weekly budget: hitting the target early doesn't mean training daily ─
        if (weeklyTarget != null && sessionsThisWeek >= weeklyTarget && daysSinceLast < 1) {
            return Directive(
                kind = Kind.REST,
                headline = "Rest today",
                reason = "You've hit $sessionsThisWeek sessions this week, which is what you set out to do.",
                soreMuscles = life.soreMuscles
            )
        }

        // ── Otherwise: train what's next ─────────────────────────────────────────
        if (nextUpDayKey == null) {
            return Directive(
                kind = if (freestyle) Kind.TRAIN else Kind.REST,
                headline = if (freestyle) "Train freestyle" else "Rest today",
                reason = if (freestyle) {
                    "No fixed program, so pick your lifts. Spacing says you're ready for one."
                } else {
                    "Nothing scheduled today."
                },
                soreMuscles = life.soreMuscles,
                freestyle = freestyle
            )
        }

        val soreOnDeck = life.soreMuscles.intersect(musclesOf(s, nextUpDayKey))
        val reason = buildString {
            append(
                when {
                    daysSinceLast >= 3 -> "It's been $daysSinceLast days since your last session."
                    weekdayMode -> "It's what today's schedule calls for."
                    // Sequence mode can't name a weekday, so it names position instead (§ degraded).
                    else -> "It's the next day in your rotation."
                }
            )
            if (soreOnDeck.isNotEmpty()) {
                append(" ")
                append(
                    "You flagged ${soreOnDeck.joinToString(" and ") { it.displayName.lowercase() }} " +
                        "as sore, so those come in lighter."
                )
            }
            readiness?.takeIf { it.percent > 0 }?.let { append(" Readiness is good.") }
        }

        return Directive(
            kind = Kind.TRAIN,
            headline = dayName(nextUpDayKey),
            reason = reason,
            dayKey = nextUpDayKey,
            soreMuscles = life.soreMuscles,
            freestyle = freestyle
        )
    }

    const val LESSON_READINESS = "coach.readiness_built_from"
    const val LESSON_HOW_THE_COACH_WORKS = "fundamentals.how_the_coach_works"

    /** Muscles the given program day actually trains — used to warn about sore ones on deck. */
    private fun musclesOf(s: AdaptationSnapshot, dayKey: String): Set<MuscleGroup> =
        s.program.firstOrNull { it.dayKey == dayKey }?.slots?.map { it.muscle }?.toSet().orEmpty()

    /** Sessions finished since [weekStartMs] — the week's budget so far. */
    fun sessionsSince(s: AdaptationSnapshot, weekStartMs: Long): Int =
        s.sessions.count { it.finishedAt != null && !it.isUntracked && it.startedAt >= weekStartMs }

    /** Whether a session was already finished today, in the snapshot's own zone. */
    fun trainedToday(s: AdaptationSnapshot): Boolean {
        val startOfDay = java.time.Instant.ofEpochMilli(s.nowMs).atZone(s.zoneId)
            .toLocalDate().atStartOfDay(s.zoneId).toInstant().toEpochMilli()
        return s.sessions.any { it.finishedAt != null && !it.isUntracked && it.startedAt >= startOfDay }
    }

    /** Bouts on this exercise that count as ordinary training — the brief's history source. */
    internal fun trainingBouts(s: AdaptationSnapshot, exerciseId: String) =
        s.exerciseHistory[exerciseId].orEmpty().filter { it.countsForProgression && !it.skipped }
}
