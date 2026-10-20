# Design canvas: render and Play from the smoke shots (2026-10-20)

The `smoke` stream of design round two runs the real desktop app headless against a disposable
engine (`scripts/design-smoke.py`, inside its own `kwin_wayland --virtual --no-lockscreen`). Two of
its shots time what they drive, and print the readings to `.impeccable/review/design-readings.json`.
These are readings, not a before and after: they pin where render and Play stand once every
round-two stream is in, for the next change to compare against.

AMD Ryzen 7 9800X3D (8 cores, 16 threads), CachyOS kernel 7.2.9, GTK 4.24.1, dev profile
(optimized + debuginfo, what `run.sh` launches), `RELAY_DESIGN_TRACE=1`. Two full runs of the
script one after the other, every shot passing (18 of 18 each); load average 1.5 to 2.6 at their
end. The fixtures' stores are on `/tmp`, which is RAM here.

```
cargo build -p relay-cli -p relay-native
python3 scripts/design-smoke.py            # or: python3 scripts/design-smoke.py render motion perf
```

## `design.render`, drawn by the open app

The `render` shot opens the app on a small design and, while it runs, asks `design.render` ten
times for each of three designs; the timings are the reply's `timings_ms` (medians of ten), and
the steps are the app's own `DESIGN_TRACE render-<step>` lines for those ten (render-SPEC §9: each
main-thread step under 8 ms on the 1,721-node fixture, recorded, not a gate).

| reply `timings_ms`, median | read | layout | paint | encode |
|---|---:|---:|---:|---:|
| Render check (3 nodes), run 1 / run 2 | 0.12 / 0.10 ms | 0.01 / 0.01 ms | 0.30 / 0.33 ms | 1.18 / 1.18 ms |
| Settings screen (38 nodes) | 0.41 / 0.41 ms | 0.03 / 0.03 ms | 2.29 / 2.25 ms | 16.45 / 16.50 ms |
| Big design (1,721 nodes, 1,720 boxes) | 8.63 / 7.85 ms | 0.44 / 0.44 ms | 8.80 / 8.83 ms | 31.02 / 31.15 ms |

| main-thread step, 1,721 nodes | median | p95 | max |
|---|---:|---:|---:|
| `render-boxes`, run 1 / run 2 | 1.73 / 1.74 ms | 1.81 / 1.86 ms | 1.81 / 1.86 ms |
| `render-content` | 1.24 / 1.28 ms | 2.73 / 2.73 ms | 2.73 / 2.73 ms |
| `render-layout` | 0.43 / 0.44 ms | 1.37 / 1.38 ms | 1.37 / 1.38 ms |
| `render-paint` | 7.54 / 7.56 ms | 8.39 / 9.42 ms | 8.39 / 9.42 ms |

On the settings screen every step stays under 5 ms (`render-paint` p95 4.9 / 4.7 ms). On the
1,721-node page `render-paint` is the one step near the budget: its median is under 8 ms and its
p95 over it in both runs. The encode, the largest cost, is off the main thread. **Lead:** the
page's paint on a render is the step to look at first if render is ever made a gate.

## Play and the APNG export

The `motion` shot plays the Motion fixture (an artboard with a 3 s timeline and seven movers:
a sliding title, a dot, a spinning icon, a popping button, a filling ring, a line drawing on, and
a layer sliding past the board) for 400 ms, pauses, seeks, writes a contact sheet, stops, and
exports the page as an APNG at 30 fps.

| | run 1 | run 2 |
|---|---:|---:|
| `play_frame`, frames | 29 | 29 |
| `play_frame`, median | 0.055 ms | 0.049 ms |
| `play_frame`, p95 | 0.065 ms | 0.059 ms |
| `play_frame`, max | 0.070 ms | 0.065 ms |
| APNG export, 90 frames, 1020 × 384 | 131 ms | 132 ms |
| canvas frames meanwhile, longest gap | 9, 16.7 ms | 9, 16.7 ms |

A frame of Play stays under 1% of the 8.3 ms budget (feature-motion §9), in line with the 38-node
board reading in `DESIGN-2026-10-20-play.md` (median 0.063 ms). The export holds the canvas to
one 60 Hz frame at most while it draws 90 frames. The shot asserts that the design's rev is
unchanged across Play, seek, the sheet and the export.

## Beside them

From the same runs, for reference (no budget change):

- the `perf` shot's 1,721-node board: `layout` median 0.41 ms (p95 1.8 ms), `content` 2.0 ms,
  `draw` median 0.05 ms over 486 frames of panning, zooming and a drag;
- `design.apply` of the 1,721 nodes 26.7 / 28.2 ms, `design.get` of them 5.6 ms median;
- the `more` shot's exports drawn back to pixels against its PNG export: SVG (rsvg-convert)
  0.16/255 mean difference, PDF (pdftoppm) 0.35/255; a box's shadow and an ellipse arc's with the
  same 16 px blur both have a 7 px 50% band (TM-25).
