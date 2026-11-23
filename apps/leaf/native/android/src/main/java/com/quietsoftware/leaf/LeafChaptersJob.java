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
 */
public class LeafChaptersJob extends JobService {
    private static final int JOB_ID = 0x4c4a;
    private static final String CHANNEL = "chapters";
    private static final String PREFS = "leaf-notices";
    private static final String SINCE = "since";

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
        JSONObject server = new JSONObject(read(new FileInputStream(new File(getFilesDir(), "config.json")))).optJSONObject("server");
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
        HttpURLConnection http = (HttpURLConnection) new URL(base.replaceAll("/+$", "") + "/v1/events?since=" + since).openConnection();
        http.setConnectTimeout(15000);
        http.setReadTimeout(30000);
        http.setRequestProperty("User-Agent", "Leaf");
        if (!token.isEmpty()) http.setRequestProperty("Authorization", "Bearer " + token);
        try {
            if (http.getResponseCode() != 200) throw new Exception("server answered " + http.getResponseCode());
            JSONArray events = new JSONArray(read(http.getInputStream()));
            long last = since;
            for (int i = 0; i < events.length(); i++) {
                JSONObject e = events.getJSONObject(i);
                long at = e.optLong("at", 0);
                last = Math.max(last, at);
                if ("new".equals(e.optString("kind"))) {
                    notify((int) (at % Integer.MAX_VALUE), e.optString("text", "New chapters"));
                }
            }
            if (last > since) prefs.edit().putLong(SINCE, last).apply();
        } finally {
            http.disconnect();
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
