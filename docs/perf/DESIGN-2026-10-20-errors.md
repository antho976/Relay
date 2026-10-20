# Design canvas: named errors on the apply path (2026-10-20)

`bus-2-errors` put a reader of its own on `design.apply`'s `ops` (`relay_bus::design::de_ops`),
so an edit that does not read is refused by its index, label and key with advice, and the bus
explains it with `details` (`OpEntry::locate`). Every apply's payload goes through that reader
twice (the bus's validation and the handler), so this is the before and after of the 500-node
apply, with `design.get` beside it as a control: it reads nothing new.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, release profile, in process:
`cargo test --release -p relay-core --test design -- --ignored --nocapture
measure_apply_500_and_get_5000` (each figure the median of 10 timed runs after 2 warm-ups). Load
average 2.1–2.3 while measuring.

## Before and after

The test binaries of `36c9f61` (before) and of this stream's code were built once each and run
alternately, five times each:

| path | before (5 runs) | after (5 runs) | change |
|---|---:|---:|---:|
| `design.apply`, 1 frame + 499 children | 3.68–3.75 ms (3.70) | 3.65–3.72 ms (3.68) | −0.6% |
| `design.get`, 5,000 nodes | 3.76–3.82 ms (3.82) | 3.76–3.83 ms (3.78) | −1.1% |

Figures in brackets are the median of the five medians. Both differences are inside the runs'
spread: the change costs nothing measurable.

## What it costs, and the step taken on the way

- The first version read each edit through an untagged `Incoming { Op(NodeOp), Json(Value) }`,
  so that one that does not read could be explained from its JSON on the spot. Serde then
  buffers the edit for the untagged try and again for the tagged `NodeOp`: 3.77–3.84 ms
  (3.82) against 3.69–3.75 ms (3.72), +2.7%, measured back to back the same way.
- `de_ops` now reads each edit straight into `NodeOp` and, on a failure, refuses it with only
  its index and serde's reason. The full explanation (label, key, advice, `details`) is built
  by `locate` from the payload, which the bus holds anyway, and only when validation has
  already failed. That is the `after` column.

## Guardrails

- The refusal path walks the edits until the first that does not read, then tries its keys one
  at a time in the least edit of its kind: bounded by `MAX_LENIENT_OPS` edits and a dozen keys,
  and only for a payload that is refused.
- `nearest` is a banded edit distance over at most 5,000 ids or a few dozen field names, on the
  refusal path only.
