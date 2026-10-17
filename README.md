# Relay native

Fresh Rust desktop client using **GTK4, VTE and GtkSourceView**, with Relay-2's
compact console and agent wall. No Tauri or webview.

The Rust engine, bus, CLI and their tests come from Relay-2. The native client
is new. [Source revisions](docs/SOURCES.md) identify both references. The full
[V3 roadmap](docs/upstream-v3/ROADMAP.md) and its supporting documents are
preserved for subsequent work; [the current scope](docs/ROADMAP.md) separates
this rebuild from that backlog.

## Run

Requires GTK 4.22+, VTE 0.84+ and GtkSourceView 5.18+, and to build, FFmpeg's development headers
(libavcodec, for the device mirror's decoder) and libclang (bindgen reads those headers). From this
checkout:

```fish
./run.sh
```

The launcher builds incrementally, starts the `dev` engine if needed, waits for
it to respond, and opens the window. It also works by absolute path from any
directory. Use `./run.sh stable` or `./run.sh test` to select another instance.
Engine startup logs go to `target/engine-<instance>.log`.
Compiler output is saved to `target/launcher-build.log` and shown on build failure.

V4 uses `$XDG_RUNTIME_DIR/relay-v4/` for engine sockets and
`~/.local/share/relay-v4/<instance>/` for data (or `$XDG_DATA_HOME/relay-v4/`).
This keeps it independent of a running Relay-2/V3 engine. Existing Relay-2/V3
projects and sessions stay in their original store; V4 starts with its own registry.

A running engine is reused when it is the build `run.sh` just made. One that
predates the build (or comes from another checkout) is restarted if it holds no
live sessions; otherwise `run.sh` warns and leaves it running its old code until
you restart it deliberately. `app.version` reports the running image's
`built_at`. Closing the window keeps the engine, sessions and worktrees alive.

`run.sh` keeps the desktop launcher pointing at the primary checkout's `dev`
instance; runs of another instance or from a linked worktree leave it alone.
`RELAY_INSTALL_DESKTOP=1 ./run.sh <instance>` repoints it deliberately.

The engine `run.sh` starts also carries the phone door, so a phone can reach it
on the LAN directly (`relay remote pair` prints the QR code), or from anywhere
through a rendezvous server you host ([docs/MOBILE.md](docs/MOBILE.md)). The phone app is
`apps/relay-android` (Kotlin, Compose, Room; the **Relay Android** workflow builds it): it keeps
a copy of the PC's data so it works with the PC away, and can start the engine through
`relay remote serve --start-engine` when the desktop app is closed ([docs/ANDROID.md](docs/ANDROID.md)).

`apps/tally` is Tally, the budget and spending tracker for Android (Kotlin, Compose, Room),
brought in from its own repository and built by the `tally-*` workflows. Its money rules are
ported to `crates/relay-money` for the engine, held to the same tests, so the phone and the PC can
each keep the whole ledger. The plan is [docs/MONEY.md](docs/MONEY.md).

Arbiter sits beside Tally in the Threads space: rule-based crypto trading and AI trading research
on your own Coinbase account, on paper or live, every order through one risk gate. Its rules,
backtester and Coinbase client are `crates/relay-arbiter`; the design is
[docs/ARBITER.md](docs/ARBITER.md).

Avex, the gym app, is the third source there. It has no internet, so Relay reads the training
history it exports (Settings, Export, Training history), read only; `crates/relay-gym` reads the file
and answers the `gym.*` ops. See [docs/GYM.md](docs/GYM.md).

Open a repository from the sidebar, then add solo agents or a group of
one or two builders sharing a reviewer. Code can edit the primary checkout or
an agent worktree. Board, Notes, Skills, Settings, usage and Android device tools
use the same bus as the CLI. **Design**, beside Agents, is each project's design canvases: an
infinite canvas of frames, text, shapes, stickies, pen strokes and pictures where everything can be
clicked, edited and moved, which the project's agents draw on too through the `design.*` ops; a
design belongs to a terminal, and Ask hands a selection to its agent
([docs/DESIGN-CANVAS.md](docs/DESIGN-CANVAS.md)). [Parity coverage](docs/PARITY.md) records the current
implementation and validation boundaries. The device mirror decodes in process with
FFmpeg's libavcodec, opened when a mirror starts (`libavcodec.so` of the major version Relay was
built against; without it everything else still runs and the mirror says what is missing), and
notification audio plays through `ffplay`. `scripts/mirror-latency/` measures the mirror end to
end against your own phone.

**Plugins** bundle skills, standing agent rules, documentation and MCP servers, and are switched
on per project from the Plugins page or the plugin key on a project's sidebar row. The first is
[Unreal Engine](plugins/unreal-engine/README.md): 20 game-development skills (design, levels,
narrative, UI, animation, AI, C++, Blueprints, GAS, networking and more) and `relay unreal-mcp`,
which builds the project, reads its log and drives a running editor through Remote Control and
Python: play sessions with screenshots, tests, profiling, Blueprint reading, asset audits, and
animation checks. The second is [Blender](plugins/blender/README.md): 6 skills and
`relay blender-mcp`, which runs Blender in background mode to inspect, render, rig-check,
measure, export and hand art to a running Unreal editor. New plugins are folders under
`plugins/`, compiled into the engine.

## Verify

```fish
cargo test --workspace
cargo clippy -p relay-native --all-targets -- -D warnings -A deprecated
python3 scripts/native-smoke.py
```

`native-smoke.py` captures every page, then runs the in-app roadmap regressions
(`notes`, `files`, `lifecycle`, `tools`, `registry`) against the same fixture
engine; `RELAY_SMOKE_ROADMAP_ONLY=notes,files` runs only the named parts.

There is no `cargo fmt --check` step: the workspace, `relay-native` included, is
deliberately not rustfmt-clean. Do not run `cargo fmt` across a crate; match the
formatting of the code around a change.

CI runs the engine, bus and CLI half of that list — `cargo test --locked` and
`cargo clippy --locked --all-targets -- -D warnings` — on every push and pull
request, along with the client logic kept out of GTK (`relay-board`,
`relay-client`). The native client itself is not covered: it pins GTK 4.22, VTE 0.84,
GtkSourceView 5.18 and pango 1.56, which the hosted runners do not carry, so its
checks and the display smoke test still have to be run on a machine that has
them. `.github/workflows/ci.yml` records what a runner would need to close that
gap.

The display smoke test creates its own temporary store, repository and fake
provider executables. It opens the GTK application at two desktop sizes and
checks saves, a review-group launch and eleven-session output delivery. It never launches a paid
model or touches the user's project store. Screenshots go under
`.impeccable/review/`.

See [verification and limits](docs/VERIFICATION.md). The application has no
engine refresh timers by default, and one housekeeping sweep: every 20 minutes
the engine deletes closed sessions' `relay/*` branches whose work is merged, and
the GitHub branch too when a merged PR proves it (`git.branch.cleanup`, BUS.md
§10.12; each deletion is audited). The one opt-in exception is the usage-limit
interval (`usage.refresh_minutes`, set from the status bar's limits popup, off by
default), which re-reads what Claude Code and Codex already saved locally and
never contacts a provider. A 30-second clock only rewrites the "updated … ago"
labels. Each terminal has a bounded stream connection separate from
control requests; project/page changes detach unneeded streams. Engine state
changes refresh the visible surface, and VTE owns terminal scrolling.
