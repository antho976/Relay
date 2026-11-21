# Tally: `apps/tally`

Tally is Antho's budget and spending tracker: **Tally RS**, Rust drawn with wgpu, for Android
(`com.quietsoftware.tally.rs`) and a desktop window. It lived in `antho976/Tally` until 2026-11-20
and moved here with its history; old pull request numbers in commit messages read
`antho976/Tally#N`. The Kotlin app it was ported from left after `c8aa08f`
(`git show c8aa08f:apps/tally/app/...`). What it is made of, how to build it and where the port
stands: `native/README.md`. How screens are ported: `native/PORTING.md`. The look: `DESIGN.md`.

## Building and verifying

`native/` is its own Cargo workspace with its own `Cargo.lock`, not a member of the root one: run
cargo from there, and plain `cargo test` at the root never builds it.

```fish
cd apps/tally/native
cargo test --locked
cargo clippy --locked --all-targets
```

The phone build: `tools/build-lib.sh` (from `native/`; it needs the Rust with Android's standard
library in `~/.local/share/rust-android` and the NDK) packs `libtally.so`, then `./gradlew
:rs:assembleDebug` from `apps/tally`. Gradle runs from `apps/tally` only; its one module is `:rs`.

CI is `.github/workflows/tally.yml` at the repository root, for changes under `apps/tally/` or
`crates/relay-money/`.

## The money rules are `crates/relay-money`

Tally RS adds no money rules of its own: the rules and the SQLite ledger are `crates/relay-money`
(a path dependency), the crate the engine runs, so the phone and the PC read the same rows to the
same numbers. A change to relay-money is tested on both sides in the same pull request, and when
its dependencies change, `cargo test` here updates this `Cargo.lock`, which goes in that pull
request too. Writes relay-money has no call for yet go through `Store::sql()` in `*_store.rs`, each
tested by reading back through relay-money; they belong in relay-money eventually.

## The promise that is easy to break

Sync (`sync*.rs`, `native/README.md`, "Sync") must never replace the server's ledger: the first
sync only pulls, `sync_ledger.rs` makes a replace unrepresentable, and tests check it against a
fake server. Tally RS never asks to connect by itself. Keep both true in any sync change.
