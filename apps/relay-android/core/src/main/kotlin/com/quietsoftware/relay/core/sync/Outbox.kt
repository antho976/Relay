package com.quietsoftware.relay.core.sync

import com.quietsoftware.relay.core.link.BusSession
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Every change the phone makes to the PC's data, in the order it was made, until the PC has it.
 *
 * An entry's [id] is the bus request id, minted when the change was made and never again
 * (BUS.md §5.3): sending it twice runs it once, so a send whose answer was lost is simply sent
 * again. That is what lets the phone edit with no PC in reach and converge when one is back.
 */
data class OutboxEntry(
    val id: String,
    val seq: Long,
    val op: String,
    /** May hold `{"$ref": "<entry id>", "path": "id"}` for a value an earlier entry's answer supplies. */
    val payload: JsonObject,
    val label: String,
    val createdAt: Long,
    val state: State = State.Pending,
    val error: BusError? = null,
    val result: JsonElement? = null,
    val attempts: Int = 0,
) {
    enum class State {
        /** Waiting to be sent, or sent with no answer yet. */
        Pending,

        /** The PC applied it. Kept briefly so later entries can read its answer. */
        Done,

        /** The PC refused it because the same fields changed there first (`*.edit_conflict`). */
        Conflict,

        /** Any other refusal. Kept until the person retries or drops it. */
        Failed,

        /** A guardrail holds it on the PC until someone allows it there or here. */
        Held,
    }

    val parked: Boolean get() = state == State.Conflict || state == State.Failed || state == State.Held
}

interface OutboxStore {
    suspend fun add(entry: OutboxEntry)

    /** Every entry not [OutboxEntry.State.Done], oldest first. */
    suspend fun open(): List<OutboxEntry>

    suspend fun get(id: String): OutboxEntry?

    suspend fun update(entry: OutboxEntry)

    suspend fun delete(id: String)

    suspend fun nextSeq(): Long

    /** Drop finished entries older than [before] whose answers nothing pending still refers to. */
    suspend fun prune(before: Long)
}

object Refs {
    const val KEY = "\$ref"
    const val PATH = "path"

    fun ref(entryId: String, path: String = "id"): JsonObject =
        JsonObject(mapOf(KEY to JsonPrimitive(entryId), PATH to JsonPrimitive(path)))

    /** The entries [payload] waits on. */
    fun of(payload: JsonElement): Set<String> = buildSet { collect(payload, this) }

    private fun collect(e: JsonElement, into: MutableSet<String>) {
        when (e) {
            is JsonObject -> {
                val target = refTarget(e)
                if (target != null) into += target else e.values.forEach { collect(it, into) }
            }
            is JsonArray -> e.forEach { collect(it, into) }
            else -> Unit
        }
    }

    private fun refTarget(o: JsonObject): String? =
        if (o.size == 2 && o.containsKey(KEY) && o.containsKey(PATH)) o.s(KEY) else null

    /** [payload] with its references to entry [from] naming entry [to] instead. */
    fun rename(payload: JsonObject, from: String, to: String): JsonObject = renameElement(payload, from, to) as JsonObject

    private fun renameElement(e: JsonElement, from: String, to: String): JsonElement = when (e) {
        is JsonObject -> if (refTarget(e) == from) ref(to, e.s(PATH).orEmpty()) else JsonObject(e.mapValues { (_, v) -> renameElement(v, from, to) })
        is JsonArray -> JsonArray(e.map { renameElement(it, from, to) })
        else -> e
    }

    /** [payload] with every reference replaced by the value it names; null while one is unanswered. */
    fun resolve(payload: JsonObject, answer: (String) -> JsonElement?): JsonObject? =
        resolveElement(payload, answer) as? JsonObject

    private fun resolveElement(e: JsonElement, answer: (String) -> JsonElement?): JsonElement? {
        return when (e) {
            is JsonObject -> {
                val target = refTarget(e)
                if (target != null) {
                    val result = answer(target) ?: return null
                    pick(result, e.s(PATH).orEmpty())
                } else {
                    val out = LinkedHashMap<String, JsonElement>()
                    for ((k, v) in e) out[k] = resolveElement(v, answer) ?: return null
                    JsonObject(out)
                }
            }
            is JsonArray -> JsonArray(e.map { resolveElement(it, answer) ?: return null })
            else -> e
        }
    }

    /** `a.b.c` into a result; a missing step is null, not an error. */
    fun pick(e: JsonElement, path: String): JsonElement {
        var cur: JsonElement = e
        if (path.isEmpty()) return cur
        for (step in path.split('.')) {
            cur = (cur as? JsonObject)?.get(step) ?: return JsonNull
        }
        return cur
    }
}

/**
 * Sends the outbox, oldest first, over a live session. Stops at the first sign the link is gone
 * (the rest waits for the next connection); parks what the PC refuses so it never blocks unrelated
 * work behind it, but holds back later entries that touch the same thing or need its answer.
 */
class OutboxRunner(
    private val store: OutboxStore,
    private val onApplied: suspend (OutboxEntry, JsonElement) -> Unit,
    private val onParked: suspend (OutboxEntry) -> Unit = {},
    private val now: () -> Long = System::currentTimeMillis,
    /** A message refused while its thread's agent is still answering: queued again under a new id, sent once it is done. */
    private val onBusy: suspend (OutboxEntry) -> Unit = {},
) {
    /** Threads whose agent is answering: messages to them wait for [threadFree]. */
    private val busyThreads: MutableSet<String> = java.util.concurrent.ConcurrentHashMap.newKeySet()

    /** The thread's turn ended: true when a message was waiting on it, and the outbox should be sent. */
    fun threadFree(id: String): Boolean = busyThreads.remove(id)

    /** A new session: whatever was answering then may be done. */
    fun forgetBusy() = busyThreads.clear()

    sealed interface Outcome {
        data class Flushed(val sent: Int, val parked: Int, val waiting: Int) : Outcome
        data class LinkLost(val sent: Int, val error: BusError) : Outcome
    }

    suspend fun flush(session: BusSession): Outcome {
        var sent = 0
        var parked = 0
        var waiting = 0
        val blockedTargets = mutableSetOf<Pair<Kind, String>>()
        val blockedEntries = mutableSetOf<String>()
        for (entry in store.open()) {
            val target = Optimistic.target(entry.op, entry.payload, entry.id)
            if (entry.parked) {
                parked++
                blockedEntries += entry.id
                target?.let { blockedTargets += it }
                continue
            }
            val refs = Refs.of(entry.payload)
            val waitsOnParked = refs.any { it in blockedEntries } || (target != null && target in blockedTargets)
            val answers = if (waitsOnParked) emptyMap() else refs.associateWith { answerOf(it) }
            val payload = if (waitsOnParked) null else Refs.resolve(entry.payload) { ref -> answers[ref] }
            if (payload == null) {
                waiting++
                blockedEntries += entry.id
                target?.let { blockedTargets += it }
                continue
            }
            // A thread answers one message at a time (thread.busy): the rest wait for its turn to end.
            val thread = if (entry.op == "thread.send") (payload["id"] as? JsonPrimitive)?.content else null
            if (thread != null && thread in busyThreads) {
                waiting++
                blockedEntries += entry.id
                target?.let { blockedTargets += it }
                continue
            }
            val attempt = entry.copy(attempts = entry.attempts + 1)
            store.update(attempt)
            val response = try {
                session.call(entry.op, payload, id = entry.id, timeoutMs = timeoutFor(entry.op))
            } catch (e: BusException) {
                return Outcome.LinkLost(sent, e.error)
            }
            // Dropped by the person while it was on its way: an answer still brings the rows up to
            // date, but the entry is not written back.
            val dropped = store.get(entry.id) == null
            if (response.ok) {
                val done = attempt.copy(state = OutboxEntry.State.Done, result = response.result, error = null)
                if (!dropped) store.update(done)
                onApplied(done, response.result)
                sent++
                continue
            }
            val error = response.error ?: BusError("internal", "bus.no_error", "The PC refused without saying why")
            if (error.transient) return Outcome.LinkLost(sent, error)
            if (dropped) continue
            if (error.code == "thread.busy" && thread != null) {
                // The refusal is recorded under this id, so the message goes again under a new one.
                busyThreads += thread
                onBusy(attempt)
                waiting++
                continue
            }
            if (error.code == "bus.replay_unavailable" && (error.details?.get("applied") as? JsonPrimitive)?.content == "true") {
                // It ran the first time; only its answer, too large to keep, is gone. Sending it
                // again under a new id would run it twice: done, with no answer (JsonNull), and
                // the row is read again.
                val done = attempt.copy(state = OutboxEntry.State.Done, result = JsonNull, error = null)
                store.update(done)
                onApplied(done, JsonNull)
                sent++
                continue
            }
            val state = when {
                error.kind == "held" -> OutboxEntry.State.Held
                error.kind == "conflict" && error.code.endsWith("edit_conflict") -> OutboxEntry.State.Conflict
                else -> OutboxEntry.State.Failed
            }
            val stuck = attempt.copy(state = state, error = error)
            store.update(stuck)
            onParked(stuck)
            parked++
            blockedEntries += entry.id
            target?.let { blockedTargets += it }
        }
        store.prune(now() - DONE_KEEP_MS)
        return Outcome.Flushed(sent, parked, waiting)
    }

    private suspend fun answerOf(entryId: String): JsonElement? {
        val e = store.get(entryId) ?: return null
        return if (e.state == OutboxEntry.State.Done) e.result else null
    }

    companion object {
        /** Finished entries stay this long so a later one can still read their answer after a restart. */
        const val DONE_KEEP_MS = 7L * 24 * 60 * 60 * 1000

        /** How long an op may take before its answer counts as lost (the old app's `LONG_OPS`). */
        fun timeoutFor(op: String): Long = when (op) {
            "project.clone" -> 1_800_000L
            "git.push", "git.fetch", "git.pr.open", "integration.request", "device.build", "device.run",
            "app.backup.now", "skill.install", "provider.update", "guardrail.confirm" -> 300_000L
            "session.create", "session.spawn", "session.resume", "task.dispatch", "git.worktree.create", "avd.boot" -> 180_000L
            "git.pr.list", "provider.refresh", "session.upload", "thread.upload", "thread.image", "design.asset.get" -> 120_000L
            "file.search", "workspace.discover", "github.repo.list", "github.connect" -> 60_000L
            else -> 30_000L
        }

        /** The id of the entity a create's answer made, for the temp id the phone showed meanwhile. */
        fun createdId(result: JsonElement): String? = (result as? JsonObject)?.let { it.l("id")?.toString() ?: it.s("name") }
    }
}
