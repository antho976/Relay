package com.quietsoftware.relay.core.testing

import com.quietsoftware.relay.core.link.Dialer
import com.quietsoftware.relay.core.link.LineSocket
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.OutboxStore
import com.quietsoftware.relay.core.sync.ReplicaStore
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.sync.Scope
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.s
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.util.concurrent.atomic.AtomicLong

class MemoryReplica : ReplicaStore {
    val rows = LinkedHashMap<Pair<Kind, String>, Row>()
    val queries = HashMap<String, Pair<String, Long>>()

    override suspend fun upsert(rows: List<Row>) = rows.forEach { this.rows[it.kind to it.id] = it }
    override suspend fun remove(kind: Kind, ids: Collection<String>) = ids.forEach { rows.remove(kind to it) }
    override suspend fun replace(kind: Kind, scope: Scope, rows: List<Row>) {
        this.rows.keys.filter { (k, _) -> k == kind }.filter { key -> this.rows[key]!!.inScope(scope) }.forEach { this.rows.remove(it) }
        upsert(rows)
    }
    override suspend fun get(kind: Kind, id: String) = rows[kind to id]
    override suspend fun all(kind: Kind, scope: Scope) = rows.values.filter { it.kind == kind && it.inScope(scope) }
    override suspend fun putQuery(key: String, json: String, at: Long) { queries[key] = json to at }
    override suspend fun query(key: String) = queries[key]
    override suspend fun clear() { rows.clear(); queries.clear() }

    private fun Row.inScope(scope: Scope) =
        (scope.projectId == null || projectId == scope.projectId) && (scope.parent == null || parent == scope.parent)
}

class MemoryOutbox : OutboxStore {
    val entries = LinkedHashMap<String, OutboxEntry>()
    private val seq = AtomicLong()

    override suspend fun add(entry: OutboxEntry) { entries[entry.id] = entry }
    override suspend fun open() = entries.values.filter { it.state != OutboxEntry.State.Done }.sortedBy { it.seq }
    override suspend fun get(id: String) = entries[id]
    override suspend fun update(entry: OutboxEntry) { entries[entry.id] = entry }
    override suspend fun delete(id: String) { entries.remove(id) }
    override suspend fun nextSeq() = seq.incrementAndGet()
    override suspend fun prune(before: Long) = Unit
}

/** Two ends of a socket made of channels. */
class Pipe : LineSocket {
    val toPhone = Channel<String>(Channel.UNLIMITED)
    val toPc = Channel<String>(Channel.UNLIMITED)
    override val incoming: ReceiveChannel<String> get() = toPhone
    override fun send(line: String) = toPc.trySend(line).isSuccess
    override fun close() {
        toPhone.close()
        toPc.close()
    }
}

/**
 * A PC door in a test: greets, checks the proof against [token], then answers bus requests with
 * [handle] and records them. Requests are deduplicated by id the way the engine does (BUS.md §5.3).
 * [ops] is what its welcome says it opens (`phone`, `cloud`, or nothing, as an older door); a
 * pairing code is taken as given, and answered with [device] and [token].
 */
class FakeDoor(
    private val scope: CoroutineScope,
    val device: String = "dev1",
    val token: String = "t0k3n",
    var engine: String? = null,
    var handle: (op: String, payload: JsonObject) -> JsonElement = { _, _ -> JsonObject(emptyMap()) },
    var ops: String? = null,
) : Dialer {
    val requests: MutableList<Pair<String, JsonObject>> = java.util.concurrent.CopyOnWriteArrayList()
    private val answered = HashMap<String, String>()
    var reachable = setOf("ws://192.168.1.20:7420")
    var dials = 0
    val pipes = mutableListOf<Pipe>()

    override suspend fun dial(url: String, timeoutMs: Long): LineSocket {
        dials++
        if (url !in reachable) throw java.io.IOException("unreachable $url")
        val pipe = Pipe()
        pipes += pipe
        scope.launch { serve(pipe) }
        return pipe
    }

    /** Send an event to every connected phone. */
    fun emit(ev: String, payload: JsonObject) {
        val line = buildJsonObject {
            put("v", 1); put("ev", ev); put("ts", "2026-10-09T00:00:00Z"); put("actor", "user"); put("payload", payload)
        }.toString()
        pipes.forEach { it.toPhone.trySend(line) }
    }

    fun dropAll() = pipes.forEach { it.close() }

    private suspend fun serve(pipe: Pipe) {
        val challenge = "c" + System.nanoTime()
        pipe.toPhone.send(buildJsonObject {
            put("v", 1); put("relay", "remote"); put("host", "desk"); put("host_id", "h1"); put("instance", "dev"); put("version", "4.0")
            put("challenge", challenge)
            engine?.let { put("engine", it) }
        }.toString())
        val hello = Wire.parse(pipe.toPc.receiveCatching().getOrNull() ?: return) ?: return
        if (hello.s("pair") != null) {
            pipe.toPhone.send(buildJsonObject { put("v", 1); put("ok", true); put("device", device); put("token", token); ops?.let { put("ops", it) } }.toString())
            return pipe.close()
        }
        val ok = hello.s("device") == device && hello.s("proof") == Wire.proof(challenge, token)
        pipe.toPhone.send(buildJsonObject {
            put("v", 1); put("ok", ok)
            if (ok) put("device", device) else put("error", "auth.bad_proof")
            if (ok) ops?.let { put("ops", it) }
        }.toString())
        if (!ok) return pipe.close()
        for (line in pipe.toPc) {
            val req = Wire.parse(line) ?: continue
            val id = req.s("id")!!
            answered[id]?.let { pipe.toPhone.send(it.replaceFirst("{\"v\":1,", "{\"v\":1,\"replayed\":true,")); continue }
            val op = req.s("op")!!
            val payload = req["payload"] as? JsonObject ?: JsonObject(emptyMap())
            requests += op to payload
            val answer = try {
                val result = handle(op, payload)
                buildJsonObject { put("v", 1); put("id", id); put("ok", true); put("result", result) }
            } catch (e: Refuse) {
                buildJsonObject {
                    put("v", 1); put("id", id); put("ok", false)
                    put("error", buildJsonObject {
                        put("kind", e.kind); put("code", e.code); put("message", e.message ?: "")
                        e.confirm?.let { put("confirm", it) }
                    })
                }
            }
            val text = answer.toString()
            if (!op.startsWith("bus.") && !op.endsWith(".list") && !op.endsWith(".get")) answered[id] = text
            pipe.toPhone.send(text)
        }
    }

    class Refuse(val kind: String, val code: String, message: String = code, val confirm: JsonObject? = null) : Exception(message)
}

fun obj(vararg pairs: Pair<String, Any?>): JsonObject = buildJsonObject {
    for ((k, v) in pairs) when (v) {
        null -> Unit
        is String -> put(k, v)
        is Number -> put(k, v)
        is Boolean -> put(k, v)
        is JsonElement -> put(k, v)
        is List<*> -> put(k, kotlinx.serialization.json.JsonArray(v.map { e -> if (e is JsonElement) e else JsonPrimitive(e.toString()) }))
        else -> put(k, v.toString())
    }
}
