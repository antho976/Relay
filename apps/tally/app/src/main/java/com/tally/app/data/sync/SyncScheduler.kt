package com.tally.app.data.sync

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import com.tally.app.data.db.SyncSchema
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.di.AppScope
import com.tally.app.work.SyncWorker
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * When the phone syncs with the owner's server, so that nobody ever has to: at launch, when the app
 * comes to the front, a few seconds after a change here (a burst of edits is one sync), the moment
 * a network comes back, and every fifteen minutes in the background (SyncWorker). Wherever the phone
 * is: the server is on the internet, so home Wi-Fi or not makes no difference.
 *
 * Until the phone is connected, the same moments start [CloudConnect] instead.
 */
@Singleton
class SyncScheduler @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: TallyDatabase,
    private val settings: SettingsRepository,
    private val store: SyncStore,
    private val sync: RelaySync,
    private val connect: CloudConnect,
    @AppScope private val scope: CoroutineScope,
) {
    private var started = false
    private var pending: Job? = null
    @Volatile private var dirty = false

    /** Once, from the Application. */
    @OptIn(FlowPreview::class)
    fun start() {
        if (started) return
        started = true
        scope.launch {
            // The periodic run follows the pairing: on while connected, gone once forgotten.
            store.prefs.map { it.paired }.distinctUntilChanged().collect { SyncWorker.sync(context, it) }
        }
        scope.launch {
            val ledger = db.invalidationTracker.createFlow(*SyncSchema.TABLES.toTypedArray(), emitInitialState = false).map { }
            val format = settings.settings.map { Triple(it.currency, it.monthStartDay, it.weekStartsMonday) }.distinctUntilChanged().drop(1).map { }
            merge(ledger, format)
                // While a sync runs, and just after, a write may be the sync's own or an edit
                // made meanwhile; it cannot be told which, so it is noted and the sync runs once
                // more when it ends (below). An edit is never left waiting for the next trigger.
                .filter {
                    val quiet = !sync.busy.value && System.currentTimeMillis() - sync.lastFinishedAt > SETTLE_MS
                    if (!quiet) dirty = true
                    quiet
                }
                .debounce(DEBOUNCE_MS)
                .collect { syncIfPaired() }
        }
        scope.launch {
            sync.busy.collectLatest { busy ->
                if (busy) return@collectLatest
                delay(SETTLE_MS + 100)
                if (dirty && !sync.busy.value) {
                    dirty = false
                    later(DEBOUNCE_MS)
                }
            }
        }
        watchNetwork()
        connect.ensure()
        later(0L)
    }

    /** The app came to the front. A sync that ran moments ago is not repeated. */
    fun onForeground() {
        scope.launch {
            val prefs = store.current()
            if (!prefs.paired || prefs.unpaired) {
                connect.ensure()
                return@launch
            }
            if (System.currentTimeMillis() - prefs.lastAttemptAt < FOREGROUND_GAP_MS) return@launch
            later(0L)
        }
    }

    /** The person pulled to refresh or tapped the status line. */
    fun syncNow() {
        connect.ensure()
        later(0L)
    }

    /** A network came back: whatever waited for it goes now. */
    private fun watchNetwork() {
        val cm = context.getSystemService(ConnectivityManager::class.java) ?: return
        val request = NetworkRequest.Builder().addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET).build()
        runCatching {
            cm.registerNetworkCallback(request, object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    scope.launch {
                        val prefs = store.current()
                        if (!prefs.paired || prefs.unpaired) connect.ensure() else if (prefs.lastError != null) later(NETWORK_SETTLE_MS)
                    }
                }
            })
        }
    }

    private suspend fun syncIfPaired() {
        val prefs = store.current()
        if (!prefs.paired) return
        if (prefs.unpaired) {
            connect.reconnect()
            return
        }
        val outcome = sync.syncNow()
        if (outcome is SyncOutcome.Failed && store.current().unpaired) connect.reconnect()
    }

    private fun later(delayMs: Long) {
        pending?.cancel()
        pending = scope.launch(Dispatchers.IO) {
            delay(delayMs)
            syncIfPaired()
        }
    }

    companion object {
        /** A burst of edits settles into one sync. */
        const val DEBOUNCE_MS = 3_000L
        /** Invalidations this soon after a sync are its own writes arriving late. */
        const val SETTLE_MS = 1_500L
        /** Coming back to the app twice in half a minute syncs once. */
        const val FOREGROUND_GAP_MS = 30_000L
        /** A network that just came up often needs a moment before it carries anything. */
        const val NETWORK_SETTLE_MS = 2_000L
    }
}
