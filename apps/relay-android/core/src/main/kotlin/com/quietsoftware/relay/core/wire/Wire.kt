package com.quietsoftware.relay.core.wire

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.security.MessageDigest
import java.util.UUID

/**
 * The phone door's handshake (crates/relay-remote/src/wire.rs) and the bus envelopes it carries
 * afterwards (crates/relay-bus/src/envelope.rs, docs/engine/BUS.md §1). One JSON object per line.
 *
 * Requests and payloads are strict on the PC (`deny_unknown_fields`): nothing here adds a field
 * the engine does not know. Everything the PC sends is read leniently, so a newer engine's extra
 * fields never break an older phone.
 */
object Wire {
    const val V = 1
    const val DEFAULT_PORT = 7420

    /** The op list a PC's door opens (`OpProfile` in wire.rs), as its welcome names it. */
    const val OPS_PC = "phone"

    /** The op list a cloud copy's door opens: the ledger and the training history, nothing else (docs/CLOUD.md). */
    const val OPS_CLOUD = "cloud"

    val json = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
        encodeDefaults = false
        isLenient = false
    }

    /**
     * For reading entities into models: a number where a string id is expected (ids are strings
     * so a temp id fits), and a null where a field has a default, are both taken as they come.
     */
    val lenient = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
        isLenient = true
        coerceInputValues = true
    }

    /** `sha256(challenge:token)` in lower-case hex: what a paired phone shows instead of its token. */
    fun proof(challenge: String, token: String): String =
        MessageDigest.getInstance("SHA-256")
            .digest("$challenge:$token".toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }

    fun pairHello(code: String, deviceName: String): String = buildJsonObject {
        put("v", V)
        put("pair", code.trim())
        put("device_name", deviceName.take(64))
    }.toString()

    fun proofHello(device: String, challenge: String, token: String): String = buildJsonObject {
        put("v", V)
        put("device", device)
        put("proof", proof(challenge, token))
    }.toString()

    fun request(op: String, payload: JsonObject, id: String = UUID.randomUUID().toString()): String = buildJsonObject {
        put("v", V)
        put("id", id)
        put("actor", "user")
        put("op", op)
        put("payload", payload)
    }.toString()

    /** The greeting the door sends first; null when the line is not one (or a rendezvous says the PC is away). */
    fun greeting(line: String): Greeting? {
        val o = parse(line) ?: return null
        if (o.str("relay") != "remote") return null
        val challenge = o.str("challenge") ?: return null
        return Greeting(
            host = o.str("host").orEmpty(),
            hostId = o.str("host_id").orEmpty(),
            instance = o.str("instance").orEmpty(),
            version = o.str("version").orEmpty(),
            challenge = challenge,
            engine = o.str("engine"),
            wake = o["wake"]?.let { w -> runCatching { w.jsonArray.mapNotNull { WakeTarget.from(it) } }.getOrNull() }.orEmpty(),
        )
    }

    /** A rendezvous with no PC behind it answers `{ok:false, error:"host.offline"}` in place of a greeting. */
    fun offline(line: String): String? {
        val o = parse(line) ?: return null
        if (o.bool("ok") == false) return o.str("error") ?: "host.offline"
        return null
    }

    fun welcome(line: String): Welcome? {
        val o = parse(line) ?: return null
        val ok = o.bool("ok") ?: return null
        return Welcome(
            ok = ok, device = o.str("device"), token = o.str("token"), error = o.str("error"),
            host = o.str("host"), hostId = o.str("host_id"), version = o.str("version"),
            wake = o["wake"]?.let { w -> runCatching { w.jsonArray.mapNotNull { WakeTarget.from(it) } }.getOrNull() },
            ops = o.str("ops"),
        )
    }

    /** What a line from the engine is, told apart by the key that follows `v` (envelope.rs). */
    fun classify(line: String): Incoming? {
        val o = parse(line) ?: return null
        return when {
            o.containsKey("ev") -> Incoming.Ev(
                Event(
                    ev = o.str("ev").orEmpty(),
                    ts = o.str("ts").orEmpty(),
                    actor = o.str("actor").orEmpty(),
                    cause = o.str("cause"),
                    projectId = o.long("project_id"),
                    payload = (o["payload"] as? JsonObject) ?: JsonObject(emptyMap()),
                ),
            )
            o.containsKey("stream") -> Incoming.Fr(
                Frame(
                    stream = o.str("stream").orEmpty(),
                    session = o.str("session"),
                    runId = o.long("run_id"),
                    epoch = o.long("epoch"),
                    seq = o.long("seq") ?: 0,
                    data = o["data"] ?: JsonNull,
                ),
            )
            o.containsKey("ok") -> Incoming.Res(
                Response(
                    id = o.str("id"),
                    ok = o.bool("ok") == true,
                    result = o["result"] ?: JsonNull,
                    error = (o["error"] as? JsonObject)?.let { BusError.from(it) },
                    replayed = o.bool("replayed") == true,
                ),
            )
            else -> null
        }
    }

    fun parse(line: String): JsonObject? =
        runCatching { json.parseToJsonElement(line) as? JsonObject }.getOrNull()
}

data class Greeting(
    val host: String,
    val hostId: String,
    val instance: String,
    val version: String,
    val challenge: String,
    /** `running` or `starting` when the door fronts an engine it can start (docs/ANDROID.md). */
    val engine: String? = null,
    /** The PC's network cards, for Wake-on-LAN from this phone when the PC is asleep. */
    val wake: List<WakeTarget> = emptyList(),
    /** Which ops the door opens ([Wire.OPS_PC] or [Wire.OPS_CLOUD]), once the welcome said; null from an older door. */
    val ops: String? = null,
) {
    /** A door this new names the PC in the welcome, not the greeting; an older one in the greeting. */
    fun admittedBy(w: Welcome): Greeting =
        copy(host = w.host ?: host, hostId = w.hostId ?: hostId, version = w.version ?: version, wake = w.wake ?: wake, ops = w.ops ?: ops)
}

data class WakeTarget(val mac: String, val broadcast: String?) {
    companion object {
        fun from(e: JsonElement): WakeTarget? {
            val o = e as? JsonObject ?: return null
            val mac = o.str("mac") ?: return null
            return WakeTarget(mac, o.str("broadcast"))
        }
    }
}

data class Welcome(
    val ok: Boolean,
    val device: String?,
    val token: String?,
    val error: String?,
    val host: String? = null,
    val hostId: String? = null,
    val version: String? = null,
    val wake: List<WakeTarget>? = null,
    /** The door's op list, in an admitted device's welcome: a PC's or a cloud copy's. */
    val ops: String? = null,
)

sealed interface Incoming {
    data class Res(val response: Response) : Incoming
    data class Ev(val event: Event) : Incoming
    data class Fr(val frame: Frame) : Incoming
}

data class Response(
    val id: String?,
    val ok: Boolean,
    val result: JsonElement,
    val error: BusError?,
    val replayed: Boolean,
)

data class Event(
    val ev: String,
    val ts: String,
    val actor: String,
    val cause: String?,
    val projectId: Long?,
    val payload: JsonObject,
)

data class Frame(
    val stream: String,
    val session: String?,
    val runId: Long?,
    val epoch: Long?,
    val seq: Long,
    val data: JsonElement,
)

/** BUS.md §1.2. `kind` is one of invalid, not_found, conflict, refused, held, unavailable, internal. */
data class BusError(
    val kind: String,
    val code: String,
    val message: String,
    val hint: String? = null,
    val details: JsonObject? = null,
    val confirm: JsonObject? = null,
) {
    /**
     * Worth sending again later: the link was down or timed out, the PC was quitting, or it was
     * busy for a moment. Every other `unavailable` (a provider not installed, a spawn or a clone
     * that failed, no Git head) is the PC's lasting answer, and asking again gets it again.
     */
    val transient: Boolean get() = code.startsWith("link.") || code == "app.quitting" ||
        (kind == "unavailable" && (code.endsWith(".busy") || code.endsWith(".rate_limited")))

    companion object {
        fun from(o: JsonObject): BusError {
            val code = o.str("code") ?: "unknown"
            return BusError(
            kind = o.str("kind") ?: "internal",
            code = code,
            // The door's own wording ("… is not open to a paired phone") reads as a fault; it means
            // the PC runs a Relay older than this app.
            message = if (code == "remote.op") UPDATE_PC else o.str("message").orEmpty(),
            hint = o.str("hint"),
            details = o["details"] as? JsonObject,
            confirm = o["confirm"] as? JsonObject,
            )
        }

        const val UPDATE_PC = "Your PC runs an older Relay that does not open this to a phone yet. Update Relay on the PC and restart it to use it here."

        fun link(code: String, message: String) = BusError("unavailable", code, message)
    }
}

class BusException(val error: BusError) : Exception("${error.code}: ${error.message}")

internal fun JsonObject.str(key: String): String? = (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.contentOrNull
internal fun JsonObject.bool(key: String): Boolean? = (this[key] as? JsonPrimitive)?.booleanOrNull
internal fun JsonObject.long(key: String): Long? = (this[key] as? JsonPrimitive)?.longOrNull
internal fun JsonObject.int(key: String): Int? = (this[key] as? JsonPrimitive)?.intOrNull

/** Leniently read a field of a result: the PC may add fields or send a number as a string in a later version. */
fun JsonElement?.obj(): JsonObject? = this as? JsonObject
fun JsonElement?.text(): String? = (this as? JsonPrimitive)?.contentOrNull?.takeIf { (this as JsonPrimitive).isString || it.isNotEmpty() }
fun JsonObject.s(key: String): String? = (this[key] as? JsonPrimitive)?.contentOrNull
fun JsonObject.l(key: String): Long? = (this[key] as? JsonPrimitive)?.longOrNull
fun JsonObject.b(key: String): Boolean? = (this[key] as? JsonPrimitive)?.booleanOrNull
fun JsonObject.arr(key: String): List<JsonElement> = runCatching { this[key]?.jsonArray?.toList() }.getOrNull().orEmpty()
fun JsonObject.o(key: String): JsonObject? = this[key] as? JsonObject
fun JsonElement.asObj(): JsonObject = jsonObject
fun JsonElement.asStr(): String? = runCatching { jsonPrimitive.contentOrNull }.getOrNull()
