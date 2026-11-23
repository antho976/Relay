# Leaf: `apps/leaf`

Leaf is Antho's reading app: Rust drawn with wgpu (like Tally RS), for Android and a desktop
window. **Books** (EPUB, PDF, text) are imported from the phone and kept in a local SQLite shelf;
**comics** are fetched by the owner's server. The approved mockups are Relay design 42 ("Leaf ·
reading app"). What is built and how: `README.md`.

## Two halves, one workspace

`native/` is its own Cargo workspace with its own `Cargo.lock`, not a member of the root one:
run cargo from there, and plain `cargo test` at the root never builds it.

| Crate | What |
|---|---|
| `leaf-proto` | The wire types the phone and the server share, and the rules both must read alike: title matching, gaps, which copy of a chapter to read. |
| `leaf-server` | The comics autopilot beside a Suwayomi it drives (`server/compose.yml`). Headless. |
| `leaf-app` | The phone app (wgpu; renderer copied from Tally RS): books (`shelf`, `epub`, `typeset`, `book`, `notes`, `stats`) and comics. |

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
```

## The promise that is easy to break

Following a series must need nothing more from the owner: the autopilot picks the source,
keeps chapters downloaded ahead, and moves a series to another source when its own runs out or
stops answering, carrying the place across. It moves only for a clear gain (`decide` in
`autopilot.rs`, with tests): never trade places between sources that keep pace, and keep the
translation being read until it runs out. A change there keeps those tests and adds one.

Suwayomi is never exposed: leaf-server is the only door, and it needs `LEAF_TOKEN` to listen
anywhere but loopback.

## Books: what is easy to break

- A place and a highlight are (section, block, byte) into the parsed EPUB, and stored that way in
  `shelf.db`: `epub.rs` must keep giving the same blocks for the same file. A change to how XHTML
  becomes blocks moves every saved place and highlight; don't make one without a migration plan.
- Nothing slow on the UI thread: a book is parsed on a worker, set in lines only near the screen,
  and every shelf write goes through the shelf's thread. Shots (`--shot`) run capped:
  `systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 target/debug/leaf --shot ...`.
- Reading time counts only while the page moves (`book::ReadClock`); don't count frames.
- Shots of books use public-domain EPUBs downloaded into `.dev/` (never committed).

## Developing against a real Suwayomi

`apps/leaf/.dev/` (gitignored) holds a local Suwayomi jar and data. Run it with its root dir there
(`-Dsuwayomi.tachidesk.config.server.rootDir=$PWD/data`), then `leaf-server` with
`LEAF_DATA=apps/leaf/.dev/leaf-data`. Never commit `.dev/`.
