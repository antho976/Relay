package com.tally.app.work

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.tally.app.data.sync.RelaySync
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit

/**
 * Syncs with the owner's server every fifteen minutes (the least WorkManager allows) while there is
 * a network, so the phone, the server and the PC meet even on days the app is not opened. A failed
 * run waits for the next one: the reason is the status line, never a retry storm or a notification.
 */
@HiltWorker
class SyncWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val sync: RelaySync,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        runCatching { sync.syncNow() }
        return Result.success()
    }

    companion object {
        private const val NAME = "cloud-sync"

        /** Schedules the periodic run while connected, cancels it when not. Safe on every launch. */
        fun sync(context: Context, paired: Boolean) {
            val work = WorkManager.getInstance(context)
            if (paired) {
                val request = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                    .build()
                work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
            } else {
                work.cancelUniqueWork(NAME)
            }
        }
    }
}
