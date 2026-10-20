# Design canvas: depth, connector ends and the early size refusal on the apply path (2026-10-20)

`engine-integrity` put three checks on `design.apply` (MODEL-SPEC §2.8, §2.11): the nesting
ceiling (W14) on every add and every move into another parent, the check of the connector ends
and pictures a list wrote, once it has run (B6), and the refusal of a list at the edit that takes
the design over its size (B59). The depth check runs once per add, so this is its before and
after on the 500-node apply, with `design.get` beside it as a control: it reads nothing new.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, release profile, in process:
`cargo test --release -p relay-core --test design -- --ignored --nocapture
measure_apply_500_and_get_5000`. Other streams were building beside it (load average 8 to 16),
so the reading is the instruction count, which the load does not move, with the wall clock
beside it.

## Before and after

The test binaries of `fde916d` (before) and of this stream's code were built once each and run
alternately.

Instructions, `perf stat -e instructions:u` over the whole test process (its setup, 22 applies
of 500 nodes, that is 11,000 adds, and 12 reads of 5,000 nodes), five runs each:

| | before | after | change |
|---|---:|---:|---:|
| whole harness | 2,815.5–2,816.0 M (2,815.8) | 2,817.1–2,818.3 M (2,817.3) | +1.5 M (+0.06%) |

Figures in brackets are the medians. That is about 140 instructions an add: tens of
nanoseconds, against the 8 µs each add of the 500-node apply takes (4 ms for 500).

Wall clock, each figure the median of 10 timed runs after 2 warm-ups, eight alternating runs
each at load average 8.5:

| path | before (8 runs) | after (8 runs) | change |
|---|---:|---:|---:|
| `design.apply`, 1 frame + 499 children | 3.92–4.21 ms (3.98) | 3.95–5.51 ms (3.99) | +0.1% |
| `design.get`, 5,000 nodes | 4.00–5.68 ms (4.08) | 4.07–4.21 ms (4.13) | +1.3% |

Figures in brackets are the median of the eight medians; the highs are runs another build
crowded. `design.get` runs none of this code, so its change is the spread: the checks cost
nothing measurable.

## What it costs, and the steps taken on the way

- The first version walked an add's parent chain twice in `Run.parents` (once to see it was all
  known, once to count it), built the page's name it did not need, and put every node it added
  in the map: +7.1 M instructions (about 650 an add), and 3.93–4.02 ms (4.00) against
  4.03–4.17 ms (4.13) on the 500-node apply, +3%, in eight alternating runs at load 9 (the
  ranges leave out the two runs on each side another build crowded).
- The depth check now walks once without allocating, only a node that can hold children goes
  in the map (where a leaf sits is read when B6 asks: one row when its parent is known), and the
  last parent's depth is remembered, since a list adds a node's children one after another;
  a move or a delete forgets it. That is the `after` column.

## A move to another page beside 1,500 connectors

After a move to another page, `release_ends` checks both ends of every connector in the design
to find the ones the move parted. Each check placed the line and the end with `place()`, which
read the node's row and, for a chain not yet seen, its ancestors. On a design of 4,500 nodes
holding 1,500 connectors, that was about 4,500 queries under the store lock for a move of one
card. Now the design's parent map is read once (the parent index covers it, so no bodies are
read), and only when the design holds a connector. `parted_at` reads the pages from that map in
memory, and first checks that the line or its end sits in something the list moved.

Harness: `measure_apply_beside_1500_connectors` in `crates/relay-core/tests/design_integrity.rs`
(1,500 frames each holding a text, and a line from each text to the next one; each list's undo
runs after it, untimed), `cargo test --release -p relay-core --test design_integrity --
--ignored --nocapture measure_apply_beside`. The before binary is this stream's code with the
review's two correctness fixes (3b1b1fc, 89c12c4) but without this change. Both binaries were
built once and run alternately, eight runs each, at load average 4 to 11. Each figure is the
median of 30 timed runs after 2 warm-ups; in brackets, the median of the eight medians.

| path | before (8 runs) | after (8 runs) | change |
|---|---:|---:|---:|
| `design.apply` update | 30.1–31.3 µs (30.5) | 29.7–31.4 µs (30.2) | none |
| move into another parent on the page | 270–281 µs (271) | 270–279 µs (272) | none |
| delete (reads every connector's ends) | 1.12–1.17 ms (1.13) | 1.13–1.15 ms (1.15) | none |
| move to another page | 6.65–6.83 ms (6.69) | 1.97–2.02 ms (2.00) | −70% |

Instructions, `perf stat -e instructions:u` over the whole harness: 8,849–8,855 M (8,852) before
and 4,740–4,741 M (4,740) after. The harness moves across pages 64 times (32 moves and their 32
undos), so that is about 64 M fewer instructions a move.

The steps:

- Reading the map alone took the move to 2.31–2.35 ms (three runs).
- Placing the ends from the map without `place()`, which builds the page's name and walks each
  chain twice per end, took it to 2.03–2.06 ms.
- Reserving the map's size and skipping the read when the design holds no connector gives the
  `after` column.

Timers put in the code to measure this (not kept) show what remains over the delete: about
0.47 ms for the parent read of 4,500 rows (before reserving), and the in-memory check of 3,000
ends. The scan of connector bodies costs the delete the same.

## Guardrails

- Depth: one cached recursive CTE for a chain the list has not seen, bounded at `MAX_DEPTH + 6`
  levels of primary-key lookups; one downward CTE for a move into a deeper place (index
  `design_nodes_parent`, the same bound). Nothing recurses in Rust, so a chain nested past the
  limit before it existed cannot overflow anything.
- B6: one stat for each `src` a list writes; for each end it writes, a row read, or a chain read
  the first time; the nearest id (every id of the design) only on the way to a refusal.
- `release_ends` reads the connectors only when the list deleted a node (as before) or moved one
  to another page. After such a move it reads where every node sits once, from the parent
  index, and only when the design holds a connector; nothing in it is per end.
- W14 on a chain past `MAX_DEPTH + 6`, which only rows another build wrote can hold: a move
  between two such places counts each chain to its page with one recursive CTE, bounded by
  `MAX_NODES`.
- B59: one pass over the list's edits to find the last that can shrink the design, and a test per
  edit once the design is past the cap.
