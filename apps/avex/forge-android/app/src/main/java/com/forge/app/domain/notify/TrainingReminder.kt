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
        noFixedPlan: Boolean = false
    ): Nudge? {
        if (trainedToday) return null
        if (dayName.isNullOrBlank()) {
            return when {
                // A planned rest day is a feature, not an absence — affirm it instead of nagging.
                isScheduledRestDay -> Nudge(
                    title = "Rest day",
                    body = "No workout scheduled today — recovery is where the gains land. Sleep, eat well, come back fresh."
                )
                // Freestyle: no day to name, but still nudge (streak-aware), pointed at free logging.
                noFixedPlan -> if (streakDays >= STREAK_AT_RISK_MIN)
                    Nudge(
                        title = "Don't break your streak",
                        body = "🔥 $streakDays-day streak — keep the chain alive. Log whatever you train today."
                    )
                else
                    Nudge(
                        title = "Time to train",
                        body = "Ready when you are — log whatever you're training today."
                    )
                // No program / no schedule (and not freestyle): stay quiet, as before.
                else -> null
            }
        }
        return if (streakDays >= STREAK_AT_RISK_MIN)
            Nudge(
                title = "Don't break your streak",
                body = "🔥 $streakDays-day streak — $dayName is on today. Keep the chain alive."
            )
        else
            Nudge(
                title = "Time to train",
                body = "$dayName today — ready when you are."
            )
    }
}
