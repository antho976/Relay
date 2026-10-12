package com.tally.link

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.UUID
import java.util.concurrent.TimeUnit
import okhttp3.Response as HttpResponse

/**
 * Why talking to Relay failed, in words a status line can show as they are. Tally talks to the
 * cloud copy of Relay on the owner's server (docs in the bot repo, deploy/relay/README.md), whose
 * door is the same as a PC's.
 */
class RelayFailure(
    val reason: String,
    /** The door said this phone is no longer paired: retrying will not help, pairing again will. */
    val unpaired: Boolean = false,
    /** The bus error's code when Relay refused a request (`money.sync_stale`, …). */
    val code: String? = null,
) : Exception(reason)

/** One admitted connection: the greeting it opened with, and the bus behind it. */
class RelaySession internal constructor(
    private val socket: RelaySocket,
    val greeting: Greeting,
    val welcome: Welcome,
) : AutoCloseable {

    /** Sends one bus request and waits for its response; lines meant for anything else are skipped. */
    suspend fun request(op: String, payload: JsonObject, timeoutMs: Long = REQUEST_TIMEOUT_MS): JsonElement? {
        val id = UUID.randomUUID().toString()
        socket.send(RelayWire.request(op, payload, id))
        val response = try {
            withTimeout(timeoutMs) {
                var found: Response? = null
                while (found == null) found = RelayWire.response(socket.next(), id)
                found
            }
        } catch (e: TimeoutCancellationException) {
            throw RelayFailure("Your server did not answer in time")
        }
        if (!response.ok) {
            val reason = when (response.errorCode) {
                "remote.op" -> "Your server's Relay does not take Tally's sync. Update Relay on the server."
                "bus.unknown_op" -> "Your server's Relay does not know Tally's sync. Update Relay on the server."
                else -> response.errorMessage ?: response.errorCode ?: "Your server refused the sync"
            }
            throw RelayFailure(reason, code = response.errorCode)
        }
        return response.result
    }

    override fun close() = socket.close()

    companion object {
        /** A large first sync is one request; Relay answers it in one transaction. */
        const val REQUEST_TIMEOUT_MS = 60_000L
    }
}

/**
 * The door's client: try each route in order until a door greets, answer with [hello], and hand
 * back the admitted connection. A refusal after the hello is final (a spent code, a revoked
 * phone); a route that cannot be reached moves on to the next. One per app.
 */
class RelayClient(base: OkHttpClient = OkHttpClient()) {

    private val http: OkHttpClient = base.newBuilder()
        .connectTimeout(CONNECT_TIMEOUT_S, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS)
        // The door pings every 30 s and closes a phone silent for 90; answering is OkHttp's.
        .pingInterval(25, TimeUnit.SECONDS)
        .build()

    suspend fun connect(
        routes: List<String>,
        hello: (Greeting) -> String,
        welcomeTimeoutMs: Long = WELCOME_TIMEOUT_MS,
        /** Pairing: once the code went out it may be spent, so no other route is tried. */
        oneShot: Boolean = false,
    ): RelaySession {
        if (routes.isEmpty()) throw RelayFailure("There is no address to reach your server at. Connect again.")
        var last = "Your server could not be reached"
        for (route in routes) {
            val socket = try {
                RelaySocket.open(http, route)
            } catch (e: IllegalArgumentException) {
                last = "That address does not read as a server's"
                continue
            }
            val greeting = try {
                withTimeout(GREETING_TIMEOUT_MS) { RelayWire.greeting(socket.next()) }
            } catch (e: CancellationException) {
                socket.close()
                if (e !is TimeoutCancellationException) throw e
                last = "Your server did not answer in time"
                continue
            } catch (e: Exception) {
                socket.close()
                last = "Your server could not be reached"
                continue
            }
            if (greeting == null || greeting.refusal != null) {
                socket.close()
                last = when {
                    greeting == null -> "Something answered at that address, but not Relay"
                    greeting.refusal == "host.offline" -> "Relay is not running on your server right now"
                    else -> "Your server refused the connection (${greeting.refusal})"
                }
                continue
            }
            socket.send(hello(greeting))
            val welcome = try {
                withTimeout(welcomeTimeoutMs) { RelayWire.welcome(socket.next()) }
            } catch (e: CancellationException) {
                socket.close()
                if (e !is TimeoutCancellationException) throw e
                last = "Your server did not answer in time"
                if (oneShot) break else continue
            } catch (e: Exception) {
                socket.close()
                last = "Your server could not be reached"
                if (oneShot) break else continue
            }
            if (welcome == null || !welcome.ok) {
                socket.close()
                throw denial(welcome?.error)
            }
            return RelaySession(socket, greeting, welcome)
        }
        throw RelayFailure(last)
    }

    companion object {
        const val CONNECT_TIMEOUT_S = 10L
        const val GREETING_TIMEOUT_MS = 15_000L
        const val WELCOME_TIMEOUT_MS = 20_000L
        /** A pairing may wait on a person at the other end: the door gives them 75 s. */
        const val PAIR_WELCOME_TIMEOUT_MS = 90_000L

        /** The door's refusal codes (crates/relay-remote/src/bridge.rs), in plain words. */
        fun denial(code: String?): RelayFailure = when (code) {
            "pair.invalid" -> RelayFailure("The pairing code was wrong or has expired. Connect again.")
            "pair.declined" -> RelayFailure("Your server declined this phone.")
            "pair.unconfirmed" -> RelayFailure("Nobody approved this phone in time. Connect again.")
            "auth.unknown_device" -> RelayFailure("Your server no longer knows this phone. Connect it again.", unpaired = true)
            "auth.bad_proof" -> RelayFailure("Your server no longer accepts this phone's key. Connect it again.", unpaired = true)
            null -> RelayFailure("Something answered at that address, but not Relay")
            else -> RelayFailure("Your server refused the connection ($code)")
        }
    }
}

/** A WebSocket as a queue of lines: what the door sends arrives as [next], in order. */
internal class RelaySocket private constructor() : WebSocketListener() {

    private val lines = Channel<String>(Channel.UNLIMITED)
    private lateinit var ws: WebSocket

    suspend fun next(): String = try {
        lines.receive()
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        // Closed by the server, or the link failed under it: the same thing to the person waiting.
        throw RelayFailure("Your server closed the connection")
    }

    fun send(line: String) {
        if (!ws.send(line)) throw RelayFailure("Your server closed the connection")
    }

    fun close() {
        ws.close(1000, null)
        lines.close()
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        // One line per message, but a frame holding several is several lines.
        text.split('\n').filter { it.isNotBlank() }.forEach { lines.trySend(it) }
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: HttpResponse?) {
        lines.close(t)
    }

    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
        webSocket.close(1000, null)
        lines.close()
    }

    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
        lines.close()
    }

    companion object {
        /** Throws [IllegalArgumentException] for a URL OkHttp cannot read. */
        fun open(http: OkHttpClient, url: String): RelaySocket {
            val socket = RelaySocket()
            socket.ws = http.newWebSocket(Request.Builder().url(url).build(), socket)
            return socket
        }
    }
}
