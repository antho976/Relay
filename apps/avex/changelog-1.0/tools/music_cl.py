#!/usr/bin/env python3
"""The changelog's score: the trailer's instruments (trailer-1.0/tools/music.py), re-arranged.

    python3 changelog-1.0/tools/music_cl.py changelog-1.0/out/score.wav

120 BPM, so a beat is 0.5 s (30 frames at 60 fps) and a bar is 2 s. D major, D A Bm G: lighter
than the trailer's D minor, because this one reports rather than sells.

   1–2   title          pad and the bell theme
   3–14  six features   four on the floor; every seam lands on an even bar's downbeat
  15–18  also changed   the same groove with sixteenth hats
  19–20  small details  groove continues under the checklist
  21     the breath     drums out, a riser
  22–23  end card       the D chord, the bell theme, ring-out

It also writes kicks.json next to the score, so tools/synccheck.py can measure every kick against
the picture.
"""
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'trailer-1.0' / 'tools'))
import numpy as np
import music as m

m.BPM, m.BEAT, m.BAR, m.NBARS = 120, .5, 2.0, 23
m.LEN = int(round(m.NBARS * m.BAR * m.SR))
m.CHORDS['Em'] = ['E3', 'B3', 'E4', 'G4', 'B4']
m.ROOT['Em'] = 'E2'
T, N, midi, hz = m.T, m.N, m.midi, m.hz
CYCLE = ['D', 'A', 'Bm', 'G']
GROOVE = range(3, 21)
KICKS_OUT = []


def chord_at(bar):
    return {1: 'D', 2: 'G', 21: 'A', 22: 'D', 23: 'D'}.get(bar) or CYCLE[(bar - 3) % 4]


def energy(bar):
    if bar <= 2: return .3
    if bar <= 14: return .75
    if bar <= 18: return .9
    if bar <= 20: return .7
    if bar == 21: return .3
    return .9


def build():
    S = {k: m.Stem(k, s) for k, s in [('pad', .45), ('bell', .55), ('bass', .05), ('arp', .35), ('kick', .08), ('snare', .4),
                                      ('toms', .3), ('hats', .12), ('fx', .4),
                                      ('choir', .5), ('drone', .3)]}  # empty here; the shared mastering ducks them
    kicks = []

    for bar in range(1, m.NBARS + 1):
        ch, e = chord_at(bar), energy(bar)
        dur = m.BAR + (1.5 if bar == m.NBARS else .5)
        tt = np.arange(N(dur)) / m.SR
        cutoff = (800 + 2400 * e) * (1 + .12 * np.sin(2 * np.pi * tt / m.BAR))
        x = np.zeros(N(dur))
        for i, nm in enumerate(m.CHORDS[ch]):
            for dt in (-11, -4, 5, 12):
                x += m.saw(hz(midi(nm)) * 2 ** (dt / 1200), dur, cutoff, phase_seed=bar * 100 + i * 10 + dt, max_h=40)
        env = m.adsr(len(x), .25 if bar in (1, 21) else .04, .2, .9, .5 if bar < m.NBARS else 1.4)
        S['pad'].add(x * env, T(bar), (.008 + .024 * e) * (1.15 if bar in GROOVE else 1.4))

    for bar, beat, nm in [(1, 0, 'F#5'), (1, 1, 'A5'), (1, 2, 'D6'), (2, 0, 'B5'), (2, 2, 'G5'),
                          (22, 0, 'D5'), (22, 0, 'A5'), (22, 1, 'F#5'), (22, 2, 'A5'), (22, 3, 'D6'), (23, 0, 'F#5'), (23, 2, 'D5')]:
        S['bell'].add(m.bell(midi(nm), 3.0), T(bar, beat), .13, pan=.25 if beat % 2 else -.25)

    for bar in GROOVE:
        root, e = midi(m.ROOT[chord_at(bar)]), energy(bar)
        for i in range(8):
            d = m.BEAT / 2
            mm = root + (12 if i % 2 else 0)
            tt = np.arange(N(d)) / m.SR
            cut = (200 + 900 * e) * (1 + 2.5 * np.exp(-tt / .04))
            x = m.saw(hz(mm), d, cut, max_h=50, phase_seed=i) + .6 * np.sin(2 * np.pi * hz(mm) * tt)
            S['bass'].add(x * m.adsr(len(x), .004, .06, .7, .04), T(bar, i / 2), (.13 + .06 * e) * .65)

    for bar in list(GROOVE) + [22]:
        ch, e = m.CHORDS[chord_at(bar)], energy(bar)
        pat = [2, 3, 4, 3, 2, 4, 3, 1]
        for i in range(16):
            mm = midi(ch[pat[i % 8]]) + 12
            d = m.BEAT / 4
            tt = np.arange(N(d * 1.8)) / m.SR
            cut = (600 + 3400 * e) * (1 + 1.5 * np.exp(-tt / .03))
            x = m.saw(hz(mm), d * 1.8, cut, max_h=40, phase_seed=i, pulse=True) * np.exp(-tt / (.06 + .04 * e))
            S['arp'].add(x, T(bar, i / 4), (.07 + .09 * e) * (.6 if bar == 22 else 1) * 1.5, pan=.35 * np.sin(i * 1.3))

    def K(t, big=False, g=1.0):
        S['kick'].add(m.kick(big), t, g * .5); kicks.append(round(t, 4))
    for bar in GROOVE:
        for b in range(4):
            K(T(bar, b), big=(b == 0 and bar % 2 == 0), g=.9)
        S['snare'].add(m.snare(), T(bar, 1), .7); S['snare'].add(m.snare(), T(bar, 3), .7)
        for i in range(16 if 15 <= bar <= 18 else 8):
            step = 4 if 15 <= bar <= 18 else 2
            S['hats'].add(m.hat(), T(bar, i / step * 1.0), .12 if i % 2 == 0 else .06, pan=.2)
    for bar in (14, 18):
        for i, p in enumerate([110, 98, 88, 78]):
            S['toms'].add(m.tom(p), T(bar, 3 + i * .25), .3, pan=-.4 + i * .25)
    K(T(22), big=True, g=1.0)

    S['fx'].add(m.impact(2.5), T(3), .22)
    S['fx'].add(m.riser(m.BAR * .95), T(21), .22)
    S['fx'].add(m.impact(4.0), T(22), .45)
    S['fx'].add(m.reverse_swell(m.BAR * .5), T(2, 2), .05)

    KICKS_OUT[:] = kicks
    return S, kicks


m.build = build

if __name__ == '__main__':
    out = sys.argv[1]
    m.main(out)
    Path(out).with_name('kicks.json').write_text(json.dumps(KICKS_OUT))
    print(f'{len(KICKS_OUT)} kicks → {Path(out).with_name("kicks.json")}')
