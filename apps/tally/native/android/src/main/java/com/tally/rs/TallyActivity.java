package com.tally.rs;

import android.app.ActivityManager;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ComponentName;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;

import androidx.core.content.FileProvider;
import androidx.core.view.WindowCompat;

import com.google.androidgamesdk.GameActivity;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.InputStream;

/**
 * Tally RS's activity: GameActivity, plus the two things Android hands an app only through Java.
 *
 * The app is Rust (libtally.so); this class exists because
 * - the file picker answers through {@link #onActivityResult}, a callback only an Activity
 *   subclass receives, and
 * - the share sheet needs {@link FileProvider}, an app class that JNI's FindClass cannot reach
 *   from Rust's own thread (it sees only the system's classes), and
 * - Back: AndroidX's back dispatcher finishes the activity before the key reaches native code,
 *   so a callback here hands every Back to Rust, which goes back a page or leaves.
 * - the launcher icon is switched between dark and light through {@link #setIcon}, which
 *   needs the PackageManager, and {@link #leave}, which picks the moment it is safe to.
 * Rust calls {@link #pickFile} and {@link #shareFile} on this activity; everything else, the
 * browser included, is done from Rust. Nothing here decides anything: it moves bytes and intents.
 */
public class TallyActivity extends GameActivity {
    /**
     * A pick's request code: PICK with the pick's number (Rust's settings_files::begin_pick) in
     * the low bits, PICK_MASK (settings_files::PICK_TAG_MASK, the two must agree). The answer
     * carries it back, to the activity and process that asked or to the ones the system made
     * again meanwhile, so Rust knows which pick a file answers. Within the low 16 bits, which
     * is all a request code may use.
     */
    private static final int PICK = 0x4000;
    private static final int PICK_MASK = 0x3fff;
    /** The largest file handed to the app: a statement or a backup is far smaller. */
    private static final int MAX_BYTES = 32 * 1024 * 1024;

    /**
     * The picked file's name and bytes to Rust, for the pick tagged {@code pick}; empty bytes
     * when it could not be read (or is too big).
     */
    private static native void nativeFilePicked(long pick, String name, byte[] bytes);

    /** The picker for pick {@code pick} came back with no file (cancelled): Rust stops waiting. */
    private static native void nativePickCancelled(long pick);

    /**
     * Dark icons in the status and navigation bars over a light palette, light ones over Night.
     * Window changes must happen on the UI thread, which Rust's thread is not.
     */
    public void setLightBars(final boolean light) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                androidx.core.view.WindowInsetsControllerCompat c =
                    androidx.core.view.WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
                c.setAppearanceLightStatusBars(light);
                c.setAppearanceLightNavigationBars(light);
            }
        });
    }

    /** The launcher icon wanted (dark or light); null leaves whichever is on alone. */
    private volatile Boolean iconDark;

    /**
     * The launcher icon follows Appearance when the person picked Light or Dark there: the dark
     * icon (LauncherDark) or the light one (LauncherLight). {@code choice} is 0 while Appearance
     * follows the phone (the icon on stays, whichever it is), 1 for Light, 2 for Dark. It never
     * follows the phone's own light and dark: Pixel Launcher and Launcher3 delete the home-screen
     * shortcut of an alias turned off, so that would take the icon off the home screen at every
     * change.
     *
     * Turning an alias off also makes Android remove, some seconds later, every task that alias
     * started, wherever it is: the app would close by itself, a half-typed entry with it (and a
     * picker or the bank's browser out would come back to nothing). So the switch waits for a
     * moment no task of the app was started by the alias going off: now if that is so (a task
     * the bank's link started), else when the person leaves from the first page ({@link #leave})
     * or the activity finishes.
     *
     * Answers true when the icon is about to change and the person has not been told for this
     * choice yet: Rust then says the icon may need adding to the home screen again. Once per
     * change, across launches (a switch still waiting is not told again at the next start).
     */
    public boolean setIcon(final int choice) {
        iconDark = choice == 0 ? null : choice == 2;
        boolean waiting = iconWaiting();
        SharedPreferences prefs = getSharedPreferences("tally_icon", MODE_PRIVATE);
        int told = prefs.getInt("told", 0);
        boolean tell = waiting && told != choice;
        if (tell) {
            prefs.edit().putInt("told", choice).apply();
        } else if (!waiting && told != 0) {
            // In place (or left alone): the next change is told afresh.
            prefs.edit().remove("told").apply();
        }
        runOnUiThread(() -> {
            Boolean want = iconDark;
            if (want != null && !startedBy(alias(!want))) {
                switchIcon();
            }
        });
        return tell;
    }

    private ComponentName alias(boolean dark) {
        return new ComponentName(this, "com.tally.rs." + (dark ? "LauncherDark" : "LauncherLight"));
    }

    /** On, counting the manifest's default (LauncherDark on, LauncherLight off). */
    private boolean enabled(PackageManager pm, ComponentName c) throws PackageManager.NameNotFoundException {
        int state = pm.getComponentEnabledSetting(c);
        if (state == PackageManager.COMPONENT_ENABLED_STATE_DEFAULT) {
            return pm.getActivityInfo(c, PackageManager.MATCH_DISABLED_COMPONENTS).enabled;
        }
        return state == PackageManager.COMPONENT_ENABLED_STATE_ENABLED;
    }

    /** The icon showing is not the one wanted. */
    private boolean iconWaiting() {
        Boolean want = iconDark;
        if (want == null) return false;
        PackageManager pm = getPackageManager();
        try {
            return !enabled(pm, alias(want)) || enabled(pm, alias(!want));
        } catch (Exception e) {
            return false;
        }
    }

    /** A task of this app was started through {@code c} (when unsure, yes). */
    private boolean startedBy(ComponentName c) {
        try {
            ActivityManager am = getSystemService(ActivityManager.class);
            if (am == null) return true;
            for (ActivityManager.AppTask task : am.getAppTasks()) {
                ActivityManager.RecentTaskInfo info = task.getTaskInfo();
                if (c.equals(info.origActivity) || (info.baseIntent != null && c.equals(info.baseIntent.getComponent()))) {
                    return true;
                }
            }
            return false;
        } catch (Exception e) {
            return true;
        }
    }

    /** Turns the wanted icon on and the other off. Only changes what differs. */
    private void switchIcon() {
        Boolean want = iconDark;
        if (want == null) return;
        PackageManager pm = getPackageManager();
        ComponentName on = alias(want);
        ComponentName off = alias(!want);
        try {
            if (!enabled(pm, on)) {
                pm.setComponentEnabledSetting(on, PackageManager.COMPONENT_ENABLED_STATE_ENABLED, PackageManager.DONT_KILL_APP);
            }
            if (enabled(pm, off)) {
                pm.setComponentEnabledSetting(off, PackageManager.COMPONENT_ENABLED_STATE_DISABLED, PackageManager.DONT_KILL_APP);
            }
        } catch (Exception ignored) {
            // A launcher that will not switch keeps the icon it has.
        }
    }

    /**
     * Back on the first page: the task goes to the back. With an icon switch waiting, the task is
     * finished and taken out of Recents instead, and the icon switched: nothing is open to lose,
     * and coming back starts afresh from the new icon rather than in a task Android is about to
     * remove.
     */
    public void leave() {
        runOnUiThread(() -> {
            if (iconWaiting()) {
                finishAndRemoveTask();
                switchIcon();
            } else {
                moveTaskToBack(true);
            }
        });
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        // Its task is going anyway: no harm in turning off the alias that started it.
        if (isFinishing() && iconWaiting()) {
            switchIcon();
        }
    }

    /** The Back gesture or key, to Rust. */
    private static native void nativeBack();

    @Override
    protected void onCreate(android.os.Bundle saved) {
        // GameActivity reads the library's name from the component in the intent. A launch (or a
        // restore after the process was stopped) through an alias since turned off would make
        // that lookup throw: name this activity itself, which carries the same meta-data and is
        // never turned off.
        Intent launch = getIntent();
        ComponentName self = new ComponentName(this, TallyActivity.class);
        if (launch != null && !self.equals(launch.getComponent())) {
            setIntent(new Intent(launch).setComponent(self));
        }
        super.onCreate(saved);
        // Edge to edge on every version: the surface is the whole screen, and the app alone keeps
        // clear of the bars and the keyboard (android.rs reads their insets). Without this,
        // before Android 15 the window already stops at the bars and shrinks for the keyboard
        // (adjustResize), and the app would make room for them twice. Below Android 11 this sets
        // the LAYOUT_STABLE, LAYOUT_FULLSCREEN and LAYOUT_HIDE_NAVIGATION flags.
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                nativeBack();
            }
        });
    }

    /**
     * Asks the person for a file of {@code mime} (any file too: banks label CSVs oddly), for
     * Rust's pick numbered {@code n}.
     */
    public void pickFile(final String mime, final long n) {
        final long tag = n & PICK_MASK;
        runOnUiThread(() -> {
            Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            pick.addCategory(Intent.CATEGORY_OPENABLE);
            pick.setType("*/*");
            pick.putExtra(Intent.EXTRA_MIME_TYPES, new String[] {mime, "*/*"});
            try {
                startActivityForResult(pick, PICK | (int) tag);
            } catch (ActivityNotFoundException e) {
                nativeFilePicked(tag, "", new byte[0]);
            }
        });
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if ((request & ~PICK_MASK) != PICK) {
            return;
        }
        final long tag = request & PICK_MASK;
        if (result != RESULT_OK || data == null || data.getData() == null) {
            // Cancelled: Rust lets go of the pick, rather than keep it as out for ever.
            nativePickCancelled(tag);
            return;
        }
        final Uri uri = data.getData();
        // Read off the main thread; Rust takes it from there.
        new Thread(() -> nativeFilePicked(tag, nameOf(uri), read(uri))).start();
    }

    private String nameOf(Uri uri) {
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                String name = c.getString(0);
                if (name != null) return name;
            }
        } catch (Exception ignored) {
            // The name is only shown; the bytes are what matter.
        }
        return "file";
    }

    /** The size the provider gives, or -1 when it gives none. */
    private long sizeOf(Uri uri) {
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.SIZE}, null, null, null)) {
            if (c != null && c.moveToFirst() && !c.isNull(0)) {
                return c.getLong(0);
            }
        } catch (Exception ignored) {
            // Not every provider knows; the read below stops at MAX_BYTES anyway.
        }
        return -1;
    }

    /**
     * The file's bytes, or none when it cannot be read or is bigger than MAX_BYTES (Rust then
     * says it could not be opened). A file the provider says is too big is not read at all, and
     * one that gives no size stops being read at MAX_BYTES, before it is held: this runs on a
     * thread of its own, where running out of memory would end the app.
     */
    private byte[] read(Uri uri) {
        long size = sizeOf(uri);
        if (size > MAX_BYTES) return new byte[0];
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            if (in == null) return new byte[0];
            ByteArrayOutputStream out = new ByteArrayOutputStream(size > 0 ? (int) size : 64 * 1024);
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) {
                if (out.size() + n > MAX_BYTES) return new byte[0];
                out.write(buf, 0, n);
            }
            return out.toByteArray();
        } catch (Throwable e) {
            // OutOfMemoryError too: the person hears the file could not be opened, and the app
            // carries on.
            return new byte[0];
        }
    }

    /** Hands the file Rust wrote under the cache's {@code shared} folder to the share sheet. */
    public void shareFile(final String path, final String mime) {
        runOnUiThread(() -> {
            File file = new File(path);
            Uri uri = FileProvider.getUriForFile(this, getPackageName() + ".files", file);
            Intent send = new Intent(Intent.ACTION_SEND);
            send.setType(mime);
            send.putExtra(Intent.EXTRA_STREAM, uri);
            send.setClipData(ClipData.newRawUri(file.getName(), uri));
            send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            try {
                startActivity(Intent.createChooser(send, file.getName()));
            } catch (ActivityNotFoundException ignored) {
                // No app takes files: nothing to hand it to.
            }
        });
    }
}
