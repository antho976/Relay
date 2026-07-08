package com.forge.app.ui.onboarding

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeMotion

/**
 * The little looping "how this mode trains" illustrations on the plan-mode cards — a live drawing in
 * place of a video, so each choice is SHOWN, not narrated. All three share one visual vocabulary
 * (day frames = outline strokes, logged work = accent bars) and one loop length; the system
 * reduce-motion preference freezes them on their final frame.
 */

// Mirrors the Remotion videos' loop (240 frames @ 30fps, remotion-vignettes/src/theme.ts) so the
// Canvas vignettes breathe at the same pace as the video cards beside them.
private const val LOOP_MS = 8000

/** 0→1 looping phase, or a static 1f when the user removed animations. */
@Composable
private fun vignettePhase(): Float {
    if (ForgeMotion.durationScale <= 0f) return 1f
    val transition = rememberInfiniteTransition(label = "vignette")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(LOOP_MS, easing = LinearEasing), RepeatMode.Restart),
        label = "vignette_phase"
    )
    return phase
}

/** Per-element reveal: 0 before [start], eased 0→1 across [start]..[start]+[ramp], 1 after. */
private fun reveal(phase: Float, start: Float, ramp: Float = 0.08f): Float {
    val t = ((phase - start) / ramp).coerceIn(0f, 1f)
    return t * t * (3f - 2f * t) // smoothstep
}

/** Sized by the caller — [PlanModeMedia] gives video and Canvas twin the same aspect slot. */
@Composable
internal fun PlanModeVignette(mode: String, modifier: Modifier = Modifier) {
    val phase = vignettePhase()
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Canvas(modifier = modifier) {
        when (mode) {
            PLAN_GENERATED -> drawGenerated(phase, accent, outline)
            PLAN_CUSTOM -> drawCustom(phase, accent, outline, muted)
            else -> drawFreestyle(phase, accent, muted)
        }
    }
}

/** Build me a plan — four day frames appear, then fill themselves with exercise bars. */
private fun DrawScope.drawGenerated(phase: Float, accent: Color, outline: Color) {
    val gap = 8.dp.toPx()
    val cardW = (size.width - gap * 3) / 4f
    val cardH = size.height
    val stroke = 1.dp.toPx()
    val corner = CornerRadius(4.dp.toPx())
    for (c in 0 until 4) {
        val left = c * (cardW + gap)
        val frame = reveal(phase, 0.03f + c * 0.05f)
        if (frame <= 0f) continue
        drawRoundRect(
            color = outline.copy(alpha = 0.35f * frame),
            topLeft = Offset(left, 0f),
            size = Size(cardW, cardH),
            cornerRadius = corner,
            style = Stroke(stroke)
        )
        // Three exercise bars per day, filling in left-to-right across the week.
        val barH = 3.dp.toPx()
        val inset = 6.dp.toPx()
        for (i in 0 until 3) {
            val t = reveal(phase, 0.24f + (c * 3 + i) * 0.05f)
            if (t <= 0f) continue
            val fullW = (cardW - inset * 2) * if (i == 2) 0.6f else 1f
            drawRoundRect(
                color = accent.copy(alpha = t),
                topLeft = Offset(left + inset, inset + i * (barH + 5.dp.toPx())),
                size = Size(fullW * t, barH),
                cornerRadius = CornerRadius(barH / 2)
            )
        }
    }
}

/** I'll make my own — one day frame; rows are added one at a time by a moving `+`. */
private fun DrawScope.drawCustom(phase: Float, accent: Color, outline: Color, muted: Color) {
    val cardW = size.width * 0.58f
    val left = (size.width - cardW) / 2f
    val corner = CornerRadius(4.dp.toPx())
    drawRoundRect(
        color = outline.copy(alpha = 0.35f),
        topLeft = Offset(left, 0f),
        size = Size(cardW, size.height),
        cornerRadius = corner,
        style = Stroke(1.dp.toPx())
    )
    val inset = 8.dp.toPx()
    val barH = 3.dp.toPx()
    val rowStep = 9.dp.toPx()
    val widths = floatArrayOf(1f, 0.75f, 0.9f, 0.55f)
    var placedRows = 0
    for (i in widths.indices) {
        val t = reveal(phase, 0.12f + i * 0.2f, ramp = 0.1f)
        if (t <= 0f) break
        placedRows = i + 1
        drawRoundRect(
            color = accent.copy(alpha = t),
            topLeft = Offset(left + inset, inset + i * rowStep),
            size = Size((cardW - inset * 2) * widths[i] * t, barH),
            cornerRadius = CornerRadius(barH / 2)
        )
    }
    // The "you" cursor: a + waiting under the last placed row, gently pulsing.
    val plusY = inset + placedRows * rowStep + barH / 2
    if (plusY < size.height - inset / 2) {
        val pulse = 0.75f + 0.25f * kotlin.math.sin(phase * 6f * Math.PI).toFloat()
        val r = 3.2.dp.toPx()
        val cx = left + inset + r
        val strokeW = 1.4.dp.toPx()
        drawLine(muted.copy(alpha = pulse), Offset(cx - r, plusY), Offset(cx + r, plusY), strokeW, StrokeCap.Round)
        drawLine(muted.copy(alpha = pulse), Offset(cx, plusY - r), Offset(cx, plusY + r), strokeW, StrokeCap.Round)
    }
}

/** Go with the flow — no frames at all: logged sets pop in scattered, whenever. */
private fun DrawScope.drawFreestyle(phase: Float, accent: Color, muted: Color) {
    // (x, y, width) in fractions of the canvas; order is the pop-in order.
    val logs = listOf(
        floatArrayOf(0.06f, 0.18f, 0.20f),
        floatArrayOf(0.55f, 0.10f, 0.14f),
        floatArrayOf(0.30f, 0.44f, 0.24f),
        floatArrayOf(0.74f, 0.38f, 0.18f),
        floatArrayOf(0.12f, 0.72f, 0.16f),
        floatArrayOf(0.48f, 0.78f, 0.22f),
        floatArrayOf(0.84f, 0.70f, 0.10f)
    )
    val barH = 3.dp.toPx()
    logs.forEachIndexed { i, (x, y, w) ->
        val t = reveal(phase, 0.06f + i * 0.13f, ramp = 0.07f)
        if (t <= 0f) return@forEachIndexed
        val overshoot = 1f + 0.35f * kotlin.math.sin(t * Math.PI).toFloat() // pop, then settle
        val width = size.width * w * overshoot
        val cx = size.width * x + size.width * w / 2
        // A tiny leading dot + its set bar — the freestyle "logged it" vocabulary.
        drawCircle(muted.copy(alpha = t), radius = 1.6.dp.toPx(), center = Offset(cx - width / 2 - 4.dp.toPx(), size.height * y + barH / 2))
        drawRoundRect(
            color = accent.copy(alpha = t),
            topLeft = Offset(cx - width / 2, size.height * y),
            size = Size(width, barH),
            cornerRadius = CornerRadius(barH / 2)
        )
    }
}
