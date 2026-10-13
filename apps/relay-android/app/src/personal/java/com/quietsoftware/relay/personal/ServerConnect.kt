package com.quietsoftware.relay.personal

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
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.TimeUnit

/**
 * PERSONAL BUILD ONLY. Antho's server: the bot beside Relay's cloud copy (antho976/antho-s-bot,
 * `/api/connect`). The release build never holds this address (CI checks its APK for it).
 */
const val SERVER = "https://antho-s-bot-8dve.onrender.com"

class ConnectFailure(message: String) : Exception(message)

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
 * PERSONAL BUILD ONLY. Asking the owner's server to pair this phone with the owner's PC (docs/CLOUD.md,
 * "Connecting through your server"): the phone keeps a random secret and sends only its sha256; the
 * owner taps Allow on the Discord DM the server sends, the PC opens a pairing window and hands its
 * link over through the server, and the phone collects it with the secret. The PC then asks on its
 * own screen before it pairs the phone.
 */
class ServerConnect(
    private val server: String = SERVER,
    base: OkHttpClient = OkHttpClient(),
) {
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
            throw ConnectFailure("Your server could not be reached. Check the connection and try again.")
        }
    }

    private fun post(path: String, body: JsonObject): Request =
        Request.Builder().url(server + path).post(body.toString().toRequestBody(JSON)).build()

    /** Whether a PC checks in at the server, so a request can reach it. */
    suspend fun pcConnected(): Boolean {
        val (status, o) = call(Request.Builder().url("$server/api/connect").get().build())
        if (status != 200 || o?.str("service") != "relay-connect") throw ConnectFailure("Your server does not answer as expected ($status)")
        return o.bool("pc") == true
    }

    /** Asks the owner to let this phone pair with the PC. */
    suspend fun ask(deviceName: String): Asked {
        val verifier = newVerifier()
        val body = buildJsonObject {
            put("kind", "relay")
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
        throw ConnectFailure(
            when (answer?.str("error")) {
                "no_pc" -> "No PC is connected to your server yet. On the PC, run: relay cloud connect ${server.removePrefix("https://")}"
                "busy" -> "Your server has requests waiting already. Answer those, then try again."
                "slow_down" -> "Something on your network keeps asking. Wait a minute and try again."
                "off" -> "Your server does not take requests to connect: it is turned off there."
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
