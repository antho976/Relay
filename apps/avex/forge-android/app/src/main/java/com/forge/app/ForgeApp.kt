package com.forge.app

import android.app.Application
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.service.ReminderScheduler
import com.forge.app.service.WeeklyRecapWorker
import com.forge.app.service.WorkoutSessionService
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.util.Date
import javax.inject.Inject

@HiltAndroidApp
class ForgeApp : Application(), Configuration.Provider {

    @Inject lateinit var workerFactory: HiltWorkerFactory
    @Inject lateinit var programRepository: ProgramRepository
    @Inject lateinit var settingsRepository: SettingsRepository
    @Inject lateinit var reminderScheduler: ReminderScheduler
    @Inject lateinit var wearStatePublisher: com.forge.app.service.wear.WearStatePublisher
    private val startupGate = StartupGate()
    internal suspend fun awaitStorageReady() = startupGate.await()

    /** App-lifetime work that should survive any screen (the wear publisher's collectors, W1). */
    private val appScope = CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.Default)

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    override fun onCreate() {
        super.onCreate()
        installCrashLogger()
        if (isDebuggable()) installStrictMode()
        appScope.launch(Dispatchers.IO) {
            try {
                applyPendingRestore()
                startupGate.complete()
                startAppServices()
            } catch (failure: Exception) {
                startupGate.fail(failure)
                runCatching { writeCrashLog(failure) }
            }
        }
    }

    private fun startAppServices() {
        WorkoutSessionService.createChannels(this)
        WeeklyRecapWorker.schedule(this)
        // Seed-if-empty + load the DB-backed active program into the Program facade (program-unlock Phase 1).
        // Re-arm the daily training reminder from its persisted setting (no-op when it's off).
        CoroutineScope(Dispatchers.IO).launch {
            programRepository.ensureLoaded()
            // Finish any regeneration that died between its program transaction and the deload
            // marker beside it (M-06). Two stores, no shared transaction: this is what stops them
            // disagreeing forever. Fail-soft — a reconciliation that throws must not stop startup.
            runCatching { programRepository.reconcilePendingGeneration() }
            reminderScheduler.ensureScheduled(
                settingsRepository.trainingReminderEnabled.first(),
                settingsRepository.trainingReminderHour.first()
            )
        }
        // The user's custom (freestyle-created) exercises into their facade, the way the program is
        // loaded into Program above: the muscle/name resolvers are synchronous and process-wide, so
        // the registry has to be, too. Collected for the app's life so a create/save is reflected.
        appScope.launch {
            settingsRepository.customExercises.collect { com.forge.app.program.CustomExerciseRegistry.setAll(it) }
        }
        // Mirror phone state to the wrist (W1): /session/live + /timer/state + /config collectors,
        // and the app-open /glance/today refresh. All fail-soft — no watch means unread DataItems.
        wearStatePublisher.start(appScope)
    }

    /**
     * If a restore staged a database file (see BackupRepository.restoreFromUri), swap it in NOW —
     * before Room is ever opened. Doing the swap at boot avoids closing/replacing the DB while
     * flows are still reading it (the old restore path raced with active observers until exit).
     * A #14 backup also stages the DataStore preferences file; swap that in too, before DataStore
     * is first read (it's lazily created on first access, which happens after this).
     */
    private fun applyPendingRestore() {
        val liveDb = getDatabasePath("forge.db")
        // Carrying on when this returns false is safe BECAUSE of what RestoreApply guarantees: it
        // never returns having left a mixture. Either the whole set is live, or none of it is and
        // the set is queued to retry — so the boot below always runs on one coherent dataset, and
        // the user is told nothing rather than told something untrue.
        if (!RestoreApply.apply(filesDir, liveDb)) {
            check(!RestoreApply.hasUnsettledRecovery(filesDir)) { "Restore recovery needs another attempt" }
            return
        }

        // The set is live, but its pre-restore snapshots are still on disk. The restored database
        // is proven only once THIS configuration has opened it — migrations, identity check and a
        // read — so that happens here, before anything else touches Room, and the snapshots are
        // released or put back on the answer. Staging already opened the file through the same
        // builder, so a failure here means the bytes were damaged in between; it used to mean an
        // app that could not start, with the previous database already discarded.
        // A disposable validator, closed BEFORE rollback. Never initialize or close the app's
        // singleton Room instance while its underlying file is still being selected.
        val validator = com.forge.app.data.db.forgeDatabaseBuilder(this, liveDb.path).build()
        val opened = try {
            runCatching {
                validator.openHelper.writableDatabase.query("SELECT count(*) FROM session")
                    .use { it.moveToFirst() }
            }.isSuccess
        } finally { validator.close() }
        if (opened) {
            RestoreApply.confirm(filesDir, liveDb)
            // One-shot flag so MainActivity can confirm the restore landed on this launch — the swap
            // runs at boot before any UI exists, and the staging path restarts the process silently
            // otherwise. Only written for a fully clean, opened restore so we never claim
            // "successfully" on a partial one.
            runCatching { File(filesDir, RESTORE_DONE_FLAG).writeText("1") }
        } else {
            check(RestoreApply.revert(filesDir, liveDb)) { "Could not finish restore rollback" }
            runCatching { File(filesDir, RESTORE_FAILED_FLAG).writeText("1") }
        }
    }

    /**
     * Write every uncaught exception to a local file before the process dies, then hand off
     * to the platform handler so the crash still surfaces normally. Offline-friendly forensics:
     * a crash becomes "here's the stack trace" instead of a silent close. Logs live in
     * `filesDir/crashes/`; only the 10 most recent are kept.
     */
    private fun installCrashLogger() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            runCatching { writeCrashLog(throwable) }
            previous?.uncaughtException(thread, throwable)
        }
    }

    private fun writeCrashLog(t: Throwable) {
        val dir = File(filesDir, "crashes").apply { mkdirs() }
        val trace = StringWriter().also { t.printStackTrace(PrintWriter(it)) }.toString()
        File(dir, "crash_${System.currentTimeMillis()}.txt")
            .writeText("Avex crash @ ${Date()}\n\n$trace")
        // Keep only the 10 most recent crash logs.
        dir.listFiles()
            ?.sortedByDescending { it.lastModified() }
            ?.drop(10)
            ?.forEach { it.delete() }
    }

    /** Surfaces accidental main-thread disk/network and leaked DB cursors/closeables in debug. */
    private fun installStrictMode() {
        StrictMode.setThreadPolicy(
            StrictMode.ThreadPolicy.Builder()
                .detectDiskReads()
                .detectDiskWrites()
                .detectNetwork()
                .penaltyLog()
                .build()
        )
        StrictMode.setVmPolicy(
            StrictMode.VmPolicy.Builder()
                .detectLeakedClosableObjects()
                .detectLeakedSqlLiteObjects()
                .penaltyLog()
                .build()
        )
    }

    private fun isDebuggable(): Boolean =
        (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

    companion object {
        /** One-shot marker written after a boot-time restore swap; read + cleared by MainActivity. */
        const val RESTORE_DONE_FLAG = "restore_just_completed"
        /** One-shot marker written when a landed restore could not be opened and was put back. */
        const val RESTORE_FAILED_FLAG = "restore_put_back"
    }
}
