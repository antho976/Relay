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
cargo run -- --shot out.png --screen chat          # first-run, home, drawer, repos, chat, models, commands, asks, new-agent, settings, changes, board, task, inbox (sample data)
tools/build-lib.sh                                 # librelaycode.so for arm64 (add x86_64 for the emulator)
cd .. && ./gradlew :app:assembleDebug              # native/android/build/outputs/apk/debug/app-debug.apk
```

Shots run capped, as Leaf's: `systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 target/debug/relay-code --shot …`.

The personal build fills in the owner's server so the first screen needs one tap:
`RELAY_CODE_SERVER=<server> tools/build-lib.sh` (read at build time; blank in every other build).

### Testing against a real door

`tests/live_door.rs` pairs and connects against a throwaway engine. Pairing is the owner's to do
(agents may not run `relay remote pair`), so run it yourself:

```fish
relay --instance test remote serve --bind 127.0.0.1:7431 --start-engine &
set -x RELAY_E2E_LINK (relay --instance test remote pair --no-confirm | string match -r 'relay://pair\S+')
cd apps/relay-code/native && cargo test -p rc-link --test live_door -- --nocapture
```
