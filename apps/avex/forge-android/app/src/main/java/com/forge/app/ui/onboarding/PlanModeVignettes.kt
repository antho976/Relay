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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.PI
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
 * video finishes decoding moves nothing. The one thing the videos have that this does not is their
 * vendored typeface; this sets type in the platform's own sans and mono, which the faces there stand in
 * for.
 *
 * Type is drawn at a size derived from that scale rather than from the type scale, because this is a
 * picture of an interface, not an interface: it must keep the video's proportions at any card width
 * and must not reflow at 200% font scale (the card's own label and description below still scale).
 *
 * **Change one side, change the other.** If a composition's copy, timings, counts or positions move,
 * these must move with them or the hand-off starts to jump.
 */

/** The Remotion canvas these coordinates are written on (`theme.ts`). */
private const val ART_W = 1128f
private const val DP = 4f

/** `theme.ts`: LOOP_FRAMES 150 @ 30fps, and the phase timeline. Everything below is in frames. */
private const val LOOP_MS = 5000
private const val FRAMES = 150f
private const val HOLD_END = 18f
private const val CLEAR_END = 32f

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

private fun bump(frame: Float, from: Float, length: Float): Float {
    if (frame <= from || frame >= from + length) return 0f
    return sin((frame - from) / length * PI.toFloat())
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
            onBg = scheme.onBackground,
            muted = scheme.onSurfaceVariant,
            outline = scheme.outline,
            tile = scheme.surfaceVariant,
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
    val onBg: Color,
    val muted: Color,
    val outline: Color,
    val tile: Color,
    val measurer: TextMeasurer
)

/** Canvas px → this Canvas's px. */
private fun DrawScope.px(v: Float): Float = v * size.width / ART_W

/**
 * One line of type in a line box of [lineH] at ([x], [top]), centred vertically in it as CSS does.
 * [centreIn] > 0 centres it horizontally across that width instead of starting at [x].
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
            fontFamily = if (mono) FontFamily.Monospace else FontFamily.SansSerif,
            fontWeight = if (mono) FontWeight.Normal else FontWeight.Medium,
            fontSize = px(sizePx).toSp(),
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
private const val DAY_GAP = 7 * DP
private const val DAY_SIZE = 10 * DP
private const val BAR_RADIUS = 4 * DP
private const val SHUFFLE_IN = 36f
private const val SHUFFLE_STAGGER = 2f
private const val LOCK_IN = 58f
private const val LOCK_STAGGER = 7f
private const val LOCK_LEN = 18f
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

        roundRect(ink.outline, x, TRACK_TOP, BAR_W, TRACK_H, BAR_RADIUS)
        if (height > 0.001f) {
            val h = TRACK_H * height
            roundRect(
                lerp(ink.muted, ink.accent, decided),
                x, TRACK_TOP + TRACK_H - h, BAR_W, h, BAR_RADIUS,
                // Considered in muted at the 0.35 rung; decided in full accent.
                alpha = 0.35f + 0.65f * decided
            )
        }
        type(
            ink, WEEK_DAYS[i], x, TRACK_TOP + TRACK_H + DAY_GAP, DAY_SIZE, DAY_SIZE,
            lerp(ink.muted, ink.onBg, decided), mono = true, centreIn = BAR_W
        )
    }
}

// ---- Custom.tsx: training days dragged by hand into an empty week. -------------------------------

private val PLACED = listOf(0 to "PUSH", 2 to "PULL", 4 to "LEGS")
private const val SLOT_W = 34 * DP
private const val SLOT_H = 40 * DP
private const val SLOT_PITCH = 38 * DP
private const val SLOT_TOP = 8.5f * DP
private const val SLOT_RADIUS = 8 * DP
private const val DASH = 0.75f * DP
private const val SLOT_DAY_GAP = 5 * DP
private const val CHIP_SIZE = 9 * DP
private const val TOUCH_R = 9 * DP
private const val SLOT_LEFT = (ART_W - (6 * SLOT_PITCH + SLOT_W)) / 2f
private const val ENTER_X = ART_W + 6 * DP
private val DRAGS = listOf(42f, 68f, 94f)
private const val TRAVEL = 20f
private const val DROP_LEN = 14f

/** Drag [i] at [frame]: its x, and its lift (1 carried, 0 settled in the slot). */
private fun dragX(i: Int, frame: Float): Float {
    val t = smoothstep(frame, DRAGS[i], DRAGS[i] + TRAVEL)
    return ENTER_X + (SLOT_LEFT + PLACED[i].first * SLOT_PITCH - ENTER_X) * t
}

private fun dragLift(i: Int, frame: Float): Float =
    if (frame < DRAGS[i] + TRAVEL) 1f else 1f - settle(frame, DRAGS[i] + TRAVEL, DROP_LEN)

private fun DrawScope.drawCustom(frame: Float, ink: Ink) = clipRect {
    val held = heldOut(frame)
    val dash = PathEffect.dashPathEffect(floatArrayOf(px(9f), px(6f)))
    WEEK_DAYS.forEachIndexed { i, day ->
        val p = PLACED.indexOfFirst { it.first == i }
        val filled = if (p < 0) 0f else maxOf(held, smoothstep(frame, DRAGS[p] + TRAVEL, DRAGS[p] + TRAVEL + 4f))
        val x = SLOT_LEFT + i * SLOT_PITCH
        drawRoundRect(
            ink.muted.copy(alpha = 0.35f),
            Offset(px(x + DASH / 2f), px(SLOT_TOP + DASH / 2f)),
            Size(px(SLOT_W - DASH), px(SLOT_H - DASH)),
            CornerRadius(px(SLOT_RADIUS - DASH / 2f)),
            style = Stroke(px(DASH), pathEffect = dash)
        )
        type(
            ink, day, x, SLOT_TOP + SLOT_H + SLOT_DAY_GAP, DAY_SIZE, DAY_SIZE,
            lerp(ink.muted, ink.onBg, filled), mono = true, centreIn = SLOT_W
        )
    }
    PLACED.forEachIndexed { i, (slot, label) ->
        when {
            held > 0f -> chip(ink, label, SLOT_LEFT + slot * SLOT_PITCH, 0f, held)
            frame >= DRAGS[i] -> chip(ink, label, dragX(i, frame), dragLift(i, frame), 1f)
        }
    }
    // The finger, riding each tile until it is dropped.
    PLACED.indices.forEach { i ->
        val start = DRAGS[i]
        val a = minOf(smoothstep(frame, start, start + 4f), 1f - smoothstep(frame, start + TRAVEL + 1f, start + TRAVEL + 7f))
        if (a <= 0f) return@forEach
        val r = TOUCH_R * (1f - 0.15f * bump(frame, start + TRAVEL - 2f, 8f))
        drawCircle(
            ink.onBg.copy(alpha = 0.35f * a),
            radius = px(r),
            center = Offset(px(dragX(i, frame) + SLOT_W / 2f), px(SLOT_TOP + SLOT_H * 0.62f))
        )
    }
}

/** A training day: an accent tile with its split. [lift] 1 = carried (bigger, tilted), 0 = placed. */
private fun DrawScope.chip(ink: Ink, label: String, x: Float, lift: Float, alpha: Float) {
    val pivot = Offset(px(x + SLOT_W / 2f), px(SLOT_TOP + SLOT_H / 2f))
    withTransform({
        rotate(-5f * lift, pivot)
        scale(1f + 0.1f * lift, 1f + 0.1f * lift, pivot)
    }) {
        roundRect(ink.accent, x, SLOT_TOP, SLOT_W, SLOT_H, SLOT_RADIUS, alpha = alpha)
        type(ink, label, x, SLOT_TOP, CHIP_SIZE, SLOT_H, ink.onBg.copy(alpha = alpha.coerceIn(0f, 1f)), mono = true, centreIn = SLOT_W)
    }
}

// ---- Freestyle.tsx: a timeline up to NOW, a check popping on whenever something is logged. -------

/** Name, day, x on the canvas, and the frame it is logged on — `Freestyle.tsx`'s ENTRIES. */
private data class Entry(val name: String, val day: String, val x: Float, val at: Float)

private val ENTRIES = listOf(
    Entry("Squat", "MON", 22 * DP, 50f),
    Entry("Run", "THU", 72 * DP, 60f),
    Entry("Pull-ups", "FRI", 122 * DP, 66f),
    Entry("Bench", "SUN", 186 * DP, 94f),
    Entry("Row", "WED", 234 * DP, 106f)
)
private const val ENTRY_NAME_TOP = 13 * DP
private const val ENTRY_NAME_LINE = 12 * DP
private const val ENTRY_NAME_SIZE = 10 * DP
private const val LINE_Y = 37 * DP
private const val NODE_R = 8 * DP
private const val ENTRY_DAY_TOP = 49 * DP
private const val ENTRY_DAY_SIZE = 9 * DP
private const val LINE_W = 0.75f * DP
private const val HEAD_R = 3.5f * DP
private const val START_X = 4 * DP
private const val END_X = 268 * DP
private const val LOG_IN = 38f
private const val LOG_DONE = 118f
private const val REWIND_FROM = HOLD_END + 2f

/** Where NOW is: held at the end, rewinding to the start, then stepping through each logged moment. */
private fun headX(frame: Float): Float {
    if (frame < LOG_IN) return END_X + (START_X - END_X) * smoothstep(frame, REWIND_FROM, LOG_IN)
    val keys = listOf(LOG_IN to START_X) + ENTRIES.map { it.at to it.x } + (LOG_DONE to END_X)
    for (k in 1 until keys.size) {
        if (frame <= keys[k].first) {
            val (f0, x0) = keys[k - 1]
            val (f1, x1) = keys[k]
            return x0 + (x1 - x0) * smoothstep(frame, f0, f1)
        }
    }
    return END_X
}

private fun DrawScope.drawFreestyle(frame: Float, ink: Ink) {
    val held = heldOut(frame)
    val head = headX(frame)
    val nudge = ENTRIES.fold(0f) { m, e -> maxOf(m, bump(frame, e.at - 4f, 10f)) }
    // NOW's own label steps aside while NOW sits on a check, whose day label is in the same place.
    val clear = ENTRIES.fold(1f) { m, e ->
        if (held > 0f || frame >= e.at) minOf(m, smoothstep(abs(head - e.x), 16 * DP, 28 * DP)) else m
    }

    if (head > START_X) {
        roundRect(ink.muted, START_X, LINE_Y - LINE_W / 2f, head - START_X, LINE_W, LINE_W / 2f, alpha = 0.35f)
    }
    for (e in ENTRIES) {
        val pop = if (held > 0f) 1f else settle(frame, e.at, 14f)
        val alpha = (if (held > 0f) held else minOf(1f, pop * 1.5f)).coerceIn(0f, 1f)
        if (alpha <= 0f) continue
        val centre = Offset(px(e.x), px(LINE_Y))
        val ring = if (held > 0f) 0f else smoothstep(frame, e.at, e.at + 16f)
        if (ring > 0f && ring < 1f) {
            drawCircle(
                ink.accent.copy(alpha = 0.6f * (1f - ring)),
                radius = px(NODE_R * (1f + ring) - LINE_W / 2f),
                center = centre,
                style = Stroke(px(LINE_W))
            )
        }
        // The node and its check, on the video's -8..8 box scaled to NODE_R and the pop.
        val u = NODE_R / 8f * pop
        drawCircle(ink.accent, radius = px(8f * u), center = centre, alpha = alpha)
        val check = Path().apply {
            moveTo(centre.x + px(-3.6f * u), centre.y + px(0.2f * u))
            lineTo(centre.x + px(-1.1f * u), centre.y + px(2.7f * u))
            lineTo(centre.x + px(3.8f * u), centre.y + px(-2.4f * u))
        }
        drawPath(check, ink.onBg, alpha = alpha, style = Stroke(px(1.75f * u), cap = StrokeCap.Round, join = StrokeJoin.Round))
        type(
            ink, e.name, e.x - 40 * DP, ENTRY_NAME_TOP, ENTRY_NAME_SIZE, ENTRY_NAME_LINE,
            ink.onBg.copy(alpha = alpha), mono = false, centreIn = 80 * DP
        )
        type(
            ink, e.day, e.x - 40 * DP, ENTRY_DAY_TOP, ENTRY_DAY_SIZE, ENTRY_DAY_SIZE,
            ink.muted.copy(alpha = alpha), mono = true, centreIn = 80 * DP
        )
    }
    drawCircle(ink.accent, radius = px(HEAD_R * (1f + 0.35f * nudge)), center = Offset(px(head), px(LINE_Y)))
    if (clear > 0f) {
        type(
            ink, "NOW", head - 40 * DP, ENTRY_DAY_TOP, ENTRY_DAY_SIZE, ENTRY_DAY_SIZE,
            ink.muted.copy(alpha = clear), mono = true, centreIn = 80 * DP
        )
    }
}
