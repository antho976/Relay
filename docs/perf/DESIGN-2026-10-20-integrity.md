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

## Guardrails

- Depth: one cached recursive CTE for a chain the list has not seen, bounded at `MAX_DEPTH + 6`
  levels of primary-key lookups; one downward CTE for a move into a deeper place (index
  `design_nodes_parent`, the same bound). Nothing recurses in Rust, so a chain nested past the
  limit before it existed cannot overflow anything.
- B6: one stat for each `src` a list writes; for each end it writes, a row read, or a chain read
  the first time; the nearest id (every id of the design) only on the way to a refusal.
- `release_ends` reads the connectors only when the list deleted a node (as before) or moved one
  to another page.
- B59: one pass over the list's edits to find the last that can shrink the design, and a test per
  edit once the design is past the cap.
