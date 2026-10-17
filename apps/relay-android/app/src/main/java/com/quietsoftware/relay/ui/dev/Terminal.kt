package com.quietsoftware.relay.ui.dev

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.term.DirectInput
import com.quietsoftware.relay.ui.term.MAX_SP
import com.quietsoftware.relay.ui.term.MIN_SP
import com.quietsoftware.relay.ui.term.TerminalView
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * One agent's terminal, full screen: the plate's identity strip as the header, the screen, a row
 * of the keys a phone keyboard lacks, and a composer. Fitted, the PTY is borrowed at this phone's
 * size while the screen is open (the agent lays itself out for the phone); "Use the PC's width"
 * keeps the PC's columns, scaled to fit or panned sideways. With the PC away it shows the text the
 * phone saved and the composer waits. The agent's own page is its chat (`ChatScreen`); this is
 * the screen under it, for what a conversation cannot show.
 */
@Composable
fun TerminalScreen(name: String, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val session by remember(name) { nav.relay.session(name) }.collectAsStateWithLifecycle(null)
    val feed = rememberFeed(nav.relay, name)
    val status by feed.status.collectAsStateWithLifecycle()
    val life = rememberLifecycle(nav) { nav.back() }
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboard.current
    val focus = remember { FocusRequester() }
    val hold = remember { Hold() }

    val fit = s.prefs.fitTerminal
    val fitting by rememberUpdatedState(fit)
    // Taken from the saved prefs once they are read, so a screen restored before them never writes the default over them.
    var font by remember(s.loaded) { mutableFloatStateOf(s.prefs.terminalFont.coerceIn(MIN_SP, MAX_SP)) }
    var pcFont by remember { mutableStateOf<Float?>(null) }
    var phone by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    var menu by remember { mutableStateOf(false) }
    var mailing by remember { mutableStateOf(false) }
    var text by rememberSaveable(name) { mutableStateOf("") }
    // Typing straight to the terminal, with Ctrl and Alt held for the next key.
    val direct = s.prefs.terminalDirect
    var ctrl by remember { mutableStateOf(false) }
    var alt by remember { mutableStateOf(false) }
    var input by remember { mutableStateOf<DirectInput?>(null) }

    val live = status == TerminalFeed.Status.Live
    val ready = session?.attachable == true && s.online
    val pcCols by produceState(0, feed) {
        feed.version.collect {
            value = feed.pcSize?.first ?: synchronized(feed.lock) { feed.terminal.cols }
            delay(200)
        }
    }

    KeepScreenOn(s.prefs.keepScreenOn)
    // A pinch moves the size many times a second: kept here, written once it settles.
    LaunchedEffect(font, s.loaded) {
        if (!s.loaded) return@LaunchedEffect
        delay(600)
        if (font != nav.shell.state.value.prefs.terminalFont) nav.shell.update { it.copy(terminalFont = font) }
    }
    LaunchedEffect(fit, live, phone) {
        val size = phone
        if (fit && live && size != null) {
            feed.resize(size.first, size.second, borrow = true)
            hold.borrowed = true
        } else if (!fit && hold.borrowed) {
            feed.resize(0, 0, borrow = false)
            hold.borrowed = false
        }
    }
    // The wall may keep the feed streaming after this screen: the PC gets its own size back now.
    DisposableEffect(feed) {
        onDispose {
            if (hold.borrowed) feed.resize(0, 0, borrow = false)
            feed.save()
        }
    }

    fun setDirect(on: Boolean) {
        if (!on) input?.lower()
        ctrl = false
        alt = false
        nav.shell.update { it.copy(terminalDirect = on) }
    }
    // Opened in direct mode, or switched to it: the keyboard comes up for the terminal.
    LaunchedEffect(direct, input, ready) { if (direct && ready) input?.raise() }

    fun zoom(step: Float) {
        if (fit) font = (font + step).coerceIn(MIN_SP, MAX_SP) else pcFont = (hold.shown + step).coerceIn(MIN_SP, MAX_SP)
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            Header(session, name, onBack = nav::back, onDetails = { nav.session(name) }, onMenu = { menu = true }) {
                IconKey("notes", { nav.chat(name) }, size = 18.dp, tint = c.ink2)
            }
            Banner(status, session, s.online, life)
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth().background(c.screen)) {
                val density = LocalDensity.current
                val ratio = rememberMonoRatio()
                val widthPx = constraints.maxWidth.toFloat()
                val shown = if (fit) font else (pcFont ?: fitSp(widthPx, pcCols, ratio, density).takeIf { it > 0f } ?: font).coerceIn(MIN_SP, MAX_SP)
                hold.shown = shown
                // The PC's columns at the smallest size still wider than the phone: pan sideways.
                val needPx = if (fit || pcCols <= 0) 0f else pcCols * ratio * with(density) { shown.sp.toPx() } + with(density) { 12.dp.toPx() } + 2f
                val wide = needPx > widthPx
                Box(Modifier.fillMaxSize().then(if (wide) Modifier.horizontalScroll(rememberScrollState()) else Modifier)) {
                    TerminalView(
                        feed,
                        shown,
                        onFontSp = { if (fitting) font = it else pcFont = it },
                        onFit = { cols, rows -> if (fitting) phone = cols to rows },
                        modifier = if (wide) Modifier.width(with(density) { needPx.toDp() }).fillMaxHeight() else Modifier.fillMaxSize(),
                        onTap = { if (ready) if (direct) input?.raise() else focus.requestFocus() },
                    )
                }
                if (direct && ready) {
                    AndroidView(
                        factory = { ctx ->
                            DirectInput(ctx, feed.terminal, feed.lock, onText = {}, onKey = {}).also { input = it }
                        },
                        modifier = Modifier.size(1.dp),
                        update = { v ->
                            v.onText = { typed ->
                                feed.key(Keys.typed(typed, ctrl, alt))
                                ctrl = false
                                alt = false
                            }
                            v.onKey = { seq -> feed.key(seq) }
                        },
                        onRelease = { input = null },
                    )
                }
            }
            KeyRow(feed, ready, scope, direct, ctrl, alt, onCtrl = { ctrl = !ctrl }, onAlt = { alt = !alt })
            if (direct) DirectBar(ready, onKeyboard = { input?.raise() }, onBox = { setDirect(false) }) else Composer(
                text,
                { text = it },
                ready,
                placeholder = when {
                    session == null -> "This agent is closed"
                    !s.online -> "Needs the PC to type to the agent"
                    !ready -> "The agent is not running"
                    else -> "Type to the agent"
                },
                focus,
                onDirect = { setDirect(true) },
            ) {
                val typed = text
                if (typed.isEmpty()) {
                    feed.key(Keys.ENTER)
                } else {
                    text = ""
                    scope.launch {
                        if (!feed.submit(typed)) {
                            if (text.isEmpty()) text = typed
                            nav.shell.toast("Not sent: the PC did not take it")
                        }
                    }
                }
            }
        }
    }

    LifecycleSheets(life)
    if (menu) {
        val cols = synchronized(feed.lock) { feed.terminal.cols }
        val sub = session?.let { "${it.provider} · ${it.role} · $cols columns" + if (fit && hold.borrowed) ", fitted to this phone" else "" }
        DevSheet({ menu = false }, name, sub) { close ->
            session?.let { ss ->
                SheetRow("Changes", "commit", { close { nav.git(ss.projectId, ss.worktree.ifBlank { null }, name) } })
                if (ss.state != "closed" && !ss.pending) SheetRow("Mail the agent", "mail", { close { mailing = true } })
            }
            SheetRow("Copy text", "copy", {
                close {
                    val all = synchronized(feed.lock) { feed.terminal.plainText(800) }
                    scope.launch {
                        clipboard.copy(all)
                        nav.shell.toast("Copied")
                    }
                }
            })
            SheetRow("Larger text", "plus", { zoom(1f) })
            SheetRow("Smaller text", "minus", { zoom(-1f) })
            SheetRow(
                if (fit) "Use the PC's width" else "Fit to this phone",
                if (fit) "columns" else "phone-install",
                { close { nav.shell.update { it.copy(fitTerminal = !fit) } } },
            )
            SheetRow("Session details", "sliders", { close { nav.session(name) } })
            session?.takeUnless { it.pending }?.let { ss ->
                for (act in actsFor(ss.state)) {
                    SheetRow(act.label, act.glyph, { close { life.run(act, ss) } }, danger = act.danger, dim = !s.online)
                }
            }
        }
    }
    if (mailing) {
        session?.let { ss -> MailSheet(ss, nav) { mailing = false } }
    }
}

/** What outlives a recomposition but is not drawn: whether this screen borrowed the PTY, and the size last drawn. */
private class Hold {
    var borrowed = false
    var shown = 11f
}

@Composable
internal fun KeepScreenOn(on: Boolean) {
    val view = LocalView.current
    DisposableEffect(view, on) {
        view.keepScreenOn = on
        onDispose { view.keepScreenOn = false }
    }
}

// ---- Header and banner ----

/** The plate's identity strip, grown to a page header: back, lamp, name, state, what it is, its branch, ⋯. */
@Composable
internal fun Header(session: Session?, name: String, onBack: () -> Unit, onDetails: () -> Unit, onMenu: () -> Unit, extra: @Composable () -> Unit = {}) {
    val c = Relay.colors
    val held = session?.state == "blocked"
    Column(Modifier.fillMaxWidth().background(c.console).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(end = 2.dp), verticalAlignment = Alignment.CenterVertically) {
            IconKey("arrow-left", onBack, size = 18.dp, tint = c.ink2)
            Column(
                Modifier.weight(1f).clip(Radii.icon).clickable(onClickLabel = "Session details", onClick = onDetails).padding(horizontal = 4.dp, vertical = 6.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                    if (session != null) LampDot(session.lamp, 6.dp)
                    T(name, Modifier.weight(1f, fill = false), Relay.type.plateName, if (held) c.heldText else c.ink, maxLines = 1)
                    session?.let { T(it.stateLabel, style = Relay.type.plateState, color = if (held) c.heldText else c.ink3, maxLines = 1) }
                }
                if (session != null) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        T("${session.provider} · ${session.role}".uppercase(), style = Relay.type.plateMeta, color = c.ink3, maxLines = 1)
                        if (session.branch.isNotBlank()) Branch(session, Modifier.weight(1f, fill = false))
                    }
                }
            }
            extra()
            IconKey("more", onMenu, size = 18.dp, tint = c.ink2)
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
    }
}

/** Where the stream stands when it is not live, with the key that brings a stopped agent back. */
@Composable
internal fun Banner(status: TerminalFeed.Status, session: Session?, online: Boolean, life: Lifecycle) {
    val c = Relay.colors
    val line: String
    val dot: Color?
    when (status) {
        TerminalFeed.Status.Live -> return
        TerminalFeed.Status.Connecting -> {
            line = "Reaching the PC"
            dot = c.waiting
        }
        TerminalFeed.Status.Saved -> {
            line = "Saved on this phone · the PC is away"
            dot = c.inkDim
        }
        TerminalFeed.Status.Stopped -> {
            line = session?.let { "${it.stateLabel} · ${slateHint(it, online)}" } ?: "Closed · this agent is gone from the PC"
            dot = null
        }
    }
    val revive = session?.takeIf { status == TerminalFeed.Status.Stopped && !it.pending }?.let { reviveAct(it.state) }
    Row(
        Modifier.fillMaxWidth().background(c.slab).padding(start = 12.dp, end = 8.dp, top = 6.dp, bottom = 6.dp).heightIn(min = 32.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (dot != null) Dot(dot, 6.dp) else session?.let { LampDot(it.lamp, 6.dp) }
        T(line, Modifier.weight(1f), Relay.type.caption, c.ink2, maxLines = 2)
        if (revive != null && session != null) {
            Key(
                if (revive == Act.Start) "Start session" else revive.label,
                { life.run(revive, session) },
                Modifier.alpha(if (online) 1f else .5f),
                kind = KeyKind.Primary,
                glyph = revive.glyph,
                enabled = life.busy == null,
                compact = true,
            )
        }
    }
    Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
}

// ---- Keys and composer ----

/** A key a phone keyboard lacks and an agent CLI keeps asking for. */
private class TermKey(val label: String, val spoken: String, val send: (TerminalFeed, CoroutineScope) -> Unit)

/** Arrows honour the program's cursor mode; `y` and `n` answer a prompt, so Enter follows them. */
private val TERM_KEYS = listOf(
    TermKey("Esc", "Escape") { f, _ -> f.key(Keys.ESC) },
    TermKey("Tab", "Tab") { f, _ -> f.key(Keys.TAB) },
    TermKey("⇧Tab", "Shift Tab") { f, _ -> f.key(Keys.SHIFT_TAB) },
    TermKey("↑", "Up") { f, _ -> f.key(synchronized(f.lock) { Keys.arrowUp(f.terminal) }) },
    TermKey("↓", "Down") { f, _ -> f.key(synchronized(f.lock) { Keys.arrowDown(f.terminal) }) },
    TermKey("←", "Left") { f, _ -> f.key(synchronized(f.lock) { Keys.arrowLeft(f.terminal) }) },
    TermKey("→", "Right") { f, _ -> f.key(synchronized(f.lock) { Keys.arrowRight(f.terminal) }) },
    TermKey("⏎", "Enter") { f, _ -> f.key(Keys.ENTER) },
    TermKey("^C", "Control C") { f, _ -> f.key(Keys.CTRL_C) },
    TermKey("y", "Yes") { f, scope -> scope.launch { f.submit("y") } },
    TermKey("n", "No") { f, scope -> scope.launch { f.submit("n") } },
    TermKey("/", "Slash") { f, _ -> f.key("/") },
    TermKey("^D", "Control D") { f, _ -> f.key(Keys.CTRL_D) },
)

/** Keys a full keyboard has for a pager or an editor, offered while typing directly. */
private val PAGE_KEYS = listOf(
    TermKey("PgUp", "Page up") { f, _ -> f.key(Keys.PAGE_UP) },
    TermKey("PgDn", "Page down") { f, _ -> f.key(Keys.PAGE_DOWN) },
    TermKey("Home", "Home") { f, _ -> f.key(synchronized(f.lock) { Keys.home(f.terminal) }) },
    TermKey("End", "End") { f, _ -> f.key(synchronized(f.lock) { Keys.end(f.terminal) }) },
)

/**
 * The keys a phone keyboard lacks. Typing directly, Ctrl and Alt lead the row: tapped, they hold
 * for the next key typed (Ctrl then c is ^C), lit while they do; the page keys join the end.
 */
@Composable
private fun KeyRow(feed: TerminalFeed, enabled: Boolean, scope: CoroutineScope, direct: Boolean, ctrl: Boolean, alt: Boolean, onCtrl: () -> Unit, onAlt: () -> Unit) {
    val c = Relay.colors
    val face = remember { TextStyle(fontFamily = Fonts.FiraMono, fontSize = 14.sp) }
    @Composable
    fun Cap(label: String, spoken: String, lit: Boolean = false, onClick: () -> Unit) {
        Box(
            Modifier
                .height(40.dp)
                .widthIn(min = 44.dp)
                .alpha(if (enabled) 1f else .4f)
                .clip(Radii.keycap)
                .background(if (lit) c.ink else c.slab)
                .border(1.dp, if (lit) c.ink else c.strong, Radii.keycap)
                .clickable(enabled = enabled, onClickLabel = spoken, role = Role.Button, onClick = onClick)
                .padding(horizontal = 10.dp),
            contentAlignment = Alignment.Center,
        ) {
            T(label, style = face, color = if (lit) c.wall else c.ink, maxLines = 1)
        }
    }
    Row(
        Modifier.fillMaxWidth().background(c.wall).horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (direct) {
            Cap("Ctrl", if (ctrl) "Control, held for the next key" else "Control for the next key", ctrl, onCtrl)
            Cap("Alt", if (alt) "Alt, held for the next key" else "Alt for the next key", alt, onAlt)
        }
        for (k in TERM_KEYS) Cap(k.label, k.spoken) { k.send(feed, scope) }
        if (direct) for (k in PAGE_KEYS) Cap(k.label, k.spoken) { k.send(feed, scope) }
    }
}

/** In place of the message box while typing directly: what is happening, the keyboard, and the way back. */
@Composable
private fun DirectBar(enabled: Boolean, onKeyboard: () -> Unit, onBox: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding().padding(start = 14.dp, end = 8.dp, bottom = 8.dp).heightIn(min = 44.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        KeyboardGlyph(16.dp, c.ink3)
        T(if (enabled) "Keys go straight to the terminal" else "Needs the agent running", Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 2)
        Key("Keyboard", onKeyboard, kind = KeyKind.Plain, enabled = enabled, compact = true)
        Key("Message box", onBox, kind = KeyKind.Quiet, compact = true)
    }
}

/** Up to four lines and one round key: with text it sends and submits; empty, it is Enter. */
@Composable
private fun Composer(text: String, onText: (String) -> Unit, enabled: Boolean, placeholder: String, focus: FocusRequester, onDirect: () -> Unit, onSend: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).navigationBarsPadding().padding(start = 2.dp, end = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(
            Modifier.size(TOUCH).alpha(if (enabled) 1f else .4f).clip(Radii.icon)
                .clickable(enabled = enabled, onClickLabel = "Type straight to the terminal", role = Role.Button, onClick = onDirect),
            contentAlignment = Alignment.Center,
        ) {
            KeyboardGlyph(20.dp, c.ink2)
        }
        Field(
            text,
            onText,
            Modifier.weight(1f).alpha(if (enabled) 1f else .6f).focusRequester(focus).focusProperties { canFocus = enabled },
            placeholder = placeholder,
            singleLine = false,
            maxLines = 4,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
        )
        Box(
            Modifier
                .size(44.dp)
                .alpha(if (enabled) 1f else .4f)
                .clip(CircleShape)
                .background(c.ink)
                .clickable(enabled = enabled, onClickLabel = if (text.isEmpty()) "Enter" else "Send", role = Role.Button, onClick = onSend),
            contentAlignment = Alignment.Center,
        ) {
            if (text.isEmpty()) ReturnGlyph(18.dp, c.wall) else Glyph("arrow-up", 18.dp, c.wall)
        }
    }
}

/** A keyboard on the glyphs' 16-unit grid: its case, two rows of keys and the space bar. */
@Composable
internal fun KeyboardGlyph(size: Dp, tint: Color) {
    Box(
        Modifier.size(size).drawWithCache {
            val u = this.size.minDimension / 16f
            val stroke = Stroke(width = 1.3f * u, cap = StrokeCap.Round, join = StrokeJoin.Round)
            val case = Path().apply {
                addRoundRect(androidx.compose.ui.geometry.RoundRect(1.5f * u, 3.5f * u, 14.5f * u, 12.5f * u, 1.6f * u, 1.6f * u))
            }
            val keys = Path().apply {
                for (row in listOf(6.2f, 8.2f)) for (x in listOf(4.25f, 6.75f, 9.25f, 11.75f)) {
                    moveTo(x * u, row * u)
                    lineTo(x * u + 0.01f, row * u)
                }
                moveTo(5.5f * u, 10.4f * u)
                lineTo(10.5f * u, 10.4f * u)
            }
            onDrawBehind {
                drawPath(case, tint, style = stroke)
                drawPath(keys, tint, style = stroke)
            }
        },
    )
}

/** ⏎ on the glyphs' 16-unit grid: down from the top right, then left to an arrowhead. */
@Composable
internal fun ReturnGlyph(size: Dp, tint: Color) {
    Box(
        Modifier.size(size).drawWithCache {
            val u = this.size.minDimension / 16f
            val path = Path().apply {
                moveTo(13f * u, 3.5f * u)
                lineTo(13f * u, 9.5f * u)
                lineTo(3.5f * u, 9.5f * u)
                moveTo(6.5f * u, 6.5f * u)
                lineTo(3.5f * u, 9.5f * u)
                lineTo(6.5f * u, 12.5f * u)
            }
            val stroke = Stroke(width = 1.4f * u, cap = StrokeCap.Round, join = StrokeJoin.Round)
            onDrawBehind { drawPath(path, tint, style = stroke) }
        },
    )
}

// ---- Mail ----

/**
 * Priority mail reaches an agent that is busy: the engine hands it over at the agent's next bus
 * call, where typed text would only sit in the PTY until it reads its prompt.
 */
@Composable
internal fun MailSheet(session: Session, nav: Nav, onDismiss: () -> Unit) {
    var body by rememberSaveable(session.name) { mutableStateOf("") }
    DevSheet(onDismiss, "Mail ${session.name}", "Delivered as priority mail: the agent reads it at its next step, even while busy. For an agent waiting at its prompt, typing below is quicker.") { close ->
        Field(body, { body = it }, Modifier.fillMaxWidth().padding(horizontal = 6.dp), placeholder = "Also update the changelog when you are done.", singleLine = false, minLines = 3, maxLines = 8)
        SheetKeys(
            "Send",
            {
                val text = body.trim()
                close {
                    nav.shell.change(
                        "mailbox.send",
                        buildJsonObject {
                            put("project_id", session.projectId)
                            put("to", session.name)
                            put("text", text)
                            put("priority", true)
                        },
                        "Mail ${session.name}",
                    ) { change -> if (change is Hub.Change.Now || nav.shell.state.value.online) nav.shell.toast("Mailed ${session.name}") }
                }
            },
            { close {} },
            enabled = body.isNotBlank(),
        )
    }
}
