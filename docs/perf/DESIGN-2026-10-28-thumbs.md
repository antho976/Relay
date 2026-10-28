# Design gallery: a thumbnail off the main thread's single block (2026-10-28)

The 10-28 audit (design-video, "Still open, in part") read each gallery thumbnail at 42 ms on
the GTK thread at 4,903 nodes. That figure was the whole thumbnail's wall time, `design.get`'s
round trip included. Split up, the main thread's part was 16.7 ms of CPU in one block, enough to
drop a frame whenever the gallery redraws a card after another window's edit.

The fix (relay/sharp-egret, `apps/relay-native/src/design/gallery.rs`):

- The answer is decoded into a scene on a worker thread (`gio::spawn_blocking`): `GetOut` from
  JSON, `Scene::new`, and the named colours and styles.
- Layout, paint and the texture each run as their own turn of the main loop at low priority
  (`design::yield_low`), so a frame due between them is drawn first. Paint and the texture use
  the window's GSK renderer, so they stay on the main thread.

Ask's picture (`ask.rs`) is drawn after the same yield, so the press's frame shows first. It
still renders on the main thread, with the board's renderer.

## How it was read

- `scripts/perf/design-scale.py --boards 40 114 --views gallery`: one `design.apply` every
  200 ms from another window, 40 edits, about 10 thumbnails each run.
- The `DESIGN_TRACE thumb` line now splits each thumbnail. `ms` and `fetch` are wall time.
  `decode`, `layout`, `paint` and `texture` are the CPU time of the thread that ran the step
  (`CLOCK_THREAD_CPUTIME_ID`), so a busy machine moves them less than it moves wall time.
- Before is the same trace with the decode back on the main thread and no yields between the
  steps, built from the same tree. Before and after were alternated, three runs each.
- AMD Ryzen 7 9800X3D, CachyOS 7.3.0, governor `performance`, rustc 1.99.1, GTK 4.24.1,
  **dev profile**, inside `kwin_wayland --virtual`. The load average was 10 to 17, with other
  sessions building.
- Raw rows: `runs/2026-10-28-thumbs/{before,after}-{1,2,3}.jsonl`.

## Readings

The median of three runs. Each run's figure is that run's median over its thumbnails.

| nodes | figure | before | after |
|---:|---|---:|---:|
| 4,903 | main-thread CPU per thumbnail | 16.7 ms | 10.7 ms |
| 4,903 | longest single block on the main thread | 16.7 ms | 4.4 ms |
| 4,903 | decode, off the main thread after | 6.4 ms | 6.5 ms |
| 4,903 | UI-thread CPU per edit (harness) | 5.62 ms | 4.06 ms |
| 1,721 | main-thread CPU per thumbnail | 6.5 ms | 4.7 ms |
| 1,721 | longest single block on the main thread | 6.5 ms | 2.5 ms |
| 1,721 | UI-thread CPU per edit (harness) | 2.86 ms | 2.41 ms |

- The decode's 6.4 ms has left the main thread whole.
- The main thread still spends layout, paint and texture on each thumbnail (about 2, 4 and 4.5 ms at
  4,903 nodes), but never more than one step's worth between two frames.
- A thumbnail's wall time is unchanged within the noise: 45.7 ms before, 48.3 ms after, at
  4,903 nodes. Most of it is `design.get`'s round trip.

## Left

- Layout still measures text through the board's Pango painter on the main thread. Moving it
  off would need a measurer of its own on the worker thread, which is a second copy of
  `Painter::new_layout`.
- No ratchet guards this: the harness's figures are CPU time under a shared compositor, too
  noisy to gate on. The trace's split is what to read it again with.
