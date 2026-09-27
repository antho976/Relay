#!/usr/bin/env python3
"""The trailer's sound effects, synthesised. Each one is checked against the limits in
remotion-release/RULES.md before it is written, and the script fails if one is out:

  soft cues   < 5% of energy above 4 kHz, ≥ 55% below 1 kHz
  air cues    (whoosh, page) < 10% above 4 kHz, so they read as air and never as hiss
  every cue   peak held no longer than ~120 ms, first sample exactly 0, peak −3 dBFS

    python3 tools/sfx.py audio/sfx
"""
import sys, subprocess
from pathlib import Path
import numpy as np

SR = 48000
rng = np.random.default_rng(7)


def N(s): return int(round(s * SR))
def tt(d): return np.arange(N(d)) / SR


def lp(x, fc, order=2):
    X = np.fft.rfft(x); f = np.fft.rfftfreq(len(x), 1 / SR)
    return np.fft.irfft(X / np.sqrt(1 + (f / fc) ** (2 * order)), len(x))


def hp(x, fc, order=2):
    X = np.fft.rfft(x); f = np.fft.rfftfreq(len(x), 1 / SR)
    return np.fft.irfft(X / np.sqrt(1 + (np.maximum(f, 1e-3) / fc) ** (-2 * order)), len(x))


def attack(x, ms=1.5):
    n = N(ms / 1000)
    x[:n] *= np.linspace(0, 1, n)
    return x


def tap():        # a soft wooden knock: a row or a tile landing
    t = tt(.12)
    f = 700 + 500 * np.exp(-t / .006)
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / .022) + .5 * np.sin(2 * np.pi * 210 * t) * np.exp(-t / .04)
    return attack(x)


def tick():       # the watch crown's detent
    t = tt(.05)
    return attack(np.sin(2 * np.pi * 1500 * t) * np.exp(-t / .006) * .8 + np.sin(2 * np.pi * 380 * t) * np.exp(-t / .01))


def pop():        # a card or a glyph settling
    t = tt(.16)
    f = 260 + 420 * (1 - np.exp(-t / .02))
    return attack(np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / .045))


def confirm():    # a set logged: two soft mallet notes a fifth apart
    t = tt(.7)
    x = np.zeros(len(t))
    for i, f in enumerate([659.3, 987.8]):
        s = t - i * .075
        m = s >= 0
        x[m] += (np.sin(2 * np.pi * f * s[m]) + .25 * np.sin(2 * np.pi * 2 * f * s[m])) * np.exp(-s[m] / .16)
    return attack(lp(x, 2500))


def chime():      # rest over / one thing arriving: a warm three-note bell
    t = tt(1.4)
    x = np.zeros(len(t))
    for i, f in enumerate([587.3, 880.0, 1174.7]):
        s = t - i * .05
        m = s >= 0
        for r, a, d in [(1, 1, .5), (2.0, .3, .25), (3.0, .1, .12)]:
            x[m] += a * np.sin(2 * np.pi * f * r * s[m]) * np.exp(-s[m] / d) / (1 + i * .3)
    return attack(lp(x, 3000), 4)


def buzz():       # a wrist haptic: 40 ms of a low motor
    t = tt(.09)
    env = np.clip(t / .004, 0, 1) * np.clip((.045 - t) / .006, 0, 1)
    x = (np.sin(2 * np.pi * 165 * t) + .3 * np.sin(2 * np.pi * 330 * t)) * env
    return attack(lp(x, 900))


def whoosh():     # a camera push: air moving, peaking at the landing
    d = .55
    t = tt(d)
    x = rng.standard_normal(len(t))
    out = np.zeros(len(t))
    hop, win = 256, 1024
    w = np.hanning(win)
    f = np.fft.rfftfreq(win, 1 / SR)
    norm = np.zeros(len(t))
    for s0 in range(0, len(t) - win, hop):
        k = s0 / len(t)
        fc = 350 + 1500 * np.sin(np.pi * min(1, k * 1.15)) ** 2
        g = np.exp(-((np.log2(np.maximum(f, 1) / fc)) ** 2) / .8)
        out[s0:s0 + win] += np.fft.irfft(np.fft.rfft(x[s0:s0 + win] * w) * g, win) * w
        norm[s0:s0 + win] += w * w
    out /= np.maximum(norm, 1e-6)
    env = np.sin(np.pi * np.clip(t / d, 0, 1)) ** 2.5
    return attack(lp(out * env, 3200), 5)


def page():       # a page turning: a short paper swish
    t = tt(.3)
    x = lp(hp(rng.standard_normal(len(t)), 400), 1900, order=3)
    env = np.exp(-((t - .09) / .05) ** 2) + .3 * np.exp(-((t - .17) / .03) ** 2)
    return attack(x * env, 3)


def stamp():      # a verdict stamped onto a row
    t = tt(.2)
    body = np.sin(2 * np.pi * (120 + 80 * np.exp(-t / .01)) * t) * np.exp(-t / .05)
    slap = lp(rng.standard_normal(len(t)), 1800) * np.exp(-t / .008) * .6
    return attack(body + slap)


def hit():        # the PR: a short, deep punch
    t = tt(.7)
    f = 48 + 90 * np.exp(-t / .04)
    sub = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / .3)
    thud = lp(rng.standard_normal(len(t)), 1200) * np.exp(-t / .03) * .5
    return attack(np.tanh(1.5 * (sub + thud)))


SOFT = {'tap', 'tick', 'pop', 'confirm', 'chime', 'buzz', 'stamp', 'hit'}
AIR = {'whoosh', 'page'}


def check(name, x):
    X = np.abs(np.fft.rfft(x)) ** 2; f = np.fft.rfftfreq(len(x), 1 / SR); tot = X.sum()
    above4k = X[f > 4000].sum() / tot * 100
    below1k = X[f < 1000].sum() / tot * 100
    a = np.abs(x); peak = a.max()
    held = (a > .9 * peak).nonzero()[0]
    hold_ms = (held[-1] - held[0]) / SR * 1000 if len(held) else 0
    ok = x[0] == 0 and hold_ms <= 120 and (name not in SOFT or (above4k < 5 and below1k >= 55)) and (name not in AIR or above4k < 10)
    print(f'  {name:8s} {len(x) / SR * 1000:5.0f} ms  >4k {above4k:4.1f}%  <1k {below1k:5.1f}%  peak held {hold_ms:5.1f} ms  {"ok" if ok else "FAIL"}')
    return ok


def main(out):
    Path(out).mkdir(parents=True, exist_ok=True)
    fails = 0
    for fn in (tap, tick, pop, confirm, chime, buzz, whoosh, page, stamp, hit):
        x = fn().astype(np.float64)
        x[0] = 0.0
        x = x / np.abs(x).max() * 10 ** (-3 / 20)
        x[-N(.004):] *= np.linspace(1, 0, N(.004))
        fails += not check(fn.__name__, x)
        pcm = (x * 32767).astype('<i2')
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 's16le', '-ar', str(SR), '-ac', '1', '-i', '-', f'{out}/{fn.__name__}.wav'], input=pcm.tobytes(), check=True)
    if fails:
        sys.exit(f'{fails} cue(s) out of limits')


if __name__ == '__main__':
    main(sys.argv[1])
