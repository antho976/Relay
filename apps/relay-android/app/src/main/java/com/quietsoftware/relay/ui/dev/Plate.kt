package com.quietsoftware.relay.ui.dev

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.term.Terminal
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.term.TerminalView
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.math.abs
import kotlin.math.ceil

/** The lanes a worktree is coloured by on the PC (terminal.rs). */
private val LANES = listOf(0xFF4F8FD9, 0xFFC9A13B, 0xFFA56CD6, 0xFF3FB0A5, 0xFFD97A4F).map { Color(it) }

/** The PC's hash: UTF-16 units folded by 31, as `String.hashCode`, so a worktree keeps its colour on both. */
internal fun laneColor(s: Session): Color {
    val key = s.worktree.ifEmpty { s.branch }
    return LANES[(abs(key.hashCode().toLong()) % LANES.size).toInt()]
}

/** "PROVIDER · ROLE · INTENT", or "· PAIR x" when it says nothing of what it does. */
internal fun plateMeta(s: Session): String {
    val intent = s.intent?.trim().orEmpty()
    val tail = when {
        intent.isNotEmpty() -> " · $intent"
        !s.pairWith.isNullOrBlank() -> " · pair ${s.pairWith}"
        else -> ""
    }
    return "${s.provider} · ${s.role}$tail".uppercase()
}

/** A session the phone made and the PC has not yet: shown, not yet openable. */
internal val Session.pending: Boolean get() = Optimistic.isTemp(name)

internal fun Session.shownName(): String = if (pending) "New agent" else name

/** What a slate says under the state word (terminal.rs `slate_hint`), shortened for a phone. */
internal fun slateHint(s: Session, online: Boolean): String = when {
    s.pending -> if (online) "The PC is preparing its worktree." else "Starts when the PC is back."
    s.state == "parked" -> "Parked — wake to continue. Scrollback and worktree are kept."
    s.state == "restorable" -> "The process ended. Resume restores its conversation and worktree."
    s.state == "exited" -> "The process exited. Resume to start it again."
    s.state == "created" -> "Not started yet. Start launches the provider in its worktree."
    s.state == "closed" -> "Closed."
    s.attachable -> if (online) "Open it to watch it live." else "The PC is away: its last screen, as this phone saved it."
    else -> "No signal from this session."
}

/**
 * The PC's identity strip (`.umd`): lamp, name (red when it needs the person), what it is and
 * does, the branch beside its lane colour, the state word, then [trailing] keys. 26dp on the
 * console colour with an edge below.
 */
@Composable
internal fun PlateStrip(s: Session, modifier: Modifier = Modifier, trailing: @Composable RowScope.() -> Unit = {}) {
    val c = Relay.colors
    val held = s.state == "blocked"
    BoxWithConstraints(modifier.fillMaxWidth().background(c.console)) {
        // A phone's width cuts the one line short at "CLAUDE · BUILDER · …": what the agent says it
        // does gets a line of its own, in its own words, beside the branch.
        val narrow = maxWidth < 520.dp
        Column(Modifier.fillMaxWidth()) {
            Row(
                Modifier.fillMaxWidth().heightIn(min = 26.dp).padding(start = 8.dp, end = 2.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(7.dp),
            ) {
                LampDot(s.lamp, 6.dp)
                T(s.shownName(), Modifier.widthIn(max = 150.dp), Relay.type.plateName, if (held) c.heldText else c.ink, maxLines = 1)
                if (narrow) {
                    T("${s.provider} · ${s.role}".uppercase(), Modifier.weight(1f), Relay.type.plateMeta, c.ink3, maxLines = 1)
                } else {
                    T(plateMeta(s), Modifier.weight(1f), Relay.type.plateMeta, c.ink3, maxLines = 1)
                    if (s.branch.isNotBlank()) Branch(s, Modifier.widthIn(max = 120.dp))
                }
                T(s.stateLabel, style = Relay.type.plateState, color = if (held) c.heldText else c.ink3, maxLines = 1)
                trailing()
            }
            val intent = s.intent?.trim().orEmpty()
            if (narrow && (intent.isNotEmpty() || s.branch.isNotBlank())) {
                Row(
                    Modifier.fillMaxWidth().padding(start = 21.dp, end = 10.dp, bottom = 5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    if (intent.isNotEmpty()) T(intent, Modifier.weight(1f), Relay.type.caption, c.ink2, maxLines = 1)
                    if (s.branch.isNotBlank()) Branch(s, if (intent.isNotEmpty()) Modifier.widthIn(max = 130.dp) else Modifier.weight(1f, fill = false))
                }
            }
            Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
        }
    }
}

/** The lane square and the branch, cut in the middle so both ends stay readable. */
@Composable
internal fun Branch(s: Session, modifier: Modifier = Modifier, size: Float = 11f) {
    val c = Relay.colors
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        Box(Modifier.size(6.dp).background(laneColor(s)))
        BasicText(
            s.branch,
            style = TextStyle(fontFamily = Fonts.FiraMono, fontSize = size.sp, color = c.ink3),
            maxLines = 1,
            overflow = TextOverflow.MiddleEllipsis,
        )
    }
}

/** A strip key (terminal.rs: 22px, radius 2): a glyph, dimmed when the PC is away. */
@Composable
internal fun StripKey(act: Act, onClick: () -> Unit, dim: Boolean = false) {
    val c = Relay.colors
    Box(
        Modifier
            .size(width = 34.dp, height = 26.dp)
            .alpha(if (dim) .4f else 1f)
            .clip(SQUARE)
            .clickable(role = Role.Button, onClickLabel = act.label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(22.dp).clip(SQUARE).border(1.dp, c.strong, SQUARE), contentAlignment = Alignment.Center) {
            Glyph(act.glyph, 13.dp, c.ink2)
        }
    }
}

private val SQUARE = RoundedCornerShape(2.dp)

// ---- Text size ----

/** Fira Mono's advance per pixel of text size, as [TerminalView] measures its cell. */
@Composable
internal fun rememberMonoRatio(): Float {
    val context = LocalContext.current
    return remember {
        val face = ResourcesCompat.getFont(context, R.font.fira_mono_400) ?: Typeface.MONOSPACE
        Paint().apply { typeface = face; textSize = 100f }.measureText("M") / 100f
    }
}

/** The text size that fits [cols] columns into [widthPx], inside the view's 6dp margins; 0 when unknown. */
internal fun fitSp(widthPx: Float, cols: Int, ratio: Float, density: Density): Float {
    if (cols <= 0 || ratio <= 0f) return 0f
    val textPx = (widthPx - with(density) { 12.dp.toPx() }) / cols / ratio * 0.995f
    return with(density) { textPx.toSp().value }
}

/** [TerminalView]'s row height in pixels at [sp]. */
internal fun cellHeightPx(sp: Float, density: Density): Float = ceil(with(density) { sp.sp.toPx() } * 1.25f)

// ---- The miniature ----

@Immutable
private data class MiniShape(val cols: Int = 0, val blank: Int = 0)

/** Screen rows under the last one with text or the cursor: a fresh TUI draws at the top. */
private fun blankBelow(t: Terminal): Int {
    var last = t.rows - 1
    while (last > t.cursorRow) {
        val line = t.row(last)
        var empty = true
        for (col in 0 until line.length) {
            val cp = line.char(col)
            if (cp > 0 && cp != 32) {
                empty = false
                break
            }
        }
        if (!empty) break
        last--
    }
    return t.rows - 1 - last
}

/**
 * A live agent's screen at a glance: the shared feed, drawn small and not touchable, showing the
 * last lines with text in them. A fresh TUI draws from the top and leaves the bottom rows
 * blank, so the view is drawn taller by those rows and clipped, which keeps its content in sight.
 */
@Composable
internal fun Miniature(relay: RelayData, name: String, height: Dp) {
    val feed = rememberFeed(relay, name)
    val density = LocalDensity.current
    val ratio = rememberMonoRatio()
    val shape by produceState(MiniShape(), feed) {
        // A StateFlow: a collector that is still waiting sees only the newest version.
        feed.version.collect {
            value = synchronized(feed.lock) { MiniShape(feed.pcSize?.first ?: feed.terminal.cols, blankBelow(feed.terminal)) }
            delay(MINI_POLL_MS)
        }
    }
    BoxWithConstraints(Modifier.fillMaxWidth().height(height).clipToBounds()) {
        val fit = fitSp(constraints.maxWidth.toFloat(), shape.cols, ratio, density)
        // A narrow PC pane is shown whole; a wide one from its left edge, still readable.
        val font = if (fit <= 0f) MINI_SP else fit.coerceIn(MINI_MIN_SP, MINI_MAX_SP)
        val extra = with(density) { (shape.blank * cellHeightPx(font, density)).toDp() }
        TerminalView(
            feed,
            font,
            NO_FONT,
            NO_FIT,
            Modifier.fillMaxWidth().wrapContentHeight(Alignment.Top, unbounded = true).height(height + extra),
            interactive = false,
        )
    }
}

private val NO_FONT: (Float) -> Unit = {}
private val NO_FIT: (Int, Int) -> Unit = { _, _ -> }
private const val MINI_SP = 7.5f
private const val MINI_MIN_SP = 7f
private const val MINI_MAX_SP = 8f
private const val MINI_POLL_MS = 250L

// ---- Saved text ----

/**
 * The last [n] lines of what the phone saved for [name] (TerminalFeed's own cache), as plain
 * text: the PTY's bytes are run through a terminal so escape sequences never show.
 */
@Composable
internal fun rememberSavedLines(relay: RelayData, name: String, n: Int): List<String> {
    val payload = remember(name) { buildJsonObject { put("session", name); put("lines", TerminalFeed.SAVED_LINES) } }
    val answer by remember(name) { relay.cached("session.scrollback", payload) }.collectAsStateWithLifecycle(null)
    val text = (answer?.result as? JsonObject)?.s("text")
    val lines by produceState(emptyList<String>(), text, n) {
        value = withContext(Dispatchers.Default) { lastLines(text, n) }
    }
    return lines
}

private fun lastLines(text: String?, n: Int): List<String> {
    if (text.isNullOrBlank()) return emptyList()
    val t = Terminal(160, 48, scrollbackLimit = TerminalFeed.SAVED_LINES)
    t.feed(text.replace("\r\n", "\n").replace("\n", "\r\n"))
    return t.plainText(n).lines().takeLast(n)
}

/** Saved lines in the terminal's face, on the plate's screen colour. */
@Composable
internal fun Excerpt(lines: List<String>, modifier: Modifier = Modifier) {
    val c = Relay.colors
    BasicText(
        lines.joinToString("\n"),
        modifier.fillMaxWidth(),
        style = TextStyle(fontFamily = Fonts.FiraMono, fontSize = 9.sp, lineHeight = 12.sp, color = c.termFg.copy(alpha = .62f)),
        softWrap = false,
        overflow = TextOverflow.Clip,
    )
}
