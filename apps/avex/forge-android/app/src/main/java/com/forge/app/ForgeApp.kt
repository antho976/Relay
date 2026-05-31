package com.forge.app

import android.app.Application
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.forge.app.service.WeeklyRecapWorker
import com.forge.app.service.WorkoutSessionService
import dagger.hilt.android.HiltAndroidApp
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.util.Date
import javax.inject.Inject

@HiltAndroidApp
class ForgeApp : Application(), Configuration.Provider {

    @Inject lateinit var workerFactory: HiltWorkerFactory

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    override fun onCreate() {
        super.onCreate()
        installCrashLogger()
        if (isDebuggable()) installStrictMode()
        WorkoutSessionService.createChannels(this)
        WeeklyRecapWorker.schedule(this)
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
