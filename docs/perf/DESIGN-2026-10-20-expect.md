# Design canvas: undo expectations on the apply path (2026-10-20)

`engine-expect` makes every applied `design.apply` keep what its undo expects to find (feature-agent-api
W7): each field it updated at its value now, each moved node's parent, and what each added container
holds. It computes them from the nodes the answer already reads, plus one read of the parent index when
an added node can hold children (never one walk per node). It also gives agents a short answer (W5) and
bounds a full one (B7).

The path that does the most new work is an undo that re-adds a whole design: every node is an add, and
every container among them gets a `within`. This reading times that path and the delete before it.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, release profile, in process.
`cargo test --release -p relay-core --test design_expect -- --ignored --nocapture
measure_an_undo_of_a_5000_node_delete`. Load average 2 to 4 while it ran.

The harness: a page holding 4,999 top-level nodes, one in five a frame. Each round deletes them all in
one lenient list (full answer, as the person's canvas asks) and undoes it with the answer's inverse and
expectations. Each figure is the median of 5 timed rounds after 2 warm-ups, one run each.

| path | before (`edf2cd7`) | after | change |
|---|---:|---:|---:|
| delete of 4,999 nodes, full answer | 30.5 ms | 31.4 ms | +0.9 ms (+3%) |
| undo re-adding them, with expectations | 40.8 ms | 47.2 ms | +6.4 ms (+16%) |

The delete's change is within the spread these runs show elsewhere. The undo's is not. It is the new
`expect` of the undo itself: 1,000 `within` lists built from one parent-map read, serialized for the
audit's size check and again into the full answer.

Both are single runs of one build each, so treat the +16% as a lead rather than a settled cost. It is
the rarest apply there is (a whole design back in one step), and it stays well under 100 ms. An
ordinary edit's added work is one `Expect` per touched node. Leads for the `perf` stream:

- Serialize the undo and its expectations once, and reuse the bytes for the audit row.
- Leave `within` out for a container that holds nothing, since the undo deletes it whole either way.
