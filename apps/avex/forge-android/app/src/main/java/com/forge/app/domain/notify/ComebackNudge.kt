package com.forge.app.domain.notify

import com.forge.app.domain.coach.LifeEvents

/**
 * The Monday "come back" nudge after a week with no training, as a pure decision.
 *
 * It used to fire every Monday of a lapse with the same two lines ("No pressure, your plan's right
 * where you left it. Even one session this week keeps your momentum going."), for as long as the
 * lapse lasted, including for someone who had never logged a session and someone who spent the week
 * on cardio. A nudge that repeats unchanged for two months is a nag; this one speaks at about one,
 * two and four weeks, names the last workout and how long ago it was, and then stops.
 */
object ComebackNudge {

    data class Nudge(val title: String, val body: String)

    /**
     * @param daysSinceLast calendar days since the last finished, tracked session; null = never.
     * @param lastDayName the last workout's name, or null (freestyle, or no name to give).
     * @param cardioMinutesLastWeek cardio logged in the week that just ended; any at all is training.
     */
    fun build(
        daysSinceLast: Int?,
        lastDayName: String?,
        cardioMinutesLastWeek: Int,
        freestyle: Boolean
    ): Nudge? {
        val days = daysSinceLast ?: return null
        if (cardioMinutesLastWeek > 0) return null
        // The worker runs on Mondays and only after a whole Monday-to-Sunday week without a session,
        // so the last one is at least 8 days back: a lapse is seen at d, d+7, d+14... with d in
        // 8..14. Speak in the first, second and fourth week of it, then let it be. (`days / 7` read a
        // last session on a Monday as 14 days on the first Monday, week 2, and skipped week 1.)
        val week = (days - 1) / 7
        if (week !in setOf(1, 2, 4)) return null
        val gap = LifeEvents.gapPhrase(days)
        val last = if (lastDayName != null) "Last workout: $lastDayName, $gap ago." else "Last workout $gap ago."
        val rest = when {
            days >= LifeEvents.LAYOFF_MIN_DAYS -> "The first session back starts lighter."
            freestyle -> "Log whatever you train, whenever."
            else -> "Your plan is where you left it."
        }
        return Nudge(title = "Ready when you are", body = "$last $rest")
    }
}
