package com.quietsoftware.relay.data.link

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.TimeUnit

/** [unsupported]: the server does not know what was asked for (a bot older than the request's kind). */
class ConnectFailure(message: String, val unsupported: Boolean = false) : Exception(message)

/** A request the owner has not answered yet, and the secret that collects its link. */
data class Asked(val id: String, val code: String, val notified: Boolean, val expiresAt: Long, val verifier: String)

/** Where a request stands. */
sealed interface Reply {
    data object Waiting : Reply
    /** Allowed on Discord; the PC has not opened its window yet (it must be on, running Relay). */
    data object WaitingPc : Reply
    data object Denied : Reply
    data object Expired : Reply
    data object Spent : Reply
    data class Ready(val link: String) : Reply
    data class Trouble(val reason: String) : Reply
}

/**
 * Asking the owner's server (the bot beside Relay's cloud copy, antho976/antho-s-bot,
 * `/api/connect`) to pair this phone with the owner's PC (docs/CLOUD.md, "Connecting through your
 * server"), the way Tally 2 asks it for the ledger: the phone keeps a random secret and sends only
 * its sha256; the owner taps Allow on the Discord DM the server sends, the PC opens a pairing window
 * and hands its link over through the server, and the phone collects it with the secret. The PC
 * then asks on its own screen before it pairs the phone. [server] is whatever the person typed;
 * [normalize] makes it `https://host`.
 */
class ServerConnect(
    server: String,
    base: OkHttpClient = OkHttpClient(),
) {
    /** `https://host`, however it was typed. */
    val server = normalize(server)

    /** The server's host, as the person would type it. */
    val host: String get() = server.substringAfter("://")

    // Render wakes a sleeping service on the first request; give it the time it takes.
    private val http = base.newBuilder().callTimeout(45, TimeUnit.SECONDS).build()

    /** The status and the JSON object the server answered with (null when it was not one). */
    private suspend fun call(request: Request): Pair<Int, JsonObject?> = withContext(Dispatchers.IO) {
        try {
            http.newCall(request).execute().use { r ->
                val o = runCatching { Json.parseToJsonElement(r.body?.string().orEmpty()).jsonObject }.getOrNull()
                r.code to o
            }
        } catch (e: java.io.IOException) {
            throw ConnectFailure("Your server could not be reached. Check the address and the connection, then try again.")
        }
    }

    private fun url(path: String) = (server + path).toHttpUrlOrNull() ?: throw ConnectFailure("That does not read as a server's address")

    private fun post(path: String, body: JsonObject): Request =
        Request.Builder().url(url(path)).post(body.toString().toRequestBody(JSON)).build()

    /** Whether a PC checks in at the server, so a request can reach it. */
    suspend fun pcConnected(): Boolean {
        val (status, o) = call(Request.Builder().url(url("/api/connect")).get().build())
        if (status != 200 || o?.str("service") != "relay-connect") throw ConnectFailure("That address does not answer as your server ($status)")
        return o.bool("pc") == true
    }

    /**
     * Asks the owner to let this phone pair: with the PC ([KIND_PC]), or with the cloud copy beside
     * the bot ([KIND_CLOUD], a kind the bot must know: antho976/antho-s-bot).
     */
    suspend fun ask(deviceName: String, kind: String = KIND_PC): Asked {
        val verifier = newVerifier()
        val body = buildJsonObject {
            put("kind", kind)
            put("device", deviceName)
            put("challenge", challengeOf(verifier))
        }
        var (status, o) = call(post("/api/connect/enroll", body))
        // Tally or the PC on the same network may have asked a moment before.
        repeat(3) {
            if (status == 429 && o?.str("error") == "slow_down") {
                kotlinx.coroutines.delay(6_000)
                call(post("/api/connect/enroll", body)).let { status = it.first; o = it.second }
            }
        }
        val answer = o
        if (status == 201 && answer != null) {
            val id = answer.str("id") ?: throw ConnectFailure("Your server's answer did not read")
            return Asked(id, answer.str("code").orEmpty(), answer.bool("notified") == true, answer.long("expiresAt") ?: 0, verifier)
        }
        val error = answer?.str("error")
        // A refusal the bot has no word for, of a request it read: it does not know this kind yet.
        if (status in 400..499 && status != 429 && error !in KNOWN_ERRORS) throw ConnectFailure("Your server does not offer this yet ($status)", unsupported = true)
        throw ConnectFailure(
            when (error) {
                "no_pc" -> noPc(host)
                "busy" -> "Your server has requests waiting already. Answer those, then try again."
                "slow_down" -> "Something on your network keeps asking. Wait a minute and try again."
                "off" -> "Your server does not take requests to connect: it is turned off there."
                "cloud_off" -> "Your server's copy is not running there."
                else -> "Your server could not take the request ($status)"
            },
        )
    }

    /** Where [asked] stands; once the PC opened its window, the link to it. */
    suspend fun claim(asked: Asked): Reply {
        val body = buildJsonObject {
            put("id", asked.id)
            put("verifier", asked.verifier)
        }
        val (status, o) = try {
            call(post("/api/connect/claim", body))
        } catch (e: ConnectFailure) {
            return Reply.Trouble(e.message.orEmpty())
        }
        o ?: return Reply.Trouble("Your server's answer did not read ($status)")
        return readReply(o, status)
    }

    companion object {
        private val JSON = "application/json".toMediaType()

        /** The Relay app asking for the PC: the PC opens the window and asks on its own screen. */
        const val KIND_PC = "relay"

        /** The Relay app asking for the cloud copy beside the bot, which answers Tally's and Avex's reads. */
        const val KIND_CLOUD = "relay-cloud"

        private val KNOWN_ERRORS = setOf("no_pc", "busy", "slow_down", "off", "cloud_off")

        fun noPc(host: String) = "No PC is connected to your server yet. On the PC, run: relay cloud connect $host"

        /**
         * `https://host` for whatever was typed or pasted: a bare host, a trailing slash, the
         * `wss://…/relay` address the PC prints, or an `/api/connect` or Tally `/api/tally` address,
         * which all name the same server. Blank stays blank.
         */
        fun normalize(server: String): String {
            var s = server.trim().trimEnd('/')
            if (s.isEmpty()) return ""
            s = when {
                s.startsWith("wss://", ignoreCase = true) -> "https://" + s.substring(6)
                s.startsWith("ws://", ignoreCase = true) -> "http://" + s.substring(5)
                s.startsWith("https://", ignoreCase = true) || s.startsWith("http://", ignoreCase = true) -> s
                else -> "https://$s"
            }
            for (suffix in listOf("/relay", "/api/connect", "/api/tally", "/dashboard/devices", "/dashboard")) {
                if (s.endsWith(suffix, ignoreCase = true)) return s.dropLast(suffix.length).trimEnd('/')
            }
            return s
        }

        fun readReply(o: JsonObject, status: Int): Reply = when (o.str("status")) {
            "waiting" -> Reply.Waiting
            "waiting_pc" -> Reply.WaitingPc
            "denied" -> Reply.Denied
            "expired" -> Reply.Expired
            "spent" -> Reply.Spent
            "ready" -> o.str("link")?.takeIf { it.startsWith("relay://pair?") }?.let { Reply.Ready(it) }
                ?: Reply.Trouble("Your server's link did not read")
            "error" -> Reply.Trouble(o.str("reason") ?: "Your server could not answer")
            else -> Reply.Trouble("Your server answered $status")
        }

        fun newVerifier(): String = ByteArray(32).also { SecureRandom().nextBytes(it) }.hex()

        /** The sha256 of the verifier as text: what the server keeps and compares. */
        fun challengeOf(verifier: String): String =
            MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.UTF_8)).hex()

        /** The four characters the phone and the DM both show. */
        fun codeOf(verifier: String): String = challengeOf(verifier).take(4).uppercase()

        private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it) }
        private fun JsonObject.str(key: String): String? = this[key]?.jsonPrimitive?.takeIf { it.isString }?.content
        private fun JsonObject.bool(key: String): Boolean? = this[key]?.jsonPrimitive?.booleanOrNull
        private fun JsonObject.long(key: String): Long? = this[key]?.jsonPrimitive?.longOrNull
    }
}
