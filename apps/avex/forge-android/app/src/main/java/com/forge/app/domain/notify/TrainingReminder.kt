package com.forge.app.domain.notify

/**
 * Pure decision for the daily training reminder (engagement thread). The worker fetches the
 * inputs (did you train today, what's scheduled, your streak); this decides whether — and what —
 * to say, so the wording + the "don't nag" rules stay unit-testable with no Android types.
 *
 * One notification carries both jobs: a plain "train today" nudge, upgraded to a streak-protection
 * message when a streak is on the line — so the user never gets two reminders in a day.
 */
object TrainingReminder {

    /** Below this a streak isn't really "a streak" worth protecting — keep the nudge plain. */
    const val STREAK_AT_RISK_MIN = 2

    data class Nudge(val title: String, val body: String)

    /**
     * @param trainedToday already logged a session today — never nudge.
     * @param dayName the workout scheduled for today, or null on a rest day / no schedule — never nudge.
     * @param streakDays current streak; ≥ [STREAK_AT_RISK_MIN] upgrades the copy to protect it.
     * @return the notification to post, or null to stay quiet.
     */
    fun build(trainedToday: Boolean, dayName: String?, streakDays: Int): Nudge? {
        if (trainedToday) return null
        if (dayName.isNullOrBlank()) return null
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
