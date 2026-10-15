package com.quietsoftware.relay.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.Dp
import com.quietsoftware.relay.ui.theme.Palette
import kotlin.math.PI
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import com.quietsoftware.relay.ui.theme.Relay as Theme

/**
 * The mark and its opening (apps/relay-native/src/start.rs, kept in step with it): three lanes
 * staggered like a relay's start, back to front, and the signal that has broken away ahead of the
 * front one, on Relay's 120-unit grid. `t` is a point on the start screen's opening, 0..1; the
 * lanes draw in from the back with lit heads, the front lane's head breaks away as the signal and
 * its landing kicks back through the lanes.
 */
object Mark {
    /** The whole opening, as on the PC. */
    const val OPENING_MS = 1_900

    /** Each lane (y, from x, to x), back to front, and the stroke. */
    val LANE = arrayOf(floatArrayOf(87f, 23f, 77f), floatArrayOf(61f, 37f, 91f), floatArrayOf(35f, 51f, 71f))
    const val STROKE = 16f
    const val SIGNAL_X = 95f
    const val SIGNAL_Y = 35f
    const val SIGNAL_DOT = 10f
    const val TRAIL = 24f
    const val RECOIL_UNITS = 3f

    /** Where each part's motion sits on the 0..1 timeline. */
    class Span(val from: Float, val to: Float)
    val LANES = arrayOf(Span(.02f, .30f), Span(.07f, .35f), Span(.12f, .40f))
    val SIGNAL_IN = Span(.40f, .56f)
    val RECOIL = Span(.50f, .64f)
    const val RECOIL_STEP = .03f
    val WAVE = Span(0f, .7f)
    val PING = Span(.52f, 1f)
    val WORDMARK = Span(.36f, .66f)
    val GREETING = Span(.50f, .78f)
    val QUESTION = Span(.56f, .84f)
    val CARD_DEV = Span(.62f, .92f)
    val CARD_THREADS = Span(.70f, 1f)
    val FOOT = Span(.84f, 1f)

    private fun at(t: Float, s: Span) = ((t - s.from) / (s.to - s.from)).coerceIn(0f, 1f)

    /** Eased out: fast start, soft landing. */
    fun phase(t: Float, s: Span): Float = 1f - (1f - at(t, s)).pow(3)

    /** Eased out with a small overshoot: it arrives and settles. */
    fun phaseBack(t: Float, s: Span): Float {
        val x = at(t, s) - 1f
        val c = 1.70158f
        return 1f + (c + 1f) * x * x * x + c * x * x
    }

    /** How far lane `order` (0 is the front) is kicked back at `t`, 0..1: out and home again. */
    fun recoil(t: Float, order: Int): Float {
        val from = RECOIL.from + RECOIL_STEP * order
        val x = ((t - from) / (RECOIL.to - RECOIL.from)).coerceIn(0f, 1f)
        return sin(PI.toFloat() * x) * (1f - x)
    }
}

/** The mark at opening point `t` (1 settled), its signal swollen by `beat` (0..1). */
fun DrawScope.drawMark(t: Float, beat: Float, ink: Color, signal: Color) {
    val u = size.minDimension / 120f
    val front = Mark.LANE.lastIndex
    for (k in Mark.LANE.indices) {
        val (y, from, to) = Mark.LANE[k]
        val span = Mark.LANES[k]
        val shift = Mark.RECOIL_UNITS * Mark.recoil(t, front - k)
        val a = from - shift
        val b = from + (to - from) * Mark.phaseBack(t, span) - shift
        if (b - a < .5f) continue
        drawLine(ink, Offset(a * u, y * u), Offset(b * u, y * u), Mark.STROKE * u, StrokeCap.Round)
        // Lit at the head until it lands; the front lane's lit head is the signal itself.
        val lit = if (k == front) (if (t < Mark.SIGNAL_IN.from) 1f else 0f) else 1f - Mark.phase(t, Mark.Span(span.to - .06f, span.to + .08f))
        if (lit <= 0f) continue
        val tail = min(b - a, Mark.TRAIL)
        for (step in 0 until 8) {
            val x = b - tail * (8 - step) / 8f
            drawLine(signal.copy(alpha = lit * (.12f + .11f * step)), Offset(x * u, y * u), Offset((x + tail / 8f + .6f) * u, y * u), Mark.STROKE * u, StrokeCap.Butt)
        }
        drawCircle(signal.copy(alpha = lit), Mark.STROKE / 2f * u, Offset(b * u, y * u))
    }
    if (t < Mark.SIGNAL_IN.from) return
    // The signal breaks away from the front lane's tip, a streak behind it, and settles ahead.
    val (y, _, end) = Mark.LANE[front]
    val travel = Mark.phaseBack(t, Mark.SIGNAL_IN)
    val x = end + (Mark.SIGNAL_X - end) * travel
    val r = Mark.STROKE / 2f + (Mark.SIGNAL_DOT - Mark.STROKE / 2f) * travel + 1.6f * beat
    val streak = 1f - Mark.phase(t, Mark.SIGNAL_IN)
    if (streak > .01f) {
        drawLine(signal.copy(alpha = .55f * streak), Offset(max(x - 30f * streak, end) * u, y * u), Offset(x * u, y * u), r * 1.2f * u, StrokeCap.Butt)
    }
    drawCircle(signal, r * u, Offset(x * u, y * u))
}

/** The mark, settled unless `t` and `beat` say otherwise; read in the draw phase, so a running
 *  opening redraws the mark without recomposing it. */
@Composable
fun RelayMark(
    size: Dp,
    modifier: Modifier = Modifier,
    ink: Color = Theme.colors.ink,
    signal: Color = Palette.SIGNAL,
    t: () -> Float = { 1f },
    beat: () -> Float = { 0f },
) {
    Canvas(modifier.size(size)) { drawMark(t(), beat(), ink, signal) }
}
