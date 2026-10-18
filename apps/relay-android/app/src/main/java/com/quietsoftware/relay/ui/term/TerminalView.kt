package com.quietsoftware.relay.ui.term

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.animation.core.AnimationState
import androidx.compose.animation.core.animateDecay
import androidx.compose.animation.rememberSplineBasedDecay
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.res.ResourcesCompat
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.term.Line
import com.quietsoftware.relay.core.term.Style
import com.quietsoftware.relay.core.term.TermColor
import com.quietsoftware.relay.core.term.Terminal
import com.quietsoftware.relay.core.term.urlAt
import com.quietsoftware.relay.core.term.wordAt
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.delay
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlin.math.floor

/**
 * An agent's terminal drawn as the PC draws it (terminal.rs): Fira Mono on the screen colour,
 * the PC's ANSI palette, 1.25 cell height. Output is followed unless the person scrolled back;
 * pinch changes the text size. [onFit] gets the columns and rows that fit, whenever they change,
 * so the screen can borrow the PTY at this phone's size. With [onCopy], a long press selects the
 * word under the finger and a drag carries the selection on, for a Copy key; with [onLink], a tap
 * on a printed web address opens it. [hiddenBelow] is how much of the view's bottom its parent
 * clips away; the scroll thumb and the Latest key keep above it.
 */
@Composable
fun TerminalView(
    feed: TerminalFeed,
    fontSp: Float,
    onFontSp: (Float) -> Unit,
    onFit: (cols: Int, rows: Int) -> Unit,
    modifier: Modifier = Modifier,
    onTap: () -> Unit = {},
    interactive: Boolean = true,
    onCopy: ((String) -> Unit)? = null,
    onLink: ((String) -> Unit)? = null,
    hiddenBelow: Dp = 0.dp,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val c = Relay.colors
    val regular = remember { ResourcesCompat.getFont(context, R.font.fira_mono_400) ?: Typeface.MONOSPACE }
    val bold = remember { ResourcesCompat.getFont(context, R.font.fira_mono_500) ?: Typeface.MONOSPACE }
    remember { TermColor.setAnsiPalette(Palette.ANSI); 0 }
    val textPx = with(density) { fontSp.sp.toPx() }
    val paint = remember(textPx) {
        Paint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG).apply {
            typeface = regular
            textSize = textPx
        }
    }
    val cellW = remember(paint) { paint.measureText("M") }
    val cellH = remember(textPx) { (textPx * 1.25f).let { kotlin.math.ceil(it) } }
    val baseline = remember(paint, cellH) { (cellH - (paint.fontMetrics.descent + paint.fontMetrics.ascent)) / 2f }

    // What has been drawn: held while the program has synchronized output open (DEC 2026).
    var shown by remember { mutableLongStateOf(0L) }
    LaunchedEffect(feed) {
        feed.version.collectLatest {
            delay(FRAME_MS)
            if (!feed.terminal.synchronizedOutput) shown = it
        }
    }
    // Lines scrolled back from the bottom; 0 follows the output.
    var back by remember { mutableIntStateOf(0) }
    var dragCarry by remember { mutableFloatStateOf(0f) }
    // A flick keeps scrolling and slows as a list does; a touch stops it.
    val scope = rememberCoroutineScope()
    val decay = rememberSplineBasedDecay<Float>()
    var fling by remember { mutableStateOf<Job?>(null) }
    val tap by rememberUpdatedState(onTap)
    val fontState by rememberUpdatedState(fontSp)
    val setFont by rememberUpdatedState(onFontSp)
    // The gesture outlives a zoom: it reads the row height as it is now, not as it was at the first frame.
    val rowPx by rememberUpdatedState(cellH)
    val colPx by rememberUpdatedState(cellW)
    val copy by rememberUpdatedState(onCopy)
    val link by rememberUpdatedState(onLink)
    val haptic = LocalHapticFeedback.current
    // Cells from where a long press began to where the finger is: rows as the buffer counts them.
    var selection by remember { mutableStateOf<Selection?>(null) }
    var selecting by remember { mutableStateOf(false) }

    BoxWithConstraints(modifier) {
        val padX = with(density) { 6.dp.toPx() }
        val padY = with(density) { 4.dp.toPx() }
        val widthPx = constraints.maxWidth.toFloat()
        val heightPx = constraints.maxHeight.toFloat()
        val cols = ((widthPx - padX * 2) / cellW).toInt().coerceAtLeast(20)
        val rows = ((heightPx - padY * 2) / cellH).toInt().coerceAtLeast(4)
        LaunchedEffect(cols, rows) { onFit(cols, rows) }

        Canvas(
            Modifier
                .fillMaxSize()
                // Where the view stands, for a screen reader (and a test): following, or how far back.
                .semantics { stateDescription = if (back > 0) "$back lines back" else "Following the output" }
                .then(
                    if (interactive) Modifier.pointerInput(Unit) {
                        // One finger scrolls the output, two zoom the text, a tap focuses the composer.
                        val slop = viewConfiguration.touchSlop
                        val flingFrom = viewConfiguration.minimumFlingVelocity
                        val holdFor = viewConfiguration.longPressTimeoutMillis
                        val padX = 6.dp.toPx()
                        val padY = 4.dp.toPx()
                        /** The cell under [at], as (row in the buffer, column). */
                        fun cellAt(at: Offset): Pair<Int, Int> = synchronized(feed.lock) {
                            val t = feed.terminal
                            val total = t.scrollbackSize + t.rows
                            val visible = floor((size.height - padY * 2) / rowPx).toInt().coerceAtLeast(1)
                            val first = (total - visible - back.coerceAtMost(t.scrollbackSize)).coerceAtLeast(0)
                            val row = (first + ((at.y - padY) / rowPx).toInt()).coerceIn(0, total - 1)
                            row to ((at.x - padX) / colPx).toInt().coerceIn(0, t.cols - 1)
                        }
                        /** Moves the view by [dy] pixels of finger travel; false at either end. */
                        fun scrollBy(dy: Float): Boolean {
                            dragCarry += dy
                            val lines = (dragCarry / rowPx).toInt()
                            if (lines == 0) return true
                            dragCarry -= lines * rowPx
                            val max = synchronized(feed.lock) { feed.terminal.scrollbackSize }
                            val next = (back + lines).coerceIn(0, max)
                            val moved = next != back
                            back = next
                            return moved
                        }
                        awaitEachGesture {
                            val down = awaitFirstDown(requireUnconsumed = false)
                            val flung = fling?.isActive == true
                            fling?.cancel()
                            val velocity = VelocityTracker()
                            velocity.addPosition(down.uptimeMillis, down.position)
                            var travel = 0f
                            var gesture = flung
                            var pinched = false
                            var moved = false
                            // A pinch scales the size it started from: touches arrive several times
                            // per frame, faster than the size shown can follow.
                            var pinchFrom = 0f
                            var pinch = 0f
                            // A finger held still selects, when the screen copies: what ended the wait otherwise goes on below.
                            var pending: PointerEvent? = null
                            if (copy != null && !flung) {
                                val ended = withTimeoutOrNull(holdFor) {
                                    var e: PointerEvent
                                    do {
                                        e = awaitPointerEvent()
                                        val f = e.changes.firstOrNull { it.id == down.id }
                                    } while (f != null && f.pressed && e.changes.count { it.pressed } == 1 && (f.position - down.position).getDistance() <= slop)
                                    e
                                }
                                if (ended == null) {
                                    val (row, col) = cellAt(down.position)
                                    selection = synchronized(feed.lock) {
                                        val t = feed.terminal
                                        val word = t.wordAt(row, col) ?: col..col
                                        Selection(row, word.first, row, word.last, t.dropped, t.cleared)
                                    }
                                    selecting = true
                                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                    do {
                                        val e = awaitPointerEvent()
                                        val f = e.changes.firstOrNull { it.pressed } ?: break
                                        if ((f.position - down.position).getDistance() > slop) {
                                            val (r, cc) = cellAt(f.position)
                                            selection = synchronized(feed.lock) { selection?.now(feed.terminal) }?.let { now ->
                                                // Kept as the press began it: the anchor stays, the end follows the finger.
                                                val anchored = selection!!.let { it.copy(startRow = it.startRow - (now.dropped - it.dropped).toInt()) }
                                                anchored.copy(endRow = r, endCol = cc, dropped = now.dropped)
                                            }
                                        }
                                        e.changes.forEach { it.consume() }
                                    } while (e.changes.any { it.pressed })
                                    selecting = false
                                    return@awaitEachGesture
                                }
                                pending = ended
                                // What the finger travelled while the wait went on, up to the event that ended it.
                                ended.changes.firstOrNull { it.id == down.id }?.let { travel = it.previousPosition.y - down.position.y }
                            }
                            do {
                                val event = pending ?: awaitPointerEvent()
                                pending = null
                                // Moved off the spot (sideways too, as a pan of a wide terminal does, or as it lifts): not a tap.
                                event.changes.firstOrNull { it.id == down.id }?.let { if ((it.position - down.position).getDistance() > slop) moved = true }
                                val pressed = event.changes.count { it.pressed }
                                if (pressed >= 2) {
                                    pinched = true
                                    if (pinch == 0f) {
                                        pinchFrom = fontState
                                        pinch = 1f
                                    }
                                    pinch *= event.calculateZoom()
                                    setFont((pinchFrom * pinch).coerceIn(MIN_SP, MAX_SP))
                                    event.changes.forEach { it.consume() }
                                    gesture = true
                                } else if (pressed == 1) {
                                    pinch = 0f
                                    val finger = event.changes.first { it.pressed }
                                    velocity.addPosition(finger.uptimeMillis, finger.position)
                                    val dy = finger.positionChange().y
                                    travel += dy
                                    if (gesture || kotlin.math.abs(travel) > slop) {
                                        gesture = true
                                        scrollBy(dy)
                                        event.changes.forEach { it.consume() }
                                    }
                                }
                            } while (event.changes.any { it.pressed })
                            val vy = velocity.calculateVelocity().y
                            if (gesture && !pinched && kotlin.math.abs(vy) > flingFrom) {
                                fling = scope.launch {
                                    var last = 0f
                                    AnimationState(0f, vy).animateDecay(decay) {
                                        val moved = scrollBy(value - last)
                                        last = value
                                        if (!moved) cancelAnimation()
                                    }
                                    dragCarry = 0f
                                }
                            } else {
                                dragCarry = 0f
                            }
                            // A touch that only stopped a flick is not a tap; one with a selection up puts it away.
                            if (!gesture && !moved) {
                                if (selection != null) {
                                    selection = null
                                } else {
                                    val (row, col) = cellAt(down.position)
                                    val url = link?.let { synchronized(feed.lock) { feed.terminal.urlAt(row, col) } }
                                    if (url != null) link?.invoke(url) else tap()
                                }
                            }
                        }
                    }
                    else Modifier,
                ),
        ) {
            // Read so a new frame redraws.
            shown.let { }
            drawRect(c.screen)
            val canvas = drawContext.canvas.nativeCanvas
            synchronized(feed.lock) {
                val t = feed.terminal
                val total = t.scrollbackSize + t.rows
                val visible = floor((size.height - padY * 2) / cellH).toInt().coerceAtLeast(1)
                val scroll = back.coerceAtMost(t.scrollbackSize)
                val first = (total - visible - scroll).coerceAtLeast(0)
                for (i in 0 until visible) {
                    val index = first + i
                    if (index >= total) break
                    drawLine(canvas, t.lineAt(index), padX, padY + i * cellH, cellW, cellH, baseline, paint, regular, bold, c.termFg.toArgb())
                }
                selection?.now(t)?.let { sel ->
                    // The selection, in reading order, over the rows in view.
                    val (sr, sc, er, ec) = sel.ordered()
                    for (row in maxOf(sr, first)..minOf(er, first + visible - 1, total - 1)) {
                        val from = if (row == sr) sc else 0
                        val to = if (row == er) ec else t.lineAt(row).length - 1
                        if (to < from) continue
                        drawRect(
                            c.selection,
                            topLeft = androidx.compose.ui.geometry.Offset(padX + from * cellW, padY + (row - first) * cellH),
                            size = androidx.compose.ui.geometry.Size((to - from + 1) * cellW, cellH),
                        )
                    }
                }
                if (scroll > 0 && total > visible) {
                    // Where the view sits in the scrollback: a thin thumb on the right edge.
                    val track = size.height - padY * 2 - with(density) { hiddenBelow.toPx() }
                    val thumb = (track * visible / total).coerceAtLeast(with(density) { 24.dp.toPx() })
                    val top = padY + (track - thumb) * first / (total - visible).coerceAtLeast(1)
                    val w = with(density) { 3.dp.toPx() }
                    drawRoundRect(
                        c.ink3.copy(alpha = .55f),
                        topLeft = androidx.compose.ui.geometry.Offset(size.width - w - with(density) { 2.dp.toPx() }, top),
                        size = androidx.compose.ui.geometry.Size(w, thumb),
                        cornerRadius = androidx.compose.ui.geometry.CornerRadius(w / 2, w / 2),
                    )
                }
                if (scroll == 0 && t.cursorVisible) {
                    val rowOnScreen = t.scrollbackSize + t.cursorRow - first
                    if (rowOnScreen in 0 until visible) {
                        val x = padX + t.cursorCol.coerceAtMost(t.cols - 1) * cellW
                        val y = padY + rowOnScreen * cellH
                        drawRect(c.termCursor.copy(alpha = .55f), topLeft = androidx.compose.ui.geometry.Offset(x, y), size = androidx.compose.ui.geometry.Size(cellW, cellH))
                    }
                }
            }
        }
        val sel = selection
        if (sel != null && !selecting && onCopy != null) {
            Row(
                Modifier.align(Alignment.TopCenter).padding(top = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Key("Copy", {
                    val text = synchronized(feed.lock) {
                        sel.now(feed.terminal)?.ordered()?.let { (sr, sc, er, ec) -> feed.terminal.textBetween(sr, sc, er, ec) }.orEmpty()
                    }
                    selection = null
                    if (text.isNotEmpty()) onCopy(text)
                }, kind = KeyKind.Primary, glyph = "copy", compact = true)
                Key("Cancel", { selection = null }, kind = KeyKind.Plain, compact = true)
            }
        }
        if (back > 0) {
            Box(Modifier.align(Alignment.BottomEnd).padding(start = 10.dp, top = 10.dp, end = 10.dp, bottom = 10.dp + hiddenBelow)) {
                Key("Latest", {
                    fling?.cancel()
                    back = 0
                }, kind = KeyKind.Plain, glyph = "arrow-down", compact = true)
            }
        }
    }
}

/**
 * A stretch of cells from where a long press began ([startRow], [startCol]) to where the finger
 * went, rows as the buffer counted them when it had [dropped] lines and [cleared] clears.
 */
private data class Selection(val startRow: Int, val startCol: Int, val endRow: Int, val endCol: Int, val dropped: Long, val cleared: Int) {
    /** Start and end in reading order, whichever way the finger went. */
    fun ordered(): List<Int> =
        if (endRow < startRow || (endRow == startRow && endCol < startCol)) listOf(endRow, endCol, startRow, startCol) else listOf(startRow, startCol, endRow, endCol)

    /** The same text in [t] as it is now: moved up by the lines it lost off the top; null once cleared or gone. */
    fun now(t: Terminal): Selection? {
        if (t.cleared != cleared) return null
        val shift = (t.dropped - dropped).toInt()
        val last = t.scrollbackSize + t.rows - 1
        val (sr, sc, er, ec) = ordered()
        if (er - shift < 0 || sr - shift > last) return null
        val start = if (sr - shift < 0) 0 to 0 else (sr - shift) to sc
        val end = if (er - shift > last) last to Int.MAX_VALUE - 1 else (er - shift) to ec
        return Selection(start.first, start.second, end.first, end.second, t.dropped, t.cleared)
    }
}

private const val FRAME_MS = 16L
const val MIN_SP = 6f
const val MAX_SP = 20f

/** One row: background runs, then text runs, each run one style. */
private fun drawLine(
    canvas: android.graphics.Canvas,
    line: Line,
    x0: Float,
    y0: Float,
    cellW: Float,
    cellH: Float,
    baseline: Float,
    paint: Paint,
    regular: Typeface,
    bold: Typeface,
    defaultFg: Int,
) {
    val n = line.length
    // Backgrounds.
    var col = 0
    while (col < n) {
        val st = line.style(col)
        val bgc = background(st, defaultFg)
        var end = col + 1
        while (end < n && line.style(end) == st) end++
        if (bgc != 0) {
            paint.color = bgc
            canvas.drawRect(x0 + col * cellW, y0, x0 + end * cellW, y0 + cellH, paint)
        }
        col = end
    }
    // Text, one style at a time; each character placed on its own cell so wide glyphs and
    // fallback fonts never push the rest of the line out of the grid.
    val chars = CharArray(2)
    col = 0
    while (col < n) {
        val cp = line.char(col)
        if (cp <= 0) {
            col++
            continue
        }
        val st = line.style(col)
        if (Style.invisible(st)) {
            col++
            continue
        }
        paint.typeface = if (Style.bold(st)) bold else regular
        paint.isFakeBoldText = false
        paint.textSkewX = if (Style.italic(st)) -0.2f else 0f
        var fg = foreground(st, defaultFg)
        if (Style.dim(st)) fg = (fg and 0x00FFFFFF) or (0x99 shl 24)
        paint.color = fg
        val x = x0 + col * cellW
        val extra = line.extra(col)
        val wide = col + 1 < n && line.char(col + 1) == Line.WIDE_TAIL
        val plain = PLAIN[cp]
        if (extra == null && plain != null) {
            // Media marks only a colour-emoji font carries (Claude Code's ⏺): their plain twin in
            // the terminal's own face, in the cell's colour, as VTE shows them on the PC.
            chars[0] = plain
            canvas.drawText(chars, 0, 1, x, y0 + baseline, paint)
        } else if (extra == null && !wide && textPresentation(cp)) {
            // Any other symbol with an emoji form: ask for its text form.
            chars[0] = cp.toChar()
            chars[1] = '\uFE0E'
            canvas.drawText(chars, 0, 2, x, y0 + baseline, paint)
        } else if (extra == null && cp < 0x10000) {
            chars[0] = cp.toChar()
            canvas.drawText(chars, 0, 1, x, y0 + baseline, paint)
        } else {
            canvas.drawText(String(Character.toChars(cp)) + (extra ?: ""), x, y0 + baseline, paint)
        }
        if (Style.underline(st)) canvas.drawRect(x, y0 + cellH - 2f, x + cellW * (if (wide) 2 else 1), y0 + cellH - 1f, paint)
        if (Style.strikethrough(st)) canvas.drawRect(x, y0 + cellH / 2, x + cellW * (if (wide) 2 else 1), y0 + cellH / 2 + 1f, paint)
        col += if (wide) 2 else 1
    }
    paint.textSkewX = 0f
    paint.typeface = regular
}

/** ⏺ ⏹ ⏸ ⏵: record, stop, pause and play, as the geometric shapes Fira Mono draws. */
private val PLAIN = mapOf(0x23FA to '\u25CF', 0x23F9 to '\u25A0', 0x23F8 to '\u2016', 0x23F5 to '\u25B6')

/** Narrow symbols with an emoji form, in the ranges terminal programs draw as plain marks. */
private fun textPresentation(cp: Int): Boolean =
    cp in 0x2300..0x23FF || cp in 0x2600..0x27BF || cp in 0x2B00..0x2BFF || cp == 0x2139 || cp in 0x2190..0x21FF

private fun foreground(st: Long, defaultFg: Int): Int {
    val inverse = Style.inverse(st)
    val code = if (inverse) Style.bg(st) else Style.fg(st)
    val rgb = when {
        TermColor.isDefault(code) -> if (inverse) SCREEN_RGB else defaultFg and 0xFFFFFF
        // Bold brightens the eight base colours, as xterm and VTE do.
        Style.bold(st) && code in 0..7 -> TermColor.palette256(code + 8)
        else -> TermColor.resolve(code, defaultFg and 0xFFFFFF)
    }
    return rgb or (0xFF shl 24)
}

/** 0 for the screen's own colour, which is already painted. */
private fun background(st: Long, defaultFg: Int): Int {
    val inverse = Style.inverse(st)
    val code = if (inverse) Style.fg(st) else Style.bg(st)
    if (TermColor.isDefault(code)) return if (inverse) (defaultFg or (0xFF shl 24)) else 0
    return TermColor.resolve(code, 0) or (0xFF shl 24)
}

private const val SCREEN_RGB = 0x0A0A0B
