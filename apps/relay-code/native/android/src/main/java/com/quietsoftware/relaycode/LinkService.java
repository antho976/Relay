package com.quietsoftware.relaycode;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.RemoteInput;
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

    /** The screen went on or off: off, Rust reads nothing on a timer. */
    static native void nativeScreen(boolean on);

    /** Something was done from a notification ("reply", "allow" or "deny"): Rust asks the PC. */
    static native void nativeAct(String kind, String agent, long hold, String text);

    /** Whether the native side can take a call now. */
    static boolean loaded() {
        return LOADED;
    }

    /** A notification's actions ({@link #alert}'s kind): open only, reply, a hold, or quiet. */
    static final int KIND_OPEN = 0;
    static final int KIND_REPLY = 1;
    static final int KIND_HOLD = 2;
    static final int KIND_QUIET = 3;

    private android.content.BroadcastReceiver screen;

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
    static void alert(Context context, int id, String title, String body, String agent, int kind, long hold) {
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.createNotificationChannel(new NotificationChannel(ASKS, "Agents that need you", NotificationManager.IMPORTANCE_HIGH));
        Intent open = new Intent(context, CodeActivity.class)
                .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(AGENT, agent);
        PendingIntent tap = PendingIntent.getActivity(context, id, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder b = new Notification.Builder(context, ASKS)
                .setSmallIcon(R.drawable.ic_notify)
                .setContentTitle(title)
                .setContentText(body)
                .setContentIntent(tap)
                .setAutoCancel(true);
        // What can be done from the shade (D18): a reply, or a held command allowed once or denied.
        // Each acts as the owner, so each needs the phone unlocked; Open unlocks through the app.
        if (kind == KIND_REPLY) {
            RemoteInput words = new RemoteInput.Builder(ActionReceiver.REPLY).setLabel("Reply to " + agent).build();
            b.addAction(unlocked(new Notification.Action.Builder(null, "Reply", ActionReceiver.intent(context, id, "reply", agent, 0, true))
                    .addRemoteInput(words)
                    .setAllowGeneratedReplies(false)));
            b.addAction(new Notification.Action.Builder(null, "Open", tap).build());
        } else if (kind == KIND_HOLD) {
            b.addAction(unlocked(new Notification.Action.Builder(null, "Allow once", ActionReceiver.intent(context, id, "allow", agent, hold, false))));
            b.addAction(unlocked(new Notification.Action.Builder(null, "Deny", ActionReceiver.intent(context, id, "deny", agent, hold, false))));
        } else if (kind == KIND_QUIET) {
            // What was done: no sound again, and it clears by itself.
            b.setOnlyAlertOnce(true).setTimeoutAfter(10_000);
        }
        // On a locked screen only that something waits shows: no question, no command, no actions
        // (on every Android, so also where actions cannot ask for the phone unlocked).
        b.setVisibility(Notification.VISIBILITY_PRIVATE).setPublicVersion(new Notification.Builder(context, ASKS)
                .setSmallIcon(R.drawable.ic_notify)
                .setContentTitle("Relay Code")
                .setContentText(lockedWords(kind))
                .build());
        nm.notify(id, b.build());
    }

    /** An action that acts as the owner: it needs the phone unlocked (Android 12 and later). */
    static Notification.Action unlocked(Notification.Action.Builder action) {
        if (android.os.Build.VERSION.SDK_INT >= 31) action.setAuthenticationRequired(true);
        return action.build();
    }

    /** What a notification says on a locked screen, by its kind: that something waits, not what. */
    static String lockedWords(int kind) {
        switch (kind) {
            case KIND_REPLY: return "An agent asks you";
            case KIND_HOLD: return "A command needs your OK";
            case KIND_QUIET: return "Done";
            default: return "An agent has news";
        }
    }

    /** Takes a notification away (what it asked was done). Any thread. */
    static void clear(Context context, int id) {
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        if (nm != null) nm.cancel(id);
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
                screen = new android.content.BroadcastReceiver() {
                    @Override
                    public void onReceive(Context context, Intent intent) {
                        try {
                            nativeScreen(Intent.ACTION_SCREEN_ON.equals(intent.getAction()));
                        } catch (UnsatisfiedLinkError ignored) {
                            // A library without it: timers run as before.
                        }
                    }
                };
                android.content.IntentFilter f = new android.content.IntentFilter(Intent.ACTION_SCREEN_ON);
                f.addAction(Intent.ACTION_SCREEN_OFF);
                // System broadcasts: a dynamic receiver needs no export flag for them.
                registerReceiver(screen, f);
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
        if (screen != null) {
            try {
                unregisterReceiver(screen);
            } catch (RuntimeException ignored) {
                // Already gone.
            }
            screen = null;
        }
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
