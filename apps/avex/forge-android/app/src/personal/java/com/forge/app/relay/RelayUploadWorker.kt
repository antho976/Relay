package com.forge.app.relay

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import com.forge.app.ForgeApp
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.repo.BackupRepository
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.util.concurrent.TimeUnit

/** PERSONAL BUILD ONLY. What the uploader needs from the app's graph. */
@EntryPoint
@InstallIn(SingletonComponent::class)
interface RelayEntryPoint {
    fun database(): ForgeDatabase
    fun backupRepository(): BackupRepository
}

/**
 * PERSONAL BUILD ONLY. Writes Avex's full training-history export (the same file Settings, Export,
 * Training history writes) and sends it to Relay's cloud copy. Skipped while a workout is in
 * progress, unless asked for by hand: finishing it schedules the next send.
 */
class RelayUploadWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val store = RelayStore(applicationContext)
        if (store.paired == null) return Result.success()
        (applicationContext as ForgeApp).awaitStorageReady()
        val graph = EntryPointAccessors.fromApplication(applicationContext, RelayEntryPoint::class.java)
        val byHand = inputData.getBoolean(BY_HAND, false)
        if (!byHand && graph.database().sessionDao().getActiveSession() != null) return Result.success()
        return try {
            val file = graph.backupRepository().exportFullDataJson()
            val sessions = RelayUploader.upload(store, file.readText())
            store.sent(sessions)
            Result.success()
        } catch (e: RelayFailure) {
            store.failed(e.reason)
            if (e.unpaired || runAttemptCount >= MAX_ATTEMPTS) Result.failure() else Result.retry()
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            store.failed(e.message ?: e.javaClass.simpleName)
            if (runAttemptCount >= MAX_ATTEMPTS) Result.failure() else Result.retry()
        }
    }

    companion object {
        private const val WORK = "avex-relay-upload"
        private const val BY_HAND = "by_hand"
        private const val MAX_ATTEMPTS = 5

        /**
         * Sends once the phone has a connection. [delaySeconds] lets a burst of edits settle into
         * one send: a newer request replaces one still waiting.
         */
        fun schedule(context: Context, delaySeconds: Long = 30, byHand: Boolean = false) {
            val request = OneTimeWorkRequestBuilder<RelayUploadWorker>()
                .setInitialDelay(delaySeconds, TimeUnit.SECONDS)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 1, TimeUnit.MINUTES)
                .setInputData(workDataOf(BY_HAND to byHand))
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(WORK, ExistingWorkPolicy.REPLACE, request)
        }
    }
}

/**
 * PERSONAL BUILD ONLY. Starts watching the training history once the app is up: a provider is the
 * one hook a build type can add without touching the shared code (it is created before
 * Application.onCreate, so the work is posted to run after it). Any change to a workout, its sets,
 * cardio or coach goals schedules a send.
 */
class RelayUploadStarter : ContentProvider() {

    override fun onCreate(): Boolean {
        val context = context?.applicationContext ?: return true
        Handler(Looper.getMainLooper()).post { watch(context) }
        return true
    }

    private fun watch(context: Context) {
        val app = context as? ForgeApp ?: return
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            app.awaitStorageReady()
            val db = EntryPointAccessors.fromApplication(app, RelayEntryPoint::class.java).database()
            db.invalidationTracker
                .createFlow("session", "logged_exercise", "logged_set", "cardio_entry", "coach_goal", emitInitialState = false)
                .collect { if (RelayStore(app).paired != null) RelayUploadWorker.schedule(app) }
        }
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0
}
