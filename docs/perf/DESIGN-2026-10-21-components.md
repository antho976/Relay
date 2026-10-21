# Design canvas: components on the apply path (2026-10-21)

Components keep instances as real nodes. After a list's edits, every main the list reached is synced
to its instances (`crates/relay-core/src/handlers/design/components.rs`) with ordinary edits, under
the store lock. Two costs matter:

- what this adds to every list of a design without components, which is nearly every list;
- what a main's change costs with many instances.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, in process.

## Every list: the ratchet

`python3 scripts/perf/ratchet.py`: the guarded rows' user-space instructions, dev profile, the median
of 3 runs (`DESIGN-2026-10-20-scale.md`). `main` (`a479385`) was measured the same way, to tell this
change from what was there before.

| row | `main` | first try (partial indexes) | committed (`designs.components`) |
|---|---:|---:|---:|
| `apply.update.5000` | 0.43 M (+0.18%) | 0.44 M (+2.75%) | 0.43 M (+0.25%) |
| `apply.add.5000` | 0.44 M (+0.27%) | 0.45 M (+2.18%) | 0.44 M (+0.52%) |
| `apply.delete_subtree.500` | 7.21 M (+0.05%) | 7.79 M (+8.09%) | 7.22 M (+0.10%) |
| `apply.resize_pinned.2000` | 9.77 M (+0.18%) | 10.35 M (+6.14%) | 9.80 M (+0.42%) |
| `apply.undo_1000_adds.500` | 128.20 M (+0.08%) | 142.29 M (+11.09%) | 128.36 M (+0.21%) |
| `apply.delete_all_4998` | 609.58 M (+0.06%) | 669.57 M (+9.90%) | 610.49 M (+0.21%) |

Each percentage is the row against its ceiling in `ratchet.json`. Every guarded row of the committed
version is within the 1.5% tolerance, the largest at +0.82% (`path.design.commit.4903`, a client path
this change does not touch: the spread of a run).

There were two wrong turns, both caught by the ratchet.

1. The question "does this design hold a main?" was first a query on a partial index over the bodies
   (`instr(body, '"component":true') > 0`). Without statistics, SQLite took the primary key's
   `design_id` range instead, which reads every body. That made a one-node update 25 times dearer at
   5,000 nodes (0.43 M to 10.9 M). `INDEXED BY` fixed the query.
2. The two partial indexes still cost every node write: SQLite tests an index's condition on each
   insert, update and delete, about 12,000 instructions a row here (`undo_1000_adds` +11%,
   `delete_all_4998` +10%). The committed version has no index. Instead, `designs.components`
   (migration v32) is set once a design holds a main and is read with the design row every list
   already reads, so a design without components pays nothing. The nodes following a main are found
   by one pass over the design's bodies, only when that main changed.

## A main's change: its instances

`cargo test --release -p relay-core --test design_components -- --ignored --nocapture
a_mains_change_costs_with_its_instances`: a row main of 10 nodes and 40 instances of it (441 nodes).
One update of the main's title colour reaches all 40; the figure beside it is an update of a plain
node of the same design. Median of 15 applies, release profile.

| edit | time |
|---|---:|
| a plain node | 0.04 ms |
| the main, 40 instances | 2.32 ms, then 2.11 ms once the main is read once per main, not once per instance |

The 2.1 ms is 40 node writes (the title in each instance), each an ordinary update with its undo and
expectation, plus 40 instance subtrees read and compared: about 50 µs per instance. It holds the store
lock. At this rate a main with 300 instances would pass the 16 ms budget for a list.

Leads, none taken:

- Compare a main's changed fields with each instance before reading the whole instance subtree.
- Keep an index of instance roots by main (a column written by the sync), so `FOLLOWERS` stops being
  a pass over the bodies.
