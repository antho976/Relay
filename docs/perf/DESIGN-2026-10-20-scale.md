# Design canvas: engine scans and scene commits at scale (2026-10-20)

The `perf` stream of design round two: the design engine's and the canvas's paths measured at
500 to 5,000 nodes, guarded by a ratchet, and fixed where a reading was over budget (W44, B58,
B85, TM-80 part a). Two harnesses, both new:

- **Engine and scene rows in `crates/relay-core/examples/perf.rs`** (W44, TM-80a): 37 rows with the
  harness's usual instruments, of which this reading uses the instruction count (`--counters`,
  the measured call alone, every thread it touches) and the wall clock. A new `--sql` pass lists
  each row's statements by text, how often they ran and SQLite's own time for them.
- **The native client's CPU per edit, `scripts/perf/design-scale.py`**, in place of teal-moose's
  lost `bigperf.py`: relay-native inside `kwin_wayland --virtual`, one `design.apply` every 200 ms
  as another window sends it, 40 edits.

AMD Ryzen 7 9800X3D (8 cores, 16 threads), CachyOS kernel 7.2.9, governor `performance`, rustc
1.99.0, GTK 4.24.1, **dev profile** (`opt-level` 1 for the workspace, 3 for dependencies, what
`run.sh` launches), in process. The fixture's stores are on `/tmp`, which is RAM here.

- Engine rows before: `2393d3b` (the harness on the integration branch, nothing else changed),
  load average 13 to 16, other sessions building.
- Engine rows after: `9fb3426`, load average 2 to 3.
- Instruction counts are what to compare: three runs of one build agree within 0.7% on every row,
  and the load does not move them. The wall clock is given beside them, but before and after ran
  at different loads.

## The rows

The designs live in an engine of their own, built the first time a design row runs and put back
before each case, so the other rows' store is unchanged:

- **`500`, `2000`, `5000`**: a page and frames of 49 children each (one in three a text), up to that
  many nodes. Frame `f0`'s children are pinned to its right and bottom edges.
- **`1721`, `4903`**: `design-smoke.py perf`'s design (phone artboards in auto layout; each a title
  and ten rows of a text and a toggle) with 40 and 114 artboards. These are the sizes the
  2026-10-16 reading and teal-moose's TM-80 measured.
- **`siblings`**: a page and 4,998 top-level nodes, one in five an empty frame.

Every edit is sent as the person's canvas sends it: the user, a full answer, and an undo with the
answer's inverse, lenient, and its `expect`. A row that adds at the 5,000-node cap first deletes,
uncounted, what it adds back, so the design holds 5,000 when the edit is done. The 1,000-edit
batch and its undo are 20 frames of 49 children; at 5,000 these are the design's last 20 frames.

Each figure is the median over three runs of the per-iteration median. Iterations per run are 30,
or 6 for the large lists.

| row | before | after | change | wall before | wall after |
|---|---:|---:|---:|---:|---:|
| `apply.update.500` | 0.46 M | 0.43 M | −8.0% | 0.059 ms | 0.042 ms |
| `apply.add.500` | 0.47 M | 0.44 M | −6.7% | 0.070 ms | 0.043 ms |
| `apply.move.500` | 0.54 M | 0.51 M | −6.1% | 0.062 ms | 0.044 ms |
| `apply.delete_subtree.500` (50 nodes) | 8.00 M | 7.21 M | −9.9% | 0.599 ms | 0.439 ms |
| `apply.batch_1000.500` | 172.85 M | 159.30 M | −7.8% | 16.9 ms | 10.6 ms |
| `apply.undo_1000_adds.500` | 166.78 M | 128.09 M | −23.2% | 12.1 ms | 8.3 ms |
| `apply.resize.500` | 0.78 M | 0.74 M | −4.9% | 0.094 ms | 0.054 ms |
| `apply.resize_pinned.500` | 11.06 M | 9.79 M | −11.5% | 0.851 ms | 0.677 ms |
| `get.500` | 10.82 M | 10.82 M | 0.0% | 0.739 ms | 0.697 ms |
| `apply.update.2000` | 0.46 M | 0.43 M | −8.1% | 0.045 ms | 0.039 ms |
| `apply.add.2000` | 0.47 M | 0.44 M | −6.6% | 0.059 ms | 0.043 ms |
| `apply.move.2000` | 0.55 M | 0.51 M | −6.1% | 0.058 ms | 0.044 ms |
| `apply.delete_subtree.2000` | 14.32 M | 13.52 M | −5.5% | 0.831 ms | 0.750 ms |
| `apply.batch_1000.2000` | 179.67 M | 166.14 M | −7.5% | 13.4 ms | 11.0 ms |
| `apply.undo_1000_adds.2000` | 180.58 M | 141.41 M | −21.7% | 15.2 ms | 9.1 ms |
| `apply.resize.2000` | 0.80 M | 0.76 M | −4.9% | 0.069 ms | 0.057 ms |
| `apply.resize_pinned.2000` | 11.07 M | 9.76 M | −11.9% | 0.897 ms | 0.670 ms |
| `get.2000` | 42.39 M | 42.39 M | 0.0% | 3.45 ms | 2.70 ms |
| `apply.update.5000` | 0.47 M | 0.43 M | −8.5% | 0.070 ms | 0.038 ms |
| `apply.add.5000` | 0.47 M | 0.44 M | −6.6% | 0.120 ms | 0.044 ms |
| `apply.move.5000` | 0.55 M | 0.51 M | −6.0% | 0.070 ms | 0.043 ms |
| `apply.delete_subtree.5000` | 26.88 M | 26.08 M | −3.0% | 1.60 ms | 1.16 ms |
| `apply.batch_1000.5000` | 184.15 M | 170.65 M | −7.3% | 14.2 ms | 11.2 ms |
| `apply.undo_1000_adds.5000` | 197.52 M | 157.69 M | −20.2% | 14.3 ms | 9.9 ms |
| `apply.resize.5000` | 0.80 M | 0.76 M | −4.6% | 0.063 ms | 0.056 ms |
| `apply.resize_pinned.5000` | 11.10 M | 9.80 M | −11.7% | 0.863 ms | 0.683 ms |
| `get.5000` | 105.37 M | 105.36 M | 0.0% | 8.55 ms | 6.71 ms |
| `apply.group_1999_of_4998` | 9,701.84 M | 454.14 M | −95.3% | 421.6 ms | 29.8 ms |
| `apply.ungroup_1999_of_4998` | 9,171.00 M | 621.53 M | −93.2% | 467.8 ms | 34.4 ms |
| `apply.delete_all_4998` | 696.79 M | 609.23 M | −12.6% | 54.8 ms | 42.0 ms |
| `apply.undo_delete_all_4998` | 824.93 M | 743.92 M | −9.8% | 70.8 ms | 51.4 ms |
| `path.design.commit.1721` | 6.77 M | 6.78 M | +0.1% | 0.556 ms | 0.545 ms |
| `path.design.reread_delta.1721` | 17.13 M | 17.13 M | 0.0% | 1.22 ms | 1.13 ms |
| `path.design.reread_whole.1721` | 128.59 M | 128.58 M | 0.0% | 9.62 ms | 8.65 ms |
| `path.design.commit.4903` | 19.11 M | 19.12 M | +0.0% | 1.65 ms | 1.55 ms |
| `path.design.reread_delta.4903` | 48.60 M | 48.60 M | 0.0% | 3.82 ms | 3.18 ms |
| `path.design.reread_whole.4903` | 367.09 M | 367.06 M | 0.0% | 31.2 ms | 25.9 ms |

The engine rows are `op.design.*`.

An edit of one node costs the same at 500 and at 5,000 nodes: update, add, move and resize do not
grow with the design. Two rows grow with it:

- **`design.get`** reads every row, as it must.
- **A subtree delete** grows from 7.2 M to 26.1 M for the same 50 nodes. After any delete,
  `release_ends` reads every body in the design that names a connector end
  (`instr(body, '"from":')`). That read is 0.8 ms of SQLite time at 5,000 nodes. The whole edit
  is 1.2 ms, under budget, so it is left alone (W46's connector index is deferred).

## B58: group, ungroup, and deleting everything

The budget is the engine's own: a handler holds the store lock for at most 16 ms (PLAYBOOK §1),
since every other request waits behind it. Grouping 1,999 of 4,998 siblings took 422 ms of lock,
and ungrouping them 468 ms. Both are a select-all and Ctrl+G on a large page. The 2,000-edit list
is the most one list takes. The `--sql` pass showed where the time went (raw runs `sql-b58-*`):

1. **The nesting check's walk down scanned the design.** A move into a deeper place asks how deep
   the moved node's own subtree goes (`HEIGHT`, a recursive CTE). SQLite planned the recursive step
   with `design_nodes` as the outer loop, searching it by `design_id` alone. Each step read every
   row of the design: about 250 µs for a leaf at 5,000 rows. In a SQLite test of the same schema
   and 5,000 rows, a frame holding 900 children took 226 ms. In the group, 1,999 of these took
   502 ms of SQLite time.
   - The fix: `CROSS JOIN` keeps the CTE outer, so each step is one search of
     `design_nodes_parent`. A unit test checks the plan.
   - Group: 9,702 M → 2,011 M instructions, 422 → 101 ms.
2. **Counting and walking siblings per edit.** Two queries cost O(siblings) on every edit:
   - each move and each delete counts the siblings below its node, for its undo (`index_of`, a
     `COUNT` over the parent index);
   - each add or move at an index walks to it (`OFFSET`).

   A list moving 2,000 nodes out of a parent of 5,000 ran 1,999 counts, at about 50 µs each.
   - The fix (`Siblings` in `apply.rs`): after two asks of one parent, the list reads that
     parent's children once, in paint order, and keeps them as its own adds, moves, deletes and
     renumbering change them.
   - A copy writes its own rows, so it lets go of its parent's list through `slot`.
   - A list that asks after a parent once or twice queries SQLite as before. So a single edit
     costs the same (within +0.6% in the runs taken just before and after this one change).
3. **Renumbering in a loop.** An ungroup puts node after node into one gap, and so does the undo of
   a group. Each node went halfway between its neighbours, so the gap was spent every 30 nodes,
   and then every sibling was renumbered: 264,264 row updates for one ungroup.
   - The fix: renumbering now spaces siblings 1024 apart. A node goes one step from the node the
     list put there last, above or below it, so a gap holds a thousand.
   - After: 7,113 updates (two renumberings).
   - A unit test runs a 205-edit list in one parent through a renumbering, and checks the
     engine's order against the client's `Scene` and the undo against the order before.
   - Group 2,011 M → 494 M (36 ms); ungroup 9,171 M → 660 M (42 ms).
4. **The audited result was copied.** The pipeline cloned every audited handler's result to build
   the audit entry, then returned the original.
   - The fix (`engine.rs`): the result moves into the audit outcome and back out to the reply.
   - Every audited op takes this path: −6.9% instructions on a one-node update, −7.0% on the
     1,000-add batch, −9.0% on the 4,998-node delete.
5. **The undo was serialized only to be measured.** `apply.rs` wrote the undo and its
   expectations into a buffer only to learn whether they fit the audit's 256 KiB. It now counts
   the bytes as they are written: −0.5% to −3.2% on the large lists.

What is left over the 16 ms budget is linear in the size of the list, not in the size of the design.
These are the largest lists there are, all in the dev profile:

| list | after | per edit |
|---|---:|---:|
| group of 1,999 | 29.8 ms | 227 k instructions |
| ungroup of 1,999 | 34.4 ms | 311 k |
| delete of 4,998 | 42.0 ms | 122 k a node |
| its undo | 51.4 ms | 149 k a node |

For comparison, the 1,000-add batch is 11 ms. The rest is the work of each edit:

- up to five row reads a move;
- each node's body parsed once and serialized about four times: into the undo's `Value`, the
  audit row, the answer's `Value` and the socket.

**Not done: the audit summary.** `crate::audit::append` serializes the whole result to learn that
it is over 64 KiB, then stores a marker. With that serialization skipped, as an experiment that was
not kept (`experiment-summary-*`), the delete of 4,998 costs 5.8% less and the 1,000-add batch
4.6% less. A writer that stops at 64 KiB would keep most of that, at the price of the marker's exact
`bytes`. `src/audit.rs` is not this stream's file. `handlers/audit.rs`, which is, serializes
nothing on this path. This is the first lead for whoever owns the audit log.

## B85: the canvas's commit and reread

A commit is the canvas running its own edit before sending it: `fit`, `expect_for` and
`apply_effective`, which clones the scene. A reread is what follows another window's edit:
`design.get since_rev`, the wire, decoding, and the scene cloned and patched (or a whole read and
`Scene::new`). The native client's diff of the old scene and the new one lives in relay-native and
is not in the harness. The client reading below has it.

| path | 1,721 nodes | 4,903 nodes |
|---|---:|---:|
| commit (one fill) | 0.55 ms | 1.55 ms |
| reread, what changed since | 1.13 ms | 3.18 ms |
| reread, whole | 8.65 ms | 25.9 ms |

**No journal in `Scene::apply_all`.** The commit is 1.55 ms at 4,903 nodes. Most of that is the
clone a journal would save. It runs once per gesture, not per frame. It is a small part of the
frame that follows an edit at that size: about 1.4 ms of layout, 3.4 ms of content and 4.8 ms of
draw (below). A whole reread at 4,903 nodes would not fit a frame. The client reads whole only to open a
design, after `design.since_too_old`, or when a delta does not fit.

## TM-80: what another window's edit costs the client

`scripts/perf/design-scale.py`, three runs of each build, alternated before, after, before, after,
at load average 1.4 to 2.6:

- **Before**: `acfb1a7`, the integration branch before client-layout and client-scene merged.
- **After**: this branch, with native-gallery's B53 gate, native-doc's delta reread through
  `Scene::patch`, and the FxHash maps of client-scene and client-layout.

CPU per edit with the rest rate taken out, as teal-moose measured it:

- the GTK main thread's, from its schedstat;
- the process's, from `/proc/<pid>/stat`.

Figures are the median of three runs, with the range in brackets.

| nodes | view | UI thread before | UI thread after | process before | process after | teal-moose (UI / process) |
|---:|---|---:|---:|---:|---:|---|
| 1,721 | gallery only | 11.5 (11.3–11.6) | 7.8 (7.7–8.0) | 20.0 (19.5–20.3) | 16.0 (15.8–16.0) | 16.0–17.5 / 29.8–31.3 |
| 1,721 | board open | 16.2 (16.2–16.9) | 4.4 (4.2–4.4) | 27.5 (27.5–28.3) | 4.5 (4.0–4.8) | 17.8–18.5 / 31.8–32.8 |
| 4,903 | gallery only | 25.5 (24.8–25.7) | 18.9 (18.6–19.0) | 38.5 (37.8–38.5) | 30.8 (30.8–31.3) | 30.0–30.3 / 45.8–46.0 |
| 4,903 | board open | 40.4 (40.3–41.8) | 10.7 (10.4–11.1) | 61.3 (61.0–63.0) | 10.5 (10.5–11.3) | 40.5–40.8 / 63.3–64.0 |

All figures are in ms per edit. The before build reproduces teal-moose's reading at 4,903 nodes
(40.4 against 40.5–40.8 ms). With the board open, an edit costs the client 73–74% less on its
main thread and 83–84% less in all. Nothing behind the board rebuilds or renders a thumbnail any
more: three gallery renders in 40 edits and no thumbnail. Teal-moose's profile found the hidden
gallery's cards rebuilt, and a thumbnail rendered, for every edit. At rest, with the board
up, the client uses no CPU: 0.000% to 0.001%, in every run.

At 4,903 nodes with the board open, after:

| phase | median per edit (three runs) |
|---|---|
| reread | 5.26–5.74 ms |
| — design.get since_rev | 1.85–1.93 ms |
| — scene clone and patch | 1.50–1.58 ms |
| — diff | 1.83–2.23 ms |
| layout (before) | 4.28–4.40 ms |
| layout (after) | 1.33–1.40 ms |
| content (before) | 3.70–3.85 ms |
| content (after) | 3.39–3.52 ms |
| draw (before) | 7.98–8.30 ms |
| draw (after) | 4.77–4.96 ms |

At 1,721 nodes the reread is 2.0–2.1 ms.

With the gallery alone, the remaining cost is the thumbnail. One is rendered for every edit: 41
renders in 40 edits, 19.4–19.7 ms each at 1,721 nodes and 42.6–43.0 ms at 4,903 (trace wall time,
part of it off the main thread).

## Leads from earlier streams

**engine-expect's +16% on the undo of a 5,000-node delete** (DESIGN-2026-10-20-expect.md, release,
single runs). The undo's own expectations were measured by computing none and no empty `within`,
an experiment that was not kept (`experiment-expect-*`):

| row | expectations' share | empty `within` lists |
|---|---:|---:|
| undo of the 4,998-node delete | 4.9% (36 M instructions) | 1.3% |
| 1,000-add batch | 10.5% | — |
| undo of 1,000 adds | 11.5% | — |

Where the cost goes:

- **Building them.** `undo_expect` reads the whole design's parent map and builds a children index
  as soon as the list added a container. For the batch that is 3,000 rows for 20 roots.
- **Serializing them three times.** Into a `Value` for the audit, into a buffer to size it, and
  into the answer.

The cheap part is done: sizing now counts instead of buffering (B58, step 5). Two further changes
are left to the owner of `expect.rs`:

- read each root's subtree through the parent index instead of the whole map;
- leave out `within` for an empty container. That one is not only speed: an empty `within` is
  what makes an undo refuse to delete a frame someone has since put something in.

In this harness the expectations are 4.9% of that undo, not 16%. The rest of the earlier figure
was single runs at a different load, or the release build.

**engine-pins' follow pass**, first reading: `apply.resize_pinned` against `apply.resize`. This is
one frame resized, its 49 children pinned to its right and bottom edges, against the same resize
with nothing pinned.

| | instructions | wall |
|---|---:|---:|
| resize, nothing pinned | 0.74–0.76 M | 0.054–0.057 ms |
| resize, 49 pinned children | 9.76–9.80 M | 0.67–0.68 ms |

About 185 k instructions (12 µs) a follower: the price of an update edit, which is what each
follow is. The cost does not grow with the design (500 to 5,000 nodes alike) and is under any
budget. The pass's `instr(body, '"pin_')` read touches only the resized frame's children.

## Guardrails

- **`scripts/perf/ratchet.py`** (ceilings in `scripts/perf/ratchet.json`):
  - It takes three runs of the 37 rows, and fails when a row's median instruction count is over
    its ceiling by more than 1.5%.
  - `--lower` writes every row that beat its ceiling; a ceiling is raised only by hand, with the
    reason in the commit.
  - On the before runs it fails on 28 of 37 rows. On this branch it passes.
  - It needs the CPU's counters, which CI's virtual machines do not have. It is a check to run
    before merging a change to the design engine or the scene, like the rest of `perf.rs`. Wiring
    it into CI needs a runner with a PMU, or callgrind on CI's generic glibc (BASELINE-2026-10-14).
- **The `--sql` pass** shows a statement count per row. The counts are deterministic: 18,011 for
  the group, 19,132 for the ungroup.
- **Unit tests in `apply.rs`**: the walk down's query plan, and a long list's order checked
  against the client's `Scene`.
- **`scripts/perf/design-scale.py`** reruns the client reading. It needs the native build and a
  virtual compositor.

## Open leads

1. **The audit summary**: `crate::audit::append` serializes a large result only to drop it. That is
   5.8% of a 4,998-node delete (B58 above).
2. **The canvas's reread at 4,903 nodes is 5.3–5.7 ms.** Two parts of it scale with the design,
   both in `doc.rs`:
   - **The clone before the patch (1.5 ms).** `Scene::patch` already refuses a delta that does not
     fit and leaves the scene as it was. Patching the scene in hand would drop the clone, if the
     replay of held edits (`rebase`) allows it.
   - **The diff (1.8–2.2 ms).** It compares every node, where a delta names the changed ones.
3. **The gallery renders a thumbnail for every edit** of a design while the gallery shows: 43 ms
   at 4,903 nodes. A debounce of a second would leave one per burst of edits (native-gallery).
4. **The largest lists are still over 16 ms of lock**, linear in their length. A 5,000-node delete
   reads, parses and serializes each node four or five times. One serialization of the undo, kept
   as bytes for the audit row, would need `UndoOp` to carry raw JSON.
5. **A subtree delete reads every connector body in the design** (`release_ends`): 0.8 ms at 5,000
   nodes, the cost W46's connector index would remove.

## Raw runs

`docs/perf/runs/2026-10-20-scale/`:

- `before-{1,2,3}.jsonl`, `after-{1,2,3}.jsonl`: `perf run 'op.design*' 'path.design*'
  --counters`; the load at their end is in `before-load.txt` and `after-load.txt`.
- `client-{before,after}-{1,2,3}.jsonl`: `design-scale.py`, alternated; the load is in
  `client-load.txt`.
- `sql-b58-{before,height,siblings}.jsonl`: the B58 rows with `--sql`, taken before any fix, after
  the HEIGHT fix, and after the siblings fix. The `--sql` pass's own callback slows the wall clock;
  read its counts.
- `experiment-expect-{normal,none,nowithin}.jsonl` and `experiment-summary-{kept,skipped}.jsonl`:
  the experiments above. Those builds were measured, not kept.
