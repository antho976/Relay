package com.forge.app.ui.academy

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Path
import com.forge.app.domain.academy.AcademyCardio
import com.forge.app.domain.academy.AcademyCoachLessons
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.AcademyTraining
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.sin

/**
 * The contents list's miniatures, one per lesson (2026-09-26).
 *
 * The first cut drew the lesson's full drawing at 84dp with its words and guides stripped off. Antho:
 * they looked *"off"*, because a frame from a drawing that only makes sense once it has finished,
 * with its labels removed, reads as something half-loaded. So each lesson gets its own composition
 * here, made for its size the way an app icon is: one idea, heavy strokes, the accent in one place,
 * and nothing that needs a word to be complete. They are drawn still and never animate.
 */
object AcademyThumbs {

    private val byId: Map<String, Sketch> by lazy {
        mapOf(
            AcademyTraining.gettingStronger.id to stairs(),
            AcademyTraining.effort.id to reserve(),
            AcademyTraining.volume.id to doseCurve(),
            AcademyTraining.form.id to ramp(),
            AcademyTraining.recovery.id to waves(),
            AcademyTraining.soreness.id to twoPains(),
            AcademyTraining.protein.id to plateau(),
            AcademyCoachLessons.howItDecides.id to loop(),
            AcademyCoachLessons.readiness.id to band(),
            AcademyCoachLessons.blocks.id to phases(),
            AcademyCardio.zone2.id to zone(),
            AcademyCardio.intervals.id to pulses()
        )
    }

    fun forLesson(lessonId: String): Sketch? = byId[AcademyRegistry.canonical(lessonId)]

    private const val INK_W = 1.9f
    private const val ACCENT_W = 1.6f
    private const val GUIDE_W = 2.2f

    private fun still(vararg marks: SketchMark): Sketch = sketch("") { at(0f, 1f, *marks) }

    private fun ink(path: Path) = SketchMark.Line(path, Ink.INK, weight = INK_W)
    private fun accent(path: Path) = SketchMark.Line(path, Ink.ACCENT, weight = ACCENT_W)
    private fun guide(path: Path) = SketchMark.Line(path, Ink.GUIDE, weight = GUIDE_W)

    private fun stairs(): Sketch = with(AcademySketches) {
        val steps = line(16f, 88f, 48f, 88f, 48f, 68f, 80f, 68f, 80f, 48f, 112f, 48f, 112f, 26f, 146f, 26f)
        still(
            guide(line(12f, 96f, 150f, 96f)),
            SketchMark.Area(under(steps, 16f, 146f, 96f), Ink.INK),
            ink(steps),
            SketchMark.Dot(146f, 26f, 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun reserve(): Sketch = with(AcademySketches) {
        val done = (0 until 7).map { i ->
            SketchMark.Column(22f + i * 14f, 22f + i * 8f, 92f, 9f, Ink.INK)
        }
        val left = (0 until 2).map { i ->
            SketchMark.Column(124f + i * 14f, 78f + i * 6f, 92f, 9f, Ink.GUIDE)
        }
        still(*(done + left).toTypedArray(), accent(line(114f, 12f, 114f, 98f)))
    }

    private fun doseCurve(): Sketch = with(AcademySketches) {
        fun g(x: Float) = 92f - 70f * (1f - exp(-(x - 16f) / 36f))
        val c = curve(16f, 148f) { g(it) }
        still(
            guide(line(16f, 12f, 16f, 92f, 148f, 92f)),
            SketchMark.Band(56f, 14f, 100f, 92f, Ink.ACCENT),
            SketchMark.Area(under(c, 16f, 148f, 92f), Ink.INK),
            ink(c),
            SketchMark.Dot(56f, g(56f), 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun ramp(): Sketch = with(AcademySketches) {
        val xs = listOf(26f, 54f, 82f, 110f, 138f)
        val tops = listOf(76f, 62f, 48f, 35f, 18f)
        still(
            guide(line(12f, 92f, 150f, 92f)),
            *xs.indices.map { i ->
                SketchMark.Column(xs[i], tops[i], 92f, 17f, if (i == 4) Ink.ACCENT else Ink.INK)
            }.toTypedArray()
        )
    }

    private fun waves(): Sketch = with(AcademySketches) {
        val w = smooth(12f, 74f, 22f, 74f, 38f, 94f, 54f, 58f, 70f, 80f, 90f, 42f, 106f, 64f, 126f, 24f, 148f, 36f)
        still(
            dotted(line(12f, 74f, 150f, 74f)),
            SketchMark.Area(under(w, 12f, 148f, 100f), Ink.INK),
            ink(w),
            SketchMark.Dot(126f, 24f, 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun twoPains(): Sketch = with(AcademySketches) {
        val hump = smooth(14f, 90f, 24f, 70f, 34f, 30f, 44f, 38f, 58f, 76f, 72f, 88f)
        val sharp = line(88f, 90f, 91f, 34f, 108f, 36f, 114f, 16f, 120f, 34f, 136f, 34f, 142f, 16f, 148f, 32f)
        still(
            guide(line(12f, 92f, 74f, 92f)),
            guide(line(86f, 92f, 150f, 92f)),
            SketchMark.Area(under(hump, 14f, 72f, 92f), Ink.INK),
            ink(hump),
            SketchMark.Area(under(sharp, 88f, 148f, 92f), Ink.ACCENT),
            accent(sharp)
        )
    }

    private fun plateau(): Sketch = with(AcademySketches) {
        fun b(x: Float) = 92f - 68f * (1f - exp(-(x - 16f) / 24f))
        val c = curve(16f, 148f) { b(it) }
        still(
            guide(line(16f, 12f, 16f, 92f, 148f, 92f)),
            SketchMark.Area(under(c, 16f, 148f, 92f), Ink.INK),
            accent(line(86f, 92f, 86f, 16f)),
            ink(c),
            SketchMark.Dot(86f, b(86f), 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun loop(): Sketch = with(AcademySketches) {
        val cx = 80f
        val cy = 55f
        val r = 38f
        fun on(deg: Float) = Offset(cx + r * cos(deg * PI.toFloat() / 180f), cy + r * sin(deg * PI.toFloat() / 180f))
        val arcs = Path()
        listOf(-90f, 0f, 90f, 180f).forEach { from ->
            val a0 = from + 16f
            val a1 = from + 74f
            val p0 = on(a0)
            arcs.moveTo(p0.x, p0.y)
            var a = a0
            while (a < a1) {
                a = minOf(a + 3f, a1)
                val p = on(a)
                arcs.lineTo(p.x, p.y)
            }
            val tip = on(a1)
            val back = on(a1 - 6f)
            arcs.addPath(arrowHead(tip.x, tip.y, tip.x - back.x, tip.y - back.y, size = 5f))
        }
        val top = on(-90f)
        val right = on(0f)
        val left = on(180f)
        val bottom = on(90f)
        still(
            ink(arcs),
            SketchMark.Dot(top.x, top.y, 5.5f, Ink.INK, filled = false),
            SketchMark.Dot(right.x, right.y, 5.5f, Ink.INK, filled = false),
            SketchMark.Dot(left.x, left.y, 5.5f, Ink.INK, filled = false),
            SketchMark.Dot(bottom.x, bottom.y, 7f, Ink.ACCENT, halo = true)
        )
    }

    private fun band(): Sketch = with(AcademySketches) {
        val ys = listOf(44f, 36f, 48f, 38f, 34f, 46f, 40f, 50f, 78f)
        val xs = ys.indices.map { 18f + it * 15.5f }
        val trace = smooth(*xs.indices.flatMap { listOf(xs[it], ys[it]) }.toFloatArray())
        still(
            SketchMark.Band(12f, 26f, 150f, 58f, Ink.GUIDE, radius = 4f),
            ink(trace),
            SketchMark.Dot(xs.last(), ys.last(), 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun phases(): Sketch = with(AcademySketches) {
        val fitness = smooth(14f, 84f, 60f, 74f, 104f, 64f, 148f, 56f)
        val shows = smooth(14f, 94f, 90f, 94f, 118f, 90f, 134f, 70f, 148f, 44f)
        still(
            SketchMark.Band(14f, 14f, 58f, 22f, Ink.GUIDE, radius = 3f),
            SketchMark.Band(62f, 14f, 104f, 22f, Ink.GUIDE, radius = 3f),
            SketchMark.Band(108f, 14f, 124f, 22f, Ink.GUIDE, radius = 3f),
            SketchMark.Band(128f, 14f, 148f, 22f, Ink.ACCENT, radius = 3f),
            ink(fitness),
            SketchMark.Area(under(shows, 14f, 148f, 98f), Ink.ACCENT),
            accent(shows),
            SketchMark.Dot(148f, 44f, 6f, Ink.ACCENT, halo = true)
        )
    }

    private fun zone(): Sketch = with(AcademySketches) {
        val hr = smooth(14f, 90f, 30f, 80f, 44f, 60f, 64f, 56f, 90f, 58f, 116f, 55f, 148f, 56f)
        still(
            guide(gridH(listOf(24f, 48f, 64f, 88f), 12f, 150f)),
            SketchMark.Band(12f, 48f, 150f, 64f, Ink.ACCENT),
            SketchMark.Area(under(hr, 14f, 148f, 96f), Ink.INK),
            ink(hr)
        )
    }

    private fun pulses(): Sketch = with(AcademySketches) {
        val wave = Path().apply {
            moveTo(12f, 88f)
            lineTo(26f, 70f)
            var x = 30f
            repeat(4) {
                lineTo(x, 70f); lineTo(x, 22f); lineTo(x + 14f, 22f); lineTo(x + 14f, 70f)
                x += 28f
                lineTo(x, 70f)
            }
            lineTo(150f, 88f)
        }
        val fills = Path().apply {
            var x = 30f
            repeat(4) {
                addRoundRect(RoundRect(x, 22f, x + 14f, 92f, CornerRadius(0f)))
                x += 28f
            }
        }
        still(
            guide(line(12f, 92f, 150f, 92f)),
            SketchMark.Area(fills, Ink.ACCENT),
            accent(wave)
        )
    }
}
