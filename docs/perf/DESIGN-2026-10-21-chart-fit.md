# Design canvas: charts that fill, sized by the layout (2026-10-21)

A chart frame with `w_sizing` or `h_sizing` `fill` was drawn at its stored size whatever room its
card gave it, so a narrower screen pushed its parts out of the card (160 px over at 1280 × 800 in
a test design). `design.apply` now lays out what holds such a chart once a list has run, writes
the chart's laid-out size, and draws it again (`handlers/design/charts.rs`, `fit_charts`;
BUS.md, Charts). That is a layout under the store lock, so it was measured before it was kept.

AMD Ryzen 7 9800X3D (8 cores, 16 threads), CachyOS kernel 7.2.9, governor `performance`, rustc
1.99.0, **dev profile**, in process (`crates/relay-core/examples/perf.rs`, `--counters`). Figures
are user-space instructions per edit, the per-iteration median of 30; the wall clock is beside
them. Load average 5 to 10, other sessions building: the instruction counts do not move with it.

## The rows

New in `perf.rs`: `1721c` and `4903c` are the `1721` and `4903` designs (phone artboards in auto
layout) with a stacked bar chart that fills artboard `a0`'s width. One text is edited:

- `apply.update_text.<n>`: a text in artboard `a3`, away from the chart;
- `apply.update_text_beside_chart.<n>`: a text in `a0`, beside it.

## Readings

| row | first cut: the whole page laid out | the chart's page child laid out | and the chart read only where reached |
|---|---:|---:|---:|
| `update_text.1721` (no chart) | 0.44 M, 0.042 ms | 0.44 M | 0.44 M |
| `update_text.1721c` | 61.13 M, 3.84 ms | 0.59 M | **0.55 M**, 0.050 ms |
| `update_text.4903c` | 177.71 M, 10.90 ms | 0.59 M | **0.56 M**, 0.049 ms |
| `update_text_beside_chart.1721c` | — | 6.09 M, 0.34 ms | **2.80 M**, 0.19 ms (with `CROSS JOIN`) |
| `update_text_beside_chart.4903c` | — | 12.28 M, 0.60 ms | **2.79 M**, 0.19 ms (with `CROSS JOIN`) |

1. **The whole page, laid out on every edit** (first cut): 140 times the edit at 1,721 nodes and
   growing with the design, 10.9 ms of lock at 4,903, two thirds of the 16 ms budget. Not kept.
2. **Only the page child that holds the chart.** A page lays nothing out, so where a chart is drawn
   depends on nothing outside its artboard; only an edit in that artboard lays it out, and only
   that artboard is read. An edit elsewhere drops to 0.59 M at either size. An edit beside the
   chart still grew with the design (6.1 M → 12.3 M): the read of the artboard's rows joined the
   recursive walk to `design_nodes` as a plain join, and SQLite searched the design's primary key
   by `design_id` alone with an automatic index on the walk, a pass over every row. `CROSS JOIN`
   keeps the walk outside, so each row is one key lookup: 2.8 M at both sizes. A unit test checks
   the plan (`the_subtree_is_read_by_index`), and fails on the plain join.
3. **The chart's body read only where the list reached.** Each chart's artboard is found from the
   parent index first; its row is read and parsed only when it is in a reached artboard. 0.59 M →
   0.55 M for an edit elsewhere. What is left (+0.11 M, about 10 µs) is the two parent walks that
   find the edited node's artboard and the chart's.

A design without a chart that fills reads the list off the `designs` row it already reads and
does nothing else: `update_text.1721` and `.4903` are 0.44 M on both builds.

## The ratchet

`scripts/perf/ratchet.py` on this branch and on `main` with only the new `perf.rs` rows, both on
this machine today: every existing row is within 0.5% of `main`'s figure (`apply.move.*` +0.3%,
the larger `designs` row read). `main` itself is over six ceilings set on 2026-10-20 by 1.5 to
4.3% (`apply.resize.*` +4.0 to 4.3%, `apply.batch_1000.*`, `apply.ungroup_1999_of_4998`), which
this change does not move; those ceilings are left as they are. The eight new rows take their
ceilings from the last column above.
