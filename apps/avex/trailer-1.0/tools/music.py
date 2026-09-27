#!/usr/bin/env python3
"""The Avex 1.0 trailer score, synthesised from nothing but numpy.

    python3 tools/music.py out/score.wav

100 BPM, so a beat is 0.6 s (36 frames at 60 fps) and a bar is 2.4 s. D minor, i–VI–III–VII
(Dm Bb F C). The arrangement follows the film bar for bar:

   1–4   open          drone, then pad and the bell theme
   5–7   the plan      pulse bass and arp come in
   8–25  the week      the build: toms from 12, a fill into the PR on 14, half-time drums from 17
  26     the break     one held A chord and a riser
  27–34  the drop      braam, impact, full kit, choir and the lead (second braam on 31)
  35–36  offline/free  a breath: Gm, A and the bell theme, a riser into
  37–40  the finale    the hit the logo draws on, then Bb, C and D major on bar 40
  41–45  testers       a warm outro under the call for testers: G D Bm A D

Nobody in this loop can hear it, so the script prints measurements (per-stem level, band balance,
per-bar level) and the mix is judged on those.
"""
import sys, subprocess
import numpy as np

SR = 48000
BPM = 100
BEAT = 60 / BPM
BAR = 4 * BEAT
NBARS = 45
LEN = int(round(NBARS * BAR * SR))
rng = np.random.default_rng(1001)


def T(bar, beat=0.0):
    return (bar - 1) * BAR + beat * BEAT


def N(sec):
    return int(round(sec * SR))


NOTE = {'C': 0, 'C#': 1, 'Db': 1, 'D': 2, 'D#': 3, 'Eb': 3, 'E': 4, 'F': 5, 'F#': 6, 'Gb': 6, 'G': 7,
        'G#': 8, 'Ab': 8, 'A': 9, 'A#': 10, 'Bb': 10, 'B': 11}


def midi(name):
    i = 2 if len(name) > 2 and name[1] in '#b' else 1
    return 12 * (int(name[i:]) + 1) + NOTE[name[:i]]


def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# ── Harmony ─────────────────────────────────────────────────────────────────────────────────────
CHORDS = {
    'Dm': ['D3', 'A3', 'D4', 'F4', 'A4'], 'Bb': ['Bb2', 'F3', 'Bb3', 'D4', 'F4'],
    'F': ['F2', 'C3', 'F3', 'A3', 'C4'], 'C': ['C3', 'G3', 'C4', 'E4', 'G4'],
    'Gm': ['G2', 'D3', 'G3', 'Bb3', 'D4'], 'A': ['A2', 'E3', 'A3', 'C#4', 'E4'],
    'D': ['D3', 'A3', 'D4', 'F#4', 'A4'], 'G': ['G2', 'D3', 'G3', 'B3', 'D4'], 'Bm': ['B2', 'F#3', 'B3', 'D4', 'F#4'],
}
ROOT = {'Dm': 'D2', 'Bb': 'Bb1', 'F': 'F2', 'C': 'C2', 'Gm': 'G1', 'A': 'A1', 'D': 'D2', 'G': 'G1', 'Bm': 'B1'}
CYCLE = ['Dm', 'Bb', 'F', 'C']
OVERRIDE = {1: 'Dm', 2: 'Dm', 3: 'Bb', 4: 'C', 26: 'A', 35: 'Gm', 36: 'A', 37: 'Dm', 38: 'Bb', 39: 'C', 40: 'D',
            41: 'G', 42: 'D', 43: 'Bm', 44: 'A', 45: 'D'}


def chord_at(bar):
    if bar in OVERRIDE:
        return OVERRIDE[bar]
    return CYCLE[(bar - 5) % 4] if bar < 27 else CYCLE[(bar - 27) % 4]


WEEK = range(8, 26)
DROP = range(27, 35)
FINALE = range(37, 40)
OUTRO = range(41, 45)


def energy(bar):
    if bar <= 4: return .15
    if bar <= 7: return .3
    if bar <= 25: return .35 + .45 * (bar - 8) / 17
    if bar == 26: return .2
    if bar <= 34: return 1.0
    if bar <= 36: return .55
    if bar <= 40: return 1.0
    return .35


# ── Primitives ──────────────────────────────────────────────────────────────────────────────────
def lp_gain(f, fc, order=2):
    return 1.0 / np.sqrt(1.0 + (f / fc) ** (2 * order))


def saw(freq, dur, cutoff, order=2, phase_seed=None, max_h=90, pulse=False):
    """Band-limited saw by additive synthesis; `cutoff` is a scalar or a per-sample array."""
    n = N(dur)
    t = np.arange(n) / SR
    out = np.zeros(n)
    r = np.random.default_rng(None if phase_seed is None else abs(int(phase_seed)))
    K = int(min(max_h, 17000 / freq))
    for k in range(1, K + 1):
        if pulse and k % 2 == 0:
            continue
        fk = freq * k
        g = (1.0 / k) * lp_gain(fk, cutoff, order)
        if np.max(g) < 2e-4:
            break
        out += g * np.sin(2 * np.pi * fk * t + r.uniform(0, 2 * np.pi))
    return out


def adsr(n, a, d, s, r):
    a, d, r = N(a), N(d), N(r)
    env = np.full(n, s, float)
    if a:
        env[:a] = np.linspace(0, 1, a, endpoint=False)
    env[a:a + d] = np.linspace(1, s, max(1, len(env[a:a + d])))
    if r:
        env[-r:] *= np.linspace(1, 0, r) ** 1.5
    return env


def noise(n):
    return rng.standard_normal(n)


def band(x, lo=None, hi=None, order=2):
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    g = np.ones_like(f)
    if hi: g *= lp_gain(f, hi, order)
    if lo: g *= 1 / np.sqrt(1 + (np.maximum(f, 1e-3) / lo) ** (-2 * order))
    return np.fft.irfft(X * g, len(x))


def sweep_noise(dur, lo0, lo1, hi0, hi1, curve=2.0):
    n = N(dur)
    x = noise(n + 2048)
    hop, win = 512, 2048
    w = np.hanning(win)
    out = np.zeros(len(x)); norm = np.zeros(len(x))
    f = np.fft.rfftfreq(win, 1 / SR)
    for s in range(0, len(x) - win, hop):
        k = min(1.0, s / n) ** curve
        lo = lo0 * (lo1 / lo0) ** k
        hi = hi0 * (hi1 / hi0) ** k
        g = lp_gain(f, hi) / np.sqrt(1 + (np.maximum(f, 1e-3) / lo) ** -4)
        out[s:s + win] += np.fft.irfft(np.fft.rfft(x[s:s + win] * w) * g, win) * w
        norm[s:s + win] += w * w
    return (out / np.maximum(norm, 1e-6))[:n]


def pan2(x, p=0.0):
    a = (p + 1) * np.pi / 4
    return np.stack([x * np.cos(a), x * np.sin(a)], 1)


class Stem:
    def __init__(self, name, send=0.0):
        self.name, self.send = name, send
        self.buf = np.zeros((LEN, 2))

    def add(self, x, t, gain=1.0, pan=0.0):
        if x.ndim == 1:
            x = pan2(x, pan)
        i = N(t)
        j = min(LEN, i + len(x))
        if j > i:
            self.buf[i:j] += x[:j - i] * gain


# ── Instruments ─────────────────────────────────────────────────────────────────────────────────
def kick(big=False):
    d = .9 if big else .55
    n = N(d); t = np.arange(n) / SR
    f = 42 + 110 * np.exp(-t / .03) + 25 * np.exp(-t / .18)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / (.42 if big else .26))
    click = band(noise(n), lo=1500, hi=5000) * np.exp(-t / .004) * .25
    return np.tanh(1.6 * (body + click)) * .9


def snare():
    n = N(1.0); t = np.arange(n) / SR
    tone = np.sin(2 * np.pi * 185 * t + 3 * np.exp(-t / .01)) * np.exp(-t / .07) * .6
    nz = band(noise(n), lo=900, hi=6000) * np.exp(-t / .12) * .7
    return tone + nz


def tom(pitch=70):
    n = N(1.4); t = np.arange(n) / SR
    f = pitch * (1 + .5 * np.exp(-t / .05))
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / .45)
    thump = band(noise(n), hi=700) * np.exp(-t / .05) * .5
    return np.tanh(1.3 * (body + thump))


def hat():
    """Closed only. The open hat's long 'tsss' is gone on purpose."""
    n = N(.1); t = np.arange(n) / SR
    return band(noise(n), lo=6500, hi=11000) * np.exp(-t / .025)


def crash():
    """A soft, darker cymbal: shorter tail and less top than before, so it marks the hit and gets out."""
    n = N(1.6); t = np.arange(n) / SR
    x = band(noise(n), lo=2500, hi=8000) * np.exp(-t / .45)
    return x * (1 - np.exp(-t / .004))


def bell(m, dur=3.0):
    n = N(dur); t = np.arange(n) / SR
    f = hz(m)
    out = np.zeros(n)
    for ratio, amp, dec in [(1, 1, 1.6), (2.0, .45, .9), (3.01, .25, .5), (4.2, .12, .3), (5.43, .06, .2)]:
        out += amp * np.sin(2 * np.pi * f * ratio * t) * np.exp(-t / dec)
    return out * (1 - np.exp(-t / .004))


def braam(root_m, dur=3.2):
    n = N(dur); t = np.arange(n) / SR
    cut = 180 + 2400 * np.exp(-t / .5) * (1 - np.exp(-t / .03))
    out = np.zeros(n)
    for i, m in enumerate([root_m, root_m + 12, root_m + 19, root_m + 24]):
        for dt in (-9, 0, 9):
            out += saw(hz(m) * 2 ** (dt / 1200), dur, cut, order=2, phase_seed=i * 10 + dt + 50, max_h=60) * (1.0 if i < 2 else .6)
    return np.tanh(1.8 * out * adsr(n, .01, .4, .7, 1.2) * .35)


def impact(dur=4.0):
    n = N(dur); t = np.arange(n) / SR
    f = 28 + 60 * np.exp(-t / .25)
    sub = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 1.4)
    hit = band(noise(n), hi=2500) * np.exp(-t / .12) * .6
    return np.tanh(1.4 * (sub + hit))


def riser(dur):
    x = sweep_noise(dur, 300, 3000, 1200, 9000, curve=1.6)
    t = np.arange(len(x)) / SR
    tone = sum(np.sin(2 * np.pi * np.cumsum(hz(62 + o) * 2 ** (2 * t / dur)) / SR) for o in (0, 7, 12)) * .12
    return (x * .7 + tone) * (t / dur) ** 2.2


def reverse_swell(dur):
    x = band(noise(N(dur)), lo=2000, hi=9000)
    t = np.arange(len(x)) / SR
    return x * (t / dur) ** 3


def choir(ch, dur):
    tt = np.arange(N(dur)) / SR
    out = np.zeros(len(tt))
    for i, nm in enumerate(CHORDS[ch][1:]):
        f0 = hz(midi(nm))
        vib = 1 + .004 * np.sin(2 * np.pi * 5.2 * tt + i)
        for k in range(1, int(4200 / f0)):
            fk = f0 * k
            g = sum(a / np.sqrt(1 + ((fk - F) / bw) ** 2) for F, bw, a in [(750, 90, 1), (1150, 110, .6), (2600, 160, .25)]) / k ** .3
            out += g * np.sin(2 * np.pi * fk * np.cumsum(vib) / SR + i * k)
    return out


# ── Arrangement ─────────────────────────────────────────────────────────────────────────────────
def build():
    S = {k: Stem(k, s) for k, s in [
        ('drone', .3), ('pad', .45), ('choir', .5), ('bell', .55), ('bass', .05), ('arp', .35), ('lead', .4),
        ('kick', .08), ('snare', .45), ('toms', .3), ('hats', .12), ('cym', .3), ('fx', .4), ('braam', .3)]}
    kicks = []

    n = N(4 * BAR + 2); t = np.arange(n) / SR
    dr = saw(hz(midi('D2')), n / SR, 300, max_h=20, phase_seed=1) + .6 * np.sin(2 * np.pi * hz(midi('D1')) * t)
    S['drone'].add(dr * np.clip(t / (1.2 * BAR), 0, 1) ** 2 * np.clip((4 * BAR + 2 - t) / 2, 0, 1), 0, .07)

    for bar in range(2, NBARS + 1):
        if bar == 26:
            continue
        ch, e = chord_at(bar), energy(bar)
        dur = BAR + (1.5 if bar == NBARS else .6)
        tt = np.arange(N(dur)) / SR
        cutoff = (700 + 2600 * e) * (1 + .15 * np.sin(2 * np.pi * tt / BAR))
        x = np.zeros(N(dur))
        for i, nm in enumerate(CHORDS[ch]):
            for dt in (-12, -4, 5, 13):
                x += saw(hz(midi(nm)) * 2 ** (dt / 1200), dur, cutoff, phase_seed=bar * 100 + i * 10 + dt, max_h=40)
        env = adsr(len(x), .35 if bar < 27 or bar >= 41 else .05, .2, .9, .6 if bar < NBARS else 1.4)
        lvl = (.008 + .026 * e) * (.6 if bar <= 4 else 1) * (1.25 if bar in (37, 38, 39, 40) else 1) * (1.15 if bar >= 41 else 1)
        S['pad'].add(x * env, T(bar) - (.02 if bar in DROP or bar in FINALE else 0), lvl)

    x = sum(saw(hz(midi(nm)), BAR, 900, phase_seed=900 + i) for i, nm in enumerate(CHORDS['A']))
    S['pad'].add(x * adsr(len(x), .2, .2, .8, .3), T(26), .03)

    for bar in list(DROP) + list(FINALE) + [40]:
        x = choir(chord_at(bar), BAR + .5)
        S['choir'].add(x * adsr(len(x), .25, .2, .9, .5), T(bar), .045)

    MOTIF = [(2, 0, 'D5'), (2, 1, 'F5'), (2, 2, 'A5'), (3, 0, 'F5'), (3, 1, 'D5'), (3, 2, 'Bb4'),
             (4, 0, 'C5'), (4, 1, 'E5'), (4, 2, 'G5'), (4, 3, 'E5'),
             (35, 0, 'D5'), (35, 1, 'G5'), (35, 2, 'Bb5'), (36, 0, 'A5'), (36, 1, 'E5'), (36, 2, 'C#5'),
             (38, 0, 'D5'), (38, 1, 'F5'), (38, 2, 'Bb5'), (39, 0, 'C6'), (39, 1, 'G5'), (39, 2, 'E5'),
             (40, 0, 'F#5'), (40, 2, 'A5'), (40, 0, 'D6'),
             (41, 0, 'D5'), (41, 1, 'G5'), (41, 2, 'B5'), (42, 0, 'A5'), (42, 2, 'F#5'),
             (43, 0, 'F#5'), (43, 1, 'D5'), (43, 2, 'B4'), (44, 0, 'E5'), (44, 1, 'C#5'), (44, 2, 'A4'), (44, 3, 'E5'),
             (45, 0, 'D5'), (45, 0, 'A5'), (45, 2, 'F#5')]
    for bar, beat, nm in MOTIF:
        S['bell'].add(bell(midi(nm), 3.5), T(bar, beat), .14 if bar < 41 else .11, pan=.25 if beat % 2 else -.25)

    bass_bars = list(range(5, 26)) + list(DROP) + list(FINALE) + list(OUTRO)
    for bar in bass_bars:
        root, e = midi(ROOT[chord_at(bar)]), energy(bar)
        for i in range(8):
            d = BEAT / 2
            m = root + (12 if (i % 4 == 3 and (bar in DROP or bar in FINALE)) else 0)
            tt = np.arange(N(d)) / SR
            cut = (180 + 900 * e) * (1 + 2.5 * np.exp(-tt / .05))
            x = saw(hz(m), d, cut, max_h=50, phase_seed=i) + .7 * np.sin(2 * np.pi * hz(m) * tt)
            x *= adsr(len(x), .004, .08, .7, .05)
            S['bass'].add(x, T(bar, i / 2), (.16 + .1 * e) * .7 * (.7 if bar in OUTRO else 1))

    for bar in list(range(5, 26)) + list(DROP) + list(FINALE) + list(OUTRO):
        ch, e = CHORDS[chord_at(bar)], energy(bar)
        pat = [2, 3, 4, 3, 2, 3, 4, 1]
        for i in range(16):
            m = midi(ch[pat[i % 8]]) + 12
            d = BEAT / 4
            tt = np.arange(N(d * 1.8)) / SR
            cut = (500 + 3500 * e) * (1 + 1.5 * np.exp(-tt / .03))
            x = saw(hz(m), d * 1.8, cut, max_h=40, phase_seed=i, pulse=True) * np.exp(-tt / (.07 + .05 * e))
            S['arp'].add(x, T(bar, i / 4), .09 + .11 * e, pan=.35 * np.sin(i * 1.3))

    LEAD = [['D5', 'F5', 'A5', 'A5'], ['Bb5', 'A5', 'F5', 'D5'], ['C6', 'A5', 'F5', 'A5'], ['G5', 'E5', 'C5', 'E5'],
            ['D5', 'F5', 'A5', 'D6'], ['D6', 'C6', 'Bb5', 'F5'], ['A5', 'C6', 'F6', 'C6'], ['E6', 'D6', 'C6', 'G5']]
    for bar, notes in zip(DROP, LEAD):
        for i, nm in enumerate(notes):
            d = BEAT * .95
            tt = np.arange(N(d)) / SR
            f = hz(midi(nm))
            x = sum(saw(f * 2 ** (c / 1200), d, 3200 * (1 + np.exp(-tt / .08)), max_h=40, phase_seed=i + c) for c in (-7, 7))
            S['lead'].add(x * adsr(len(x), .01, .1, .75, .12), T(bar, i), .11)

    def K(t, big=False, g=1.0):
        S['kick'].add(kick(big), t, g * .68); kicks.append(t)
    for bar in range(17, 26):                              # half-time build
        K(T(bar, 0), g=.8); K(T(bar, 2.5), g=.55)
        S['snare'].add(snare(), T(bar, 2), .8)
    for bar in list(DROP) + list(FINALE):                  # full
        for b in range(4):
            K(T(bar, b), big=(b == 0))
        S['snare'].add(snare(), T(bar, 1), .95); S['snare'].add(snare(), T(bar, 3), .95)
    for bar in OUTRO:                                      # a soft heartbeat under the call for testers
        K(T(bar, 0), g=.5); K(T(bar, 2), g=.35)
    for bar in range(12, 26):
        for b, p, g in [(0, 70, .5), (1.5, 82, .3), (2, 64, .45), (3.5, 90, .25)]:
            if bar < 17 and b in (1.5, 3.5):
                continue
            S['toms'].add(tom(p), T(bar, b), g * (.6 + .4 * energy(bar)), pan=-.3 if p > 75 else .2)
    for bar in (13, 18, 22, 34):                           # fills: into the PR, and into each phrase
        for i, p in enumerate([110, 98, 88, 78, 70, 62]):
            S['toms'].add(tom(p), T(bar, 2.5 + i * .25), .35, pan=-.5 + i * .2)
    for bar in range(21, 26):
        for i in range(8):
            S['hats'].add(hat(), T(bar, i / 2), .12 if i % 2 else .16)
    for bar in list(DROP) + list(FINALE):
        for i in range(16):
            S['hats'].add(hat(), T(bar, i / 4), .15 if i % 2 == 0 else .07, pan=.2)
    for bar in OUTRO:
        for i in range(8):
            S['hats'].add(hat(), T(bar, i / 2), .06 if i % 2 else .09, pan=-.2)
    S['cym'].add(crash(), T(27), .16); S['cym'].add(crash(), T(37), .16)
    S['cym'].add(crash(), T(14), .08); S['cym'].add(crash(), T(31), .08)
    for bar in (25, 36):                                   # snare roll into the break, and into the finale
        for i in range(16):
            S['snare'].add(snare(), T(bar, 2 + i / 8), .15 + .6 * (i / 15) ** 2)

    S['braam'].add(braam(midi('D1')), T(27), .5)
    S['braam'].add(braam(midi('D1'), 4.5), T(37), .55)
    S['braam'].add(braam(midi('Bb0'), 2.4), T(31), .28)
    S['fx'].add(impact(), T(27), .55); S['fx'].add(impact(), T(37), .6); S['fx'].add(impact(5.0), T(40), .5)
    S['fx'].add(riser(BAR * .96), T(26), .3)
    S['fx'].add(riser(BAR * 2 - .1), T(35), .18)
    S['fx'].add(reverse_swell(BAR), T(30), .1)
    S['fx'].add(reverse_swell(BAR * .75), T(3, 1), .04)
    S['fx'].add(impact(3.0), T(2), .1)
    K(T(40), big=True, g=1.0)
    return S, kicks


def reverb_ir(sec=3.4, pre=.025):
    n = N(sec); t = np.arange(n) / SR
    chans = []
    for _ in range(2):
        x = noise(n)
        lo = band(x, hi=2500) * np.exp(-6.9 * t / sec)
        hi = (x - band(x, hi=2500)) * np.exp(-6.9 * t / (sec * .4))
        chans.append(np.concatenate([np.zeros(N(pre)), (lo + hi) * (1 - np.exp(-t / .01))]))
    ir = np.stack(chans, 1)
    return ir / np.sqrt((ir ** 2).sum() / 2)


def convolve(x, ir):
    n = len(x) + len(ir) - 1
    size = 1 << (n - 1).bit_length()
    out = np.zeros((len(x), 2))
    for c in range(2):
        out[:, c] = np.fft.irfft(np.fft.rfft(x[:, c], size) * np.fft.rfft(ir[:, c], size), size)[:len(x)]
    return out


def main(out_path):
    S, kicks = build()
    duck = np.ones(LEN)
    for k in kicks:
        i = N(k); j = min(LEN, i + N(.35))
        s = np.arange(j - i) / SR
        duck[i:j] = np.minimum(duck[i:j], 1 - .55 * np.exp(-s / .11) * (1 - np.exp(-s / .004)))
    for name in ('bass', 'pad', 'arp', 'choir', 'drone'):
        S[name].buf *= duck[:, None]

    dry = sum(s.buf for s in S.values())
    send = sum(s.buf * s.send for s in S.values())
    mix = dry + .55 * convolve(send, reverb_ir())

    X = np.fft.rfft(mix, axis=0)
    f = np.fft.rfftfreq(len(mix), 1 / SR)[:, None]
    shelf = 1 + 0.5 / np.sqrt(1 + (3500 / np.maximum(f, 1)) ** 4)
    tame = 1 / np.sqrt(1 + (np.maximum(f, 1) / 11000) ** 4)          # soften the very top: no fizz
    dip = 1 - 0.3 * np.exp(-((np.log2(np.maximum(f, 1) / 300)) ** 2) / 0.5)
    hp = 1 / np.sqrt(1 + (28 / np.maximum(f, 1)) ** 8)
    mix = np.fft.irfft(X * shelf * tame * dip * hp, len(mix), axis=0)

    w = N(.05)
    env = np.sqrt(np.convolve((mix ** 2).mean(1), np.ones(w) / w, 'same')) + 1e-9
    thr = np.sqrt(np.mean(env ** 2)) * 1.4
    gain = np.where(env > thr, (env / thr) ** (1 / 2.5 - 1), 1.0)
    sm = N(.08)
    mix *= np.convolve(gain, np.ones(sm) / sm, 'same')[:, None]
    mix = np.tanh(mix / (np.abs(mix).max() * .8)) * .8
    mix[-N(1.5):] *= np.linspace(1, 0, N(1.5))[:, None] ** 2
    mix[0] = 0

    pcm = (np.clip(mix, -1, 1) * 32767).astype('<i2')
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 's16le', '-ar', str(SR), '-ac', '2', '-i', '-', out_path], input=pcm.tobytes(), check=True)

    def db(x): return 20 * np.log10(np.sqrt(np.mean(x ** 2)) + 1e-12)
    print('stem levels (dB RMS over the whole film):')
    for s in S.values():
        print(f'  {s.name:6s} {db(s.buf):6.1f}')
    m = mix.mean(1)
    Xm = np.abs(np.fft.rfft(m)) ** 2
    fm = np.fft.rfftfreq(len(m), 1 / SR)
    tot = Xm.sum()
    for lo, hi in [(20, 120), (120, 500), (500, 2000), (2000, 6000), (6000, 20000)]:
        print(f'  band {lo:5d}-{hi:5d} Hz: {100 * Xm[(fm >= lo) & (fm < hi)].sum() / tot:5.1f}%')
    print('per bar (dB RMS):', ' '.join(f'{b}:{db(m[N(T(b)):N(T(b + 1))]):.0f}' for b in range(1, NBARS + 1)))


if __name__ == '__main__':
    main(sys.argv[1])
