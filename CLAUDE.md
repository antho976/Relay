# Working in this repository

## Verifying a change

```fish
cargo test                  # engine, bus and CLI — no display, no GTK
cargo clippy --all-targets -- -D warnings
```

Both run in CI on every push and pull request, so a change that breaks them is
caught before it is merged rather than in use.

**Do not run `cargo test --workspace` in a headless environment.** The workspace
includes `relay-native`, and building it stops at `pango-sys` long before any
test runs. Plain `cargo test` uses the default members, which are exactly the
nine headless crates (`relay-bus`, `relay-core`, `relay-cli`, `relay-remote`,
`relay-money` for Tally's money rules, `relay-arbiter` for Arbiter's trading rules, `relay-gym`
for Avex's training history, and the
native client's GTK-free logic:
`relay-board` for the board, `relay-client` for the Git panel's diff/graph/status
helpers and the device mirror's decoder gate and input payloads).
The full list in `README.md` is for a developer machine with GTK.

`crates/relay-key-proxy`, the API-key proxy (`docs/KEY-PROXY.md`), is its own workspace with its
own `Cargo.lock`, so root can build it alone: check it with
`cargo test --locked --manifest-path crates/relay-key-proxy/Cargo.toml` (and the same for clippy).

## The phone app: `apps/relay-android`

Relay's Android app (Kotlin, Compose, Room, Gradle; `docs/ANDROID.md`), independent of Cargo.
Verify a change with

```fish
cd apps/relay-android && ./gradlew :core:test :app:testDebugUnitTest :app:lintRelease
```

All three run in CI (`relay-android.yml`, only for changes under `apps/relay-android`), with both
APK builds. `:core` is plain JVM (the wire, the replica and outbox rules, the terminal emulator)
and needs no Android SDK; without one, a change to `:app` is written, not built, and the summary
must say so. Robolectric unpacks native libraries into `java.io.tmpdir`: on a machine whose `/tmp`
is full, run the app tests with `JAVA_TOOL_OPTIONS=-Djava.io.tmpdir=<somewhere with room>`.

The door it talks to is `crates/relay-remote`. Every op the app sends must be in `PHONE_OPS`
(`wire.rs`), or the door refuses it with `remote.op`; the app's own link and sync code runs
against a real door in `LiveDoorTest` (ANDROID.md, "Building and installing"). The PC is the
authority for its data: the app reads a replica and sends edits through an outbox
(`core/sync`), so an op it may queue offline needs a rule in `Optimistic` for how the row looks
until the PC answers.

## Avex: `apps/avex`

Avex, the gym app for Android and Wear OS, lives here with its own Gradle build
(`apps/avex/forge-android`) and its own instructions, `apps/avex/CLAUDE.md`: read them before
changing anything under `apps/avex/`. Run Gradle from the app's own folder, never from the root:
Avex and the Relay phone app both have an `:app` module. Its CI is `.github/workflows/avex.yml`,
only for changes under `apps/avex/`, and nothing in `cargo test` touches it. Its public build
holds no `INTERNET` permission, ever; `crates/relay-gym` reads the file it exports.

## Relay Code: `apps/relay-code`

The phone app's Rust rewrite (wgpu, coding only, Relay design 44), installed beside
`apps/relay-android` until it replaces it. Its own Cargo workspace (`apps/relay-code/native`) and
its own instructions, `apps/relay-code/CLAUDE.md`: read them before changing anything there. It
talks to the same door, so every op it sends must be in `PHONE_OPS` too.

## Tally: `apps/tally` and `crates/relay-money`

Tally, the budget tracker, is Tally RS: Rust drawn with wgpu, for Android (a GameActivity shell in
`apps/tally/native/android`) and a desktop window. It lives in `apps/tally` with its own
instructions, `apps/tally/CLAUDE.md`: read them before changing anything there. Its Cargo
workspace is `apps/tally/native`, separate from the root one, with its own `Cargo.lock`, so plain
`cargo test` here never builds it. Its CI is `.github/workflows/tally.yml`.

Tally RS builds on `crates/relay-money` through a path dependency: the money rules and the SQLite
ledger the engine runs, so the phone and the PC each hold the whole ledger (`docs/MONEY.md`) by the
same code. **A change to `relay-money` is a change to Tally too**: run Tally's tests with it
(`cd apps/tally/native && cargo test --locked`; `tally.yml` runs for `crates/relay-money` changes),
and when `relay-money`'s dependencies change, a plain `cargo test` there updates Tally's
`Cargo.lock`, which goes in the same pull request. Stored enum names stay the Kotlin app's constant
names: existing ledgers and backups hold them.

## The native client cannot be built everywhere

`apps/relay-native` pins GTK 4.22, VTE 0.84, GtkSourceView 5.18 and pango 1.56.
Ubuntu 24.04 — the usual agent-session image — carries GTK 4.14 and pango 1.52,
and no combination of `apt` packages closes that gap. CI builds it in an Arch
Linux container instead (`native client (GTK)` in `ci.yml`: clippy and its tests,
which need no display); on a machine with those libraries,

```fish
cargo clippy -p relay-native --all-targets -- -D warnings -A deprecated
cargo test -p relay-native
```

So: **if you change anything under `apps/relay-native/` in a session without
them, you cannot compile it, and you must say so plainly in your summary and in
the pull request**, and point at that CI job's result. Verifying
an API you cannot build against by reading the `gtk4`/`gdk4` crate sources under
`~/.cargo/registry` is worth doing, and is still not a compile. Do not describe
such a change as verified.

## Invariants that are easy to break

- **Handlers run inside one SQLite transaction with the store mutex held**
  (`BUS.md` §5.1). Anything slow — a subprocess, a network call, a tree walk —
  blocks every other request, keystrokes included. Register it with
  `Engine::register_unlocked` (queries) or `Engine::register_staged`
  (mutations with a slow read phase) instead of `Engine::register`.
- **Every subprocess forked from a handler goes through
  `proc::output_with_timeout`.** A child that never returns otherwise freezes
  the bus.
- **The store is one connection behind one mutex.** Reads serialize with writes.
  Prefer `prepare_cached` over `Connection::query_row`, which recompiles its SQL
  on every call.
- **`session.input` must not touch SQLite** (D148). It is the one request whose
  latency a person feels.
- Adding a column means appending a migration to `store::MIGRATIONS` and bumping
  `SCHEMA_VERSION`; every earlier version must stay openable.

## Performance work

Start from `docs/perf/PLAYBOOK.md`: Relay's journeys and budgets, the invariants above as
numbers, the harness (`scripts/perf/`) and the open leads. Every claim carries a before and an
after from the same machine and profile; readings go in `docs/perf/` in the existing format.

## Style

The workspace is **not** rustfmt-clean, in places deliberately — roughly 900
hunks differ from rustfmt's default. Do not reformat files you are otherwise not
changing, and do not run `cargo fmt` across the workspace; it would bury a real
change in noise. Match the density of the code around you.
