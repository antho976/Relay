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
| `leaf-server` | The comics autopilot beside a Suwayomi it drives (`server/compose.yml`). Headless: `autopilot` (the pass), `budget` (per-source requests), `thumbs`, `recs` (AniList), `webtoon` (paid episodes), `api`. |
| `leaf-app` | The phone app (wgpu; renderer copied from Tally RS): tabs Home, Search, My space (`home`, `search`, `space`); books (`shelf`, `epub`, `typeset`, `book`, `notes`, `stats`), comics (`series`, `reader`), picks (`recs`) and app updates (`update`). |

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

## The server asks each source politely (2026-11-23)

The owner's IP must not get banned. Everything in leaf-server that reaches a site through
Suwayomi (a search, `refresh`, `pages`, page pictures not downloaded, `enqueue`) or AniList goes
through `Ctx::ask` with its source's name: `Lane::Back` for the autopilot (waits its turn, skipped
while the source is paused), `Lane::Fore` only for what the phone is waiting on. A new call that
reaches a site goes through it too, a background loop never asks every series at once (`plan`
spreads the pass), and downloads go through the line (`queue_downloads`, drained per source), not
straight to Suwayomi. An error starting with `PAUSED` is not the source's failure: it never counts
toward moving a series. Chapter pictures come only from Suwayomi's downloaded files. Tests of
these rules pass time in; none sleep.

Search asks only the broad sources (`LEAF_SEARCH_SOURCES`) unless the owner asks for all, keeps
each source's answer 10 minutes, and the phone searches only after typing pauses. Promo pages are
recognised only from page bytes already passing through (`promo.rs`): never fetch a page to hash it,
except the one page the owner hides by hand.

`Series` and `Chapter` gain fields only through JSON the server adds (`paid`, `thumb`): leaf-server
builds both structs field by field (`autopilot.rs`), and leaf-app in its tests, so a new field on
them breaks both builds. The phone reads `paid` and `thumb` beside the structs, from the same JSON
(`net::Extras`, 2026-11-23); moving them into the structs is a change to both crates at once.

## Books: what is easy to break

- A place and a highlight are (section, block, byte) into the parsed EPUB, and stored that way in
  `shelf.db`: `epub.rs` must keep giving the same blocks for the same file. A change to how XHTML
  becomes blocks moves every saved place and highlight; don't make one without a migration plan.
- Nothing slow on the UI thread: a book is parsed on a worker, set in lines only near the screen,
  and every shelf write goes through the shelf's thread. Shots (`--shot`) run capped:
  `systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 target/debug/leaf --shot ...`.
- Reading time counts only while the page moves (`book::ReadClock`); don't count frames.
- Shots of books use public-domain EPUBs downloaded into `.dev/` (never committed).

## The phone's UI: settled (design 42, 2026-11-22)

- Tabs are Home, Search and My space only; Notes and Stats are not tabs (My space's ⋯ reaches
  them). My space's own tabs are Updates (first), Recent, Following, Downloads (2026-11-23). No
  source names anywhere in the UI, except Settings' Sources list, which the owner asked for
  (2026-11-23). Adding a series is not following it: the bell is.
- A download is a copy on this phone for reading offline (`offline.rs`, `offline/<chapter>/`, apart
  from the page cache, which may be trimmed): nothing downloads without being asked. What the
  server keeps (its one next chapter, chapters it fetched for the phone's line) is plumbing and is
  never shown as a download; Downloads and the rows' states come from the phone's folder alone,
  and deleting one never asks the server (2026-11-23). Its line goes one chapter at a time and
  puts a chapter the server lacks in the server's own line first, so pages come from the server's
  disk; select bars (`saved::select_bar`) are fixed chrome, tested in `flows.rs`.
- While a menu is open, the page under it takes no tap (`kit::hold`; `flows.rs` tests it). Nor
  does a tap on fixed chrome drawn after the page (the tab bar, a fixed top): `Ui::begin_frame`
  holds it and `kit::chrome` / `kit::fixed_top` give it back; new fixed chrome registers itself so.
- The spotlight is never dragged: it rests 18 s on each, slides gently, and its dots choose one.
  It is a rounded card on Home (design 42 page 15, 2026-11-23), and only covers that fill it sharp
  take a turn. No cover is drawn past 1.3× its own pixels (`images::MAX_UPSCALE`).
- My space rows and chapter rows use smaller pictures than the artboards draw (the owner,
  2026-11-23): square covers 88 (`space::ROW_H`), chapter pictures 72 × 54 (`series::PIC`).
- Chapter pictures come only from the server (`/v1/chapters/{id}/thumb`): the phone never fetches
  pages to make one. A pick (`recs`) is not in your space: its sheet adds it, bell off.
- A sheet over a page (a pick's, the app update's) holds every tap from the page under it; the
  series page's bottom key is fixed chrome (`kit::chrome`). `flows.rs` tests each.

## Developing against a real Suwayomi

`apps/leaf/.dev/` (gitignored) holds a local Suwayomi jar and data. Run it with its root dir there
(`-Dsuwayomi.tachidesk.config.server.rootDir=$PWD/data`), then `leaf-server` with
`LEAF_DATA=apps/leaf/.dev/leaf-data`. Never commit `.dev/`.
