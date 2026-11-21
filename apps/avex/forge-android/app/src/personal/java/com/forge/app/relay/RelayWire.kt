package com.forge.app.relay

import org.json.JSONObject
import java.net.URLDecoder
import java.security.MessageDigest
import java.util.UUID

/*
 * PERSONAL BUILD ONLY (src/personal). The public Avex holds no INTERNET permission and none of this
 * code; see RELAY.md beside this source set.
 *
 * Relay's door, wire v1 (antho976/Relay, crates/relay-remote/src/wire.rs): the engine greets with a
 * challenge, Avex answers with a pairing code once or a proof every time after, the engine admits
 * it, and then each line is one bus request or response. The same handshake Tally speaks
 * (apps/tally/.../sync/RelayWire.kt); this is the part of it Avex needs.
 */
object RelayWire {
    const val V = 1

    /** What a device sends instead of its token: sha256 of "challenge:token", lowercase hex. */
    fun proof(challenge: String, token: String): String =
        MessageDigest.getInstance("SHA-256").digest("$challenge:$token".toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }

    fun pairHello(code: String, deviceName: String): String =
        JSONObject().put("v", V).put("pair", code.trim()).put("device_name", deviceName).toString()

    fun proofHello(device: String, challenge: String, token: String): String =
        JSONObject().put("v", V).put("device", device).put("proof", proof(challenge, token)).toString()

    /** A bus request as Avex sends it: always the user, a fresh id per request. */
    fun request(op: String, payload: JSONObject, id: String = UUID.randomUUID().toString()): String =
        JSONObject().put("v", V).put("id", id).put("actor", "user").put("op", op).put("payload", payload).toString()

    fun parse(line: String): JSONObject? = runCatching { JSONObject(line) }.getOrNull()
}

/** The engine's first line: its challenge, or a rendezvous saying nobody is behind it. */
data class Greeting(val host: String, val challenge: String?, val refusal: String?) {
    companion object {
        fun from(line: String): Greeting? {
            val o = RelayWire.parse(line) ?: return null
            if (o.has("ok") && !o.optBoolean("ok", true)) return Greeting("", null, o.optString("error", "refused"))
            if (o.optString("relay") != "remote") return null
            val challenge = o.optString("challenge").takeIf { it.isNotEmpty() } ?: return null
            return Greeting(o.optString("host"), challenge, null)
        }
    }
}

data class Welcome(val ok: Boolean, val device: String?, val token: String?, val error: String?) {
    companion object {
        fun from(line: String): Welcome? {
            val o = RelayWire.parse(line) ?: return null
            if (!o.has("ok")) return null
            return Welcome(
                ok = o.getBoolean("ok"),
                device = o.optString("device").takeIf { it.isNotEmpty() },
                token = o.optString("token").takeIf { it.isNotEmpty() },
                error = o.optString("error").takeIf { it.isNotEmpty() },
            )
        }
    }
}

/**
 * A `relay://pair` link, as `relay remote pair --via-only` prints it on the server. Avex reaches
 * the cloud copy through its rendezvous only (`via`); direct addresses are ignored.
 */
data class PairLink(val code: String, val host: String, val via: String) {
    companion object {
        fun parse(text: String): PairLink? {
            val match = Regex("^relay://pair\\?(.*)$", RegexOption.IGNORE_CASE).find(text.trim()) ?: return null
            val params = HashMap<String, String>()
            for (pair in match.groupValues[1].split('&')) {
                if (pair.isEmpty()) continue
                val eq = pair.indexOf('=')
                val key = decode(if (eq < 0) pair else pair.substring(0, eq))
                params[key] = decode(if (eq < 0) "" else pair.substring(eq + 1))
            }
            val code = params["code"]?.takeIf { it.isNotBlank() } ?: return null
            if (params["v"] != "1") return null
            val via = params["via"]?.takeIf { Regex("^wss?://.+", RegexOption.IGNORE_CASE).matches(it) } ?: return null
            return PairLink(code, params["host"]?.takeIf { it.isNotBlank() } ?: "Relay", via)
        }

        private fun decode(s: String): String = runCatching { URLDecoder.decode(s, "UTF-8") }.getOrDefault(s)
    }
}

/** Why talking to Relay failed, in words the screen can show as they are. */
class RelayFailure(val reason: String, val unpaired: Boolean = false) : Exception(reason) {
    companion object {
        /** The door's refusal codes (crates/relay-remote/src/bridge.rs), in plain words. */
        fun denial(code: String?): RelayFailure = when (code) {
            "pair.invalid" -> RelayFailure("The pairing code was wrong or has expired. Run the pair command on the server again.")
            "pair.declined" -> RelayFailure("The server declined this pairing.")
            "pair.unconfirmed" -> RelayFailure("Nobody approved the pairing on the server in time.")
            "auth.unknown_device", "auth.bad_proof" -> RelayFailure("Relay no longer knows this phone. Pair it again.", unpaired = true)
            null -> RelayFailure("Something answered, but not Relay")
            else -> RelayFailure("Relay refused the connection ($code)")
        }
    }
}
