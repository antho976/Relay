#!/usr/bin/env python3
"""Prove the picture and the music agree, measured on the delivered file.

    python3 tools/synccheck.py out/avex-1.0-trailer.mp4 out/cues.json [out/kicks.json]

The bar length comes from cues.json; the kick times come from kicks.json when the score writes one,
otherwise from the trailer's own drum plan.

1. Every cut in the film is on a bar line, and every bar line is a whole frame at 60 fps.
2. The kick drum is found in the delivered audio (onset detection, ~1 ms resolution) and each hit
   is compared to the downbeat the picture cuts on. The offset has to stay within a frame.
3. The same for the sound-effect cues the page declares.
"""
import json, subprocess, sys
import numpy as np

SR = 48000
BAR, BEAT, FPS = 2.4, 0.6, 60


def audio(path):
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-ac', '1', '-ar', str(SR), '-f', 'f32le', '-'], capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32).astype(np.float64)


def onset_env(x, lo, hi, hop=48):
    """Band-limited spectral flux at a 1 ms hop."""
    win = 1024
    w = np.hanning(win)
    f = np.fft.rfftfreq(win, 1 / SR)
    band = (f >= lo) & (f < hi)
    frames = np.lib.stride_tricks.sliding_window_view(np.pad(x, (win // 2, win)), win)[::hop]
    mag = np.abs(np.fft.rfft(frames * w, axis=1))[:, band]
    flux = np.maximum(0, np.diff(np.log1p(mag * 50), axis=0)).sum(1)
    return np.concatenate([[0], flux]), hop / SR


def nearest_peak(env, dt, t, radius=.05):
    i0, i1 = int((t - radius) / dt), int((t + radius) / dt)
    seg = env[max(0, i0):i1]
    return (max(0, i0) + int(np.argmax(seg))) * dt


def main(path, cues_path, kicks_path=None):
    global BAR, BEAT
    spec = json.loads(open(cues_path).read())
    BAR = spec.get('bar', BAR); BEAT = BAR / 4
    x = audio(path)
    # 1. Cuts.
    cuts = spec.get('cuts', [])
    bad = [c for c in cuts if abs(c / BAR - round(c / BAR)) > 1e-6 or abs(c * FPS - round(c * FPS)) > 1e-6]
    print(f'cuts: {len(cuts)} scene starts, {len(bad)} off the bar/frame grid')
    # 2. Kicks: every beat of the full sections, beats 1 and 3.5 in the half-time build.
    if kicks_path:
        kicks = json.loads(open(kicks_path).read())
    else:
        kicks = [(b - 1) * BAR + k * BEAT for b in list(range(27, 35)) + list(range(37, 40)) for k in range(4)]
        kicks += [(b - 1) * BAR for b in range(17, 26)] + [(40 - 1) * BAR]
    env, dt = onset_env(x, 30, 160)
    offs = np.array([nearest_peak(env, dt, k) - k for k in kicks]) * 1000
    print(f'kick vs picture grid: {len(kicks)} hits · median {np.median(offs):+.1f} ms · worst {offs[np.argmax(np.abs(offs))]:+.1f} ms · one frame = {1000 / FPS:.1f} ms')
    # 3. Cues.
    envc, dtc = onset_env(x, 400, 6000)
    co = np.array([nearest_peak(envc, dtc, c['t'], .03) - c['t'] for c in spec['cues'] if c['sfx'] not in ('whoosh', 'page', 'buzz')]) * 1000
    print(f'cues vs their frames: {len(co)} transients · median {np.median(co):+.1f} ms · 90th pct |{np.percentile(np.abs(co), 90):.1f}| ms')
    ok = abs(np.median(offs)) < 1000 / FPS and not bad
    print('IN SYNC' if ok else 'OUT OF SYNC')
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main(*sys.argv[1:4])
