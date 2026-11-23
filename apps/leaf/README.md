# Leaf

Antho's reading app, in Rust drawn with wgpu (Tally RS's renderer), for Android and a desktop
window. Mockups: Relay design 42. **Round one is comics**: manhwa, manga and manhua that a server
fetches, follows and keeps downloaded; books (EPUB, PDF) come next.

## How comics reach the phone

```
phone (Leaf) ──https──▶ anthobot.bid/leaf ──Cloudflare Tunnel──▶ bot's Caddy ──▶ leaf-server ──▶ Suwayomi ──▶ sources
```

- **Suwayomi** (`ghcr.io/suwayomi/suwayomi-server`) runs Mihon's extensions (one per site:
  Asura Scans, MangaDex, Drake Scans), downloads chapters and converts pages to WebP: a 46.5 MB
  chapter became 6.3 MB. Its WebP quality setting is broken in 2.4 ("No compression type set!"),
  so leaf-server sets the conversion with no quality.
- **leaf-server** (`native/crates/leaf-server`) is the autopilot and the phone's only door
  (`/v1/...`, bearer token). Follow searches every source and pins the series to the one you can
  read the most chapters of in a row; every 6 h it refreshes, keeps the next 5 unread chapters
  downloaded, and once a day compares sources, moving a series when yours is down to its last
  few chapters in a row and another has more, or when yours failed three refreshes. AniList gives
  each series its other titles, so a source that names it differently is still found (MangaDex's
  "Junk to King: I Am A Scavenger" is Drake Scans' "Became the King by Scavenging"). Extensions
  update daily. Checked live 2026-11-22: Junk to King moved from MangaDex (chapters 23–44 missing)
  to Drake Scans (all 84) with chapters 1–22 still read and 23–26 already downloading.
- **The phone** (`native/crates/leaf-app`): Connect, Library, Search and Follow, Series (with
  rows for missing chapters), and a continuous vertical reader that chains chapters, caches pages
  on disk and reports progress. The last library and series are kept for offline.

## Building

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p leaf-app                                  # desktop window
cargo run -p leaf-app -- --shot out.png --screen reader --id 1   # headless shot vs a local leaf-server
tools/build-lib.sh                                     # libleaf.so (needs ~/.local/share/rust-android + NDK)
cd .. && ./gradlew assembleDebug                       # native/android/build/outputs/apk/debug/app-debug.apk
```

Shots and anything that opens a window: headless only (`--shot`), never on the desktop.

## The server on server1

`~/apps/leaf` holds `native/` (leaf-proto and leaf-server only) and `server/` (`compose.yml`,
`.env` with `LEAF_TOKEN` and `LEAF_SOURCES`). Nothing is published: leaf-server joins the bot's
network as `leaf-server`, and the bot's Caddy routes `/leaf/*` to it (Discord-bot-V2 #44).

```fish
ssh server1
cd ~/apps/leaf/server
docker compose up -d --build        # after copying a new native/ over
docker compose logs -f leaf-server
cat .env                            # the token the phone needs
```

Until leaf-server is on main with an image of its own, a new version is copied over by hand
(`tar` of `native/` without `target/`).

## Not done yet

- Books: EPUB/PDF import and the book reader, notes, review (design 42 pages 2–11).
- The phone does not yet download chapters ahead on its own while closed; it prefetches while
  reading.
- leaf-server is not built by CI or published as an image.
