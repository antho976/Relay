# Design canvas: forward-compatible bodies (2026-10-20)

`bus-1-compat` changed how every design node is read and checked: serde no longer refuses an
unknown key (it lands in `Node::extra`, written back as it was), every parse goes through
`Node::read`, an edit's node is read through `read_node`, and a strict list's add runs
`Node::check_new` (the per-kind field table, B8). This is the before and after of the two engine
paths that touch every node, as MODEL-SPEC §5 #10 asks.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, release profile, in process:
`cargo test --release -p relay-core --test design -- --ignored --nocapture
measure_apply_500_and_get_5000` (each figure the median of 10 timed runs after 2 warm-ups). Load
average 2.0–2.5 while measuring, other agents building beside it.

## Before and after

The two test binaries (`7655fe5` and `f90a97f`) were built once each and run alternately, five
times each, so both saw the same machine state:

| path | before (5 runs) | after (5 runs) | change |
|---|---:|---:|---:|
| `design.apply`, 1 frame + 499 children | 3.54–3.61 ms (3.57) | 3.64–3.68 ms (3.65) | +2.3% |
| `design.get`, 5,000 nodes | 3.73–3.79 ms (3.76) | 3.78–3.85 ms (3.82) | +1.6% |

Figures in brackets are the median of the five medians.

## What it costs, and the step taken on the way

- The first version read an edit's node by building a `serde_json::Value` of it and reading the
  node from that (`Node::read`), so a node that does not read can be read again key by key. That
  cost the 500-node apply 13%: 4.18–4.25 ms against 3.54–3.58 ms, measured back to back. Taking
  `read_node` out alone brought it back to 3.70–3.72 ms.
- `read_node` now reads through an untagged `Incoming { Node, Json(Value) }`: serde buffers the
  edit once (the internally tagged `NodeOp` already had, and hands that buffer over rather than
  copying it), tries the node directly from it, and keeps JSON only for a node that does not read.
  That is the `after` column.
- What is left on `design.apply` is `check_new` (about 0.08 ms of the 500 adds: taking it out gave
  3.61–3.63 ms) and the flattened `extra` (an empty map per node, and an identifier visitor that
  can keep an unknown key).
- `design.get` does no new work under the lock. It parses rows after the lock is released, as
  before, now through `Node::read_str` (the derived fast path first; an unreadable row is left out
  with a warning instead of failing the design, B61). The difference is within the two columns'
  spread of each other's runs and within what monomorphising the new read paths can move.

## Guardrails

- Both figures stay well inside the 16 ms-per-request budget of `docs/perf/PLAYBOOK.md`; the
  1,720-node agent apply of `DESIGN-2026-10-16.md` scales with the same per-node costs.
- The slow path (`read_map`, a probe per key) runs only for a node that does not read: a body a
  newer build wrote, or an agent's typo. It is never on the canvas's or the agents' usual path.
