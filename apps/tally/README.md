# Tally

The budget and spending tracker for Android, in Relay's look, synced with nothing to set up.
You log what you spend in two seconds, and the app answers one question at all times: how much
room is left this month, and is the month on pace.

Tally is **Tally RS**: Rust, drawn with wgpu, over antho976/RelayV4's `relay-money` (the money
rules and the SQLite ledger). Everything lives in [`native/`](native/README.md): how it is made,
how to build it, and where the port stands. The Kotlin app (Jetpack Compose, Room, Hilt) it was
ported from left the repo after `cf4b63b`; read it there with `git show cf4b63b:app/...`.

## Sync, with nothing to do

Tally keeps its ledger on the phone and syncs it with **Relay's cloud copy on the owner's
server**: the same Render service as the Discord bot (antho976/antho-s-bot, `deploy/relay/`).
The PC's Relay syncs with that copy too, so the phone, the server and the PC hold one ledger.

1. Tally asks the server to let it in (`POST /api/tally/enroll`) only when you tap Connect,
   sending only the sha256 of a secret it keeps.
2. The bot DMs the server's owner on Discord with a four-character code the phone shows too, and
   **Allow** / **Deny**. That tap is the only step, once per phone.
3. Tally collects a Relay pairing link (`POST /api/tally/claim`), pairs with the cloud engine
   through its rendezvous (`wss://<server>/relay`), and runs its first sync, which only pulls.

The server address is built in (`https://anthobot.bid`). See `native/README.md`, "Sync", for what
the phone will and will not send.

## Build

```
cd native
cargo test                       # the app's rules
cargo run                        # the desktop window
tools/build-lib.sh               # libtally.so for arm64
cd .. && ./gradlew :rs:assembleDebug
```

## CI

`.github/workflows/ci.yml`: guard (wrapper checksum, no signing material, no build output), then
`cargo test` and clippy on the host, and an arm64 type-check of the Android library.

## Money rules

They live in RelayV4's `crates/relay-money`, shared with the Relay engine. Tally RS adds no money
rules of its own; writes relay-money has no call for yet go through `native/crates/tally-rs/src/*_store.rs`.
