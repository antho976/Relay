package com.quietsoftware.leaf;

import android.app.job.JobInfo;
import android.app.job.JobParameters;
import android.app.job.JobScheduler;
import android.app.job.JobService;
import android.content.ComponentName;
import android.content.Context;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.util.Log;

/**
 * Auto downloads' night run while Leaf is closed (smart.rs): about once an hour, with a network,
 * Rust looks whether tonight's run is due (on, and not run since midnight), and if so deletes the
 * chapters read, puts the next ones in line and saves them before the job's time is up. Everything
 * it decides is Rust's ({@link #nativeNight}); this only gives it the hours, the folder and whether
 * the network is metered. When Leaf is open, the app runs it itself and this finds it done.
 */
public class LeafNightJob extends JobService {
    private static final int JOB_ID = 0x4c4e;

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

    /** What it did, in words for the log ("not due", "added 3, removed 2"). */
    private static native String nativeNight(String filesDir, long nowMs, int offsetMin, boolean unmetered);

    /** Keeps the hourly job scheduled; a job already there is left alone. */
    public static void schedule(Context c) {
        JobScheduler js = c.getSystemService(JobScheduler.class);
        if (js == null || js.getPendingJob(JOB_ID) != null) return;
        JobInfo job = new JobInfo.Builder(JOB_ID, new ComponentName(c, LeafNightJob.class))
                .setPeriodic(60L * 60 * 1000)
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setPersisted(true)
                .build();
        try {
            js.schedule(job);
        } catch (RuntimeException ignored) {
            // Too many jobs, or the boot permission withheld: the app still runs it when open.
        }
    }

    private Thread worker;

    @Override
    public boolean onStartJob(final JobParameters params) {
        if (!NATIVE) return false;
        worker = new Thread(() -> {
            try {
                long now = System.currentTimeMillis();
                int offset = java.util.TimeZone.getDefault().getOffset(now) / 60000;
                String did = nativeNight(getFilesDir().getAbsolutePath(), now, offset, unmetered());
                Log.i("leaf", "night: " + did);
            } catch (Throwable e) {
                Log.w("leaf", "night: " + e);
            }
            jobFinished(params, false);
        }, "leaf-night");
        worker.start();
        return true;
    }

    @Override
    public boolean onStopJob(JobParameters params) {
        // Out of time: what is half saved is taken up where it stopped next time.
        return true;
    }

    private boolean unmetered() {
        ConnectivityManager cm = (ConnectivityManager) getSystemService(CONNECTIVITY_SERVICE);
        if (cm == null) return false;
        Network n = cm.getActiveNetwork();
        if (n == null) return false;
        NetworkCapabilities c = cm.getNetworkCapabilities(n);
        return c != null && c.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED);
    }
}
