package com.tally.app.work

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.tally.app.data.backup.BackupStore
import com.tally.app.data.repo.DataResult
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit

/** Writes the weekly automatic backup, so a lost phone costs at most a week. */
@HiltWorker
class BackupWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val store: BackupStore,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = when (runCatching { store.backupNow() }.getOrNull()) {
        // A folder that went away is not worth retrying: the copy on the phone was written.
        is DataResult.Done, is DataResult.Failed -> Result.success()
        null -> Result.retry()
    }

    companion object {
        private const val NAME = "weekly-backup"

        /** Schedules the weekly run when [on], cancels it when not. Safe to call on every launch. */
        fun sync(context: Context, on: Boolean) {
            val work = WorkManager.getInstance(context)
            if (on) {
                val request = PeriodicWorkRequestBuilder<BackupWorker>(7, TimeUnit.DAYS).build()
                work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
            } else {
                work.cancelUniqueWork(NAME)
            }
        }
    }
}
