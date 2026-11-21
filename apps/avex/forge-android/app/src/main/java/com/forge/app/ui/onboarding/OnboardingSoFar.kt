package com.forge.app.ui.onboarding

import com.forge.app.program.Equipment

/**
 * The footer's "plan so far": one mono line that grows with every answer, so the plan is seen
 * taking shape while it is answered for (Relay onboarding designs, 2026-10-21).
 *
 * Pure, so the copy for every step is pinned by `PlanSoFarTest` without composing anything.
 */
internal enum class SoFarPage { MODE, GOAL, EXPERIENCE, DAYS, WEEKDAYS, GYM, GEAR, SPOTS, WEEK, EXTRAS }

internal data class SoFarAnswers(
    val planMode: String = "",
    val goal: String = "",
    val experience: String = "",
    val daysPerWeek: Int = 0,
    val sessionMinutes: Int? = null,
    val fixedDays: Boolean? = null,
    val weekdays: Set<Int> = emptySet(),
    val equipment: Set<String> = emptySet(),
    val problemAreas: Set<String> = emptySet(),
    /** The week's day names in order, once the split is known. */
    val dayNames: List<String> = emptyList(),
    /** The week's total sets (generated, or planned before the gear is known). */
    val weekSets: Int = 0
)

private val WEEKDAY_NAMES = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")

/** The line for [page], or null when nothing has been answered yet that it could say. */
internal fun planSoFar(page: SoFarPage, a: SoFarAnswers): String? {
    val modeLabel = PLAN_MODE_DETAILS.firstOrNull { it.first == a.planMode }?.second
    val goalLabel = GOAL_DETAILS.firstOrNull { it[0] == a.goal }?.get(1)
    val expLabel = EXPERIENCE_DETAILS.firstOrNull { it[0] == a.experience }?.get(1)
    val minutes = a.sessionMinutes?.takeIf { it > 0 }
    val days = a.daysPerWeek.takeIf { it in 1..7 }
    fun dayCount(n: Int) = if (n == 1) "1 day" else "$n days"
    val gym = when {
        a.equipment.isEmpty() -> null
        a.equipment.size == Equipment.entries.size -> "full gym"
        a.equipment.size == 1 -> "1 piece"
        else -> "${a.equipment.size} pieces"
    }
    // The plan's answers in the order they were given, each only once it is known.
    fun built(withGym: Boolean, withSpots: Boolean): String? = listOfNotNull(
        goalLabel,
        days?.let(::dayCount),
        minutes?.let { "$it min" },
        gym.takeIf { withGym },
        a.problemAreas.size.takeIf { withSpots && it > 0 }?.let { if (it == 1) "1 spot flagged" else "$it spots flagged" }
    ).joinToString(" · ").ifEmpty { null }

    return when (page) {
        SoFarPage.MODE -> modeLabel
        SoFarPage.GOAL -> when {
            goalLabel == null -> modeLabel
            else -> "${modeShort(a.planMode)} · ${goalLabel.lowercase()}"
        }
        SoFarPage.EXPERIENCE -> listOfNotNull(goalLabel ?: modeLabel, expLabel?.lowercase()).joinToString(" · ").ifEmpty { null }
        SoFarPage.DAYS, SoFarPage.WEEKDAYS -> built(withGym = false, withSpots = false)
        SoFarPage.GYM, SoFarPage.GEAR -> built(withGym = true, withSpots = false)
        SoFarPage.SPOTS -> built(withGym = true, withSpots = true)
        SoFarPage.WEEK -> days?.let {
            listOfNotNull(dayCount(it), "${a.weekSets} sets", minutes?.let { m -> "~$m min each" }).joinToString(" · ")
        }
        SoFarPage.EXTRAS -> when (a.planMode) {
            PLAN_CUSTOM -> "Ready · you build the plan next"
            PLAN_FREESTYLE -> "Ready · log whatever you train"
            else -> {
                val first = a.dayNames.firstOrNull()
                val weekday = a.weekdays.minOrNull()?.takeIf { a.fixedDays == true }?.let { WEEKDAY_NAMES.getOrNull(it) }
                when {
                    first == null -> "Ready"
                    weekday != null -> "Ready · $first is first, $weekday"
                    else -> "Ready · $first is first"
                }
            }
        }
    }
}

private fun modeShort(mode: String): String = when (mode) {
    PLAN_CUSTOM -> "Own plan"
    PLAN_FREESTYLE -> "No plan"
    else -> "Plan"
}
