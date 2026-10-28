package com.quietsoftware.relay

import android.app.Activity
import android.app.Application
import android.content.Intent
import android.os.Bundle
import android.os.StrictMode
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.di.AppScope
import com.quietsoftware.relay.service.LinkService
import com.quietsoftware.relay.service.Notifier
import com.quietsoftware.relay.service.SyncWorker
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltAndroidApp
class RelayApp : Application(), Configuration.Provider {
    @Inject lateinit var workerFactory: HiltWorkerFactory
    @Inject lateinit var relay: Relay
    @Inject lateinit var settings: Settings
    @Inject @AppScope lateinit var scope: CoroutineScope

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().setWorkerFactory(workerFactory).build()

    /** Activities started and not stopped: the app is on screen while this is above zero. */
    @Volatile private var started = 0

    val foreground: Boolean get() = started > 0

    /** Long-press the launcher icon: the four things most often opened straight away. */
    private fun shortcuts() {
        val list = listOf(
            "new-agent" to "New agent",
            "new-task" to "New task",
            "new-thread" to "New thread",
            "inbox" to "Inbox",
        ).mapIndexed { i, (open, label) ->
            ShortcutInfoCompat.Builder(this, open)
                .setShortLabel(label)
                .setRank(i)
                .setIcon(IconCompat.createWithResource(this, R.mipmap.ic_launcher))
                .setIntent(Intent(this, MainActivity::class.java).setAction(Intent.ACTION_VIEW).putExtra(MainActivity.EXTRA_OPEN, open))
                .build()
        }
        runCatching { ShortcutManagerCompat.setDynamicShortcuts(this, list) }
    }

    override fun onCreate() {
        super.onCreate()
        // A debug build logs disk and network on the main thread, and leaked closeables, so a slow
        // call there shows in logcat before anyone feels it.
        if (BuildConfig.DEBUG) {
            StrictMode.setThreadPolicy(StrictMode.ThreadPolicy.Builder().detectDiskReads().detectDiskWrites().detectNetwork().penaltyLog().build())
            StrictMode.setVmPolicy(StrictMode.VmPolicy.Builder().detectLeakedClosableObjects().detectLeakedSqlLiteObjects().penaltyLog().build())
        }
        val notifier = Notifier(this, relay, settings)
        notifier.start(scope) { foreground }
        relay.start()
        SyncWorker.schedule(this)
        shortcuts()
        registerActivityLifecycleCallbacks(object : ActivityLifecycleCallbacks {
            override fun onActivityStarted(activity: Activity) {
                if (started++ == 0) {
                    relay.kick()
                    notifier.clear()
                    // A foreground service may only be started from the foreground (Android 12):
                    // start it now, so it is already running when the app leaves the screen.
                    scope.launch {
                        if (settings.prefs.first().stayConnected && relay.isPaired()) LinkService.start(this@RelayApp)
                    }
                }
            }

            override fun onActivityStopped(activity: Activity) {
                started = (started - 1).coerceAtLeast(0)
            }

            override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit
            override fun onActivityResumed(activity: Activity) = Unit
            override fun onActivityPaused(activity: Activity) = Unit
            override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
            override fun onActivityDestroyed(activity: Activity) = Unit
        })
    }
}
