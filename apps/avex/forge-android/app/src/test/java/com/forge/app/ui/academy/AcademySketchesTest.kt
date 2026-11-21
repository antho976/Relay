package com.forge.app.ui.academy

import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonBlock
import com.forge.app.domain.units.WeightUnit
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Every lesson is taught by a drawing, and every drawing says what it shows (§14). */
@RunWith(RobolectricTestRunner::class)
class AcademySketchesTest {

    @Test
    fun everyLessonHasADrawingThatSpeaks() {
        AcademyRegistry.lessons.forEach { lesson ->
            val sketch = AcademySketches.forLesson(lesson.id)
            assertTrue("${lesson.id} has no drawing", sketch.marks.isNotEmpty())
            assertTrue("${lesson.id} drawing has no description", sketch.description.isNotBlank())
            assertTrue(
                "${lesson.id} drawing must keep ink or accent for its thumbnail",
                sketch.marks.map { it.mark }.any { (it is SketchMark.Line && it.ink != Ink.GUIDE) || it is SketchMark.Dot }
            )
        }
    }

    @Test
    fun everyMarkLandsInsideTheDrawAndTheBox() {
        WeightUnit.entries.forEach { unit -> AcademyRegistry.lessons.forEach { lesson ->
            AcademySketches.forLesson(lesson.id, unit).marks.forEach { t ->
                assertTrue("${lesson.id}: window out of range", t.t0 >= 0f && t.t1 <= 1f && t.t0 < t.t1)
                val m = t.mark
                if (m is SketchMark.Label) {
                    assertTrue("${lesson.id}: label '${m.text}' outside the box", m.x in 0f..160f && m.y in 0f..110f)
                }
            }
        } }
    }

    @Test
    fun aDrawingPrintsWeightsInTheReadersUnit() {
        val kgWord = Regex("""\bkg\b""", RegexOption.IGNORE_CASE)
        val lbWord = Regex("""\blb\b""", RegexOption.IGNORE_CASE)
        WeightUnit.entries.forEach { unit ->
            AcademyRegistry.lessons.forEach { lesson ->
                val sketch = AcademySketches.forLesson(lesson.id, unit)
                val words = sketch.marks.mapNotNull { (it.mark as? SketchMark.Label)?.text } + sketch.description
                words.forEach { w ->
                    val wrong = if (unit == WeightUnit.KG) lbWord else kgWord
                    assertFalse("${lesson.id} in $unit prints '$w'", wrong.containsMatchIn(w))
                }
            }
        }
        val lb = AcademySketches.forLesson("training.getting_stronger", WeightUnit.LB)
        assertTrue(lb.marks.any { (it.mark as? SketchMark.Label)?.text == "135 lb" })
    }

    @Test
    fun everyFigureALessonNamesExists() {
        AcademyRegistry.lessons.forEach { lesson ->
            lesson.blocks.filterIsInstance<LessonBlock.Figure>().forEach {
                assertTrue("${lesson.id} names a missing figure '${it.key}'", figureExists(it.key))
            }
        }
    }

    @Test
    fun everyLessonHasADedicatedThumbnail() {
        AcademyRegistry.lessons.forEach { lesson ->
            val thumb = AcademyThumbs.forLesson(lesson.id)
            assertTrue("${lesson.id} has no thumbnail", thumb != null && thumb.marks.isNotEmpty())
            assertTrue(
                "${lesson.id} thumbnail must not carry words",
                thumb!!.marks.none { it.mark is SketchMark.Label }
            )
        }
    }

    @Test
    fun aRetiredIdDrawsTheLessonThatAbsorbedIt() {
        assertTrue(AcademySketches.has("fundamentals.warmups"))
    }
}
