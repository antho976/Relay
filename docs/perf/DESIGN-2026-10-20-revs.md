# Design canvas: revisions on the apply and get paths (2026-10-20)

`engine-revs` put the round-two revisions to work (feature-agent-api W6, W20, W21; TM-58):

- `design.apply` with an older `base_rev` reads what changed since and applies on top.
- `design.get since_rev` answers only what changed.
- `design.changed` names the ids an apply touched.
- An edit that changes nothing makes no rev.

Neither op's ordinary path does anything new: a list without `base_rev` reads nothing more, and
a whole `design.get` reads the same columns as before. So this reading is a before and after of
those paths, plus a first reading of the two new ones.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, release profile, in process.

- Before and after: `cargo test --release -p relay-core --test design -- --ignored --nocapture
  measure_apply_500_and_get_5000`.
- New paths: `cargo test --release -p relay-core --test design_revs -- --ignored --nocapture
  measure_since_rev_and_base_rev_on_5000`.

Load average was 2 to 4.5 while these ran.

## Before and after

`df26545` (before, the integration branch with engine-integrity) and this stream's code (after).
Each test binary was built once and the two were run alternately.

Instructions, `perf stat -e instructions:u` over the whole test process (its setup, 22 applies
of 500 nodes, and 12 reads of 5,000 nodes), five runs each:

| | before | after | change |
|---|---:|---:|---:|
| whole harness | 2,821.2–2,822.2 M (2,821.3) | 2,812.2–2,812.8 M (2,812.5) | −8.8 M (−0.31%) |

Figures in brackets are the medians. A sampled profile (`perf record -e instructions:u`) puts the
difference in the allocator: 15.4% of samples in `_int_malloc` before, 14.5% after. The release
binaries carry no symbols, so it is not traced further. The new bookkeeping on these paths
costs less than that:

- the event's id lists, 100 ids an apply;
- six flags per row in `design.get`'s ordering.

Wall clock, each figure the median of 10 timed runs after 2 warm-ups, eight alternating runs
each:

| path | before (8 runs) | after (8 runs) | change |
|---|---:|---:|---:|
| `design.apply`, 1 frame + 499 children | 3.94–4.04 ms (3.98) | 3.93–4.07 ms (3.96) | −0.5% |
| `design.get`, 5,000 nodes | 3.96–4.04 ms (4.02) | 4.02–4.12 ms (4.07) | +1.2% |

Figures in brackets are the median of the eight medians. Both changes are within the spread of
the runs. An earlier set at load average 8 to 12 had the two the other way round.

## The new paths

The most a design holds: a page, 10 frames and 4,989 children.

- Before each timed read, one update changes one node.
- The reads ask for what changed since the rev before that update.
- The apply sends one update with `base_rev` one behind. It writes nothing that changed, so it
  applies with `rebased: true`.

Each figure is the median of 10 timed runs after 2 warm-ups. The range and the bracketed median
are over eight runs of the binary.

| path | 8 runs | for comparison |
|---|---:|---|
| `design.get since_rev`, one node changed | 1.06–1.09 ms (1.07) | the whole read: 4.07 ms |
| `design.get since_rev` with `page`, one node changed | 1.23–1.25 ms (1.23) | |
| `design.apply`, one update, `base_rev` one behind | 43–46 µs (44) | the same update without `base_rev`: 35–37 µs (36) |

**What a reread costs.** A board that rereads after another session's edit gets the change in a
quarter of the time of a whole read, and the answer holds one node instead of 5,000. The 1.07 ms
that remains is spent in three places:

- copying every row's id, parent, order and kind under the store lock;
- building the children lists;
- the walk that gives each answered node its place among its siblings.

Cut to a page, the read copies every body: a node that moved onto the page brings what it holds,
whose rows did not change. That is the same copy a whole read makes, and only the changed nodes
are parsed.

**What `base_rev` costs.** About 8 µs per list: two indexed reads, one of the rows written since
(`design_nodes_rev`) and one of the ids removed since. The parent map is read only when a delete
meets a change.

## Guardrails

- `base_rev`, when the list writes anything:
  - one read through `design_nodes_rev` of the rows written since;
  - one read of the design's removals since (at most the window's);
  - only when a delete meets a change, one read of the parent index, with walks bounded at
    `MAX_DEPTH + 6` levels.
- `base_rev` on a list that only adds reads nothing.
- `since_rev` is unlocked as before: the lock is held only to copy rows. The cost lives in that
  copy and in the ordering that runs after the lock.
- Tombstones: a list that removes something deletes the removals older than `SINCE_WINDOW` revs.
  `design_removed` holds at most the last 1,000 revs' removals.
- TM-58: an update whose stored body would not change writes nothing, so a list of them makes no
  rev, event, thumbnail or undo.

## Open leads

- A `since_rev` read without `page` or `nodes` needs places only for the changed rows. It could
  read their siblings through `design_nodes_parent` instead of copying every row's place. That
  is worth doing if boards start rereading on every foreign edit (TM-80, native-doc).
