package com.quietsoftware.relay.core

import com.quietsoftware.relay.core.link.BusSession
import com.quietsoftware.relay.core.link.Dialer
import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.sync.Ledger
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.OutboxRunner
import com.quietsoftware.relay.core.sync.OutboxStore
import com.quietsoftware.relay.core.sync.QueryKey
import com.quietsoftware.relay.core.sync.ReplicaStore
import com.quietsoftware.relay.core.sync.Syncer
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Everything between the screens and the PC: the link, the replica, the outbox, and the rule
 * that decides which of them a request goes through.
 *
 * - **Reads** come from the replica, which the [Syncer] keeps in step while the PC is in reach.
 *   A query that is not an entity is cached by its op and payload, so a screen opened before
 *   shows its last answer with the PC away.
 * - **Edits to the PC's data** go through the outbox ([change]): shown at once, sent now if the
 *   PC is in reach, later if not, exactly once either way.
 * - **Acts** that only make sense with the PC there (typing into a terminal, allowing a held
 *   command, pushing) go straight to it ([call]) and fail with `link.down` when it is away.
 * - **The server's copy** (docs/CLOUD.md), when one is paired ([cloudProfiles]), stands in for
 *   the PC on the reads it holds ([CLOUD_READS]: the ledger and the training history) while the
 *   PC is away. Nothing else is ever sent to it: no resync, no outbox, no act.
 */
class Hub(
    private val scope: CoroutineScope,
    dialer: Dialer,
    profiles: Link.ProfileSource,
    val store: ReplicaStore,
    val outbox: OutboxStore,
    private val now: () -> Long = System::currentTimeMillis,
    cloudProfiles: Link.ProfileSource? = null,
    private val cloudGraceMs: Long = CLOUD_GRACE_MS,
) {
    val ledger = Ledger(store, outbox, now)
    val link = Link(scope, dialer, profiles, now)
    val syncer = Syncer(scope, ledger, store, now)

    /**
     * The second link, to the server's copy: data only, dialled while the PC link is not online
     * (after [cloudGraceMs], so a PC that answers at once costs the server nothing), and stopped
     * as soon as it is.
     */
    val cloud = Link(scope, dialer, cloudProfiles ?: NoCloud, now, Wire.OPS_CLOUD)

    private val cloudStale = MutableSharedFlow<Set<String>>(extraBufferCapacity = 64)

    /** Query domains gone stale on the PC ([Syncer.stale]) or on the server's copy: open screens read theirs again. */
    val stale: Flow<Set<String>> = merge(syncer.stale, cloudStale)

    /** The server's reads asked for in this process, by key, for [catchUpFromCloud]. */
    private val cloudAsked = ConcurrentHashMap<String, Pair<String, JsonObject>>()
    private val standingIn = AtomicBoolean(false)

    private val _parked = MutableSharedFlow<OutboxEntry>(extraBufferCapacity = 16)

    /** An outbox entry the PC refused or held: the screens say so, the person decides. */
    val parked: SharedFlow<OutboxEntry> = _parked

    private val _outboxChanged = MutableStateFlow(0L)

    /** Bumps whenever the outbox moves, for the status bar's count. */
    val outboxChanged: StateFlow<Long> = _outboxChanged

    private val runner = OutboxRunner(
        outbox,
        onApplied = { e, r ->
            ledger.applied(e, r)
            _outboxChanged.value++
        },
        onParked = {
            _parked.tryEmit(it)
            _outboxChanged.value++
        },
        now = now,
    )
    private val flushing = Mutex()

    val state: StateFlow<LinkState> get() = link.state
    val session: StateFlow<BusSession?> get() = link.session

    init {
        link.onSession = { s ->
            val holds = scope.launch { s.events.collect { if (it.ev == "guardrail.resolved") resolved(it.payload) } }
            scope.launch {
                s.ended.await()
                holds.cancel()
            }
            syncer.attach(s)
            flush()
        }
        cloud.onSession = { s -> cloudSession(s) }
        scope.launch {
            ledger.queued.collect {
                _outboxChanged.value++
                flush()
            }
        }
    }

    fun start() {
        link.start()
        if (!standingIn.compareAndSet(false, true)) return
        scope.launch {
            link.state.map { it is LinkState.Online }.distinctUntilChanged().collectLatest { online ->
                if (online) {
                    cloud.stop()
                } else {
                    delay(cloudGraceMs)
                    cloud.start()
                }
            }
        }
    }

    /** The server's copy was paired or forgotten: dial it again, if the PC is away. */
    fun cloudChanged() {
        if (link.state.value is LinkState.Online) cloud.stop() else cloud.restart()
    }

    /** A session with the server's copy: its ledger and history events only, nothing read into the replica. */
    private suspend fun cloudSession(s: BusSession) {
        val events = scope.launch { s.events.collect { ev -> staleOnCloud(ev.ev).takeIf { it.isNotEmpty() }?.let { cloudStale.tryEmit(it) } } }
        scope.launch {
            s.ended.await()
            events.cancel()
        }
        s.result("bus.subscribe", buildJsonObject { put("events", JsonArray(CLOUD_EVENTS.map(::JsonPrimitive))) })
        // Tally or Avex may have synced while nothing was listening: what screens show is read again.
        cloudStale.tryEmit(CLOUD_DOMAINS)
    }

    @Volatile private var again = false

    /** Send what the outbox holds, if the PC is in reach. Safe to call any time, from anywhere. */
    suspend fun flush() {
        again = true
        if (!flushing.tryLock()) return
        try {
            while (again) {
                again = false
                val s = link.session.value?.takeIf { it.isOpen } ?: break
                if (runner.flush(s) is OutboxRunner.Outcome.LinkLost) break
            }
        } finally {
            flushing.unlock()
            _outboxChanged.value++
        }
        // An entry queued between the last pass and the unlock is not left waiting.
        if (again && link.session.value?.isOpen == true) flush()
    }

    /**
     * A hold the PC resolved: an outbox entry it was holding is done when someone allowed it (the
     * confirm ran it, under its own id), and refused when someone said no. Resending would only
     * replay `held` (BUS.md §5.3).
     */
    private suspend fun resolved(payload: JsonObject) {
        val hold = payload.l("hold_id") ?: return
        val state = payload.s("state").orEmpty()
        for (e in outbox.open().filter { it.state == OutboxEntry.State.Held }) {
            val held = (e.error?.confirm?.get("payload") as? JsonObject)?.l("hold_id") ?: e.error?.details?.l("hold_id")
            if (held != hold) continue
            if (state == "rejected") {
                val why = payload.s("reason")?.let { ": $it" }.orEmpty()
                outbox.update(e.copy(state = OutboxEntry.State.Failed, error = BusError("refused", "guardrail.rejected", "Denied on the PC$why")))
            } else {
                val done = e.copy(state = OutboxEntry.State.Done, error = null)
                outbox.update(done)
                ledger.applied(done, kotlinx.serialization.json.JsonNull)
            }
            _outboxChanged.value++
        }
    }

    /** An edit to the PC's data, through the outbox. Ops that cannot wait are sent straight away instead. */
    suspend fun change(op: String, payload: JsonObject, label: String): Change {
        if (op !in Optimistic.QUEUEABLE) return Change.Now(call(op, payload))
        val entry = ledger.enqueue(op, payload, label)
        return Change.Queued(entry)
    }

    /** An act that needs the PC now. Throws [BusException] (`link.down` with the PC away). */
    suspend fun call(op: String, payload: JsonObject = JsonObject(emptyMap()), timeoutMs: Long = OutboxRunner.timeoutFor(op)): JsonElement =
        link.require().result(op, payload, timeoutMs = timeoutMs)

    /**
     * A read that is not an entity: the PC's answer when in reach (cached for later); with the PC
     * away, the server's copy's for a read it holds (cached the same way, marked as its); the
     * last cached answer otherwise. [Answer.fresh] and [Answer.cloud] say which.
     */
    suspend fun query(op: String, payload: JsonObject = JsonObject(emptyMap())): Answer {
        val key = QueryKey.of(op, payload)
        if (op in CLOUD_READS) cloudAsked[key] = op to payload
        val s = link.session.value?.takeIf { it.isOpen }
        if (s != null) {
            try {
                val result = s.result(op, payload, timeoutMs = OutboxRunner.timeoutFor(op))
                val at = now()
                store.putQuery(key, result.toString(), at)
                return Answer(result, at, fresh = true)
            } catch (e: BusException) {
                if (!e.error.transient) throw e
            }
        }
        fromCloud(op, payload, key)?.let { return it }
        return saved(key) ?: throw BusException(BusError.link("link.down", "Not connected to the PC, and this was never read before"))
    }

    /** The cached answer only, for a screen's first frame. */
    suspend fun cached(op: String, payload: JsonObject = JsonObject(emptyMap())): Answer? = saved(QueryKey.of(op, payload))

    private suspend fun saved(key: String): Answer? {
        val (json, at) = store.query(key) ?: return null
        // The mark names the answer it was written with; one the PC wrote since has another time.
        val cloud = store.query(cloudMark(key))?.second == at
        return Answer(Wire.json.parseToJsonElement(json), at, fresh = false, cloud = cloud)
    }

    /**
     * One of the server's copy's reads, asked of it: cached under the PC's key, so the screen
     * that showed the PC's answer shows this one, with a mark saying where it came from. Null
     * when the copy cannot answer (not its read, not connected, refused): the cache stands in.
     */
    private suspend fun fromCloud(op: String, payload: JsonObject, key: String): Answer? {
        if (op !in CLOUD_READS) return null
        val s = cloud.session.value?.takeIf { it.isOpen } ?: return null
        val result = try {
            s.result(op, payload, timeoutMs = OutboxRunner.timeoutFor(op))
        } catch (_: BusException) {
            return null
        }
        val at = now()
        store.putQuery(key, result.toString(), at)
        store.putQuery(cloudMark(key), "true", at)
        return Answer(result, at, fresh = false, cloud = true)
    }

    /**
     * The background catch-up with the PC out of reach: read again, from the server's copy, what
     * screens asked of it in this process and the Tally and Avex overviews, so they open on a
     * recent copy. Waits up to [waitMs] for the copy's link. Returns how many it read.
     */
    suspend fun catchUpFromCloud(waitMs: Long = CLOUD_CATCH_UP_WAIT_MS): Int {
        if (link.state.value is LinkState.Online) return 0
        cloud.start()
        cloud.kick()
        withTimeoutOrNull(waitMs) { cloud.state.first { it is LinkState.Online || it is LinkState.Revoked || it is LinkState.Unpaired } }
        if (cloud.session.value?.isOpen != true) return 0
        val asked = CLOUD_CATCH_UP.associateBy { QueryKey.of(it.first, it.second) } + cloudAsked
        return asked.count { (key, q) -> fromCloud(q.first, q.second, key) != null }
    }

    suspend fun discard(entryId: String) {
        ledger.discard(entryId)
        _outboxChanged.value++
    }

    suspend fun retry(entryId: String, overwrite: Boolean) {
        ledger.retry(entryId, overwrite)
        _outboxChanged.value++
        flush()
    }

    /** Forget the PC's data: another PC was paired, or this one forgotten. */
    suspend fun forgetData() = flushing.withLock {
        store.clear()
        outbox.open().forEach { outbox.delete(it.id) }
        _outboxChanged.value++
    }

    sealed interface Change {
        /** Sent straight away; the PC's answer. */
        data class Now(val result: JsonElement) : Change

        /** In the outbox; shown already, sent when the PC is in reach. */
        data class Queued(val entry: OutboxEntry) : Change
    }

    /** A read's answer: the PC's just now ([fresh]), the server's copy's ([cloud]), or the phone's saved one. */
    data class Answer(val result: JsonElement, val at: Long, val fresh: Boolean, val cloud: Boolean = false)

    companion object {
        /** How long the PC link may be down before the server's copy is dialled. */
        const val CLOUD_GRACE_MS = 3_000L

        const val CLOUD_CATCH_UP_WAIT_MS = 30_000L

        /**
         * The reads the server's copy answers for the PC: `CLOUD_OPS` (crates/relay-remote/src/wire.rs)
         * less its writes (`money.sync`, `gym.upload`) and `gym.raw`, which no screen reads.
         */
        val CLOUD_READS = setOf(
            "gym.cardio", "gym.lift.get", "gym.lifts", "gym.series", "gym.session.get", "gym.sessions", "gym.summary",
            "money.invest.list", "money.invest.summary", "money.lists", "money.series", "money.summary", "money.tx.list",
        )

        /** What the phone hears from the server's copy: its ledger and training history moving. */
        val CLOUD_EVENTS = listOf("money.*", "gym.*")

        private val CLOUD_DOMAINS = setOf("money", "gym")

        /** Read again by the catch-up even if no screen asked yet: the Tally and Avex overviews. */
        private val CLOUD_CATCH_UP = listOf("money.summary" to JsonObject(emptyMap()), "gym.summary" to JsonObject(emptyMap()))

        /** The query domains an event from the server's copy leaves stale: its own, or both after a gap. */
        fun staleOnCloud(ev: String): Set<String> = when {
            ev == "bus.lagged" -> CLOUD_DOMAINS
            ev.substringBefore('.') in CLOUD_DOMAINS -> setOf(ev.substringBefore('.'))
            else -> emptySet()
        }

        /** Beside a cached answer the server's copy gave: when it was written. */
        private fun cloudMark(key: String) = "~cloud $key"
    }
}

/** No server's copy is paired: the second link stays [LinkState.Unpaired]. */
private object NoCloud : Link.ProfileSource {
    override suspend fun current(): PcProfile? = null

    override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) = Unit
}
