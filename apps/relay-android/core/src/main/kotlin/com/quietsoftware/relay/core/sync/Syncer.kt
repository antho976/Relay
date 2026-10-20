package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.link.BusSession
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Event
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.conflate
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** One thing to read from the PC into the replica. */
sealed interface Fetch {
    data object Workspaces : Fetch
    data object Projects : Fetch
    data object Sessions : Fetch
    data object Restorable : Fetch
    data object Holds : Fetch
    data object Notifications : Fetch
    data object Threads : Fetch
    data object Dashboard : Fetch
    data object Usage : Fetch
    data object Providers : Fetch
    data class Tasks(val projectId: Long?) : Fetch
    data class Task(val id: Long) : Fetch
    data class Modules(val projectId: Long?) : Fetch
    data class Notes(val projectId: Long) : Fetch
    data class Note(val id: Long) : Fetch
    data class Mail(val projectId: Long) : Fetch
    data class Labels(val projectId: Long) : Fetch
    data class Thread(val id: Long) : Fetch

    /** Everything, as on connecting: the PC may have changed anything while the phone was away. */
    data object All : Fetch
}

/** Canonical cache keys for queries that are not entities. */
object QueryKey {
    fun of(op: String, payload: JsonObject = JsonObject(emptyMap())): String = "$op ${canonical(payload)}"

    private fun canonical(e: JsonElement): String = when (e) {
        is JsonObject -> e.entries.sortedBy { it.key }.joinToString(",", "{", "}") { "\"${it.key}\":${canonical(it.value)}" }
        is JsonArray -> e.joinToString(",", "[", "]") { canonical(it) }
        else -> e.toString()
    }

    /** The op domain of a key, `git` for `git.status {…}`. */
    fun domain(key: String): String = key.substringBefore(' ').substringBefore('.')
}

/** What an event means for the replica: rows to write now, things to read again, query domains gone stale. */
data class Routed(
    val upserts: List<Row> = emptyList(),
    val removals: List<Pair<Kind, String>> = emptyList(),
    val fetches: Set<Fetch> = emptySet(),
    val stale: Set<String> = emptySet(),
)

/**
 * Turns the PC's events into replica changes (BUS.md §3.3): a `noun.changed` carrying the whole
 * row is written as-is; a hint (`task_id`, `project_id`, `bulk`) is read again; everything else
 * marks the screens' cached queries in that domain stale.
 */
object EventRouter {
    fun route(ev: Event): Routed {
        val p = ev.payload
        val project = ev.projectId ?: p.l("project_id")
        return when (ev.ev) {
            "bus.lagged" -> Routed(fetches = setOf(Fetch.All), stale = setOf("*"))
            "session.changed" -> {
                val full = p.containsKey("name") && p.containsKey("state")
                val rows = if (full && p.s("state") != "closed") listOfNotNull(Row.of(Kind.Session, p)) else emptyList()
                val gone = if (full && p.s("state") == "closed") listOf(Kind.Session to p.s("name")!!) else emptyList()
                val fetch = buildSet {
                    if (!full) add(Fetch.Sessions)
                    if (p.s("state") in setOf("restorable", "exited", "closed", "running")) add(Fetch.Restorable)
                    add(Fetch.Dashboard)
                }
                Routed(rows, gone, fetch, setOf("session"))
            }
            "task.changed" -> when {
                p.containsKey("title") && p.containsKey("column") -> Routed(listOfNotNull(Row.of(Kind.Task, p)), fetches = setOf(Fetch.Dashboard), stale = setOf("task"))
                p.l("task_id") != null -> Routed(fetches = setOf(Fetch.Task(p.l("task_id")!!), Fetch.Dashboard), stale = setOf("task"))
                p.arr("task_ids").isNotEmpty() || p.b("bulk") == true || project != null ->
                    Routed(fetches = setOf(Fetch.Tasks(project), Fetch.Dashboard), stale = setOf("task"))
                else -> Routed(fetches = setOf(Fetch.Tasks(null)), stale = setOf("task"))
            }
            "task.deleted" -> Routed(removals = listOfNotNull(p.l("id")?.let { Kind.Task to it.toString() }), fetches = setOf(Fetch.Dashboard), stale = setOf("task"))
            "module.changed", "module.deleted" -> Routed(fetches = setOf(Fetch.Modules(project), Fetch.Tasks(project)), stale = setOf("module", "task"))
            "notes.changed" -> if (p.containsKey("body") && p.l("id") != null) Routed(listOfNotNull(Row.of(Kind.Note, p)), stale = setOf("notes"))
            else Routed(fetches = setOfNotNull(project?.let { Fetch.Notes(it) } ?: p.l("note_id")?.let { Fetch.Note(it) }), stale = setOf("notes"))
            "notes.deleted" -> Routed(removals = listOfNotNull((p.l("id") ?: p.l("note_id"))?.let { Kind.Note to it.toString() }), stale = setOf("notes"))
            "project.changed", "project.deleted" -> Routed(fetches = setOf(Fetch.Projects, Fetch.Dashboard), stale = setOf("project"))
            "workspace.changed", "workspace.deleted" -> Routed(fetches = setOf(Fetch.Workspaces, Fetch.Projects), stale = setOf("workspace"))
            "mailbox.new" -> Routed(listOfNotNull(if (p.containsKey("text")) Row.of(Kind.Mail, p) else null), stale = setOf("mailbox", "task"))
            "mailbox.changed" -> Routed(fetches = setOfNotNull(project?.let { Fetch.Mail(it) }), stale = setOf("mailbox"))
            "notify.new", "notify.changed" -> Routed(fetches = setOf(Fetch.Notifications, Fetch.Dashboard), stale = setOf("notify"))
            "guardrail.held", "guardrail.resolved", "guardrail.refused" -> Routed(fetches = setOf(Fetch.Holds, Fetch.Dashboard), stale = setOf("guardrail"))
            "thread.changed" -> when {
                p.b("deleted") == true -> Routed(removals = listOfNotNull(p.l("id")?.let { Kind.Thread to it.toString() }), stale = setOf("thread"))
                p.o("thread") != null -> Routed(listOfNotNull(Row.of(Kind.Thread, p.o("thread")!!)), stale = setOf("thread"))
                else -> Routed(fetches = setOf(Fetch.Threads), stale = setOf("thread"))
            }
            "thread.message" -> Routed(listOfNotNull(p.o("message")?.let { Row.of(Kind.Message, it) }), stale = setOf("thread"))
            "usage.changed" -> Routed(fetches = setOf(Fetch.Usage), stale = setOf("usage"))
            "provider.version", "provider.update.changed" -> Routed(fetches = setOf(Fetch.Providers), stale = setOf("provider"))
            else -> Routed(stale = setOf(ev.ev.substringBefore('.')).let { s ->
                // Code events also move what the session and task screens show.
                when (ev.ev.substringBefore('.')) {
                    "git", "file", "worktree" -> s + setOf("git", "file", "worktree")
                    "overlap" -> s + "session"
                    "run", "avd", "device" -> s + setOf("device", "avd")
                    "settings" -> s + setOf("guardrail", "notify")
                    else -> s
                }
            })
        }
    }

    /**
     * What the phone subscribes to: everything it keeps or shows. `resource.sample` only while a
     * screen watches it. `design.*` keeps an open design in step while an agent draws it: no
     * routing case, the default marks domain `design` stale, which both design screens read.
     */
    val SUBSCRIBE = listOf(
        "session.*", "guardrail.*", "notify.*", "task.*", "project.*", "workspace.*", "mailbox.*",
        "notes.*", "module.*", "git.*", "file.*", "worktree.*", "overlap.*", "integration.*", "usage.*",
        "run.*", "device.*", "avd.*", "skill.*", "plugin.*", "provider.*", "settings.*", "github.*",
        "thread.*", "money.*", "arbiter.*", "gym.*", "design.*",
    )

    /** Whether the door sends [ev] to a phone that subscribed to [events]: as socket.rs `Filter::matches` reads them. */
    fun subscribes(ev: String, events: List<String> = SUBSCRIBE): Boolean =
        events.isEmpty() || events.any { it == "*" || it == ev || (it.endsWith(".*") && ev.startsWith(it.dropLast(1))) }
}

/**
 * The stale marks a screen showing [domain] reads again on: its own, and `*` after a gap.
 * Conflated, so a read that is still running leaves at most one more behind it: an agent's run
 * of `design.apply`, one `design.changed` each, costs an open design two reads, the last after
 * the last event. It also keeps a slow read from holding [Syncer.stale]'s bounded buffer, which
 * once full would drop every later mark for every screen.
 */
fun Flow<Set<String>>.rereadsOf(domain: String): Flow<Set<String>> = filter { domain in it || "*" in it }.conflate()

/**
 * Keeps the replica in step with the PC over a live session: a full read on connect, then
 * events, each turned into writes and coalesced re-reads. Never holds the replica's lock across
 * a network call.
 */
class Syncer(
    private val scope: CoroutineScope,
    private val ledger: Ledger,
    private val store: ReplicaStore,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private val pending = LinkedHashSet<Fetch>()
    private val pendingLock = Mutex()
    private var drain: Job? = null
    @Volatile private var session: BusSession? = null

    private val _stale = MutableSharedFlow<Set<String>>(extraBufferCapacity = 64)

    /** Query domains whose cached answers are out of date; open screens read theirs again. */
    val stale: SharedFlow<Set<String>> = _stale

    private val _deltas = MutableSharedFlow<Pair<Long, String>>(extraBufferCapacity = 1024)

    /** A thread's reply as it is written (`thread.delta`): (thread id, text). Not stored. */
    val deltas: SharedFlow<Pair<Long, String>> = _deltas

    private val _syncedAt = MutableStateFlow(0L)

    /** When the replica last caught up with the PC in full. */
    val syncedAt: StateFlow<Long> = _syncedAt

    /** A new session: subscribe, then read everything. Events that arrive meanwhile are applied too. */
    suspend fun attach(s: BusSession) {
        session = s
        val events = scope.launch { s.events.collect { onEvent(it) } }
        // A session's events end with it; the next session brings its own.
        scope.launch {
            s.ended.await()
            events.cancel()
        }
        s.result("bus.subscribe", buildJsonObject { put("events", JsonArray(EventRouter.SUBSCRIBE.map(::JsonPrimitive))) })
        resync(s)
    }

    fun detach() {
        session = null
    }

    suspend fun resync(s: BusSession) {
        run(s, Fetch.All)
        _syncedAt.value = now()
        _stale.tryEmit(setOf("*"))
    }

    suspend fun onEvent(ev: Event) {
        if (ev.ev == "thread.delta") {
            val id = ev.payload.l("thread") ?: return
            _deltas.tryEmit(id to ev.payload.s("text").orEmpty())
            return
        }
        val r = EventRouter.route(ev)
        if (r.upserts.isNotEmpty()) ledger.upsert(r.upserts)
        r.removals.groupBy({ it.first }, { it.second }).forEach { (kind, ids) -> ledger.remove(kind, ids) }
        if (r.stale.isNotEmpty()) _stale.tryEmit(r.stale)
        if (r.fetches.isNotEmpty()) schedule(r.fetches)
    }

    /** Coalesce re-reads: a burst of events becomes one read per thing. */
    suspend fun schedule(fetches: Set<Fetch>) {
        pendingLock.withLock {
            pending += fetches
            if (drain != null) return
            drain = scope.launch {
                while (true) {
                    delay(COALESCE_MS)
                    // Whatever was asked for while the last batch ran is read in the next one; the
                    // drain ends under the lock, so nothing asked for meanwhile is left behind.
                    val batch = pendingLock.withLock {
                        val b = pending.toList()
                        pending.clear()
                        if (b.isEmpty() || session == null) drain = null
                        b
                    }
                    val s = session ?: break
                    if (batch.isEmpty()) break
                    val all = Fetch.All in batch
                    for (f in if (all) listOf(Fetch.All) else batch) runCatching { run(s, f) }
                }
            }
        }
    }

    /** Read one thing from the PC into the replica. */
    suspend fun run(s: BusSession, f: Fetch) {
        when (f) {
            Fetch.All -> {
                val projects = run(s, Fetch.Projects).let { store.all(Kind.Project).mapNotNull { it.id.toLongOrNull() } }
                for (each in listOf(Fetch.Workspaces, Fetch.Sessions, Fetch.Restorable, Fetch.Tasks(null), Fetch.Modules(null),
                    Fetch.Holds, Fetch.Notifications, Fetch.Threads, Fetch.Dashboard, Fetch.Usage, Fetch.Providers)) {
                    tolerant { run(s, each) }
                }
                for (id in projects) {
                    tolerant { run(s, Fetch.Notes(id)) }
                    tolerant { run(s, Fetch.Mail(id)) }
                    tolerant { run(s, Fetch.Labels(id)) }
                }
                // Recent threads are read in full so they open with the PC away.
                store.all(Kind.Thread).sortedByDescending { it.json.s("updated_at") }.take(RECENT_THREADS)
                    .mapNotNull { it.id.toLongOrNull() }.forEach { tolerant { run(s, Fetch.Thread(it)) } }
            }
            Fetch.Workspaces -> list(s, "workspace.list", JsonObject(emptyMap()), "workspaces", Kind.Workspace, Scope.All)
            Fetch.Projects -> list(s, "project.list", JsonObject(emptyMap()), "projects", Kind.Project, Scope.All)
            Fetch.Sessions -> list(s, "session.list", JsonObject(emptyMap()), "sessions", Kind.Session, Scope.All)
            Fetch.Restorable -> list(s, "session.restorable", JsonObject(emptyMap()), "sessions", Kind.Restorable, Scope.All)
            Fetch.Holds -> list(s, "guardrail.holds.list", buildJsonObject { put("open_only", true) }, "holds", Kind.Hold, Scope.All)
            Fetch.Notifications -> list(s, "notify.list", buildJsonObject { put("limit", 200) }, "notifications", Kind.Notification, Scope.All)
            Fetch.Threads -> list(s, "thread.list", JsonObject(emptyMap()), "threads", Kind.Thread, Scope.All)
            is Fetch.Tasks -> {
                val rows = mutableListOf<Row>()
                var offset = 0L
                while (true) {
                    val payload = buildJsonObject {
                        f.projectId?.let { put("project_id", it) }
                        put("limit", PAGE)
                        if (offset > 0) put("offset", offset)
                    }
                    val result = s.result("task.list", payload) as? JsonObject ?: break
                    result.arr("tasks").mapNotNullTo(rows) { (it as? JsonObject)?.let { t -> Row.of(Kind.Task, t) } }
                    offset = result.l("next_offset") ?: break
                }
                ledger.replace(Kind.Task, Scope(projectId = f.projectId), rows)
            }
            is Fetch.Task -> try {
                val t = s.result("task.get", buildJsonObject { put("task_id", f.id) }) as? JsonObject
                t?.let { Row.of(Kind.Task, it) }?.let { ledger.upsert(listOf(it)) }
            } catch (e: BusException) {
                if (e.error.kind == "not_found") ledger.remove(Kind.Task, listOf(f.id.toString())) else throw e
            }
            is Fetch.Modules -> list(s, "module.list", buildJsonObject { f.projectId?.let { put("project_id", it) }; put("include_archived", true) }, "modules", Kind.Module, Scope(projectId = f.projectId))
            is Fetch.Notes -> list(s, "notes.list", buildJsonObject { put("project_id", f.projectId) }, "notes", Kind.Note, Scope(projectId = f.projectId))
            is Fetch.Note -> try {
                val n = s.result("notes.get", buildJsonObject { put("note_id", f.id) }) as? JsonObject
                n?.let { Row.of(Kind.Note, it) }?.let { ledger.upsert(listOf(it)) }
            } catch (e: BusException) {
                if (e.error.kind == "not_found") ledger.remove(Kind.Note, listOf(f.id.toString())) else throw e
            }
            is Fetch.Mail -> list(s, "mailbox.list", buildJsonObject { put("project_id", f.projectId); put("limit", 200) }, "messages", Kind.Mail, Scope(projectId = f.projectId))
            is Fetch.Labels -> list(s, "task.label.list", buildJsonObject { put("project_id", f.projectId) }, "labels", Kind.Label, Scope(projectId = f.projectId), projectId = f.projectId)
            is Fetch.Thread -> {
                val out = s.result("thread.get", buildJsonObject { put("id", f.id) }) as? JsonObject ?: return
                out.o("thread")?.let { Row.of(Kind.Thread, it) }?.let { ledger.upsert(listOf(it)) }
                val messages = out.arr("messages").mapNotNull { (it as? JsonObject)?.let { m -> Row.of(Kind.Message, m, parent = f.id.toString()) } }
                ledger.replace(Kind.Message, Scope(parent = f.id.toString()), messages)
            }
            Fetch.Dashboard -> cache(s, "dashboard.get")
            Fetch.Usage -> cache(s, "usage.get")
            Fetch.Providers -> cache(s, "provider.list")
        }
    }

    private suspend fun list(s: BusSession, op: String, payload: JsonObject, field: String, kind: Kind, scope: Scope, projectId: Long? = null) {
        val result = s.result(op, payload) as? JsonObject ?: return
        val rows = result.arr(field).mapNotNull { (it as? JsonObject)?.let { o -> Row.of(kind, o, projectId = projectId) } }
        ledger.replace(kind, scope, rows)
    }

    private suspend fun cache(s: BusSession, op: String, payload: JsonObject = JsonObject(emptyMap())) {
        val result = s.result(op, payload)
        store.putQuery(QueryKey.of(op, payload), result.toString(), now())
    }

    /** One failed read (an op an older PC lacks, a refusal) must not stop the rest of a resync. */
    private suspend fun tolerant(block: suspend () -> Unit) {
        try {
            block()
        } catch (e: BusException) {
            if (e.error.code == "link.down") throw e
        }
    }

    companion object {
        const val COALESCE_MS = 250L
        const val PAGE = 1000
        const val RECENT_THREADS = 12
    }
}
