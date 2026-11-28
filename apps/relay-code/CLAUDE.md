# Relay Code: `apps/relay-code`

Relay's phone app for coding, in Rust drawn with wgpu. The plan and every decision are Relay design
44 ("Relay Code · phone app rewrite in Rust (coding only)"): read its page 1 before changing what
the app does. What is built and how: `README.md`.

`native/` is its own Cargo workspace with its own `Cargo.lock`, not a member of the root one: run
cargo from there; plain `cargo test` at the root never builds it.

```fish
cd apps/relay-code/native
cargo test
cargo clippy --all-targets -- -D warnings
```

## Settled with the owner (2026-11-24/25, design 44)

- **Coding only.** No Tally, Arbiter, Avex, Threads, Clips, Aide, notes or extensions.
- **No Connect or Disconnect key, anywhere.** Once paired, the link connects by itself on launch,
  right after pairing, and after any drop. Only a PC that forgot the phone stops it (pair again).
  `rc-link/tests/fake_door.rs` holds this: keep it passing.
- **Route A:** one drawer like Claude's app; the repo is one chip in every top bar, and switching
  it keeps you on the same kind of page. No "default project" setting.
- **One bubble per message.** Never match a sent message to the log by its words (that is how the
  old app drew duplicates): by count now, by the PC's id once `chat.send` exists.
- **The background service is approved** (`LinkService`). The board stays (light).
- Claude's look in the house style (`theme.rs`): warm near-black, one coral accent, the agent's
  words in a serif, Geist and Geist Mono.

## Easy to break

- Nothing slow on the UI thread: every request goes through `rc_link::Link::call` and is polled
  each frame; files are written off the thread (`app::write_json`).
- One `Link` per process (`App::new`'s `OnceLock`): Android restarts the app's loop per activity
  while `LinkService` keeps the process.
- **No tap falls through** (the owner's top complaint, 2026-11-25). Anything drawn over the page
  (a top bar, the message box, a popup, the drawer, a sheet, a dialog) calls `kit::above` with its
  layer and the area it covers; a raw `ui.tap` read must go through `ui.tap_here()`. New overlays
  get a `flows.rs` test that a tap on them reaches nothing under them.
- The agent's live status, mode and questions come from `session.screen` (rendered rows), never
  from `session.scrollback` (the raw stream is ~6 MB for Claude Code's TUI).
- Every op the app sends must be in `PHONE_OPS` (`crates/relay-remote/src/wire.rs`).
