package com.quietsoftware.relaycode;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
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
 */
public class LinkService extends Service {
    static final String WORDS = "words";
    private static final String CHANNEL = "link";
    private static final int ID = 1;

    /** The network changed: Rust redials now. */
    static native void nativeNetwork();

    private ConnectivityManager.NetworkCallback callback;

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String words = intent != null ? intent.getStringExtra(WORDS) : null;
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, "Link to your PC", NotificationManager.IMPORTANCE_MIN));
        Intent open = new Intent(this, CodeActivity.class).setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent tap = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification n = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.drawable.ic_notify)
                .setContentTitle(words != null && !words.isEmpty() ? words : "Relay Code")
                .setContentIntent(tap)
                .setOngoing(true)
                .build();
        int type = android.os.Build.VERSION.SDK_INT >= 34 ? ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE : 0;
        ServiceCompat.startForeground(this, ID, n, type);
        if (callback == null) {
            ConnectivityManager cm = getSystemService(ConnectivityManager.class);
            callback = new ConnectivityManager.NetworkCallback() {
                @Override
                public void onAvailable(Network network) {
                    nativeNetwork();
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
