package com.tally.rs;

import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;

import androidx.core.content.FileProvider;

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
 * - the launcher icon is switched between dark and light through {@link #setIconDark}, which
 *   needs the PackageManager.
 * Rust calls {@link #pickFile} and {@link #shareFile} on this activity; everything else, the
 * browser included, is done from Rust. Nothing here decides anything: it moves bytes and intents.
 */
public class TallyActivity extends GameActivity {
    private static final int PICK = 4101;
    /** The largest file handed to the app: a statement or a backup is far smaller. */
    private static final int MAX_BYTES = 32 * 1024 * 1024;

    /** The picked file's name and bytes to Rust; empty bytes when it could not be read. */
    private static native void nativeFilePicked(String name, byte[] bytes);

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

    /**
     * The launcher icon follows the app's palette: the dark icon (LauncherDark) or the light one
     * (LauncherLight). Only changes what differs, so calling it on every start costs nothing.
     */
    public void setIconDark(final boolean dark) {
        final android.content.pm.PackageManager pm = getPackageManager();
        final android.content.ComponentName on = new android.content.ComponentName(this, "com.tally.rs." + (dark ? "LauncherDark" : "LauncherLight"));
        final android.content.ComponentName off = new android.content.ComponentName(this, "com.tally.rs." + (dark ? "LauncherLight" : "LauncherDark"));
        try {
            if (pm.getComponentEnabledSetting(on) != android.content.pm.PackageManager.COMPONENT_ENABLED_STATE_ENABLED) {
                pm.setComponentEnabledSetting(on, android.content.pm.PackageManager.COMPONENT_ENABLED_STATE_ENABLED, android.content.pm.PackageManager.DONT_KILL_APP);
            }
            int offState = pm.getComponentEnabledSetting(off);
            if (offState != android.content.pm.PackageManager.COMPONENT_ENABLED_STATE_DISABLED) {
                pm.setComponentEnabledSetting(off, android.content.pm.PackageManager.COMPONENT_ENABLED_STATE_DISABLED, android.content.pm.PackageManager.DONT_KILL_APP);
            }
        } catch (Exception ignored) {
            // A launcher that will not switch keeps the icon it has.
        }
    }

    /** The Back gesture or key, to Rust. */
    private static native void nativeBack();

    @Override
    protected void onCreate(android.os.Bundle saved) {
        super.onCreate(saved);
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                nativeBack();
            }
        });
    }

    /** Asks the person for a file of {@code mime} (any file too: banks label CSVs oddly). */
    public void pickFile(final String mime) {
        runOnUiThread(() -> {
            Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            pick.addCategory(Intent.CATEGORY_OPENABLE);
            pick.setType("*/*");
            pick.putExtra(Intent.EXTRA_MIME_TYPES, new String[] {mime, "*/*"});
            try {
                startActivityForResult(pick, PICK);
            } catch (ActivityNotFoundException e) {
                nativeFilePicked("", new byte[0]);
            }
        });
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK || result != RESULT_OK || data == null || data.getData() == null) {
            return;
        }
        final Uri uri = data.getData();
        // Read off the main thread; Rust takes it from there.
        new Thread(() -> nativeFilePicked(nameOf(uri), read(uri))).start();
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

    private byte[] read(Uri uri) {
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            if (in == null) return new byte[0];
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
                if (out.size() > MAX_BYTES) return new byte[0];
            }
            return out.toByteArray();
        } catch (Exception e) {
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
