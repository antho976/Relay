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

    /** App-lifetime work that should survive any screen (the wear publisher's collectors, W1). */
    private val appScope = CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.Default)

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
        // Re-arm the daily training reminder from its persisted setting (no-op when it's off).
        CoroutineScope(Dispatchers.IO).launch {
            programRepository.ensureLoaded()
            reminderScheduler.ensureScheduled(
                settingsRepository.trainingReminderEnabled.first(),
                settingsRepository.trainingReminderHour.first()
            )
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
        val pending = File(filesDir, "pending_restore.db")
        val pendingPrefs = File(filesDir, "pending_restore_prefs.pb")
        val pendingPhotos = File(filesDir, "pending_restore_photos")
        val pendingAvatar = File(filesDir, "pending_restore_avatar.jpg")
        if (!pending.exists() && !pendingPrefs.exists() && !pendingPhotos.exists() && !pendingAvatar.exists()) return
        // Captured before the prefs swap deletes it: a restore that carries prefs but no avatar must
        // end with NO live avatar (see the avatar block below), so the restored avatarDefaultId pref
        // can't be left pointing at nothing while a previously-seeded cover lingers (a file/pref desync).
        val restoringPrefs = pendingPrefs.exists()
        // A backup always contains the DB, so a successful DB swap is the signal that "a restore landed";
        // MainActivity reads the flag below to confirm it to the user on this launch. We only confirm
        // "restored successfully" when EVERY staged component swapped — a failed sub-swap (kept for a
        // retry on the next boot) must not be reported as a clean restore.
        var applied = false
        var anyFailed = false
        if (pending.exists()) {
            // DB swap also drops stale WAL/-shm sidecars so SQLite can't replay old frames over the
            // restored file. deleteOrThrow makes a surviving sidecar a failed swap (staged file kept).
            val live = getDatabasePath("forge.db")
            if (swapStagedFile(pending, live, afterSwap = {
                    deleteOrThrow(File(live.path + "-wal"))
                    deleteOrThrow(File(live.path + "-shm"))
                })) { pending.delete(); applied = true } else anyFailed = true
        }
        if (pendingPrefs.exists()) {
            // Must match preferencesDataStore(name = "forge_settings").
            if (swapStagedFile(pendingPrefs, File(filesDir, "datastore/forge_settings.preferences_pb"))) pendingPrefs.delete()
            else anyFailed = true
        }
        if (pendingPhotos.isDirectory) {
            val swapped = runCatching {
                // Must match ProgressPhotoRepository's "progress_photos" folder. Swap via rename: move
                // the current folder aside first, slot the restored one in, then drop the old copy — and
                // if the slot-in fails, move the original back. renameTo within filesDir is atomic, so
                // there's no window where the user is left with neither folder.
                val livePhotos = File(filesDir, "progress_photos")
                val oldPhotos = File(filesDir, "progress_photos.old")
                if (oldPhotos.exists()) oldPhotos.deleteRecursively()
                val hadLive = livePhotos.exists()
                if (hadLive && !livePhotos.renameTo(oldPhotos)) error("Could not move current photos aside")
                if (!pendingPhotos.renameTo(livePhotos)) {
                    if (hadLive) oldPhotos.renameTo(livePhotos) // roll back to the originals
                    error("Could not move restored photos into place")
                }
                oldPhotos.deleteRecursively()
            }.isSuccess
            // Only discard the staged folder once it's actually in place; otherwise keep it for retry.
            if (swapped) pendingPhotos.deleteRecursively() else anyFailed = true
        }
        if (pendingAvatar.exists()) {
            // Must match AvatarRepository.FILE_NAME.
            if (swapStagedFile(pendingAvatar, File(filesDir, "avatar.jpg"))) pendingAvatar.delete()
            else anyFailed = true
        } else if (restoringPrefs && !anyFailed) {
            // The restore replaced the prefs but carried no avatar → the restored state has none. Clear
            // any live avatar so a previously-seeded default cover can't outlive the (now blank)
            // avatarDefaultId — otherwise the cover shows but the picker rings nothing. The one-time
            // seed re-runs cleanly on next Profile open. Gated on a clean prefs swap (!anyFailed).
            runCatching { File(filesDir, "avatar.jpg").delete() }
        }
        // One-shot flag so MainActivity can confirm the restore landed on this launch — the swap runs at
        // boot before any UI exists, and the staging path restarts the process silently otherwise. Only
        // written for a fully clean restore so we never claim "successfully" on a partial one.
        if (applied && !anyFailed) runCatching { File(filesDir, RESTORE_DONE_FLAG).writeText("1") }
    }

    /**
     * Swap [pending] into place at [live] via a temp-in-the-same-dir + atomic rename. A direct
     * copyTo(live, overwrite=true) truncates and streams — if it dies mid-copy (disk full, process
     * killed) the live file is left partial/corrupt with the original gone. Rename on one filesystem
     * is atomic: a failed copy leaves the intact original untouched and the next boot retries from
     * [pending]. Returns true once [live] is the restored file; [afterSwap] runs only on success.
     */
    private fun swapStagedFile(pending: File, live: File, afterSwap: () -> Unit = {}): Boolean = runCatching {
        live.parentFile?.mkdirs()
        val staged = File(live.parentFile, "${live.name}.restoring")
        if (staged.exists()) staged.delete()
        pending.copyTo(staged, overwrite = true)
        if (!staged.renameTo(live)) {
            staged.delete()
            error("Could not move ${live.name} into place")
        }
        afterSwap()
    }.isSuccess

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
    }
}
