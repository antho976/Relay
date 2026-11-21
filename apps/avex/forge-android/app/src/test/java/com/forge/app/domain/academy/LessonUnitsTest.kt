package com.forge.app.domain.academy

import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

/** Lesson weights print in the reader's unit, and no lesson hardcodes one (DESIGN §11). */
class LessonUnitsTest {

    private val kgWord = Regex("""\bkg\b""")
    private val lbWord = Regex("""\blb\b""")

    private fun Lesson.copyLines(): List<String> = buildList {
        add(title); add(summary); add(unlock.label); add(unlock.detail)
        blocks.forEach { b ->
            when (b) {
                is LessonBlock.Heading -> add(b.text)
                is LessonBlock.Paragraph -> add(b.text)
                is LessonBlock.Callout -> add(b.text)
                is LessonBlock.Bullets -> addAll(b.items)
                is LessonBlock.Example -> { add(b.label); add(b.fallback) }
                is LessonBlock.Figure -> add(b.caption)
            }
        }
    }

    @Test
    fun anExampleLoadIsARoundFigureInEachSystem() {
        assertEquals("60 kg", exampleWeight(60.0, 135.0, WeightUnit.KG))
        assertEquals("62.5 kg", exampleWeight(62.5, 140.0, WeightUnit.KG))
        assertEquals("135 lb", exampleWeight(60.0, 135.0, WeightUnit.LB))
        // Stones read the pound figure the way every logged set does.
        assertEquals("9 st 9 lb", exampleWeight(60.0, 135.0, WeightUnit.ST))
        assertEquals("10 st", exampleWeight(62.5, 140.0, WeightUnit.ST))
    }

    @Test
    fun tokensResolveInEveryUnit() {
        val text = "At {w:80|175}, aim for {r:1.6|0.7} and up to {r:2.2|1}."
        assertEquals("At 80 kg, aim for 1.6 g per kg and up to 2.2 g per kg.", text.inUnit(WeightUnit.KG))
        assertEquals("At 175 lb, aim for 0.7 g per lb and up to 1 g per lb.", text.inUnit(WeightUnit.LB))
        assertEquals("At 12 st 7 lb, aim for 0.7 g per lb and up to 1 g per lb.", text.inUnit(WeightUnit.ST))
        assertEquals("No weights here.", "No weights here.".inUnit(WeightUnit.ST))
    }

    @Test
    fun noLessonHardcodesAWeightUnit() {
        AcademyRegistry.lessons.forEach { lesson ->
            lesson.copyLines().forEach { line ->
                val bare = line.replace(Regex("""\{[wr]:[^}]*\}"""), "")
                assertFalse("${lesson.id} hardcodes kg: $line", kgWord.containsMatchIn(bare))
                assertFalse("${lesson.id} hardcodes lb: $line", lbWord.containsMatchIn(bare))
            }
        }
    }

    @Test
    fun everyLessonReadsCleanlyInEveryUnit() {
        WeightUnit.entries.forEach { unit ->
            AcademyRegistry.lessons.forEach { lesson ->
                val lines = lesson.inUnit(unit).copyLines()
                lines.forEach { line ->
                    assertFalse("${lesson.id} in $unit leaves a token: $line", '{' in line || '}' in line)
                    if (unit == WeightUnit.KG) {
                        assertFalse("${lesson.id} in kg prints lb: $line", lbWord.containsMatchIn(line))
                    } else {
                        assertFalse("${lesson.id} in $unit prints kg: $line", kgWord.containsMatchIn(line))
                    }
                }
            }
        }
    }

    @Test
    fun theProteinLessonPrintsTheReadersUnit() {
        val lb = AcademyTraining.protein.inUnit(WeightUnit.LB).copyLines().joinToString(" ")
        assertTrue(lb, "0.7 g per lb" in lb && "175 lb" in lb)
        val kg = AcademyTraining.protein.inUnit(WeightUnit.KG).copyLines().joinToString(" ")
        assertTrue(kg, "1.6 g per kg" in kg && "80 kg" in kg)
    }

    @Test
    fun linesShownOutsideTheReaderCarryNoToken() {
        // The contents page and the notification feed print these as they are, unresolved.
        AcademyRegistry.lessons.forEach { l ->
            listOf(l.title, l.summary, l.unlock.label, l.unlock.detail).forEach {
                assertTrue("${l.id} needs a unit-free line: $it", it.weightTokens().isEmpty())
            }
        }
    }

    @Test
    fun eachPairIsTheSameExampleInBothSystems() {
        // Round plate figures differ from a straight conversion, but never by more than a jump: a
        // typo like {w:60|315} would teach a pound reader a different lesson.
        val kgPerLb = 0.45359237
        AcademyRegistry.lessons.forEach { lesson ->
            lesson.copyLines().flatMap { it.weightTokens() }.forEach { (kind, kg, lb) ->
                val asKg = if (kind == 'w') lb * kgPerLb else lb / kgPerLb
                assertTrue(
                    "${lesson.id}: {$kind:$kg|$lb} is ${"%.1f".format(asKg)} against $kg",
                    abs(asKg - kg) / kg <= 0.10
                )
            }
        }
    }
}
