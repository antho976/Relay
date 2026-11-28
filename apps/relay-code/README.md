# Relay Code

Relay's phone app for coding, redone from scratch in Rust drawn with wgpu (asked for 2026-11-24:
the Kotlin app at `apps/relay-android` was slow to get around, drew duplicate chat bubbles, could not
change model or run a command, and opened designs on the whole canvas). The plan, the mockups and
every decision are Relay design 44, "Relay Code · phone app rewrite in Rust (coding only)": page 1
holds decisions D1 to D13 and the build order, pages 2 to 4 the screens. It installs beside the
Kotlin app as **Relay Code** (`com.quietsoftware.relaycode`) until it covers what that app does for
coding; then `apps/relay-android` goes.

## What is built

| Piece | What |
|---|---|
| `crates/rc-link` | The link to the PC's phone door (`crates/relay-remote`), a port of the Kotlin app's `Wire.kt`, `PairLink.kt`, `Handshake.kt`, `BusSession.kt`, `Link.kt` and `ServerConnect.kt`. Pairing with a link, or through the owner's server (Allow on Discord). **No Connect key**: once paired, the link connects when the app starts, right after pairing, and again by itself after any drop (backoff 1 to 30 s, at once when the app comes back or the network changes). Only a PC that forgot the phone stops it. Request ids are UUIDs, as the engine reads and answers them. |
| `crates/relay-code` | The app. `pair` (first screen), `home` (the repo's agents), `drawer` (route A: repo, New agent, Needs you, agents, Changes, Designs, Board, Inbox, the PC at the foot; the repo sheet; the New agent sheet), `chat`, `changes` (E5), `board` (and one task), `inbox` (agents that ask, held commands, review), `designs` (E1, E3, E4), `settings`. `data` reads the conversation (tested), `screen` reads the agent's terminal foot (status, mode, question; tested), `attach` sends photos. Renderer, kit and front ends from Leaf (`apps/leaf`), which took them from Tally RS. |
| `native/android` | GameActivity plus two Java classes: `CodeActivity` (Back, clipboard, notifications, the photo picker) and `LinkService`, the foreground service the owner approved (design 44, Q2), which keeps the process and the link alive in the background and redials when the network changes. |

**The chat** draws your words in a bubble, the agent's in a serif, tool calls folded into one line,
every edit's diff under it (wrapped, the first lines shown, the rest a tap away), and its thinking
shown, not hidden. Live, as the terminal shows it: the mark breathing, Claude Code's own status
word, elapsed time and tokens, the permission mode (a chip; a tap is Shift-Tab), and a question it
waits on as a card with its real answers. Those come from the terminal's foot through the engine's
`session.screen` (rendered rows, not the 6 MB raw stream); a PC too old for it gets elapsed time
and Yes (Enter) / No (Esc). The model chip opens a picker card over it: the model (`/model`) and the effort (`/effort`; a slash
command the log never shows is let go after 10 s, so it cannot wait forever); `/` and `!` list
commands; `+` attaches a photo (shrunk to 2048 px, sent with `session.upload`, its path put in the
message). The conversation is read every 0.7 s while the agent works, and at once when the PC says
the agent started or stopped (`session.changed`). A finger held on a message copies it, a code block has its own copy key, and a link (a
`[words](https://…)` or a bare web address) opens in the browser on a tap; scrolled up,
a key above the box goes back to the newest. Home ends in a box that starts an agent in the repo.

**Cost per frame.** The chat reads the log into items and measures them only when the log or a fold
changes, and then only what is new; a frame draws what is on screen. With 400 messages (each with
thinking, an edit's 60-line diff and a reply), on the desktop's CPU in release: a frame went from
22.7 ms to 0.26 ms, a frame after a new message from 10.4 ms to 0.36 ms
(`cargo test --release chat_frame_cost -- --ignored --nocapture`). The conversation is saved for
the next start only when something came in, serialized off the UI thread.

**Taps never fall through.** Every surface draws on a layer (`ui::layer`: content, chrome, popup,
drawer, sheet, dialog) and marks what it covers; a tap reaches a control only when no higher layer
covers that point (`ui.rs`, "Layers"; `flows.rs` holds it for sheets, the drawer and the picker).

**Designs** are laid out by the PC's own rules (`relay-client`) and painted on the CPU on the
designs screen's worker thread (`design_paint`), never in a frame. Image nodes show their real
pictures: a second thread, the fetch thread, reads each with `design.asset.get` (in pieces, the open
design's before the list's), decodes PNG, JPEG (`jpeg-decoder`, which decodes a big photo straight
at 1/2, 1/4 or 1/8), WebP and a GIF's first frame, shrunk to what the page is ever drawn at, and
keeps them for the session (64 MB, the least lately drawn let go first). A page shows at once with
tiles; each picture repaints as its photos come.

### Not built yet

The PC-side `chat.send` that gives each message an id (D6; until then a sent message is matched to
the log **by count**, never by its words, so it cannot show twice). On designs, SVG pictures stay
tiles, and blur, glows, blend modes and motion are left out (the line under a page says what it
leaves out). No
CI job yet: `.github/workflows/` changes need the owner.

## Building

```fish
cd apps/relay-code/native
cargo test                                         # the link (with a stand-in door) and the app's rules and flows
cargo clippy --all-targets -- -D warnings
cargo run                                          # a phone-shaped window that pairs and connects like the phone
cargo run -- --shot out.png --screen chat          # first-run, home, drawer, repos, chat, models, commands, asks, new-agent, settings, changes, board, task, inbox, update (sample data)
tools/build-lib.sh                                 # librelaycode.so for arm64 (add x86_64 for the emulator)
cd .. && ./gradlew :app:assembleDebug              # native/android/build/outputs/apk/debug/app-debug.apk
```

Shots run capped, as Leaf's: `systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 target/debug/relay-code --shot …`.

The personal build fills in the owner's server so the first screen needs one tap:
`RELAY_CODE_SERVER=<server> tools/build-lib.sh` (read at build time; blank in every other build).

### First install on your phone

Once, by cable; every build after that comes from the PC (below).

1. **The personal library**, with your server so the first screen needs one tap, from `native/`:
   `RELAY_CODE_SERVER=<your server> tools/build-lib.sh` (arm64; it also sets the build number,
   `RELAY_CODE_BUILD`, the commit count).
2. **The APK**: `cd .. && ./gradlew :app:assembleDebug`, which writes
   `native/android/build/outputs/apk/debug/app-debug.apk` (about 20 MB; 19.7 MB at build 2826, versionCode
   and versionName `0.2826` from the commit count), signed with this PC's debug key.
3. **Install** with the phone on USB debugging: `adb install -r native/android/build/outputs/apk/debug/app-debug.apk`.
4. **Pair**: open Relay Code, Pair through my server, then Allow on Discord. It connects by itself
   from then on.
5. **Notifications**: allow them when Android asks, so an agent that asks you, finishes or has a
   command held reaches you (Settings has a switch for each).
6. **Installs for updates**: the first update opens Android's "Install unknown apps" page for
   Relay Code. Allow it, come back, and tap Install.
7. **Every later build**: `tools/publish-apk.sh <apk> <instance>`. The phone offers it within 15
   minutes, or at once from Settings › Check for updates now. Updates install only over a build
   signed with the same key, so publish debug builds from this PC.

### Updates from the PC

The phone updates from the PC it is paired with, as Leaf does from its server (`update.rs`). It
knows its build (`RELAY_CODE_BUILD`, the commit count `tools/build-lib.sh` builds at; Gradle's
`versionCode` takes the same number), asks the PC's `phone.app.latest` when it opens, when it comes
back to the front and every 15 minutes while in front, and offers a newer build in a sheet: Update
downloads `phone.app.apk` in 3 MiB pieces, checks its SHA-256 and hands it to Android's installer
(Later hides that build for a day). To publish a build to the phone:

```fish
tools/build-lib.sh && cd .. && ./gradlew :app:assembleDebug
tools/publish-apk.sh native/android/build/outputs/apk/debug/app-debug.apk dev   # or stable
```

`publish-apk.sh` copies the APK and its `latest.json` into the instance's data folder
(`~/.local/share/relay-v4/<instance>/phone/`), where the engine reads them. Nothing to deploy or
restart. Android installs an update only over a build signed with the same key, so publish the
kind of build the phone has.

### A smoke run on an emulator

```fish
tools/smoke.sh            # builds for x86_64, installs, screenshots Home, a chat, Changes and Settings
```

It runs on an emulator only (one already running, else it boots `$AVD` headless and closes it
after), under a Relay device lease, on sample data: the debug build's `screen` extra
(`smoke.rs`), so it needs no PC and no pairing. It writes the shots and `startup.tsv` to
`smoke-out/`: `am start -W`'s TotalTime and the app's own "first frame N ms after start", each
from a cold start. First run, 2026-11-27, Tally_API36 (x86_64, Android 16) on the desktop:

| Screen | TotalTime | First frame |
|---|---|---|
| Home | 661 ms | 463 ms |
| Chat | 812 ms | 424 ms |
| Changes | 766 ms | 502 ms |
| Settings | 936 ms | 444 ms |

### Testing against a real door

`tests/live_door.rs` pairs, connects, and tries what the app is new to (`chat.send` once under one
id, `session.transcript` with `before`, `phone.app.latest` and `.apk`) against a throwaway `test`
engine on this PC. Pairing is the owner's to do, so the owner runs it, in a terminal of their own:

1. `apps/relay-code/tools/live-check.sh` starts the test engine, its door on loopback, a scratch project and a fake APK.
2. It prints one pairing command: run it, as it is, in a second terminal.
3. It runs the live test, prints `Live check passed.`, and stops what it started.
