package com.quietsoftware.relaycode;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.net.ConnectivityManager;
import android.net.Network;
import android.os.IBinder;

import androidx.core.app.ServiceCompat;

/**
 * Keeps Relay Code's process, and so its link to the paired PC (Rust, rc-link), alive while the
 * app is in the background, with one quiet ongoing notification saying where the link stands
 * (design 44, Q2: the owner approved this service). It also tells Rust when the network changes,
 * so the link redials at once instead of waiting out its backoff. It holds no connection itself.
 *
 * Android can start it again by itself (START_STICKY) in a fresh process with no activity, so it
 * loads librelaycode.so itself, and no native call or Android refusal here may throw: a service
 * that throws is restarted, throws again, and crash-loops.
 */
public class LinkService extends Service {
    static final String WORDS = "words";
    /** On a notification's tap: the agent whose chat opens. */
    static final String AGENT = "agent";
    private static final String CHANNEL = "link";
    private static final String ASKS = "asks";
    private static final int ID = 1;

    /** Whether librelaycode.so is loaded: only GameActivity loads it otherwise. */
    private static final boolean LOADED = load();

    private static boolean load() {
        try {
            System.loadLibrary("relaycode");
            return true;
        } catch (Throwable e) {
            return false;
        }
    }

    /** The service while it runs: its line is changed in place. */
    private static volatile LinkService running;

    /** The network changed: Rust redials now. */
    static native void nativeNetwork();

    /**
     * The link and the notifications start for the process (Rust, background.rs), so they run
     * with no activity too. Rust calls {@link #alert} and {@link #words} back from its own thread.
     */
    static native void nativeStart(Context context, String dataDir);

    private ConnectivityManager.NetworkCallback callback;

    /**
     * Changes the ongoing notification's line if the service runs, through NotificationManager
     * (starting a foreground service again from the background is refused on Android 12 and
     * later). False when it is not running, so the caller starts it. Any thread.
     */
    static boolean words(Context context, String words) {
        if (running == null) return false;
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        if (nm != null) nm.notify(ID, notification(context, words));
        return true;
    }

    /**
     * An agent asks you, finished, or has a command held: a notification whose tap opens its
     * chat. One per agent ({@code id}): a newer one replaces the older. Any thread.
     */
    static void alert(Context context, int id, String title, String body, String agent) {
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.createNotificationChannel(new NotificationChannel(ASKS, "Agents that need you", NotificationManager.IMPORTANCE_HIGH));
        Intent open = new Intent(context, CodeActivity.class)
                .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(AGENT, agent);
        PendingIntent tap = PendingIntent.getActivity(context, id, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification n = new Notification.Builder(context, ASKS)
                .setSmallIcon(R.drawable.ic_notify)
                .setContentTitle(title)
                .setContentText(body)
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build();
        nm.notify(id, n);
    }

    private static Notification notification(Context context, String words) {
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        if (nm != null) nm.createNotificationChannel(new NotificationChannel(CHANNEL, "Link to your PC", NotificationManager.IMPORTANCE_MIN));
        Intent open = new Intent(context, CodeActivity.class).setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent tap = PendingIntent.getActivity(context, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Builder(context, CHANNEL)
                .setSmallIcon(R.drawable.ic_notify)
                .setContentTitle(words != null && !words.isEmpty() ? words : "Relay Code")
                .setContentIntent(tap)
                .setOngoing(true)
                .build();
    }

    @Override
    public void onCreate() {
        super.onCreate();
        running = this;
        if (LOADED) {
            try {
                nativeStart(getApplicationContext(), getFilesDir().getAbsolutePath());
            } catch (UnsatisfiedLinkError ignored) {
                // A library without it: the activity's loop still notifies while it is open.
            }
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        // A sticky restart brings no intent: the line is the plain name until Rust says more.
        String words = intent != null ? intent.getStringExtra(WORDS) : null;
        int type = android.os.Build.VERSION.SDK_INT >= 34 ? ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE : 0;
        try {
            ServiceCompat.startForeground(this, ID, notification(this, words), type);
        } catch (RuntimeException e) {
            // Android refused a foreground start here (from the background, Android 12 and
            // later): stop rather than be killed for not starting, and do not come back by
            // ourselves. The next time the app is opened it starts the service again.
            stopSelf();
            return START_NOT_STICKY;
        }
        if (callback == null) {
            ConnectivityManager cm = getSystemService(ConnectivityManager.class);
            callback = new ConnectivityManager.NetworkCallback() {
                @Override
                public void onAvailable(Network network) {
                    if (!LOADED) return;
                    try {
                        nativeNetwork();
                    } catch (UnsatisfiedLinkError ignored) {
                        // A library without the call: the link redials on its own backoff.
                    }
                }
            };
            try {
                cm.registerDefaultNetworkCallback(callback);
            } catch (RuntimeException e) {
                callback = null;
            }
        }
        return START_STICKY;
    }

    @Override
    public void onDestroy() {
        if (running == this) running = null;
        if (callback != null) {
            try {
                getSystemService(ConnectivityManager.class).unregisterNetworkCallback(callback);
            } catch (RuntimeException ignored) {
                // Already gone.
            }
            callback = null;
        }
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
