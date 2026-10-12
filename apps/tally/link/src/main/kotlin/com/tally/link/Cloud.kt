package com.tally.link

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.TimeUnit

/*
 * Asking the owner's server to let this phone sync (antho-s-bot, server/features/tally): no shell,
 * no link to paste. The phone makes a random verifier and sends only its sha256; the bot DMs the
 * owner on Discord with Allow and Deny and the four characters both screens show; once allowed,
 * the phone shows the verifier and collects a `relay://pair` link for Relay's cloud copy, which it
 * presents to the door like any pairing link.
 */

/** A request in flight: what the phone keeps until it is answered, so a restart resumes it. */
data class Enrollment(
    val server: String,
    val id: String,
    val verifier: String,
    val code: String,
    val expiresAt: Long,
    /** The bot reached the owner on Discord; false sends them to the dashboard instead. */
    val notified: Boolean,
)

/** What the server said about this phone's request. */
sealed interface ClaimReply {
    data object Waiting : ClaimReply
    data object Denied : ClaimReply
    data object Expired : ClaimReply
    /** Allowed and already used up its links: ask again. */
    data object Spent : ClaimReply
    data class Ready(val link: PairLink) : ClaimReply
    data class Trouble(val reason: String) : ClaimReply
}

/** What `GET /api/tally` says: whether this server carries Relay's cloud copy and takes requests. */
data class ServerHello(val cloud: Boolean)

class CloudFailure(val reason: String, val cloudOff: Boolean = false) : Exception(reason)

class CloudApi(base: OkHttpClient = OkHttpClient()) {

    private val http: OkHttpClient = base.newBuilder()
        .connectTimeout(15, TimeUnit.SECONDS)
        // Render wakes a sleeping service on the first request; give it the time it takes.
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    private val json = Json { ignoreUnknownKeys = true }

    suspend fun hello(server: String): ServerHello {
        val o = call(Request.Builder().url(api(server)).get().build()).second
            ?: throw CloudFailure("That address does not answer as your server")
        if (o.str("service") != "tally-cloud") throw CloudFailure("That address does not answer as your server")
        return ServerHello(cloud = o.bool("cloud") == true)
    }

    /** Asks to sync. The owner gets a DM; the reply carries what to poll [claim] with. */
    suspend fun enroll(server: String, deviceName: String, kind: String = "tally"): Enrollment {
        val verifier = newVerifier()
        val challenge = challengeOf(verifier)
        val body = buildJsonObject {
            put("kind", kind)
            put("device", deviceName.take(100))
            put("challenge", challenge)
        }
        val (status, o) = call(post(api(server) + "/enroll", body))
        if (status == 201 && o != null) {
            val id = o.str("id") ?: throw CloudFailure("Your server's answer did not read")
            return Enrollment(
                server = normalize(server),
                id = id,
                verifier = verifier,
                code = o.str("code") ?: codeOf(challenge),
                expiresAt = o.long("expiresAt") ?: (System.currentTimeMillis() + 15 * 60_000L),
                notified = o.bool("notified") == true,
            )
        }
        throw when (o?.str("error")) {
            "cloud_off" -> CloudFailure("Relay is not running on your server yet", cloudOff = true)
            "off" -> CloudFailure("Your server has Tally sync turned off", cloudOff = true)
            "busy" -> CloudFailure("Your server has requests waiting already. Answer those, then try again.")
            "slow_down" -> CloudFailure("Asked a moment ago. Try again in half a minute.")
            else -> CloudFailure("Your server could not take the request ($status)")
        }
    }

    suspend fun claim(e: Enrollment): ClaimReply {
        val body = buildJsonObject {
            put("id", e.id)
            put("verifier", e.verifier)
        }
        val (status, o) = call(post(api(e.server) + "/claim", body))
        o ?: return ClaimReply.Trouble("Your server's answer did not read ($status)")
        return when (o.str("status")) {
            "waiting" -> ClaimReply.Waiting
            "denied" -> ClaimReply.Denied
            "expired" -> ClaimReply.Expired
            "spent" -> ClaimReply.Spent
            "ready" -> o.str("link")?.let { PairLink.parse(it) }?.let { ClaimReply.Ready(it) }
                ?: ClaimReply.Trouble("Your server sent a link that does not read")
            else -> ClaimReply.Trouble(o.str("reason") ?: "Your server could not open a pairing window")
        }
    }

    private fun post(url: String, body: JsonObject): Request =
        Request.Builder().url(url).post(body.toString().toRequestBody(JSON)).build()

    /** The status and the JSON object the server answered with (null when it was not one). */
    private suspend fun call(request: Request): Pair<Int, JsonObject?> = withContext(Dispatchers.IO) {
        try {
            http.newCall(request).execute().use { r ->
                val text = r.body?.string().orEmpty()
                r.code to runCatching { json.parseToJsonElement(text).jsonObject }.getOrNull()
            }
        } catch (e: IOException) {
            throw CloudFailure("Your server could not be reached")
        } catch (e: IllegalArgumentException) {
            throw CloudFailure("That address does not read as a server's")
        }
    }

    private fun JsonObject.str(key: String): String? = (this[key] as? JsonPrimitive)?.contentOrNull
    private fun JsonObject.bool(key: String): Boolean? = (this[key] as? JsonPrimitive)?.booleanOrNull
    private fun JsonObject.long(key: String): Long? = (this[key] as? JsonPrimitive)?.longOrNull

    companion object {
        private val JSON = "application/json".toMediaType()
        private val random = SecureRandom()

        /** `https://host` for whatever was typed: a bare host, a trailing slash, a wss:// address. */
        fun normalize(server: String): String {
            var s = server.trim().trimEnd('/')
            s = when {
                s.startsWith("wss://", ignoreCase = true) -> "https://" + s.substring(6)
                s.startsWith("ws://", ignoreCase = true) -> "http://" + s.substring(5)
                s.startsWith("https://", ignoreCase = true) || s.startsWith("http://", ignoreCase = true) -> s
                else -> "https://$s"
            }
            // A pasted Relay address (…/relay) or API address names the same server.
            return s.removeSuffix("/relay").removeSuffix("/api/tally").trimEnd('/')
        }

        fun api(server: String): String = normalize(server) + "/api/tally"

        /** 32 random bytes as lowercase hex: what only this phone knows. */
        fun newVerifier(): String {
            val bytes = ByteArray(32)
            random.nextBytes(bytes)
            return hex(bytes)
        }

        fun challengeOf(verifier: String): String =
            hex(MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.UTF_8)))

        /** The four characters the phone and the owner's DM both show. */
        fun codeOf(challenge: String): String = challenge.take(4).uppercase()

        private fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it) }
    }
}
