package com.quietsoftware.relay.core.sync

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import java.util.UUID

/**
 * The replica and the outbox as one: every write from the PC passes through here so the phone's
 * pending edits stay laid over it, and every edit the phone makes lands here first so it shows
 * at once. One lock orders the two, so an event from the PC can never interleave with an edit
 * half-applied.
 */
class Ledger(
    private val rows: ReplicaStore,
    private val outbox: OutboxStore,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private val lock = Mutex()
    private val _queued = MutableSharedFlow<OutboxEntry>(extraBufferCapacity = 64)

    /** Each new entry, so the link can send it at once when the PC is in reach. */
    val queued: SharedFlow<OutboxEntry> = _queued

    // ---- From the PC ----

    suspend fun upsert(rows: List<Row>) = lock.withLock {
        if (rows.isEmpty()) return@withLock
        val open = outbox.open()
        this.rows.upsert(rows.map { overlay(it, open) })
    }

    suspend fun remove(kind: Kind, ids: Collection<String>) = lock.withLock { rows.remove(kind, ids) }

    /**
     * A list op's answer is the whole truth for its scope, except for rows the phone made that the
     * PC has not seen yet: those stay until their create is answered or dropped.
     */
    suspend fun replace(kind: Kind, scope: Scope, server: List<Row>) = lock.withLock {
        val open = outbox.open()
        val kept = server.map { overlay(it, open) }
        val ids = kept.map { it.id }.toSet()
        val made = rows.all(kind, scope).filter { it.base == null && it.id !in ids }
        rows.replace(kind, scope, kept + made)
    }

    // ---- From the phone ----

    /**
     * Record a change: queued for the PC, shown here now. Returns the entry, whose id is the
     * request id and, for a create, names the temp row (`tmp:<id>`) the phone shows meanwhile.
     */
    suspend fun enqueue(op: String, payload: JsonObject, label: String): OutboxEntry {
        val entry = lock.withLock {
            val e = OutboxEntry(
                id = UUID.randomUUID().toString(),
                seq = outbox.nextSeq(),
                op = op,
                payload = payload,
                label = label,
                createdAt = now(),
            )
            outbox.add(e)
            val target = Optimistic.target(op, payload, e.id)
            if (target != null) {
                val (kind, raw) = target
                val id = resolveTemp(raw)
                val current = rows.get(kind, id)
                val next = Optimistic.apply(op, payload, current?.json, e.id, now())
                when {
                    next == null && current != null -> rows.upsert(listOf(current.copy(json = current.json.hidden(), pending = true)))
                    next != null -> {
                        val row = Row.of(kind, next, current?.projectId ?: projectHint(payload), current?.parent ?: parentHint(op, payload))
                        if (row != null) rows.upsert(listOf(row.copy(id = current?.id ?: row.id, base = current?.base, pending = true)))
                    }
                }
            }
            e
        }
        _queued.tryEmit(entry)
        return entry
    }

    /** The PC applied [entry]: its answer becomes the base, and a create's temp row gives way to the real one. */
    suspend fun applied(entry: OutboxEntry, result: JsonElement) = lock.withLock {
        val open = outbox.open()
        Optimistic.created(entry.op, entry.id, result)?.let { (temp, _) ->
            Optimistic.target(entry.op, entry.payload, entry.id)?.let { (kind, _) -> rows.remove(kind, listOf(temp)) }
        }
        val fresh = Optimistic.resultRows(entry.op, result)
        if (fresh.isNotEmpty()) {
            rows.upsert(fresh.map { overlay(it, open) })
        } else {
            rebuild(entry, open)
        }
    }

    /**
     * The person dropped a parked entry: the row goes back to what the PC last said, plus whatever
     * else is pending. Entries that need its answer are dropped with it.
     */
    suspend fun discard(entryId: String) = lock.withLock {
        val entry = outbox.get(entryId) ?: return@withLock
        outbox.delete(entryId)
        rebuild(entry, outbox.open())
        // Entries that wait on a dropped one's answer (a label on a task made offline, an agent's
        // start after its create) could never be sent: they go with it.
        val gone = mutableSetOf(entryId)
        while (true) {
            val orphans = outbox.open().filter { e -> e.id !in gone && Refs.of(e.payload).any { it in gone } }
            if (orphans.isEmpty()) break
            for (o in orphans) {
                outbox.delete(o.id)
                gone += o.id
                rebuild(o, outbox.open())
            }
        }
    }

    /**
     * Send a parked entry again: a conflict is resent without its `expected` check, so the phone's
     * version wins. It goes under a new id, since the old one is spent on the refusal, which the PC
     * would only replay; so a create's temp row takes the new id, and entries that wait on its
     * answer (an agent's start, a message in a thread made offline) wait on the new one.
     */
    suspend fun retry(entryId: String, overwrite: Boolean) = lock.withLock {
        val entry = outbox.get(entryId) ?: return@withLock null
        val payload = if (overwrite) JsonObject(entry.payload.filterKeys { it != "expected" && it != "expected_updated_at" }) else entry.payload
        outbox.delete(entryId)
        val again = entry.copy(id = UUID.randomUUID().toString(), payload = payload, state = OutboxEntry.State.Pending, error = null, attempts = 0)
        outbox.add(again)
        val dependents = outbox.open().filter { entryId in Refs.of(it.payload) }
        for (d in dependents) outbox.update(d.copy(payload = Refs.rename(d.payload, entryId, again.id)))
        Optimistic.target(entry.op, entry.payload, entryId)?.let { (kind, id) ->
            if (id == Optimistic.tempId(entryId)) rows.remove(kind, listOf(id))
        }
        val open = outbox.open()
        remake(again, open)
        for (d in dependents) open.firstOrNull { it.id == d.id }?.let { remake(it, open) }
        again
    }?.also { _queued.tryEmit(it) }

    /** The row [entry] makes (its own temp row) built again from its payload, with what is pending on it laid over. */
    private suspend fun remake(entry: OutboxEntry, open: List<OutboxEntry>) {
        val (kind, id) = Optimistic.target(entry.op, entry.payload, entry.id) ?: return
        if (id != Optimistic.tempId(entry.id)) return
        val made = Optimistic.apply(entry.op, entry.payload, null, entry.id, entry.createdAt) ?: return
        val row = Row.of(kind, made, projectHint(entry.payload), parentHint(entry.op, entry.payload)) ?: return
        var json = row.json
        for (e in open) {
            if (e.id == entry.id) continue
            val (k, raw) = Optimistic.target(e.op, e.payload, e.id) ?: continue
            if (k == kind && raw == id) json = Optimistic.apply(e.op, e.payload, json, e.id, e.createdAt) ?: json.hidden()
        }
        rows.upsert(listOf(row.copy(json = json, base = null, pending = true)))
    }

    // ---- Overlays ----

    private suspend fun rebuild(entry: OutboxEntry, open: List<OutboxEntry>) {
        val (kind, raw) = Optimistic.target(entry.op, entry.payload, entry.id) ?: return
        val id = resolveTemp(raw)
        val current = rows.get(kind, id) ?: return
        val base = current.base
        if (base == null) {
            // A row the phone made: gone if its create is gone, otherwise rebuilt from the create.
            val stillMade = open.any { Optimistic.target(it.op, it.payload, it.id) == (kind to id) && it.op.endsWith(".create") }
            if (!stillMade) rows.remove(kind, listOf(id))
            return
        }
        rows.upsert(listOf(overlay(current.copy(json = base), open)))
    }

    /** [server] with every open entry that targets it applied in order. */
    private suspend fun overlay(server: Row, open: List<OutboxEntry>): Row {
        var json: JsonObject = server.json
        var touched = false
        for (e in open) {
            val (kind, raw) = Optimistic.target(e.op, e.payload, e.id) ?: continue
            if (kind != server.kind || resolveTemp(raw) != server.id) continue
            // A delete hides the row rather than dropping it, so a restore after it brings it back.
            json = Optimistic.apply(e.op, e.payload, json, e.id, e.createdAt) ?: json.hidden()
            touched = true
        }
        return server.copy(json = json, base = server.json, pending = touched)
    }

    /** A temp id whose create the PC has answered names the real row now. */
    private suspend fun resolveTemp(id: String): String {
        if (!Optimistic.isTemp(id)) return id
        val entry = outbox.get(id.removePrefix(Optimistic.TEMP)) ?: return id
        if (entry.state != OutboxEntry.State.Done) return id
        return entry.result?.let { Optimistic.created(entry.op, entry.id, it)?.second } ?: id
    }

    private fun projectHint(payload: JsonObject): Long? = (payload["project_id"] as? kotlinx.serialization.json.JsonPrimitive)?.content?.toLongOrNull()

    private fun parentHint(op: String, payload: JsonObject): String? {
        if (op != "thread.send") return null
        return when (val t = payload["id"]) {
            is JsonObject -> (t[Refs.KEY] as? kotlinx.serialization.json.JsonPrimitive)?.content?.let { Optimistic.tempId(it) }
            is kotlinx.serialization.json.JsonPrimitive -> t.content
            else -> null
        }
    }

    companion object {
        /** A row the phone deleted but the PC has not: kept, flagged, so lists hide it and Undo can bring it back. */
        const val HIDDEN = "_hidden"

        fun JsonObject.hidden(): JsonObject = JsonObject(LinkedHashMap(this).also { it[HIDDEN] = kotlinx.serialization.json.JsonPrimitive(true) })

        fun JsonObject.isHidden(): Boolean = (this[HIDDEN] as? kotlinx.serialization.json.JsonPrimitive)?.content == "true"
    }
}
