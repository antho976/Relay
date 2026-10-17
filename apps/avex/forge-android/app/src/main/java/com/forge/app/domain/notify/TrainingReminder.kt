package com.forge.app.domain.notify

/**
 * Pure decision for the daily training reminder (engagement thread). The worker fetches the
 * inputs (did you train today, what's scheduled, your streak); this decides whether — and what —
 * to say, so the wording + the "don't nag" rules stay unit-testable with no Android types.
 *
 * One notification carries both jobs: a plain "train today" nudge, upgraded to a streak-protection
 * message when a streak is on the line — so the user never gets two reminders in a day. On a
 * scheduled rest day it offers a gentle "recover well" note instead of going silent.
 */
object TrainingReminder {

    /** Below this a streak isn't really "a streak" worth protecting — keep the nudge plain. */
    const val STREAK_AT_RISK_MIN = 2

    data class Nudge(val title: String, val body: String)

    /**
     * @param trainedToday already logged a session today — never nudge.
     * @param dayName the workout scheduled for today, or null on a rest day / no schedule.
     * @param streakDays current streak; ≥ [STREAK_AT_RISK_MIN] upgrades the copy to protect it.
     * @param isScheduledRestDay today is an explicit rest day in the weekly schedule — when there's
     *   no [dayName], turn the silence into a gentle recovery note rather than nothing.
     * @param noFixedPlan freestyle ("go with the flow"): there's no scheduled day to name, but the
     *   user opted into the daily nudge, so still remind them — with plan-agnostic copy.
     * @return the notification to post, or null to stay quiet.
     */
    fun build(
        trainedToday: Boolean,
        dayName: String?,
        streakDays: Int,
        isScheduledRestDay: Boolean = false,
        noFixedPlan: Boolean = false,
        /**
         * Calendar days since the last finished session, null when there has never been one. A
         * break of [com.forge.app.domain.coach.LifeEvents.LAYOFF_MIN_DAYS] or more is named in the
         * copy, the way Home names it, so the first nudge back is not the same line as any other day.
         */
        daysSinceLast: Int? = null,
        /** On a rest day, the next scheduled workout and when ("Thursday"), for the rest note. */
        nextDayName: String? = null,
        nextWhen: String? = null
    ): Nudge? {
        if (trainedToday) return null
        val layoff = daysSinceLast?.takeIf { it >= com.forge.app.domain.coach.LifeEvents.LAYOFF_MIN_DAYS }
        val gap = layoff?.let { com.forge.app.domain.coach.LifeEvents.gapPhrase(it) }
        if (dayName.isNullOrBlank()) {
            return when {
                // A planned rest day is a feature, not an absence: say what comes next instead of nagging.
                isScheduledRestDay -> Nudge(
                    title = "Rest day",
                    body = if (nextDayName != null && nextWhen != null) "Nothing scheduled today. Next: $nextDayName, $nextWhen."
                    else "Nothing scheduled today."
                )
                // Freestyle: no day to name, but still nudge, pointed at free logging.
                noFixedPlan -> when {
                    gap != null -> Nudge("Time to train", "First session in $gap. Log whatever you train today.")
                    streakDays >= STREAK_AT_RISK_MIN -> Nudge("$streakDays-day streak", "Log whatever you train today to keep it.")
                    else -> Nudge("Time to train", "Log whatever you train today.")
                }
                // No program / no schedule (and not freestyle): stay quiet, as before.
                else -> null
            }
        }
        return when {
            gap != null -> Nudge(
                title = "$dayName today",
                body = "First session in $gap. Loads start lighter: this one is about showing up."
            )
            streakDays >= STREAK_AT_RISK_MIN -> Nudge(
                title = "$streakDays-day streak",
                body = "$dayName is on today."
            )
            daysSinceLast != null && daysSinceLast >= 3 -> Nudge(
                title = "Time to train",
                body = "$dayName today. Your last session was $daysSinceLast days ago."
            )
            else -> Nudge(title = "Time to train", body = "$dayName today.")
        }
    }
}
