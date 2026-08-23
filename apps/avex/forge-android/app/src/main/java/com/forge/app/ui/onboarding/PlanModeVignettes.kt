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
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.rememberTextMeasurer
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.abs
import kotlin.math.cos

/**
 * The live twin of the three plan-mode vignette videos — what you see on API < 28, if a WebP fails to
 * decode, for the moment before one finishes decoding, and (frozen on its last frame) when the system
 * reduce-motion preference is on. Reduce-motion is the one that matters: those users never see the
 * video at all, so this IS the illustration for them, and it has to say the same thing.
 *
 * It is a deliberate transcription: every number below is the SAME number as in the matching
 * `remotion-vignettes/src` composition, read on that project's 1128×288 canvas and scaled to whatever
 * width this one gets. Marks land in the same places in both, so the hand-off when a video finishes
 * decoding moves nothing.
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
private const val ART_H = 288f

/** `theme.ts`: LOOP_FRAMES 150 @ 30fps, and the phase timeline, as fractions of one loop. */
private const val LOOP_MS = 5000
private const val FRAMES = 150f
private const val HOLD_END = 18f / FRAMES
private const val CLEAR_END = 32f / FRAMES

/** `Marks.tsx`. */
private const val SET_W = 38f
private const val SET_H = 14f
private const val SET_GAP = 10f
private const val LABEL_SIZE = 40f
private const val LABEL_TRACK = 7f

private fun setsWidth(sets: Int): Float = sets * SET_W + (sets - 1) * SET_GAP

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
    // Phase 0 and phase 1 are the same held plan, so landing on either at the end is the same picture.
    return if (runs.value >= PLAN_LOOPS) 1f else runs.value % 1f
}

/** 0 before [from], smoothstepped 0→1 across [from]..[to], 1 after. `theme.ts`'s `smoothstep`. */
private fun smoothstep(v: Float, from: Float, to: Float): Float {
    val t = ((v - from) / (to - from)).coerceIn(0f, 1f)
    return t * t * (3f - 2f * t)
}

/** 1 while the opening plan is held, 1→0 as it clears, 0 thereafter. */
private fun heldOut(phase: Float): Float = 1f - smoothstep(phase, HOLD_END, CLEAR_END)

/** Sized by the caller — [PlanModeMedia] gives video and twin the same aspect slot. */
@Composable
internal fun PlanModeVignette(mode: String, modifier: Modifier = Modifier, replays: Int = 0) {
    val phase = vignettePhase(replays)
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = rememberTextMeasurer()
    Canvas(modifier = modifier) {
        val ink = Ink(accent, muted, measurer)
        when (mode) {
            PLAN_GENERATED -> drawGenerated(phase, ink)
            PLAN_CUSTOM -> drawCustom(phase, ink)
            else -> drawFreestyle(phase, ink)
        }
    }
}

/** What every mark on these cards is drawn with. */
private class Ink(val accent: Color, val muted: Color, val measurer: TextMeasurer)

/** `Marks.tsx`'s MonoLabel, centred in the same [LABEL_SIZE]-tall row box the video gives it. */
private fun DrawScope.monoLabel(ink: Ink, text: String, x: Float, y: Float, alpha: Float) {
    if (alpha <= 0f) return
    val s = size.width / ART_W
    val layout = ink.measurer.measure(
        text,
        TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = (LABEL_SIZE * s).toSp(),
            letterSpacing = (LABEL_TRACK * s).toSp(),
            color = ink.muted
        )
    )
    drawText(
        layout,
        topLeft = Offset(x * s, y * s + (LABEL_SIZE * s - layout.size.height) / 2f),
        alpha = alpha.coerceIn(0f, 1f)
    )
}

/** `Marks.tsx`'s SetRow: `sets` blocks in a row, each at its own alpha. */
private fun DrawScope.setRow(ink: Ink, x: Float, y: Float, sets: Int, alphaAt: (Int) -> Float) {
    val s = size.width / ART_W
    for (i in 0 until sets) {
        val alpha = alphaAt(i)
        if (alpha <= 0f) continue
        drawRoundRect(
            color = ink.accent.copy(alpha = alpha.coerceIn(0f, 1f)),
            topLeft = Offset((x + i * (SET_W + SET_GAP)) * s, y * s),
            size = Size(SET_W * s, SET_H * s),
            cornerRadius = CornerRadius(SET_H * s / 2f)
        )
    }
}

/** Vertical offset that centres a row of blocks against its label. */
private const val BLOCK_DY = (LABEL_SIZE - SET_H) / 2f

// ---- Generated.tsx: MON PUSH ▪▪▪▪▪ — the week arrives, then the exercises tally across it. -------

private val WEEK = listOf(Triple("MON", "PUSH", 5), Triple("WED", "PULL", 4), Triple("FRI", "LEGS", 5))
private const val ROWS_IN = 40f / FRAMES
private const val ROW_STAGGER = 10f / FRAMES
private const val TALLY_IN = 66f / FRAMES
private const val TALLY_STEP = 3.4f / FRAMES
private const val WEEK_PITCH = 84f
private const val COL_SPLIT = 170f
private const val COL_WORK = 380f

private fun DrawScope.drawGenerated(phase: Float, ink: Ink) {
    val left = (ART_W - (COL_WORK + setsWidth(WEEK.maxOf { it.third }))) / 2f
    val top = (ART_H - (LABEL_SIZE + (WEEK.size - 1) * WEEK_PITCH)) / 2f
    val held = heldOut(phase)
    var tallied = 0

    WEEK.forEachIndexed { r, (day, split, work) ->
        val at = ROWS_IN + r * ROW_STAGGER
        val appear = (held + smoothstep(phase, at, at + 12f / FRAMES)).coerceIn(0f, 1f)
        val first = tallied
        tallied += work
        if (appear <= 0f) return@forEachIndexed
        val y = top + r * WEEK_PITCH
        monoLabel(ink, day, left, y, appear)
        monoLabel(ink, split, left + COL_SPLIT, y, appear)
        // One tally across the WHOLE week — it does not restart at a row break.
        setRow(ink, left + COL_WORK, y + BLOCK_DY, work) { i ->
            val t = TALLY_IN + (first + i) * TALLY_STEP
            held + smoothstep(phase, t, t + 7f / FRAMES)
        }
    }
}

// ---- Custom.tsx: BENCH ▪▪▪ — one day, named exercise by named exercise, never finished. ---------

private val DAY = listOf("BENCH" to 3, "INCLINE" to 4, "DIPS" to 3)
private val ROW_IN = listOf(46f / FRAMES, 76f / FRAMES, 106f / FRAMES)
private const val CURSOR_TAP = -6f / FRAMES
private const val CURSOR_TRAVEL = 9f / FRAMES
private const val CURSOR_R = 19f
private const val DAY_PITCH = 62f
private const val COL_SETS = 300f
private const val ADD_GAP = 18f

private fun DrawScope.drawCustom(phase: Float, ink: Ink) {
    val s = size.width / ART_W
    val left = (ART_W - (COL_SETS + setsWidth(DAY.maxOf { it.second }))) / 2f
    val top = (ART_H - (LABEL_SIZE + DAY.size * DAY_PITCH)) / 2f
    val held = heldOut(phase)

    DAY.forEachIndexed { i, (name, sets) ->
        val appear = (held + smoothstep(phase, ROW_IN[i], ROW_IN[i] + 14f / FRAMES)).coerceIn(0f, 1f)
        if (appear <= 0f) return@forEachIndexed
        val y = top + i * DAY_PITCH
        monoLabel(ink, name, left, y, appear)
        setRow(ink, left + COL_SETS, y + BLOCK_DY, sets) { appear }
    }

    // `+ ADD` waits on the first open line, dropping as soon as a row lands on the one above it.
    val stepped = ROW_IN.fold(0f) { acc, t -> acc + smoothstep(phase, t + CURSOR_TAP, t + CURSOR_TAP + CURSOR_TRAVEL) }
    val line = if (phase < CLEAR_END) DAY.size.toFloat() else stepped
    val alpha = maxOf(
        1f - smoothstep(phase, HOLD_END, HOLD_END + 8f / FRAMES),
        smoothstep(phase, CLEAR_END, CLEAR_END + 10f / FRAMES)
    )
    // Whole cycles across the loop, phased so both holds — the freeze included — catch it at full.
    val blink = 0.55f + 0.45f * cos(phase * 2f * Math.PI.toFloat() * 5f)
    val press = 1f - 0.16f * ROW_IN.fold(0f) { m, t ->
        maxOf(m, maxOf(0f, 1f - abs(phase - (t + CURSOR_TAP)) / (6f / FRAMES)))
    }
    val a = (alpha * blink).coerceIn(0f, 1f)
    if (a <= 0f) return
    val r = CURSOR_R * press
    val cx = (left + CURSOR_R) * s
    val cy = (top + line * DAY_PITCH + LABEL_SIZE / 2f) * s
    val pen = ink.accent.copy(alpha = a)
    // The video's 3px ring sits inside radius r, so the stroke centre is r - 1.5, not r + 1.5.
    drawCircle(pen, radius = (r - 1.5f) * s, center = Offset(cx, cy), style = Stroke(3f * s))
    drawRoundRect(pen, Offset(cx - r / 2f * s, cy - 2f * s), Size(r * s, 4f * s), CornerRadius(2f * s))
    drawRoundRect(pen, Offset(cx - 2f * s, cy - r / 2f * s), Size(4f * s, r * s), CornerRadius(2f * s))
    monoLabel(ink, "ADD", left + CURSOR_R * 2f + ADD_GAP, top + line * DAY_PITCH, a)
}

// ---- Freestyle.tsx: WED ▪▪▪▪ — landing wherever, in no order. No grid is the whole argument. -----

/** Day stamp, canvas position, set count, and the frame it lands on — `Freestyle.tsx`'s LOGS. */
private data class Logged(val day: String, val x: Float, val y: Float, val sets: Int, val at: Float)

private val LOGS = listOf(
    Logged("WED", 30f, 18f, 4, 42f),
    Logged("TUE", 700f, 150f, 4, 52f),
    Logged("SAT", 44f, 200f, 3, 74f),
    Logged("MON", 560f, 46f, 5, 86f),
    Logged("FRI", 420f, 226f, 4, 104f),
    Logged("THU", 132f, 104f, 3, 116f)
)

/** `Freestyle.tsx`'s day stamp column + its gap to the blocks. */
private const val STAMP_OFFSET = 93f + 18f

private fun DrawScope.drawFreestyle(phase: Float, ink: Ink) {
    val held = heldOut(phase)
    for (log in LOGS) {
        val at = log.at / FRAMES
        val appear = (held + smoothstep(phase, at, at + 14f / FRAMES)).coerceIn(0f, 1f)
        if (appear <= 0f) continue
        monoLabel(ink, log.day, log.x, log.y, appear)
        setRow(ink, log.x + STAMP_OFFSET, log.y + BLOCK_DY, log.sets) { appear }
    }
}
