package com.quietsoftware.relay.service

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.data.Relay
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import java.util.concurrent.TimeUnit

/**
 * The background catch-up when the link is not kept open: every half hour on a network, reach
 * the PC, send what the outbox holds, read what changed. The [Notifier] announces whatever the
 * read brought in that needs the person. With the PC out of reach, the server's copy is asked
 * for the ledger's and the training history's readings instead (docs/CLOUD.md).
 */
@HiltWorker
class SyncWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val relay: Relay,
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        if (!relay.isPaired()) return Result.success()
        relay.start()
        relay.kick()
        val online = withTimeoutOrNull(45_000) { relay.state.first { it is LinkState.Online || it is LinkState.Revoked } }
        if (online !is LinkState.Online) {
            // The PC is away: the server's copy, if one is paired, brings Tally's and Avex's readings up to date.
            relay.hub.catchUpFromCloud()
            return Result.success()
        }
        // The full read runs as the session opens; give it and the outbox time to finish.
        withTimeoutOrNull(60_000) {
            relay.hub.syncer.syncedAt.first { it > 0 }
            relay.hub.flush()
        }
        delay(2_000)
        return Result.success()
    }

    companion object {
        private const val NAME = "relay-sync"

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<SyncWorker>(30, TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }
    }
}
