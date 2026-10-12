package com.tally.app.data.sync

import androidx.compose.runtime.Immutable
import com.tally.app.BuildConfig
import com.tally.app.di.AppScope
import com.tally.link.ClaimReply
import com.tally.link.CloudApi
import com.tally.link.CloudFailure
import com.tally.link.Enrollment
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/** Where connecting this phone to the owner's server stands, for the status bar and Settings. */
@Immutable
sealed interface Connect {
    /** The person keeps this phone's ledger on this phone only. */
    data object PhoneOnly : Connect
    /** Not connected, and not asking right now (no network yet, or before the first try). */
    data object Idle : Connect
    data object Reaching : Connect
    /** Asked; waiting for the owner's Allow on Discord (or on the dashboard when not [notified]). */
    data class Waiting(val code: String, val notified: Boolean, val expiresAt: Long, val server: String) : Connect
    data object Pairing : Connect
    data object Connected : Connect
    data object Denied : Connect
    /** The request ran out before anyone answered. */
    data object Expired : Connect
    data class Trouble(val reason: String, val cloudOff: Boolean = false) : Connect
}

/**
 * Connects this phone to Relay's cloud copy on the owner's server with nothing to set up: it asks
 * the server (antho-s-bot's /api/tally), the owner taps Allow on the Discord DM that sends, and the
 * phone pairs and runs its first sync, which merges. Starts by itself at launch and whenever the
 * app comes back while not connected; a request in flight survives the app being closed.
 */
@Singleton
class CloudConnect @Inject constructor(
    private val store: SyncStore,
    private val api: CloudApi,
    private val sync: RelaySync,
    @AppScope private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow<Connect>(Connect.Idle)
    val state: StateFlow<Connect> = _state.asStateFlow()

    private var job: Job? = null

    /** The server this phone talks to: the person's address, or the one this build was made for. */
    fun serverOf(prefs: SyncPrefs): String = CloudApi.normalize(prefs.server ?: BuildConfig.TALLY_SERVER)

    /**
     * Connects unless connected, asked not to, or already on it. A refusal the person has to see
     * (Denied, a server without Relay) stays until they ask again with [retry].
     */
    fun ensure() {
        if (job?.isActive == true || _state.value.isFinal()) return
        job = scope.launch { connect() }
    }

    /** The person asked again: start over, forgetting a request that was refused or ran out. */
    fun retry() {
        job?.cancel()
        _state.value = Connect.Idle
        job = scope.launch {
            store.clearPending()
            connect()
        }
    }

    suspend fun setPhoneOnly(on: Boolean) {
        store.setPhoneOnly(on)
        if (on) {
            job?.cancel()
            store.clearPending()
            _state.value = Connect.PhoneOnly
        } else {
            _state.value = Connect.Idle
            ensure()
        }
    }

    /** A new address: the old request and pairing belong to the old server. */
    suspend fun setServer(url: String?) {
        job?.cancel()
        sync.forget()
        store.setServer(url)
        store.clearPending()
        _state.value = Connect.Idle
        ensure()
    }

    /** The server forgot this phone (revoked, or its data lost): ask again from the start. */
    suspend fun reconnect() {
        job?.cancel()
        sync.forget()
        store.clearPending()
        _state.value = Connect.Idle
        ensure()
    }

    private fun Connect.isFinal() = this == Connect.Denied || this == Connect.Expired || (this is Connect.Trouble && cloudOff)

    private suspend fun connect() {
        try {
            val prefs = store.current()
            when {
                prefs.phoneOnly -> { _state.value = Connect.PhoneOnly; return }
                prefs.paired && !prefs.unpaired -> { _state.value = Connect.Connected; return }
                prefs.paired -> sync.forget()
            }
            val server = serverOf(prefs)
            val pending = prefs.pending?.takeIf { it.server == server && it.expiresAt > System.currentTimeMillis() }
            val enrollment = pending ?: ask(server) ?: return
            await(enrollment)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            _state.value = Connect.Trouble("Connecting did not finish. It will try again.")
        }
    }

    /** Asks the server to let this phone in; null when it could not (the state says why). */
    private suspend fun ask(server: String): Enrollment? {
        _state.value = Connect.Reaching
        return try {
            if (!api.hello(server).cloud) {
                _state.value = Connect.Trouble("Relay is not running on your server yet", cloudOff = true)
                return null
            }
            api.enroll(server, sync.deviceName).also { store.savePending(it) }
        } catch (e: CloudFailure) {
            _state.value = Connect.Trouble(e.reason, cloudOff = e.cloudOff)
            null
        }
    }

    /**
     * Polls the request until the owner answers: every two seconds for the first minute, while
     * they are likely looking at Discord, then every six. Once allowed, pairs and syncs; a pairing
     * that fails collects a fresh link (the server mints one per claim) up to three times.
     */
    private suspend fun await(e: Enrollment) {
        val started = System.currentTimeMillis()
        var failures = 0
        var pairings = 0
        while (System.currentTimeMillis() < e.expiresAt) {
            _state.value = Connect.Waiting(e.code, e.notified, e.expiresAt, e.server)
            val reply = try {
                api.claim(e)
            } catch (f: CloudFailure) {
                ClaimReply.Trouble(f.reason)
            }
            when (reply) {
                ClaimReply.Waiting -> failures = 0
                is ClaimReply.Ready -> {
                    _state.value = Connect.Pairing
                    when (val outcome = sync.pair(reply.link)) {
                        is SyncOutcome.Done -> {
                            store.clearPending()
                            _state.value = Connect.Connected
                            return
                        }
                        is SyncOutcome.Failed -> if (++pairings >= 3) {
                            store.clearPending()
                            _state.value = Connect.Trouble(outcome.reason)
                            return
                        }
                        SyncOutcome.NotPaired -> Unit
                    }
                }
                ClaimReply.Denied -> {
                    store.clearPending()
                    _state.value = Connect.Denied
                    return
                }
                ClaimReply.Expired, ClaimReply.Spent -> {
                    store.clearPending()
                    _state.value = Connect.Expired
                    return
                }
                is ClaimReply.Trouble -> if (++failures >= 10) {
                    _state.value = Connect.Trouble(reply.reason)
                    return
                }
            }
            delay(if (System.currentTimeMillis() - started < 60_000) 2_000 else 6_000)
        }
        store.clearPending()
        _state.value = Connect.Expired
    }
}
