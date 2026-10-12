package com.tally.app

import android.app.Application
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.tally.app.data.repo.RecurringPoster
import com.tally.app.di.AppScope
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.sync.SyncScheduler
import com.tally.app.ui.nav.Launches
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
    @Inject lateinit var poster: dagger.Lazy<RecurringPoster>
    @Inject lateinit var settings: dagger.Lazy<SettingsRepository>
    @Inject lateinit var syncScheduler: dagger.Lazy<SyncScheduler>
    @Inject @AppScope lateinit var appScope: CoroutineScope

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    override fun onCreate() {
        super.onCreate()
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) installStrictMode()
        appScope.launch(Dispatchers.IO) {
            // No default categories here: a phone that connects takes the server's. Onboarding
            // seeds them for a ledger that starts on this phone.
            runCatching { poster.get().postDue() }
            // The weekly backup follows its switch in Settings, which is on unless turned off.
            runCatching { BackupWorker.sync(this@TallyApp, settings.get().currentBackup().auto) }
            // Connects to the owner's server by itself, then syncs after changes, on a network
            // coming back, and every fifteen minutes.
            runCatching { syncScheduler.get().start() }
        }
        RecurringWorker.schedule(this)
        runCatching { shortcuts() }
    }

    /** The launcher icon's long-press: add an entry, or search, straight away. */
    private fun shortcuts() {
        fun shortcut(id: String, label: String, action: String, icon: Int) = ShortcutInfoCompat.Builder(this, id)
            .setShortLabel(label)
            .setIcon(IconCompat.createWithResource(this, icon))
            .setIntent(Intent(this, MainActivity::class.java).setAction(action).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP))
            .build()
        ShortcutManagerCompat.setDynamicShortcuts(
            this,
            listOf(
                shortcut("add", "Add an entry", Launches.ADD, R.drawable.ic_shortcut_add),
                shortcut("search", "Search entries", Launches.SEARCH, R.drawable.ic_shortcut_search),
            ),
        )
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
