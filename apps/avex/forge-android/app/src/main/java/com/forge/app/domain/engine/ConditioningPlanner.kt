package com.forge.app.domain.engine

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.LifeEvents

/**
 * The coached conditioning week (Engine E-B): where cardio goes, how much, and why.
 *
 * The whole thesis is here. Running apps coach you like a runner; lifting apps ignore conditioning
 * entirely. This places conditioning as a LIFTER: minimum effective dose, positioned so it doesn't
 * cost the sessions that matter, with a ceiling as well as a floor.
 *
 * Pure. The caller knows the calendar; this decides the prescriptions.
 */
object ConditioningPlanner {

    /** Weekly load may climb by about this much — the standard conservative ramp. */
    const val MAX_RAMP = 1.1

    /** Hard intervals need this much clearance before a lower-body session. */
    const val INTERVAL_CLEARANCE_HOURS = 24

    /** WHO's 150 min/week, the reference the cardio hub already draws against. */
    const val HEALTH_FLOOR_MINUTES = 150

    enum class Structure { STEADY, INTERVALS }

    /**
     * One prescribed piece of conditioning.
     *
     * @param zone the target zone; [zoneBand] is its bpm range when the athlete has zones at all.
     * @param effortWord the rung-one expression of the same intent, always present.
     * @param serves the goal this session is for, in the user's words.
     */
    data class Prescription(
        val minutes: Int,
        val zone: Int,
        val zoneBand: IntRange?,
        val effortWord: String,
        val structure: Structure,
        val reason: String,
        val serves: String,
        val warmUpMinutes: Int = 0,
        val coolDownMinutes: Int = 0,
        val intervals: Int = 0,
        val lessonId: String? = null
    ) {
        /** Total time including warm-up and cool-down, which intervals always carry. */
        val totalMinutes: Int get() = minutes + warmUpMinutes + coolDownMinutes
    }

    /**
     * The week's conditioning, given everything else that's going on.
     *
     * @param weeklyTargetMinutes the athlete's own target, or the health floor as the reference.
     * @param loggedThisWeek what they've already done.
     * @param liftingDaysAhead how many lifting days remain this week — conditioning fills gaps,
     *   it doesn't compete for them.
     * @param weekdayMode false in sequence mode, where the app cannot see which day is leg day, so
     *   placement claims are suppressed rather than guessed.
     */
    fun planWeek(
        profile: ConditioningProfile,
        weeklyTargetMinutes: Int,
        loggedThisWeek: List<CardioEntry>,
        nowMs: Long,
        liftingDaysAhead: Int,
        weekdayMode: Boolean,
        blockPhase: BlockPhase? = null,
        life: LifeEvents.State = LifeEvents.State.NONE
    ): List<Prescription> {
        // Illness and a live layoff outrank conditioning entirely.
        if (life.sick || life.layoff?.away == true) return emptyList()

        val doneMinutes = loggedThisWeek.filter { it.restReason == null }.sumOf { it.durationMin }
        val target = deloadAdjusted(weeklyTargetMinutes, blockPhase)
        val remaining = (target - doneMinutes).coerceAtLeast(0)
        if (remaining < MIN_USEFUL_SESSION) return emptyList()

        // Ramp guard: never prescribe a week that jumps more than the standard increase.
        val ramp = ConditioningLoad.rampRate(loggedThisWeek, nowMs)
        val rampCapped = ramp != null && ramp > MAX_RAMP

        val sessions = (remaining / TYPICAL_SESSION).coerceIn(1, MAX_SESSIONS_PER_WEEK)
        val perSession = (remaining / sessions).coerceAtLeast(MIN_USEFUL_SESSION)

        return (1..sessions).map { i ->
            // Intervals are rationed: at most one a week, only when the base is already there, and
            // never during a deload or when the ramp is already steep.
            val isInterval = i == 1 &&
                doneMinutes >= HEALTH_FLOOR_MINUTES / 2 &&
                blockPhase != BlockPhase.DELOAD &&
                !rampCapped &&
                liftingDaysAhead <= 3
            if (isInterval) intervalSession(profile, weekdayMode) else steadySession(profile, perSession, blockPhase)
        }
    }

    private fun steadySession(
        profile: ConditioningProfile,
        minutes: Int,
        phase: BlockPhase?
    ) = Prescription(
        minutes = minutes,
        zone = 2,
        zoneBand = profile.bandFor(2),
        effortWord = "easy",
        structure = Structure.STEADY,
        reason = when (phase) {
            BlockPhase.DELOAD -> "A deload week, so this stays genuinely easy."
            else -> "Easy aerobic work, placed so it costs your lifting nothing."
        },
        serves = "your aerobic base",
        lessonId = "engine.what_zone2_is"
    )

    /**
     * Intervals always carry a warm-up and a cool-down. A prescription of "8 × 1 minute hard" with
     * no ramp in or out is an incomplete instruction, and the one most likely to hurt someone.
     */
    private fun intervalSession(profile: ConditioningProfile, weekdayMode: Boolean) = Prescription(
        minutes = 16,
        zone = 4,
        zoneBand = profile.bandFor(4),
        effortWord = "hard",
        structure = Structure.INTERVALS,
        reason = if (weekdayMode) {
            "Placed clear of your next lower-body day so it doesn't tax the same legs."
        } else {
            "Keep at least a day between this and your next heavy lower-body session."
        },
        serves = "work capacity",
        warmUpMinutes = 8,
        coolDownMinutes = 5,
        intervals = 8,
        lessonId = "engine.intervals"
    )

    /** A deload week halves conditioning too: the whole week is meant to ask less. */
    private fun deloadAdjusted(target: Int, phase: BlockPhase?): Int =
        if (phase == BlockPhase.DELOAD) target / 2 else target

    /**
     * Health-floor progress, with the step double-count rule: minutes already logged as cardio
     * are never also counted as ambient step minutes, or a logged 30-minute walk credits twice.
     */
    fun healthFloorMinutes(logged: List<CardioEntry>, ambientStepMinutes: Int, nowMs: Long): Int {
        val loggedMinutes = logged
            .filter { it.restReason == null && it.date >= nowMs - 7L * 24 * 60 * 60 * 1000 }
            .sumOf { it.durationMin }
        return maxOf(loggedMinutes, ambientStepMinutes)
    }

    private const val TYPICAL_SESSION = 35
    private const val MIN_USEFUL_SESSION = 15
    private const val MAX_SESSIONS_PER_WEEK = 4
}
