package com.quietsoftware.relay.data

import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.model.ThreadMessage
import com.quietsoftware.relay.core.sync.QueryKey
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Wire
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.time.Instant
import java.util.UUID

/**
 * One terminal agent as a conversation (`session.transcript`): the messages its provider logged,
 * read from the PC, then followed while the screen is open. Nothing streams: the PC answers what
 * came after a cursor, read again about every second while the agent works and less often while
 * it waits. What the person types still goes to the agent's terminal (`session.input`), as the
 * terminal screen sends it. With the PC away it shows the conversation the phone saved.
 */
class SessionChat(private val relay: Relay, val name: String, private val scope: CoroutineScope) {
    private val _messages = MutableStateFlow<List<ThreadMessage>>(emptyList())
    val messages: StateFlow<List<ThreadMessage>> = _messages

    private val _status = MutableStateFlow(TerminalFeed.Status.Connecting)
    val status: StateFlow<TerminalFeed.Status> = _status

    /** False when the PC has no conversation for this agent yet (or never will: no hooks ran). */
    private val _found = MutableStateFlow<Boolean?>(null)
    val found: StateFlow<Boolean?> = _found

    /** What the person sent that the log does not show yet: drawn as a bubble marked sending. */
    private val _sending = MutableStateFlow<List<ThreadMessage>>(emptyList())
    val sending: StateFlow<List<ThreadMessage>> = _sending

    /** Why the PC would not answer, while it would not: an engine too old for the op says so here. */
    private val _error = MutableStateFlow<BusException?>(null)
    val error: StateFlow<BusException?> = _error

    /** For each message being sent, the person's messages the log already held when it went. */
    private val before = java.util.concurrent.ConcurrentHashMap<String, Set<String>>()
    private var cursor: Long? = null
    private var at = 0L
    private var job: Job? = null
    private val sendLock = Mutex()
    private val first = buildJsonObject { put("session", name); put("limit", LIMIT) }

    fun start() {
        if (job?.isActive == true) return
        job = scope.launch(Dispatchers.Default) {
            // Shown at once, while the PC is asked: the link decides whether it is still "saved".
            relay.hub.cached(OP, first)?.let { show(it.result, replace = true); at = it.at }
            combine(relay.state, relay.hub.session, relay.session(name)) { link, bus, s -> Triple(link is LinkState.Online && bus != null, s?.attachable == true, s?.state == "running" || s?.state == "spawning") }
                .distinctUntilChanged()
                .collectLatest { (online, attachable, busy) ->
                    if (!online) {
                        if (_status.value != TerminalFeed.Status.Connecting || _messages.value.isNotEmpty()) _status.value = TerminalFeed.Status.Saved
                        return@collectLatest
                    }
                    // Kept while it runs: the link and the agent's state are what restart it.
                    while (true) {
                        try {
                            read()
                            _error.value = null
                            _status.value = if (attachable) TerminalFeed.Status.Live else TerminalFeed.Status.Stopped
                        } catch (e: BusException) {
                            // The PC is there and refused this read: what the agent is doing still stands.
                            _error.value = e
                            _status.value = if (attachable) TerminalFeed.Status.Live else TerminalFeed.Status.Stopped
                        }
                        delay(if (busy || _sending.value.isNotEmpty()) BUSY_MS else if (attachable) WAITING_MS else STOPPED_MS)
                    }
                }
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        save()
    }

    private suspend fun read() {
        val after = cursor
        val payload = if (after == null) first else buildJsonObject { put("session", name); put("after", after); put("limit", LIMIT) }
        val result = relay.call(OP, payload) as? JsonObject ?: return
        at = System.currentTimeMillis()
        show(result, replace = after == null || (result["reset"] as? JsonPrimitive)?.booleanOrNull == true)
    }

    private fun show(result: JsonElement, replace: Boolean) {
        val o = result as? JsonObject ?: return
        _found.value = (o["found"] as? JsonPrimitive)?.booleanOrNull ?: true
        cursor = (o["cursor"] as? JsonPrimitive)?.longOrNull
        val got = (o["messages"] as? JsonArray).orEmpty().mapNotNull { runCatching { Wire.json.decodeFromJsonElement<ThreadMessage>(it) }.getOrNull() }
        if (!replace && got.isEmpty()) return
        val merged = if (replace) got else (_messages.value + got).distinctBy { it.id }
        _messages.value = merged.takeLast(KEEP)
        // A sent message the log now has, or one long overdue, stops waiting.
        val now = System.currentTimeMillis()
        val users = merged.filter { it.role == "user" }
        _sending.update { list ->
            list.filter { mine ->
                // Only a message logged since this one was sent can be it: a picture alone has no words to tell it by.
                val known = before[mine.id].orEmpty()
                val said = users.any { it.id !in known && gist(it.text) == gist(mine.text) }
                !said && now - Instant.parse(mine.createdAt).toEpochMilli() < SENDING_MS
            }
        }
        before.keys.retainAll(_sending.value.mapTo(HashSet()) { it.id })
    }

    /** A message as typed, whichever way its provider logged the pictures it named (a path, or `[Image #1]`). */
    private fun gist(text: String): String =
        text.lines().filterNot { ATTACHED_LINE in it }.joinToString("\n").replace(IMAGE_TAG, "").trim()

    /** Keep the conversation for when the PC is away: the newest part, as the first read's answer. */
    fun save() {
        val list = _messages.value
        if (list.isEmpty()) return
        val c = cursor ?: return
        val json = buildJsonObject {
            put("messages", JsonArray(list.takeLast(LIMIT).map { Wire.json.encodeToJsonElement(ThreadMessage.serializer(), it) }))
            put("cursor", c)
            put("reset", false)
            put("found", true)
        }
        val when_ = at
        scope.launch(Dispatchers.Default) { runCatching { relay.hub.store.putQuery(QueryKey.of(OP, first), json.toString(), when_) } }
    }

    // ---- Input ----

    /** Raw keys (Esc, an arrow, a menu's number): sent in order, unanswered. */
    fun key(data: String) {
        val s = relay.hub.session.value ?: return
        scope.launch { sendLock.withLock { s.post("session.input", input(data)) } }
    }

    /**
     * A message to the agent: typed into its terminal, then Enter as a separate write so the TUI
     * does not take the two for a paste. Several lines go as one bracketed paste, which both
     * Claude Code and Codex ask for.
     */
    suspend fun send(text: String): Boolean {
        val s = relay.hub.session.value ?: return false
        val mine = ThreadMessage("tmp:${UUID.randomUUID()}", role = "user", body = buildJsonObject { put("text", text) }, createdAt = Instant.now().toString())
        before[mine.id] = _messages.value.filter { it.role == "user" }.mapTo(HashSet()) { it.id }
        _sending.update { it + mine }
        sendLock.withLock {
            try {
                val typed = if (text.contains('\n')) "$PASTE_START$text$PASTE_END" else text
                s.result("session.input", input(typed), timeoutMs = 5_000)
                delay(TerminalFeed.ENTER_DELAY_MS)
                s.result("session.input", input(Keys.ENTER), timeoutMs = 5_000)
            } catch (e: BusException) {
                _sending.update { list -> list.filter { it.id != mine.id } }
                return false
            }
        }
        return true
    }

    private fun input(data: String) = buildJsonObject { put("session", name); put("data", data) }

    /** A picture for the agent, into its checkout (`session.upload`): the absolute path a message names, or null when the PC would not take it. */
    suspend fun upload(picture: ByteArray): String? = try {
        val payload = buildJsonObject {
            put("session", name)
            put("bytes_b64", android.util.Base64.encodeToString(picture, android.util.Base64.NO_WRAP))
        }
        ((relay.call("session.upload", payload) as? JsonObject)?.get("absolute") as? JsonPrimitive)?.content
    } catch (e: BusException) {
        null
    }

    companion object {
        const val OP = "session.transcript"
        const val LIMIT = 300
        private const val KEEP = 1000
        private const val BUSY_MS = 1_000L
        private const val WAITING_MS = 3_000L
        private const val STOPPED_MS = 15_000L
        private const val SENDING_MS = 60_000L
        private const val PASTE_START = "\u001b[200~"
        private const val PASTE_END = "\u001b[201~"
        private const val ATTACHED_LINE = ".relay/attachments/"
        private val IMAGE_TAG = Regex("""\[Image #\d+]""")
    }
}
