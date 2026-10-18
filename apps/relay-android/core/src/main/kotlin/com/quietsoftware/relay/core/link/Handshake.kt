package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.Welcome
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import java.util.concurrent.atomic.AtomicInteger

/**
 * A PC this phone paired with, and everything needed to reach it again. The server's copy
 * (docs/CLOUD.md) is kept in the same shape: it is a door too, with fewer ops behind it.
 */
data class PcProfile(
    val hostId: String,
    val name: String,
    val instance: String,
    val device: String,
    val token: String,
    val routes: List<Route>,
    val policy: com.quietsoftware.relay.core.wire.RoutePolicy = com.quietsoftware.relay.core.wire.RoutePolicy.Auto,
    val lastRoute: String? = null,
    val wake: List<com.quietsoftware.relay.core.wire.WakeTarget> = emptyList(),
    val pairedAt: Long = 0,
    val version: String = "",
)

/** Why a connection attempt ended without a session. */
sealed class Refusal(val code: String, val text: String) {
    /** The PC no longer knows this phone: pair again. */
    class Unpaired(code: String) : Refusal(code, "This PC no longer knows this phone. Pair it again.")

    /** Every route was tried and none answered. */
    class Unreachable(val tried: List<String>) : Refusal("link.unreachable", "The PC did not answer")

    /** Anything else the door said no to. */
    class Denied(code: String, text: String) : Refusal(code, text)

    /**
     * The door let this phone in, but it is not what was asked for: a server's copy where a PC
     * was meant, or a PC where the server's copy was. Its welcome said which ([Wire.OPS_PC],
     * [Wire.OPS_CLOUD]); a door too old to say is taken at its word.
     */
    class WrongDoor(expected: String) : Refusal(
        "link.wrong_door",
        if (expected == Wire.OPS_CLOUD) "That is a PC, not your server's copy. Use a link that relay remote pair --via-only printed on the server."
        else "That is your server's copy, not a PC. Add it on the PC's page, under Your server's copy.",
    ) {
        companion object {
            /** Whether a welcome that let this phone in came from the kind of door [expected]. */
            fun matches(expected: String, welcome: Welcome): Boolean = welcome.ops == null || welcome.ops == expected
        }
    }

    companion object {
        fun of(error: String?): Refusal = when (error) {
            "auth.unknown_device", "auth.bad_proof" -> Unpaired(error)
            "pair.invalid" -> Denied(error, "That pairing code is not valid any more. Run `relay remote pair` on the PC for a new one.")
            "pair.declined" -> Denied(error, "The PC declined this phone.")
            "pair.unconfirmed" -> Denied(error, "Nobody approved this phone on the PC in time. Run `relay remote pair` again.")
            "host.offline" -> Denied(error, "Your server answered, but the PC is not connected to it.")
            else -> Denied(error ?: "internal", "The PC refused the connection (${error ?: "no reason given"}).")
        }
    }
}

object Handshake {
    const val GREETING_TIMEOUT_MS = 10_000L
    const val WELCOME_TIMEOUT_MS = 20_000L
    const val PAIR_WELCOME_TIMEOUT_MS = 90_000L
    const val DIAL_TIMEOUT_MS = 8_000L

    /** How long to give one route before also trying the next ("happy eyeballs"). */
    const val STAGGER_MS = 400L

    data class Greeted(val socket: LineSocket, val route: Route, val greeting: Greeting)

    /**
     * Dial [routes] in order, each starting [STAGGER_MS] after the one before unless it already
     * failed, and keep the first that greets. The others are closed unanswered: the door drops a
     * connection that never says hello.
     */
    suspend fun greet(dialer: Dialer, routes: List<Route>): Result<Greeted> = coroutineScope {
        if (routes.isEmpty()) return@coroutineScope Result.failure(RefusalException(Refusal.Unreachable(emptyList())))
        val winner = CompletableDeferred<Greeted?>()
        val failed = Channel<Unit>(Channel.UNLIMITED)
        val failures = AtomicInteger(0)
        var refused: Refusal? = null
        val attempts = mutableListOf<Job>()
        val starter = launch {
            for (route in routes) {
                if (winner.isCompleted) break
                attempts += launch {
                    try {
                        val socket = dialer.dial(route.url, DIAL_TIMEOUT_MS)
                        val first = withTimeoutOrNull(GREETING_TIMEOUT_MS) { socket.incoming.receiveCatching().getOrNull() }
                        val greeting = first?.let { Wire.greeting(it) }
                        if (greeting == null) {
                            first?.let { Wire.offline(it) }?.let { refused = Refusal.of(it) }
                            socket.close()
                            throw IllegalStateException("no greeting from ${route.url}")
                        }
                        if (!winner.complete(Greeted(socket, route, greeting))) socket.close()
                    } catch (e: CancellationException) {
                        throw e
                    } catch (_: Throwable) {
                        if (failures.incrementAndGet() == routes.size) winner.complete(null)
                        failed.trySend(Unit)
                    }
                }
                // The next route starts once this one failed, or after a short head start.
                withTimeoutOrNull(STAGGER_MS) { failed.receive() }
            }
        }
        val got = winner.await()
        starter.cancel()
        attempts.forEach { it.cancel() }
        if (got != null) Result.success(got)
        else Result.failure(RefusalException(refused ?: Refusal.Unreachable(routes.map { it.url })))
    }

    /** Prove this phone holds its token, on a socket that greeted; the door must be the kind [ops] names. */
    suspend fun prove(greeted: Greeted, device: String, token: String, ops: String = Wire.OPS_PC): Result<Welcome> {
        val socket = greeted.socket
        if (!socket.send(Wire.proofHello(device, greeted.greeting.challenge, token))) {
            socket.close()
            return Result.failure(RefusalException(Refusal.Unreachable(listOf(greeted.route.url))))
        }
        val line = withTimeoutOrNull(WELCOME_TIMEOUT_MS) { socket.incoming.receiveCatching().getOrNull() }
        val welcome = line?.let { Wire.welcome(it) }
        if (welcome == null) {
            socket.close()
            return Result.failure(RefusalException(Refusal.Unreachable(listOf(greeted.route.url))))
        }
        if (!welcome.ok) {
            socket.close()
            return Result.failure(RefusalException(Refusal.of(welcome.error)))
        }
        if (!Refusal.WrongDoor.matches(ops, welcome)) {
            socket.close()
            return Result.failure(RefusalException(Refusal.WrongDoor(ops)))
        }
        return Result.success(welcome)
    }

    /**
     * Present a pairing code. The code is spent the moment it is sent, so once one route has
     * greeted no other is tried. The PC may ask a person to approve this phone first, which can
     * take up to 75 seconds. A door of another kind than [ops] is refused once it has answered:
     * the code is spent by then, and the credential it gave is dropped.
     */
    suspend fun pair(dialer: Dialer, link: PairLink, deviceName: String, ops: String = Wire.OPS_PC): Result<PcProfile> {
        val greeted = greet(dialer, link.routes).getOrElse { return Result.failure(it) }
        val socket = greeted.socket
        try {
            if (!socket.send(Wire.pairHello(link.code, deviceName))) {
                return Result.failure(RefusalException(Refusal.Unreachable(listOf(greeted.route.url))))
            }
            val line = withTimeoutOrNull(PAIR_WELCOME_TIMEOUT_MS) { socket.incoming.receiveCatching().getOrNull() }
            val welcome = line?.let { Wire.welcome(it) }
                ?: return Result.failure(RefusalException(Refusal.of("pair.unconfirmed")))
            if (!welcome.ok) return Result.failure(RefusalException(Refusal.of(welcome.error)))
            if (!Refusal.WrongDoor.matches(ops, welcome)) return Result.failure(RefusalException(Refusal.WrongDoor(ops)))
            val device = welcome.device ?: return Result.failure(RefusalException(Refusal.of("hello.shape")))
            val token = welcome.token ?: return Result.failure(RefusalException(Refusal.of("hello.shape")))
            val g = greeted.greeting.admittedBy(welcome)
            return Result.success(
                PcProfile(
                    hostId = g.hostId.ifEmpty { link.hostId },
                    name = g.host.ifEmpty { link.host },
                    instance = g.instance.ifEmpty { link.instance },
                    device = device,
                    token = token,
                    routes = link.routes,
                    lastRoute = greeted.route.url,
                    wake = g.wake,
                    pairedAt = System.currentTimeMillis(),
                    version = g.version,
                ),
            )
        } finally {
            socket.close()
        }
    }
}

class RefusalException(val refusal: Refusal) : Exception(refusal.text)
