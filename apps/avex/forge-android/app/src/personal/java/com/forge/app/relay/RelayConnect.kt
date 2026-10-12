package com.forge.app.relay

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.IOException
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.TimeUnit

/**
 * PERSONAL BUILD ONLY. Antho's server: the bot beside Relay's cloud copy (antho976/antho-s-bot,
 * `/api/connect`). The public build never holds this address, nor anything in this source set.
 */
const val RELAY_SERVER = "https://antho-s-bot-8dve.onrender.com"

/** A request the owner has not answered yet, and the secret that collects its link. */
data class ConnectRequest(val id: String, val code: String, val notified: Boolean, val verifier: String)

/** Where a request stands. */
sealed interface ConnectReply {
    data object Waiting : ConnectReply
    data object Denied : ConnectReply
    data object Expired : ConnectReply
    data object Spent : ConnectReply
    data class Ready(val link: String) : ConnectReply
    data class Trouble(val reason: String) : ConnectReply
}

/**
 * PERSONAL BUILD ONLY. Asking Antho's server to let this phone send its training history
 * (antho976/Relay, docs/CLOUD.md, "Connecting through your server"): the phone keeps a random
 * secret and sends only its sha256; Antho taps Allow on the Discord DM the server sends; the phone
 * then collects a pairing link for the cloud copy with the secret, and pairs with it.
 */
class RelayConnect(private val server: String = RELAY_SERVER) {

    // Render wakes a sleeping service on the first request; give it the time it takes.
    private val http = OkHttpClient.Builder().callTimeout(45, TimeUnit.SECONDS).build()

    private suspend fun post(path: String, body: JSONObject): Pair<Int, JSONObject?> = withContext(Dispatchers.IO) {
        val request = Request.Builder().url(server + path).post(body.toString().toRequestBody(JSON)).build()
        try {
            http.newCall(request).execute().use { r -> r.code to runCatching { JSONObject(r.body?.string().orEmpty()) }.getOrNull() }
        } catch (e: IOException) {
            throw RelayFailure("Your server could not be reached. Check the connection and try again.")
        }
    }

    /** Asks Antho to let this phone in. */
    suspend fun ask(deviceName: String): ConnectRequest {
        val verifier = newVerifier()
        val body = JSONObject().put("kind", "avex").put("device", deviceName).put("challenge", challengeOf(verifier))
        var (status, o) = post("/api/connect/enroll", body)
        // Tally or the Relay app on this phone may have asked a moment before.
        repeat(3) {
            if (status == 429 && o?.optString("error") == "slow_down") {
                delay(6_000)
                post("/api/connect/enroll", body).let { status = it.first; o = it.second }
            }
        }
        val answer = o
        if (status == 201 && answer != null && answer.has("id")) {
            return ConnectRequest(answer.getString("id"), answer.optString("code"), answer.optBoolean("notified"), verifier)
        }
        throw RelayFailure(
            when (answer?.optString("error")) {
                "cloud_off" -> "Your server's Relay cloud copy is not running."
                "busy" -> "Your server has requests waiting already. Answer those, then try again."
                "slow_down" -> "Something on your network keeps asking. Wait a minute and try again."
                "off" -> "Your server does not take requests to connect: it is turned off there."
                else -> "Your server could not take the request ($status)"
            },
        )
    }

    /** Where [request] stands; once allowed, the pairing link. */
    suspend fun claim(request: ConnectRequest): ConnectReply {
        val (status, o) = try {
            post("/api/connect/claim", JSONObject().put("id", request.id).put("verifier", request.verifier))
        } catch (e: RelayFailure) {
            return ConnectReply.Trouble(e.reason)
        }
        o ?: return ConnectReply.Trouble("Your server's answer did not read ($status)")
        return reply(o, status)
    }

    companion object {
        private val JSON = "application/json".toMediaType()

        fun reply(o: JSONObject, status: Int): ConnectReply = when (o.optString("status")) {
            "waiting" -> ConnectReply.Waiting
            "denied" -> ConnectReply.Denied
            "expired" -> ConnectReply.Expired
            "spent" -> ConnectReply.Spent
            "ready" -> o.optString("link").takeIf { it.startsWith("relay://pair?") }?.let { ConnectReply.Ready(it) }
                ?: ConnectReply.Trouble("Your server's link did not read")
            "error" -> ConnectReply.Trouble(o.optString("reason").ifEmpty { "Your server could not answer" })
            else -> ConnectReply.Trouble("Your server answered $status")
        }

        fun newVerifier(): String = ByteArray(32).also { SecureRandom().nextBytes(it) }.hex()

        /** The sha256 of the verifier as text: what the server keeps and compares. */
        fun challengeOf(verifier: String): String =
            MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.UTF_8)).hex()

        private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it) }
    }
}
