package com.quietsoftware.relay.service

import android.Manifest
import android.app.PendingIntent
import android.content.pm.PackageManager
import android.os.Build
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.quietsoftware.relay.MainActivity
import com.quietsoftware.relay.R
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * "Stay connected": keeps the link to the PC open with the app in the background, as an ongoing
 * notification that says where the link stands. Without it the link lives only while Relay is on
 * screen, and the background sync ([SyncWorker]) catches up every so often instead.
 */
@AndroidEntryPoint
class LinkService : Service() {
    @Inject lateinit var relay: Relay
    @Inject lateinit var settings: Settings

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var watch: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            // Stop means "not in the background any more": the switch goes off with it, so opening
            // the app does not start the service again behind the person's back.
            scope.launch { settings.update { it.copy(stayConnected = false) } }
            stopSelf()
            return START_NOT_STICKY
        }
        // The special-use type exists from Android 14; before it the manifest's type is enough.
        val type = if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE else 0
        ServiceCompat.startForeground(this, Notifier.LINK_ID, notification("Connecting to your PC"), type)
        relay.start()
        if (watch?.isActive != true) {
            watch = scope.launch {
                combine(relay.state, relay.profile) { state, profile -> state to profile }.collect { (state, profile) ->
                    val pc = profile?.name?.ifBlank { null } ?: "your PC"
                    val text = when (state) {
                        is LinkState.Online -> "Connected to $pc"
                        is LinkState.Starting -> "Starting Relay on $pc"
                        is LinkState.Connecting -> "Reaching $pc"
                        is LinkState.Offline -> "$pc is out of reach · edits wait on this phone"
                        LinkState.Unpaired -> "No PC paired"
                        LinkState.Stopped -> "Disconnected"
                        is LinkState.Revoked -> "$pc no longer knows this phone"
                    }
                    // The ongoing notification is the service's own; without the permission it is simply not updated.
                    if (Build.VERSION.SDK_INT < 33 || ContextCompat.checkSelfPermission(this@LinkService, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) {
                        try {
                            NotificationManagerCompat.from(this@LinkService).notify(Notifier.LINK_ID, notification(text))
                        } catch (_: SecurityException) {
                            // Taken back meanwhile.
                        }
                    }
                    if (state is LinkState.Revoked || state == LinkState.Unpaired) stopSelf()
                }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun notification(text: String) = NotificationCompat.Builder(this, Notifier.CH_LINK)
        .setSmallIcon(R.drawable.ic_stat_relay)
        .setContentTitle("Relay")
        .setContentText(text)
        .setOngoing(true)
        .setSilent(true)
        .setPriority(NotificationCompat.PRIORITY_MIN)
        .setContentIntent(PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE))
        .addAction(0, "Stop", PendingIntent.getService(this, 1, Intent(this, LinkService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE))
        .build()

    companion object {
        const val ACTION_STOP = "com.quietsoftware.relay.STOP_LINK"

        fun start(context: Context) {
            runCatching { ContextCompat.startForegroundService(context, Intent(context, LinkService::class.java)) }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, LinkService::class.java))
        }
    }
}

/** After a reboot or an update, "Stay connected" comes back on its own. */
@AndroidEntryPoint
class BootReceiver : BroadcastReceiver() {
    @Inject lateinit var relay: Relay
    @Inject lateinit var settings: Settings

    // Off the main thread: whether it is paired unseals the token, a keystore call. goAsync keeps
    // the receiver alive for that read, well inside its ten seconds.
    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        CoroutineScope(Dispatchers.IO).launch {
            try {
                if (settings.prefs.first().stayConnected && relay.isPaired()) LinkService.start(context)
                SyncWorker.schedule(context)
            } finally {
                pending.finish()
            }
        }
    }
}
