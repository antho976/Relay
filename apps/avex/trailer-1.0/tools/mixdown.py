#!/usr/bin/env python3
"""Lay the sound effects over the score, every cue on its frame, and level the result.

    python3 tools/mixdown.py out/score.wav out/cues.json out/mix.wav

The score (tools/score.py) and the picture (index.html) are written on the same 100 BPM grid, and
the cues come straight from the page, so nothing here is timed by hand. Every sfx file is
normalised to the same −3 dBFS peak by tools/sfx.py, so LEVEL below is the only place the
hierarchy lives. Transients sit higher than tones on purpose (RULES.md §4). The finished mix is
gained to −16 LUFS integrated and must peak no hotter than −2 dBFS.
"""
import json, subprocess, sys
from pathlib import Path
import numpy as np

SR = 48000
HERE = Path(__file__).resolve().parent.parent
LEVEL = {'tap': .7, 'tick': .5, 'pop': .6, 'confirm': .5, 'chime': .45, 'buzz': .8, 'whoosh': .32, 'page': .55, 'stamp': .6, 'hit': .85}
SFX_BUS = .8
TARGET_LUFS = -16.0


def decode(path, ch=2):
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(path), '-f', 'f32le', '-ac', str(ch), '-ar', str(SR), '-'], check=True, capture_output=True).stdout
    return np.frombuffer(raw, dtype=np.float32).reshape(-1, ch).astype(np.float64)


def loudness(path):
    r = subprocess.run(['ffmpeg', '-nostats', '-i', str(path), '-af', 'ebur128', '-f', 'null', '-'], capture_output=True, text=True)
    return float([l for l in r.stderr.splitlines() if l.strip().startswith('I:')][-1].split()[1])


def write(path, x):
    pcm = (np.clip(x, -1, 1) * 32767).astype('<i2').tobytes()
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 's16le', '-ar', str(SR), '-ac', '2', '-i', '-', str(path)], input=pcm, check=True)


def main(score_path, cues_path, out_path):
    spec = json.loads(Path(cues_path).read_text())
    n = int(round(spec['dur'] * SR))
    mix = np.zeros((n, 2))
    score = decode(score_path)[:n]
    mix[:len(score)] += score
    cache = {}
    for c in spec['cues']:
        name = c['sfx']
        if name not in cache:
            cache[name] = decode(HERE / 'audio' / 'sfx' / f'{name}.wav')
        s = cache[name] * LEVEL[name] * c.get('gain', 1) * SFX_BUS
        at = int(round(c['t'] * SR))
        end = min(n, at + len(s))
        if end > at:
            mix[at:end] += s[:end - at]
    tmp = Path(out_path).with_suffix('.raw.wav')
    write(tmp, mix)
    gain = 10 ** ((TARGET_LUFS - loudness(tmp)) / 20)
    tmp.unlink()
    mix *= gain
    peak = 20 * np.log10(np.abs(mix).max() + 1e-12)
    if peak > -2.0:
        mix *= 10 ** ((-2.0 - peak) / 20)
    write(out_path, mix)
    peak = 20 * np.log10(np.abs(mix).max() + 1e-12)
    print(f'{len(spec["cues"])} cues · {spec["dur"]:.3f} s · {loudness(out_path):.1f} LUFS integrated · peak {peak:.2f} dBFS')


if __name__ == '__main__':
    main(*sys.argv[1:4])
