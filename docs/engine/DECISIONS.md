# Decisions

The *why* behind anything surprising. Append; never rewrite history. Reference as `D<n>`.

- **D1 (2026-08-17) Stack.** Tauri 2 + Svelte 5 + Rust workspace, per SPEC §1, chosen over a
  native-Rust (gpui) UI because agents know the web stack better and the app is built with agents.
  WebKitGTK's limits from v3 are handled by design (frame-coalesced terminal writes, WebGL decided
  at spike time), not rediscovered.
- **D2 The bus is designed on paper first.** `docs/BUS.md` was written and adversarially reviewed
  (24 findings, all but nits accepted) before any code; the op catalogue (173 ops) is registered
  and schema-published from phase 1 even though most answer `bus.not_implemented`.
- **D3 Sync handlers inside the request transaction.** Handlers are `fn(&mut Ctx, P) -> Result<R>`
  running with the store lock held; audit is appended in the same transaction; events and
  `after_commit` side effects run after commit. Async work returns a handle and finishes via
  events + a `<op>.completed` audit row (BUS.md §5.1). Simpler than async handlers and it makes
  "audit is the source of truth" literally true.
- **D4 Validation before authorization.** Own-task checks read payload fields, so a payload that
  fails schema never reaches policy (BUS.md §5).
- **D5 Guardrails bind agents through hooks, not `file.write`.** Providers write with their own
  tools; `guardrail.gate` + Claude Code PreToolUse hooks + a git pre-commit hook are the
  enforcement doors; Codex writes are post-hoc (BUS.md §9.3).
  *Note 2026-10-06:* the Codex clause is superseded by D132 and D163. Codex is hooked through
  `.codex/hooks.json`, its `apply_patch` edits meet one `write` gate per file and its shell
  commands the `exec` gate plus a `write` gate per visible target; no post-hoc watcher or
  `guardrail.violation` was ever built.
- **D6 `guardrail.confirm` re-executes with the confirm request's id** so the original id keeps
  replaying `held`; confirmer is the actor, original actor is `on_behalf_of` (BUS.md §9.4).
- **D7 One engine per instance** via `flock` on `<instance>.lock` plus a socket probe; the store is
  opened `locking_mode=EXCLUSIVE`. A held lock with a dead socket is refused, not taken over.
- **D8 Idempotency is forever** (`audit.req_id UNIQUE`), not a 24 h window; only audited requests
  are deduplicated. Queries and `invalid` requests are never audited.
- **D9 Settings are a dotted-path tree of leaf rows over a defaults tree**; `settings.set {path:""}`
  replaces the root (what `settings.reset` records as its inverse).
- **D10 Schema names are namespaced** (`TaskMoveIn`, not `MoveIn`) so `$defs` never depend on
  schemars' collision suffixes.
- **D11 Nullable patch fields** (`Option<Option<T>>`) use `relay_bus::nullable` so an explicit
  `null` clears and an absent field leaves alone.
- **D12 `TaskState` / `Priority` values** (BUS.md §11.1) confirmed 2026-08-17: `Priority` keeps
  v3's `low | medium | high | urgent` (not `normal`); `TaskState` as proposed. Breaking to change.
- **D13 Per-project guardrail overrides live in settings** at `guardrails.projects.<project_id>`
  (caps, thresholds), merged over the global `guardrails` subtree by `guardrail.config.get`.
  Decided in phase 2 so the v3 importer had somewhere to put v3's `max_files`/`max_lines`.
- **D14 v3 import mapping.** v3 "sessions" (`~/.config/dev.antho.relay/sessions.json`) are v4
  projects: the entry whose `repo` matches the project supplies its name and guardrail caps.
  Free-form v3 columns map by name to the five fixed ones (Queue → ready; unknown → backlog +
  warning); `description` + acceptance criteria + target files + comments fold into one `body`;
  `size_hint` small/medium/large → S/M/L; `priority` values carry over as-is; v3 `runs` are not
  imported (warning with the count). Import is one-time per source (`meta.import_v3:<dir>`,
  `conflict/import.already_done`); attachments are copied under the store's `attachments/`.
- **D15 Backups use SQLite's online backup API** from the request's own connection (the
  pipeline already holds the store mutex, so `Store::backup` would deadlock inside a handler —
  hence `backup_with`). Upgrades back up before migrating and refuse to migrate if that fails.
- **D16 Every FK gets an index, mechanically checked** by the migration test at every schema
  version (SPEC §1's rule, enforced rather than remembered).
- **D17 Worktree pool** at `<repo>/.relay/worktrees/<session-name>` on branch `relay/<name>`
  (v3's layout, kept); `.relay/` goes into `.git/info/exclude`, never the tracked `.gitignore`.
  Reads through gix, mutations through the `git` binary (SPEC §1). Removal purges build dirs
  first (`target`, `build`, `.gradle`, …) and reports bytes freed; the branch always survives.
- **D18 PTY teardown order is law:** SIGTERM to the child's process group, grace, SIGKILL,
  then drop writer + master so the reader thread ends on EIO. The exit callback holds a `Weak`
  engine and only transitions *live* states, so `shutdown()` marks sessions `restorable`
  *before* killing and `close` commits `closed` while the callback waits on the store lock.
  Every process exit is an audit row `session.spawn.completed` with `parent_req` = the spawn.
- **D19 "dirty" includes untracked files.** `gix::Repository::is_dirty` ignores them; an
  agent's new file is a change, so worktree listing and recovery use a status walk that counts
  untracked (`worktree::is_dirty`).
- **D20 Recovery reaps only its own store's orphans.** Spawned children carry `RELAY_STORE`
  (the engine's store path) beside `RELAY_SESSION`/`RELAY_INSTANCE`; a test engine can never
  reap the real dev engine's sessions, and two instances never cross.
- **D21 Phase 3 implements `session.create/spawn/get/list`** (registry phase 3, not 6): the PTY
  and worktree mechanics need real sessions to be testable. Until phase 6 a spawn is
  `<provider binary> [prompt]` with `settings.providers.<p>.path` as the override; profiles,
  hooks, resume, park and wake come with the Provider trait.
- **D22 Guardrail evaluation is pure; enforcement persistence is not.** `guardrail.check` and
  `guardrail.gate` share one evaluator, but only the gate writes holds/notifications. A held or
  refused gate commits that state and its error audit row in the same transaction, even though
  ordinary handler errors roll back. Confirmation skips exactly the policy that made the hold;
  every other policy is evaluated again against current files/config.
- **D23 Enforcement hooks are owned and reversible per worktree.** Each worktree gets
  worktree-local `core.hooksPath`; Relay chains the prior pre-commit hook, remembers its path,
  and restores it on close. Claude `PreToolUse` is merged into `.claude/settings.local.json`
  and only Relay's handler is removed. The hidden CLI adapter is argument shaping only; policy
  remains `guardrail.gate`, and adapter/infrastructure failures block instead of failing open.
  Lifecycle/reporting hooks still wait for phase 5's awareness ops.
- **D24 Authorization precedes implementation availability.** Registry schema validation still
  runs first (D4), then fixed actors + role/scope checks, then handler lookup. A denied future op
  therefore returns `actor.allowlist`/`actor.scope`, never `bus.not_implemented`; unfinished
  handlers cannot become an authorization oracle or temporary side door.
- **D25 Destructive-write counting is linear multiset comparison.** It counts removed/added line
  multiplicities in O(n), treating pure line movement as non-destructive and avoiding an LCS's
  quadratic worst case on generated files. Shape validators separately protect structured
  critical files where order/content semantics matter.
- **D26 Mailbox fan-out is one message plus durable recipient rows.** A broadcast keeps one
  canonical body while each live session gets its own acknowledgement timestamp. This makes
  unread state genuinely per-agent, preserves history across restarts, and avoids copying the
  same text N times. The original `to` selector remains visible as a session name or `*`.
- **D27 The session brief is deterministic provider-neutral text.** Core assembles five ordered
  parts (state, peers, standing notes, adjacent tasks, skills) and prepends the result as the
  provider's first prompt argument. `session.brief` returns the exact same text and parts for
  inspection. Skills state explicitly says Phase 11 until skills exist, so missing context is
  distinguishable from a broken injector.
- **D28 Overlap scans use gix plus tree-sitter, never heuristic regexes.** gix supplies changed
  paths and HEAD blobs; tree-sitter compares named declarations in Rust, TypeScript/TSX,
  Svelte, and Kotlin. Findings have stable content-derived fingerprints, so acknowledgements
  survive rescans while stale findings become inactive. Explicit agent claims share the same
  durable overlap surface.
- **D29 Claude lifecycle hooks are best-effort; enforcement remains fail-closed.** SessionStart,
  PostToolUse, Stop, and Notification report through `session.report` and may update state or
  create notifications/mail. Reporting failure exits zero because a Stop hook must never trap
  the provider. The PreToolUse adapter remains exit-2 fail-closed under D23.
- **D30 Provider discovery is explicit and cached.** `provider.list` never spawns a subprocess;
  it combines configured/PATH executables with the last `provider.refresh` result. Refresh runs
  bounded `--version` and auth-status commands, records the exact spawn profile, and notifies only
  on a change from a previously observed version. There is no background version watcher because
  idle subprocesses and polling are forbidden.
- **D31 Provider is immutable; launch shaping has one owner.** `providers::Driver` is the only
  place Claude/Codex fresh and resume arguments are formed. A session keeps its provider forever,
  and branch/model/effort become immutable after its first spawn. Park and shutdown snapshot PTY
  scrollback; wake/resume preload it and start a new epoch using the provider's own resume command.
- **D32 PAIR is two identities over one checkout.** Builder and reviewer have separate names,
  tokens, PTYs, providers, and audit actors but reciprocal pairing and one branch/worktree. The Git
  hook gates the runtime `$RELAY_SESSION` instead of its installer, and chained hook ownership is
  removed newest-first only when the last session leaves the checkout.
- **D33 Tauri keeps one control door and adds a Channel data plane.** Every UI action still enters
  the single `bus` command and the engine pipeline. `session.attach` supplies a Tauri Channel for
  base64 PTY frames; output is coalesced behind a 16 ms timer only after a frame arrives, so IPC is
  capped near 60 fps without an idle timer or polling loop.
- **D34 Module deletion remembers exact membership.** A deleted module is genuinely unlinked from
  its board tasks, as the bus contract says. `module_unlinked_tasks` remembers that membership so
  undo can restore only tasks that are still unassigned; a task deliberately reassigned while the
  module is deleted is never clobbered by restore.
- **D35 Phase 8 bus edits use the same frozen hold replay as provider hooks.** `file.write`, file
  mutations, and `git.commit` evaluate the phase-4 policy engine directly. A hold freezes the exact
  original op and payload; confirmation replays it while skipping only that hold's policy. This
  keeps the editor and agents on one policy path without turning `file.write` into an agent side door.
- **D36 Worktree refresh is event-driven and trailing-coalesced.** The first file or git read starts
  one `notify` watcher for that worktree. A filesystem event schedules one 125 ms trailing refresh
  burst and emits both `file.changed` and `git.changed`; there is no idle process or polling timer.
- **D37 Relay trash is data plus a durable pointer.** `file.delete` moves the entry to
  `<worktree>/.relay/trash/<id>/payload` and records its original worktree-relative path in schema v7.
  Restore conflicts instead of overwriting a replacement, so undo remains recoverable and explicit.
- **D38 Integration runs are durable before they are asynchronous.** `integration.request` commits a
  queued row, then an `after_commit` worker creates a throwaway worktree from the project's base,
  merges all selected branches, runs the configured build and optional device command, and advances
  state through system-audited writes. The worktree survives for inspection until explicit discard.
- **D39 CodeMirror owns editor state, while the bus owns files.** The Svelte Code workspace uses
  CodeMirror 6 for edit and merge views, but it never reads or writes the filesystem directly. Save,
  stage, commit, PR, search, trash, and integration actions remain typed bus operations; browser
  fixture mode implements the same interaction surface without Rust.
- **D40 Shell state has a core-owned serializable spine.** Page, pane, focus, and window metadata
  live in the engine and every mutation emits `ui.changed`. The Svelte shell mirrors those events.
  This preserves the `executor: ui` authorization boundary while making socket-driven UI requests,
  headless state inspection, and deterministic bus tests possible without a private frontend API.
- **D41 Named layouts persist opaque UI state.** Schema v8 stores project/name/state rows. Core owns
  naming, replacement, deletion, and inverse audit envelopes; the shell owns the state shape and
  applies it from `layout.changed`. A newer shell can extend that JSON without another migration.
- **D42 Resource sampling exists only while observed.** `app.resources.get` is an on-demand snapshot.
  `app.resources.watch {on:true}` starts one two-second sampler and closing the resource panel turns
  it off. Idle Relay has no resource timer or subprocess, preserving the phase-1 performance rule.
- **D43 Phase 9 chrome is navigation, not a second application layer.** Dashboard cards,
  notifications, settings, layouts, status panels, and the command palette all call typed bus ops.
  The sidebar and top bar select existing project surfaces; they do not duplicate task, session,
  file, or Git state in a shell-specific store.
- **D44 The shell has one theme source, and it is TypeScript.** `apps/relay-app/src/lib/theme.ts`
  declares every colour, radius, space step, type size and motion value; `applyTheme()` writes them
  as CSS custom properties on `<html>` before mount, xterm reads `xtermTheme()`, CodeMirror reads
  `cmTheme.ts`, and a dev-build assertion reads the vars back so no surface can drift (SPEC §13).
  Components consume only `var(--…)` aliases; the only literals outside the module are the first-
  paint `<html>` background in `index.html` and the Settings mode swatches, which read `PALETTES`.
  Modes are matte (default), dark and OLED; the panel-alpha slider is one rgb/alpha value on
  `--surface-1/2`, and floating panels use `--surface-solid-*` so menus never go translucent.
- **D45 The v4 visual world is the broadcast multiviewer.** Chosen 2026-08-17 through Impeccable's
  direction round (seed 12fd7365) over an assigned wire-desk direction: every session is a source
  tile with a tally frame and a UMD label bar *under* it, red means "needs you", green means live,
  amber means waiting on you, and colour means nothing else; primary actions are lit white keys;
  corners are square; the wall sits on 2px gutters. `PRODUCT.md` holds product truth for the skill,
  `DESIGN.md` records the built system. v3's WebKitGTK rules (no backdrop-filter, no masks/SVG
  filters, stripped native controls, transform/opacity motion) are binding, so glass is alpha only.
- **D46 The terminal is absolutely positioned inside its plate.** A content-sized xterm feeds
  FitAddon its own rendered height and the wall's `1fr` rows chase it (v3, round 6). `.terminal`
  is `position:absolute` in `.screen`, refits skip 0×0, and wall rows have a 280px floor with the
  wall growing (and `<main>` scrolling) instead of tiles overlapping.
- **D47 Device work exists only while requested and uses the Android platform tools already on the
  machine.** `device.list` is an explicit ADB query. A visible mirror owns an `adb exec-out
  screenrecord` H.264 process and a bounded catch-up buffer; closing it tears the process down.
  Device runs are durable rows before their Gradle or custom command starts, then expose one bounded
  build and logcat stream over both doors. A restart closes interrupted rows as stopped. This avoids
  an idle watcher and avoids making a separately installed scrcpy server a hidden Relay dependency.
- **D48 Skills are global rows with durable project enable edges.** Schema v10 keeps soft-deleted
  skill rows so audit undo restores the same id and enable set. Brief assembly reads only enabled,
  live skills in deterministic name/id order and injects their markdown verbatim. Plugins remain a
  real, implemented empty query in 4.0 rather than a UI-only promise.
- **D49 MCP is a process adapter, not a fourth execution path.** `relay mcp` speaks stdio JSON-RPC,
  renders tools directly from the bus registry schemas, and sends every call through the Unix socket
  and normal authorization/audit pipeline. Stream and Tauri-only ops are omitted because MCP has no
  Relay data-plane attachment. Claude launches get an explicit untracked MCP config that inherits
  the session identity environment; Codex keeps the CLI door until its binding changes.
- **D50 First-run composes existing ops.** The shell detects providers, creates a workspace and first
  project, and optionally invokes the v3 importer without a private setup API. `workspace.create`
  creates a missing absolute directory, while the project must remain an existing Git repository.
  Appearance polish stores a compositor-sized wallpaper plus a separate small preview, and shortcut,
  provider-path, and guardrail edits remain ordinary settings/bus mutations.
- **D51 Installed skills keep GitHub as their source of truth.** The visible Skills page installs and
  refreshes bounded `SKILL.md` files from a GitHub repository instead of authoring instruction text.
  Schema v11 records source URL, relative path, and revision while project enable edges remain stable.
  Dispatch still injects the stored revision deterministically, so a remote change cannot alter a
  running task until the user explicitly refreshes it.
- **D52 GitHub authentication belongs to GitHub CLI.** Relay starts `gh auth login --web --clipboard`,
  observes completion through `github.changed`, and asks `gh api --paginate user/repos` for every
  accessible repository. Tokens remain in GitHub CLI's credential store and never enter Relay SQLite.
- **D53 First-run discovers before it creates.** `workspace.discover` derives an omitted path from the
  process current directory and scans bounded descendants for Git roots. The user selects a local root
  or a connected GitHub repository; `project.clone` then places remote repositories inside the chosen
  workspace. Retrying a failed clone or import reuses already-created setup records.
- **D54 AVDs reuse the Phase 10 device pipeline.** Relay calls installed Android SDK tools only on
  explicit `avd.*` requests. AVD creation uses installed images and profiles, boot launches the
  emulator, and ADB discovery supplies its serial to the existing mirror/deploy/logcat operations.
  No emulator process, SDK scan, or device watcher runs while the surface is idle.
- **D55 Blank workspace discovery is repository-aware.** A desktop development build can inherit a
  deep directory such as `apps/relay-app/src-tauri` as its process CWD. If that directory is inside
  a Git checkout, Relay selects the checkout's parent so the repository is discovered as a project
  instead of becoming an accidental workspace or clone destination. Explicit paths are unchanged.
- **D56 Registry removal forgets Relay metadata, never repositories.** Project removal refuses live
  sessions, integrations, and device runs, then clears project-owned Relay rows in foreign-key order
  while preserving the audit trail and every filesystem path. Workspace removal still requires its
  projects to be removed first.
- **D57 Code browsing is lazy and refresh work is bounded.** The visible file tree reads one directory
  level at a time, overlapping refresh requests collapse into one follow-up, and recursive watchers
  ignore high-churn build/cache paths while retaining source and Git metadata events. Generated
  directories are omitted from the tree so a local compiler cannot starve Relay's control bus.
- **D58 Navigation names destinations; window controls arrange them.** Board and Notes sit in the
  sidebar under Plugins, while the top bar keeps only Agents/Code plus global actions. The old
  File/Edit/View labels duplicated navigation and hid capabilities behind misleading desktop-menu
  names. Code pane visibility, widths, and the Git graph are durable per-project layout state and are
  also captured by named window presets. Terminal attachment reacts to lifecycle values, not session
  object replacement, so ordinary shell refreshes cannot replay every existing terminal.
- **D59 Session context is a worktree file, not prompt payload.** This supersedes D27's transport
  detail while preserving its deterministic content and `session.brief` inspection API. Before every
  start, wake, or resume, core writes `.relay/session-brief.md`; the provider receives only a short
  instruction to read it plus any assigned task text. This keeps large peer, note, and skill summaries
  out of the launch argument while leaving them available to the agent on demand.
- **D60 Session chrome stays neutral and moves above output.** This supersedes D45's tally frame and
  below-output UMD placement. The colored tally rails competed
  with terminal output and became especially noisy across a multi-agent wall. State remains visible
  through the compact header lamp, label, and blocked text; terminal plates have no colored or focus
  outline, and their header separator remains neutral. The header sits above output so controls stay
  spatially stable and opening a side-tile menu cannot refocus and reorder a review layout.
- **D61 Ordinary Tauri bus calls share one inert data Channel.** The single Tauri command keeps a
  Channel argument because PTY, mirror, and run attachments need it, but allocating a fresh native
  callback for every control-plane query or mutation created needless WebKitGTK IPC allocator churn.
  The UI now reuses one control Channel and allocates dedicated Channels only for live streams. Two
  retained native crash dumps ended in this IPC path, so this is also a crash-containment boundary.
- **D62 Project navigation opens work; top-bar chrome does not duplicate it.** This supersedes D58's
  Agents/Code switcher. Selecting a project opens its Agents canvas and the top bar keeps only the
  Code shortcut. Settings replaces project chrome with a back action and no sidebar. Project Rename
  is an inline, select-all edit in the sidebar; its compact overflow menu drills into less frequent
  base-branch and command forms instead of expanding one large editor.
- **D63 Agent tiles use normalized freeform bounds.** The Agents canvas stores snapped `x`, `y`, `w`,
  and `h` percentages under `agents.canvas.<project_id>`. Percentages preserve the arrangement across
  window sizes, while minimum pixel dimensions protect terminal usability during a drag or resize.
  Device mirror and run sources stay in their own bounded strip because their aspect and lifecycle
  constraints differ from interactive terminal tiles. Git history similarly stores a dedicated
  vertical split alongside the existing Code pane layout.
- **D64 Usage is harvested from provider work already in flight.** `usage.get` remains the only UI
  query. Claude's Relay-only `--settings` file installs a silent status-line command that writes the
  rate-limit payload Claude already supplies after a turn. Codex usage is read from the newest
  bounded tail among recent rollout files, where the CLI records the same rate-limit event shown by
  its own status line. Neither path starts a provider process, calls an endpoint, polls in the
  background, or spends a token. Stored `usage.report` rows remain a compatible fallback.
- **D65 Wallpaper legibility has two independent controls.** Wallpaper dim changes the image layer;
  content protection raises effective panel opacity without mutating the chosen theme or the user's
  base panel-opacity value. Both preview live, persist with the appearance subtree, and are included
  in the pre-paint cache so a bright wallpaper cannot flash behind low-contrast text at startup.
- **D66 Observed resource history lives in the observing UI.** Core continues to emit bounded
  `resource.sample` snapshots only while `app.resources.watch` is on. The open resource panel keeps
  the last 30 samples and draws CPU, process-memory, and worktree-disk trends; closing it drops that
  transient history and stops sampling. No new database rows or idle timer are introduced.
- **D67 Device creation defaults are recommendations, not required form work.** The UI ranks installed
  images by API level, Google services flavor, and host-friendly architecture, then chooses a Pixel
  profile and derives an editable AVD name. The raw image/profile selectors remain behind Change.
  Existing typed `avd.create` and `avd.boot` operations stay authoritative; quick actions add cold
  boot and serial copy without turning Relay into an SDK package manager.
- **D68 Notification sounds are local synthesis presets.** Off plus four short Web Audio patterns are
  bundled as code, preview on selection, and play only for enabled categories after a `notify.new`
  event. Relay stores a stable preset id, loads old boolean settings compatibly, and has no media
  download, audio file lifecycle, or background sound process.
- **D69 Branch creation is a typed worktree mutation.** The Code surface never invokes Git directly.
  `git.branch.create` validates names with `git check-ref-format`, resolves the optional start point,
  refuses duplicates with stable codes, and creates or switches in the explicitly selected worktree.
  Focused bus coverage repeats the operation across many branches so the native branch UI does not
  reintroduce an untyped side door while the separate WebKitGTK allocator investigation remains open.
- **D70 File and module iconography is semantic data, not downloaded art.** The file tree maps common
  names and extensions to compact colored glyphs with a neutral fallback. Modules store one of 18
  stable icon ids and render them through a single stroke pack while still displaying legacy custom
  icon strings. No asset fetch, image persistence, or platform font dependency is introduced.
- **D71 Dense creation surfaces use grouped choices and a live identity preview.** Module creation is
  a composed identity, icon, and priority form rather than a horizontal field run. Task creation keeps
  its dedicated page but replaces size and priority selects with compact visual choices and reflects
  the selected metadata in a restrained preview strip. The underlying bus payloads are unchanged.
- **D72 Notes are plain-text documents with UI-local edit buffers.** Notes keep the existing durable
  bus and database contract so dispatch and agent append remain deterministic. The desktop surface
  adds an indexed library, open-document tabs, one unsaved draft per opened note, explicit save and
  discard, find/replace, text-format helpers, wrap/font controls, and Ctrl N/S/F. Rich-document state,
  proprietary formatting, and standalone-editor features that cannot survive agent reads are omitted.
- **D73 Dashboard workload summaries are assembled by one global query.** `dashboard.get` adds a
  per-project summary of open, ready, active, review, recent-done, live-session, and blocked-session
  counts alongside the existing decision, activity, and resource data. The control-room UI derives
  every card from this response and deep-links to existing project pages; it introduces no polling,
  cached dashboard table, or decorative controls that pretend to act.
- **D74 Skill replacement is explicit only for visible conflicts.** The skills table keeps deleted
  rows so undo can preserve ids and project enablement, but those rows are absent from `skill.list`.
  `skill.install` therefore reclaims an invisible deleted name automatically instead of reporting a
  phantom conflict. A live different-source collision returns structured installed and incoming
  identities; `replace_skill_id` grants atomic replacement permission for that exact row only, so
  the UI can offer review, refresh, and replacement without delete-then-install data loss.
- **D75 Session controls float above the canvas, while session geometry stays canvas-owned.** The
  session menu is one viewport-bounded overlay rather than a descendant of a clipped terminal tile.
  A single session always claims the whole wall regardless of stale saved bounds. Freeform move and
  resize use the same desktop mouse-event path proven by the native window handles, then persist the
  snapped normalized bounds from D63.
- **D76 Wallpaper is a library, not one replaceable setting.** Appearance keeps an ordered set of
  downscaled full images and previews plus one selected id. The current `wallpaper` fields remain as
  a compatibility projection for pre-paint startup and older settings. Content and console plates
  both respect panel opacity; xterm enables transparent canvas rendering so its CSS plate can show
  the selected wallpaper without weakening overlays.
- **D77 Resource snapshots include Relay itself.** `app.resources.get` and `resource.sample` report
  the current Relay app/engine PID, RSS, and sampled CPU beside agent processes and worktree disk.
  This remains requested-only sampling while the resource panel is open, preserving D66.
- **D78 Canonical skill entries outrank provider adapters.** A repository may mirror one named
  skill under top-level `skills/` and `.agents`, `.claude`, `.openclaw`, or another adapter folder.
  Relay prefers `skills/`, then its native `.agents/skills/` form, then ordinary non-hidden paths,
  and collapses byte-identical same-rank copies before touching SQLite. Different same-rank bodies
  still require an explicit subdirectory.
  This keeps a multi-provider repository install atomic instead of reporting an inserted row that
  disappears when a later adapter copy rolls the transaction back.
- **D81 Installed skills group by repository source.** A GitHub source containing several skills is
  one collapsible parent in the Skills list, with the individual skills and project enablement
  switches beneath it. Single-skill sources stay flat. Disclosure never changes enablement or the
  selected preview, keeping the workspace/project tree interaction consistent.
- **D82 Provider-native roles replace startup prompts.** This supersedes D59's transport instruction.
  Fresh, wake, and resume launches carry no positional prompt. Claude appends an untracked
  `.relay/role-instructions.md` to its normal system prompt; Codex receives the same role through its
  `developer_instructions` config override. The first real user turn calls actor-bound
  `session.bootstrap`, whose empty payload is resolved from the session token and returns only task,
  branch, module, pair, and optional durable launch text. The complete `.relay/session-brief.md`
  remains an inspectable snapshot but is never loaded merely because a provider started. Schema v12
  stores optional launch text so removing the positional prompt does not discard public bus input.
- **D83 Canvas measurement follows the canvas node, not component mount.** The Agents wall may mount
  empty or switch through focus/review layouts where the freeform canvas does not exist. Its resize
  observer attaches reactively whenever that node appears. Session additions reflow into distinct
  default slots, stale placement reads are rejected by project and membership key, and settings
  receive plain placement snapshots rather than Svelte state proxies.
- **D84 Agent tiles occupy non-overlapping dock slots.** This supersedes D63 and the freeform geometry
  portion of D75 and D83. One session fills the wall; two split it; three use a primary half plus a
  side stack; four use quarters; larger sets use bounded two- or three-column grids according to the
  measured wall width. Header drags swap ordered slots stored at `agents.dock.<project_id>` and never
  create arbitrary rectangles. Review renders every keyed terminal once so promoting focus cannot
  reuse one xterm for another session, and terminal attachments reject mismatched session frames.
- **D85 The Agents grid uses the proven v3 two-column pane model.** This supersedes D84's count-based
  templates and swap-only drag. Project layout stores explicit ordered `a` and `b` columns plus a
  clamped split at `agents.dock.<project_id>`. New sessions enter the lighter column, a lone occupied
  column expands to full width, and narrow walls stack the same two columns. Header drops move a pane
  across columns or insert it above or below another pane; arbitrary coordinates and overlap remain
  impossible. The model is adapted from v3's `layout.js` and `Canvas.svelte`, with Relay-2 settings
  bus persistence and measured wall width replacing local storage and a viewport media query.
- **D86 Solo terminal columns use two grid tracks.** Hiding the resize seam removes it from grid
  placement and leaves only the `a` and `b` column elements. The solo rules therefore collapse one
  of exactly two tracks. Keeping the normal layout's third track would place a right-only column in
  a zero-width track and make every surviving terminal disappear until the left column was repopulated.
- **D87 Device mirrors are satellite windows.** This supersedes the mirror portion of D63. Opening a
  mirror creates or focuses one native window per device; that webview owns the H.264 Channel and the
  requested-only mirror runtime, and closing it stops the ADB capture before the window exits. Run and
  logcat sources remain in the Agents device strip. A phone mirror no longer consumes the workspace
  needed to edit code while watching the device.
- **D88 Device mirrors decode frames directly before falling back to MSE.** Android `screenrecord`
  can emit one complete static frame and then stay silent. JMuxer and WebKit's media element retain
  that frame until another arrives, making startup take seconds and every interaction look one frame
  behind. The mirror therefore uses low-latency WebCodecs in AVCC mode and paints decoded frames to a
  canvas, matching the proven v3 decoder strategy. MSE remains the compatibility fallback; the Tauri
  door's render-interval batches are marked complete there so JMuxer does not add another frame of
  parser delay. No poller or periodic flush is introduced.
- **D89 Mirror input uses Android's direct input service command.** Mirror coordinates are scaled to
  the physical display once in the bus handler, then delivered through `adb shell cmd input` instead
  of the slower wrapper script. The typed input op and validation remain unchanged; this only removes
  avoidable process startup from taps, swipes, keys, and text.
- **D90 The vendored scrcpy transport is the device mirror.** This supersedes D88 and D89. Android
  `screenrecord` exposes an unframed byte stream whose ADB read boundaries are not H.264 access-unit
  boundaries; treating them as frames caused corruption, buffering, and decoder stalls. Relay now
  bundles the same pinned scrcpy v4.1 server as v3, reads its length-prefixed config/key/delta packets,
  and feeds those exact access units to the proven WebCodecs-first decoder. Input uses scrcpy's
  persistent control socket with explicit touch DOWN/MOVE/UP messages, so clicks and drags no longer
  spawn Android commands or wait for stale video. The existing typed bus ops remain the only control
  plane; the mirror channel remains the requested-only data plane.
- **D91 Device run logs follow the launched app, not the whole phone.** After install and launch,
  Relay resolves the package PID and attaches `logcat` only to that process and the main, system,
  and crash buffers. The Run pane queues incoming lines and commits one bounded batch per animation
  frame. This restores v3's behavior and prevents device-wide log traffic from monopolizing the
  WebKit main thread immediately after a successful build.
- **D92 Primary device runs verify upstream before Gradle.** A requested run from the canonical
  project checkout fetches its configured upstream and fast-forwards only when the checkout is
  clean and strictly behind. Before applying an update that removes or renames tracked paths,
  Relay preserves the prior commit under `refs/relay/snapshots/device-sync-*`, so every prior file
  remains recoverable without blocking a legitimate upstream update. An up-to-date or ahead-only
  primary runs locally. Dirty, diverged, or unreachable states fail with stable `device.sync_*`
  codes before Gradle starts. No force, reset, push, merge commit, or polling is allowed, and linked
  session or integration worktrees are never synchronized by a run.
- **D93 Guardrail hooks persist and refresh an absolute Relay CLI path.** A desktop launcher does not
  inherit the user's interactive shell PATH, so a bare `relay` command can strand an otherwise valid
  worktree. Relay resolves a CLI built or packaged beside the app before PATH, rewrites the hook on
  session launch, and refreshes an existing Relay-owned hook before UI commits. User-owned hook paths
  remain untouched and continue to chain through D23.
- **D94 Tauri bus commands acknowledge synchronously and complete through a response event.** The
  retained native crashes all detected allocator corruption in WebKitGTK's main loop while Wry was
  completing a custom-URI request, including ordinary bus calls with backend work still active.
  The single `bus` command now validates its arguments, returns `()` immediately, dispatches on the
  async runtime, and emits the full typed `Response` on `bus:response` with the original request id.
  Data still uses the existing Tauri Channels, and every operation still traverses the one engine
  pipeline. This removes the long-lived custom-URI responder without adding a side door.
- **D95 Appearance persistence is leaf-granular across the Tauri door.** Wallpaper images are large
  data URLs, and the old settings form resent and echoed the complete wallpaper library on every
  unrelated Save. Startup also fetched both the library and selected image as one multi-megabyte
  object. Relay now reads only the appearance leaves each surface needs, saves ordinary visual
  controls independently, and sends library or selected-image leaves only when those values change.
  Existing settings paths and wallpaper data remain compatible.
- **D96 Bus-driven commits execute the user hook and guardrail exactly once.** A generated Relay
  pre-commit hook normally calls the socket door, but `git.commit` already holds the engine's store
  transaction while its in-process guardrail runs. Invoking the generated hook from that operation
  deadlocked against the same transaction. The handler now refreshes the hook, runs only the chained
  user-owned pre-commit hook, evaluates the final staged diff in-process, then asks Git to commit
  without re-running hooks. Direct terminal commits still traverse the generated Relay hook normally.
- **D97 Hidden work keeps its state, not its workload.** The Agents and Code surfaces stay mounted
  after first use so page navigation does not discard xterm or CodeMirror state. Hidden terminal panes
  detach from their channel while retaining their terminal and cursor, then request only output since
  that cursor when shown; focus tabs follow the same rule. PTY catch-up is capped at 256 KiB on a saved
  frame boundary: full-screen agent TUIs produce megabytes of cursor-addressed redraws from only a few
  hundred logical lines, and 1 MiB still left a newly selected project's text visibly blank while xterm
  parsed obsolete frames. The 8 MiB ring remains available through explicit scrollback. Hidden Code
  marks matching events stale and performs one refresh when it becomes visible.
- **D98 Code refreshes reuse status and scope their invalidation.** Worktree membership validation
  reads repository metadata without dirty-scanning every checkout. `worktree.list` can omit dirty
  scans, and `file.tree` can omit Git badges so the Code surface projects the already-requested
  `git.status` result onto every lazy directory. Watcher events carry project/worktree identity and
  emit one file invalidation instead of paired file/Git invalidations; recursive registration runs
  after the store lock is released and emits one reconciliation event once active. Provider reports emit
  session/task events only on visible state changes, and shell event reloads are sliced and coalesced.
- **D99 Resource sampling caches filesystem totals.** Opening the panel measures each active worktree
  once with one traversal that separates build bytes while counting total bytes. Subsequent two-second
  samples read session rows under the store lock, release it, then sample process counters and reuse
  cached disk totals. A newly observed worktree is measured outside the store lock; no periodic scan
  can starve the command bus.
- **D100 Development replacement is a graceful restart.** The Tauri process handles SIGTERM from the
  development runner, persists live PTY scrollback, marks sessions restorable, and then exits through
  Tauri. A Rust rebuild can still replace the dev UI, but it no longer turns that expected replacement
  into an unpersisted session loss.
- **D101 The brief is delivered on every spawn, and split in two.** A 28 KB `session-brief.md`
  was written into each worktree and handed to nobody: it reached an agent only alongside a
  dispatched task, so a session spawned without one — the case that most needs to know who its
  peers are — received only its 319-byte role instruction. The brief is now split. The compact
  half (state, live peers, standing notes, adjacent work, comms) is appended to the role
  instruction file that both providers already read, so it arrives on every spawn through a
  channel that was already working; skill bodies go to `.relay/session-skills.md` and are
  referenced by path. The compact half never carries the launch assignment: for Codex it ends
  up in argv, which `ps` shows to everyone, and `session.bootstrap` already delivers it
  privately. The on-disk copy, which lives inside the worktree, still carries it.
- **D102 Scratch space is not a repo-integrity concern.** The `PreToolUse` guardrail refused any
  write outside the worktree, including the session scratchpad the provider harness requires its
  agents to use — two mandatory systems issuing contradictory instructions, with the only
  workaround being to drop temporary files into a real git worktree. Writes now resolve against
  a list of roots: the worktree, `guardrails.allowed_write_roots`, and the process temp
  directory. Anything else is `refused` with code `guardrail.write_root` and a message naming
  the roots that would have worked — not `unavailable`, which described a deliberate policy
  decision as an outage and invited a retry.
- **D103 Denied commands match parsed argv, never a raw substring.** `denied_commands` was
  matched against the whole command line, so the string appearing as quoted data was enough to
  block a command — including `guardrail.check` itself, which exists precisely so an agent can
  ask "would this be allowed?" before acting. The safe way to inquire was the one way that was
  blocked. Relay now reads the line into commands and words, tracks which words came wholly from
  inside quotes, and matches a pattern only against unquoted words in one command. A word that is
  only partly quoted counts as unquoted: when in doubt, still inspect it. A `relay … guardrail.check`
  command is skipped outright, and only that command — anything chained after it is still judged.
  The 2026-10 audit (RA-010, RA-011) found the reader and the matcher both too literal: commands
  inside `( )`, `$( )`, backticks and `sh -c` were never seen, and a pattern matched only as its
  exact words in a row, so `git push origin main --force`, `git clean -fdx`, `rm -fr` and
  `git -C app reset --hard` all ran. `shell.rs` is now the one reader (the device-lease gate uses
  it too); it follows POSIX backslash rules, splits on subshells and substitutions, reads
  `$( )` inside double quotes and the argument of `sh -c` / `eval` as commands of their own. A
  pattern is read as program, subcommands and flags: the program matches by basename anywhere in
  the command (so `sudo` and `find -exec` still count), git's value-taking global options are
  skipped before the subcommand, flags match as a set anywhere after it with clusters expanded
  (`-fdx` holds `-f` and `-d`), `--force`/`-f` and `-R`/`--recursive`/`-r` are one flag for `rm`
  and `git`, and `+ref` counts as a forced push. Quoted text containing a space is still data.
  A later review found the other ways to destroy a remote branch untouched by the default
  `git push --force`: `:ref`, `--delete`/`-d`, `--mirror`, `--prune`, `--force-with-lease` and
  `--force-if-includes` now count as a forced push too.
- **D104 One answer to "may I call this?", computed once.** Relay gates in three layers: registry
  `actors`, runtime row scope, and the per-role allowlist. `bus.ops` and the MCP tool list knew
  only the first, so `relay ops --mine` reported 133 callable ops when a builder's real write
  surface was 13, and the MCP server advertised ~115 tools the allowlist refuses at runtime. A
  discovery tool that overstates capability tenfold is worse than none: it moves the refusal from
  planning time to execution time. `guardrail::callability` now answers all three layers in one
  place, and `bus.ops`, `relay ops --mine`, `session.bootstrap.can_call` and the MCP tool list all
  read it. Every `bus.ops` row carries `call` (`yes` / `no` / `self_only`) and `why`.
- **D105 `session.bootstrap` spends the one guaranteed payload on what an agent cannot guess.**
  Every agent is instructed to call it first, and it returned only identity — facts already in
  the environment — then stopped. It now also returns the live peer table, every op this session
  may really call, the brief path, the comms hint, and the guardrails it will meet (caps, denied
  commands, write roots, and the name of the dry-run op). Every field is data the engine already
  held; standing notes and skill bodies stay in the brief.
- **D106 Session visibility stops at the caller's project, in both directions.** `session.list`
  returned every session in every project, with pid, provider ref and worktree, while
  `session.get` refused a peer in the caller's *own* project. Inconsistent boundaries cannot be
  reasoned about, so agents probe to discover them — exactly the behaviour not to train in. One
  rule now: an agent sees the metadata of any session in its own project and nothing outside it.
  `session.list` is scoped, `session.get` admits same-project peers, and the private surfaces
  (`session.scrollback`, `session.brief`) stay own-session-or-PAIR.
- **D107 Codex parity is stated, not assumed.** Codex takes no `--mcp-config` and no `--settings`,
  runs no `PreToolUse` hook and files no lifecycle report, so the provider with less supervision
  also had less instrumentation. `developer_instructions` is the only channel Relay has to it, so
  it now carries the brief, the role instruction, and the `$RELAY_BIN q <op>` shell path to the
  bus. Where parity is still absent, Relay says so: `ProviderInfo.guarded` is false for Codex and
  the settings surface labels it, so unenforced sessions are a known state rather than an
  assumption.
- **D108 An unhooked provider is still observable, through its PTY.** `last_output_at` was only
  ever written by `session.report`, so every Codex session read as one that had never done
  anything. The PTY reader now stamps an in-memory timestamp on each read — no DB write, no timer
  (SPEC §15) — and `session.get`, `session.list` and the peer table overlay it when it is newer.
- **D109 The mailbox reports where a message stands, not a `delivered_at` it cannot know.**
  `mailbox.send` returned `acked_at: null` and nothing else, and a sender could not see its own
  sent mail at all. Send now returns the addressees and a delivery hint (`queued`,
  `session_parked`, `not_running`, `no_recipients`), and `mailbox.outbox` lists what this actor
  sent with each addressee's state. Relay deliberately does not add a `delivered_at` column:
  there is no injection step between storing a message and the recipient reading it, so the
  timestamp would only ever restate `sent_at`. The addressee's live state is the honest answer.
- **D110 The engine fills in the identity an agent already carries.** An agent *is* a project and
  a session, and was still required to restate both on every call, so each op began with a
  guess-and-retry against `bus.schema`. The pipeline now fills `project_id` and `session` from
  the authenticated session when an op **requires** them and the payload omits them. Only
  required fields are filled: an optional `project_id` is usually one half of an either/or, and
  filling it would break the other half. Ops with two optional targets resolve their own default
  from the actor instead — `session.peers` with neither means "my project, minus me".
- **D111 An omitted `worktree` means the caller's own tree.** `worktree` is optional on `git.*`
  and `file.*`, and defaulted to the project root. This was the one defect that produced a
  confident, well-formed, *wrong* answer with no error either way: an agent reviewing "its" diff
  before `session.done` read a tree it had never touched, saw nothing, and could reasonably
  conclude it had changed nothing. For an agent the default is now that session's worktree; the
  project root is reachable by asking for it out loud, as `worktree: "@project"`. `guardrail.check`
  judges the same tree, for the same reason.
- **D112 A destructive write is measured against the file, not against the diff.** Two false
  positives made the rule fire on writes that lost almost nothing. The percentage test had no
  floor, so any file of one or two lines was permanently 100% and could not be edited without a
  hold — while the absolute limit it was breaching was fifty lines. And on the diff-only path the
  unknown old size was substituted with the removed count, which made *every* such write come out
  at exactly 100% regardless of the file it applied to. The rule now takes the old size from the
  file on disk in both paths, and applies the percentage only to files of at least
  `destructive_write.min_file_lines` (default 30); the absolute limit still applies at any size.
  A guardrail that fires on writes nobody would call destructive teaches agents to route around
  it, which costs more than the rule was protecting.
- **D113 A `*` in a protected path stays inside one segment.** The glob matcher treated `*` as
  "any characters", separators included, so `src/*` silently protected everything under `src`
  and a pattern written for one directory locked a whole tree. `*` now matches within a segment
  and `**` crosses them, which is what the documentation always claimed and what anyone writing
  a pattern expects. Over-broad protection is not the safe direction: it trains agents to treat
  refusals as noise. A `**/` component matches zero or more whole directories, so `**/*.pem`
  covers `c.pem` and `a/b/c.pem` alike; the first matcher kept a single backtrack point and
  matched exactly one directory deep (RA-012). A `**` that is not a whole component (`**.pem`)
  still crosses separators. A glob naming a directory does not cover its contents; a literal
  path does.
- **D114 A destructive write is judged by what would be lost, not by how much changed.** Volume
  is a proxy; the question that matters is whether the content can come back. When the rule is
  about to fire, Relay asks git: a file that is committed and unmodified is one `git checkout --`
  from restoration, and an ignored file is build output, so both are allowed through. Untracked
  or modified files hold, because those bytes exist nowhere else. The check runs only on the
  path that was about to block, so ordinary writes pay nothing for it, and
  `destructive_write.allow_if_recoverable` turns it off for anyone who wants volume alone.
- **D115 `bus.wait` is the wake-up an agent has instead of a poll loop.** Everything was
  poll-only: `bus.subscribe` is socket-only with no agent-shaped consumer, so coordination meant
  re-reading `mailbox.list` on a guess. `bus.wait {events, timeout_ms}` blocks until a matching
  event arrives. It is answered by the socket door, like `bus.subscribe`, so the wait happens in
  that connection's task and never holds the engine's store lock — no timer exists anywhere, which
  is what SPEC §15 asks for. Events that fired before the call are not replayed: check state
  first, then wait.
- **D116 A claim is declarable, and a collision is information rather than an error.** The
  `claims` table, the `claimed` field on every peer and the whole `overlap.*` namespace already
  anticipated "who owns which files"; an agent had no verb to participate. `session.claim` takes
  paths and returns both the claim and any peers already holding them, so a collision becomes
  something to negotiate over `mailbox.send`. `exclusive: true` refuses instead, recording
  nothing. `session.release` gives them back. Claims are advisory: Relay reports them and does
  not enforce them, because two sessions legitimately share files and the fleet, not the bus,
  knows when that is fine.
- **D117 `guardrail.explain` preflights a plan, `guardrail.check` an action.** Caps and protected
  paths were only discoverable by tripping them, which is late: an agent has already committed to
  an approach by the time the commit is refused. Explain takes the shape of intended work — paths,
  a line total, commands — and returns a verdict per item plus the total against the caps. It is
  a query: it evaluates, holds nothing, writes nothing.
- **D118 `session.done` can say it did not finish.** The payload was a success shape, and an op
  that can only report success gets reported success. `status` is now `completed` (unchanged
  behaviour), `blocked`, or `partial`, and the latter two require `blockers` — saying you are
  stuck without saying what stuck you is not a report. A builder that stops short leaves its task
  in `active`/`blocked` rather than handing unfinished work to review, and the notification says
  what stopped it.
- **D119 `bus.whoami` answers for any actor.** Identity, role, project, worktree, callable ops and
  write roots lived across four calls plus knowing to look inside `guardrail.config.get` for the
  role list. `session.bootstrap` now gathers that for agents, but it is agent-only and
  assignment-shaped; `whoami` is the same three-layer answer for whoever is asking, the user
  included.
- **D120 Device logs are a live tail, not workspace history.** The Agents wall keeps every active
  device run, but once no run is active it retains only the newest completed source for diagnosis.
  Switching projects drops sources from the previous project. The native door batches run output once
  per frame, the runtime retains 512 recent lines, and the visible pane renders 400. Hidden panes
  detach from their feed, and off-screen logs skip auto-scroll plus remain layout/paint-contained, so
  adding a session does not reflow or synchronously measure an unbounded Gradle transcript.
  Session launch and lifecycle actions refresh only `session.list`, because providers, projects,
  usage, notifications, devices, and appearance do not change when a terminal is added or resumed.
  Durable run state remains available through `device.run.list`; raw build output is intentionally not
  a permanent log archive.
- **D121 Notification links are places, not commands.** Core stores a notification's `link` as the
  op that names the thing — `task.get`, `session.get`, `guardrail.confirm` — and the notification
  centre reads that name to decide where to navigate; it never dispatches it. Dispatching would have
  fetched rows nobody reads, and for a hold it would have answered the guardrail on the human's
  behalf from a single click. A task link opens that task's detail on its project's board; a session
  or hold link opens the agent wall focused on the session, since a hold is answered inside the
  session that tripped it. A link the UI cannot route shows no affordance and only marks read.
- **D124 Provider-native completion signals backstop agent self-reporting.** `session.done` remains the
  explicit path that moves builder work to review, but notifications cannot depend on a model always
  remembering that call. Claude Stop hooks and Codex's `agent-turn-complete` notify command both enter
  through the existing `session.report` bus op. A running-to-idle stop emits one completion notification;
  an explicit `session.done` makes the session idle first and therefore suppresses the fallback duplicate.
  Provider discovery probes version and authentication concurrently inside the explicit refresh request;
  no poller or idle process is added.
- **D125 Mouse Back uses Relay's own bounded page history.** WebKitGTK exposes the two thumb buttons
  as buttons 3 and 4 but a Tauri webview has no useful browser history to consume them. Relay records
  its last twenty surface changes instead. Back closes the topmost open sheet or popover first, then
  returns through those surfaces; both thumb buttons intentionally perform Back because Relay has no
  Forward destination.
- **D126 Terminal zoom hides panes without stopping them, and Clear fresh-starts restorable panes.** A
  zoomed wall keeps every xterm mounted at a real size and attached while only one is visible, so other
  agents continue producing and avoid the zero-size FitAddon loop. Clear is available only for a
  `restorable` session: `session.clear_restorable` drops its saved provider handle and scrollback, then
  fresh-spawns the same durable session in the same worktree. Resume keeps context; Discard removes the
  terminal and may remove its worktree.
- **D127 Desktop image drops use Tauri paths, browser drops use File objects.** Tauri intercepts OS file
  drops before WebKit can populate `DataTransfer.files`, so the Board listens to the native window event,
  converts its physical coordinates to CSS coordinates, and resolves a marked create zone, detail zone,
  or task card. Existing tasks use the path form of `task.attach`; create stages paths until the task has
  an id. The ordinary browser/file-picker path remains byte-based and fixture mode implements both forms.
- **D128 Git lanes are row-contained.** The old decorative rail painted one lane for every history and
  gave its node a root-level z-index, which both lied about merges and escaped above unrelated chrome.
  `commitGraph.ts` derives lanes from parent SHAs and each fixed-height row renders one clipped SVG.
  The Code surface also owns an isolated stacking context, so its selector and resize layers cannot
  outrank app menus or the status bar.
- **D129 Push establishes a missing upstream.** The Code surface cannot know whether a branch has
  ever reached its remote, and requiring it to opt into `set_upstream` made the normal first push of
  every Relay worktree fail with Git's setup instructions. `git.push` now inspects the selected
  branch: when no tracking branch is configured and the caller did not override the choice, it runs
  `git push -u origin <branch>`. Later pushes remain plain `git push`; explicit `set_upstream: false`
  preserves the low-level escape hatch.
- **D130 A missing-PR reminder requires remote truth.** A fully pushed, unmerged local branch is a
  useful candidate for review, but its Git state cannot prove whether GitHub already has a pull
  request. The Code surface asks `git.pr.list` once when the project opens, then labels worktrees and
  branches as either `PR #n` or `pushed · no PR`. If GitHub is unavailable it shows neither claim.
  Branches needing a PR sort to the top of the selector, and the current branch gets the same quiet
  amber state beside its PR action. File-watch refreshes do not repeat the network request.
- **D131 Launch profiles own settings and task sets; review assignments belong to the group.**
  A multi-session Solo launch creates independent worktrees, so provider, role, effort, and assignments
  are stored per agent profile instead of leaking one global choice across all six slots. A profile may
  own any number of tasks. Relay stages the full ordered set before the first provider starts, exposes
  the current task plus its queue through `session.bootstrap.tasks` and the brief, and completes or blocks
  only `sessions.task_id`. A successful completion promotes and prompts the next task. The scalar
  `session.bootstrap.task` remains the current task for compatible clients. A Review
  group keeps one unlimited shared task set because its builders and reviewer deliberately share one
  worktree, while provider and effort remain individually configurable per group member.
- **D132 Codex guardrails use provider-native project hooks without bypassing trust.** Codex now supports
  `PreToolUse` and lifecycle command hooks. Relay merges its generated handlers into `.codex/hooks.json`,
  preserves project handlers, and removes only Relay-owned commands on teardown. Codex requires a one-time
  review of the exact non-managed hook through `/hooks`; Relay surfaces that requirement and never passes
  `--dangerously-bypass-hook-trust`. Shell and patch invocations reach the same `guardrail.gate` bus path as
  Claude — patches as one write gate per file (D163) — while the Git pre-commit gate remains the final
  write-set enforcement boundary.
- **D133 Fresh sessions receive a private start turn after their queue is complete.** Provider-native
  role instructions still replace positional CLI prompts (D82), but starting an interactive CLI without
  a user turn leaves it waiting forever. Relay now stages every selected task with
  `task.dispatch { start: false }`, starts the provider once, then writes a bootstrap/start turn into its PTY. The same
  event-driven input advances a builder after `session.done`; no polling loop or idle process is added.
- **D134 Priority mail rides existing responses; agent suggestions stay outside dispatch context.**
  `mailbox.send` stores a priority bit and Relay recomputes each authenticated session's unread
  priority count after every bus call. The count is optional top-level response metadata, including
  on errors and replays, rather than part of strict per-op results or audited replay data. CLI and MCP
  adapters make the hint visible only when nonzero. Agents can prioritize only direct task-linked mail
  and can leave only one unread priority message per recipient, which keeps ordinary coordination from
  becoming urgent by default. `notes.append { target: "suggestions" }` writes observed friction to one
  unpinned per-project note, automatically stamped with session and current task. Role instructions place
  that append immediately before task completion and only after observed friction. The note is never
  standing context, so a useful review backlog does not become a recurring token tax.
- **D135 Codex delegates approval prompts by default.** Every fresh and resumed Codex session launches
  with `--approve-for-me`, which routes approval requests through Codex's automatic reviewer while
  keeping its workspace-write sandbox. Relay's own guardrail hooks and write roots remain independent;
  this is not full access and does not bypass either provider or Relay safety boundaries.
- **D136 The Agents file tree is a dock, not another terminal tile.** Files remain visible beside every
  wall layout without consuming a session slot. The dock remembers its width and open state per project,
  follows the selected worktree, refreshes from `file.*`, `git.*`, and `worktree.*` events, and hands a
  selected path to the existing Code surface for editing. Closing it leaves a narrow Files rail so the
  feature stays discoverable without taking meaningful terminal width.
- **D137 Plan is a pinned ordinary note.** The Plan page creates or reuses the project's pinned note whose
  title is exactly `Plan`, then autosaves its Markdown body through `notes.update`. This gives the human a
  dedicated endless scratch surface while keeping the content available to agents through the existing
  `notes.list` and `notes.get` ops. Plan and Notes are both real typed `Page` variants, so UI state and
  layout restoration no longer silently reject the frontend's Notes route.
- **D138 Publish actions explain repository state before acting.** Commit, push, and pull request creation
  are shown as one ordered workflow with dirty-file, upstream, ahead/behind, and PR state beside each
  action. Push still delegates missing-upstream behavior to D129, and opening an existing PR goes through
  `os.open_url`. Integration is labeled Merge test because it uses a disposable worktree and never lands
  the selected agent branches into the current checkout.
- **D139 The board carries the GitHub-issues model, and sub-tasks stay cards.** A flat five-column
  kanban with one card per task had nowhere to put a group of work, a classification, or the agent
  working it, so the board was rebuilt rather than patched (task #31). Store v16 adds `tasks.kind`
  (a first-class `task` / `feature` / `bug` / `chore` / `spike`, distinct from the new free-form
  `labels`), `tasks.parent_id`, and one-directional `task_relations` rows for `blocked_by` and
  `duplicate_of`. Four choices are worth recording. **Nesting caps at three levels**
  (`TASK_DEPTH_MAX`) against GitHub's eight: a card that indents four times stops reading as a card,
  and `task.parent.set` measures the height of the subtree being moved, not just the task, so a
  re-parent cannot smuggle a fourth level in. **`blocks` is never stored**: it is the reverse read
  of `blocked_by`, so the two ends cannot disagree. **The neutral type is `task`**, not one of the
  four the redesign asked for, because every pre-v16 row needs a value and calling old work a
  `feature` would be an invention. **A sub-task is a whole task**: its own card, its own column, its
  own dispatch, its own agent. The parent shows an `n/m` roll-up over the subtree, and nothing is
  ever collapsed into an invisible checklist. `task.dispatch { fanout: true }` sends the parent and
  every not-done descendant, one session each, and refuses without `create` (`task.fanout_target`)
  rather than quietly piling several tasks onto one agent.
- **D140 Device detection is event-driven only while device control is visible.** An explicit
  `device.list` snapshot cannot notice a phone connected afterward, while a permanent watcher would
  violate Relay's no-idle-process rule. Opening device control acquires a reference to one shared
  `adb track-devices -l` process through `device.watch`; each ADB change emits `device.changed` and
  the shell refreshes the snapshot. Closing the panel releases its reference, and the last release
  kills the process. This covers USB authorization changes and emulator arrivals without polling or
  work while the surface is idle.
- **D141 User Git holds pause in the Git panel; agent violations remain blocks.** Core already returns
  a typed `held` error with a `confirm` replay only for user bypasses, while agent command and commit-cap
  violations are refused. The Code surface reacts only to that typed user hold, shows the exact policy
  message, and requires explicit confirmation through `guardrail.confirm`. Refused errors remain errors
  and never open a confirmation prompt.
- **D142 The Agents surface exposes the same project skill switches as the library.** Skill enablement
  remains project-wide because session briefs are assembled from the project's enabled set. A compact
  top-bar popover lists every installed skill and calls the existing `skill.enable` operation, so a
  director can change the set without leaving live terminals. The surface states the runtime boundary:
  newly started or resumed agents receive the new set, while an already-running process keeps context
  it has loaded.
- **D143 Board scope can change without leaving the board.** Tasks remain project-owned, so a workspace
  switch cannot honestly merge unrelated project boards or create tasks without an owner. The Board
  header instead exposes an explicit workspace and project scope control. Choosing a workspace opens
  its first pinned or ordered project and keeps the Board surface active; choosing a project switches
  directly. The Board reacts to the project id and discards stale async results from the prior scope.
- **D144 Slow work never runs with the store mutex held.** D3 puts every handler inside one
  transaction with the store lock held, which is right for auditability but makes that lock Relay's
  global one. Handlers that walked a worktree (`resources`), forked `adb`, or probed a provider
  binary therefore blocked every other op, including PTY keystrokes, for as long as the
  filesystem or the child process took. Three rules now hold. Filesystem walks answer from a cache
  inside the transaction and re-measure on a worker afterwards, publishing through the existing
  `resource.sample` event. Every subprocess forked from a handler goes through
  `proc::output_with_timeout`, which kills the child at a deadline and returns a typed refusal
  (`device.adb_timeout`) rather than waiting forever. Anything genuinely long-running keeps using
  `after_commit` plus a worker thread, as the device mirror and run handlers already did.
- **D145 Literal SQL is prepared once per connection.** Relay holds one connection, so rusqlite's
  statement cache has a 100% hit rate, but the default capacity is 16 and Relay has roughly 50
  distinct statements, so the cache thrashed. Handlers now call `prepare_cached` for literal SQL and
  the capacity is raised to 128. Queries whose text is built with `format!` deliberately keep plain
  `prepare`: the cache is keyed on SQL text, so a query with a variable shape would evict the
  statements that benefit. Statements inside a loop are hoisted above it.
- **D146 The terminal renders through WebGL when the webview allows it.** D1 deferred the renderer
  choice to "spike time" and the spike never happened, so every pane on the wall was repainting
  through xterm's DOM renderer on the main thread, the costliest of its three backends when the
  wall shows many at once. Panes now load `@xterm/addon-webgl`, keeping `allowTransparency` so the
  wallpaper still shows through. A webview that refuses a GL context, and a context lost later, both
  fall back to the DOM renderer rather than leaving a dead canvas; the addon is disposed before the
  terminal so the context is released rather than leaked.
- **D147 An installed skill is a folder the whole app owns.** D48 made skills global rows with
  per-project enable edges, but the row only ever held one `SKILL.md` body, and the body reached an
  agent as `.relay/session-skills.md` — so a skill whose instructions point at `reference/*.md` or
  `scripts/*.mjs` only worked in the one repository that had vendored those files by hand, which is
  not what "installed" means. `skill.install` now stages each skill's whole folder out of the clone
  (bounded: 4000 files, 64 MiB, depth 12; symlinks and `.git` skipped) into `<store dir>/skills/<id>/`,
  and `crates/relay-core/src/skills.rs` materializes every enabled skill into every project root and
  every session worktree as `.claude/skills/<name>/` and `.agents/skills/<name>/`. Relay owns only
  the folders carrying a `.relay-skill` marker — a skill the repository checks in under the same name
  wins its folder untouched — and prunes its own folders when a skill is disabled or removed. The
  materialized paths go into `.git/info/exclude`, so app-wide skills never dirty `git status`.
  Enablement keeps its per-project edges as an *override*: a skill nobody has enabled anywhere is
  enabled in every project on create/install, and a new project starts with every installed skill, so
  installing once covers the workspaces that already exist. Engine start does one materialization
  pass over every project and worktree on a worker thread — one shot beside crash recovery, not a
  timer — so an existing checkout never has to wait for its next launch. The copying never runs
  under the store mutex (D144): the plan is read under the lock, the megabytes are written after it
  is released.

  Two things follow from what the providers actually read. Codex discovers skills only under
  `$CODEX_HOME/skills` and knows nothing about a checkout's `.claude/` or `.agents/`, so the
  checkout copies alone would leave every Codex session with no registered skill at all; Relay
  therefore also materializes into `~/.claude/skills/` and `$CODEX_HOME/skills/`, carrying the
  union of skills enabled in any project, since one machine-wide folder cannot hold a per-project
  answer. The same marker rule protects a skill the user wrote there by hand. `RELAY_SKILLS_HOME`
  redirects that scope for tests and scratch runs, and a `test` engine writes nothing into a real
  home. Second, the compact brief now names the enabled skills and the folders they live in
  instead of pointing at `.relay/session-skills.md`: a path to 27 KB of bodies tells an agent
  neither that a skill exists nor that it may load it, and nothing made it open the file before
  the work the skill covers. The bodies stay on disk for a provider with no skill mechanism.

- **D148 A keystroke never touches SQLite.** D3 sends every op through one transaction on the one
  connection behind the one mutex, and `session.input` is one op per character. So typing queued
  behind whatever else held the lock — a `git status` scan, `adb devices` starting its server,
  `gh` waiting on the network — and letters landed seconds late. `session.input` and
  `session.resize` are `Actors::UserOnly` with `Audit::AgentOnly`, which means that by the time
  the pipeline has checked the envelope, the door, the payload and the actor allowlist there is
  nothing left for the transaction to do: no audit row, no undo envelope, no session scope. The
  engine now resolves the target from an in-memory session-name → PTY index and writes the bytes
  with no lock taken at all. A name that is not in that index falls through to the registered
  handler, so `session.not_found` and `session.not_spawned` stay exactly as typed as before.
  The one thing a keystroke does owe the store is the idle→running edge, and the PTY carries a
  mirror of `sessions.state == 'idle'` — set by `session.report` and `session.done`, the only
  writers of that state — so the edge is a single atomic swap. The first keystroke after it hands
  the row update to a worker through `system_write`; every keystroke after that is pure memory.
- **D149 Slow work runs outside the transaction, not merely with a timeout.** D144 bounded the
  damage — cache the walk, kill the child at a deadline — but left `adb`, `gix`, `gh`, `git` and
  recursive walks inside the request's transaction, where they still owned the global lock for
  their whole run. Opening the Code page fired seven such ops and got them serialized end to end.
  The engine now has two more handler shapes beside the ordinary one. `register_unlocked` takes a
  **query** and runs it with no transaction open: it reads what it needs through `Unlocked::read`,
  which takes the lock for one short burst, and does the external work with nothing held. Queries
  are never audited, so there is no row to append and no transaction to fail — that is what makes
  the split safe without touching the write pipeline, and it is why the shape is restricted to
  queries. `register_staged` splits a **mutation** into `prepare` (short reads, then the
  subprocess, network call or tree walk, unlocked) and `finish` (short, inside the request's
  transaction, with whatever `prepare` produced). Events and deferred work queued during
  `prepare` are carried into the request, so they still fire after the commit, in order.
  Converted: `worktree.list`, `worktree.disk`, `git.status`, `git.diff`, `git.diff.file`,
  `git.log`, `git.show`, `git.branches`, `git.pr.list`, `git.suggest_message`, `file.tree`,
  `file.read`, `file.search`, `device.list`, `avd.list`, `avd.catalog`, `provider.list`,
  `usage.get`, `github.status`, `github.repo.list`, and later `audit.list` / `audit.get`, which
  copy the rows out as text under the lock and parse the stored JSON after it (unlocked); `git.commit`, `git.fetch`,
  `git.push`, `worktree.create`, `skill.install` (staged). Because each door dispatches from
  `spawn_blocking`, those queries now genuinely run in parallel rather than merely appearing to.
  (That first held only across connections: the socket door read one connection's next request
  only after answering the last, and the native client sends all of its UI traffic on one. The
  door now runs up to eight unlocked queries per connection at once and answers each by id; any
  other request waits for the queries sent before it on that connection, so a read followed by a
  write is still answered in that order. RA-015.)
  Two constraints bind. A staged op can still be reached from inside another transaction —
  `project.remove {force}` closing its sessions — so `invoke_registered` runs the missing prepare
  against the connection it already holds; that path costs what it cost before the split.
  `guardrail.confirm` is itself staged and runs the held op's prepare before its transaction
  (`Unlocked::prepare_registered`), as the op's original caller; `task.dispatch` creates and
  launches sessions as requests of their own. The audit fixes (RA-018..RA-035) staged
  `avd.create`, `avd.boot`, `git.branch.create`, `git.branch.delete`,
  `git.branch.clean_merged`, `git.stage`, `git.unstage`, `git.pr.open`, `worktree.remove`,
  `integration.discard`, `project.clone`, `task.dispatch` and `guardrail.confirm`; `git.commit`
  now also builds its commit object before the lock and publishes it with a compare-and-swap
  `update-ref` inside it. And every store lock is now accounted for: the pipeline records queue-wait
  and hold time per op and warns past one frame's worth (16 ms), so the next op that parks on the
  lock names itself instead of being felt as "the app froze".
- **D150 A settings read reads its own subtree.** Settings are leaf rows overlaid on defaults, and
  `settings.get` built the entire effective tree before picking a path out of it. With wallpapers
  living in `appearance.wallpaper*` that meant every read — `appearance.mode`, `layout.current.N`,
  a keybinding — decoded megabytes of base64 on its way past, and `settings.set` did it twice,
  once for the undo envelope and once for the event. A page switch persists `layout.current.<id>`,
  so ordinary navigation was paying for the wallpaper. A read now selects only the rows that can
  change the answer: the path, its subtree, and its ancestors — an ancestor matters because a
  scalar stored at `appearance` replaces the default object underneath it, and ordering by path
  keeps the ancestor-then-descendant application order the full-tree build relies on. Reading the
  root still builds the whole tree, because that is what was asked for.
- **D151 The audit log is a window, not an archive.** `req_id` is unique forever (BUS.md §5.3) and
  every mutation lands a row, so on a long-lived store the table becomes most of the database —
  pages the one connection reads past to reach anything else. Two limits now hold. The provider
  hooks call `session.report` on every tool use and `usage.report` on every window refresh; their
  payloads are high-volume and near-identical, so those two ops keep 2 KB of readable payload
  instead of 64 KB. The hash still identifies the request and the result summary still replays it,
  and an over-limit payload now records `{truncated, bytes}` rather than NULL, because "we chose
  not to store this" and "there was nothing to store" are different answers. Second, launch
  recovery prunes rows older than `audit.retention_days` (default 180; `0` keeps everything) and
  compacts the store only when a prune actually removed something. A row that is either half of an
  undo pair is kept whatever its age: that link is the record of what was reversed, and the
  foreign keys between the two rows have to stay intact.

- **D152 A release build is a device run without the device.** "Produce an APK" and "ship it to
  Google Play" are not a second pipeline: `device.build` reuses the run row, the run runtime, the
  bounded log buffer and the `logcat` stream that `device.run` already has, and differs only in
  what it does at each end. It takes no `device`, so it runs with nothing plugged in and its row
  stores `kind='build'` with an empty device; it runs `assemble<Variant>` / `bundle<Variant>`
  instead of `install<Variant>`; and instead of launching the app it records the newest artifact
  under `build/outputs` in the same write that finishes the run, so a finished build is never
  listed without the file it produced. Relay itself is not the thing being shipped — it is a
  Linux AppImage (SPEC §Build & Release) and cannot be a Play app; what it ships is the Android
  project Relay is an ADE for. Three consequences worth naming. Gradle keeps the credentials:
  `publish=true` runs Gradle Play Publisher's `publish<Variant>Apk` / `publish<Variant>Bundle`,
  which the target project must have that plugin applied to have at all, so the keystore and the
  Play service account stay in the target project where they already are, and Relay never holds a
  Play secret. `build_cmd` is not consulted — that field is the integration verifier's build, and
  a release artifact is a specific Gradle task, not whatever verifies a merge. And a build does
  not sync the primary checkout the way a run does: a release ships exactly what is checked out.
  The UI arms the Play upload before it fires, because a published release cannot be recalled.
- **D153 Session-owned launch context gets a session-owned path.** Review groups deliberately share
  one worktree, so the old singleton `.relay/role-instructions.md` and `.relay/session-brief.md`
  let whichever session spawned last overwrite every peer's role and inspectable context. Relay now
  writes both files below `.relay/sessions/<session>/` and passes those exact paths to the provider
  and `RELAY_BRIEF`. The skill body remains `.relay/session-skills.md` because enabled project skills
  are shared by sessions in the same checkout.
- **D154 Role instructions are an executable authorization contract.** Provider guidance and role
  allowlists live in different modules, so a sentence can otherwise tell an agent to call an op the
  engine refuses. An agent-surface test now resolves every registered op named by each role prompt
  through that role's real `bus.ops` verdict, including hidden and unimplemented ops. Reviewers may
  call `session.done`; only builder completion moves a task, so reviewer completion remains a safe
  lifecycle report. Task-attributed suggestions are requested only when bootstrap has a current task.
- **D155 Bootstrap teaches identity and discovery, not only capability names.** A flat `can_call`
  list prevents unauthorized attempts but still leaves an agent guessing payload fields. Bootstrap
  now returns `project_id` beside the project name and points to `bus.ops` for summaries,
  `bus.schema` for exact payload/result shapes, and the equivalent `$RELAY_BIN ops --mine` /
  `$RELAY_BIN schema <op>` commands. The same hint states that Relay fills the bound project and
  session identity, and that `session.done` reports the current task without a task field. Provider
  guidance names the structured `guardrail.explain` shape and the exact mailbox broadcast alias.
- **D156 Release signing stays project-owned and becomes observable.** Relay does not grow a
  keystore wizard because mutation payloads are audited and the product contract forbids storing
  credentials. The selected Android project's Gradle release configuration signs the artifact,
  exactly as a configured Android Studio release build does. Relay persists the requested variant,
  APK/AAB format, and Play-publish intent on the durable run row, then verifies the produced APK
  with the Android SDK's `apksigner` or the AAB with the JDK's `jarsigner`. The result is `signed`,
  `unsigned`, or `unverified`; missing verification tools do not erase a successfully built file.
  An active release build remains visible in the global status bar and on its Agents-wall source,
  while Device Control separates Run and Release so device availability never obscures packaging.
- **D157 Release signing may be Relay-owned without becoming repository-owned.** D156's
  project-only rule made a release depend on credentials embedded in or configured beside the
  Android checkout, which is exactly the state Device Control should let the user avoid. The new
  path is explicit and opt-in: `device.signing.create` generates one RSA PKCS12 upload key below
  Relay's instance data directory, stores its password in Linux Secret Service, and returns only
  the alias and keystore path. Because the request contains the password, this mutation is
  `audit: never`, `user_only`, and `TauriOnly`; agents, the socket/CLI door, run rows, logs, and
  SQLite never receive the secret. `device.signing.get` is the redacted status query. A temporary
  Gradle init script binds that key to the Android `release` build type for one `device.build`
  process through environment variables, so neither the selected checkout nor its Gradle files
  are changed. Projects without a Relay profile keep D156's Gradle-owned behavior. Gradle Play
  Publisher still owns Play authentication; Relay stores no Play credential. A created signing
  identity has no replace/delete control because silently rotating it can make future Android
  updates impossible; backup and recovery are deliberate follow-up work, not a casual button.
- **D158 A saved Relay key is an override, not a lock-in.** Creating the wrong upload key must not
  permanently shadow a project's established Google Play identity. Device Control can explicitly
  disable or re-enable the Relay signing profile without deleting its keystore or password; a
  disabled profile falls back to the project's normal Gradle release signing. The temporary init
  script registers through Gradle's `beforeProject` lifecycle so AGP's `finalizeDsl` callback is in
  place before Android finalizes signing configs, including on AGP 9.
- **D159 Plugins are bundled bundles, switched on per project.** D48 kept `plugin.list` as a real,
  empty query. A plugin is now a folder under `plugins/<id>/` — a `plugin.json` manifest, always-on
  agent instructions, skill folders, documentation and MCP server declarations — compiled into the
  engine by `relay-core/build.rs`, so the content and the code that delivers it ship at one
  revision with nothing to install. Schema v20 stores only the `plugin_projects` edge; an id a
  later build stops bundling is ignored rather than failing a launch. Plugins are off by default
  (unlike installed skills, D147), because they change how every agent in a project works;
  `plugin.list` reports `suggested_for` from one directory listing per project root, read with
  the store lock released. Switching a plugin on reaches agents through the existing channels
  only: its skills join the D147 materializer (an installed skill with the same folder name wins,
  and the stamp carries the bundle digest so a rebuilt engine rewrites stale folders), its
  instructions and skill descriptions join the injected half of the D101 brief under "Enabled
  plugins", and its MCP servers are merged into `.relay/relay.mcp.json` for Claude and passed as
  `--config mcp_servers.<name>.*` overrides to Codex on every start and resume. A manifest command
  of `relay` means this Relay binary; a plugin can never replace the `relay` server. A plugin's
  skills go only into the checkouts of projects that have it on, never into the machine-wide
  `~/.claude/skills`, which would reach every project (RA-123); Codex, which has no per-project
  skill folder, still gets them in `$CODEX_HOME/skills`. Skill folders
  update in running checkouts at once; the brief and MCP servers apply on the next start or
  resume, which is when a provider reads them. The first plugin, Unreal Engine, serves its tools
  from `relay unreal-mcp`: a stdio MCP server independent of the bus that reads the project on
  disk, runs UnrealBuildTool through `proc::output_with_timeout`, and reaches a running editor
  through the Remote Control HTTP API and `PythonScriptLibrary.ExecutePythonCommandEx`.
- **D160 A plugin can choose the checkout, and the Unreal bridge refuses the wrong one.** An
  Unreal editor has exactly one project open, normally the main checkout, while Relay's default
  gives each agent a new worktree: an agent's C++ landed in its worktree and its editor edits in
  the main checkout. A plugin manifest may now set `default_checkout: "primary"`; `session.create`
  without an explicit `worktree` then uses the primary checkout for projects with that plugin on
  (read once in the prepare phase so both phases agree). An explicit `worktree` still wins. The
  `unreal-mcp` live tools independently ask the editor for its open `.uproject` and refuse on a
  mismatch, and serialize editor-changing tools through a lease file in the project's `Saved/`
  (session name as holder, 15 idle minutes to expire), since agents sharing one checkout also
  share one editor. Animation checks are measurement first: `ue_anim_inspect` poses skeletons
  from animation data in editor Python and reports sides, grips, clearances and contacts in a
  frame derived from the skeleton's own left/right bone pairs, so it holds for any skeleton,
  item or mesh orientation; `ue_anim_preview` adds images, confirming each pose was applied
  before capture because the editor applies poses on its tick.
- **D161 The Blender plugin runs Blender headless, one process per call.** Game art tools usually
  bridge to a running Blender through an add-on and a socket; that needs a window, an installed
  add-on and one shared session. `relay blender-mcp` instead starts `blender -b <file>
  --factory-startup` per call with a script from `relay-cli/src/blender_py/`, arguments in a JSON
  file and one `RELAY_JSON:` result line, under `proc::output_with_timeout`. Calls are
  independent, work with no Blender open, never load the user's add-ons (bundled ones can be
  enabled per call), and cannot leave state behind. Files are resolved inside the agent's
  checkout. Renders return as MCP image content, as `ue_screenshot` does, so the agent can see
  its work; the rig check and `blender_anim_inspect` use the same character frame (from the rig's
  own `.L`/`.R` pairs) as the Unreal checks, so a problem is caught before export and
  measured again after import. `blender_to_unreal` calls the Unreal bridge in-process, sharing its
  project guard and editor lock, and compares height, root bone scale and hand sides across the
  handoff. The Blender tests run against real Blender when it is installed and skip otherwise.
- **D162 The Unreal bridge owns the editor process, and imports verify that they draw.** A real
  game session on UE 5.8/Linux showed the failures were at the edges of the tools, not in them:
  builds without `UE_ROOT`, a leftover crash reporter turning a build into an unloaded hot-reload
  module, a Remote Control port held after quitting, an editor throttled in the background, a
  scene-capture show-only list that crashed the editor, Interchange importing empty meshes and
  transient materials, imported tangents that made a mesh invisible, and EditorAssetLibrary path
  functions failing for a whole session after a failed import. The bridge now finds the engine
  from a running editor, the project log, a source tree, `Install.ini` (GUID or version) or common
  folders; stops crash reporters and refuses to build under a running editor, reading
  `UnrealEditor.modules` afterwards; launches and quits the editor itself, waiting for the port
  and reading the log for a failed bind (`ue_build restart_editor` is the Linux C++ loop); writes
  `DefaultRemoteControl.ini` keys checked against the engine header; turns the background
  throttle off during play and reports frames per second; pairs each play screenshot with its
  own file after the probe; isolates by hiding neighbours and previews away from the level;
  imports with the legacy FBX importer, normals only and replaced settings, deletes and retries a
  broken result, and renders every imported mesh once to measure its coverage. Registry lookups
  replace path-function checks. On the Blender side, export refuses meshes the check finds broken
  on the evaluated mesh and rigs not at scale 1.
- **D163 Write rules meet every edit a hook can read, and a slow gate blocks.** The audit found
  write guardrails bound only Claude's Write/Edit tools: a Bash command, and every Codex edit
  (sent as `kind: exec`), skipped protected paths, destructive writes, shape gates and write
  roots. The PreToolUse adapter now reads what a call will write before it runs. Codex's
  `apply_patch` is structured, so each file becomes a `write` gate with its new text (hunks
  applied in memory the way apply_patch matches them; a diff of counts when they do not apply).
  A shell command line is tokenised — quotes, here-documents, `$(…)`, `sh -c` — and the obvious
  writers yield targets: an overwrite or delete sends a diff of the lines it removes, an append
  or in-place edit an empty diff that still meets the path rules. This is best effort by
  construction: variables, globs and programs that open files themselves are not seen, which
  the docs say plainly. A post-hoc watcher was rejected for now: `file.restore_head` cannot bring
  back the untracked or modified content that destructive writes guard (D114) without a snapshot
  taken first, and diffing a tree inside a gate would hold the store lock. Separately, the
  adapter gives up after 20 s and exits 2: a provider kills a hook at 30 s and then runs the tool
  unchecked, so a store-lock stall used to wave every tool through (D23).
- **D164 `ui.*` is a shell model core holds, not a door a client executes.** BUS.md §6.5 described
  Relay-2's plan: `ui.*` forwarded to the main window over a Tauri executor door, `ui.absent` when
  headless. V4 has no Tauri app and never had that door; core answered `ui.*` from its in-memory
  model (D40), and the GTK client follows only the page, a focused pane's `target.session` and
  `ui.toast`. The leftovers that claimed otherwise are removed rather than made true: the
  `executor` registry attribute (and `.ui()`), which nothing read, `Doors::TauriOnly`, the engine's
  `Door::Tauri`, and `app.status.ui_connected`, which was never set and which no client read. This
  supersedes D40's `executor: ui` boundary; `session.allow_ui` (an op-name prefix check) is the
  agent gate, as it always was. The `ui.*` ops keep their behaviour and shapes: a pane or pop-out
  that opens nothing on screen still answers success, because it did update the model, and the op
  summaries now say which ops reach the screen, since a summary is all an agent reading `bus.ops`
  or an MCP tool list sees.
- **D165 Only a process outside the engine's tree may claim `user` on the socket.** `user` and
  `test` carry no token, so an agent refused by a guardrail could answer its own hold by writing
  `{"actor":"user","op":"guardrail.confirm",…}` to the socket; the command-line check (§9.5)
  sees only command lines. The socket now identifies the process behind each connection once, on
  accept: `SO_PEERCRED` for its pid (and `SO_PEERPIDFD`, where the kernel has it, to prove the
  pid was not recycled during the walk), then `/proc/<pid>/stat` parent links to the top. A peer
  below a live session's PTY child (the engine's in-memory PTY registry, not `sessions.pid`,
  which outlives restarts), below the engine in any other way, or not identifiable at all is
  refused `user`/`test` with `refused` / `actor.peer`; `bus.ping` claims nothing and is let
  through so liveness probes work from anywhere. The rule is the person's decision: every PTY
  Relay spawns is an agent session, so a `!relay …` typed into an agent's pane is refused too.
  The engine calls `prctl(PR_SET_CHILD_SUBREAPER)` before anything else in `serve::start`, so a
  descendant that double-forks or calls `setsid` lands under the engine instead of init. A
  subreaper inherits zombies nobody waits for, so a worker reaps them every 30 s — only those
  in a different session than the engine's (session trees and `setsid` daemons) and only on a
  second look, because the engine's own children share its session and have owners that wait
  for them. Checked flows that act as the user from below the engine and would break: none. The
  app's `git.commit` commits with `--no-verify` and runs the user's own hooks itself, so the
  installed pre-commit hook's `--actor user` branch runs only for a commit from a terminal
  outside Relay (allowed) or one that shed `RELAY_SESSION` inside a session (refused, as
  intended); integration merges run in a worktree without Relay's hooks; provider updates,
  builds and device runs never call the bus; the phone bridge connects from the engine's own
  process, or from `relay remote serve` started outside it; the desktop app is not an engine
  host. `relay events` now subscribes as whoever runs it, as `relay attach` already did. Known
  cost: a program the engine launches for the person (`xdg-open` starting a file manager that
  then opens a terminal) is below the engine, so `relay` typed there is refused as the user.
  This is a seatbelt against an agent's own tree, not a boundary against a same-uid process that
  gets itself started elsewhere (`systemd-run --user`, cron, editing the store).
- **D166 Agents are not handed notes; a task remembers the note it came from.** Notes are the
  thinking before a task, and the standing note was injected into every brief (D27, D101), so an
  agent started on one card read the project's whole scratchpad first and took it as
  instruction. The brief now carries state, peers, adjacent work, comms, skills and plugins, and
  no notes: an agent works from its task and reads a note (`notes.get`, `notes.standing`) only
  when it asks. What a note decided reaches the agent through the task made from it, which
  records that note as `note_id` (`task.create {note_id}`, store v26; `task.list {note_id}`
  lists them). The link is refused across projects and to a deleted note, and purging the note
  clears it rather than the task.
- **D167 Videos are code that Remotion renders, through a worker the engine keeps.** Agents make
  videos (app promos, explainers, social clips) by writing React that Remotion draws frame by
  frame, because they already write React fluently and a browser draws anything a video needs
  (CSS, SVG, canvas, WebGL, Lottie, charts); a timeline or node format of Relay's own would have
  been a language agents learn only from a skill, capped by what its painter draws. A video is a
  folder of code beside the store (`videos/p<project>/<id>-<slug>/`) that agents edit with their own
  file tools — the Video plugin makes the project's folder a write root and an `--add-dir` — plus a
  `videos` row. Headless Chrome is a render tool here, as ffmpeg is, never a webview: the UI stays
  native (PRODUCT.md). One Node.js process per instance (`videos/.relay/worker.mjs`, JSON lines on
  stdio) keeps each video's bundle, a TypeScript service and one browser between requests, so a
  still after an edit takes about 0.2 s rather than a cold start's seconds; it is spawned like
  `llama-server` (its own process group, a parent-death signal, a waiting thread), every caller
  waits on its own answer with a deadline and no lock held, and it stops after ten idle minutes.
  It is not a one-shot child, so it does not go through `proc::output_with_timeout`; setting the
  kit up (npm, the browser download) does. The kit — Remotion, React, TypeScript and four OFL font
  families at pinned versions, about 600 MB with the browser — is set up once on request
  (`video.kit.install`), never at startup, and Relay passes Remotion no licence key, so it sends
  no usage reports. What the agent sees is checked: Relay's `src/index.ts` registers the root
  through `relayRoot`, which wraps Remotion's frame seek to measure each drawn frame (boxes of
  `data-id` elements, every run of words with its font, colour, opacity and clip) and reads the
  compositions' zod schemas; the engine turns that and the pixels into issues (clipped, offscreen,
  unsafe for a platform's buttons, contrast, small text, overlap, a font that fell back). Because
  agents' edits never pass through Relay, each check, still and render records the folder's code
  as a version when it changed (content-addressed blobs), which is what restore and undo work
  from.
