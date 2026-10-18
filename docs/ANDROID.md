# Relay on Android

`apps/relay-android` is Relay's phone app: native Android (Kotlin, Jetpack Compose, Room on
SQLite), no web view, no on-device model. It replaced the Expo app that lived in
`apps/relay-mobile`. Tally is a separate app in its own repository (antho976/Tally) and is not part
of it.

It looks like the desktop app made for a phone: the same warm near-black chrome, Geist and Sora,
the r mark, the Dev | Threads switch, the sidebar (a drawer here), the status bar, the
screen-black terminal plates, and the same words. Its tokens come from `DESIGN.md` and
`apps/relay-native` (palettes from `fonts.rs`, glyph geometry from `icons.rs`, generated into
`ui/kit/Glyphs.kt`), and its fonts are the desktop's bundled files.

## One set of data

The phone is a window onto the PC and also holds the PC's data itself:

- **The replica.** Everything the PC owns that the phone shows (workspaces, projects, agents,
  tasks, modules, notes, mail, notifications, guardrail holds, threads and their messages,
  labels) is kept in the phone's Room database as the JSON the bus returned. On connecting the
  phone reads it all (`Syncer`, `Fetch.All`), then follows the PC's events: a `noun.changed`
  carrying the row is written as it is, a hint (`task_id`, `bulk`) is read again, `bus.lagged`
  reads everything again (BUS.md §1.3, §3.3). Anything a screen reads that is not an entity (git
  status, a folder, a file, usage, the budget) is cached by its op and payload, so a screen opened
  before shows its last answer with the PC away, marked "Saved on this phone".
- **The outbox.** An edit made on the phone (a task moved, a note written, mail to an agent, a
  message in a thread, a notification read, an agent asked to start) is written to the outbox
  with the bus request id it will be sent with, and shown at once as an overlay on the PC's row.
  With the PC in reach it is sent straight away; without, it waits. Because the request id is the
  idempotency key (BUS.md §5.3), an edit whose answer was lost is sent again under the same id and
  runs once. Entries go in order; a later edit that needs an earlier one's answer (a label on a task
  made offline) names it (`{"$ref": "<entry>", "path": "id"}`) and waits for it.
- **Conflicts.** Edits carry the original values of the fields they change (`expected`, as the
  desktop's editors do). If the PC changed the same field meanwhile, it refuses
  (`*.edit_conflict`) and the entry is parked, not retried: the Outbox page offers **Keep mine**
  (sent again without the check) or **Use the PC's** (dropped; the row goes back to the PC's).
  Fields nobody else touched simply merge. Other refusals are parked the same way with **Try
  again** / **Drop**. A parked entry never blocks unrelated ones behind it.

The PC stays the authority for this data: it is where the agents, repositories and worktrees
are. The phone's copy converges on it, and the phone's edits converge into it.

The rules are in the pure-JVM module (`core/`): `wire/` (the door's handshake and the bus
envelopes, byte for byte as `crates/relay-remote` and `crates/relay-bus` speak them), `link/`
(dialing every route, proof, keepalive, reconnect), `sync/` (`Ledger`, `Optimistic`, `Outbox`,
`Syncer`), `term/` (the terminal emulator) and `Hub`, which puts them together. The app module
(`app/`) adds Room, OkHttp, the screens, and the background link.

## The PC on, off, or with Relay closed

| the PC | what the phone does |
| --- | --- |
| on, Relay running (desktop open or not) | live: terminals, events, edits applied at once |
| on, Relay closed | with the door running at login (`deploy/relay-door.service`), the door starts the engine when the phone connects; the phone says "starting Relay on <PC>" meanwhile |
| asleep | **Wake the PC** sends a Wake-on-LAN packet from the phone when it is on the same network; the PC told the phone its network cards' addresses in the door's welcome, once the phone was in |
| off or out of reach | everything readable from the replica; edits wait in the outbox and go, once, when the PC is back |

The desktop app was never needed for the phone: the engine (`relay serve`) is its own process and
outlives the window (ARCHITECTURE.md). What was missing is something to start the engine after a
reboot. `relay remote serve --start-engine` is that: a small door, started at login, that starts
`relay serve` for the same instance when a paired phone is admitted and none answers, then fronts
it. The engine it starts is in its own process group and outlives the door (`KillMode=process` in
the unit), so restarting the door never ends an agent. `./run.sh` reuses the engine if it is the
binary it just built; its own door stays shut while this one holds the port, and this one fronts
it. Install:

```fish
mkdir -p ~/.config/systemd/user && cp deploy/relay-door.service ~/.config/systemd/user/
# edit ExecStart to your checkout's target/debug/relay, then
systemctl --user enable --now relay-door
```

There is no server in any of this. The routes are the person's own network, Tailscale, or a
rendezvous the person runs (docs/MOBILE.md §3–4); nothing goes through anyone else's. There is no
push service either: notifications come from the link while **Stay connected** keeps it open in
the background (an ongoing notification), and otherwise from a catch-up every half hour
(WorkManager) that reaches the PC, sends the outbox and reads what changed.

## Agents as a conversation

Tapping an agent opens it as a conversation, not a terminal: the person's messages as bubbles,
the agent's words as text, and what it did as quiet lines ("Ran `cargo test`", "Edited
`src/main.rs`") that open to show the answer's head; three or more in a row fold into one line
("Ran 3 commands, edited 2 files"). It reads the log the provider keeps anyway, Claude Code's
`projects/<cwd>/<id>.jsonl` or Codex's rollout, through `session.transcript` (BUS.md), from a
cursor: about once a second while the agent works, less while it waits. Nothing new runs on the
PC and nothing streams.

Typing still goes to the agent's terminal (`session.input`): the text, then Enter, several lines
as one bracketed paste. With nothing typed, the send key stops a working agent (Esc). When the
agent is held at a question in its terminal (a permission, a menu), the screen streams the
terminal while the question shows, reads the question off it, and offers a key per numbered
answer, and Esc. A row of keys (Esc,
Shift-Tab for the mode, arrows, Enter, `/`, Ctrl-C) covers the rest, and the terminal itself is
one tap away in the header for whatever a conversation cannot show. The conversation is kept on
the phone, so it opens with the PC away.

## On the phone itself

- **Notifications** say when an agent is held by a guardrail, blocked, or done. A held command's
  notification has **Deny** and **Allow once** (Android 12 and up asks for the phone's unlock
  first); a blocked agent's has **Reply**, which sends it priority mail it reads at its next step.
- **Share to Relay**: text from any app becomes a task, a note, a new thread, or mail to an agent.
- **Shortcuts** on the launcher icon: New agent, New task, New thread, Inbox.
- **Search** finds pages, projects, agents, tasks, notes and threads in the phone's copy, with or
  without the PC.
- **Starting an agent with the PC away** queues it: `session.create` and `session.spawn` wait in
  the outbox and run when the PC is back.

## Security

The same door and the same model as `docs/MOBILE.md` §6: a paired phone acts as `user`, only for
the ops in `PHONE_OPS`, proves its token on every connection without sending it, and the link is
not sealed after the handshake, so use Tailscale or a network you trust. The token is sealed on the
phone with a key in Android's keystore; the pairing and the replica are excluded from cloud backups
and device transfers. Wake-on-LAN targets, like the PC's name and Relay's version, are sent in the welcome, only to a
phone the door has admitted; the greeting before it names nothing.

This app adds to `PHONE_OPS`: an agent's conversation (`session.transcript`), the Threads space (`thread.*`), the ledger's reads for the Tally
panel and the Undo of an entry a thread added (`money.summary/lists/series/tx.list/invest.*`,
`money.tx.delete/restore`, `money.invest.delete/restore`), Arbiter's reads, a proposal's answer and
its halt and restart keys, and Avex's reads. A phone still cannot add, import, reset or replace
the ledger except through Tally's sync.

## Building and installing

An Android SDK (platform 37) and JDK 21; Gradle comes with the wrapper.

```fish
cd apps/relay-android
./gradlew :core:test                 # wire, sync, outbox, terminal: JVM only
./gradlew :app:assembleDebug         # app/build/outputs/apk/debug/app-debug.apk
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

`LiveDoorTest` runs the app's own link and sync code (OkHttp, the handshake, the replica on Room,
the outbox) against a real door, on the JVM, with no phone: pair with a code, let the door start
the engine, make a project and a task, edit with the link down, and check both sides agree.

```fish
relay --instance test remote serve --bind 0.0.0.0:7431 --start-engine &   # a door with no engine
set link (relay --instance test remote pair --no-confirm | string match -r 'relay://pair\S+')
RELAY_E2E_LINK=$link RELAY_E2E_REPO=/tmp/relay-e2e ./gradlew :app:testDebugUnitTest --tests '*LiveDoorTest*'
```

Run it against a throwaway engine (another instance, or `XDG_DATA_HOME` / a short
`XDG_RUNTIME_DIR` of its own): it adds a workspace, a project and a paired device there.

The application id is `com.quietsoftware.relay.android` (`.debug` for debug builds); the retired
Expo app was `com.quietsoftware.relay`, so a phone that still has it keeps both until it is
uninstalled. CI (`.github/workflows/relay-android.yml`)
runs the core tests, the app's unit tests, Lint and the APK builds, and uploads the debug APK as
`relay-android-apk`. Pair from the app's first screen with `relay remote pair` on the PC.

### The personal build

`./gradlew :app:assemblePersonal` builds Antho's own copy: the release, plus `app/src/personal`,
which pairs through his server instead of a QR code (docs/CLOUD.md, "Connecting through your
server"). The first screen has one key, Connect through my server: the server DMs him Allow on
Discord, his PC opens a pairing window and asks on its own screen, and the phone pairs. No scanning,
no typing, on the WiFi or away from it. The PC needs `relay cloud connect <server>` once.

Only that source set knows the server's address, and nothing in `main`, `debug` or `release`
refers to it: `src/standard` holds the QR pairing those builds use (`PairEntry`), and CI fails if
the debug or release APK carries the address. The personal build keeps the release application id
and signs with the release key when `keystore.properties` is complete, else with the debug key.
