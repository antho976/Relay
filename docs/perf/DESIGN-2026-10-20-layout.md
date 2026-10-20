# Design canvas: layout measured once a pass (2026-10-20)

`client-layout` rewrote `relay-client`'s `layout.rs`: every size is worked out once a pass and
kept (B45), a laid-out frame of its own size no longer measures its children to size itself,
measuring and placing share one sizing rule (`line_sizes`, B23), and `Placed`'s maps are
FxHashMaps (TM-80 part e). It also added pins, absolute children, min and max, wrap and scroll
offsets, which the readings below include. This is the before (`fde916d`) and after (`9f48271`)
of `layout::place`, which the board runs on every redraw.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, GTK 4.24.1, Pango 1.58.2. Load
average 2.9–6 while measuring, other agents building beside it.

## The board, real text measure

`python3 scripts/design-smoke.py perf` inside `kwin_wayland --virtual`: the 1,721-node design
(40 artboards, each a column with a title and a list of ten rows) on an open board, dev profile
(what `run.sh` launches), Pango measuring text. Figures are the `DESIGN_TRACE layout` medians of
nine relayouts per run; each side was run three times from one build.

| `layout::place`, 1,721 nodes | before (3 runs) | after (3 runs) | change |
|---|---:|---:|---:|
| median | 1.348, 1.398, 1.466 ms | 0.527, 0.558, 0.571 ms | −60% |
| p95 | 1.725, 1.921, 2.096 ms | 0.998, 1.062, 1.282 ms | −45% |

The same runs' `content` (1.8–3.1 ms) and `draw` (0.05 ms median) did not move: layout is the
only half that changed.

## Layout alone, fixed-width measure

A scratch program (not in the tree) built the same 1,721-node design, and a 114-artboard one of
4,903 nodes, as `Scene`s and timed `layout::place` with the tests' fixed-width `Mono` measure, 400
runs after 50 warm-ups, so only layout's own work is timed:

| `layout::place` | before | after the memo (`eda8f59`) | after (`9f48271`) |
|---|---:|---:|---:|
| 1,721 nodes, release | 0.76–0.79 ms | 0.41–0.44 ms | 0.34–0.35 ms |
| 4,903 nodes, release | 2.22 ms | 1.88 ms | 1.03 ms |
| 1,721 nodes, dev | 1.14–1.18 ms | – | 0.48–0.78 ms |
| 4,903 nodes, dev | 3.28 ms | – | 1.58 ms |

The dev-profile "after" figures were taken with the machine busier (load 4–6) and spread more.

## Where it came from

- A fixed-width row measured each child twice (its share pass, then its children pass), and
  every `lay_out` measured its children again, so a text twenty rows down a chain was measured
  2,097,151 times per relayout. The test `a_deep_chain_of_rows_measures_each_text_once` now
  holds it to at most 2.
- The fixture's rows are fill-width, hug-height rows inside a fill-width column, so each level
  measured its subtree once for its own size and again when its parent laid it out. The
  per-pass memo makes the second a lookup.
- A laid-out frame whose width and height are its own (`fixed`, or a fill it is offered) no
  longer lays out its children to size itself: only `lay_out` does, once.
- FxHash for node ids: SipHash was the top self time in teal-moose's profile of the board.

## Guardrails

- A frame has 8–16 ms (`PLAYBOOK.md` §2). On this design layout took about a sixth of the
  8.3 ms of a 120 Hz frame before; it now takes about a fifteenth.
- The memo is per pass and keyed by node and offered width (only a filling width reads the
  offer), so a probe or an edit between passes can never read a stale size.
- `perf` records the joint TM-80 reading (the scene's FxHash, the patch reread, the gallery) in
  its own file; this one is layout's share.
