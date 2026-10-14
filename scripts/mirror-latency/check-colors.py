#!/usr/bin/env python3
"""Read the probe's color chart back from screenshots of the mirror and compare.

    check-colors.py shot.png [other.png ...]

Each screenshot must show the probe on a still color chart (`probe.py --chart SECONDS`, the
screenshot taken meanwhile). The picture is found by the probe's magenta marker; each of the 16
patches is averaged over its middle and compared with the sRGB value the device drew (Probe.java's
CHART). With two or more screenshots, each later one is also compared with the first, patch by
patch — the check that a change to the decoder or the texture left the colors where they were.
H.264 itself moves a saturated patch by a few levels (4:2:0 chroma, quantization), so the
comparison with the device is a sanity bound; the one between screenshots should be near zero.
"""
import sys

from PIL import Image

CHART = [
    0xff0000, 0x00ff00, 0x0000ff, 0xffffff,
    0x00ffff, 0xff00ff, 0xffff00, 0x000000,
    0x404040, 0x808080, 0xc0c0c0, 0xe0ac69,
    0xff8000, 0x203060, 0x8a2be2, 0x2e8b57,
]
NAMES = ["red", "green", "blue", "white", "cyan", "magenta", "yellow", "black",
         "gray 25%", "gray 50%", "gray 75%", "skin", "orange", "navy", "violet", "sea green"]


def patches(path):
    image = Image.open(path).convert("RGB")
    w, h = image.size
    pixels = image.load()
    magenta = lambda x, y: (lambda r, g, b: r > 200 and g < 60 and b > 200)(*pixels[x, y])
    # The marker's top stripe: the first magenta row, read left to right. (The chart has a
    # magenta patch too, but lower and to the right.)
    top = next(((x, y) for y in range(h) for x in range(w) if magenta(x, y)), None)
    if top is None:
        sys.exit(f"{path}: no probe marker (magenta) on screen")
    x0, y0 = top
    row = y0 + 2
    x1 = x0
    while x1 + 1 < w and magenta(x1 + 1, row):
        x1 += 1
    # The bottom stripe ends where the picture does.
    column = x0 + (x1 - x0) // 4
    y1 = max(y for y in range(y0, h) if magenta(column, y))
    pw, ph = (x1 - x0 + 1) * 3, y1 - y0 + 1
    field, cw, ch = x0 + pw / 3, (pw - pw / 3) / 4, ph * 0.36 / 4
    out = []
    for i in range(16):
        cx, cy = field + cw * (i % 4 + 0.5), y0 + ph * 0.60 + ch * (i // 4 + 0.5)
        r = max(1, int(min(cw, ch) / 4))
        box = [pixels[x, y] for x in range(int(cx) - r, int(cx) + r) for y in range(int(cy) - r, int(cy) + r)]
        out.append(tuple(round(sum(p[c] for p in box) / len(box)) for c in range(3)))
    return out, (x0, y0, pw, ph)


def main():
    shots = sys.argv[1:]
    if not shots:
        sys.exit(__doc__)
    first = None
    worst_between = 0
    for path in shots:
        measured, (x, y, w, h) = patches(path)
        print(f"{path}: picture at {x},{y}, {w}x{h} on screen")
        worst = 0
        for i, (got, want) in enumerate(zip(measured, CHART)):
            want = ((want >> 16) & 255, (want >> 8) & 255, want & 255)
            diff = max(abs(a - b) for a, b in zip(got, want))
            worst = max(worst, diff)
            versus = ""
            if first is not None:
                between = max(abs(a - b) for a, b in zip(got, first[i]))
                worst_between = max(worst_between, between)
                versus = f"  vs first {between:3d}"
            print(f"  {NAMES[i]:10s} drew {want}  shows {got}  off {diff:3d}{versus}")
        print(f"  worst difference from what the device drew: {worst}")
        first = first or measured
    if len(shots) > 1:
        print(f"worst difference between screenshots: {worst_between}")


if __name__ == "__main__":
    main()
