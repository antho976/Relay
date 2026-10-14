# Working on Relay's performance

The Relay-specific half of performance work. The method — measure first, deterministic proxies,
ratchets, before-and-after from one machine — is the bundled **Performance** plugin's
(`plugins/performance/`, switch it on for the project on the Plugins page); this page is what that
method needs to know about Relay: which journeys matter, which invariants are cliffs, how to measure
each half, and where the next audit should look first.

## 1. What exists

| Thing | Where | Notes |
|---|---|---|
| Engine harness | `crates/relay-core/examples/perf.rs` | one disposable fixture (1 500-file repo, store on disk, fake providers, two live PTYs, a populated board); every bus op plus the non-op paths |
| Runner | `scripts/perf/baseline.py` (`--quick` ≈ 5 min; full ≈ 55 min with callgrind) | native wall/CPU/allocs, callgrind attribution, strace counts, ×10 rows, soak, the real `relay serve` process |
| Compare | `scripts/perf/compare.py <old run> <new run>` | trusts instruction counts and breakdowns over absolute times |
| Native client | `scripts/perf/native-baseline.py` | every page against a disposable engine: CPU at startup and at rest, wakeups, RSS, `perf` profile per page, `GDK_DEBUG=frames` log |
| Readings | `BASELINE-<date>.md`, `FIXES-<date>.md`, `runs/<date>/` | the format every new reading follows (see the plugin's `perf-audit` templates) |
| In-process lock budget | `engine.rs`: a handler holding the store lock ≥ 16 ms logs "store lock over budget" | read the log after any scenario run |

**Tools the full run wants** on the measuring machine: `valgrind` (callgrind pass; the 2026-10-07
run had none and skipped it), `perf` (native profiles), `strace`, and for deeper work `samply`,
`heaptrack`, `hyperfine` and `sysprof`. On Arch/CachyOS:
`sudo pacman -S valgrind perf heaptrack hyperfine sysprof` and `cargo install --locked samply`;
profiling needs `sysctl kernel.perf_event_paranoid=1`.

**Profiles.** The harness measures the **dev** profile (`opt-level = 1` for the workspace, `3` for
dependencies), which is what `run.sh` launches — say so in every number. `[profile.release]` strips
symbols (`strip = "symbols"`), so profile a release-like build through a profile that keeps them:

```toml
[profile.profiling]
inherits = "release"
strip = false
debug = "line-tables-only"
```

## 2. Journeys and budgets

| Journey | Where it starts → ends | Budget | Last reading |
|---|---|---|---|
| Keystroke in a terminal | key press → glyph on screen (`session.input` → PTY → frame → VTE) | engine side ≪ 1 ms; whole path inside one frame (8–16 ms) with p99.9 near p50 | 1.1 µs in process, 7.1 µs through the socket (2026-10-07) |
| Terminal output flood | child writes → rendered; Ctrl-C stays responsive | 1 MiB end to end ≪ 100 ms; input never waits behind output | 37.6 ms / MiB |
| Any page open (Board, Code, Notes, Money, Threads, Devices) | click → content painted | ≤ 100 ms | not measured end to end yet |
| Session list refresh after `session.changed` | event → sidebar repainted | ≤ 1 frame | `session.list` 13.7 µs; the GTK rebuild is unmeasured |
| Board load | open board → cards painted | ≤ 100 ms at 400 tasks | `task.list` 333 µs at 40 tasks |
| New session | create → first output on screen | ≤ 1 s (the worktree checkout is the work) | create 26 ms, first output 8 ms |
| Native cold start | launch → first frame → connected and interactive | first frame ≤ 500 ms, interactive ≤ 1 s | per-page CPU in `runs/*/native/` |
| Engine start | `relay serve` → socket answering | ≤ 20 ms on a populated store | 4.6 ms |
| At rest, N quiet sessions | 30 s idle | **0 % CPU, 0 wakeups** in engine *and* client | engine 0/0 (2026-09-19); client not yet zero (§5) |
| Phone cold start | tap → start screen with cached data | TTID ≤ 1 s mid-range | not measured (no Macrobenchmark yet) |
| Phone reconnect | link up → replica current | no visible stall; UI never blocked | not measured |

## 3. Invariants — breaking these is a cliff, not a slope

- **One store connection behind one mutex; plain `Engine::register` handlers run inside a
  transaction holding it** (`BUS.md` §5.1). Anything slow there stalls every other request —
  including the session list and scrollback a person is watching. Slow reads go in
  `register_unlocked`; slow mutations in `register_staged` (`prepare` unlocked, `finish` locked).
  The 2026-09-19 baseline showed what a violation costs: `session.list` 35 ms during an overlap
  scan, 140 µs after the fix.
- **After-commit closures run after the lock is dropped but before the reply is sent.** A slow
  after-commit (e.g. `project.relink` runs `git worktree repair` there) does not block others but
  does block the caller.
- **One client connection serializes**: on a socket, up to 8 `register_unlocked` queries run
  concurrently, but every other op waits for in-flight queries to drain and then runs inline
  (`socket.rs`). Everything the GUI sends on its shared control connection that is not unlocked is
  serialized — a slow plain op delays the next page's read.
- **`session.input` never touches SQLite** (D148). Keystrokes answer from memory.
- **Every subprocess from a handler goes through `proc::output_with_timeout`.**
- **Prefer `prepare_cached`**; the store's statement cache holds 128 (`store.rs`). Plain
  `query_row`/`execute` with SQL text recompile every call.
- **Adding a column** = a migration appended to `store::MIGRATIONS` + `SCHEMA_VERSION` bump; time a
  table-rebuild migration on a large store before shipping.
- **The GTK main thread does no I/O**: parse and decode on the tokio reader task (the client
  already does), disk reads via `gio::spawn_blocking` or the runtime's blocking pool.

## 4. Measuring each half

**Engine** — `python3 scripts/perf/baseline.py --quick` for a check, the full run for a reading;
add a scenario to `examples/perf.rs` for any new path; add a `path.*.during_*` scenario for any fix
that takes work out from under the lock (and check the slow op is still in flight when the hot one
goes out — the harness reports `overlap`).

**Native client** — builds on a developer machine with GTK 4.22 (not on CI or the usual agent
image; say so when you could not build). **Open windows only inside a private virtual compositor,
never on the person's desktop:**

```bash
SOCK=relay-headless-$$
kwin_wayland --virtual --no-lockscreen --socket=$SOCK --width 1440 --height 900 >/dev/null 2>&1 &
KWIN=$!
for i in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && break; sleep 0.05; done
WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland python3 scripts/perf/native-baseline.py
kill $KWIN
```

Keep `--no-lockscreen` (a locked virtual session never delivers frame callbacks). Useful lenses:
`GDK_DEBUG=frames` (frame log), the GTK Inspector's frame timings and recorder, `sysprof` with GTK
marks, `perf record -g -p <pid>` at rest and during a scripted interaction, and wakeups from
`/proc/<pid>/status` over 30 s.

**Android** — no SDK in agent sessions: a change under `apps/relay-android` is written, not built,
and the summary says so. `:core` is plain JVM, so its terminal parser, replica and outbox rules can be
measured with JVM benchmarks without a device. Device-level numbers need the human's phone or an
emulator (`docs/ANDROID.md`); the app has no Macrobenchmark or Baseline Profile module yet (§5).

## 5. Leads for the next audit

From a read of the code on 2026-10-14 — **not measured**. Each is a hypothesis to measure first;
several may turn out to be cheap enough not to matter. File paths are current as of that date.

### Engine

1. **Frequent GUI reads are plain `register`** — `session.list`, `project.list`,
   `workspace.list`, `task.list`, `mailbox.list`, `module.list`, `notify.list`, `dashboard.get`,
   `settings.get`, `guardrail.holds.list` take the store lock and serialize on the control
   connection. Pure SQL, so short — but each `refresh()` and each mutation's follow-up `refresh()` +
   `refresh_page()` runs several. Measure the GUI's connect burst and one mutation's follow-up as
   scenarios; consider `register_unlocked` on a read connection for the hot reads.
2. **`usage.get` walks `~/.codex/sessions`** (up to 20 000 files) and reads the tail of up to 32 of
   them on every call (`usage.rs`, `provider.rs`) — and the status bar calls it on connect and on
   `usage.changed`. Measure on a real home directory; cache keyed on directory mtimes.
3. **Handlers that still do filesystem work under the lock**: `app.resources.get` (`/proc` per
   session), `app.backup.list` (`read_dir`), `device.build` preamble (signing profile, temp script,
   probes), `task.approve` (`gix::open` + `rev_parse`), `project.add` (canonicalize, `.git/HEAD`),
   PATH lookups via `which`. Each is probably sub-millisecond — confirm and move what is not.
4. **`git.fetch` every 300 s from the GUI** (staged, network in the unlocked phase) — confirm it
   never holds the lock while the network is slow, and skip it while the window is hidden.
5. **Background workers wake every second** (orphan reaper, purge, arbiter runner) to check a
   longer interval. Check the at-rest wakeup count with them running; sleep until the next deadline
   instead.
6. **`file.tree` badges and `git.status` still spawn `git status --porcelain`** per call (FIXES
   2026-09-20 "Not done") — the watcher's events are the invalidation signal for a memo.

### Native client

7. **The sidebar rebuilds on every `session.changed`**: `render_projects` serializes all projects
   and workspaces to JSON to hash them, then `clear()`s and rebuilds the whole box when anything
   changed (`shell/registry.rs`); status counts rebuild on every reconcile (`status.rs`). With many
   sessions emitting state changes, count widgets created per event and frame time during a burst.
8. **No `GtkListView` anywhere.** Mailbox (200–1 000 rows) and git files (up to 500 per group)
   `clear()` and rebuild on change; file-tree rows are "a dozen widgets and controllers" each, 500
   per folder; Money reads a whole period's transactions with no limit. Measure page-open time and
   memory at realistic and 10× sizes; recycle rows (ListView + factory) where it is a measurable win.
9. **Editor invalidation fans out sequentially**: after a 1 s debounce, one `file.tree` for the
   root then one per expanded folder, one after another; the git panel then runs five reads
   (`editor.rs`, `code_git.rs`). Measure with ten folders expanded; batch or parallelize (they are
   unlocked queries, up to 8 concurrent per connection).
10. **Synchronous `canonicalize` on the GTK thread** for every `file.changed` event (`editor.rs`
    `concerns()`, up to three per event) and per symlink while filling the tree. Cheap on a local
    disk, a stall on a slow or network one; canonicalize once and cache.
11. **Timers that run while the window is hidden**: drift fetch (300 s), usage labels (30 s),
    wallpaper rotation (default 15 min), the start screen's endless tick callback. Only terminal
    streams detach when minimized. Measure client CPU and wakeups at rest, minimized and visible;
    the target is zero.
12. **Startup builds every page up front** (`Ui::build`: all pages, all Money/Arbiter/Avex/Threads
    pages), installs 14 fonts and icons, then **creates and presents the Notes window** before
    connecting; on connect it issues ~20 reads (six `settings.get`, three `notify.list`…).
    Measure launch → first frame → interactive; build pages lazily on first show; batch the
    connect reads.
13. **Each lifecycle op and each `device.list` opens a new socket connection** (`client.rs`,
    `status.rs`). Connect + ping is ~24 µs on the engine side, so probably fine — confirm the
    client side.
14. **Terminal feed on the GTK thread** does base64 decode + a mode scan + `feed` per chunk with
    adaptive pacing (16 KiB–1 MiB per frame). Measure keystroke latency *during* a flood (the
    budget is p99.9, not p50) and Ctrl-C responsiveness. Related: the event loop
    (`while let Ok(notice) = notices.recv().await` in `app.rs`) and every `spawn_future_local`
    run at GLib's default priority, above redraw (120) — a burst of events or output that is
    drained without yielding can hold off painting. Check frame gaps during an event burst.
15. **CSS**: one ~2 600-line static provider; broad selectors (`*`, `window > box, stack,
    scrolledwindow`, `> *:not(button)`); transitions on every button. Measure style-recalc cost in
    sysprof during a sidebar rebuild before touching anything.

### Android

16. **No Baseline Profile or Macrobenchmark** — the first, cheapest startup win (Google cites ~30 %);
    `profileinstaller` is already a dependency. Add a `:baselineprofile` module with the startup and
    board journeys; prove it with `CompilationMode.None` vs `Partial`.
17. **One `entity` table holds every kind**, and every `observe*` Flow queries it — any write
    re-runs every observer (`RelayDb.kt`). `ShellModel` combines 11 flows. Count re-emissions per
    synced event; add `distinctUntilChanged`, narrow queries, or split hot kinds into their own
    tables.
18. **`Fetch.All` on every connect** pages every task, then per project notes, mail and labels,
    plus the 12 newest threads in full. Measure on a large project set over a slow link; fetch
    deltas since the last sequence where the door can serve them.
19. **The terminal view draws under the same lock the feed takes**, per character, on the main
    thread (`TerminalView.kt`, `TerminalFeed.kt`), and the parser advances one byte at a time.
    Measure frame time during a flood on a mid-range phone; snapshot rows under the lock and draw
    outside it; batch runs of same-style text.
20. **Lists**: no keys on file search hits and `FileText` lines; Guardrails renders with `forEach`
    inside a scroll; all skills/plugins sit in one `LazyColumn` item. Fix where a profile shows
    jank.
21. **No StrictMode in debug builds** — add it to catch main-thread disk and network.

## 6. Guardrails

What exists: the harness and its readings, the "store lock over budget" log, `path.*.during_*`
scenarios. What does not yet: a **ratchet in CI**. `ci.yml` runs no benchmark. The next step after
the audit is a small deterministic gate — instruction counts (gungraun or the harness under
callgrind) for the hot ops (`session.list`, `task.list`, `session.input`, `session.scrollback`
tail, `file.tree`, engine startup), with a checked-in baseline lowered when it drops, run on the
headless crates CI already builds.
