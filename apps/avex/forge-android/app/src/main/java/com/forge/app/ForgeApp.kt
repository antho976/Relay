package com.forge.app

import android.app.Application
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.service.WeeklyRecapWorker
import com.forge.app.service.WorkoutSessionService
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
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

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    override fun onCreate() {
        super.onCreate()
        applyPendingRestore()
        installCrashLogger()
        if (isDebuggable()) installStrictMode()
        WorkoutSessionService.createChannels(this)
        WeeklyRecapWorker.schedule(this)
        // Seed-if-empty + load the DB-backed active program into the Program facade (program-unlock Phase 1).
        CoroutineScope(Dispatchers.IO).launch { programRepository.ensureLoaded() }
    }

    /**
     * If a restore staged a database file (see BackupRepository.restoreFromUri), swap it in NOW —
     * before Room is ever opened. Doing the swap at boot avoids closing/replacing the DB while
     * flows are still reading it (the old restore path raced with active observers until exit).
     * A #14 backup also stages the DataStore preferences file; swap that in too, before DataStore
     * is first read (it's lazily created on first access, which happens after this).
     */
    private fun applyPendingRestore() {
        val pending = File(filesDir, "pending_restore.db")
        val pendingPrefs = File(filesDir, "pending_restore_prefs.pb")
        if (!pending.exists() && !pendingPrefs.exists()) return
        if (pending.exists()) {
            val swapped = runCatching {
                val live = getDatabasePath("forge.db")
                live.parentFile?.mkdirs()
                pending.copyTo(live, overwrite = true)
                // The restored file is authoritative; drop stale WAL/-shm so they can't override it.
                // delete() returns false instead of throwing, so a sidecar that survives the delete
                // would let SQLite replay stale frames over the restored DB — silent corruption.
                // Treat a surviving sidecar as a failed swap so the staged file is kept and retried.
                deleteOrThrow(File(live.path + "-wal"))
                deleteOrThrow(File(live.path + "-shm"))
            }.isSuccess
            // Only discard the staged file once it's actually in place. If the swap failed (e.g.
            // disk full), keep it so the next boot retries rather than silently losing the backup.
            if (swapped) pending.delete()
        }
        if (pendingPrefs.exists()) {
            val swapped = runCatching {
                // Must match preferencesDataStore(name = "forge_settings").
                val livePrefs = File(filesDir, "datastore/forge_settings.preferences_pb")
                livePrefs.parentFile?.mkdirs()
                pendingPrefs.copyTo(livePrefs, overwrite = true)
            }.isSuccess
            if (swapped) pendingPrefs.delete()
        }
    }

    /** Delete [f]; throw if it survives so the enclosing runCatching treats the swap as failed. */
    private fun deleteOrThrow(f: File) {
        if (f.exists() && !f.delete() && f.exists()) error("Could not delete ${f.name}")
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
            .writeText("Forge crash @ ${Date()}\n\n$trace")
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
}
