package com.quietsoftware.relay.data

import android.util.Base64
import com.quietsoftware.relay.core.link.BusSession
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PtySequence
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.core.term.Terminal
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * One agent's terminal on the phone (BUS.md §7): attached to the PC's PTY, rebuilt from the
 * engine's replay, then fed live. It keeps its place by (epoch, seq): a link that drops comes
 * back from the last frame seen, a gap re-attaches, a new epoch (the agent restarted) starts the
 * screen afresh. With the PC away it shows the last text the phone saved.
 *
 * The [terminal] is fed and read under [lock]: frames arrive on a background thread, the screen
 * is drawn on the main one.
 */
class TerminalFeed(private val relay: Relay, val name: String, private val scope: CoroutineScope) {
    val terminal = Terminal(DEFAULT_COLS, DEFAULT_ROWS, scrollbackLimit = 5000)
    val lock = Any()

    enum class Status {
        /** Reaching the PC or attaching. */
        Connecting,

        /** Streaming from the PC. */
        Live,

        /** The agent is not running; its last output is shown. */
        Stopped,

        /** The PC is away; this is the text the phone saved. */
        Saved,
    }

    private val _status = MutableStateFlow(Status.Connecting)
    val status: StateFlow<Status> = _status

    private val _version = MutableStateFlow(0L)

    /** Bumps whenever the screen changed; the view redraws on it. */
    val version: StateFlow<Long> = _version

    /** The PC's own size for this terminal, when the engine said it: what "Use the PC's width" returns to. */
    @Volatile var pcSize: Pair<Int, Int>? = null
        private set

    private val sequence = PtySequence()

    /** The screen holds saved text, not the stream: the first frame fed replaces it. */
    private var fresh = true
    private var job: Job? = null
    private var borrowed: Pair<Int, Int>? = null
    private val sendLock = Mutex()

    fun start() {
        if (job?.isActive == true) return
        job = scope.launch(Dispatchers.Default) {
            showSaved()
            // Keyed on the connection and whether the agent runs, not on the whole row: every
            // `session.changed` (each keystroke, each output) would otherwise re-attach and re-probe.
            combine(relay.state, relay.hub.session, relay.session(name)) { link, bus, s -> Triple(link, bus, s?.attachable == true) }
                .distinctUntilChanged()
                .collectLatest { (link, session, attachable) ->
                    when {
                        link !is LinkState.Online || session == null -> _status.value = if (_status.value == Status.Live) Status.Saved else _status.value.takeIf { it != Status.Connecting } ?: Status.Saved
                        !attachable -> {
                            readScrollback(session)
                            _status.value = Status.Stopped
                        }
                        else -> attach(session)
                    }
                }
        }
    }

    /** Leave: the PC stops streaming, and a borrowed size goes back to it. */
    fun stop() {
        job?.cancel()
        job = null
        relay.hub.session.value?.let { s ->
            if (_status.value == Status.Live) s.post("session.detach", buildJsonObject { put("session", name) })
        }
        borrowed = null
    }

    /**
     * Attach, and again a little later when the PC refuses for any reason but the agent not
     * running (a timeout, a busy engine): a new link or a change of state cancels the wait.
     */
    private suspend fun attach(session: BusSession) {
        while (!attachOnce(session)) delay(ATTACH_RETRY_MS)
    }

    /** False when the attach was refused and is worth trying again. */
    private suspend fun attachOnce(session: BusSession): Boolean {
        _status.value = Status.Connecting
        probeSize(session)
        borrowed?.let { (c, r) -> resizeOn(session, c, r, borrow = true) }
        // Frames may arrive before the answer: start listening first.
        val frames = scope.launch(Dispatchers.Default) {
            // One re-attach at a time: every frame after a gap is a gap until the replay arrives.
            var reattach: Job? = null
            session.frames.filter { it.stream == "pty" && it.session == name }.collect { f ->
                val bytes = (f.data as? JsonPrimitive)?.content?.let { runCatching { Base64.decode(it, Base64.DEFAULT) }.getOrNull() } ?: return@collect
                val step = synchronized(lock) {
                    val step = sequence.observe(f.epoch ?: 0, f.seq)
                    when (step) {
                        PtySequence.Step.Reset -> {
                            terminal.reset()
                            terminal.feed(bytes)
                        }
                        PtySequence.Step.Feed -> {
                            // The first frame after showing saved text is the whole replay.
                            if (fresh) terminal.reset()
                            fresh = false
                            saved = null
                            terminal.feed(bytes)
                        }
                        else -> Unit
                    }
                    step
                }
                when (step) {
                    // A child of this collector, so it ends with the stream; a refusal here must not crash the app.
                    PtySequence.Step.Gap -> if (reattach?.isActive != true) {
                        reattach = launch {
                            try {
                                requestAttach(session)
                            } catch (_: BusException) {
                            }
                        }
                    }
                    PtySequence.Step.Duplicate -> Unit
                    else -> {
                        _version.value++
                        _status.value = Status.Live
                    }
                }
            }
        }
        try {
            requestAttach(session)
            _status.value = Status.Live
            session.ended.await()
        } catch (e: BusException) {
            if (e.error.code != "session.not_spawned" && e.error.code != "session.exited") return false
            readScrollback(session)
            _status.value = Status.Stopped
        } finally {
            frames.cancel()
        }
        return true
    }

    /**
     * Attach from the last frame fed (the engine sends what followed), or from the start when
     * nothing is: a gap and a new link both come here.
     */
    private suspend fun requestAttach(session: BusSession) {
        val last = synchronized(lock) {
            sequence.catchUp = true
            sequence.last
        }
        session.result("session.attach", buildJsonObject {
            put("session", name)
            last?.let { (e, q) -> put("epoch", e); put("from_seq", q) }
        })
    }

    private suspend fun probeSize(session: BusSession) {
        val out = runCatching { session.result("session.scrollback", buildJsonObject { put("session", name); put("lines", 1) }) as? JsonObject }.getOrNull() ?: return
        val c = out.l("cols")?.toInt()
        val r = out.l("rows")?.toInt()
        if (c != null && r != null && c > 0 && r > 0) {
            pcSize = c to r
            if (borrowed == null) synchronized(lock) { terminal.resize(c, r) }
            _version.value++
        }
    }

    /** The agent is not running: its last output, also kept for when the PC is away. */
    private suspend fun readScrollback(session: BusSession) {
        val payload = buildJsonObject { put("session", name); put("lines", SAVED_LINES) }
        val out = runCatching { relay.hub.query("session.scrollback", payload).result as? JsonObject }.getOrNull()
            ?: runCatching { session.result("session.scrollback", payload) as? JsonObject }.getOrNull()
            ?: return
        showText(out.s("text").orEmpty())
    }

    /** The last text the phone saved for this terminal, shown before anything else arrives. */
    private suspend fun showSaved() {
        val saved = relay.hub.cached("session.scrollback", buildJsonObject { put("session", name); put("lines", SAVED_LINES) })
        val text = (saved?.result as? JsonObject)?.s("text") ?: return
        if (_version.value == 0L) showText(text)
    }

    /** The last saved text shown, kept so a new size can wrap it again. */
    @Volatile private var saved: String? = null

    private fun showText(text: String) {
        saved = text
        synchronized(lock) {
            terminal.reset()
            terminal.feed(text.replace("\r\n", "\n").replace("\n", "\r\n"))
            // Saved text has no program behind it, so no cursor either.
            terminal.feed("\u001b[?25l")
            sequence.forget()
            fresh = true
        }
        _version.value++
    }

    /** Save the current screen's text so it opens with the PC away. Called when the person leaves. */
    fun save() {
        val text = synchronized(lock) { terminal.plainText(SAVED_LINES) }
        if (text.isBlank()) return
        scope.launch {
            relay.hub.store.putQuery(
                com.quietsoftware.relay.core.sync.QueryKey.of("session.scrollback", buildJsonObject { put("session", name); put("lines", SAVED_LINES) }),
                buildJsonObject { put("text", text) }.toString(),
                System.currentTimeMillis(),
            )
        }
    }

    // ---- Input ----

    /** Raw keys (a key-row tap, a typed character): sent in order, unanswered. */
    fun key(data: String) {
        val s = relay.hub.session.value ?: return
        scope.launch { sendLock.withLock { s.post("session.input", input(data)) } }
    }

    /**
     * The composer's text: the text, then Enter as a separate write, so a TUI does not take the
     * two for a paste. Several lines go as one bracketed paste when the program asked for it.
     */
    suspend fun submit(text: String): Boolean {
        val s = relay.hub.session.value ?: return false
        sendLock.withLock {
            try {
                when {
                    text.isEmpty() -> s.post("session.input", input(Keys.ENTER))
                    text.contains('\n') && synchronized(lock) { terminal.bracketedPaste } -> {
                        s.result("session.input", input(synchronized(lock) { Keys.paste(terminal, text) }), timeoutMs = 5_000)
                        delay(ENTER_DELAY_MS)
                        s.post("session.input", input(Keys.ENTER))
                    }
                    // No bracketed paste: one line at a time, each with its own Enter (TerminalInput.ts).
                    text.contains('\n') -> {
                        for (line in text.lines()) {
                            if (line.isNotEmpty()) {
                                s.result("session.input", input(line), timeoutMs = 5_000)
                                delay(ENTER_DELAY_MS)
                            }
                            s.result("session.input", input(Keys.ENTER), timeoutMs = 5_000)
                            delay(ENTER_DELAY_MS)
                        }
                    }
                    else -> {
                        s.result("session.input", input(text), timeoutMs = 5_000)
                        delay(ENTER_DELAY_MS)
                        s.post("session.input", input(Keys.ENTER))
                    }
                }
            } catch (e: BusException) {
                return false
            }
        }
        return true
    }

    private fun input(data: String) = buildJsonObject { put("session", name); put("data", data) }

    // ---- Size ----

    /**
     * Size the PTY for this phone (`until_detach`: the PC's size comes back when the phone
     * leaves or the link drops), or hand it back to the PC's size.
     */
    fun resize(cols: Int, rows: Int, borrow: Boolean) {
        // The PC's size unknown: at least stop borrowing again on the next attach.
        val target = if (borrow) cols to rows else pcSize ?: run { borrowed = null; return }
        if (borrow && borrowed == target) return
        borrowed = if (borrow) target else null
        synchronized(lock) { terminal.resize(target.first, target.second) }
        // Saved text has no program to redraw it at the new width: wrap it again here.
        val text = saved
        if (text != null && _status.value != Status.Live) showText(text)
        _version.value++
        val s = relay.hub.session.value ?: return
        scope.launch { resizeOn(s, target.first, target.second, borrow) }
    }

    private suspend fun resizeOn(s: BusSession, cols: Int, rows: Int, borrow: Boolean) {
        runCatching {
            s.result("session.resize", buildJsonObject {
                put("session", name)
                put("cols", cols)
                put("rows", rows)
                if (borrow) put("until_detach", true)
            })
        }
    }

    companion object {
        const val DEFAULT_COLS = 120
        const val DEFAULT_ROWS = 40
        const val SAVED_LINES = 400
        /** Between a line and its Enter, so a TUI reads them as typing (TerminalInput.ts). */
        const val ENTER_DELAY_MS = 90L
        private const val ATTACH_RETRY_MS = 2_000L
    }
}
