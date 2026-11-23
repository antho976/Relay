# Leaf: `apps/leaf`

Leaf is Antho's reading app: Rust drawn with wgpu (like Tally RS), for Android and a desktop
window. Books (EPUB, PDF) are planned; **comics come first**, fetched by the owner's server. The
approved mockups are Relay design 42 ("Leaf · reading app"). What is built and how: `README.md`.

## Two halves, one workspace

`native/` is its own Cargo workspace with its own `Cargo.lock`, not a member of the root one:
run cargo from there, and plain `cargo test` at the root never builds it.

| Crate | What |
|---|---|
| `leaf-proto` | The wire types the phone and the server share, and the rules both must read alike: title matching, gaps, which copy of a chapter to read. |
| `leaf-server` | The comics autopilot beside a Suwayomi it drives (`server/compose.yml`). Headless. |
| `leaf-app` | The phone app (wgpu; renderer copied from Tally RS). |

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

## Developing against a real Suwayomi

`apps/leaf/.dev/` (gitignored) holds a local Suwayomi jar and data. Run it with its root dir there
(`-Dsuwayomi.tachidesk.config.server.rootDir=$PWD/data`), then `leaf-server` with
`LEAF_DATA=apps/leaf/.dev/leaf-data`. Never commit `.dev/`.
