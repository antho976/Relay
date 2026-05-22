package com.forge.app.domain.parser

import com.forge.app.program.ExerciseUnit

/**
 * Converts what the user typed in the weight field into a numeric pound value used
 * by aggregates (volume, PRs, strength curves). Returns null when no sensible number
 * can be derived (e.g. "BW", empty string, garbage) — callers store the original text
 * verbatim regardless, and treat null as 0 lb for volume purposes.
 *
 * Recognised forms:
 *  - "45", "45.5"               → 45.0, 45.5         (plain lb)
 *  - "BW", "bw", "" (empty)     → null                (bodyweight)
 *  - "2 plates", "1 plate", "3p"→ N * PLATE_LB        (MWM-989, 15 lb / plate)
 *  - "30 lb", "30lb"            → 30.0                (lb suffix tolerated)
 *
 * The [unit] hint biases interpretation: if the exercise's default unit is PLATES,
 * a bare number is interpreted as a *plate count*, not lb. So "2" with [unit] = PLATES
 * returns 30.0; "2" with [unit] = DUMBBELL returns 2.0.
 */
object WeightParser {

    const val PLATE_LB: Double = 15.0

    fun parse(input: String, unit: ExerciseUnit): Double? {
        val text = input.trim().lowercase()
        if (text.isEmpty() || text == "bw") return null

        // "N plate" / "N plates" / "Np" — explicit plate notation overrides unit hint
        val plateMatch = Regex("""^([0-9]*\.?[0-9]+)\s*(plates?|p)$""").matchEntire(text)
        if (plateMatch != null) {
            val plates = plateMatch.groupValues[1].toDoubleOrNull() ?: return null
            return plates * PLATE_LB
        }

        // "N lb" / "Nlb" — always lb regardless of unit hint
        val lbMatch = Regex("""^([0-9]*\.?[0-9]+)\s*lbs?$""").matchEntire(text)
        if (lbMatch != null) {
            return lbMatch.groupValues[1].toDoubleOrNull()
        }

        // Bare number — interpret per unit hint
        val bareNumber = text.toDoubleOrNull() ?: return null
        return when (unit) {
            ExerciseUnit.PLATES -> bareNumber * PLATE_LB
            ExerciseUnit.DUMBBELL -> bareNumber
            ExerciseUnit.BODYWEIGHT -> null
        }
    }
}
