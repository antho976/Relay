package com.forge.app.domain.academy

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.trimDecimal

/**
 * Weights inside lesson copy, written once and printed in the reader's own unit (DESIGN §11: never
 * hardcode lb/kg).
 *
 * A lesson's example numbers are not conversions. "60 kg" converts to 132.3 lb, which nobody loads;
 * the pound version of the same example is 135 lb, the bar plus a 45 on each side. So every example
 * is written as a PAIR of round figures, one per plate system, and the reader's unit picks one.
 * Stones read the pound figure in the app's stone-and-pound form, as every logged set does, because
 * a lifter who weighs in stones still loads pound or kilogram plates.
 *
 * Two tokens, both resolved by [inUnit]:
 *  - `{w:60|135}` a load or a bodyweight: "60 kg", "135 lb", "9 st 9 lb".
 *  - `{r:1.6|0.7}` grams of protein per unit of bodyweight: "1.6 g per kg", "0.7 g per lb".
 *
 * Only the reader resolves tokens, so a field shown anywhere else (a lesson's title, summary and
 * unlock lines reach the contents page and the notification feed as they are) must not carry one.
 * `LessonUnitsTest` holds both rules.
 */

/** An example load in [unit]: [kg] on a kilogram scale, [lb] (its own round figure) otherwise. */
fun exampleWeight(kg: Double, lb: Double, unit: WeightUnit): String = when (unit) {
    WeightUnit.KG -> "${trimDecimal(kg)} kg"
    WeightUnit.LB -> "${trimDecimal(lb)} lb"
    WeightUnit.ST -> formatWeight(lb, WeightUnit.ST)
}

/** The bodyweight unit a protein dose is quoted per: kilograms, or pounds for pounds and stones. */
fun proteinBasis(unit: WeightUnit): String = if (unit == WeightUnit.KG) "kg" else "lb"

/** A protein dose in grams per [proteinBasis]: [perKg] or its own rounded [perLb]. */
fun proteinDose(perKg: Double, perLb: Double, unit: WeightUnit): Double =
    if (unit == WeightUnit.KG) perKg else perLb

private val TOKEN = Regex("""\{([wr]):(\d+(?:\.\d+)?)\|(\d+(?:\.\d+)?)\}""")

/** Every weight token in this text, as (kind, kg figure, lb figure). */
internal fun String.weightTokens(): List<Triple<Char, Double, Double>> =
    TOKEN.findAll(this).map { m ->
        Triple(m.groupValues[1][0], m.groupValues[2].toDouble(), m.groupValues[3].toDouble())
    }.toList()

/** This text with its weight tokens printed in [unit]. Text without tokens comes back unchanged. */
fun String.inUnit(unit: WeightUnit): String {
    if ('{' !in this) return this
    return TOKEN.replace(this) { m ->
        val kg = m.groupValues[2].toDouble()
        val lb = m.groupValues[3].toDouble()
        when (m.groupValues[1]) {
            "w" -> exampleWeight(kg, lb, unit)
            else -> "${trimDecimal(proteinDose(kg, lb, unit))} g per ${proteinBasis(unit)}"
        }
    }
}

/** The lesson as [unit] reads it: every line of copy with its weight tokens resolved. */
fun Lesson.inUnit(unit: WeightUnit): Lesson = copy(
    title = title.inUnit(unit),
    summary = summary.inUnit(unit),
    unlock = unlock.copy(label = unlock.label.inUnit(unit), detail = unlock.detail.inUnit(unit)),
    blocks = blocks.map { it.inUnit(unit) }
)

private fun LessonBlock.inUnit(unit: WeightUnit): LessonBlock = when (this) {
    is LessonBlock.Heading -> copy(text = text.inUnit(unit))
    is LessonBlock.Paragraph -> copy(text = text.inUnit(unit))
    is LessonBlock.Bullets -> copy(items = items.map { it.inUnit(unit) })
    is LessonBlock.Callout -> copy(text = text.inUnit(unit))
    is LessonBlock.Example -> copy(label = label.inUnit(unit), fallback = fallback.inUnit(unit))
    is LessonBlock.Figure -> copy(caption = caption.inUnit(unit))
}
