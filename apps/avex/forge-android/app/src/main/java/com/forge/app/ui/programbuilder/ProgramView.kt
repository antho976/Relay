package com.forge.app.ui.programbuilder

import com.forge.app.data.db.entities.OverlaySource
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.weightInputValue
import com.forge.app.domain.units.unitLabel
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.TemporalAdjusters

/**
 * The readings the Program page (2026-10-21 redesign, designs `program_pr/pe/pd`) adds to the plan
 * itself: the line under the title, the coach's change on a lift, a day's weekday and date, and
 * today's load for a lift. Pure, so they are tested without a screen.
 */

/** The running training block as the title line reads it. */
data class BlockReading(val week: Int, val weeks: Int, val phase: String)

/**
 * "Push · Pull · Legs · week 3 of 8, Accumulate": the plan's day types in the order they first
 * appear, then the block when one is running. Days whose type is outside the builder's list read by
 * their own name.
 */
internal fun programMeta(days: List<BuilderDay>, block: BlockReading?): String {
    val types = days.map { d ->
        DAY_TYPES.firstOrNull { it.first == baseArchetype(d.archetype) }?.second ?: d.name
    }.distinct()
    val split = types.joinToString(" · ")
    val week = block?.let { "week ${it.week} of ${it.weeks}, ${it.phase}" }
    return listOfNotNull(split.ifBlank { null }, week).joinToString(" · ")
}

/**
 * What the coach changed on a lift, as its tag reads: "↑ 1 rep", "↓ 1 set". Only a coach-written
 * overlay counts; a user's own edit is not a coach change. Reps compare the top of each range.
 */
internal fun coachChangeTag(baseSets: Int, baseReps: String, overlay: ProgramCustomization?): String? {
    if (overlay == null || overlay.source != OverlaySource.COACH || overlay.removed) return null
    val sets = overlay.setsOverride.takeIf { it > 0 }
    if (sets != null && sets != baseSets) {
        val d = sets - baseSets
        return "${if (d > 0) "↑" else "↓"} ${kotlin.math.abs(d)} ${if (kotlin.math.abs(d) == 1) "set" else "sets"}"
    }
    val reps = overlay.repRangeOverride
    if (reps != null && reps != baseReps) {
        val now = topOfRange(reps)
        val was = topOfRange(baseReps)
        if (now != null && was != null && now != was) {
            val d = now - was
            return "${if (d > 0) "↑" else "↓"} ${kotlin.math.abs(d)} ${if (kotlin.math.abs(d) == 1) "rep" else "reps"}"
        }
        return "New reps"
    }
    return null
}

/** The last number in a rep range ("8-10" → 10, "5" → 5); null when there is none ("AMRAP"). */
internal fun topOfRange(reps: String): Int? = Regex("""\d+""").findAll(reps).lastOrNull()?.value?.toIntOrNull()

/**
 * A day's place in the week under its bar: "Mon · 18", the date of that weekday in the current
 * week, or "Any day" when the day has no fixed weekday.
 */
internal fun weekdayDateLabel(weekdays: Set<Int>, today: LocalDate): String {
    val wd = weekdays.minOrNull() ?: return "Any day"
    val monday = today.with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    val date = monday.plusDays(wd.toLong())
    return "${WeeklySchedule.WEEKDAY_SHORT[wd]} · ${date.dayOfMonth}"
}

/**
 * Today's load for a planned lift: the coach's next target when there is one, else the heaviest
 * working weight last time. [bodyweight] lifts read their added weight ("+25 lb") or "BW".
 */
data class PlannedLoad(val lb: Double?, val bodyweight: Boolean)

internal fun loadLabel(load: PlannedLoad?, unit: WeightUnit): String? {
    if (load == null) return null
    val lb = load.lb?.takeIf { it > 0 }
    return when {
        load.bodyweight && lb == null -> "BW"
        load.bodyweight -> "+${weightInputValue(lb!!, unit)} ${unitLabel(unit)}"
        lb == null -> null
        else -> "${weightInputValue(lb, unit)} ${unitLabel(unit)}"
    }
}

/** A planned lift's sets × reps as the page draws it: "4 × 5", "3 × 8-10". */
internal fun setsTimesReps(sets: Int, reps: String): String = "$sets × ${reps.replace("-", "–")}"
