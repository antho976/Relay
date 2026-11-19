package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import kotlin.math.roundToInt

/** When the live session asks how hard a set was. Stored by [code]. */
enum class EffortPromptMode(val code: String) {
    /** Once per exercise, after its last planned set: the one the coach reads. */
    LAST_SET("last"),

    /** After every working set. */
    EVERY_SET("every"),

    /** Never; effort is still reachable from a set's own RPE cell. */
    OFF("off");

    companion object {
        fun fromCode(code: String?): EffortPromptMode = entries.firstOrNull { it.code == code } ?: LAST_SET
    }
}

/**
 * The one-tap effort question: "how many more could you have done?".
 *
 * Effort used to be two taps on a small cell after the set was already logged, with nothing asking
 * for it, so it was slow to give and easy to forget, and a finished session could not take it at
 * all. Meanwhile [EffortModel] reads only the hardest working set of a bout, so most of what a
 * careful lifter entered went unread.
 *
 * So the question is asked once, where it counts, in the unit people can actually answer. Reps in
 * reserve is a count; RPE is a scale that has to be learned. The answer is stored as the RPE it
 * equals (10 minus the reps left), so every reader of [LoggedSet.rpe] is unchanged.
 *
 * Pure, so the rule about WHEN to ask is testable without a screen.
 */
object EffortPrompt {

    /** The answers offered, hardest first. The last one means "four or more". */
    val CHOICES: List<Int> = listOf(0, 1, 2, 3, 4)

    /** The RPE a reps-left answer is stored as. */
    fun rpeFor(repsLeft: Int): Double = (10 - repsLeft.coerceIn(0, CHOICES.last())).toDouble()

    /** The reps-left answer an RPE reads as, so a set rated from the half-step picker still shows its pick. */
    fun repsLeftFor(rpe: Double?): Int? = rpe?.let { (10.0 - it).roundToInt().coerceIn(0, CHOICES.last()) }

    /** "0" … "3", and "4+" for the open end. */
    fun label(repsLeft: Int): String = if (repsLeft >= CHOICES.last()) "${CHOICES.last()}+" else "$repsLeft"

    /** What a screen reader says for one answer. */
    fun spoken(repsLeft: Int): String = when {
        repsLeft <= 0 -> "None left, that was everything"
        repsLeft == 1 -> "1 more rep left"
        repsLeft >= CHOICES.last() -> "${CHOICES.last()} or more reps left"
        else -> "$repsLeft more reps left"
    }

    /**
     * The set to ask about right now, or null when there is nothing to ask.
     *
     * Only ever the set just logged, and never one that has already answered: an RPE, or a failure
     * mark, which says "none left" by itself. Warm-ups are never asked about. In [EffortPromptMode.LAST_SET]
     * the question waits until the exercise's planned working sets are in.
     */
    fun setToAsk(sets: List<LoggedSet>, targetSets: Int, mode: EffortPromptMode): LoggedSet? {
        if (mode == EffortPromptMode.OFF) return null
        val last = sets.lastOrNull() ?: return null
        if (last.setType == EffortModel.SET_TYPE_WARMUP) return null
        if (EffortModel.setSpeaks(last)) return null
        if (mode == EffortPromptMode.LAST_SET && EffortModel.workingSets(sets).size < targetSets) return null
        return last
    }

    /**
     * For the finish summary and the finished-session page: the set that should carry an exercise's
     * effort when nothing about it says how hard it was, or null when something already does.
     */
    fun unratedSet(sets: List<LoggedSet>, exerciseRating: EffortRating?): LoggedSet? {
        if (exerciseRating != null) return null
        val working = EffortModel.workingSets(sets)
        if (working.isEmpty() || working.any { EffortModel.setSpeaks(it) }) return null
        return working.last()
    }
}
