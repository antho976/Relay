package dev.relay.latprobe;

import android.app.Activity;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Latency probe. Left third: 24 stripes encoding the CLOCK_MONOTONIC time (units of 0.1 ms,
 * low 24 bits, ~28 min wrap) at which this frame was drawn. Right two thirds: a field that flips
 * black/white on every ACTION_DOWN (touch-to-photon). Mode "anim": redraw every vsync. Mode
 * "still": draw only on touch. A LocalServerSocket "latprobe" answers each byte with 8 bytes of
 * System.nanoTime() for host clock sync; byte 'a'/'s' switches anim/still, 'h'/'l' heavy/light
 * (1500 random squares a frame, for an encoder under load), 'c'/'n' the color chart on/off.
 *
 * The color chart: CHART below, row-major in a 4x4 grid over the lower part of the touch field
 * (x from 1/3 to all of the width, y from 0.60 to 0.96 of the height), on a static frame.
 * check-colors.py reads it back from a screenshot of the mirror.
 */
public class Probe extends Activity {
    static volatile boolean anim = true;
    static volatile boolean heavy = false;
    static volatile boolean chart = false;
    static final int[] CHART = {
        0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffffff,
        0xff00ffff, 0xffff00ff, 0xffffff00, 0xff000000,
        0xff404040, 0xff808080, 0xffc0c0c0, 0xffe0ac69,
        0xffff8000, 0xff203060, 0xff8a2be2, 0xff2e8b57,
    };
    View view;

    @Override protected void onCreate(Bundle b) {
        super.onCreate(b);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        view = new Field(this);
        setContentView(view);
        view.setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
            | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        Thread t = new Thread(this::serve);
        t.setDaemon(true);
        t.start();
    }

    void serve() {
        try {
            LocalServerSocket server = new LocalServerSocket("latprobe");
            while (true) {
                LocalSocket s = server.accept();
                InputStream in = s.getInputStream();
                OutputStream out = s.getOutputStream();
                int c;
                while ((c = in.read()) >= 0) {
                    long now = System.nanoTime();
                    if (c == 'a') { anim = true; view.postInvalidate(); }
                    if (c == 's') { anim = false; }
                    if (c == 'h') { heavy = true; }
                    if (c == 'l') { heavy = false; }
                    if (c == 'c') { chart = true; view.postInvalidate(); }
                    if (c == 'n') { chart = false; view.postInvalidate(); }
                    byte[] r = new byte[8];
                    for (int i = 0; i < 8; i++) r[i] = (byte) (now >>> (56 - 8 * i));
                    out.write(r);
                    out.flush();
                }
                s.close();
            }
        } catch (Exception e) { }
    }

    static class Field extends View {
        final Paint p = new Paint();
        boolean white = false;
        Field(Context c) { super(c); }

        @Override public boolean onTouchEvent(MotionEvent e) {
            if (e.getActionMasked() == MotionEvent.ACTION_DOWN) {
                white = !white;
                invalidate();
            }
            return true;
        }

        @Override protected void onDraw(Canvas c) {
            long t = System.nanoTime() / 100_000L; // 0.1 ms
            int w = getWidth(), h = getHeight();
            int bw = w / 3;
            int n = 24;
            float sh = h / (float) (n + 2);
            p.setColor(Color.rgb(255, 0, 255));
            c.drawRect(0, 0, bw, sh, p); // marker
            for (int i = 0; i < n; i++) {
                boolean bit = ((t >>> (n - 1 - i)) & 1) != 0;
                p.setColor(bit ? Color.WHITE : Color.BLACK);
                c.drawRect(0, sh * (i + 1), bw, sh * (i + 2), p);
            }
            p.setColor(Color.rgb(255, 0, 255));
            c.drawRect(0, sh * (n + 1), bw, h, p);
            p.setColor(white ? Color.WHITE : Color.BLACK);
            c.drawRect(bw, 0, w, h, p);
            if (heavy) {
                long seed = t * 2654435761L;
                int y0 = (int) (h * 0.62f);
                for (int i = 0; i < 1500; i++) {
                    seed = seed * 6364136223846793005L + 1442695040888963407L;
                    int x = bw + (int) ((seed >>> 33) % (w - bw - 24));
                    int y = y0 + (int) ((seed >>> 17) % (h - y0 - 24));
                    p.setColor(0xff000000 | (int) (seed >>> 40));
                    c.drawRect(x, y, x + 24, y + 24, p);
                }
            }
            if (chart) {
                float cw = (w - bw) / 4f, ch = h * 0.36f / 4f, top = h * 0.60f;
                for (int i = 0; i < CHART.length; i++) {
                    p.setColor(CHART[i]);
                    c.drawRect(bw + cw * (i % 4), top + ch * (i / 4), bw + cw * (i % 4 + 1), top + ch * (i / 4 + 1), p);
                }
            }
            if (anim) postInvalidateOnAnimation();
        }
    }
}
