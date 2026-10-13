# RELAY v4 — FULL SPECIFICATION
One binary, agent-native, Linux-first. Clean-slate rewrite; v3 data imported.

═══════════════════════════════════════════════════════════════════
1. STACK
═══════════════════════════════════════════════════════════════════

## Shell & UI
- **Tauri 2** — app shell, windowing, system webview (WebKitGTK). Dev and
  stable builds get DIFFERENT bundle identifiers (com.quietsoftware.relay /
  .relay.dev) from day one so state never cross-corrupts.
- **Svelte 5 + TypeScript + Vite** — UI layer. Runes for state. Strict TS.
  Browser fixture mode preserved as a first-class loop (see §13).
- **CodeMirror 6** — code editing and diffs. Packages: @codemirror/state,
  @codemirror/view, @codemirror/merge, language packs (rust, ts, kotlin,
  svelte via lezer), one theme derived from the app token source.
- **xterm.js + @xterm/addon-webgl + @xterm/addon-fit** — terminals.
  Writes coalesced to frame boundaries (v3 fix carried forward).
  allowTransparency decided ONCE against the WebGL renderer at spike time;
  whichever wins is recorded in the spec, not rediscovered.

## Core (Rust workspace: relay-core, relay-bus, relay-app)
- **tokio** — async runtime; owns every child process and PTY read loop.
- **portable-pty** — PTY allocation, resize, kill. Reader tasks detached,
  teardown kills child → drops master → aborts reader, in that order.
- **gix** — git reads in-process: status, diff, log, branch, worktree list.
  Shell out to system git ONLY for merge, rebase, push (battle-tested paths).
- **tree-sitter** — symbol extraction for overlap detection; linear scans
  only (symbols_in stays O(n)); grammars: rust, ts, svelte, kotlin.
- **rusqlite (bundled)** — single store.db. Schema versioned (user_version),
  migrations tested against every prior version. Indexes on every FK and
  every column that appears in a WHERE. WAL mode.
- **notify** — inotify watchers per worktree; git panel and file tree refresh
  on events. NO polling timers anywhere in v4. A 60s "trust but verify"
  reconcile pass and the 20-minute merged-branch sweep (`git.branch.cleanup`,
  BUS.md §10.12) are the only clock-driven work in the app.
- **serde + schemars** — bus message types with generated JSON schema.

## Build & Release
- cargo + mold linker + sccache. Single AppImage output.
- Desktop entry written by the app on first run (correct env, correct icon).
- Version bump = tauri.conf.json version; stable promoted from dev by
  explicit build, never by accident.

═══════════════════════════════════════════════════════════════════
2. THE COMMAND BUS (the app's entire API — design before code)
═══════════════════════════════════════════════════════════════════

- ONE Tauri command (`bus`) carrying typed envelopes:
  { v: 1, id: uuid, actor: "user" | "agent:<session>" | "test",
    op: "task.create", payload: {...} }
- Every mutation in the app is a bus op. The UI is client #1. No side doors:
  if a button does something the bus can't, the button is wrong.
- **Schema versioned from op one.** v field in the envelope; breaking changes
  bump it; old ops keep working one major back.
- **Three doors:**
  1. UI (Tauri invoke)
  2. `relay cmd '<json>'` CLI — any agent, any script
  3. MCP server exposing ops as tools — Claude Code drives Relay natively
- **Reads too:** query ops (task.list, module.stats, session.peers,
  usage.get) so agents ask the app instead of scraping.
- **Audit log:** every accepted mutation appended to `audit` table:
  ts, actor, op, payload hash, result. Powers undo, blame, and debugging.
- **Guardrails execute ON the bus** (see §5). A rejected command returns a
  typed refusal the agent can read and react to.
- **Keyboard shortcuts dispatch bus ops** — remappable for free, discoverable
  by agents via schema.

═══════════════════════════════════════════════════════════════════
3. AGENT LAYER (why v4 exists)
═══════════════════════════════════════════════════════════════════

## Knowledge injection (at every dispatch)
Relay launches providers with no positional prompt. A short provider-native role instruction tells
the agent to call the actor-bound `session.bootstrap` operation on its first real turn, names the
Relay mailbox as the channel that reaches every provider, and points at `$RELAY_BRIEF`. Bootstrap
returns the current task, branch, module, pair and optional launch assignment, plus the live peer
table, every operation the session may actually call, and its guardrails. The compact brief below
travels with that role instruction on every spawn — knowledge injection that is written and never
handed over is not injection — while skill bodies go to `.relay/session-skills.md`. The complete
provider-neutral snapshot is written to `.relay/sessions/<session>/session-brief.md` for explicit inspection:
- **State brief** — worktree path, branch, project, module, the task text
  including its changelog field
- **Peer table** — live sessions: name, branch, claimed files, current task;
  refreshed mid-run on change via the mailbox
- **Adjacent tasks** — open tasks in the same module (titles + states)

## Agent actions (via bus)
- task.move (own task → in review / done), task.link_commit,
  task.changelog.write
- mailbox.send (agent↔agent messages: "don't touch store.ts, need X from you"),
  mailbox.outbox to see what it sent
- session.claim / session.release (the files I am taking on; collisions come back named)
- session.intent (one line of what I am doing, for the peer table)
- session.done {status, blockers} (finished, blocked, or partway, and why)
- bus.wait (be woken when something happens, instead of polling)
- guardrail.explain (would this plan be allowed, before starting it)
- notes.append
- merge.integration.request (see §7)
- overlap.flag / overlap.ack
- session.done (self-report completion → triggers notification + review state)

## Mailbox
- Per-repo message table; injected deltas mid-run; agents address each other
  by session name. The v3 awareness feature ported intact, now bidirectional
  and persistent.

═══════════════════════════════════════════════════════════════════
4. PROVIDERS — Claude Code + Codex only
═══════════════════════════════════════════════════════════════════

- **Provider trait:** spawn(cmd, args, env, cwd), resume(session_ref),
  auth_status(), usage(). Two implementations. A third later is two small
  files, not a refactor.
- **Claude Code:** `claude` binary, Max auth via its own login, resume flags
  per its CLI, MCP-native (gets the bus as tools).
- **Codex:** `codex` binary, its auth, its resume model; bus via CLI door
  until its MCP support is worth wiring.
- **Binding:** a session is created WITH a provider and keeps it. No
  mid-session switching ever (thinking-history hazard). Pair preset allows
  different providers per role instead: Claude builds / Codex reviews or
  inverted — cross-vendor review is the point.
- **Auth surfaced read-only:** settings shows "signed in as …" per provider,
  detected from each CLI. Relay never stores credentials.
- **Version watch:** each CLI's version shown in settings; toast on change
  ("claude 2.1.226 → 2.1.230 — spawn profile untested") because CLI updates
  changing flags/output has bitten before.
- **Usage per provider, per its own metering:** Claude Max 5h window + weekly;
  Codex in its own units. Bottom-right selector. No flattening into one fake
  number. `usage.get` performs bounded, read-only inspection of rate-limit state
  that the CLIs already received during normal work; it never calls a provider
  endpoint or spends tokens to fill the status bar.

═══════════════════════════════════════════════════════════════════
5. GUARDRAILS (enforced in core, never prose)
═══════════════════════════════════════════════════════════════════

- File-count and changed-line caps per task (existing v3 semantics).
- **Destructive-write hold:** any file write removing > N lines or > P% of a
  file is HELD; the agent gets a typed refusal with a confirm op; you get a
  notification. The data.ts truncation, made impossible to do silently.
- **Data-shape gates:** registered validators run before accepting writes to
  declared critical files (e.g. non-empty arrays, parseable, schema-valid).
- **Per-agent command allowlist:** Pair's reviewer can read everything and
  write nothing; a docs agent can't touch git ops.
- Protected paths (configurable) refuse writes outright.
- Every hold/refusal lands in the audit log and the notification center.

═══════════════════════════════════════════════════════════════════
6. BOARD — ISSUES
═══════════════════════════════════════════════════════════════════

- **Five hardcoded columns:** backlog · in review · ready · active · done.
  No custom columns, no agent column.
- **Task create (dedicated page, not a modal):** title, body, target column,
  state, priority, module (optional), size (S/M/L), image attachments via
  drag-drop, changelog field ("the sentence that ships in patch notes").
  Priority and size use compact icon choices, with a live task identity strip.
  Create → returns to a FRESH create page (rapid entry), not edit mode.
- **Task card:** priority chip (no overlap bug this time — chips truncate,
  never collide), size, module icon, copy button (copies title+body+id).
- **Dropdown preview** on every task: priority + body cut at ~100 words.
- **Dispatch flow:** task → "send to agent" → choose existing session or the
  create sheet pre-filled → knowledge injection includes the task → task
  auto-moves to active → on session.done moves to in review → your approval
  moves it to done and links the commit hash.
- **Done = hash-linked:** done tasks display their commit(s); clicking opens
  the diff in the editor pane.
- **Soft delete** with undo window (§14).

═══════════════════════════════════════════════════════════════════
7. MODULES — OWN PAGE (release tracking; what the changelog is about)
═══════════════════════════════════════════════════════════════════

- **Navigation:** Board ⇄ Modules segmented switcher at the top of the same
  tab — sibling pages, equal weight (BridgeMind Agent/Code/Chat pattern).
- **Modules index page:**
  - Header stats: module count · in flight · issues · completed ·
    completion RING (% of all tasks done across all modules)
  - Module list: icon, name, priority, per-module task counts, per-module
    progress bar
  - Create: composed identity form with name, an 18-icon pack, and priority
- **Module detail page:**
  - Its organized task list grouped by state; dropdown previews; click →
    full task detail; done section at the bottom
  - Tasks created here inherit the module and land in board backlog
  - A task belongs to ≤1 module; the module REFERENCES board tasks (single
    source of truth: the board)
- **Changelog generation:** module → "draft patch notes" renders completed
  tasks' changelog fields grouped by priority, copyable markdown. The
  release notes write themselves; you edit, not compose.
- **Module completion** archives it (undoable) and stamps the date.

═══════════════════════════════════════════════════════════════════
8. CODE
═══════════════════════════════════════════════════════════════════

- **File tree:** per-worktree; a combined worktree/branch selector; file-type
  glyphs; create, rename, delete (soft), drag-drop move,
  drag files IN from the OS; inline edit opens the editor pane; notify-driven
  refresh; git status badges (M/A/D) on rows.
- **Editor:** CodeMirror; edit + view; per-language highlighting; diff view
  via @codemirror/merge; opens from file tree, git panel, and task hashes.
  Save goes through the bus (guardrails apply to YOUR edits too — with a
  user-actor bypass confirm, not silence).
- **Git panel:** commit all (message box, Ctrl+Enter), open PR, changed
  files with counts, history graph, branch chip. gix reads + notify events;
  zero timers. Per-session or all-branches toggle.
- **Branch creation:** `git.branch.create` validates the name and optional start
  point, creates through system Git in the selected worktree, and switches by
  default. Existing names are typed conflicts rather than UI-side guesses.
- **Integration merge (first-class op):** select N session branches →
  octopus-merge into a throwaway worktree → run build command → optional
  deploy to device → report pass/fail per branch combination → discard.
  Conflicts abort with the pair named. This is the "test everything
  together" answer.
- **Worktree lifecycle:** close session → confirm → `gradlew clean`/target
  purge → worktree remove → branch kept until merged or explicitly deleted.
  A "clean merged branches" op sweeps stale ones. The 73GB problem, closed.

═══════════════════════════════════════════════════════════════════
9. DEVICE
═══════════════════════════════════════════════════════════════════

- **USB mirror:** scrcpy-class pipeline (H.264 over USB, decode in-app),
  target <50ms glass-to-glass; mirror pane dockable and poppable to its own
  window; correct scaling at any resolution (the >1080p bug stays dead).
- **Run on device:** pick worktree (or integration worktree) → gradle
  installDebug → logcat streamed into a log pane; crashes surface the stack
  trace inline.
- **Release build:** the same worktree picker with nothing plugged in → gradle
  `assembleRelease` / `bundleRelease` → the APK or AAB path lands in run
  history beside the runs, with its signature verified after the build;
  Device Control can create one project-scoped PKCS12 upload key outside the
  repository, with its password in Linux Secret Service, and inject it only
  into Relay-triggered release builds. The user can explicitly disable that
  override to use an existing project-owned upload key without deleting the
  Relay key. Without an enabled profile, Gradle's project signing remains the
  fallback. Publishing still hands the artifact to
  Gradle Play Publisher, so the Play account stays in the target project and
  Relay never holds a Play secret. Relay itself still ships as one
  AppImage: what goes to Google Play is the Android project Relay builds.
- **AVD management (final milestone):** list/create/boot emulators, deploy to
  them phoneless; same run pipeline, different target.

═══════════════════════════════════════════════════════════════════
10. SESSIONS
═══════════════════════════════════════════════════════════════════

- **Presets:** SOLO (one agent, one terminal) · PAIR (builder + reviewer on
  the SAME worktree; reviewer is read-only via allowlist; may be
  cross-provider). No swarm, no workbench, no loops.
- **Create sheet (modeled on the reference):** preset row → agent grid
  (Claude/Codex, installed + signed-in states) → effort → count selector (1-6) →
  "WILL LAUNCH" preview row → footer summary
  ("Solo · 1 session in avex").
- **Naming:** adjective-animal auto-names, stable for the session's life
  (they're your handles; never renamed by task).
- **Per-session:** provider, model, effort, branch (changeable BEFORE first
  spawn), worktree, module context.
- **Resume:** on app start, each restorable session offers resume
  INDIVIDUALLY (checkbox list, not all-or-nothing). Scrollback restored
  from disk; provider resume invoked; declined sessions cleaned.
- **Parking:** idle > T minutes → offer to kill the CLI process, keep pane +
  scrollback + worktree; touch respawns with provider resume. RAM back.

═══════════════════════════════════════════════════════════════════
11. CHROME & LAYOUT
═══════════════════════════════════════════════════════════════════

- **Top bar (custom, decorations off):** sidebar toggle + app name left · center
  Code shortcut for the active project · right: command search, window presets,
  notifications, and New session. Selecting a project opens its Agents wall, so
  a second Agents shortcut is redundant. Settings takes over the app shell with a
  back arrow and no project sidebar. There are no desktop-style File/Edit/View
  labels whose only action is navigation.
- **Sidebar:** fixed nav (Dashboard · Skills · Plugins · Board · Notes) → WORKSPACES section
  with +: workspace = a directory; PROJECTS nested under it (a project = a
  repo); active project highlighted with left marker; per-project live/idle
  pill and board count. Footer block: usage summary, account, theme toggle,
  settings gear.
- **Dashboard:** cross-project control room — a decision queue for blocked agents,
  guardrail holds, and reviews; direct destination actions; per-project workload
  pulse; active agents; recent activity; and current resource totals.
- **Pane system:** Code side panes resize with direct split handles and can be
  hidden independently; their sizes, visibility, Git graph toggle, and the vertical
  Git history split persist per project and participate in named window presets.
  The Agents wall uses two explicit, non-overlapping pane columns. One occupied
  column fills the wall; two share a draggable 20–80% split; terminals divide their
  column vertically. Header dragging moves a pane left or right and inserts it above
  or below another pane. New sessions enter the lighter column, and the columns stack
  when the wall is too narrow. Placement and split survive project navigation. Device
  sources remain in a bounded strip below the terminal wall.
- **Status bar (bottom):** left: project · branch · machine. Right: usage
  chip (per-provider selector) · resource chip → panel (per-pane RAM/CPU,
  per-worktree disk — the panel that found the 2GB and the 73GB) ·
  session count.
- **Notification center:** bell in top bar; events: agent finished, agent
  blocked/asking, guardrail hold, integration result, provider version
  change, disk threshold. Each notification deep-links. Sound per-category
  togglable, with a small local tone pack and Off.

═══════════════════════════════════════════════════════════════════
12. PANES
═══════════════════════════════════════════════════════════════════

- **Terminal** — the agent surface; branch chip + overlap chip in the strip.
- **Notes** — KWrite-inspired plain-text document workspace, per-repo, with an
  indexed document library, open-document tabs, independent unsaved buffers,
  find/replace, Markdown text helpers, wrap and font controls, document stats,
  pinned entries, created/updated timestamps, and Ctrl N/S/F. Notes remain
  agent-readable at dispatch and agent-appendable through the bus.
- **Log terminal** — Relay's own structured logs, filterable.
- **Skills tab** — install `SKILL.md` instruction files from GitHub sources,
  refresh/remove them, inject on dispatch, and materialize them as provider skill folders in
  every project root, every session worktree, and each provider's own home (Codex reads skills
  only there); enabled everywhere by default, per-project off switch. Reinstalling a
  previously removed name restores its hidden row; a genuine visible name collision identifies
  both sources and offers review, refresh, or an explicit atomic replacement.
  Relay does not present a local skill-authoring surface.
- **Plugins tab** — bundled plugins (skills + agent rules + docs + MCP servers, D159), each with a switch per project; every project row in the sidebar also opens its own plugin switches.
- **Editor / Diff / Files / Git / Mirror / Run** — per §8-9.

═══════════════════════════════════════════════════════════════════
13. SETTINGS & THEMING
═══════════════════════════════════════════════════════════════════

- **One theme source:** a single token module (colors, alphas, radii, blur)
  feeding Svelte CSS vars, xterm theme objects, AND CodeMirror theme.
  Changing one value changes every surface — verified by a startup
  assertion in dev builds.
- **Appearance:** wallpaper library with a selected custom image (downscale-on-load; previews are a
  separate render path; no 8K decode in the compositor), transparency
  slider (one --panel alpha), wallpaper dim and content-protection controls,
  modes: matte · dark · OLED (true black).
- **Notifications:** per-category toggles + local sound choice. Tones are generated
  in the shell after an event and never require a media service or background player.
- **Subscription tab:** provider auth status, usage windows, plan labels.
- **Providers:** CLI paths, detected versions, spawn profile overrides.
- **Guardrails:** caps, protected paths, destructive-write thresholds.
- **Fixture mode:** VITE_FIXTURES=1 official; realistic seeded states
  (overlaps, failed, behind-main, guardrail-held, empty everything) so UI
  work never needs the Rust side or a live agent.

═══════════════════════════════════════════════════════════════════
14. RESILIENCE
═══════════════════════════════════════════════════════════════════

- **Persistence model, spec'd per case:**
  - Webview reload: Rust alive → PTYs survive → reattach listeners,
    scrollback intact, nothing lost.
  - App restart / crash / hard-lock: PTYs dead → resume flow (§10);
    scrollback restored from the last park or graceful-shutdown snapshot; a
    crash loses output since then (nothing flushes on an interval).
- **Crash recovery on launch:** reap orphaned claude/codex processes,
  fsck sessions table against live PIDs, flag worktrees with uncommitted
  changes, offer reset for tasks stuck in active, log what it did.
- **Undo:** soft-delete + grace window (tasks, modules, notes); audit log
  is the source; Ctrl+Z on board ops.
- **Backup:** store.db copied on every version upgrade + manual "back up
  now"; keep last 5.
- **v3 import:** one-time migration of boards, tasks, modules, notes,
  session names from v3's SQLite. Run once, verified, then v3 archived.
- **Relay log file:** rotating structured log (tracing crate) in
  ~/.local/share/relay/logs; errors diagnosable after the fact without
  having launched from a terminal.
- **First-run:** infer the launch/current directory when workspace is blank,
  discover local Git repositories for a selector, or connect GitHub through
  its CLI-backed browser flow and select any accessible repository to clone,
  then optionally import v3 data.

═══════════════════════════════════════════════════════════════════
15. PERFORMANCE BUDGETS (enforced, not aspirational)
═══════════════════════════════════════════════════════════════════

- Idle, app open, unfocused: 0 subprocess spawns/min, <1% CPU.
- Focused, git panel open: event-driven; reconcile pass ≤1/min.
- Startup to interactive with 6 restored panes: <2s, no publish storm
  (coalesced single pass — the 126→13 fix is an invariant, with a test).
- Streaming 11 panes: 60fps, compositing ~0 (WebGL + frame batching).
- Overlap scan: linear per file, debounced, never in a per-pane loop.
- RAM: Relay itself <1GB with 6 panes (agents' own RAM excluded — not ours).
- Budgets asserted in CI where measurable (spawn counter harness from the
  perf passes becomes a test).

═══════════════════════════════════════════════════════════════════
16. TESTING
═══════════════════════════════════════════════════════════════════

- **Bus-driven integration tests:** every op exercised headless (the bus is
  the test API — same door as agents).
- **Guardrail adversarial suite:** the destructive-write, shape-gate, and
  allowlist paths each have a test that tries to get past them.
- **Store migration tests:** open DBs at every historical user_version.
- **PTY lifecycle test:** spawn/kill N sessions, assert zero orphans.
- **Fixture snapshots:** key UI states rendered in browser mode and
  screenshot-compared (the Roborazzi lesson, applied to Relay).

═══════════════════════════════════════════════════════════════════
17. BUILD ORDER
═══════════════════════════════════════════════════════════════════

1. Bus schema (written spec, reviewed) → bus + audit + CLI
2. Store + migrations + v3 importer
3. Worktrees + PTY + teardown tests
4. Guardrails on the bus
5. Awareness + mailbox + knowledge injection
6. Terminal panes + session create/resume/park
7. Board → dispatch flow → Modules + changelog
8. Git panel + file tree + editor + integration merge
9. Chrome: sidebar, top bar, layouts, notifications, status bar
10. Device mirror + run-on-device
11. Settings/theming polish, MCP server, first-run
12. AVD milestone + GitHub skill installation and repository onboarding

Spine before surface. The bus schema is the one thing designed on paper
before any code, because every agent skill written against it makes it
harder to change.
