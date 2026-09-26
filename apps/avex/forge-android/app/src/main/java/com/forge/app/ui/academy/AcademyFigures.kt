package com.forge.app.ui.academy

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.runtime.Stable
import androidx.compose.runtime.rememberCoroutineScope
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.fromDisplayWeight
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.roundToInt

/**
 * # Figures inside the lessons (2026-09-26)
 *
 * A lesson's opening drawing states its one idea. These go under the paragraph that explains a
 * mechanism and show it working: why the last rep costs more than it gives, how soreness fades each
 * time you repeat a movement, how the readings add up to today's readiness. Antho asked for this:
 * *"animated example of the theory in them to explain better ... go deep with graphs and animation"*.
 *
 * Two kinds, both named by a key in the lesson content (`LessonBlock.Figure`):
 *
 *  - **Animated** figures draw with the same engine and grammar as the lesson drawings. They wait
 *    until the reader has scrolled them into view, then draw once; a tap draws them again.
 *  - **Interactive** figures (`ix.*`) are the few ideas that are clearer when you move them
 *    yourself: reps in reserve, weekly volume, the warm-up for your own working weight, the protein
 *    for your own bodyweight. A slider drives the drawing directly. The calculators work in the
 *    reader's own weight unit.
 *
 * Every series here is an illustrative shape, labelled by what it shows, never a claim about the
 * reader. The numbers that ARE claims (10 sets, 1.6 g per kg, 48 to 72 hours) are the lessons' own.
 */
@Composable
fun LessonFigure(key: String, caption: String, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier.fillMaxWidth()) {
        when (key) {
            FIG_RIR -> RirExplorer()
            FIG_VOLUME -> VolumeExplorer()
            FIG_WARMUP -> WarmupCalculator()
            FIG_PROTEIN -> ProteinCalculator()
            else -> FigureSketches.forKey(key)?.let { AnimatedFigure(it) }
        }
        Spacer(Modifier.height(10.dp))
        Text(
            caption,
            style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
            color = muted
        )
    }
}

/** Every key a lesson may name. `AcademySketchesTest` checks the content against this. */
fun figureExists(key: String): Boolean =
    key in setOf(FIG_RIR, FIG_VOLUME, FIG_WARMUP, FIG_PROTEIN) || FigureSketches.forKey(key) != null

const val FIG_RIR = "ix.rir"
const val FIG_VOLUME = "ix.volume"
const val FIG_WARMUP = "ix.warmup"
const val FIG_PROTEIN = "ix.protein"

/**
 * Waits for the reader, then draws. "Seen" means most of the figure is inside the window, so a
 * figure half-scrolled past the bottom edge does not spend its animation where nobody is looking.
 */
@Composable
private fun AnimatedFigure(sketch: Sketch) {
    var seen by remember { mutableStateOf(false) }
    var replays by remember { mutableIntStateOf(0) }
    SketchCanvas(
        sketch = sketch,
        modifier = Modifier
            .onGloballyPositioned { c ->
                if (!seen && c.size.height > 0) {
                    if (c.boundsInWindow().height >= c.size.height * 0.6f) seen = true
                }
            }
            .bounceClick { replays++ },
        play = seen,
        replayKey = replays
    )
}

// ── Interactive ──────────────────────────────────────────────────────────────

/**
 * A slider's value: continuous while a finger is on it, settled onto a stop when it lets go.
 *
 * The first version snapped every movement to the nearest stop, so the dot and the drawing jumped
 * from value to value under the finger. Antho: *"choppy and laggy because it jumps to another input
 * instead of slowly getting there"*. Now the dot follows the finger exactly, the drawings take the
 * continuous value (a stop line glides between reps rather than hopping), a tap glides to its stop,
 * and a release eases onto the nearest one.
 */
@Stable
private class Slide(initial: Float, private val intervals: Int, private val scope: CoroutineScope) {
    private val anim = Animatable(initial)

    val position: Float get() = anim.value

    private fun snap(p: Float): Float = (p.coerceIn(0f, 1f) * intervals).roundToInt() / intervals.toFloat()

    fun follow(to: Float) {
        scope.launch { anim.snapTo(to.coerceIn(0f, 1f)) }
    }

    fun settle() {
        scope.launch {
            anim.animateTo(snap(anim.value), spring(Spring.DampingRatioNoBouncy, Spring.StiffnessMediumLow))
        }
    }

    fun glide(to: Float) {
        scope.launch { anim.animateTo(snap(to), ForgeMotion.standardTween(ForgeMotion.DurationEmphasized)) }
    }
}

@Composable
private fun rememberSlide(initial: Float, intervals: Int): Slide {
    val scope = rememberCoroutineScope()
    return remember(intervals) { Slide(initial, intervals, scope) }
}

/**
 * The shell every interactive figure shares: the drawing, the control's name and value, a slider.
 *
 * The drawing itself is draggable: a sideways drag anywhere across it moves the value, the way you
 * would scrub a chart, so the slider is a second handle rather than the only one. A vertical drag
 * still scrolls the lesson, because the gesture only claims the pointer once it has moved sideways.
 */
@Composable
private fun Explorer(
    sketch: Sketch,
    control: String,
    value: String,
    verdict: String?,
    slide: Slide,
    steps: Int
) {
    val scheme = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth()) {
        SketchCanvas(
            sketch = sketch,
            animate = false,
            // Held in the design box rather than centred on its content, which moves while you drag.
            fitContent = false,
            replayKey = Unit,
            modifier = Modifier.pointerInput(slide) {
                var raw = 0f
                detectHorizontalDragGestures(
                    onDragStart = { raw = slide.position },
                    onDragEnd = { slide.settle() },
                    onDragCancel = { slide.settle() }
                ) { pointer, dx ->
                    pointer.consume()
                    raw = (raw + dx / size.width).coerceIn(0f, 1f)
                    slide.follow(raw)
                }
            }
        )
        Spacer(Modifier.height(14.dp))
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                control.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = scheme.onSurfaceVariant,
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(12.dp))
            Text(value, style = MaterialTheme.typography.titleSmall, color = scheme.onBackground)
        }
        SleekSlider(slide = slide, steps = steps, label = control, valueText = value)
        if (verdict != null) {
            Spacer(Modifier.height(4.dp))
            Text(verdict, style = MaterialTheme.typography.bodyMedium, color = scheme.onBackground)
        }
    }
}

/**
 * A hairline slider: a 2dp track, the accent up to a small dot, and a 48dp-tall touch band.
 *
 * Material's slider arrived as a thick pill track with a tall bar thumb, the loudest object in a
 * lesson whose drawing is supposed to be the thing you look at. Antho: make it *"more sleek and
 * discreet"*. This keeps every affordance (drag, tap to jump, stops where the value has stops,
 * TalkBack's adjustable action) and drops the weight. [steps] counts the stops between the ends,
 * as Material's does.
 */
@Composable
private fun SleekSlider(
    slide: Slide,
    steps: Int,
    label: String,
    valueText: String
) {
    val accent = MaterialTheme.colorScheme.primary
    val track = MaterialTheme.colorScheme.outline
    val ground = MaterialTheme.colorScheme.background
    val intervals = steps + 1
    val position = slide.position
    Canvas(
        Modifier
            .fillMaxWidth()
            .height(40.dp)
            .semantics {
                contentDescription = label
                stateDescription = valueText
                progressBarRangeInfo = ProgressBarRangeInfo(position, 0f..1f, steps)
                setProgress { target -> slide.glide(target); true }
            }
            .pointerInput(slide) {
                val inset = 8.dp.toPx()
                detectTapGestures { slide.glide((it.x - inset) / (size.width - inset * 2f)) }
            }
            .pointerInput(slide) {
                // Relative, not absolute: the dot moves with the finger from wherever it was, so
                // touching the track never makes it leap to a new value.
                val inset = 8.dp.toPx()
                var raw = 0f
                detectHorizontalDragGestures(
                    onDragStart = { raw = slide.position },
                    onDragEnd = { slide.settle() },
                    onDragCancel = { slide.settle() }
                ) { pointer, dx ->
                    pointer.consume()
                    raw = (raw + dx / (size.width - inset * 2f)).coerceIn(0f, 1f)
                    slide.follow(raw)
                }
            }
    ) {
        val inset = 8.dp.toPx()
        val y = size.height / 2f
        val x = inset + (size.width - inset * 2f) * position.coerceIn(0f, 1f)
        val hair = 2.dp.toPx()
        drawLine(track, Offset(inset, y), Offset(size.width - inset, y), hair, StrokeCap.Round)
        drawLine(accent, Offset(inset, y), Offset(x, y), hair, StrokeCap.Round)
        // Stops are drawn only where there are few enough to count; thirty dots would be a texture.
        if (steps in 1..9) {
            for (i in 0..intervals) {
                val sx = inset + (size.width - inset * 2f) * i / intervals
                drawCircle(if (sx <= x) accent else track, 1.8.dp.toPx(), Offset(sx, y))
            }
        }
        drawCircle(accent.copy(alpha = 0.15f), 13.dp.toPx(), Offset(x, y))
        drawCircle(ground, 7.dp.toPx(), Offset(x, y))
        drawCircle(accent, 5.5.dp.toPx(), Offset(x, y))
    }
}

/** Slide from 0 to 5 reps left and watch the set, its RPE and what it is good for. */
@Composable
private fun RirExplorer() {
    val slide = rememberSlide(2f / 5f, 5)
    val left = slide.position * 5f
    val rir = left.roundToInt()
    Explorer(
        sketch = FigureSketches.rir(left),
        control = "Reps left in the tank",
        value = "$rir left · RPE ${10 - rir}",
        verdict = when (rir) {
            0 -> "Failure. Fine on the last set of a curl, costly on a heavy squat."
            1 -> "Very hard. Good for the last set of an exercise."
            2 -> "The usual target for a working set."
            3 -> "Working, still comfortable. It counts fully."
            4 -> "On the easy side. It counts, but for less."
            else -> "Warm-up territory. Not a working set."
        },
        slide = slide,
        steps = 4
    )
}

/** Slide weekly sets from 0 to 30 and read growth against fatigue. */
@Composable
private fun VolumeExplorer() {
    val slide = rememberSlide(12f / 30f, 30)
    val exact = slide.position * 30f
    val sets = exact.roundToInt()
    Explorer(
        sketch = FigureSketches.volume(exact),
        control = "Hard sets a week, one muscle",
        value = "$sets sets",
        verdict = when (sets) {
            in 0..4 -> "Enough to hold on to what you have."
            in 5..9 -> "Growing, with room to add."
            in 10..20 -> "The sweet spot: most of the growth, at a cost you recover from."
            in 21..25 -> "Extra growth is small now, and fatigue is climbing."
            else -> "Mostly fatigue. Few people recover from this for long."
        },
        slide = slide,
        steps = 29
    )
}

/**
 * The warm-up for the reader's own working weight, in their own unit.
 *
 * Kilograms step in 2.5 and start from a 20 kg bar; pounds and stones step in 5 lb from a 45 lb bar,
 * because those are the plates on the floor in each system. Every weight is rounded to a load you
 * can actually put on the bar and never goes below the bar itself.
 */
@Composable
private fun WarmupCalculator() {
    val unit = LocalForgeSettings.current.weightUnit
    val metric = unit == WeightUnit.KG
    val min = if (metric) 40f else 90f
    val max = if (metric) 200f else 440f
    val inc = if (metric) 2.5f else 5f
    val steps = ((max - min) / inc).roundToInt()
    val slide = rememberSlide(if (metric) 0.375f else 0.386f, steps)
    val exact = min + slide.position * (max - min)
    val work = min + (slide.position * steps).roundToInt() * inc
    val bar = if (metric) 20f else 45f
    val loads = listOf(0f, 0.5f, 0.7f, 0.85f, 1f).map { pct ->
        if (pct == 0f) bar else maxOf(bar, (work * pct / inc).roundToInt() * inc)
    }
    // Heights follow the finger; the printed loads stay on plates you can actually load.
    val shares = listOf(bar / exact, 0.5f, 0.7f, 0.85f, 1f)
    Explorer(
        sketch = FigureSketches.warmup(loads.map { inUnit(it, unit) }, shares),
        control = "Your working weight",
        value = inUnit(work, unit),
        verdict = null,
        slide = slide,
        steps = steps - 1
    )
}

/** Daily protein from the reader's own bodyweight, and how it splits over four meals. */
@Composable
private fun ProteinCalculator() {
    val unit = LocalForgeSettings.current.weightUnit
    val metric = unit == WeightUnit.KG
    val min = if (metric) 45f else 100f
    val max = if (metric) 140f else 310f
    val inc = if (metric) 1f else 5f
    val steps = ((max - min) / inc).roundToInt()
    val slide = rememberSlide(if (metric) 0.368f else 0.381f, steps)
    val body = min + (slide.position * steps).roundToInt() * inc
    fun kgOf(v: Float) = fromDisplayWeight(v.toDouble(), if (metric) WeightUnit.KG else WeightUnit.LB) * 0.45359237
    val kg = kgOf(body)
    val daily = (kg * 1.6 / 5.0).roundToInt() * 5
    val dieting = (kg * 2.2 / 5.0).roundToInt() * 5
    val meal = (daily / 4.0).roundToInt()
    // The columns grow with the finger; the grams printed on them are the rounded targets.
    val mealHeight = (kgOf(min + slide.position * (max - min)) * 1.6 / 4.0).toFloat()
    Explorer(
        sketch = FigureSketches.protein(daily, dieting, meal, mealHeight),
        control = "Your bodyweight",
        value = inUnit(body, unit),
        verdict = null,
        slide = slide,
        steps = steps - 1
    )
}

/**
 * A load picked on a kilogram or pound scale, printed in the reader's unit. Kilograms and pounds
 * print the picked number as it is (a round trip through the stored-pounds formatter turns "100 kg"
 * into "100.0 kg"); stones take the stone-and-pound form from the shared formatter.
 */
private fun inUnit(v: Float, unit: WeightUnit): String = when (unit) {
    WeightUnit.KG -> "${trim(v)} kg"
    WeightUnit.LB -> "${trim(v)} lb"
    WeightUnit.ST -> formatWeight(v.toDouble(), WeightUnit.ST)
}

private fun trim(v: Float): String = if (v % 1f == 0f) v.roundToInt().toString() else String.format(java.util.Locale.US, "%.1f", v)

// ── The drawings ─────────────────────────────────────────────────────────────

internal object FigureSketches {

    private val byKey: Map<String, Sketch> by lazy {
        mapOf(
            "fig.repeat_vs_random" to repeatVsRandom(),
            "fig.slowing_gains" to slowingGains(),
            "fig.cost_vs_stimulus" to costVsStimulus(),
            "fig.counting_sets" to countingSets(),
            "fig.range" to range(),
            "fig.rest_between_sets" to restBetweenSets(),
            "fig.twice_a_week" to twiceAWeek(),
            "fig.pain_check" to painCheck(),
            "fig.repeated_bout" to repeatedBout(),
            "fig.cut_hold" to cutHold(),
            "fig.one_change" to oneChange(),
            "fig.trust" to trust(),
            "fig.readiness_sum" to readinessSum(),
            "fig.hrv_noise" to hrvNoise(),
            "fig.volume_vs_weight" to volumeVsWeight(),
            "fig.goals_in_turn" to goalsInTurn(),
            "fig.talk_test" to talkTest(),
            "fig.base_building" to baseBuilding(),
            "fig.interference" to interference()
        )
    }

    fun forKey(key: String): Sketch? = byKey[key]

    private fun ink(path: Path, weight: Float = 1f) = SketchMark.Line(path, Ink.INK, weight = weight)
    private fun guide(path: Path) = SketchMark.Line(path, Ink.GUIDE)

    // ── Getting stronger ─────────────────────────────────────────────────────

    private fun repeatVsRandom(): Sketch = with(AcademySketches) {
        sketch("Repeating the same lifts shows a trend you can build on. Changing everything every session leaves nothing to compare.") {
            val ys = listOf(78f, 74f, 72f, 66f, 64f, 58f, 54f, 48f)
            val xs = ys.indices.map { 18f + it * 7.8f }
            val noise = listOf(60f, 36f, 76f, 50f, 30f, 68f, 44f, 58f)
            val nx = noise.indices.map { 94f + it * 7.8f }
            at(0f, 1.4f, guide(line(14f, 86f, 74f, 86f)), guide(line(90f, 86f, 150f, 86f)))
            at(0.3f, 1.4f, label("same lifts", 14f, 10f, strong = true), label("random sessions", 90f, 10f, strong = true))
            at(1.2f, 3.6f, ink(smooth(*xs.indices.flatMap { listOf(xs[it], ys[it]) }.toFloatArray())))
            stagger(1.2f, 0.6f, 0.42f, xs.indices.map { SketchMark.Dot(xs[it], ys[it], 1.7f, Ink.INK) })
            stagger(1.6f, 0.6f, 0.42f, nx.indices.map { SketchMark.Dot(nx[it], noise[it], 1.7f, Ink.GUIDE) })
            at(4.8f, 1f, SketchMark.Dot(xs.last(), ys.last(), 2.2f, Ink.ACCENT, halo = true))
            at(5.2f, 1.4f, label("a trend", 44f, 96f, Anchor.CENTER, strong = true), label("nothing to compare", 120f, 96f, Anchor.CENTER))
        }
    }

    private fun slowingGains(): Sketch = with(AcademySketches) {
        sketch("Strength climbs fast in your first months, then slower each year. Weekly jumps become monthly, then yearly.") {
            fun x(t: Float) = 14f + t * 27.2f
            fun y(t: Float) = 90f - 66f * (1f - exp(-t / 1.1f))
            val c = curve(14f, 150f) { y((it - 14f) / 27.2f) }
            at(0f, 1.6f, SketchMark.Line(axisX(90f, 14f, 150f, (0..5).map { x(it.toFloat()) }), Ink.GUIDE))
            at(0.3f, 1.4f, label("strength", 16f, 10f), label("year 1", x(1f), 98f, Anchor.CENTER), label("year 3", x(3f), 98f, Anchor.CENTER), label("year 5", x(5f), 98f, Anchor.END))
            at(1f, 4f, ink(c, 1.1f))
            at(1.6f, 4f, SketchMark.Area(under(c, 14f, 150f, 90f), Ink.INK))
            at(4.2f, 1f, SketchMark.Dot(x(0.4f), y(0.4f), 2.2f, Ink.ACCENT, halo = true))
            at(4.6f, 1.4f, label("weekly jumps", 30f, 80f, strong = true))
            at(5.2f, 1f, SketchMark.Dot(x(2f), y(2f), 1.9f, Ink.INK))
            at(5.4f, 1.4f, label("monthly", 72f, 46f))
            at(6f, 1f, SketchMark.Dot(x(4.5f), y(4.5f), 1.9f, Ink.INK))
            at(6.2f, 1.4f, label("yearly", 148f, 36f, Anchor.END))
        }
    }

    // ── Effort ───────────────────────────────────────────────────────────────

    /**
     * The reps-in-reserve explorer's drawing: ten possible reps with [left] of them in reserve.
     * [left] is continuous, so the stop line glides between reps while a finger is on the slider;
     * a rep turns solid once the line has passed its middle.
     */
    fun rir(left: Float): Sketch = with(AcademySketches) {
        val whole = left.roundToInt()
        sketch("A set of ${10 - whole} reps with $whole left in reserve, RPE ${10 - whole}.") {
            val base = 90f
            fun x(i: Float) = 18f + i * 12.4f
            fun speed(i: Int) = 60f * (1f - (i / 10f).pow(1.5f))
            val done = 10f - left
            val cols = (0 until 10).map { i ->
                if (i + 0.5f < done) SketchMark.Column(x(i.toFloat()), base - speed(i), base, 6f, Ink.INK)
                else SketchMark.Column(x(i.toFloat()), base - speed(i), base, 6f, Ink.GUIDE, ghost = true)
            }
            at(0f, 1f, guide(line(12f, base, 150f, base)))
            at(0f, 1f, *cols.toTypedArray())
            val stopX = x(done - 0.5f).coerceAtMost(x(9f) + 6.2f)
            at(0f, 1f, SketchMark.Line(line(stopX, 22f, stopX, 96f), Ink.ACCENT))
            at(0f, 1f, label("rep speed", 14f, 10f))
            at(0f, 1f, label("rep 1", x(0f), 100f, Anchor.CENTER), label("rep 10", x(9f), 100f, Anchor.CENTER))
        }
    }

    private fun costVsStimulus(): Sketch = with(AcademySketches) {
        sketch("Going from three reps in reserve to one adds a little growth. The last rep, to failure, adds almost nothing and costs far more recovery.") {
            val cx = listOf(34f, 70f, 106f, 140f)
            val stim = listOf(52f, 58f, 61f, 63f)
            val cost = listOf(20f, 27f, 38f, 64f)
            val names = listOf("3 left", "2 left", "1 left", "failure")
            at(0f, 1.4f, guide(line(14f, 90f, 150f, 90f)))
            at(0.2f, 1.2f, SketchMark.Dot(16f, 10f, 1.8f, Ink.INK), label("growth", 21f, 10f))
            at(0.4f, 1.2f, SketchMark.Dot(58f, 10f, 1.8f, Ink.GUIDE), label("recovery cost", 63f, 10f))
            stagger(1f, 1.1f, 0.9f, cx.indices.map { SketchMark.Column(cx[it] - 5f, 90f - stim[it], 90f, 8f, Ink.INK) })
            stagger(1.4f, 1.1f, 0.9f, cx.indices.map {
                SketchMark.Column(cx[it] + 5f, 90f - cost[it], 90f, 8f, if (it == 3) Ink.ACCENT else Ink.GUIDE)
            })
            stagger(1.2f, 1.2f, 0.9f, cx.indices.map { SketchMark.Label(names[it], cx[it], 99f, Anchor.CENTER, if (it == 3) Ink.INK else Ink.GUIDE) })
            at(5.4f, 1.5f, label("cost jumps", 150f, 20f, Anchor.END, strong = true))
            at(5.8f, 1.4f, label("reps left when you stop", 80f, 107f, Anchor.CENTER))
        }
    }

    // ── Volume ───────────────────────────────────────────────────────────────

    /** The volume explorer's drawing, with a marker at [sets]. */
    fun volume(sets: Float): Sketch = with(AcademySketches) {
        sketch("At ${sets.roundToInt()} hard sets a week, growth and fatigue on the illustrative curves.") {
            val base = 90f
            fun x(s: Float) = 18f + s * 4.4f
            fun growth(s: Float) = base - 66f * (1f - exp(-s / 8.3f))
            fun fatigue(s: Float) = base - 50f * (s / 30f).pow(2.2f)
            val sx = x(sets)
            at(0f, 1f, SketchMark.Line(axisX(base, 18f, 150f, listOf(x(0f), x(10f), x(20f), x(30f))), Ink.GUIDE))
            at(0f, 1f, SketchMark.Band(x(10f), 14f, x(20f), base, Ink.ACCENT))
            at(0f, 1f, ink(curve(18f, 150f) { growth((it - 18f) / 4.4f) }, 1.1f))
            at(0f, 1f, SketchMark.Line(curve(18f, 150f) { fatigue((it - 18f) / 4.4f) }, Ink.GUIDE, weight = 1.6f, style = Stroked.DASHED))
            at(0f, 1f, SketchMark.Line(line(sx, base, sx, 14f), Ink.ACCENT, weight = 0.8f))
            at(0f, 1f, SketchMark.Dot(sx, growth(sets), 2.4f, Ink.ACCENT, halo = true))
            at(0f, 1f, SketchMark.Dot(sx, fatigue(sets), 2f, Ink.GUIDE, filled = false))
            at(0f, 1f, label("0", x(0f), 98f, Anchor.CENTER), label("10", x(10f), 98f, Anchor.CENTER), label("20", x(20f), 98f, Anchor.CENTER), label("30", x(30f), 98f, Anchor.CENTER))
            at(0f, 1f, label("growth", 22f, 16f, strong = true), label("fatigue, dashed", 22f, 25f))
        }
    }

    private fun countingSets(): Sketch = with(AcademySketches) {
        sketch("A set of rows counts fully for your back and half for your biceps. A set of bench press counts fully for your chest and half for your triceps.") {
            val rows = 44f to 34f
            val bench = 44f to 80f
            val muscles = listOf("back" to 18f, "biceps" to 42f, "chest" to 70f, "triceps" to 94f)
            at(0f, 1.2f, label("rows", 36f, rows.second, Anchor.END, strong = true), label("bench", 36f, bench.second, Anchor.END, strong = true))
            at(0.3f, 1f, SketchMark.Dot(rows.first, rows.second, 2.6f, Ink.INK), SketchMark.Dot(bench.first, bench.second, 2.6f, Ink.INK))
            stagger(0.6f, 1f, 0.3f, muscles.map { (_, y) -> SketchMark.Dot(116f, y, 2.4f, Ink.INK, filled = false) })
            stagger(0.8f, 1.2f, 0.3f, muscles.map { (n, y) -> SketchMark.Label(n, 122f, y) })
            at(1.8f, 1.6f, ink(line(47f, 33f, 113f, 19f)))
            at(2.4f, 1.6f, SketchMark.Line(line(47f, 35.5f, 113f, 41f), Ink.ACCENT, style = Stroked.DASHED))
            at(3.2f, 1.6f, ink(line(47f, 79f, 113f, 71f)))
            at(3.8f, 1.6f, SketchMark.Line(line(47f, 81.5f, 113f, 93f), Ink.ACCENT, style = Stroked.DASHED))
            at(5f, 1.4f, label("1 set", 80f, 20f, Anchor.CENTER, strong = true), label("1 set", 80f, 69f, Anchor.CENTER, strong = true))
            at(5.4f, 1.4f, label("half a set", 80f, 46f, Anchor.CENTER, strong = true), label("half a set", 80f, 97f, Anchor.CENTER, strong = true))
        }
    }

    // ── Form ─────────────────────────────────────────────────────────────────

    private fun range(): Sketch = with(AcademySketches) {
        sketch("A full rep travels the whole range, including the deep, stretched end. A half rep skips the stretch, which is the part that matters most for growth.") {
            at(0f, 1.4f, SketchMark.Line(line(34f, 60f, 126f, 60f), Ink.GUIDE, weight = 2f))
            at(0.4f, 1.2f, label("top", 28f, 60f, Anchor.END), label("deep", 132f, 60f))
            at(1f, 2.4f, ink(line(34f, 34f, 126f, 34f)), ink(arrowHead(126f, 34f, 1f, 0f, 3.4f)), ink(arrowHead(34f, 34f, -1f, 0f, 3.4f)))
            at(1.4f, 1.4f, label("full rep", 34f, 24f, strong = true))
            at(3f, 2f, ink(line(34f, 84f, 80f, 84f)), ink(arrowHead(80f, 84f, 1f, 0f, 3.4f)))
            at(3.4f, 1.4f, label("half rep", 34f, 94f))
            at(4.6f, 1.6f, SketchMark.Line(line(84f, 84f, 126f, 84f), Ink.GUIDE, style = Stroked.DOTTED))
            at(5f, 1.4f, label("skipped", 105f, 94f, Anchor.CENTER))
            at(5.6f, 1.6f, SketchMark.Line(line(100f, 60f, 126f, 60f), Ink.ACCENT, weight = 2.6f))
            at(6.4f, 1.4f, label("the stretch", 113f, 49f, Anchor.CENTER, strong = true))
        }
    }

    /** The warm-up calculator's drawing: five loads with their labels and reps. */
    fun warmup(loads: List<String>, share: List<Float>): Sketch = with(AcademySketches) {
        sketch("Warm-up: ${loads.dropLast(1).joinToString(", ")}, then ${loads.last()} for your working sets.") {
            val xs = listOf(26f, 55f, 84f, 113f, 140f)
            val reps = listOf("10 reps", "5 reps", "3 reps", "1 rep", "work")
            fun top(s: Float) = 92f - s.coerceIn(0.08f, 1f) * 70f
            at(0f, 1f, guide(line(12f, 92f, 150f, 92f)))
            xs.indices.forEach { i ->
                at(0f, 1f, SketchMark.Column(xs[i], top(share[i]), 92f, 14f, if (i == 4) Ink.ACCENT else Ink.INK))
                at(0f, 1f, SketchMark.Label(loads[i], xs[i], top(share[i]) - 6f, Anchor.CENTER, Ink.INK))
                at(0f, 1f, SketchMark.Label(reps[i], xs[i], 100f, Anchor.CENTER, if (i == 4) Ink.INK else Ink.GUIDE))
            }
        }
    }

    // ── Recovery ─────────────────────────────────────────────────────────────

    private fun restBetweenSets(): Sketch = with(AcademySketches) {
        sketch("With one minute of rest, reps fall off fast across three sets. With three minutes they hold, and the total work is higher.") {
            val short = listOf(10, 7, 5)
            val long = listOf(10, 9, 8)
            val ax = listOf(24f, 40f, 56f)
            val bx = listOf(100f, 116f, 132f)
            fun top(r: Int) = 88f - r * 5.6f
            at(0f, 1.4f, guide(line(12f, 88f, 68f, 88f)), guide(line(88f, 88f, 144f, 88f)))
            at(0.3f, 1.4f, label("1 min rest", 40f, 98f, Anchor.CENTER), label("3 min rest", 116f, 98f, Anchor.CENTER, strong = true))
            stagger(1f, 1f, 0.5f, ax.indices.map { SketchMark.Column(ax[it], top(short[it]), 88f, 11f, Ink.GUIDE) })
            stagger(1.2f, 1.2f, 0.5f, ax.indices.map { SketchMark.Label("${short[it]}", ax[it], top(short[it]) - 6f, Anchor.CENTER) })
            stagger(2.8f, 1f, 0.5f, bx.indices.map { SketchMark.Column(bx[it], top(long[it]), 88f, 11f, Ink.INK) })
            stagger(3f, 1.2f, 0.5f, bx.indices.map { SketchMark.Label("${long[it]}", bx[it], top(long[it]) - 6f, Anchor.CENTER, Ink.INK) })
            at(4.8f, 1.4f, label("22 reps total", 40f, 14f, Anchor.CENTER))
            at(5.2f, 1f, SketchMark.Dot(96f, 14f, 1.8f, Ink.ACCENT, halo = true))
            at(5.2f, 1.4f, label("27 reps total", 102f, 14f, strong = true))
        }
    }

    private fun twiceAWeek(): Sketch = with(AcademySketches) {
        sketch("Training a muscle on Monday and Thursday fits two full recoveries of 48 to 72 hours into one week.") {
            fun c(i: Int) = 24f + i * 19f
            val letters = listOf("M", "T", "W", "T", "F", "S", "S")
            at(0f, 1.6f, guide(Path().apply { (0 until 7).forEach { addRoundRect(RoundRect(c(it) - 8f, 34f, c(it) + 8f, 56f, CornerRadius(2.5f))) } }))
            stagger(0.2f, 1f, 0.12f, letters.mapIndexed { i, s -> SketchMark.Label(s, c(i), 26f, Anchor.CENTER) })
            at(0.4f, 1.2f, label("one muscle, one week", 14f, 10f))
            at(1.6f, 1f, SketchMark.Dot(c(0), 45f, 3.4f, Ink.INK))
            at(2.2f, 1.8f, ink(line(c(0) + 9f, 68f, c(2) + 8f, 68f), 2f))
            at(2.8f, 1.4f, label("recovering", (c(0) + c(2) + 17f) / 2f, 78f, Anchor.CENTER))
            at(4f, 1.1f, SketchMark.Dot(c(3), 45f, 3.4f, Ink.ACCENT, halo = true))
            at(4.6f, 1.8f, ink(line(c(3) + 9f, 68f, c(5) + 8f, 68f), 2f))
            at(5.2f, 1.4f, label("recovering", (c(3) + c(5) + 17f) / 2f, 78f, Anchor.CENTER))
            at(5.8f, 1.4f, label("48 to 72 h each", 80f, 94f, Anchor.CENTER, strong = true))
        }
    }

    // ── Soreness ─────────────────────────────────────────────────────────────

    private fun painCheck(): Sketch = with(AcademySketches) {
        sketch("Ask how the pain feels. Dull and spread out is soreness: train lighter. Sharp and in one spot may be injury: stop loading it and flag it.") {
            val q = Path().apply { addRoundRect(RoundRect(40f, 4f, 120f, 22f, CornerRadius(4f))) }
            val left = Path().apply { addRoundRect(RoundRect(8f, 58f, 68f, 100f, CornerRadius(4f))) }
            val right = Path().apply { addRoundRect(RoundRect(92f, 58f, 152f, 100f, CornerRadius(4f))) }
            at(0f, 1.4f, guide(q))
            at(0.4f, 1.4f, label("how does it feel", 80f, 13f, Anchor.CENTER, strong = true))
            at(1.4f, 1.4f, ink(line(62f, 22f, 40f, 56f)), ink(arrowHead(40f, 56f, -22f, 34f, 3f)))
            at(1.8f, 1.4f, label("dull", 46f, 36f, Anchor.END))
            at(2.2f, 1.4f, ink(line(98f, 22f, 120f, 56f)), ink(arrowHead(120f, 56f, 22f, 34f, 3f)))
            at(2.6f, 1.4f, label("sharp", 114f, 36f))
            at(3.4f, 1.4f, guide(left))
            at(3.8f, 1.4f, label("soreness", 38f, 70f, Anchor.CENTER, strong = true), label("train lighter", 38f, 88f, Anchor.CENTER))
            at(4.6f, 1.4f, SketchMark.Line(right, Ink.ACCENT))
            at(5f, 1.4f, label("maybe injury", 122f, 70f, Anchor.CENTER, strong = true), label("stop, flag it", 122f, 88f, Anchor.CENTER))
        }
    }

    private fun repeatedBout(): Sketch = with(AcademySketches) {
        sketch("The first time you do a movement you get very sore. The second time, less. By the third, only a little, even as you keep progressing.") {
            val p = smooth(14f, 88f, 24f, 44f, 32f, 28f, 42f, 50f, 58f, 88f, 70f, 58f, 78f, 48f, 88f, 64f, 104f, 88f, 116f, 72f, 124f, 66f, 134f, 76f, 150f, 88f)
            at(0f, 1.4f, guide(line(12f, 88f, 150f, 88f)))
            at(0.3f, 1.4f, label("soreness", 16f, 10f))
            at(1f, 4.4f, ink(p, 1.1f))
            at(1.6f, 4.4f, SketchMark.Area(under(p, 14f, 150f, 88f), Ink.INK))
            at(1.4f, 1.4f, label("1st time", 32f, 98f, Anchor.CENTER))
            at(2.8f, 1.4f, label("2nd", 78f, 98f, Anchor.CENTER))
            at(4.2f, 1.4f, label("3rd", 124f, 98f, Anchor.CENTER, strong = true))
            at(5.4f, 1f, SketchMark.Dot(124f, 66f, 2.2f, Ink.ACCENT, halo = true))
            at(5.8f, 1.4f, label("less sore", 124f, 55f, Anchor.CENTER, strong = true))
        }
    }

    // ── Protein ──────────────────────────────────────────────────────────────

    /** The protein calculator's drawing: the day's total, the dieting ceiling, and four meals. */
    fun protein(daily: Int, dieting: Int, meal: Int, mealHeight: Float = meal.toFloat()): Sketch = with(AcademySketches) {
        sketch("About $daily grams of protein a day, up to $dieting while dieting, or about $meal grams at each of four meals.") {
            val xs = listOf(38f, 66f, 94f, 122f)
            val top = 98f - (mealHeight.coerceAtMost(70f) * 0.72f)
            at(0f, 1f, SketchMark.Label("$daily", 80f, 16f, Anchor.CENTER, Ink.INK, figure = true))
            at(0f, 1f, label("grams a day", 80f, 30f, Anchor.CENTER, strong = true))
            at(0f, 1f, label("up to $dieting g while dieting", 80f, 40f, Anchor.CENTER))
            at(0f, 1f, guide(line(22f, 98f, 138f, 98f)))
            xs.forEachIndexed { i, x ->
                at(0f, 1f, SketchMark.Column(x, top, 98f, 16f, Ink.ACCENT))
                at(0f, 1f, SketchMark.Label("$meal g", x, top - 6f, Anchor.CENTER, Ink.INK))
                at(0f, 1f, SketchMark.Label("meal ${i + 1}", x, 105f, Anchor.CENTER))
            }
        }
    }

    private fun cutHold(): Sketch = with(AcademySketches) {
        sketch("Over a cut, bodyweight trends down. A lift that stays flat, instead of slipping the way it usually does, is a win.") {
            val weight = smooth(14f, 20f, 40f, 24f, 64f, 26f, 88f, 32f, 112f, 36f, 150f, 44f)
            val lift = smooth(14f, 76f, 50f, 75f, 90f, 76f, 120f, 75f, 150f, 75f)
            at(0f, 1.4f, label("bodyweight", 14f, 10f))
            at(0.4f, 3f, ink(weight))
            at(2.6f, 1.4f, label("squat", 14f, 62f))
            at(3f, 2.4f, SketchMark.Line(line(14f, 76f, 150f, 92f), Ink.GUIDE, weight = 1.4f, style = Stroked.DASHED))
            at(3.6f, 1.4f, label("usual slip", 150f, 100f, Anchor.END))
            at(4f, 3f, SketchMark.Line(lift, Ink.ACCENT))
            at(6.8f, 1f, SketchMark.Dot(150f, 75f, 2.2f, Ink.ACCENT, halo = true))
            at(7f, 1.4f, label("held", 144f, 66f, Anchor.END, strong = true))
        }
    }

    // ── Your coach ───────────────────────────────────────────────────────────

    private fun oneChange(): Sketch = with(AcademySketches) {
        sketch("A change is proposed, you apply it, the coach watches it for 14 days, then judges whether it worked. You can undo it at any point.") {
            val ticks = Path().apply { (0 until 14).forEach { i -> val x = 54f + i * 5.8f; moveTo(x, 46f); lineTo(x, 54f) } }
            at(0f, 1.6f, guide(line(14f, 50f, 150f, 50f)))
            at(0.6f, 1f, SketchMark.Dot(20f, 50f, 3f, Ink.INK, filled = false))
            at(0.9f, 1.4f, label("proposed", 20f, 38f, Anchor.CENTER))
            at(1.6f, 1f, SketchMark.Dot(42f, 50f, 3f, Ink.INK))
            at(1.9f, 1.4f, label("you apply", 42f, 62f, Anchor.CENTER, strong = true))
            at(2.4f, 3f, ink(ticks))
            at(3f, 1.4f, label("watched 14 days", 92f, 38f, Anchor.CENTER))
            at(5.4f, 1.1f, SketchMark.Dot(142f, 50f, 3.2f, Ink.ACCENT, halo = true))
            at(5.8f, 1.4f, label("verdict", 142f, 62f, Anchor.CENTER, strong = true))
            at(6.4f, 1.6f, SketchMark.Line(Path().apply { moveTo(42f, 55f); quadraticTo(44f, 82f, 62f, 84f) }, Ink.GUIDE, style = Stroked.DOTTED))
            at(6.8f, 1.4f, label("undo any time", 66f, 84f))
        }
    }

    private fun trust(): Sketch = with(AcademySketches) {
        sketch("Each kind of change earns its own record. One that keeps working, like adding weight, can apply itself. One with misses keeps asking you.") {
            val rows = listOf(
                Triple("add weight", 26f, listOf(1, 1, 1, 1, 1, 1)),
                Triple("add a set", 56f, listOf(1, 0, 1, 1, 0, 1)),
                Triple("swap exercise", 86f, listOf(1, 0, 0))
            )
            rows.forEachIndexed { r, (name, y, record) ->
                at(r * 1.8f, 1.2f, label(name, 14f, y, strong = r == 0))
                stagger(r * 1.8f + 0.4f, 0.6f, 0.2f, record.mapIndexed { j, ok ->
                    SketchMark.Dot(64f + j * 8f, y, 2.3f, Ink.INK, filled = ok == 1)
                })
            }
            at(6f, 1f, SketchMark.Dot(118f, 26f, 2.4f, Ink.ACCENT, halo = true))
            at(6.2f, 1.4f, label("on auto", 150f, 26f, Anchor.END, strong = true))
            at(6.6f, 1.4f, label("asks you", 150f, 56f, Anchor.END), label("asks you", 150f, 86f, Anchor.END))
            at(7f, 1.4f, SketchMark.Dot(64f, 104f, 1.8f, Ink.INK), label("worked", 69f, 104f), SketchMark.Dot(100f, 104f, 1.8f, Ink.INK, filled = false), label("missed", 105f, 104f))
        }
    }

    private fun readinessSum(): Sketch = with(AcademySketches) {
        sketch("Each reading moves today's readiness a point up or down against your normal. A short night, a higher resting heart rate and a hard week add up to two points down. The total is capped.") {
            val xs = listOf(24f, 46f, 68f, 90f, 112f, 136f)
            fun y(points: Int) = 36f - points * 14f
            val moves = listOf(-1, 1, -1, 0, -1)
            var level = 0
            at(0f, 1.6f, dotted(line(14f, y(0), 150f, y(0))))
            at(0.3f, 1.4f, label("your normal", 150f, y(0) - 7f, Anchor.END), label("readiness points", 14f, 8f))
            at(0.6f, 1.6f, SketchMark.Line(line(14f, y(-3), 150f, y(-3)), Ink.GUIDE, style = Stroked.DASHED))
            at(0.9f, 1.4f, label("cap", 14f, y(-3) - 5f))
            moves.forEachIndexed { i, m ->
                val from = level
                level += m
                val t = 1.4f + i * 0.9f
                if (m == 0) {
                    at(t, 0.8f, ink(line(xs[i] - 7f, y(from), xs[i] + 7f, y(from)), 1.6f))
                } else {
                    at(t, 0.9f, SketchMark.Column(xs[i], minOf(y(from), y(level)), maxOf(y(from), y(level)), 13f, Ink.INK))
                }
                val above = m > 0
                val ly = if (above) minOf(y(from), y(level)) - 5f else maxOf(y(from), y(level)) + 5f
                at(t + 0.3f, 1f, SketchMark.Label(if (m > 0) "+$m" else "$m", xs[i], if (m == 0) y(from) + 5f else ly, Anchor.CENTER, Ink.INK))
                at(t + 0.4f, 0.8f, dotted(line(xs[i] + 8f, y(level), xs[i + 1] - 8f, y(level))))
            }
            at(6.2f, 1.2f, SketchMark.Column(xs[5], y(0), y(level), 13f, Ink.ACCENT))
            at(6.8f, 1.4f, label("$level", xs[5], y(level) + 5f, Anchor.CENTER, strong = true))
            val names = listOf("sleep", "check-in", "rest HR", "HRV", "training", "today")
            at(0.6f, 1.6f, *names.mapIndexed { i, n ->
                SketchMark.Label(n, xs[i], if (i % 2 == 0) 96f else 104f, Anchor.CENTER, if (i == 5) Ink.INK else Ink.GUIDE)
            }.toTypedArray())
        }
    }

    private fun hrvNoise(): Sketch = with(AcademySketches) {
        sketch("Heart-rate variability jumps around from night to night. One bad night barely moves the trend. Several low nights in a row do, and only that counts.") {
            val ys = listOf(48f, 44f, 52f, 46f, 50f, 42f, 72f, 47f, 45f, 51f, 43f, 49f, 46f, 50f, 48f, 55f, 58f, 62f, 60f, 64f, 63f)
            val xs = ys.indices.map { 16f + it * 6.5f }
            val trend = ys.indices.map { i ->
                val w = ys.subList(maxOf(0, i - 6), i + 1)
                w.sum() / w.size
            }
            at(0f, 1.4f, SketchMark.Line(axisX(96f, 14f, 150f, listOf(xs[0], xs[7], xs[14], xs[20])), Ink.GUIDE))
            at(0.3f, 1.4f, label("HRV, each night", 16f, 12f), label("3 weeks", 16f, 104f))
            stagger(0.8f, 0.5f, 0.16f, xs.indices.map { SketchMark.Dot(xs[it], ys[it], 1.4f, Ink.INK) })
            at(4.4f, 3f, SketchMark.Line(smooth(*xs.indices.flatMap { listOf(xs[it], trend[it]) }.toFloatArray()), Ink.ACCENT))
            at(4.6f, 1.4f, label("one bad night", xs[6], 82f, Anchor.CENTER))
            at(5.6f, 1.4f, label("your trend", 16f, 32f, strong = true))
            at(7f, 1f, SketchMark.Dot(xs.last(), trend.last(), 2.2f, Ink.ACCENT, halo = true))
            at(7.2f, 1.4f, label("a real drop", 150f, 78f, Anchor.END, strong = true))
        }
    }

    private fun volumeVsWeight(): Sketch = with(AcademySketches) {
        sketch("Through a block, sets come down as the weights go up. In the easy week at the end, both drop.") {
            fun x(w: Float) = 16f + w * 22.3f
            val sets = smooth(x(0f), 52f, x(2f), 40f, x(4f), 60f, x(5f), 72f, x(6f), 84f)
            val weight = smooth(x(0f), 78f, x(2f), 66f, x(4f), 48f, x(5f), 36f, x(6f), 74f)
            val phases = listOf(Triple(0f, 2f, "build"), Triple(2f, 4f, "heavy"), Triple(4f, 5f, "peak"), Triple(5f, 6f, "easy"))
            stagger(0f, 1f, 0.3f, phases.map { (a, b, _) ->
                SketchMark.Band(x(a) + 0.8f, 12f, x(b) - 0.8f, 16f, if (b == 6f) Ink.ACCENT else Ink.GUIDE, radius = 2f)
            })
            stagger(0.3f, 1.2f, 0.3f, phases.map { (a, b, t) -> SketchMark.Label(t, (x(a) + x(b)) / 2f, 23f, Anchor.CENTER) })
            at(1f, 1.4f, SketchMark.Line(axisX(94f, 16f, 150f, (0..6).map { x(it.toFloat()) }), Ink.GUIDE))
            at(1.2f, 1.4f, SketchMark.Band(x(5f), 30f, x(6f), 94f, Ink.ACCENT))
            at(1.6f, 4f, ink(sets, 1.1f))
            at(2f, 4f, SketchMark.Line(weight, Ink.GUIDE, weight = 1.6f, style = Stroked.DASHED))
            at(5.4f, 1.4f, label("sets", x(1.6f), 34f, Anchor.CENTER, strong = true), label("weight", x(4.2f), 40f, Anchor.END))
            at(6f, 1.4f, label("both drop", x(5.5f), 102f, Anchor.CENTER, strong = true), label("week 1", x(0.5f), 102f, Anchor.CENTER))
        }
    }

    private fun goalsInTurn(): Sketch = with(AcademySketches) {
        sketch("Chasing fat loss and strength at the same time gets both half done. Taking them in turn, with the other held steady, gets each one done well.") {
            at(0f, 1.2f, label("at once", 14f, 10f))
            at(0.4f, 1.4f, SketchMark.Band(14f, 18f, 150f, 26f, Ink.GUIDE, radius = 3f), SketchMark.Band(14f, 30f, 150f, 38f, Ink.GUIDE, radius = 3f))
            at(0.8f, 1.2f, label("fat loss", 17f, 22f), label("strength", 17f, 34f))
            at(1.6f, 1.4f, label("both half done", 150f, 47f, Anchor.END))
            at(2.6f, 1.2f, label("in turn", 14f, 62f, strong = true))
            at(3f, 1.6f, SketchMark.Band(14f, 70f, 80f, 80f, Ink.ACCENT, radius = 3f))
            at(3.4f, 1.2f, label("fat loss", 17f, 75f, strong = true))
            at(3.8f, 1.2f, SketchMark.Band(14f, 83f, 80f, 86f, Ink.GUIDE, radius = 1.5f))
            at(4.4f, 1.6f, SketchMark.Band(84f, 70f, 150f, 80f, Ink.ACCENT, radius = 3f))
            at(4.8f, 1.2f, label("strength", 87f, 75f, strong = true))
            at(5.2f, 1.4f, label("strength held", 16f, 94f))
            at(5.8f, 1.4f, label("each done well", 150f, 104f, Anchor.END, strong = true))
        }
    }

    // ── Cardio ───────────────────────────────────────────────────────────────

    private fun talkTest(): Sketch = with(AcademySketches) {
        sketch("How much you can say tells you your zone. Singing is zone 1, full sentences zone 2, short phrases zone 3, a few words zone 4, and nothing at all zone 5.") {
            val talk = listOf("can sing", "full sentences", "short phrases", "a few words", "no words")
            talk.forEachIndexed { i, t ->
                val y = 16f + i * 19f
                val end = 76f + (i + 1) * 11f
                at(i * 0.9f, 1.2f, SketchMark.Label(t, 14f, y, Anchor.START, if (i == 1) Ink.INK else Ink.GUIDE))
                at(i * 0.9f + 0.3f, 1.4f, SketchMark.Line(line(66f, y, end, y), if (i == 1) Ink.ACCENT else Ink.GUIDE, weight = if (i == 1) 2.2f else 4.4f))
                at(i * 0.9f + 0.8f, 1.2f, SketchMark.Label("zone ${i + 1}", end + 4f, y, Anchor.START, if (i == 1) Ink.INK else Ink.GUIDE))
            }
        }
    }

    private fun baseBuilding(): Sketch = with(AcademySketches) {
        sketch("Kept at the same easy heart rate, the distance you cover in 30 minutes creeps up week by week. That is your aerobic base improving.") {
            val tops = listOf(70f, 67f, 64f, 60f, 57f, 53f, 49f, 45f)
            val xs = tops.indices.map { 24f + it * 17f }
            at(0f, 1.4f, guide(line(14f, 88f, 150f, 88f)))
            at(0.3f, 1.4f, label("distance in 30 min", 14f, 10f), label("same heart rate", 14f, 20f, strong = true))
            stagger(1f, 1f, 0.45f, xs.indices.map { SketchMark.Column(xs[it], tops[it], 88f, 9f, if (it == 7) Ink.ACCENT else Ink.INK) })
            at(4.6f, 2f, dotted(line(*xs.indices.flatMap { listOf(xs[it], tops[it] - 4f) }.toFloatArray())))
            at(5.4f, 1.4f, label("week 1", xs[0], 96f, Anchor.CENTER), label("week 8", xs[7], 96f, Anchor.CENTER, strong = true))
        }
    }

    private fun interference(): Sketch = with(AcademySketches) {
        sketch("Easy cardio costs your lifting almost nothing. Hard sessions a day apart cost a little, cycling less than running. Hard intervals right before leg day cost the most.") {
            val rows = listOf(
                Triple("easy cardio, any day", 16f, Ink.INK),
                Triple("hard cycling, a day apart", 40f, Ink.INK),
                Triple("hard running, a day apart", 62f, Ink.INK),
                Triple("hard intervals, right before legs", 128f, Ink.ACCENT)
            )
            at(0f, 1.4f, label("cost to your lifting", 150f, 6f, Anchor.END))
            rows.forEachIndexed { i, (name, len, ink) ->
                val y = 26f + i * 24f
                at(0.4f + i * 1.1f, 1.2f, SketchMark.Label(name, 14f, y - 9f, Anchor.START, if (ink == Ink.ACCENT) Ink.INK else Ink.GUIDE))
                at(0.7f + i * 1.1f, 1.6f, SketchMark.Line(line(14f, y, 14f + len, y), ink, weight = if (ink == Ink.ACCENT) 3f else 4f))
            }
        }
    }
}
