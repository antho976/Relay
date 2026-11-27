package com.quietsoftware.relaycode;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;

import androidx.core.content.ContextCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.androidgamesdk.GameActivity;

/**
 * Relay Code's activity: GameActivity, plus what Android gives an app only through Java.
 *
 * The app is Rust (librelaycode.so); this class exists because
 * - Back: AndroidX's back dispatcher finishes the activity before the key reaches native code, so
 *   a callback here hands every Back to Rust, which closes a sheet, goes back a page or leaves;
 * - a tapped notification names the agent whose chat opens (LinkService posts them);
 * - the clipboard and starting the foreground service are Java-only APIs.
 * Nothing here decides anything: Rust says what to show and when.
 */
public class CodeActivity extends GameActivity {
    /** The Back gesture or key, to Rust. */
    private static native void nativeBack();

    /** A notification was tapped: the chat of {@code agent} opens. */
    private static native void nativeOpen(String agent);

    /** A picked photo, shrunk and saved at {@code path}, to Rust. */
    private static native void nativePhoto(String path);

    private static final int PICK_PHOTO = 0x5250;
    /** The longest side a photo is sent at: enough for an agent to read a screenshot. */
    private static final int MAX_SIDE = 2048;

    @Override
    protected void onCreate(android.os.Bundle saved) {
        super.onCreate(saved);
        // Edge to edge on every version: the app keeps clear of the bars itself (android.rs).
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        WindowInsetsControllerCompat c = WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        c.setAppearanceLightStatusBars(false);
        c.setAppearanceLightNavigationBars(false);
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                nativeBack();
            }
        });
        opened(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        opened(intent);
    }

    /** A notification's tap names the agent whose chat opens (LinkService.alert); once. */
    private void opened(Intent intent) {
        String agent = intent != null ? intent.getStringExtra(LinkService.AGENT) : null;
        if (agent == null || agent.isEmpty()) return;
        intent.removeExtra(LinkService.AGENT);
        nativeOpen(agent);
    }

    public void copyText(final String text) {
        runOnUiThread(() -> {
            ClipboardManager cm = getSystemService(ClipboardManager.class);
            if (cm != null) cm.setPrimaryClip(ClipData.newPlainText("Relay Code", text));
        });
    }

    /** Opens a web address the agent wrote, in the browser or the app that handles it. */
    public void openUrl(final String url) {
        runOnUiThread(() -> {
            Uri u = Uri.parse(url);
            String scheme = u.getScheme();
            // Only web addresses: a link in an agent's words never opens anything else.
            if (scheme == null || !(scheme.equals("https") || scheme.equals("http"))) return;
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, u).addCategory(Intent.CATEGORY_BROWSABLE));
            } catch (android.content.ActivityNotFoundException ignored) {
                // No browser: nothing opens.
            }
        });
    }

    /** Asks to post notifications (Android 13 and later), once, right after pairing. */
    public void askNotifications() {
        runOnUiThread(() -> {
            if (android.os.Build.VERSION.SDK_INT >= 33
                    && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[] {android.Manifest.permission.POST_NOTIFICATIONS}, 0x5243);
            }
        });
    }

    /**
     * Starts or stops {@link LinkService}; {@code words} is its ongoing notification's line. A
     * running service only has its line changed: starting it again from the background (the link
     * dropped while the app was away) is refused on Android 12 and later.
     */
    public void linkService(final boolean on, final String words) {
        runOnUiThread(() -> {
            Intent i = new Intent(this, LinkService.class).putExtra(LinkService.WORDS, words);
            if (on) {
                if (LinkService.words(this, words)) return;
                try {
                    ContextCompat.startForegroundService(this, i);
                } catch (RuntimeException ignored) {
                    // Started from the background where Android refuses it: the next resume starts it.
                }
            } else {
                stopService(i);
            }
        });
    }

    /** A light tick under the finger: on send, on a long press that copies, on a switch (Rust's Host::haptic). */
    public void haptic() {
        runOnUiThread(() -> getWindow().getDecorView().performHapticFeedback(android.view.HapticFeedbackConstants.CLOCK_TICK));
    }

    /** The system's picker, for one photo or screenshot. */
    public void pickPhoto() {
        runOnUiThread(() -> {
            Intent pick = new Intent(Intent.ACTION_GET_CONTENT).setType("image/*").addCategory(Intent.CATEGORY_OPENABLE);
            try {
                startActivityForResult(pick, PICK_PHOTO);
            } catch (android.content.ActivityNotFoundException ignored) {
                // No picker on this phone: nothing comes back.
            }
        });
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_PHOTO || result != RESULT_OK || data == null || data.getData() == null) return;
        final Uri uri = data.getData();
        // Shrunk off the main thread, then handed to Rust as a file.
        new Thread(() -> {
            try {
                BitmapFactory.Options bounds = new BitmapFactory.Options();
                bounds.inJustDecodeBounds = true;
                try (InputStream in = getContentResolver().openInputStream(uri)) {
                    BitmapFactory.decodeStream(in, null, bounds);
                }
                int sample = 1;
                while (Math.max(bounds.outWidth, bounds.outHeight) / (sample * 2) >= MAX_SIDE) sample *= 2;
                BitmapFactory.Options opts = new BitmapFactory.Options();
                opts.inSampleSize = sample;
                Bitmap bmp;
                try (InputStream in = getContentResolver().openInputStream(uri)) {
                    bmp = BitmapFactory.decodeStream(in, null, opts);
                }
                if (bmp == null) return;
                float scale = Math.min(1f, (float) MAX_SIDE / Math.max(bmp.getWidth(), bmp.getHeight()));
                if (scale < 1f) {
                    bmp = Bitmap.createScaledBitmap(bmp, Math.round(bmp.getWidth() * scale), Math.round(bmp.getHeight() * scale), true);
                }
                File out = new File(getCacheDir(), "pick-" + System.currentTimeMillis() + ".jpg");
                try (FileOutputStream f = new FileOutputStream(out)) {
                    bmp.compress(Bitmap.CompressFormat.JPEG, 88, f);
                }
                nativePhoto(out.getAbsolutePath());
            } catch (Exception ignored) {
                // Unreadable: nothing comes back.
            }
        }).start();
    }
}
