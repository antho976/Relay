# Design canvas: contrast flags in an agent's design.apply reply (2026-10-20)

An agent's applied `design.apply` is now answered with `contrast`: the contrast issues its change made
(`crates/relay-core/src/handlers/design/check.rs`). The check lays every page the list reached out
twice with the canvas's own code: once as the design is now, and once with the list's undo run on that
scene. After a `color` or `style` edit that is every page. It runs once the transaction has committed
and the store is released (`Ctx::finish_reply`), and holds the lock only for one read of the design's
rows. The person's own edits are not checked, so the canvas path is unchanged.

The path a person waits on is the store lock, and the cost lands on the agent's own reply. This reading
times both.

AMD Ryzen 7 9800X3D (16 threads), CachyOS 7.2.9, rustc 1.99.0, in process. Load average 1.6 to 2.9
while it ran. `cargo test --release -p relay-core --test design_contrast -- --ignored --nocapture
the_check_costs_on_a_full_design`.

The harness: 10 pages of 4 screens (alternately light and dark frames in column layout) holding 24
cards each. Each card holds a title, a caption in `$muted` and a labelled badge, 3,890 nodes in all.
Each timed apply changes `$muted`, so every page is checked. Each figure is the median of 15 applies.

| path | release | test profile (opt-level 1) |
|---|---:|---:|
| the person's apply (no check) | 0.03 ms | 0.03 ms |
| an agent's apply, checked | 11.5 ms | 15.2 ms |
| of which the store is held (the one read) | n/a | 0.7 ms |

The lock-held figure was taken once, with temporary timing around the read in the test profile; that
timing is not committed. The rest of the agent's figure is layout and contrast for 20 page layouts,
with nothing locked. The agent waits on it; the person, the phone and other sessions do not.

Leads, none taken:

- A `color` edit could check only the pages whose nodes name that colour (`Scene::users`), not every
  page.
- The before scene could lay out only the pages whose nodes the undo touches, and reuse the after
  layout for the rest.
