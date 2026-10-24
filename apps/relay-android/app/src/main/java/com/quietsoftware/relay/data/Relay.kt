package com.quietsoftware.relay.data

import android.content.Context
import android.os.Build
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.link.Handshake
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.model.Label
import com.quietsoftware.relay.core.model.Mail
import com.quietsoftware.relay.core.model.Module
import com.quietsoftware.relay.core.model.Note
import com.quietsoftware.relay.core.model.Notification
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Restorable
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.model.Thread
import com.quietsoftware.relay.core.model.ThreadMessage
import com.quietsoftware.relay.core.model.Workspace
import com.quietsoftware.relay.core.model.decode
import com.quietsoftware.relay.core.model.decodeAll
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Ledger.Companion.isHidden
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.QueryKey
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.sync.rereadsOf
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.core.wire.Wake
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.db.RoomOutbox
import com.quietsoftware.relay.data.db.RoomReplica
import com.quietsoftware.relay.data.db.toEntry
import com.quietsoftware.relay.data.db.toRow
import com.quietsoftware.relay.data.link.CloudStore
import com.quietsoftware.relay.data.link.OkHttpDialer
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.service.forgetNotified
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress

/**
 * What every screen talks to. Reads are flows over the replica, so they show at once and keep
 * showing with the PC away; edits go through the outbox; acts that need the PC now say so.
 */
class Relay(
    private val context: Context,
    private val db: RelayDb,
    val pc: PcStore,
    private val scope: CoroutineScope,
    val cloud: CloudStore = CloudStore(context),
) {
    private val replica = RoomReplica(db)
    private val dialer = OkHttpDialer()
    val hub = Hub(scope, dialer, pc, replica, RoomOutbox(db), cloudProfiles = cloud)

    val state: StateFlow<LinkState> get() = hub.state
    val profile: Flow<PcProfile?> = pc.profile
    val lastSeen: Flow<Long> = pc.lastSeen

    /** The link to the server's copy (docs/CLOUD.md), which answers Tally's and Avex's reads while the PC is away. */
    val cloudState: StateFlow<LinkState> get() = hub.cloud.state
    val cloudProfile: Flow<PcProfile?> = cloud.profile
    val cloudLastSeen: Flow<Long> = cloud.lastSeen

    /** A thread's reply as it streams in: (thread id, text so far is built by the screen). */
    val deltas: SharedFlow<Pair<Long, String>> get() = hub.syncer.deltas

    /** The PC's raw event stream while connected, for screens that react to one (a run's crash). */
    val parked: SharedFlow<OutboxEntry> get() = hub.parked

    init {
        // Disconnected before the process ended: the link starts out stopped, so a kick does not dial it.
        scope.launch { if (pc.disconnected.first()) hub.link.stop() }
    }

    /**
     * Start the link, from the app, a service or the background catch-up; not when the person
     * disconnected it (MOBILE.md §5), which only [connect] undoes.
     */
    fun start() {
        scope.launch { if (!pc.disconnected.first()) hub.start() }
    }

    /** The person asked to connect: the link starts and stays wanted. */
    fun connect() {
        scope.launch {
            pc.setDisconnected(false)
            hub.start()
            kick()
        }
    }

    /** Dial now instead of waiting out the backoff; the server's copy too, if it is standing in for the PC. */
    fun kick() {
        hub.link.kick()
        hub.cloud.kick()
    }

    /** The person disconnected: stays so across the background catch-up, the service and a restart, until [connect]. */
    fun disconnect() {
        hub.link.stop()
        scope.launch { pc.setDisconnected(true) }
    }

    // ---- Reading the replica ----

    private fun rows(kind: Kind): Flow<List<Row>> = db.entities().observe(kind.tag).map { list -> list.mapNotNull { it.toRow() } }

    private fun rows(kind: Kind, projectId: Long): Flow<List<Row>> = db.entities().observeProject(kind.tag, projectId).map { list -> list.mapNotNull { it.toRow() } }

    /**
     * One row as it changes. Room re-reads it on every write to the table (an upsert writes even
     * an unchanged row), so only a row that differs is parsed, off the main thread.
     */
    private fun row(kind: Kind, id: String): Flow<Row?> = db.entities().observeOne(kind.tag, id).distinctUntilChanged().map { it?.toRow() }.flowOn(Dispatchers.Default)

    fun workspaces(): Flow<List<Workspace>> = rows(Kind.Workspace).map { it.decodeAll<Workspace>().sortedBy { w -> w.order } }.flowOn(Dispatchers.Default)

    fun projects(): Flow<List<Project>> = rows(Kind.Project).map { it.decodeAll<Project>().sortedWith(compareBy({ p -> !p.pinned }, { p -> p.order }, { p -> p.name.lowercase() })) }.flowOn(Dispatchers.Default)

    fun project(id: Long): Flow<Project?> = row(Kind.Project, id.toString()).map { it?.decode<Project>() }.flowOn(Dispatchers.Default)

    fun sessions(): Flow<List<Session>> = rows(Kind.Session).map { it.decodeAll<Session>().sortedWith(SESSION_ORDER) }.flowOn(Dispatchers.Default)

    fun session(name: String): Flow<Session?> = row(Kind.Session, name).map { it?.decode<Session>() }.flowOn(Dispatchers.Default)

    fun restorable(): Flow<List<Restorable>> = rows(Kind.Restorable).map { it.decodeAll<Restorable>() }.flowOn(Dispatchers.Default)

    fun tasks(projectId: Long): Flow<List<Task>> = rows(Kind.Task, projectId).map { it.decodeAll<Task>().filter { t -> t.deletedAt == null }.sortedBy { t -> t.position } }.flowOn(Dispatchers.Default)

    fun allTasks(): Flow<List<Task>> = rows(Kind.Task).map { it.decodeAll<Task>().filter { t -> t.deletedAt == null } }.flowOn(Dispatchers.Default)

    fun task(id: String): Flow<Task?> = row(Kind.Task, id).map { r -> r?.takeUnless { it.json.isHidden() }?.decode<Task>() }.flowOn(Dispatchers.Default)

    /**
     * The id to show for [id]: itself, or, for a row the phone made (`tmp:<entry>`), the PC's id
     * once the PC has applied the create. A screen opened on a temp row follows it here.
     */
    fun resolved(id: String): Flow<String> {
        if (!Optimistic.isTemp(id)) return kotlinx.coroutines.flow.flowOf(id)
        return db.outbox().observe(id.removePrefix(Optimistic.TEMP)).map { row ->
            val e = row?.toEntry()
            if (e?.state == OutboxEntry.State.Done) e.result?.let { Optimistic.created(e.op, e.id, it)?.second } ?: id else id
        }.distinctUntilChanged()
    }

    /** Whether a row has edits the PC has not taken yet. */
    fun pending(kind: Kind, id: String): Flow<Boolean> = row(kind, id).map { it?.pending == true }.distinctUntilChanged()

    fun modules(projectId: Long): Flow<List<Module>> = rows(Kind.Module, projectId).map { it.decodeAll<Module>().filter { m -> m.deletedAt == null }.sortedBy { m -> m.order } }.flowOn(Dispatchers.Default)

    fun notes(projectId: Long): Flow<List<Note>> = rows(Kind.Note, projectId).map { it.decodeAll<Note>().sortedWith(compareBy<Note> { n -> !n.pinned }.thenByDescending { n -> n.updatedAt }) }.flowOn(Dispatchers.Default)

    fun note(id: String): Flow<Note?> = row(Kind.Note, id).map { r -> r?.takeUnless { it.json.isHidden() }?.decode<Note>() }.flowOn(Dispatchers.Default)

    fun holds(): Flow<List<Hold>> = rows(Kind.Hold).map { it.decodeAll<Hold>().filter { h -> h.state == "open" }.sortedByDescending { h -> h.createdAt } }.flowOn(Dispatchers.Default)

    fun notifications(): Flow<List<Notification>> = rows(Kind.Notification).map { it.decodeAll<Notification>().sortedByDescending { n -> n.createdAt } }.flowOn(Dispatchers.Default)

    fun threads(): Flow<List<Thread>> = rows(Kind.Thread).map { it.decodeAll<Thread>().sortedByDescending { t -> t.updatedAt } }.flowOn(Dispatchers.Default)

    fun thread(id: String): Flow<Thread?> = row(Kind.Thread, id).map { r -> r?.takeUnless { it.json.isHidden() }?.decode<Thread>() }.flowOn(Dispatchers.Default)

    fun messages(threadId: String): Flow<List<ThreadMessage>> =
        db.entities().observeParent(Kind.Message.tag, threadId).map { list -> list.mapNotNull { it.toRow() }.decodeAll<ThreadMessage>().sortedWith(MESSAGE_ORDER) }.flowOn(Dispatchers.Default)

    fun mail(projectId: Long): Flow<List<Mail>> = rows(Kind.Mail, projectId).map { it.decodeAll<Mail>().sortedBy { m -> m.sentAt } }.flowOn(Dispatchers.Default)

    fun labels(projectId: Long): Flow<List<Label>> = rows(Kind.Label, projectId).map { it.decodeAll<Label>().sortedBy { l -> l.name.lowercase() } }.flowOn(Dispatchers.Default)

    /** Everything the outbox still holds, oldest first. */
    fun outbox(): Flow<List<OutboxEntry>> = db.outbox().observeOpen().distinctUntilChanged().map { list -> list.mapNotNull { it.toEntry() } }.flowOn(Dispatchers.Default)

    /** A cached query answer, as it changes. */
    fun cached(op: String, payload: JsonObject = JsonObject(emptyMap())): Flow<Answer?> =
        // Any query row written re-reads this one: parsed (up to a long answer) only when it changed, off Main.
        db.queries().observe(QueryKey.of(op, payload)).distinctUntilChanged()
            .map { row -> row?.let { Answer(Wire.json.parseToJsonElement(it.json), it.at, fresh = false) } }.flowOn(Dispatchers.Default)

    /**
     * A query a screen shows: the cached answer first, then the PC's (or, with the PC away, the
     * server's copy's for a read it holds), read again whenever an event on either says that
     * domain moved or the PC link comes back. A burst of events while a read runs leaves one
     * more read behind it, not one each ([rereadsOf]). Never errors with the PC away while there
     * is a cached answer.
     */
    fun live(op: String, payload: JsonObject = JsonObject(emptyMap())): Flow<Live> = channelFlow {
        val key = QueryKey.of(op, payload)
        val domain = QueryKey.domain(key)
        var last: Live = Live()
        hub.cached(op, payload)?.let { last = shown(it); send(last) }
        suspend fun read() {
            try {
                last = shown(hub.query(op, payload))
            } catch (e: BusException) {
                last = last.copy(error = e.error)
            }
            send(last)
        }
        read()
        launch { hub.stale.rereadsOf(domain).collect { read() } }
        launch { hub.state.filter { it is LinkState.Online }.distinctUntilChanged().collect { if (!last.fresh) read() } }
        awaitClose()
    }.flowOn(Dispatchers.Default)

    /** A hub answer as a screen holds it; [Live.cloud] says whether the server's copy gave it. */
    private fun shown(a: Hub.Answer): Live = Live(a.result, a.at, fresh = a.fresh, cloud = a.cloud)

    data class Answer(val result: JsonElement, val at: Long, val fresh: Boolean)

    /** [cloud]: the answer came from the server's copy while the PC was away, not from the phone's own. */
    data class Live(val result: JsonElement? = null, val at: Long = 0, val fresh: Boolean = false, val error: BusError? = null, val cloud: Boolean = false) {
        val loading: Boolean get() = result == null && error == null
    }

    // ---- Changing things ----

    /** An edit to the PC's data: shown now, sent now or when the PC is back. */
    suspend fun change(op: String, payload: JsonObject, label: String): Hub.Change = hub.change(op, payload, label)

    /** An act that needs the PC now; throws `link.down` with it away. */
    suspend fun call(op: String, payload: JsonObject = JsonObject(emptyMap())): JsonElement = hub.call(op, payload)

    suspend fun discard(entryId: String) = hub.discard(entryId)

    suspend fun retry(entryId: String, overwrite: Boolean) = hub.retry(entryId, overwrite)

    // ---- Pairing ----

    suspend fun pair(link: PairLink): Result<PcProfile> = withContext(Dispatchers.IO) {
        val result = Handshake.pair(dialer, link, deviceName())
        result.onSuccess { profile ->
            val previous = pc.current()
            if (previous != null && previous.hostId != profile.hostId) {
                hub.forgetData()
                forgetNotified(context)
            }
            pc.save(profile)
            hub.link.restart()
        }
        result
    }

    /** Forget the PC: its credential, and the copy of its data. Edits not yet sent are lost. */
    suspend fun forget() {
        hub.link.stop()
        pc.forget()
        hub.forgetData()
        forgetNotified(context)
        hub.link.start()
    }

    /**
     * Pair the server's copy (docs/CLOUD.md) with a link `relay remote pair --via-only` printed on
     * the server. It is a second host beside the PC, never in its place: the PC's pairing and this
     * phone's copy of its data stay as they are. A link that turns out to be a PC's is refused.
     */
    suspend fun pairCloud(link: PairLink): Result<PcProfile> = withContext(Dispatchers.IO) {
        val result = Handshake.pair(dialer, link, deviceName(), Wire.OPS_CLOUD)
        result.onSuccess { profile ->
            cloud.save(profile)
            hub.cloudChanged()
        }
        result
    }

    /** Forget the server's copy: its credential only. What it answered stays, marked as its, until read again. */
    suspend fun forgetCloud() {
        hub.cloud.stop()
        cloud.forget()
        hub.cloudChanged()
    }

    /**
     * Wake a sleeping PC with a Wake-on-LAN packet to each network card it reported. Only reaches
     * it from its own network. True when at least one packet left the phone.
     */
    suspend fun wake(): Boolean = withContext(Dispatchers.IO) {
        val targets = pc.current()?.wake.orEmpty()
        var sent = false
        DatagramSocket().use { socket ->
            socket.broadcast = true
            for (t in targets) {
                val packet = Wake.magicPacket(t.mac) ?: continue
                for (address in listOfNotNull(t.broadcast, "255.255.255.255").distinct()) {
                    runCatching {
                        socket.send(DatagramPacket(packet, packet.size, InetAddress.getByName(address), Wake.PORT))
                        sent = true
                    }
                }
            }
        }
        if (sent) scope.launch { kotlinx.coroutines.delay(3_000); hub.link.kick() }
        sent
    }

    suspend fun isPaired(): Boolean = pc.profile.first() != null

    private fun deviceName(): String = listOf(Build.MANUFACTURER.replaceFirstChar { it.uppercase() }, Build.MODEL).distinct().joinToString(" ").take(64)

    companion object {
        /**
         * A thread's messages in time order, the ones the PC has not answered yet last: their
         * stamps are the phone's clock, which may be behind the PC's.
         */
        val MESSAGE_ORDER: Comparator<ThreadMessage> = compareBy({ m -> Optimistic.isTemp(m.id) }, { m -> m.createdAt }, { m -> m.id.toLongOrNull() ?: Long.MAX_VALUE })

        /** Needs you first, then working, then the rest (the old app's order, and the PC's). */
        private val STATE_ORDER = listOf("blocked", "running", "spawning", "idle", "created", "restorable", "parked", "exited", "closed")
        val SESSION_ORDER: Comparator<Session> = compareBy({ STATE_ORDER.indexOf(it.state).let { i -> if (i < 0) 99 else i } }, { it.name })
    }
}
