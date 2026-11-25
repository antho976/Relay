package com.quietsoftware.leaf;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.job.JobInfo;
import android.app.job.JobParameters;
import android.app.job.JobScheduler;
import android.app.job.JobService;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/**
 * New chapters of the series whose bell is on, announced while Leaf is closed.
 *
 * About once an hour, with a network, this asks the server Leaf is connected to (the app's own
 * {@code files/config.json}: {@code {"server":{"base":...,"token":...}}}) for what happened since
 * the last time ({@code GET <base>/v1/events?since=<ms>}) and posts one notice per event of kind
 * "new"; the server makes those only for belled series. Tapping one opens Leaf. The time of the
 * last event shown is kept, so nothing is announced twice; the first run only notes the time, so
 * a new phone isn't flooded with the server's history.
 *
 * When to post is Rust's ({@code notices.rs}, through {@link #nativePlan}): quiet hours hold
 * notices until they end, and "one summary a day" posts a single one at its time; what is held is
 * kept here between runs. Without the library, every notice goes at once as before.
 */
public class LeafChaptersJob extends JobService {
    private static final int JOB_ID = 0x4c4a;
    private static final String CHANNEL = "chapters";
    private static final String PREFS = "leaf-notices";
    private static final String SINCE = "since";
    private static final String PLAN = "plan";

    private static final boolean NATIVE;

    static {
        boolean ok;
        try {
            System.loadLibrary("leaf");
            ok = true;
        } catch (Throwable e) {
            ok = false;
        }
        NATIVE = ok;
    }

    /** What to post: {"post":[{"id","text"}],"state":{...}} (notices.rs). */
    private static native String nativePlan(String notify, String state, String events, long nowMs, int offsetMin);

    /** The server's second address (over Tailscale), built in; "" when there is none. */
    private static native String nativeAltBase();

    /** Keeps the hourly job scheduled; a job already there is left alone, so its hour holds. */
    public static void schedule(Context c) {
        JobScheduler js = c.getSystemService(JobScheduler.class);
        if (js == null || js.getPendingJob(JOB_ID) != null) return;
        JobInfo job = new JobInfo.Builder(JOB_ID, new ComponentName(c, LeafChaptersJob.class))
                .setPeriodic(60L * 60 * 1000)
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setPersisted(true)
                .build();
        try {
            js.schedule(job);
        } catch (RuntimeException ignored) {
            // Too many jobs, or the boot permission withheld: no notices, nothing else lost.
        }
    }

    private Thread worker;

    @Override
    public boolean onStartJob(final JobParameters params) {
        worker = new Thread(() -> {
            boolean retry = false;
            try {
                check();
            } catch (Exception e) {
                retry = true;
            }
            jobFinished(params, retry);
        }, "leaf-chapters");
        worker.start();
        return true;
    }

    @Override
    public boolean onStopJob(JobParameters params) {
        if (worker != null) worker.interrupt();
        return true;
    }

    private void check() throws Exception {
        // No settings yet (someone who only reads books): nothing to ask, and nothing to retry.
        File config = new File(getFilesDir(), "config.json");
        if (!config.isFile()) return;
        JSONObject cfg = new JSONObject(read(new FileInputStream(config)));
        JSONObject server = cfg.optJSONObject("server");
        JSONObject notify = cfg.optJSONObject("notify");
        if (server == null) return;
        String base = server.optString("base", "");
        String token = server.isNull("token") ? "" : server.optString("token", "");
        if (base.isEmpty()) return;
        SharedPreferences prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        long since = prefs.getLong(SINCE, -1);
        if (since < 0) {
            prefs.edit().putLong(SINCE, System.currentTimeMillis()).apply();
            return;
        }
        // A network that blocks the server's address (a public wifi) is gone round by its second
        // address, as the app does (net.rs, Client::go).
        String alt = server.isNull("alt") ? "" : server.optString("alt", "");
        if (alt.isEmpty() && NATIVE) {
            try {
                alt = nativeAltBase();
            } catch (Throwable e) {
                alt = "";
            }
        }
        HttpURLConnection http;
        try {
            http = ask(base, since, token);
        } catch (Exception e) {
            if (alt == null || alt.isEmpty() || alt.replaceAll("/+$", "").equals(base.replaceAll("/+$", ""))) throw e;
            http = ask(alt, since, token);
        }
        try {
            if (http.getResponseCode() != 200) throw new Exception("server answered " + http.getResponseCode());
            JSONArray events = new JSONArray(read(http.getInputStream()));
            long last = since;
            for (int i = 0; i < events.length(); i++) {
                last = Math.max(last, events.getJSONObject(i).optLong("at", 0));
            }
            if (!planned(prefs, notify, events)) {
                for (int i = 0; i < events.length(); i++) {
                    JSONObject e = events.getJSONObject(i);
                    String kind = e.optString("kind");
                    if ("new".equals(kind) || "elsewhere".equals(kind)) {
                        long at = e.optLong("at", 0);
                        notify((int) (at % Integer.MAX_VALUE), e.optString("text", "New chapters"));
                    }
                }
            }
            if (last > since) prefs.edit().putLong(SINCE, last).apply();
        } finally {
            http.disconnect();
        }
    }

    /** The events since {@code since} from one address; throws when it can't be reached or a
     *  proxy in front of it says it is down. */
    private static HttpURLConnection ask(String base, long since, String token) throws Exception {
        HttpURLConnection http = (HttpURLConnection) new URL(base.replaceAll("/+$", "") + "/v1/events?since=" + since).openConnection();
        http.setConnectTimeout(15000);
        http.setReadTimeout(30000);
        http.setRequestProperty("User-Agent", "Leaf");
        if (!token.isEmpty()) http.setRequestProperty("Authorization", "Bearer " + token);
        int code = http.getResponseCode();
        if ((code >= 502 && code <= 504) || (code >= 520 && code <= 530)) {
            http.disconnect();
            throw new Exception("server answered " + code);
        }
        return http;
    }

    /** Posts what Rust says to post now, and keeps what it holds. False without the library. */
    private boolean planned(SharedPreferences prefs, JSONObject notify, JSONArray events) {
        if (!NATIVE) return false;
        try {
            long now = System.currentTimeMillis();
            int offset = java.util.TimeZone.getDefault().getOffset(now) / 60000;
            String answer = nativePlan(notify == null ? "{}" : notify.toString(), prefs.getString(PLAN, "{}"), events.toString(), now, offset);
            JSONObject plan = new JSONObject(answer);
            JSONArray post = plan.optJSONArray("post");
            for (int i = 0; post != null && i < post.length(); i++) {
                JSONObject p = post.getJSONObject(i);
                notify(p.optInt("id"), p.optString("text", "New chapters"));
            }
            JSONObject state = plan.optJSONObject("state");
            if (state != null) prefs.edit().putString(PLAN, state.toString()).apply();
            return true;
        } catch (Throwable e) {
            return false;
        }
    }

    private void notify(int id, String text) {
        if (android.os.Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return;
        }
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, "New chapters", NotificationManager.IMPORTANCE_DEFAULT));
        Intent open = new Intent(this, LeafActivity.class).setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        PendingIntent tap = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification n = new Notification.Builder(this, CHANNEL).setSmallIcon(R.drawable.ic_notify)
                .setContentTitle(text)
                .setContentIntent(tap)
                .setAutoCancel(true)
                .build();
        nm.notify(id, n);
    }

    private static String read(InputStream in) throws Exception {
        try (InputStream s = in) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            int n;
            while ((n = s.read(buf)) > 0) {
                out.write(buf, 0, n);
                if (out.size() > (1 << 20)) break;
            }
            return out.toString(StandardCharsets.UTF_8.name());
        }
    }
}
