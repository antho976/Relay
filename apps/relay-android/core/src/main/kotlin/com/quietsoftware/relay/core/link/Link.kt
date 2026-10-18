package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.orderRoutes
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.onTimeout
import kotlinx.coroutines.selects.select
import kotlinx.serialization.json.JsonObject

/** Where the link to the PC stands, for the status bar and the home screen. */
sealed interface LinkState {
    /** No PC is paired. */
    data object Unpaired : LinkState

    /** The person pressed Disconnect; nothing dials until they connect again. */
    data object Stopped : LinkState

    data class Connecting(val attempt: Int) : LinkState

    /** Admitted, waiting on the PC's engine (the door is starting it). */
    data class Starting(val route: Route, val greeting: Greeting) : LinkState

    data class Online(val route: Route, val greeting: Greeting) : LinkState

    /** Unreachable for now; dials again at [retryAt] (epoch ms) or on [Link.kick]. */
    data class Offline(val reason: String, val retryAt: Long) : LinkState

    /** The PC refused this phone's credential: pair again. */
    data class Revoked(val reason: String) : LinkState
}

/**
 * Keeps one admitted [BusSession] to the paired PC while the app wants one: dials every route
 * (direct first), proves the token, subscribes, keeps the session alive with `bus.ping`, and
 * redials with backoff when it drops. Everything that talks to the PC goes through [session].
 * [ops] is the kind of door it must reach: a PC's, or the server's copy for the second link
 * ([com.quietsoftware.relay.core.Hub.cloud]).
 */
class Link(
    private val scope: CoroutineScope,
    private val dialer: Dialer,
    private val profiles: ProfileSource,
    private val now: () -> Long = System::currentTimeMillis,
    private val ops: String = Wire.OPS_PC,
) {
    /** Where the paired PC's profile lives. The app persists it; tests keep it in memory. */
    interface ProfileSource {
        suspend fun current(): PcProfile?

        /** Record what changed after a connection: the route that worked, the PC's name, wake targets. */
        suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting)
    }

    /** Called with each new session, before anyone else sees it: subscribe, resync, flush. */
    var onSession: suspend (BusSession) -> Unit = {}

    private val _state = MutableStateFlow<LinkState>(LinkState.Connecting(0))
    val state: StateFlow<LinkState> = _state.asStateFlow()

    private val _session = MutableStateFlow<BusSession?>(null)
    val session: StateFlow<BusSession?> = _session.asStateFlow()

    private val wake = Channel<Unit>(Channel.CONFLATED)
    private var loop: Job? = null
    @Volatile private var wanted = true

    fun start() {
        wanted = true
        if (loop?.isActive == true) {
            wake.trySend(Unit)
            return
        }
        loop = scope.launch { run() }
    }

    /** Dial now instead of waiting out the backoff: the app came to the foreground, the network changed. */
    fun kick() {
        if (!wanted) return
        start()
    }

    /** Disconnect and stay disconnected until [start]. */
    fun stop() {
        wanted = false
        loop?.cancel()
        loop = null
        _session.value?.close()
        _session.value = null
        _state.value = LinkState.Stopped
    }

    /** Drop the current session and dial again, e.g. after the profile's routes changed. */
    fun restart() {
        _session.value?.close()
        start()
    }

    /** The live session, or a `link.down` error the caller can queue around. */
    fun require(): BusSession = _session.value?.takeIf { it.isOpen } ?: throw BusException(com.quietsoftware.relay.core.wire.BusError.link("link.down", "Not connected to the PC"))

    private suspend fun run() {
        var backoff = RECONNECT_MIN_MS
        var attempt = 0
        while (wanted) {
            val profile = profiles.current()
            if (profile == null) {
                _state.value = LinkState.Unpaired
                wake.receive()
                continue
            }
            attempt++
            _state.value = LinkState.Connecting(attempt)
            val routes = orderRoutes(profile.routes, profile.policy, profile.lastRoute)
            val greeted = Handshake.greet(dialer, routes).getOrElse { e ->
                val refusal = (e as? RefusalException)?.refusal
                backoff = waitOut(refusal?.text ?: "The PC did not answer", backoff)
                continue
            }
            val welcome = Handshake.prove(greeted, profile.device, profile.token, ops).getOrElse { e ->
                val refusal = (e as? RefusalException)?.refusal
                // Neither a forgotten credential nor the wrong kind of door gets better by dialing again.
                if (refusal is Refusal.Unpaired || refusal is Refusal.WrongDoor) {
                    _state.value = LinkState.Revoked(refusal.text)
                    wanted = false
                    return
                }
                backoff = waitOut(refusal?.text ?: "The PC did not let this phone in", backoff)
                continue
            }
            check(welcome.ok)
            val greeting = greeted.greeting.admittedBy(welcome)
            val session = BusSession(greeted.socket, greeted.route, greeting, profile.device, scope)
            profiles.connected(profile, greeted.route, greeting)
            val starting = greeting.engine == "stopped" || greeting.engine == "starting"
            if (starting) _state.value = LinkState.Starting(greeted.route, greeting)
            // The first request waits for the engine when the door had to start it.
            val ready = runCatching {
                session.result("bus.ping", JsonObject(emptyMap()), timeoutMs = if (starting) ENGINE_START_TIMEOUT_MS else BusSession.DEFAULT_TIMEOUT_MS)
            }
            if (ready.isFailure) {
                session.close()
                backoff = waitOut("The PC's engine did not answer", backoff)
                continue
            }
            // The session is usable before anyone is told the link is up.
            _session.value = session
            _state.value = LinkState.Online(greeted.route, greeting)
            val began = now()
            runCatching { onSession(session) }
            keepAlive(session)
            _session.value = null
            if (!wanted) break
            // A session that ends as soon as it starts is not a reason to dial in a tight loop.
            if (now() - began < STABLE_MS) {
                backoff = waitOut("Connection lost, reconnecting", backoff)
            } else {
                backoff = RECONNECT_MIN_MS
                attempt = 0
                _state.value = LinkState.Offline("Connection lost, reconnecting", now())
            }
        }
    }

    /** Ping until the session ends; two missed pings in a row mean the path is dead. */
    private suspend fun keepAlive(session: BusSession) {
        var missed = 0
        while (session.isOpen && wanted) {
            val ended = select {
                session.ended.onAwait { true }
                wake.onReceive { false }
                onTimeout(KEEPALIVE_MS) { false }
            }
            if (ended) break
            val ok = runCatching { session.result("bus.ping", timeoutMs = PING_TIMEOUT_MS) }.isSuccess
            missed = if (ok) 0 else missed + 1
            if (missed >= 2) {
                session.close()
                break
            }
        }
    }

    private suspend fun waitOut(reason: String, backoff: Long): Long {
        _state.value = LinkState.Offline(reason, now() + backoff)
        select {
            wake.onReceive { }
            onTimeout(backoff) { }
        }
        return (backoff * 2).coerceAtMost(RECONNECT_MAX_MS)
    }

    companion object {
        const val RECONNECT_MIN_MS = 1_000L
        const val RECONNECT_MAX_MS = 30_000L
        const val KEEPALIVE_MS = 25_000L
        const val PING_TIMEOUT_MS = 10_000L
        const val ENGINE_START_TIMEOUT_MS = 45_000L
        const val STABLE_MS = 5_000L
    }
}
