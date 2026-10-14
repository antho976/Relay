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

## Tally 2 (antho976/Tally) and `crates/relay-money`

Tally, the budget tracker for Android (Kotlin, Compose, Room, Gradle), lives in its own repository,
`antho976/Tally`, with its own CI. It is not in this tree.

`crates/relay-money` is a port of that repository's `core/` with the same tests. The phone and the
PC each hold the whole ledger (`docs/MONEY.md`), so **a money rule changed on one side is changed on
the other in the same piece of work**: a PR in each repository, cross-linked, tests included.
Stored enum names are the Kotlin constant names.

## The native client cannot be built everywhere

`apps/relay-native` pins GTK 4.22, VTE 0.84, GtkSourceView 5.18 and pango 1.56.
Ubuntu 24.04 — the hosted CI runners and the usual agent-session image — carries
GTK 4.14 and pango 1.52, and no combination of `apt` packages closes that gap.

So: **if you change anything under `apps/relay-native/`, you cannot compile it,
and you must say so plainly in your summary and in the pull request.** Verifying
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

## Style

The workspace is **not** rustfmt-clean, in places deliberately — roughly 900
hunks differ from rustfmt's default. Do not reformat files you are otherwise not
changing, and do not run `cargo fmt` across the workspace; it would bury a real
change in noise. Match the density of the code around you.
