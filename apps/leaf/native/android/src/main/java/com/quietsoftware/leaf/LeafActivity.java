package com.quietsoftware.leaf;

import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.pdf.PdfRenderer;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import androidx.core.content.FileProvider;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.androidgamesdk.GameActivity;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;

/**
 * Leaf's activity: GameActivity, plus what Android gives an app only through Java.
 *
 * The app is Rust (libleaf.so); this class exists because
 * - Back: AndroidX's back dispatcher finishes the activity before the key reaches native code, so
 *   a callback here hands every Back to Rust, which goes back a page or leaves;
 * - the reader hides the status and navigation bars ({@link #setImmersive}), which must happen on
 *   the UI thread through the window's insets controller;
 * - the file picker answers through {@link #onActivityResult}, a callback only an Activity
 *   subclass receives: each picked book is copied into the app's inbox folder here and its path
 *   handed to Rust ({@link #nativeBookPicked}), so no book's bytes cross JNI;
 * - the share sheet needs {@link FileProvider}, an app class JNI's FindClass cannot reach from
 *   Rust's own threads;
 * - PDF pages are drawn by the platform's {@link PdfRenderer} ({@link #pdfRender});
 * - notices of new chapters come from a periodic job ({@link LeafChaptersJob}), scheduled here, and
 *   posting them needs the person's leave on Android 13 and later ({@link #askNotifications});
 * - while reading, the screen stays on ({@link #setReading});
 * - a link opens in the app that takes it ({@link #openUrl}: Webtoon's paid episodes open the
 *   Webtoon app when it is installed);
 * - an app update Rust downloaded and checked is handed to Android's installer
 *   ({@link #installApk}), through the FileProvider, after the person allows Leaf to install apps.
 * Nothing here decides anything: it moves files, bytes and intents.
 */
public class LeafActivity extends GameActivity {
    /** The Back gesture or key, to Rust. */
    private static native void nativeBack();

    /** A picked book, copied to {@code path} in the inbox; {@code name} is what it was called. */
    private static native void nativeBookPicked(String path, String name);

    private static final int PICK_BOOKS = 0x4c45;
    /** The largest book copied in. */
    private static final long MAX_BYTES = 400L * 1024 * 1024;

    @Override
    protected void onCreate(android.os.Bundle saved) {
        super.onCreate(saved);
        // Edge to edge on every version: the surface is the whole screen, and the app keeps clear
        // of the bars and the keyboard itself (android.rs reads their insets).
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        WindowInsetsControllerCompat c = WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        // Light icons: the app is dark.
        c.setAppearanceLightStatusBars(false);
        c.setAppearanceLightNavigationBars(false);
        LeafChaptersJob.schedule(this);
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                nativeBack();
            }
        });
    }

    /** Reading: the bars are to stay hidden, and are hidden again whenever the window comes back. */
    private volatile boolean immersive;

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        // The shade, a dialog or the app switcher brings the bars back; reading, they go again.
        if (hasFocus && immersive) setImmersive(true);
    }

    /** While reading, the bars hide; a swipe from the edge shows them for a moment. */
    public void setImmersive(final boolean on) {
        immersive = on;
        runOnUiThread(() -> {
            WindowInsetsControllerCompat c = WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
            if (on) {
                c.setSystemBarsBehavior(WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                c.hide(WindowInsetsCompat.Type.systemBars());
            } else {
                c.show(WindowInsetsCompat.Type.systemBars());
            }
        });
    }

    /** A light tick (a row caught by a drag that selects), as the system's touch feedback allows. */
    public void tick() {
        runOnUiThread(() -> getWindow().getDecorView().performHapticFeedback(android.view.HapticFeedbackConstants.CLOCK_TICK));
    }

    /** On a network that isn't metered (Wi-Fi, as people mean it): "Download Next up" may run. */
    public boolean unmetered() {
        ConnectivityManager cm = (ConnectivityManager) getSystemService(CONNECTIVITY_SERVICE);
        if (cm == null) return false;
        Network n = cm.getActiveNetwork();
        if (n == null) return false;
        NetworkCapabilities c = cm.getNetworkCapabilities(n);
        return c != null && c.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED);
    }

    /** A book or a strip is open: the screen stays on. */
    public void setReading(final boolean on) {
        runOnUiThread(() -> {
            if (on) {
                getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            } else {
                getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            }
        });
    }

    // Notices

    /** Asks to post notifications (Android 13 and later), once a bell is first turned on. */
    public void askNotifications() {
        runOnUiThread(() -> {
            if (android.os.Build.VERSION.SDK_INT >= 33
                    && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[] {android.Manifest.permission.POST_NOTIFICATIONS}, 0x4c46);
            }
        });
    }

    // Books from the phone

    /** The system's picker, for one or many EPUB, PDF and text files. */
    public void pickBooks() {
        runOnUiThread(() -> {
            Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            pick.addCategory(Intent.CATEGORY_OPENABLE);
            pick.setType("*/*");
            // Some file managers call an EPUB application/octet-stream.
            pick.putExtra(Intent.EXTRA_MIME_TYPES, new String[] {"application/epub+zip", "application/pdf", "text/plain", "application/octet-stream"});
            pick.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            try {
                startActivityForResult(pick, PICK_BOOKS);
            } catch (ActivityNotFoundException ignored) {
                // No picker on this phone: nothing comes back.
            }
        });
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_BOOKS || result != RESULT_OK || data == null) {
            return;
        }
        final List<Uri> uris = new ArrayList<>();
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int i = 0; i < clip.getItemCount(); i++) {
                Uri u = clip.getItemAt(i).getUri();
                if (u != null) uris.add(u);
            }
        } else if (data.getData() != null) {
            uris.add(data.getData());
        }
        if (uris.isEmpty()) return;
        // Copied off the main thread; Rust brings each in as it lands.
        new Thread(() -> {
            File inbox = new File(getFilesDir(), "inbox");
            inbox.mkdirs();
            long stamp = System.currentTimeMillis();
            for (int i = 0; i < uris.size(); i++) {
                Uri uri = uris.get(i);
                String name = nameOf(uri);
                String safe = name.replaceAll("[^A-Za-z0-9._ -]", "_");
                File part = new File(inbox, stamp + "" + i + "-" + safe + ".part");
                File done = new File(inbox, stamp + "" + i + "-" + safe);
                if (copy(uri, part) && part.renameTo(done)) {
                    nativeBookPicked(done.getAbsolutePath(), name);
                } else {
                    part.delete();
                    // Rust says plainly that it could not be read: an empty file stands for it.
                    try {
                        new FileOutputStream(done).close();
                        nativeBookPicked(done.getAbsolutePath(), name);
                    } catch (Exception ignored) {
                        // Nothing to tell it with.
                    }
                }
            }
        }, "leaf-pick").start();
    }

    private String nameOf(Uri uri) {
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                String name = c.getString(0);
                if (name != null) return name;
            }
        } catch (Exception ignored) {
            // Only shown; the bytes are what matter.
        }
        return "book";
    }

    /** Streams the file to {@code to}, at most MAX_BYTES. False when it could not be read. */
    private boolean copy(Uri uri, File to) {
        try (InputStream in = getContentResolver().openInputStream(uri); OutputStream out = new FileOutputStream(to)) {
            if (in == null) return false;
            byte[] buf = new byte[64 * 1024];
            long total = 0;
            int n;
            while ((n = in.read(buf)) > 0) {
                total += n;
                if (total > MAX_BYTES) return false;
                out.write(buf, 0, n);
            }
            return true;
        } catch (Throwable e) {
            return false;
        }
    }

    // The clipboard and the share sheet

    public void copyText(final String text) {
        runOnUiThread(() -> {
            ClipboardManager cm = getSystemService(ClipboardManager.class);
            if (cm != null) cm.setPrimaryClip(ClipData.newPlainText("Leaf", text));
        });
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

    // Links and app updates

    /** Opens {@code url} in the app that takes it. False when none does. */
    public boolean openUrl(final String url) {
        Intent view = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
        view.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        // The manifest's <queries> lets this see the apps that open web links (Android 11+).
        if (view.resolveActivity(getPackageManager()) == null) {
            return false;
        }
        runOnUiThread(() -> {
            try {
                startActivity(view);
            } catch (ActivityNotFoundException ignored) {
                // Gone between the check and now.
            }
        });
        return true;
    }

    /**
     * Hands the APK Rust downloaded (under the cache's {@code updates} folder, checked against its
     * SHA-256) to Android's installer: 1 when it opened, 0 when Leaf may not install apps yet (the
     * "Install unknown apps" page for Leaf opens instead), -1 when it failed.
     */
    public int installApk(final String path) {
        if (android.os.Build.VERSION.SDK_INT >= 26 && !getPackageManager().canRequestPackageInstalls()) {
            runOnUiThread(() -> {
                Intent allow = new Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + getPackageName()));
                try {
                    startActivity(allow);
                } catch (ActivityNotFoundException ignored) {
                    // No such page on this phone: the sheet still says what to allow.
                }
            });
            return 0;
        }
        try {
            File file = new File(path);
            Uri uri = FileProvider.getUriForFile(this, getPackageName() + ".files", file);
            Intent install = new Intent(Intent.ACTION_VIEW);
            install.setDataAndType(uri, "application/vnd.android.package-archive");
            install.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
            runOnUiThread(() -> {
                try {
                    startActivity(install);
                } catch (ActivityNotFoundException ignored) {
                    // No installer: nothing opens.
                }
            });
            return 1;
        } catch (Throwable e) {
            return -1;
        }
    }

    // PDF pages, drawn by the platform

    private String pdfPath;
    private ParcelFileDescriptor pdfFd;
    private PdfRenderer pdf;

    /** The renderer for {@code path}, kept open for the next page. Call holding the lock. */
    private PdfRenderer open(String path) throws java.io.IOException {
        if (pdf != null && path.equals(pdfPath)) return pdf;
        closePdf();
        pdfFd = ParcelFileDescriptor.open(new File(path), ParcelFileDescriptor.MODE_READ_ONLY);
        pdf = new PdfRenderer(pdfFd);
        pdfPath = path;
        return pdf;
    }

    private void closePdf() {
        try {
            if (pdf != null) pdf.close();
            if (pdfFd != null) pdfFd.close();
        } catch (Exception ignored) {
            // Closing what is already gone.
        }
        pdf = null;
        pdfFd = null;
        pdfPath = null;
    }

    /** A PDF's page count, or -1 when it can't be opened (a locked PDF). */
    public synchronized int pdfPageCount(String path) {
        try {
            return open(path).getPageCount();
        } catch (Throwable e) {
            closePdf();
            return -1;
        }
    }

    /**
     * Page {@code n} drawn {@code width} pixels wide on white: 4 bytes of width, 4 of height (big
     * endian), then RGBA rows. Null when it can't be drawn.
     */
    public synchronized byte[] pdfRender(String path, int n, int width) {
        try {
            PdfRenderer r = open(path);
            if (n < 0 || n >= r.getPageCount()) return null;
            try (PdfRenderer.Page page = r.openPage(n)) {
                int w = Math.max(1, width);
                int h = Math.max(1, (int) ((long) page.getHeight() * w / Math.max(1, page.getWidth())));
                // A very long page is kept within what a texture holds: drawn narrower, so it
                // keeps its shape instead of being squashed into the height.
                if (h > 8000) {
                    w = Math.max(1, (int) ((long) page.getWidth() * 8000 / Math.max(1, page.getHeight())));
                    h = 8000;
                }
                Bitmap bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
                bmp.eraseColor(Color.WHITE);
                page.render(bmp, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
                ByteBuffer out = ByteBuffer.allocate(8 + w * h * 4);
                out.putInt(w).putInt(h);
                // ARGB_8888 is laid out R, G, B, A in memory.
                bmp.copyPixelsToBuffer(out);
                bmp.recycle();
                return out.array();
            }
        } catch (Throwable e) {
            closePdf();
            return null;
        }
    }

    @Override
    protected void onDestroy() {
        synchronized (this) {
            closePdf();
        }
        super.onDestroy();
    }
}
