package com.forge.app.domain.parser

import com.forge.app.program.ExerciseUnit
import com.forge.app.domain.units.normalizeDecimalInput

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

    // Compiled once: parse runs per logged set, per import row and per history recompute.
    private val PLATE_REGEX = Regex("""^([0-9]*\.?[0-9]+)\s*(plates?|p)$""")
    private val LB_REGEX = Regex("""^([0-9]*\.?[0-9]+)\s*lbs?$""")

    fun parse(input: String, unit: ExerciseUnit, plateLb: Double = PLATE_LB): Double? {
        // Normalise the decimal separator first: toDoubleOrNull below is locale-independent and
        // takes only '.', so a comma-locale keyboard's "82,5" parsed as null and logged a set with
        // no weight — while still displaying "82,5" back to the user.
        val text = normalizeDecimalInput(input).lowercase()
        if (text.isEmpty() || text == "bw") return null

        // "N plate" / "N plates" / "Np" — explicit plate notation. Every branch drops a non-finite
        // result: a 309-digit number parses to Infinity, and a huge plate count overflows to it.
        val plateMatch = PLATE_REGEX.matchEntire(text)
        if (plateMatch != null) {
            val plates = plateMatch.groupValues[1].toDoubleOrNull() ?: return null
            return (plates * plateLb).takeIf { it.isFinite() }
        }

        // "N lb" / "Nlb" — always lb regardless of unit hint
        val lbMatch = LB_REGEX.matchEntire(text)
        if (lbMatch != null) {
            return lbMatch.groupValues[1].toDoubleOrNull()?.takeIf { it.isFinite() }
        }

        // Bare number. On a PLATES exercise it's a plate count (field is labelled "PLATES");
        // otherwise it's literal pounds. Reject negatives and overflow ("1e999" parses to Infinity),
        // either of which would corrupt volume / PRs.
        val n = text.toDoubleOrNull()?.takeIf { it >= 0.0 && it.isFinite() } ?: return null
        return (if (unit == ExerciseUnit.PLATES) n * plateLb else n).takeIf { it.isFinite() }
    }
}
