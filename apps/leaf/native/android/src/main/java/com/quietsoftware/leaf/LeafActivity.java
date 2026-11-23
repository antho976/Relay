package com.quietsoftware.leaf;

import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.androidgamesdk.GameActivity;

/**
 * Leaf's activity: GameActivity, plus what Android gives an app only through Java.
 *
 * The app is Rust (libleaf.so); this class exists because
 * - Back: AndroidX's back dispatcher finishes the activity before the key reaches native code, so
 *   a callback here hands every Back to Rust, which goes back a page or leaves;
 * - the reader hides the status and navigation bars ({@link #setImmersive}), which must happen on
 *   the UI thread through the window's insets controller.
 * Nothing here decides anything.
 */
public class LeafActivity extends GameActivity {
    /** The Back gesture or key, to Rust. */
    private static native void nativeBack();

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
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                nativeBack();
            }
        });
    }

    /** While reading, the bars hide; a swipe from the edge shows them for a moment. */
    public void setImmersive(final boolean on) {
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
}
