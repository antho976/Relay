package com.forge.app.relay

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import java.util.UUID
import java.util.concurrent.TimeUnit

/**
 * PERSONAL BUILD ONLY. One admitted connection to Relay's cloud copy (antho976/Relay,
 * docs/CLOUD.md), opened, used for a request or two, and closed.
 */
class RelaySession private constructor(private val socket: LineSocket) : AutoCloseable {

    /** Sends one bus request and waits for its answer; anything else on the line is skipped. */
    suspend fun request(op: String, payload: JSONObject, timeoutMs: Long = REQUEST_TIMEOUT_MS): JSONObject? {
        val id = UUID.randomUUID().toString()
        socket.send(RelayWire.request(op, payload, id))
        val answer = try {
            withTimeout(timeoutMs) {
                var found: JSONObject? = null
                while (found == null) {
                    found = RelayWire.parse(socket.next())?.takeIf { it.optString("id") == id && it.has("ok") }
                }
                found
            }
        } catch (e: TimeoutCancellationException) {
            throw RelayFailure("Relay did not answer in time")
        }
        if (!answer.getBoolean("ok")) {
            val error = answer.optJSONObject("error")
            val reason = when (error?.optString("code")) {
                "remote.op", "bus.unknown_op" -> "Relay on the server is too old to take Avex's history. Update it."
                else -> error?.optString("message")?.takeIf { it.isNotEmpty() } ?: "Relay refused the upload"
            }
            throw RelayFailure(reason)
        }
        return answer.optJSONObject("result")
    }

    override fun close() = socket.close()

    companion object {
        const val REQUEST_TIMEOUT_MS = 60_000L
        private const val GREETING_TIMEOUT_MS = 15_000L
        private const val WELCOME_TIMEOUT_MS = 20_000L
        /** A pairing may wait on a person approving it at the server: the door gives them 75 s. */
        const val PAIR_WELCOME_TIMEOUT_MS = 90_000L

        private val http: OkHttpClient by lazy {
            OkHttpClient.Builder()
                .connectTimeout(10, TimeUnit.SECONDS)
                .readTimeout(0, TimeUnit.SECONDS)
                // The door pings every 30 s and closes a device silent for 90.
                .pingInterval(25, TimeUnit.SECONDS)
                .build()
        }

        /** Dial [url], answer the greeting with [hello], and hand back the admitted connection. */
        suspend fun open(
            url: String,
            welcomeTimeoutMs: Long = WELCOME_TIMEOUT_MS,
            hello: (Greeting) -> String,
        ): Pair<RelaySession, Welcome> {
            val socket = try {
                LineSocket.open(http, url)
            } catch (e: IllegalArgumentException) {
                throw RelayFailure("That address does not read as Relay's")
            }
            try {
                val greeting = try {
                    withTimeout(GREETING_TIMEOUT_MS) { Greeting.from(socket.next()) }
                } catch (e: TimeoutCancellationException) {
                    throw RelayFailure("Relay did not answer in time")
                } ?: throw RelayFailure("Something answered, but not Relay")
                if (greeting.refusal != null) {
                    throw RelayFailure(
                        if (greeting.refusal == "host.offline") "Relay's cloud copy is not running right now"
                        else "Relay refused the connection (${greeting.refusal})"
                    )
                }
                socket.send(hello(greeting))
                val welcome = try {
                    withTimeout(welcomeTimeoutMs) { Welcome.from(socket.next()) }
                } catch (e: TimeoutCancellationException) {
                    throw RelayFailure("Relay did not answer in time")
                }
                if (welcome == null || !welcome.ok) throw RelayFailure.denial(welcome?.error)
                return RelaySession(socket) to welcome
            } catch (e: Throwable) {
                socket.close()
                throw e
            }
        }
    }
}

/** A WebSocket as a queue of lines: what Relay sends arrives as [next], in order. */
internal class LineSocket private constructor() : WebSocketListener() {

    private val lines = Channel<String>(Channel.UNLIMITED)
    private lateinit var ws: WebSocket

    suspend fun next(): String = try {
        lines.receive()
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        throw RelayFailure("The connection to Relay closed")
    }

    fun send(line: String) {
        if (!ws.send(line)) throw RelayFailure("The connection to Relay closed")
    }

    fun close() {
        ws.close(1000, null)
        lines.close()
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        // One line per message, but a frame holding several is several lines.
        text.split('\n').filter { it.isNotBlank() }.forEach { lines.trySend(it) }
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
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
        fun open(http: OkHttpClient, url: String): LineSocket {
            val socket = LineSocket()
            socket.ws = http.newWebSocket(Request.Builder().url(url).build(), socket)
            return socket
        }
    }
}
