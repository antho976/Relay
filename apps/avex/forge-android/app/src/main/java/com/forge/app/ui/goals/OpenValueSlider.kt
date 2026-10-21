package com.forge.app.ui.goals

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.OpenColors

/**
 * The redesign's value input (2026-10-21, Antho's pick for the goal target and the check-in): a
 * 4dp track filled red up to a white knob ringed in red. With [stops] it snaps to that many named
 * positions and draws a dot at each, red once passed; with [nowMark] a short ink tick marks where
 * you are at the start. [fraction] null is an unanswered question: the bare track, no knob.
 *
 * Internal to the app module so the check-in sheet draws the same control; not in ui/common, which
 * only holds the kit listed in DESIGN §8.
 */
@Composable
internal fun OpenValueSlider(
    fraction: Float?,
    onFraction: (Float) -> Unit,
    description: String,
    stateText: String,
    modifier: Modifier = Modifier,
    stops: Int = 0,
    nowMark: Boolean = false,
    height: Dp = 40.dp,
    knob: Dp = 24.dp,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    val onChange by rememberUpdatedState(onFraction)
    fun snapped(f: Float): Float {
        val c = f.coerceIn(0f, 1f)
        return if (stops > 1) Math.round(c * (stops - 1)) / (stops - 1).toFloat() else c
    }
    Canvas(
        modifier
            .fillMaxWidth()
            .height(height)
            .semantics {
                contentDescription = description
                stateDescription = stateText
                progressBarRangeInfo = ProgressBarRangeInfo(
                    fraction ?: 0f, 0f..1f, steps = if (stops > 2) stops - 2 else 0
                )
                setProgress { v -> onChange(snapped(v)); true }
            }
            .pointerInput(stops) {
                val inset = knob.toPx() / 2f
                detectTapGestures { p -> onChange(snapped((p.x - inset) / (size.width - 2 * inset))) }
            }
            .pointerInput(stops) {
                val inset = knob.toPx() / 2f
                detectHorizontalDragGestures(
                    onDragStart = { p -> onChange(snapped((p.x - inset) / (size.width - 2 * inset))) }
                ) { change, _ ->
                    change.consume()
                    onChange(snapped((change.position.x - inset) / (size.width - 2 * inset)))
                }
            }
    ) {
        val inset = knob.toPx() / 2f
        val w = size.width - 2 * inset
        val cy = size.height / 2f
        val th = 4.dp.toPx()
        val r = CornerRadius(th / 2f)
        drawRoundRect(OpenColors.Unreached, topLeft = Offset(inset, cy - th / 2f), size = Size(w, th), cornerRadius = r)
        val f = fraction
        if (f != null && f > 0f) {
            drawRoundRect(accent, topLeft = Offset(inset, cy - th / 2f), size = Size(w * f, th), cornerRadius = r)
        }
        if (stops > 1) {
            for (i in 0 until stops) {
                val sf = i / (stops - 1).toFloat()
                val lit = f != null && sf <= f + 1e-4f
                drawCircle(if (lit) accent else OpenColors.Rule, 3.dp.toPx(), Offset(inset + w * sf, cy))
            }
        }
        if (nowMark) {
            val mw = 2.dp.toPx()
            val mh = 16.dp.toPx()
            drawRect(ink, topLeft = Offset(inset - mw / 2f, cy - mh / 2f), size = Size(mw, mh))
        }
        if (f != null) {
            val c = Offset(inset + w * f, cy)
            val kr = knob.toPx() / 2f
            drawCircle(accent, kr + 1.5.dp.toPx(), c)
            drawCircle(ink, kr - 1.5.dp.toPx(), c)
        }
    }
}
