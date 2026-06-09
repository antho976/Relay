package com.forge.app.domain.parser

import com.forge.app.program.ExerciseUnit

/**
 * Converts what the user typed in the weight field into a numeric pound value used
 * by aggregates (volume, PRs, strength curves). Returns null when no sensible number
 * can be derived (e.g. "BW", empty string, garbage) — callers store the original text
 * verbatim regardless, and treat null as 0 lb for volume purposes.
 *
 * Recognised forms:
 *  - "45", "45.5"               → 45.0, 45.5         (plain lb; or a plate COUNT if unit = PLATES)
 *  - "BW", "bw", "" (empty)     → null                (bodyweight)
 *  - "2 plates", "1 plate", "3p"→ N * plateLb         (explicit plate count)
 *  - "30 lb", "30lb"            → 30.0                (lb suffix forces lb even on plate exercises)
 *
 * The [unit] hint biases bare numbers: on a PLATES exercise the field shows "PLATES", so a bare
 * "2" is a *plate count* → 2 * [plateLb]; on any other unit "2" is 2 lb. [plateLb] is the user's
 * configured weight-per-plate (default [PLATE_LB]).
 */
object WeightParser {

    const val PLATE_LB: Double = 15.0

    fun parse(input: String, unit: ExerciseUnit, plateLb: Double = PLATE_LB): Double? {
        val text = input.trim().lowercase()
        if (text.isEmpty() || text == "bw") return null

        // "N plate" / "N plates" / "Np" — explicit plate notation
        val plateMatch = Regex("""^([0-9]*\.?[0-9]+)\s*(plates?|p)$""").matchEntire(text)
        if (plateMatch != null) {
            val plates = plateMatch.groupValues[1].toDoubleOrNull() ?: return null
            return plates * plateLb
        }

        // "N lb" / "Nlb" — always lb regardless of unit hint
        val lbMatch = Regex("""^([0-9]*\.?[0-9]+)\s*lbs?$""").matchEntire(text)
        if (lbMatch != null) {
            return lbMatch.groupValues[1].toDoubleOrNull()
        }

        // Bare number. On a PLATES exercise it's a plate count (field is labelled "PLATES");
        // otherwise it's literal pounds. Reject negatives (which would corrupt volume / PRs).
        val n = text.toDoubleOrNull()?.takeIf { it >= 0.0 } ?: return null
        return if (unit == ExerciseUnit.PLATES) n * plateLb else n
    }
}
