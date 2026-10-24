package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Event
import com.quietsoftware.relay.core.wire.Frame
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Incoming
import com.quietsoftware.relay.core.wire.Response
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

/**
 * One admitted connection: bus requests out, responses matched back by id, events and data-plane
 * frames handed to whoever listens. It ends when the socket does; the [Link] above dials again.
 */
class BusSession internal constructor(
    private val socket: LineSocket,
    val route: Route,
    val greeting: Greeting,
    val device: String,
    private val scope: CoroutineScope,
) {
    private val pending = ConcurrentHashMap<String, CompletableDeferred<Response>>()
    private val closed = AtomicBoolean(false)

    private val _events = MutableSharedFlow<Event>(extraBufferCapacity = EVENT_BUFFER)
    private val _frames = MutableSharedFlow<Frame>(extraBufferCapacity = FRAME_BUFFER)

    /** Events after `bus.subscribe`. A reader too slow to keep up is told with a local `bus.lagged`. */
    val events: SharedFlow<Event> = _events

    /** `pty` frames and other data-plane lines. A terminal detects a gap from `seq` and re-attaches. */
    val frames: SharedFlow<Frame> = _frames

    /** Completes when the socket is gone, for any reason. */
    val ended = CompletableDeferred<Unit>()

    private var lostEvents = 0

    private val reader: Job = scope.launch {
        try {
            for (line in socket.incoming) {
                when (val m = Wire.classify(line)) {
                    is Incoming.Res -> m.response.id?.let { pending.remove(it)?.complete(m.response) }
                    is Incoming.Ev -> deliver(m.event)
                    is Incoming.Fr -> _frames.tryEmit(m.frame)
                    null -> Unit
                }
            }
        } finally {
            shut()
        }
    }

    private var lagging: Job? = null

    private fun deliver(event: Event) = synchronized(this) {
        tellLost(event.ts)
        if (!_events.tryEmit(event)) {
            lostEvents++
            // A burst that overflows and then goes quiet has no later event to carry the news:
            // tell it as soon as the reader catches up.
            if (lagging?.isActive != true) lagging = scope.launch {
                while (synchronized(this@BusSession) { lostEvents > 0 && !tellLost(event.ts) }) delay(LAGGED_RETRY_MS)
            }
        }
    }

    /** A `bus.lagged` for the events dropped so far, if there is room for it now; true once none are owed. Under the lock. */
    private fun tellLost(ts: String): Boolean {
        if (lostEvents == 0) return true
        val lagged = Event("bus.lagged", ts, "system", null, null, JsonObject(mapOf("dropped" to kotlinx.serialization.json.JsonPrimitive(lostEvents))))
        if (!_events.tryEmit(lagged)) return false
        lostEvents = 0
        return true
    }

    val isOpen: Boolean get() = !closed.get()

    /**
     * Send one request and wait for its answer. [id] is the idempotency key (BUS.md §5.3): a
     * mutation resent with the same id after a timeout runs once, and the second answer says
     * `replayed`. Throws [BusException] with the engine's typed error, or `link.timeout` /
     * `link.down` when the answer never came.
     */
    suspend fun call(op: String, payload: JsonObject = JsonObject(emptyMap()), id: String = UUID.randomUUID().toString(), timeoutMs: Long = DEFAULT_TIMEOUT_MS): Response {
        if (closed.get()) throw BusException(BusError.link("link.down", "Not connected to the PC"))
        val answer = CompletableDeferred<Response>()
        pending[id] = answer
        if (!socket.send(Wire.request(op, payload, id))) {
            pending.remove(id)
            throw BusException(BusError.link("link.down", "Not connected to the PC"))
        }
        val response = withTimeoutOrNull(timeoutMs) { answer.await() }
        pending.remove(id)
        return response ?: throw BusException(BusError.link("link.timeout", "The PC did not answer $op in time"))
    }

    /** [call], returning the result or throwing the engine's error. */
    suspend fun result(op: String, payload: JsonObject = JsonObject(emptyMap()), id: String = UUID.randomUUID().toString(), timeoutMs: Long = DEFAULT_TIMEOUT_MS): JsonElement {
        val r = call(op, payload, id, timeoutMs)
        if (!r.ok) throw BusException(r.error ?: BusError("internal", "bus.no_error", "The PC refused without saying why"))
        return r.result
    }

    /** Fire and forget, for keystrokes: the answer is read and dropped. */
    fun post(op: String, payload: JsonObject): Boolean = !closed.get() && socket.send(Wire.request(op, payload))

    fun close() {
        socket.close()
        shut()
    }

    private fun shut() {
        if (!closed.compareAndSet(false, true)) return
        socket.close()
        val down = Response(null, false, kotlinx.serialization.json.JsonNull, BusError.link("link.down", "The link to the PC closed"), false)
        pending.values.forEach { it.complete(down) }
        pending.clear()
        ended.complete(Unit)
    }

    companion object {
        const val DEFAULT_TIMEOUT_MS = 30_000L
        const val EVENT_BUFFER = 4096
        const val FRAME_BUFFER = 4096
        /** How often a lagged marker that found no room is tried again. */
        const val LAGGED_RETRY_MS = 50L
    }
}
