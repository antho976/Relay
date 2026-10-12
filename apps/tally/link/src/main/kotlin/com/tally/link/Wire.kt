package com.tally.link

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import java.net.URLDecoder
import java.security.MessageDigest
import java.util.UUID

/*
 * Relay's phone door, wire v1 (crates/relay-remote/src/wire.rs): the PC greets with a challenge,
 * the phone answers with a pairing code once or a proof every time after, the PC admits it, and
 * then each line is one bus request or response (crates/relay-bus/src/envelope.rs). The same
 * handshake apps/relay-mobile speaks; this is the part of it Tally needs, as plain functions.
 */

object RelayWire {
    const val V = 1

    /** The port `relay remote serve` listens on unless told otherwise. */
    const val DEFAULT_PORT = 7420

    private val json = Json { ignoreUnknownKeys = true }

    /** What a phone sends instead of its token: sha256 of "challenge:token", lowercase hex. */
    fun proof(challenge: String, token: String): String =
        MessageDigest.getInstance("SHA-256").digest("$challenge:$token".toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }

    /** Pairing: the code, and the name the PC lists this phone under. */
    fun pairHello(code: String, deviceName: String): String = buildJsonObject {
        put("v", V)
        put("pair", code.trim())
        put("device_name", deviceName)
    }.toString()

    /** Every connection after pairing: the device id and a proof good for this challenge only. */
    fun proofHello(device: String, challenge: String, token: String): String = buildJsonObject {
        put("v", V)
        put("device", device)
        put("proof", proof(challenge, token))
    }.toString()

    /** A bus request as the phone sends it: always the user, a fresh id per request. */
    fun request(op: String, payload: JsonObject, id: String = UUID.randomUUID().toString()): String = buildJsonObject {
        put("v", V)
        put("id", id)
        put("actor", "user")
        put("op", op)
        put("payload", payload)
    }.toString()

    fun parse(line: String): JsonObject? = runCatching { json.parseToJsonElement(line).jsonObject }.getOrNull()

    /**
     * The PC's first line. A rendezvous with nobody behind it answers `{ok:false, error}` instead,
     * which reads as [Greeting.refusal].
     */
    fun greeting(line: String): Greeting? {
        val o = parse(line) ?: return null
        if (o.bool("ok") == false) return Greeting(refusal = o.str("error") ?: "refused")
        if (o.str("relay") != "remote") return null
        val challenge = o.str("challenge") ?: return null
        return Greeting(
            host = o.str("host").orEmpty(),
            hostId = o.str("host_id").orEmpty(),
            instance = o.str("instance").orEmpty(),
            version = o.str("version").orEmpty(),
            challenge = challenge,
        )
    }

    fun welcome(line: String): Welcome? {
        val o = parse(line) ?: return null
        val ok = o.bool("ok") ?: return null
        return Welcome(ok = ok, device = o.str("device"), token = o.str("token"), error = o.str("error"))
    }

    /** The response to request [id], or null when [line] is anything else (an event, a frame). */
    fun response(line: String, id: String): Response? {
        val o = parse(line) ?: return null
        if (o.str("id") != id) return null
        val ok = o.bool("ok") ?: return null
        val error = o["error"] as? JsonObject
        return Response(
            ok = ok,
            result = o["result"],
            errorCode = error?.str("code"),
            errorMessage = error?.str("message"),
        )
    }

    private fun JsonObject.str(key: String): String? = (this[key] as? JsonPrimitive)?.contentOrNull
    private fun JsonObject.bool(key: String): Boolean? = (this[key] as? JsonPrimitive)?.booleanOrNull
}

data class Greeting(
    val host: String = "",
    val hostId: String = "",
    val instance: String = "",
    val version: String = "",
    val challenge: String = "",
    /** Set when the route answered but there is no PC behind it (`host.offline`). */
    val refusal: String? = null,
)

data class Welcome(val ok: Boolean, val device: String?, val token: String?, val error: String?)

data class Response(val ok: Boolean, val result: JsonElement?, val errorCode: String?, val errorMessage: String?)

/**
 * Where a PC can be reached and the code that pairs with it: the `relay://pair?…` link
 * `relay remote pair` prints (crates/relay-remote/src/pairlink.rs), or an address and a code
 * typed by hand.
 */
data class PairLink(
    val code: String,
    val host: String,
    val hostId: String,
    val instance: String,
    /** `ws://192.168.1.20:7420`, …: the same network, or a tailnet. Tried first. */
    val direct: List<String>,
    /** `wss://server/join/<room>`: the PC's own rendezvous server, if it has one. */
    val via: String?,
) {
    val routes: List<String> get() = direct + listOfNotNull(via)

    companion object {
        /** A `relay://pair` link, or null for anything else, so a stray paste is not misread. */
        fun parse(text: String): PairLink? {
            val match = Regex("^relay://pair\\?(.*)$", RegexOption.IGNORE_CASE).find(text.trim()) ?: return null
            val params = HashMap<String, String>()
            for (pair in match.groupValues[1].split('&')) {
                if (pair.isEmpty()) continue
                val eq = pair.indexOf('=')
                val key = decode(if (eq < 0) pair else pair.substring(0, eq))
                params[key] = decode(if (eq < 0) "" else pair.substring(eq + 1))
            }
            val code = params["code"]
            if (params["v"] != "1" || code.isNullOrBlank()) return null
            val ws = Regex("^wss?://.+", RegexOption.IGNORE_CASE)
            val direct = params["direct"].orEmpty().split(',').map { it.trim() }.filter { ws.matches(it) }
            val via = params["via"]?.takeIf { ws.matches(it) }
            if (direct.isEmpty() && via == null) return null
            return PairLink(
                code = code,
                host = params["host"]?.takeIf { it.isNotBlank() } ?: "Relay PC",
                hostId = params["id"].orEmpty(),
                instance = params["instance"]?.takeIf { it.isNotBlank() } ?: "stable",
                direct = direct,
                via = via,
            )
        }

        /** An address and a code typed by hand. Null when the address does not read. */
        fun manual(address: String, code: String): PairLink? {
            val route = address(address) ?: return null
            if (code.none { it.isLetterOrDigit() }) return null
            return PairLink(code.trim(), "Relay PC", "", "stable", listOf(route), null)
        }

        /**
         * What a person types for the PC: `192.168.1.20`, `192.168.1.20:7420`, `ws://…`,
         * `wss://…`, or `http(s)://…`. The port defaults to the door's own.
         */
        fun address(text: String): String? {
            val t = text.trim()
            if (t.isEmpty()) return null
            if (Regex("^wss?://\\S+$", RegexOption.IGNORE_CASE).matches(t)) return t.trimEnd('/')
            if (Regex("^https?://\\S+$", RegexOption.IGNORE_CASE).matches(t)) return "ws" + t.substring(4).trimEnd('/')
            if (Regex("^[a-z0-9.-]+(:\\d{1,5})?$", RegexOption.IGNORE_CASE).matches(t)) {
                return "ws://" + if (':' in t) t else "$t:${RelayWire.DEFAULT_PORT}"
            }
            return null
        }

        private fun decode(s: String): String = runCatching { URLDecoder.decode(s, "UTF-8") }.getOrDefault(s)
    }
}
