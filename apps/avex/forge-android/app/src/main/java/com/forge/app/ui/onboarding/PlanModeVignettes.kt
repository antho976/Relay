package com.forge.app.ui.onboarding

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import com.forge.app.ui.theme.AvexFonts
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.sin

/**
 * The live twin of the three plan-mode vignette videos — what you see on API < 28, if a WebP fails to
 * decode, for the moment before one finishes decoding, and (frozen on its last frame) when the system
 * reduce-motion preference is on. Reduce-motion is the one that matters: those users never see the
 * video at all, so this IS the illustration for them, and it has to say the same thing.
 *
 * It is a deliberate transcription: every number below is the SAME number as in the matching
 * `remotion-vignettes/src` composition, read on that project's 1128×288 canvas (4px = 1dp) and scaled
 * to whatever width this one gets. Marks land in the same places in both, so the hand-off when a
 * video finishes decoding moves nothing. Type is set in the platform's own sans and mono, which are
 * the faces the videos vendor (Roboto Medium, Droid Sans Mono), so even the letters match.
 *
 * Type is drawn at a size derived from that scale rather than from the type scale, because this is a
 * picture of an interface, not an interface: it must keep the video's proportions at any card width
 * and must not reflow at 200% font scale (the card's own label and description below still scale).
 * The sizes it derives are the type scale's own at the card's natural width: every mono label is
 * `labelSmall` (10sp, tracked 0.5sp).
 *
 * **Change one side, change the other.** If a composition's copy, timings, counts or positions move,
 * these must move with them or the hand-off starts to jump.
 */

/** The Remotion canvas these coordinates are written on (`theme.ts`). */
private const val ART_W = 1128f
private const val ART_H = 288f
private const val DP = 4f

/** `theme.ts`: LOOP_FRAMES 150 @ 30fps, and the phase timeline. Everything below is in frames. */
private const val LOOP_MS = 5000
private const val FRAMES = 150f
private const val HOLD_END = 18f
private const val CLEAR_END = 32f
private const val BUILD_IN = 38f
private const val BUILD_END = 124f

/** `theme.ts`: labelSmall on the canvas, and Home's day cell (`CellShape`, 12dp on 34dp). */
private const val LABEL_SIZE = 10 * DP
private const val LABEL_LINE = 12 * DP
private const val LABEL_TRACKING = 0.5f * DP
private const val CELL = 34 * DP
private const val CELL_RADIUS = 12 * DP

/** `theme.ts` RUNG: the §5 ladder values these marks use. */
private const val RUNG_TRACK = 0.25f
private const val RUNG_BAR = 0.7f

/**
 * 0→1 across one loop, run [PLAN_LOOPS] times and then held at the end — the same contract the videos
 * get from `repeatCount`. [replays] restarts it, so tapping a card replays the twin exactly as it
 * replays a video. A static 1f when the user has removed animations: the settled final state.
 */
@Composable
private fun vignettePhase(replays: Int): Float {
    if (ForgeMotion.durationScale <= 0f) return 1f
    val runs = remember { Animatable(0f) }
    LaunchedEffect(replays) {
        runs.snapTo(0f)
        runs.animateTo(
            targetValue = PLAN_LOOPS.toFloat(),
            animationSpec = tween(LOOP_MS * PLAN_LOOPS, easing = LinearEasing)
        )
    }
    // Phase 0 and phase 1 are the same held picture, so landing on either at the end is the same frame.
    return if (runs.value >= PLAN_LOOPS) 1f else runs.value % 1f
}

// ---- theme.ts's curves, one for one. -----------------------------------------------------------

private fun smoothstep(v: Float, from: Float, to: Float): Float {
    val t = ((v - from) / (to - from)).coerceIn(0f, 1f)
    return t * t * (3f - 2f * t)
}

private fun heldOut(frame: Float): Float = 1f - smoothstep(frame, HOLD_END, CLEAR_END)

private fun settle(frame: Float, from: Float, length: Float): Float {
    if (frame <= from) return 0f
    val t = ((frame - from) / length).coerceAtMost(1f)
    return if (t >= 1f) 1f else 1f - exp(-6f * t) * cos(4.2f * t)
}

private fun hash01(a: Int, b: Int): Float {
    val x = sin(a * 127.1 + b * 311.7) * 43758.5453
    return (x - floor(x)).toFloat()
}

/** Sized by the caller — [PlanModeMedia] gives video and twin the same aspect slot. */
@Composable
internal fun PlanModeVignette(mode: String, modifier: Modifier = Modifier, replays: Int = 0) {
    val phase = vignettePhase(replays)
    val scheme = MaterialTheme.colorScheme
    val measurer = rememberTextMeasurer()
    Canvas(modifier = modifier) {
        val ink = Ink(
            accent = scheme.primary,
            onAccent = scheme.onPrimary,
            onBg = scheme.onBackground,
            bg = scheme.background,
            muted = scheme.onSurfaceVariant,
            outline = scheme.outline,
            card = scheme.surfaceContainerHigh,
            measurer = measurer
        )
        val frame = phase * FRAMES
        when (mode) {
            PLAN_GENERATED -> drawGenerated(frame, ink)
            PLAN_CUSTOM -> drawCustom(frame, ink)
            else -> drawFreestyle(frame, ink)
        }
    }
}

/** What every mark on these cards is drawn with: `theme.ts`'s tokens, read live from the scheme. */
private class Ink(
    val accent: Color,
    /** On an accent fill. `theme.ts` uses BG for it: what the theme measures for the default Red. */
    val onAccent: Color,
    val onBg: Color,
    val bg: Color,
    val muted: Color,
    val outline: Color,
    /** The card's row fill (`theme.ts` CARD): what generated's opaque track and shuffle blend onto. */
    val card: Color,
    val measurer: TextMeasurer
)

/** Canvas px → this Canvas's px. */
private fun DrawScope.px(v: Float): Float = v * size.width / ART_W

/**
 * One line of type in a line box of [lineH] at ([x], [top]), centred vertically in it as CSS does.
 * [centreIn] > 0 centres it horizontally across that width instead of starting at [x]. Mono is
 * `labelSmall`'s tracking; the videos balance CSS's trailing spacing so both centre the same way.
 */
private fun DrawScope.type(
    ink: Ink,
    text: String,
    x: Float,
    top: Float,
    sizePx: Float,
    lineH: Float,
    color: Color,
    mono: Boolean,
    centreIn: Float = 0f
) {
    val layout = ink.measurer.measure(
        text,
        TextStyle(
            fontFamily = if (mono) AvexFonts.Monospace else AvexFonts.SansSerif,
            fontWeight = if (mono) FontWeight.Normal else FontWeight.Medium,
            fontSize = px(sizePx).toSp(),
            letterSpacing = px(if (mono) LABEL_TRACKING else 0f).toSp(),
            color = color
        )
    )
    val dx = if (centreIn > 0f) (px(centreIn) - layout.size.width) / 2f else 0f
    drawText(layout, topLeft = Offset(px(x) + dx, px(top) + (px(lineH) - layout.size.height) / 2f))
}

private fun DrawScope.roundRect(color: Color, x: Float, y: Float, w: Float, h: Float, r: Float, alpha: Float = 1f) {
    drawRoundRect(color, Offset(px(x), px(y)), Size(px(w), px(h)), CornerRadius(px(r)), alpha = alpha.coerceIn(0f, 1f))
}

// ---- Generated.tsx: a Mon–Sun bar week that shuffles, then locks day by day. --------------------

private val WEEK_DAYS = listOf("M", "T", "W", "T", "F", "S", "S")
private val WEEK_SETS = listOf(22, 18, 0, 24, 16, 0, 0)
private const val BAR_W = 16 * DP
private const val BAR_PITCH = 36 * DP
private const val TRACK_TOP = 5 * DP
private const val TRACK_H = 45 * DP
private const val DAY_GAP = 6 * DP
private const val BAR_RADIUS = 4 * DP
private const val SHUFFLE_IN = BUILD_IN
private const val SHUFFLE_STAGGER = 2f
private const val LOCK_STAGGER = 7f
private const val LOCK_LEN = 18f
private const val LOCK_IN = BUILD_END - 6 * LOCK_STAGGER - LOCK_LEN
private const val SHUFFLE_STEP = 3

private fun shuffleAt(col: Int, frame: Float): Float {
    val k = floor(frame / SHUFFLE_STEP).toInt()
    val a = hash01(col, k)
    val b = hash01(col, k + 1)
    return 0.2f + 0.8f * (a + (b - a) * smoothstep(frame - k * SHUFFLE_STEP, 0f, SHUFFLE_STEP.toFloat()))
}

private fun DrawScope.drawGenerated(frame: Float, ink: Ink) {
    val held = heldOut(frame)
    val peak = WEEK_SETS.max().toFloat()
    val left = (ART_W - (6 * BAR_PITCH + BAR_W)) / 2f
    // WeekBarRail's colours, blended onto the card and drawn opaque as the video draws them: the
    // track is outline at its 0.25 rung, a bar not yet decided is muted at the 0.7 rung.
    val track = lerp(ink.card, ink.outline, RUNG_TRACK)
    val considered = lerp(ink.card, ink.muted, RUNG_BAR)
    WEEK_SETS.forEachIndexed { i, sets ->
        val target = sets / peak
        val lockAt = LOCK_IN + i * LOCK_STAGGER
        val start = SHUFFLE_IN + i * SHUFFLE_STAGGER
        val trying = smoothstep(frame, start, start + 8f)
        val from = shuffleAt(i, lockAt)
        val built = if (frame < lockAt) trying * shuffleAt(i, frame)
        else from + (target - from) * settle(frame, lockAt, LOCK_LEN)
        val height = (held * target + built).coerceIn(0f, 1.06f)
        val decided = if (sets > 0) maxOf(held, smoothstep(frame, lockAt, lockAt + 6f)) else 0f
        val x = left + i * BAR_PITCH

        roundRect(track, x, TRACK_TOP, BAR_W, TRACK_H, BAR_RADIUS)
        if (height > 0.001f) {
            val h = TRACK_H * height
            roundRect(lerp(considered, ink.accent, decided), x, TRACK_TOP + TRACK_H - h, BAR_W, h, BAR_RADIUS)
        }
        // The letter lights in one step, at the bar's half-way to accent.
        type(
            ink, WEEK_DAYS[i], x, TRACK_TOP + TRACK_H + DAY_GAP, LABEL_SIZE, LABEL_LINE,
            if (decided >= 0.5f) ink.onBg else ink.muted, mono = true, centreIn = BAR_W
        )
    }
}

// ---- Custom.tsx: training days dragged by hand into an empty week. -------------------------------

private val PLACED = listOf(0 to "PUSH", 2 to "PULL", 4 to "LEGS")
private const val SLOT_PITCH = 38 * DP
private const val SLOT_DAY_GAP = 6 * DP
private const val SLOT_TOP = (ART_H - (CELL + SLOT_DAY_GAP + LABEL_LINE)) / 2f
private const val DASH_W = 1.5f * DP
private const val DASH_INSET = 1 * DP
private const val DASH_ON = 4 * DP
private const val DASH_OFF = 3 * DP
private const val LIFT = 0.06f
private const val SLOT_LEFT = (ART_W - (6 * SLOT_PITCH + CELL)) / 2f
private const val ENTER_X = ART_W + 6 * DP
private const val TRAVEL = 20f
private const val DROP_LEN = 14f
private const val DRAG_SPACING = (BUILD_END - BUILD_IN - TRAVEL - DROP_LEN) / 2f
private val DRAGS = listOf(0, 1, 2).map { BUILD_IN + it * DRAG_SPACING }
private const val SETTLE_COLOUR = 8f

private fun dragX(i: Int, frame: Float): Float {
    val t = smoothstep(frame, DRAGS[i], DRAGS[i] + TRAVEL)
    return ENTER_X + (SLOT_LEFT + PLACED[i].first * SLOT_PITCH - ENTER_X) * t
}

/** 1 carried, 0 set down in its cell. */
private fun dragLift(i: Int, frame: Float): Float =
    if (frame < DRAGS[i] + TRAVEL) 1f else 1f - settle(frame, DRAGS[i] + TRAVEL, DROP_LEN)

/** 0 in hand (accent), 1 placed in the week (onBg). */
private fun dragPlaced(i: Int, frame: Float): Float =
    smoothstep(frame, DRAGS[i] + TRAVEL, DRAGS[i] + TRAVEL + SETTLE_COLOUR)

private fun DrawScope.drawCustom(frame: Float, ink: Ink) = clipRect {
    val held = heldOut(frame)
    // Home's dashed day cell: muted 0.6, 1.5dp, 4dp on 3dp off, inset 1dp.
    val dash = PathEffect.dashPathEffect(floatArrayOf(px(DASH_ON), px(DASH_OFF)))
    WEEK_DAYS.forEachIndexed { i, day ->
        val p = PLACED.indexOfFirst { it.first == i }
        val lit = p >= 0 && (held >= 0.5f || frame >= DRAGS[p] + TRAVEL + 2f)
        val x = SLOT_LEFT + i * SLOT_PITCH
        drawRoundRect(
            ink.muted.copy(alpha = 0.6f),
            Offset(px(x + DASH_INSET), px(SLOT_TOP + DASH_INSET)),
            Size(px(CELL - 2 * DASH_INSET), px(CELL - 2 * DASH_INSET)),
            CornerRadius(px(CELL_RADIUS)),
            style = Stroke(px(DASH_W), pathEffect = dash)
        )
        type(
            ink, day, x, SLOT_TOP + CELL + SLOT_DAY_GAP, LABEL_SIZE, LABEL_LINE,
            if (lit) ink.onBg else ink.muted, mono = true, centreIn = CELL
        )
    }
    PLACED.forEachIndexed { i, (slot, label) ->
        when {
            held > 0f -> tile(ink, label, SLOT_LEFT + slot * SLOT_PITCH, 0f, 1f, held)
            frame >= DRAGS[i] -> tile(ink, label, dragX(i, frame), dragLift(i, frame), dragPlaced(i, frame), 1f)
        }
    }
}

/**
 * A training day: accent while carried (WeekBarRail's dragged bar, lifted to 1.06), onBg once placed
 * (Home's filled day). Its label is the ink for whichever fill it is on.
 */
private fun DrawScope.tile(ink: Ink, label: String, x: Float, lift: Float, placed: Float, alpha: Float) {
    val pivot = Offset(px(x + CELL / 2f), px(SLOT_TOP + CELL / 2f))
    val scale = 1f + LIFT * lift
    withTransform({ scale(scale, scale, pivot) }) {
        roundRect(lerp(ink.accent, ink.onBg, placed), x, SLOT_TOP, CELL, CELL, CELL_RADIUS, alpha = alpha)
        type(
            ink, label, x, SLOT_TOP, LABEL_SIZE, CELL,
            lerp(ink.onAccent, ink.bg, placed).copy(alpha = alpha.coerceIn(0f, 1f)), mono = true, centreIn = CELL
        )
    }
}

// ---- Freestyle.tsx: a week of time up to NOW, a checked cell popping on whenever something is logged.

private const val DAY0_X = 22 * DP
private const val DAY_PITCH = 35 * DP
private const val START_X = 4 * DP
private const val END_X = DAY0_X + 7 * DAY_PITCH
private const val REWIND_FROM = HOLD_END + 2f

/** The frame NOW crosses [x]: one even pace from BUILD_IN at START_X to BUILD_END at END_X. */
private fun atX(x: Float): Float = BUILD_IN + (x - START_X) / (END_X - START_X) * (BUILD_END - BUILD_IN)

/** Name, day, x on the canvas, and the frame it is logged on — `Freestyle.tsx`'s ENTRIES. */
private data class Entry(val name: String, val day: String, val x: Float, val at: Float)

private fun entry(name: String, day: String, d: Int): Entry {
    val x = DAY0_X + d * DAY_PITCH
    return Entry(name, day, x, atX(x))
}

private val ENTRIES = listOf(
    entry("Squat", "MON", 0),
    entry("Run", "TUE", 1),
    entry("Pull-ups", "THU", 3),
    entry("Bench", "SAT", 5),
    entry("Row", "SUN", 6)
)
private const val ENTRY_NAME_SIZE = 10 * DP
private const val ENTRY_NAME_TOP = 13 * DP
private const val ENTRY_NAME_LINE = 12 * DP
private const val LINE_Y = 37 * DP
private const val NODE = 16 * DP
private const val NODE_RADIUS = NODE * CELL_RADIUS / CELL
private const val RING_W = 2 * DP
private const val ENTRY_DAY_TOP = 49 * DP
private const val LINE_W = 1 * DP
private const val LINE_GAP = 3 * DP
private const val LABEL_HALF = 11 * DP
private const val POP_LEN = 14f

/** Where NOW is: held at the end, rewinding to the start, then stepping through each logged moment. */
private fun headX(frame: Float): Float {
    if (frame < BUILD_IN) return END_X + (START_X - END_X) * smoothstep(frame, REWIND_FROM, BUILD_IN)
    val keys = listOf(BUILD_IN to START_X) + ENTRIES.map { it.at to it.x } + (BUILD_END to END_X)
    for (k in 1 until keys.size) {
        if (frame <= keys[k].first) {
            val (f0, x0) = keys[k - 1]
            val (f1, x1) = keys[k]
            return x0 + (x1 - x0) * smoothstep(frame, f0, f1)
        }
    }
    return END_X
}

private fun popOf(e: Entry, frame: Float, held: Float): Float = if (held > 0f) 1f else settle(frame, e.at, POP_LEN)

/** The spine's visible runs: START_X up to NOW's ring, less a gap around every entry on it. */
private fun spine(frame: Float, held: Float, head: Float): List<Pair<Float, Float>> {
    val end = head - NODE / 2f - LINE_GAP
    val holes = ENTRIES.map { e ->
        val r = (NODE / 2f + LINE_GAP) * popOf(e, frame, held).coerceIn(0f, 1f)
        (e.x - r) to (e.x + r)
    }.filter { (a, b) -> b > a }
    val runs = mutableListOf<Pair<Float, Float>>()
    var from = START_X
    for ((a, b) in holes) {
        if (a > from && from < end) runs += from to minOf(a, end)
        from = maxOf(from, b)
    }
    if (end > from) runs += from to end
    return runs
}

private fun DrawScope.drawFreestyle(frame: Float, ink: Ink) {
    val held = heldOut(frame)
    val head = headX(frame)
    // NOW's own label steps aside while NOW sits on an entry, whose day label is in the same place.
    val clear = ENTRIES.fold(1f) { m, e ->
        if (held > 0f || frame >= e.at) minOf(m, smoothstep(abs(head - e.x), 20 * DP, 28 * DP)) else m
    }

    // Coach's ledger spine: outline 0.35, 1dp, drawn up to NOW.
    for ((a, b) in spine(frame, held, head)) {
        drawRect(
            ink.outline.copy(alpha = 0.35f),
            Offset(px(a), px(LINE_Y - LINE_W / 2f)),
            Size(px(b - a), px(LINE_W))
        )
    }
    for (e in ENTRIES) {
        val pop = popOf(e, frame, held)
        val alpha = (if (held > 0f) held else minOf(1f, pop * 1.5f)).coerceIn(0f, 1f)
        if (alpha <= 0f) continue
        val centre = Offset(px(e.x), px(LINE_Y))
        // Home's trained day: an onBg cell with its check in bg ink, on the video's -8..8 box scaled
        // to NODE and the pop.
        val u = NODE / 16f * pop
        drawRoundRect(
            ink.onBg,
            Offset(centre.x - px(8f * u), centre.y - px(8f * u)),
            Size(px(16f * u), px(16f * u)),
            CornerRadius(px(NODE_RADIUS * pop)),
            alpha = alpha
        )
        val check = Path().apply {
            moveTo(centre.x + px(-3.6f * u), centre.y + px(0.2f * u))
            lineTo(centre.x + px(-1.1f * u), centre.y + px(2.7f * u))
            lineTo(centre.x + px(3.8f * u), centre.y + px(-2.4f * u))
        }
        drawPath(check, ink.bg, alpha = alpha, style = Stroke(px(1.75f * u), cap = StrokeCap.Round, join = StrokeJoin.Round))
        type(
            ink, e.name, e.x - 40 * DP, ENTRY_NAME_TOP, ENTRY_NAME_SIZE, ENTRY_NAME_LINE,
            ink.onBg.copy(alpha = alpha), mono = false, centreIn = 80 * DP
        )
        type(
            ink, e.day, e.x - 40 * DP, ENTRY_DAY_TOP, LABEL_SIZE, LABEL_LINE,
            ink.muted.copy(alpha = alpha), mono = true, centreIn = 80 * DP
        )
    }
    // NOW: Home's today ring, a 2dp accent border inside a cell-shaped box. Never a fill.
    drawRoundRect(
        ink.accent,
        Offset(px(head - NODE / 2f + RING_W / 2f), px(LINE_Y - NODE / 2f + RING_W / 2f)),
        Size(px(NODE - RING_W), px(NODE - RING_W)),
        CornerRadius(px(NODE_RADIUS - RING_W / 2f)),
        style = Stroke(px(RING_W))
    )
    if (clear > 0f) {
        val labelX = head.coerceIn(LABEL_HALF, ART_W - LABEL_HALF)
        type(
            ink, "NOW", labelX - 40 * DP, ENTRY_DAY_TOP, LABEL_SIZE, LABEL_LINE,
            ink.onBg.copy(alpha = clear), mono = true, centreIn = 80 * DP
        )
    }
}
