package com.forge.app.service

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import android.net.Uri
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BackupRepository
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first
import java.util.concurrent.TimeUnit

/** Weekly auto-backup to app-private storage (#86). Silently overwrites the auto_backup slot; also
 *  mirrors into a user-picked folder when one is set (GYMAP-67). Self-gates on the enable pref, so the
 *  schedule stays a simple idempotent KEEP and the toggle needs no cancel/reschedule dance. */
@HiltWorker
class AutoBackupWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val backupRepo: BackupRepository,
    private val settingsRepo: SettingsRepository
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = try {
        // User turned auto-backup off: the worker still wakes weekly but does nothing.
        if (!settingsRepo.autoBackupEnabled.first()) {
            Result.success()
        } else {
            val folder = settingsRepo.backupFolderUri.first()?.let { Uri.parse(it) }
            backupRepo.autoBackup(folder)
            Result.success()
        }
    } catch (e: Exception) {
        // Bound the retries: a permanent failure (corrupt DB, no free storage) shouldn't
        // keep retrying with backoff forever. Give up after a few attempts — and on giving up,
        // record it so Settings can warn the user instead of silently losing their backup.
        if (runAttemptCount >= MAX_ATTEMPTS) {
            backupRepo.recordAutoBackupFailure()
            Result.failure()
        } else Result.retry()
    }

    companion object {
        private const val TAG = "forge_auto_backup"
        private const val MAX_ATTEMPTS = 3

        fun schedule(context: Context) {
            val constraints = Constraints.Builder()
                .setRequiresBatteryNotLow(true)
                .build()
            val request = PeriodicWorkRequestBuilder<AutoBackupWorker>(7, TimeUnit.DAYS)
                .addTag(TAG)
                .setConstraints(constraints)
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.MINUTES)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                TAG,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
        }
    }
}
