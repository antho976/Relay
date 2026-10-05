package com.tally.app

import android.app.Application
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.tally.app.data.repo.LedgerRepository
import com.tally.app.data.repo.RecurringPoster
import com.tally.app.di.AppScope
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.work.BackupWorker
import com.tally.app.work.RecurringWorker
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltAndroidApp
class TallyApp : Application(), Configuration.Provider {

    @Inject lateinit var workerFactory: HiltWorkerFactory
    // Lazy: injection runs on the main thread before the first frame; these are only needed on IO.
    @Inject lateinit var ledger: dagger.Lazy<LedgerRepository>
    @Inject lateinit var poster: dagger.Lazy<RecurringPoster>
    @Inject lateinit var settings: dagger.Lazy<SettingsRepository>
    @Inject @AppScope lateinit var appScope: CoroutineScope

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    override fun onCreate() {
        super.onCreate()
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) installStrictMode()
        appScope.launch(Dispatchers.IO) {
            runCatching { ledger.get().seedCategoriesIfEmpty() }
            runCatching { poster.get().postDue() }
            // The weekly backup follows its switch in Settings, which is on unless turned off.
            runCatching { BackupWorker.sync(this@TallyApp, settings.get().currentBackup().auto) }
        }
        RecurringWorker.schedule(this)
    }

    /** Debug only: disk or network on the main thread, leaked closeables and cursors all log loudly. */
    private fun installStrictMode() {
        StrictMode.setThreadPolicy(
            StrictMode.ThreadPolicy.Builder().detectDiskReads().detectDiskWrites().detectNetwork().penaltyLog().build()
        )
        StrictMode.setVmPolicy(
            StrictMode.VmPolicy.Builder()
                .detectLeakedClosableObjects()
                .detectLeakedSqlLiteObjects()
                .detectActivityLeaks()
                .penaltyLog()
                .build()
        )
    }
}
