# Fixes 2026-10-28: the assistant drawer's streaming reply (N7)

The assistant drawer showed a reply as it streamed the way the Threads view did before 4986bed.
Each `thread.delta` set the whole reply so far on one label (`set_text(text() + delta)`), so pango
shaped and wrapped every word of it again on every delta. Each delta also added a tick callback
and a 120 ms timer to scroll, and moved the reply and the working row to the end of the column
even when they were already there. The cost of a delta grew with the reply.

The drawer now does what the Threads view does. Deltas are appended to a string and shown once
per frame. The reply is split into labels at line breaks once the last label passes 240
characters (`threads_view::pending_split`, now shared), and only the last label is set again.
The scroll is one idle callback per frame shown, and only when the reader is at the bottom. The
working row is moved only when something came after it. The finished message is drawn as before.
`stick_to_bottom`, the tick plus the 120 ms second look, stays for whole messages and for opening
a conversation, where a card filled from a later read takes its height late.

Machine: Ryzen 7 9800X3D (16 threads), rustc 1.99.1 (Arch), GTK 4.24.1, NVIDIA (Vulkan
renderer), **dev profile** (`opt-level` 1, dependencies at 3). `perf_event_paranoid` is 2, so
`perf stat -t` counts user-space instructions on the GTK thread, and nothing is sampled. Other
sessions were building throughout, so task-clock is noisier than instructions. The client ran in
a private `kwin_wayland --virtual` (1440×900), three runs each side; the median is shown. Raw runs:
`runs/2026-10-28-stream/` (`before/`, `after/`, `look/`, `run-stream.sh`).

## The harness

`native-baseline.py --stream-only [--stream-chars 2000,8000]`. The fixture's provider now also
answers as a thread's agent (`claude -p`, stream-json). A message reading `native-stream <chars>
<chunk> <ms>` gets a Markdown reply of headings, paragraphs and bullets, in `<chunk>`-character
`text_delta` events `<ms>` apart. The engine turns these into `thread.delta` events exactly as it
does for Claude. The client sits on the Board page with the drawer opened by `RELAY_NATIVE_ASSISTANT`.
The row counts the GTK thread over the stream and its settle, and reads `GDK_DEBUG=frames` for every
frame drawn between the send and 0.5 s after the reply ends. `frame_end` is a frame's time from its
start to the end of its paint. Here the replies are 8-character deltas 10 ms apart: 250 deltas for
2,000 characters, and 1,000 for 8,000.

## Reading

| row | | before | after |
|---|---|---:|---:|
| 2,000 characters (2.6 s) | GTK-thread instructions | 8.54 G | **1.54 G** (−82 %) |
| | task-clock | 641 ms | **172 ms** |
| | frame_end p50 / p95 | 2.4 / 7.1 ms | **0.9 / 1.4 ms** |
| | frames over 16.7 ms | 0 | 0 |
| 8,000 characters (10.1 s) | GTK-thread instructions | 96.6 G | **6.24 G** (−94 %) |
| | task-clock | 6,594 ms | **627 ms** |
| | frames drawn | 457 | **602** |
| | frame_end p50 / p95 | 13.4 / 28.0 ms | **0.9 / 1.4 ms** |
| | frames over 16.7 ms | 155 | **1** |
| idle (drawer open, no reply) | GTK-thread instructions | 67.5 k | 66.0 k |

Per delta, over idle: 34 M instructions at 2,000 characters and 97 M at 8,000 before. After, it is
6.2 M at both lengths, so the cost of a delta no longer grows with the reply. Before, an 8,000-character reply kept the GTK
thread busy for 65 % of its 10 s. A third of its frames went over budget, and the window drew 457
frames where 600 were due. That is what a person sees as a stutter while a long reply arrives,
with keystrokes into a terminal waiting behind it.

The one frame over budget that remains (31.6–31.9 ms, all three runs) is mostly layout and falls
at the end of the stream. That is where the finished reply replaces the streamed text with its
Markdown blocks, which this change does not touch. The before runs' worst frame was the same size
(32.3–33.1 ms).

## It looks the same

The streamed text keeps its look. A boundary between two labels stands for one line break, with
no spacing between them, so the lines fall where one label holding the whole reply would put
them. The Threads view has relied on this since 4986bed. To show it, the provider can hold a
streamed reply unfinished (`RELAY_PERF_STREAM_HOLD=6`, a fourth number in the message), and the
screenshot is taken during the hold. `RELAY_PERF_NATIVE` runs a build kept aside against the same
fixture.

With both builds, the 2,000-character reply caught mid-stream (`look/drawer-mid-stream-*.png`)
is identical pixel for pixel in the drawer, except for the overlay scrollbar's 4 px column. That
column differs by two levels of grey (41 against 39), the scrollbar fading at a different moment.
The text, its wrapping, the working row and the Stop key are the same, and both builds have
followed the reply to the bottom.

## Checks

- `cargo clippy -p relay-native --all-targets -- -D warnings -A deprecated`: clean.
- `cargo test -p relay-native`: 294 passed. `pending_split` keeps its tests in `threads_view.rs`
  (a streamed reply splits only at line breaks and loses nothing).
- `cargo clippy --all-targets -- -D warnings`: clean. No headless crate changed, so `cargo test` was
  not rerun.
