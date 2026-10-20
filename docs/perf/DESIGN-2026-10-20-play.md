# Design canvas: a frame of Play (2026-10-20)

`native-motion-play` added Play: the page's motion runs on the canvas, one frame per tick of the
frame clock. Play is new, so this is a reading, not a before and after; the comparison that
matters is a frame of Play against the whole page rebuild the canvas does after an edit, timed
in the same runs. Budget (feature-motion §9): a frame's build under 8.3 ms at p95 (120 Hz).

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, GTK 4.24.1, dev profile (optimized +
debuginfo, what `run.sh` launches). Load average 7.5–10 while measuring, other agents building
beside it.

## What a frame does

Each tick moves Play's time and drops the page's cached render node; the draw then takes every
mover's pose at that time (`Clocks::poses`, into one map kept from frame to frame) and paints the
page with them (`Painter::with_posing`). A node that neither moves nor holds a mover is painted
once, as a render node kept until the clocks are made again (an edit, a reread, a picture
loading), and appended as it is every frame after: one frame repaints the movers and their
ancestors only. No layout, no text measuring, no chrome refresh, no engine traffic.

## The 1,721-node design, 50 movers

The ignored test `a_play_frame_builds_within_its_budget` (`apps/relay-native/src/design/paint.rs`)
builds the perf reading's design (`scripts/design-smoke.py perf`: 40 phone artboards in auto
layout, each a title and ten rows of a text and a toggle; 1,721 nodes) with 50 movers: in 5
artboards the title slides up and fades in, and 9 toggle knobs slide back and forth on a loop.
It lays the page out with Pango measuring, then times 40 whole rebuilds at rest and 241 frames
of Play at 120 Hz steps (poses and the page's render node, as `content_node` builds them). Run
three times inside `kwin_wayland --virtual`:

```
cargo test -p relay-native --no-run
WAYLAND_DISPLAY=<virtual socket> target/debug/deps/relay_native-<hash> a_play_frame --ignored --nocapture
```

| per frame | run 1 | run 2 | run 3 |
|---|---:|---:|---:|
| whole rebuild at rest, median | 1.603 ms | 1.574 ms | 1.523 ms |
| whole rebuild at rest, p95 | 2.468 ms | 2.572 ms | 1.954 ms |
| Play, first frame (paints and keeps the stills) | 1.500 ms | 1.449 ms | 1.609 ms |
| Play, median of the next 240 | 0.113 ms | 0.126 ms | 0.092 ms |
| Play, p95 | 0.139 ms | 0.149 ms | 0.137 ms |
| Play, max | 0.184 ms | 0.197 ms | 0.158 ms |

A frame of Play costs about a twelfth of a rebuild and under 2% of the 8.3 ms budget; its first
frame costs what a rebuild does, once.

## On the board

A scripted run of the settings screen (38 nodes, `scripts/design-smoke.py`'s `edit` shot with a
script giving the title, a toggle knob and the button a motion and pressing Play) inside the
same compositor, `RELAY_DESIGN_TRACE=1`: 95 frames, `DESIGN_TRACE play_frame` median 0.063 ms,
p95 0.084 ms, max 0.179 ms (the first). That run used a scratch drive verb to press Play;
`drive.rs`'s own `play`, `pause`, `seek` and `stop` verbs are native-motion-export's.

## Not timed here

- The GPU's render of each frame's node. It renders the same page it renders at rest; the
  frame draw while panning the same design was 0.08 ms median (`DESIGN-2026-10-16.md`).
- Pictures that animate themselves (SMIL in an SVG): each is repainted every frame inside the
  page, its cost GtkSvg's.

## Guardrails

- At rest nothing ticks: Play never starts by itself, a motion that does not loop pauses at its
  end (no more frames), and Play pauses when the canvas is unmapped.
- `RELAY_DESIGN_TRACE=1` prints `DESIGN_TRACE play_frame {ms} ms movers {n}` for every frame of a
  real session.
- The ignored test asserts the p95 under 8.3 ms; it needs a display, so CI does not run it.
