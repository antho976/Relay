# RELAY v4 — THE COMMAND BUS

Schema v1. This document is the app's API. It is written before the code and every later
phase (SPEC §17) is built against it. Changing anything here after agent skills exist against
it costs more than getting it right now — so this is the one thing we argue about on paper.

Companion to `SPEC.md` §2. Where the two disagree, this document is more specific and wins;
where this document is silent, SPEC wins.

---

## 0. Principles (the rules everything below obeys)

1. **One door shape, three doors.** Every mutation and every read is a bus op. The UI is
   client #1 and has no privileges the CLI or an agent lacks. If a button does something the
   bus can't, the button is wrong.
2. **Control plane vs data plane.** The bus is the control plane: typed, validated, audited.
   Byte streams (PTY output, mirror video, logcat, log tail) are the data plane: attached *by*
   a bus op, delivered on a stream, never audited per byte. That is the only exception and it
   is spelled out in §7. There is no third category.
3. **Typed refusal, never silence.** Every rejected op returns a machine-readable error with a
   `kind`, a stable `code`, and — for holds — the exact op that would confirm it. Agents react
   to codes, not prose.
4. **Audit is the source of truth for "what happened."** Every accepted mutation, hold and
   refusal is one row. Undo reads the audit log; so does blame; so does the debugger.
5. **Schema is generated, not described.** The Rust types are the schema; `schemars` renders
   `schema/bus.v1.json`; CI fails if the committed file differs from the generated one. This
   document lists ops and shapes for humans; the JSON schema is what tools consume.
6. **Seatbelts, not sandboxes.** Guardrails and allowlists stop *mistakes* by agents that
   share your uid. They are not a security boundary against a hostile process on your own
   machine — nothing in-process can be. See §9.

---

## 1. Envelope

### 1.1 Request

```ts
interface Request {
  v: 1;                       // envelope schema major. Bumped only for breaking changes.
  id: string;                 // UUID v4, minted by the caller. Idempotency key (§5.3).
  actor: Actor;               // who is asking (§4)
  op: string;                 // "noun.verb" or "noun.sub.verb" (§3)
  payload: object;            // op-specific; {} when the op takes nothing
  token?: string;             // actor binding for agent actors on the socket door (§4.2)
}
type Actor = "user" | `agent:${string}` | "test" | "system";
// "system" is internal: reconcile passes, watcher-originated events, async completions and
// crash recovery act as it. Every door rejects a request that claims it (`bus.actor`).
```

Project scope is **not** an envelope field: ops that act within a project carry
`project_id` in their payload. This keeps the envelope identical for every op and makes
"which project" a first-class, validated argument rather than ambient context. The CLI
fills it in from `RELAY_PROJECT` or the cwd's repo root (§8.2).

### 1.2 Response

```ts
type Response =
  | { v: 1; id: string;        ok: true;  result: object; replayed?: true; mail?: MailHint }
  | { v: 1; id: string | null; ok: false; error: BusError; replayed?: true; mail?: MailHint };
interface MailHint { priority: number }
// id is null only when the request could not be parsed far enough to read one
// (kind "invalid", code "bus.parse"). A replayed `held` says replayed too.

interface BusError {
  kind: "invalid" | "not_found" | "conflict" | "refused" | "held" | "unavailable" | "internal";
  code: string;               // stable, dotted, e.g. "task.not_found", "guardrail.destructive_write"
  message: string;            // human sentence, may change; never parse it
  details?: object;           // structured, per-code, documented with the code
  hint?: string;              // what to do about it, for humans and agents
  confirm?: { op: string; payload: object };   // present iff kind === "held": the op that lifts the
                              // hold. It is a USER op (§9.3): an agent that receives a hold waits
                              // for the `guardrail.resolved` event / mailbox message, it does not
                              // call confirm itself.
}
```

Kinds — exhaustive, and each maps to a CLI exit code (§8.2):

| kind | meaning | typical codes |
|---|---|---|
| `invalid` | the request itself is malformed: bad envelope, unknown op, payload fails schema, bad actor/token | `bus.parse`, `bus.envelope`, `bus.unknown_op`, `bus.schema`, `bus.actor` |
| `not_found` | a referenced entity does not exist (or is soft-deleted and the op doesn't accept that) | `task.not_found`, `session.not_found` |
| `conflict` | the request is well-formed but the current state forbids it | `session.already_spawned`, `task.column_transition` |
| `refused` | a policy said no and there is no confirm path | `guardrail.protected_path`, `actor.allowlist`, `guardrail.cap` |
| `held` | a policy paused it; `confirm` names the op that releases it | `guardrail.destructive_write`, `guardrail.shape_gate`, `guardrail.user_bypass` |
| `unavailable` | a dependency is missing or down: provider CLI absent, no device, git binary missing | `provider.not_installed`, `device.none` |
| `internal` | our bug. Logged with a trace id in `details.trace` | `internal` |

`replayed: true` on a success means the `id` was seen before and the recorded result was
returned without re-executing (§5.3).

`mail` is live, unaudited response metadata for an authenticated agent session. It is omitted
when there is no unread priority mail and is recomputed after every op, including errors,
acknowledgements, and idempotent replays. It does not change an op's typed `result`.

### 1.3 Events

Events are facts about mutations that already happened. They are emitted after commit,
never before, and carry enough for a client to update without a re-query in the common case.

```ts
interface Event {
  v: 1;
  ev: string;                 // "task.changed", "session.state", ... (§3.3)
  ts: string;                 // RFC 3339, UTC
  actor: Actor;               // who caused it
  cause?: string;             // request id that caused it (absent for system-originated, e.g. reconcile)
  project_id?: number;
  payload: object;
}
```

Events are not audited (their cause is). Events are best-effort delivery: a client that was
not subscribed at the time never sees them and re-queries instead — every event has a query op
that yields the same truth.

**Event budget (SPEC §15).** Events are for *transitions*, not for activity:
- `session.changed` fires on state/field transitions only. It never fires because output
  arrived — `last_output_at` and the busy dot are derived by the pane from the `pty` stream.
- Watcher-originated `file.changed` is coalesced to ≥ 100 ms per worktree and carries the
  list of paths, not one event per path.
- `resource.sample` is emitted only while a client has said `app.resources.watch {on: true}`
  (the panel is open) or from the 60 s reconcile pass — never from a standing timer.
- Eleven streaming panes therefore produce eleven `pty` streams and zero bus events per second.

---

## 2. Identity

| thing | id | why |
|---|---|---|
| request | UUID v4 string | minted client-side so retries carry the same key |
| workspace, project, task, module, note, message, hold, notification, audit row, integration | `i64` autoincrement, per table, one store | one `store.db`, one writer; short ids are what agents copy and humans say aloud (`#42`) |
| session | `i64` id **and** `name` (adjective-animal, unique among sessions that are not `closed`) | the name is the handle: env var, peer table, mailbox address. Ops accept `session: string` = name. Ids appear in results for stability |
| worktree | path (absolute) | git's identity for it; the store maps session → path |
| provider | `"claude" \| "codex"` | closed enum in v4 |
| commit | full 40-hex sha | never abbreviated in payloads; abbreviate for display only |

Soft-deleted rows keep their id; `*.restore` brings the same id back. Ids are never reused.

---

## 3. Op naming and classification

### 3.1 Names

`noun.verb` or `noun.sub.verb`; lower snake; the noun is singular. Verbs come from a fixed
vocabulary and always mean the same thing:

| verb | kind | meaning |
|---|---|---|
| `get` | query | one entity by id |
| `list` | query | many, filtered, sorted, never paginated below 1000 rows (we are one user) |
| `create` | mutation | new row, returns it |
| `update` | mutation | patch fields on one row (only fields present are changed) |
| `delete` | mutation | soft-delete (§5.4) unless the op says otherwise |
| `restore` | mutation | undo a soft-delete |
| `move` | mutation | change position/parent (task column, pane slot) |
| domain verbs | as listed | `spawn`, `park`, `dispatch`, `confirm`, `flag`, `ack`, `append`, `pin`, … |

### 3.2 Registry attributes

Every op is registered with, and the schema publishes:

- `kind`: `mutation` | `query`
- `audit`: `always` | `agent_only` | `never` — mutations default `always`; high-frequency
  data-plane control ops (`session.input`, `session.resize`, `ui.*`) are `agent_only`;
  queries are `never`
- `undo`: `none` | `inverse` — `inverse` means the handler records an inverse envelope in
  the audit row and `audit.undo` can replay it (§5.5)
- `scope`: `global` | `project` | `session` — which id the payload must carry
- `actors`: default allow-set before per-session allowlists apply (§9.1)
- `doors`: `all` | `socket_only` — a socket-only op (`bus.subscribe`, `bus.unsubscribe`,
  `bus.wait`, `design.render`, `design.render.serve`) acts on the connection that sends it, so the socket door answers it; dispatched
  to the engine directly it is `invalid` / `bus.door`
- `stream`: the data-plane stream the op attaches, if any (§7)
- `emits`: event names it can produce

### 3.3 Event names

`noun.changed` (entity upserted; payload is the full entity, a re-query hint, or, from a batch op such as
`project.reorder` / `workspace.reorder`, `{projects: Project[]}` / `{workspaces: Workspace[]}`
with every entity it changed), `noun.deleted` (payload `{id}`),
plus domain events (`session.state`, `session.output` on the data plane, `guardrail.held`,
`notify.new`, `integration.result`, `provider.version`, `resource.sample`, `ui.changed` and
`ui.toast` from the shell model of §6.5). `design.changed` carries the `Design` row, never its
nodes; `design.deleted` is `{id, project_id}` (§10.18). Rule: **every
mutation emits at least one event**, and every event's payload is reproducible by a query op.
`design.asset.add` and `design.picture.add` are the exception: they store a content-addressed file and
change no row any query shows, so they emit no event (a documented exemption). From `design.apply`,
`design.changed` also carries `prev_rev` (the rev before the list), `touched` (the ids it added, changed
or moved), `removed` (the ids it removed, at most 100 of each), `all: true` when either list was cut,
and `styles: true` when the list changed the design's named colours or styles (§10.18).

A full-entity `noun.changed` carries the row with its own `id`. A hint carries the scope it
touched instead — `task_id`, `module_id` or `project_id`, plus what changed (`state`, `col`,
`task_ids`, `bulk: true`) — and is not a row: a client that needs the entity re-queries it.
Session lifecycle transitions (`task.changed {task_id, state}`), module delete/restore
(`task.changed {module_id, task_ids}`) and the v3 import (`{project_id, bulk: true}`) send
hints. `bus.wait`'s `matching` is a conjunction, so a wait on `{id: N}` misses the hints and
one on `{task_id: N}` misses the full rows: wait on the event name and re-read.

On one connection, a request's `design.changed` can arrive before or after its reply, or interleave
with an earlier request's event. A client tells its own changes by the event's `cause` (§1.3), the id
of the request that made it, or by rev: `prev_rev` and `touched` say what a list did, and
`design.get` with `since_rev: prev_rev` reads the rest.

---

## 4. Actors and binding

### 4.1 Actors

- `user` — you, at the UI or the CLI. Full default rights; guardrails still apply to your
  writes with a `held` + `guardrail.user_bypass` confirm instead of a hard refusal (SPEC §8).
  On the socket only a process outside Relay's own process tree may claim it (§4.2).
- `agent:<name>` — a running session's CLI process (Claude Code / Codex) or anything it
  spawns. Rights = the session's allowlist (§9.1). Its ops appear in the peer table and audit
  under its name.
- `test` — the integration test harness. Behaves like `user` but is refused outside dev/test
  instances (`bus.actor` on stable).

### 4.2 Binding

- **Socket door** (the only door the engine has; the native client, the CLI, MCP and the phone
  door are all its clients): `agent:<name>` requires `token` equal to the session's token, minted at
  `session.spawn` and exported to the child as `RELAY_SESSION=<name>` and
  `RELAY_TOKEN=<token>`. Wrong or missing token → `invalid` / `bus.actor`. `user` needs no
  token (same uid, same trust — §0.6), but it must come from **outside the engine's process
  tree** (D165). On accept the engine reads the peer's pid (`SO_PEERCRED`, and `SO_PEERPIDFD`
  where the kernel has it, so a recycled pid is not mistaken for the peer) and follows its
  `/proc/<pid>/stat` parent links to the top. A peer below a live session's PTY child, or below
  the engine anywhere else (a session's orphan, a build, a hook), or one that cannot be
  identified (pid 0 from another pid namespace, an unreadable `/proc`, a process already gone)
  may not claim `user` or `test`: every such request except `bus.ping` is `refused` /
  `actor.peer`, with `details.peer` (`session` + `details.session`, `engine_child`, `unknown`).
  That includes the person's own `!relay …` typed into an agent's pane — every PTY Relay
  spawns is an agent session; user actions come from the app, the phone, or a terminal outside
  Relay's sessions. The peer is identified once per connection, so `session.input` and every
  other request pays one comparison. The engine's own process passes (the phone bridge of
  `relay serve --remote` connects from it), as does `relay remote serve` started outside Relay.
  The engine is a child subreaper (`PR_SET_CHILD_SUBREAPER`), so a session descendant that
  double-forks or calls `setsid` is reparented to the engine rather than to init and stays
  below it; the engine reaps those orphans itself. `agent:<name>` requests are unaffected.
  `test` needs `RELAY_INSTANCE` ∈ {dev, test}.
  "Same uid" is checked, not assumed: the engine refuses a runtime directory that is a symlink,
  not its own or open to group/other, and a lock file that is not a regular file of its own;
  clients refuse a socket whose `SO_PEERCRED` uid is not theirs. Without `XDG_RUNTIME_DIR` the
  runtime directory is `/run/user/<uid>` when that is private, else under `~/.cache` — never a
  predictable name in `/tmp`.
- **Phone door** (`crates/relay-remote`, `docs/MOBILE.md`): a paired phone's lines reach the
  socket door over a WebSocket, on the LAN or through a rendezvous server, after a
  per-connection proof of the device's token. The door forwards only `user` (and `test` on a
  dev/test instance); any other actor is `invalid` / `bus.actor` before the socket sees it. So
  "`user` needs no token" means, in practice, *same uid* on the socket and *holds a paired
  device's token* over the network — and, until that channel is sealed end to end, anyone on
  the path of a paired connection, who can inject lines into it (MOBILE.md §6). A `user_only`
  op is reachable from the network; it is a boundary against agents, not against the LAN.
- **In process** (the test harness): trusted; `agent:<name>` is bound by name without a token.
- The token is per session-lifetime; parking and waking keep it; `session.close` revokes it.

---

## 5. Pipeline: what happens to a request

```
receive → parse envelope → idempotency check → resolve actor → validate payload (schema)
        → authorize (allowlist, own-task) → policy (guardrails) → HANDLER → audit → events → respond
```

Validation precedes authorization because the own-task/own-session checks read payload
fields; a payload that fails schema never reaches policy.

### 5.1 Ordering guarantees

- Steps up to and including the handler run **serially per store**. The store is one SQLite
  connection behind one `std::sync::Mutex`, and an ordinary handler runs inside a transaction
  with it held, so one request at a time touches the store — reads included: a query waits for
  the lock exactly as a mutation does, and WAL gives no concurrent readers here. Latency budget:
  p99 < 5 ms for store-only ops; a request that holds or waits for the lock 16 ms or more is
  logged at `warn` (`store lock over budget`). Slow work therefore belongs in one of the other
  two handler shapes below, or behind a handle that finishes via events (`integration.request`
  → `integration.result`). A plain handler that runs a subprocess blocks every other request,
  keystrokes included, for as long as it runs.
- Audit append is in the **same SQLite transaction** as the mutation. If the audit row can't
  be written, the mutation didn't happen.
- Events are emitted after the transaction commits, before the store lock is released, so
  two commits reach subscribers in commit order. After-commit work runs once the lock is gone.
- **Three handler shapes, one pipeline** (D144, D149). Ordinary handlers run inside the
  transaction as above. A **query** may instead be registered *unlocked*: it takes the store for
  short reads only and does its external work — a subprocess, a network call, a tree walk — with
  nothing held. Queries are never audited, so nothing about the table in §5.1a changes. A
  **mutation** may be registered *staged*: a read/external phase runs before the transaction opens
  and hands its result to a short phase inside it. Everything above still holds for the staged
  half — same transaction, same audit row, same event ordering. Nothing bypasses this pipeline;
  the shapes differ only in how much of a request is inside the transaction.
- **`session.input` and `session.resize` never open one** (D148). Both are `actors: user_only`
  with `audit: agent_only`, so for the actor that can call them there is no row to write, no undo
  envelope and no session scope — every check above still runs, and then the bytes go to the PTY
  from an in-memory index. A session with no live PTY falls through to the ordinary handler so its
  typed refusal is unchanged.
- **Async completion is audited too.** A handler that returned a handle (`integration.request`,
  `project.clone`, `device.run`, `device.build`, `session.spawn`) causes a second row when it finishes:
  `actor: system`, `op: "<op>.completed"`, a fresh `req_id`, `parent_req` = the original
  request id, `kind: ok | error`, `code`, `result_summary`. "What happened to my request" is
  always `audit.list {parent_req}` (a user query, §10.3).

### 5.1a What is audited

| outcome | mutation | query |
|---|---|---|
| `ok` | row (`kind: ok`) | never |
| `held` / `refused` | row (`kind: held` / `refused`) | never |
| `conflict` / `not_found` / `unavailable` / `internal` | row (`kind: error`, `code`) | never |
| `invalid` (unparseable, unknown op, bad schema, bad actor) | **never** — unattributable and spammable; goes to the log at `warn` | never |

`audit: agent_only` mutations follow the table only when the actor is `agent:*`; for
`user`/`test` they are not audited at all.

### 5.2 Validation

Payloads are validated against the generated schema **before** policy runs, so a guardrail
never sees a malformed request and an agent never receives a policy answer for a request that
would have failed on shape anyway. Unknown fields are an error (`bus.schema`), not ignored —
silently dropped fields are how v3 shipped no-ops. A `bus.schema` error carries no `details`:
its `message` is serde's, which names the field for an unknown, missing or mistyped top-level
field ("unknown field `x`, expected one of …") but gives no path into a nested value.

### 5.3 Idempotency

`id` is unique per audited request, forever (`audit.req_id UNIQUE`). A duplicate `id` returns
the recorded response with `replayed: true` and executes nothing — including a recorded `held`,
which keeps replaying `held` even after the hold is resolved: the confirm runs under its own id
(§9.4). A duplicate that arrives while the first copy is still running waits for it and then
replays its row. Duplicate `id` with a *different* payload hash is `conflict` /
`bus.id_reused`. Requests that are not audited (queries, `invalid`) are not deduplicated — they
are safe to repeat by nature. A script that retries `relay cmd` must send the full-envelope form
with its own `id` (`relay cmd '{"id":"…","op":"…","payload":{…}}'`); `relay cmd <op> <payload>` mints a
fresh id per call, so each call is a new request.

A replay whose stored result was too large to keep is not sent back. It answers `conflict` /
`bus.replay_unavailable` with `details.applied: true`: the first copy was applied. Read the current
state (`design.get`, say) and do not send the request again.

"Forever" is bounded by retention (D151): launch recovery prunes audit rows older than
`settings: audit.retention_days` (default 180, `0` keeps everything), so a request id from
before that window is no longer deduplicated. Rows that are either half of an undo pair are
never pruned. `session.report` and `usage.report` keep 2 KB of readable payload rather than the
64 KB of §11.4 — their hash and result summary are unchanged, so idempotency is not affected.
An allowed `guardrail.gate` and an accepted `session.report` — an audit row per agent tool call —
keep only a summary (`session`, `kind`, the command up to 200 characters or the tool name, and
`slim: true`); refusals and holds keep the whole payload (migration v27 slimmed older rows the
same way). The hourly retention pass removes allowed `guardrail.gate`, `session.report` and
`usage.report` rows older than `settings: audit.hook_retention_days` (default 30, `0` leaves them
to the general window).

### 5.4 Soft delete

`task`, `module`, `note`, `file` (files go to the primary checkout's `.relay/trash/<id>/`, not
the OS trash, so removing a session's worktree does not take them along) and
`session` (closed) are soft-deleted: `deleted_at` set, excluded from `list` unless
`include_deleted: true`, restorable by `*.restore` for the grace window (`settings:
undo.grace_days`, default 7; `0` keeps them), then hard-deleted by the reconcile pass. Hard
delete is never an op an agent can call. A closed session's row stays, because audit, mailbox
and task history name it; only its scrollback is dropped. The same pass keeps the notification
table to read rows under 30 days, any row under 180 and at most 5,000, and the mailbox to
messages under 30 days unless one is still unacked (180 at most), and drops guardrail holds
answered more than 30 days ago (a confirmed one only once its session has closed). The engine runs it hourly on
its own (not through the bus, so it lands no audit row); `app.reconcile` runs it now.

### 5.5 Undo

Ops marked `undo: inverse` write `audit.undo_op = {op, payload, expect: {updated_at}}` in
their audit row — the inverse envelope plus the entity's `updated_at` *after* the op.
`audit.undo {audit_id}` executes that inverse as the calling actor; if the entity's
`updated_at` no longer matches `expect` (someone edited it since) it is `conflict` /
`audit.stale` unless `force: true` — an undo never silently clobbers a later edit.
`session.update` is the exception: a running session's reports move its `updated_at`, so it
records the fields it wrote (`expect.fields`) and the undo is stale only if one of them changed
again (RA-403); an older row carrying `updated_at` is still checked against it. The undo's
own audit row carries `undo_of: <audit_id>` and its own `undo_op` (the original envelope), so
undoing an undo is a redo, and the original row gets `undone_by`. A batch op
(`project.reorder`, `workspace.reorder`) records one inverse for all its rows; it stamps them
all with one `updated_at`, and the undo is stale once any of them no longer carries it. Board ops (`task.move`,
`task.update`, `task.delete`, `module.*`) are `inverse`; the UI's Ctrl+Z is `audit.undo` on the
last un-undone board row by `user`. Nothing that touches a PTY, git or the filesystem is
undoable through the bus — those have their own restore paths (trash, reflog).

---

## 6. Doors

### 6.1 Native client (UI)

`apps/relay-native` (GTK) is a client of the socket door (§6.2) like any other: it sends `user`
requests, takes events through `bus.subscribe`, and attaches `pty`, `logcat` and `mirror`
streams on its own connections. It has no private door and no command the CLI lacks. Relay-2's
Tauri `invoke` door (`bus:response`, `bus:event`, Tauri `Channel`s) does not exist in V4; there
is no Tauri shell or webview.

### 6.2 Unix socket

`$XDG_RUNTIME_DIR/relay-v4/<instance>.sock` (without that variable, the fallbacks in §4.2),
`instance ∈ {stable, dev, test}`; the directory is 0700 and the socket 0600. Relay-2 and V3 used
`$XDG_RUNTIME_DIR/relay/`; a client pointed there reaches a different engine and store.
Newline-delimited JSON: each line is one `Request`, each response line is one `Response`;
correlation by `id`; pipelining allowed. `bus.subscribe {events?: string[]}` turns the
connection into a subscriber: `Event` lines are interleaved with responses (distinguished by
the `ev` key). Data-plane frames on this door are `{v, stream, session | run_id | mirror_id,
epoch?, seq, data}` lines (§7).

The ops the door answers itself (`bus.subscribe`, `bus.unsubscribe`, `bus.wait`) pass the same
envelope, actor/token, schema and allowlist checks as any request. An agent's subscription and
waits carry only its own project's events and project-less ones (`device.*` from any project,
since devices are shared), and of `mailbox.new` only mail to it, from it, or broadcast. A
subscriber too slow to keep up is sent `{ev: "bus.lagged", payload: {dropped}}` in place of what
it missed, and should refetch. A `bus.wait` ends when its client disconnects. A request line is
at most 64 MiB; a longer one is discarded and answered `bus.too_large` (with no `id`). Once
`app.quit` (or SIGINT/SIGTERM) is under way, requests are answered `app.quitting`, and the door
closes, ending its connections, before the engine kills its children.

**The render door and its lease.** `design.render` and `design.render.serve` are socket-only: the
socket door answers them itself, after the same envelope, actor and allowlist checks as any request
(`bus.door` from anywhere else). The desktop app offers to draw with `design.render.serve
{fonts?}` on a connection of its own that carries nothing else; the engine keeps a weak sender to it
(the *lease*) until that connection closes, newest lease first, and a second `serve` on one lease
replaces it (the font families it lists, at most 2,000, are what `design.fonts` answers). An
agent's `design.render` is checked against the store first (the design is its own project's, `base_rev`
is current, the page and nodes exist), so a refusal sends nothing. Then the engine writes one
`{ev: "design.render.request"}` line, with a fresh `render_id` and a `deadline_unix_ms`, to the
newest lease only (never a broadcast, and not an event any subscriber sees), and the door waits with no lock
held. The app draws and answers with `design.render.answer {render_id, drawn | error}` on the same
connection; the engine checks the picture against what was asked (a PNG of at most 4 MiB within
`max_side`, 4 megapixels and the box limit), writes it to `design-renders/<render id>.png` beside the store (the
newest 32 are kept, and none older than an hour once a newer one is written), and the door answers
the agent. Four renders wait at once, engine-wide. A render ends in `unavailable` /
`design.render.no_renderer` (no lease), `.busy` (four waiting, or the app's queue full), `.timeout`
(no answer in `timeout_ms`; the client leaving ends the wait too) or `.renderer_gone` (the lease's
connection closed first); `invalid` / `design.render.args` for an option out of range; and
`design.render.failed` (`details.reason`) or `.no_surface` from the app. A client that wants only boxes and
issues passes `png: false`.

The socket is served by whichever process owns the engine: `relay serve` (headless engine —
what the test suite and CI drive; also how you run Relay's core on a machine with no display),
or `relay serve --remote` with the phone door in the same process. The desktop app is a client
of this socket. Who may claim `user` on it is decided by the connecting process (§4.2).

**One engine per instance.** The engine takes a non-blocking `flock` on
`$XDG_RUNTIME_DIR/relay-v4/<instance>.lock` before binding. Lock free → anything at the socket
path is stale and is unlinked. Lock held → `relay serve` exits 5 with "engine already running
for instance … (pid N)", whether or not the socket answers: a held lock with a dead socket is
refused, not taken over (D7). The store is opened with `locking_mode=EXCLUSIVE` by the engine,
so a second process cannot even open it read-write by mistake.

### 6.3 CLI

`relay` — see §8. It is a thin client of §6.2 with no logic of its own beyond argument
shaping and exit codes.

### 6.4 MCP

`relay mcp` is the stdio MCP server. Each implemented control-plane op the caller may actually
call becomes a tool of the same name, and the actor is bound from the MCP process environment
exactly like the CLI. Stream attachments and `bus.subscribe` / `bus.unsubscribe` need a live
connection and are not advertised as MCP tools. Tool calls cross the Unix socket and the normal engine pipeline;
the MCP process contains no domain logic.

`tools/list` is filtered by all three §9.1 layers — every tool in it is one the role can really
call — and carries `inputSchema` only. Result schemas are two thirds of the document and matter
only *after* choosing an op, so they stay one `bus.schema {op}` call away. Together these take a
builder's list from 128 tools / ~146 KB to roughly 84 / ~35 KB: a payload a harness keeps rather
than drops (D104). `RELAY_MCP_OPS` overrides the selection with a comma-separated list of op names
or `namespace.*` patterns; the engine still refuses anything the role may not call. A list that
comes out empty is reported through `session.report`, and on stderr, instead of failing silently.

### 6.5 `ui.*` ops: a shell model held by core

No client executes `ui.*` ops. Core answers every one itself, like any other op (validated,
allowlisted — agents need `session.allow_ui` — and audited when `agent_only` and the caller is
an agent), against an in-memory shell model: page, project, panes, focus and windows. The model
starts at the dashboard with one `main` window, is not persisted, and is the same whether or not
a client is connected, so a headless engine answers `ui.*` with success too; there is no
`ui.absent`. Every change emits `ui.changed` with the whole model (D40); `ui.pane.move` adds
`move: {pane, to, edge}` beside it (RA-420), so a client follows a move as it follows any other
change.

What reaches the screen is what the native client does with those events:

| op | native client |
|---|---|
| `ui.page.switch` | follows: opens that project and page. It also sends `ui.page.switch` itself when you navigate, so `ui.state.page` tracks the window, and drops the echo of its own request |
| `ui.pane.open` / `ui.pane.focus` | follows only a focused pane whose `target.session` is set: it focuses that session's terminal (on Agents). Any other target — a diff, a note, a file, a run, a mirror — opens nothing |
| `ui.pane.close`, `ui.pane.move` | nothing of their own; their `ui.changed` carries the whole model, so the client re-applies its page and focused session as for any other change (a no-op when they already match) |
| `ui.window.popout` / `.close` | nothing: no OS window opens or closes |
| `ui.toast` | shows the text in its notice bar; `level` and `ttl_ms` are carried but ignored |
| `ui.layout.apply` | applies the stored state from `layout.changed` |
| `ui.state`, `ui.window.list` | report the model: panes and windows recorded by `ui.*` ops, never the native window's own panes |

`os.reveal` and `os.open_url` run `xdg-open` on the engine's machine after commit; they involve
no client either.

Relay-2 planned a UI executor door (`bus:ui-op` / `bus:ui-result`) that answered `ui.*` from the
main window and returned `unavailable` / `ui.absent` headless. V4 never had it, and the
`executor` registry attribute that described it was removed on 2026-10-06 (D164).

---

## 7. Data plane (the one exception)

| stream | attached by | frames | notes |
|---|---|---|---|
| `pty` | `session.attach {session, from_seq?, epoch?}` | `{v:1, stream:"pty", session, epoch, seq, data: base64}`, one frame per PTY read (up to 64 KiB) | socket lines, after a catch-up from the scrollback ring. A subscriber more than 1024 frames behind loses frames (the engine logs it) and should re-attach from its last `(epoch, seq)`. `epoch` increments on every spawn/wake and `seq` restarts within it; `session.scrollback` returns `{text, epoch, seq}` so a client can resume exactly |
| `mirror` | `device.mirror.start` | `{v:1, stream:"mirror", mirror_id, seq, data}`: a base64 string is one H.264 packet; an object is the mirror's status (first, on every change, and a terminal one that ends the stream) | on the socket connection that started it; the mirror stops when that connection closes |
| `logcat` | `device.run`, `device.build` | `{v:1, stream:"logcat", run_id, seq, data: line}` | a build streams Gradle output only, then ends; a subscriber that falls behind loses lines (logged) |
| `log` | `app.log.tail` | none yet | declared, not implemented: the op answers `unavailable` / `bus.not_implemented` and attaches nothing. The engine logs to `relay serve`'s output, filtered by `RELAY_LOG` |

Input to a PTY is a bus op (`session.input`, `audit: agent_only`), not a stream — it is
low-volume in bytes and it matters who typed into whose terminal. `paste` (default false) sends
`data` as one paste: when the program has turned on bracketed paste (DECSET 2004, read from the
PTY's own output in memory, never the store) it goes between `ESC[200~` and `ESC[201~` with its
line breaks kept and every ESC and 8-bit CSI removed; otherwise each line break (`\r\n`, `\r` or
`\n`) becomes a space, so no line in it is sent as Enter.

---

## 8. CLI

### 8.1 Commands

```
relay cmd <op> [<json-payload> | -]         one op; payload from arg or stdin; prints Response
relay cmd '<json-envelope>'                  full envelope (id, actor, op, payload) if you need to
relay q   <op> [<json>]                      alias of cmd for queries; prints result only, pretty
relay events [--filter <glob>]               subscribe and print Event lines until killed
relay schema [<op>]                          print bus.v1.json, or one op's payload+result schema
relay ops [--mine]                           list ops (with --mine: only those my actor may call)
relay ping                                   liveness; exit 0 if the engine answers
relay mcp                                    stdio MCP server; actor bound from this environment
relay serve [--instance dev|test] [--store <path>]   run a headless engine (owns the socket)
```

### 8.2 Environment and defaults

| var | effect |
|---|---|
| `RELAY_INSTANCE` | `stable` (default) / `dev` / `test` — which socket |
| `RELAY_SESSION` + `RELAY_TOKEN` | actor becomes `agent:<name>` bound by token; set for every spawned session |
| `RELAY_PROJECT` | default `project_id` for project-scoped ops; else resolved from cwd's repo root via `project.list`, else error `bus.project_required` |
| `RELAY_ACTOR` | override actor (`user`/`test`); ignored if `RELAY_SESSION` is set unless `--actor` is passed explicitly |
| `RELAY_BRIEF` | worktree-relative path of this session's brief; the role instruction points at it |
| `RELAY_WORKTREE` | this session's worktree; also the default tree for `git.*` / `file.*` when the payload omits `worktree` (§9.1, D111) |
| `RELAY_MCP_OPS` | deliberate override of the MCP tool selection: op names or `namespace.*`, comma-separated (§6.4) |

An agent rarely needs `RELAY_PROJECT`: the engine fills `project_id` and `session` from the
authenticated session for ops that require them (§9.1).

Exit codes: `0` ok · `1` `invalid`/`not_found`/`conflict`/`internal` · `2` `refused` ·
`3` `held` (stdout carries the `confirm` op) · `4` `unavailable` · `5` engine unreachable.

Output is the raw `Response` JSON on stdout, one line, always — scripts parse it; humans use
`relay q`. `relay q` prints the result on stdout and a typed error on **stderr**: an error printed
beside real results reads as a result, and every caller then has to special-case a failure that
arrived looking like success.

`relay ops` lists ops; `--mine` applies all three §9.1 layers and prints `call` / `why` per row
with a callable count, and `--grep <text>` filters on name and summary.

---

## 9. Authorization and guardrails on the bus

### 9.1 Allowlists

Three layers, all enforced before the handler — and all three reported together, in one place.
`bus.ops` returns `call` (`yes` / `no` / `self_only`) and `why` on every row; `relay ops --mine`
prints them, `session.bootstrap.can_call` lists the ops that pass, and the MCP `tools/list`
advertises exactly that set. A discovery surface that knows only layer 1 overstates a builder's
write surface roughly tenfold and turns a clean upfront refusal into a mid-task failure (D104),
so no door computes this for itself.

**Layer 1 — op-level `actors` (registry, fixed).** Lifecycle and configuration ops are
`user_only`: `task.dispatch`, `task.approve/unapprove`, `session.create/spawn/resume/park/wake/close`,
`session.input/resize/discard_restorable`, `audit.undo`, `guardrail.confirm/reject`,
`guardrail.config.set`, `settings.*` mutations, `app.*` mutations, `workspace.*` and
`project.*` mutations, `provider.refresh`, `worktree.*` mutations, `git.branch.clean_merged`,
`integration.discard`, `device.*` and `avd.*` mutations except `device.claim` / `device.release`,
`skill.*` mutations, `notify.settings.set`, `design.purge`, `design.picture.key_set/key_remove`, and the
queries `design.asset.get` (an agent reads a picture from `assets_dir`) and `design.render.serve` / `design.render.answer` (the desktop app's half of a render, §6.2). `app.resources.watch` is the one `app.*` mutation
every actor may call; it only turns resource sampling on and off.
SPEC §10 says no swarm: an agent never spawns, closes, or types into a session, full stop. The one
exception is the Dev space's assistant (docs/THREADS.md, "The assistant"), which may *ask* for
these with `thread.ask`; only the person's `thread.ask.resolve` runs them, as the person.

**Layer 2 — row scope (runtime).** See *Own task / self* below: an op naming a `session` or a
`task_id` only ever acts on the caller's own. Reads reach any session in the caller's **own
project**; `session.list` draws the same boundary, and the private surfaces (`session.brief`,
`session.scrollback`) narrow it back to own-or-PAIR (D106).

**Layer 3 — per-session role (settings, editable via `guardrail.config.set`).** A session
carries a `role` set at `session.create`: `builder` (default), `reviewer`, `docs`. Baseline
allow-sets, straight from SPEC §3's agent action list plus reads:

| role | may call |
|---|---|
| `builder` | all queries · `task.link_commit` · `task.changelog.write` (own) · `task.comment` (own) · `task.update` (own; body/changelog only) · `mailbox.*` · `notes.append` · `overlap.flag/ack` · `integration.request` · `session.done/report/intent/claim/release` (self) · `device.claim/release` · `usage.report` · `guardrail.gate/check` |
| `reviewer` | all queries · `mailbox.*` · `notes.append` · `overlap.flag` · `task.changelog.write` / `task.comment` on the reviewed task · `session.done/report/intent/claim/release` (self) · `device.claim/release` · `usage.report` · `guardrail.check` — **nothing** that writes files, commits, or moves the task |
| `docs` | builder's set minus `task.link_commit`/`integration.request`, plus `notes.create/update` |

**Open to every role:** `design.*` (`guardrail::OPEN_TO_EVERY_ROLE`) passes layer 3 for every role
whatever its allowlist says, so a saved list cannot shut it out. A design is the person's to ask for
and to undo: every `design.apply` answers with its inverse, which the board's Undo replays through
`audit.undo`; `design.purge` stays the person's by its registry actors. Agents are told to touch a
design only when asked.

No role moves a task on the board: an agent hands its task to review with `session.done`, which
judges the branch first (§9.2), waits for the task's other builders and tells the reviewer. A
`task.move` from an agent is `conflict` / `task.column_transition` even where a configured
allowlist still lists it.

Session options widen a role deliberately, per session, never by default:
- `bus_writes: true` — adds `file.*` and `git.stage/unstage/commit` mutations for agents that
  are told to write *through* Relay (guardrails then run in-line, §9.2). Off by default because
  both providers write with their own tools; the enforcement doors in §9.3 cover that.
- `allow_ui: true` — adds `ui.*` and `os.*` so an agent may open a diff pane or switch pages.

`user` and `test` bypass layer 3 (never layer 1's `agent_only`-for-agents semantics — those
ops simply admit everyone). A denied op is `refused` / `actor.allowlist` with `details.role`,
`details.op`, and `details.option` naming the session option that would allow it, if any.

**Own task / self.** For every op with `task_id` or `session` in its payload and an agent
actor, core compares against the actor's session record: the actor's name ∈ `task.sessions`
and `session == actor's name` (or its PAIR partner, or — for queries — any session in the same
project). Otherwise `refused` / `actor.scope`. `task.dispatch` is user-only, so "own task" is
always assigned by you, never claimed.

**Identity is filled in, not demanded.** An agent is a project and a session; the pipeline fills
`project_id` and `session` from the authenticated session when an op *requires* them and the
payload omits them (D110). Only required fields are filled — an optional `project_id` is usually
one half of an either/or. Ops with two optional targets resolve their own default from the actor
instead: `session.peers` with neither means "my project, minus me".

### 9.2 Guardrail policies (SPEC §5), as bus behaviour

| policy | applies to | outcome |
|---|---|---|
| protected paths | `guardrail.gate {kind: write}`, `file.write/create/rename/move/delete`, `git.commit` / `gate {kind: commit}` touching them | `refused` / `guardrail.protected_path` |
| destructive write (> N lines removed, or > P% of a file of at least `min_file_lines`) | `gate {kind: write}`, `file.write` | `held` / `guardrail.destructive_write`; `confirm: guardrail.confirm {hold_id}`; notification. The old size is read from the file on disk, for a `new_text` and a `diff` alike; the percentage is skipped for short files, where a share measures nothing (D112) |
| shape gates (registered validators for critical files) | `gate {kind: write}`, `file.write` | `held` / `guardrail.shape_gate` with `details.validator`, `details.reason` |
| per-task caps (files, lines) | `gate {kind: commit}`, `git.commit`, `session.done` | `refused` / `guardrail.cap` with the numbers |
| branch re-check | `session.done {status: completed}` from an agent on a task | every commit since base (newest 200) against protected paths, the net diff against caps and protected paths, and each shape-gated file the branch changes as it is at `HEAD`; uses the session's grants without spending them; `refused` with the policy's code (a per-commit refusal names `details.commit`). A hook can be skipped, so this is the check a skipped hook cannot dodge (RA-109). git runs with the settings that change its output pinned (`-c core.quotePath=true`, `color.ui=never`, `diff.noprefix=false`, …). If git still cannot measure — no base, no history in common with it, a config value git refuses to read — the done is `refused` / `guardrail.unmeasured`, never taken as within limits; `blocked`/`partial` stay sayable, and the person moves the task |
| hook bypass | `gate {kind: exec}` from an agent | `git commit -n`/`--no-verify` (also in flag clusters and abbreviations), any `core.hooksPath` override (`-c`, `--config-env`, `GIT_CONFIG_*`) and `git config` writes to it: `refused` / `guardrail.hook_bypass`, not grantable |
| write roots | `gate {kind: write}` with a path outside the worktree | allowed inside `guardrails.allowed_write_roots` or the process temp directory (scratch space is not a repo-integrity concern); otherwise `refused` / `guardrail.write_root`, naming the roots that would have worked (D102) |
| denied commands | `gate {kind: exec}` | matched against the **parsed argv** of each command in the line, never a raw substring, so quoted data naming a pattern is not a match; a `relay … guardrail.check` command is exempt so the dry run is always askable (D103). The defaults are `rm -rf`, `git reset --hard`, `git clean -fd` and `git push --force`, and a `git push … -f` pattern covers every push that can destroy what the remote holds: `--force`, `-f`, `+ref`, `:ref`, `--force-with-lease`, `--force-if-includes`, `--mirror`, `--delete`/`-d` and `--prune`. `git push` and `git push -u origin <branch>` are not |
| user bypass | any of the above when actor is `user` | `held` / `guardrail.user_bypass` — you confirm, it proceeds, audit says you did |

### 9.3 Enforcement doors — how a policy actually binds an agent

Claude Code and Codex write files with their **own** tools and commit with their **own**
`git`; they do not call `file.write`. So the bus exposes one gating op and Relay installs
callers of it wherever the provider lets us:

- **`guardrail.gate`** (mutation · agent · session) — `{session, kind: "write" | "commit" |
  "exec", path?, new_text?, diff?, command?}` → `{verdict: "allow"}`. A refusal is not a
  verdict: it arrives as `ok: false` with a typed `refused` error, and a hold as a typed `held`
  error whose `confirm` carries `{hold_id}`, so a caller checks `ok` before `result`. (The result
  type still declares `refuse`/`hold`, `error` and `hold_id`; the gate never sets them.
  `guardrail.check`, the dry run, does return its verdict in the result.) Unlike `guardrail.check` (a pure dry run) it may **create a hold** and it
  is audited. It is what a hook calls; the hook fails the tool on `hold` as on `refuse`. A
  person who confirms the hold (`guardrail.confirm`) leaves a single-use pass for that exact
  action — same policy, kind, path, text, diff and command — so the agent's identical retry,
  or the person's own re-run commit, goes through once; anything different is gated afresh.
  An agent held on `destructive_write` is also told it may ask for the path
  (`guardrail.request`).
- **Claude Code**: at `session.spawn` Relay merges a per-worktree
  `.claude/settings.local.json` (or the equivalent the CLI version accepts — `provider.list`
  reports the spawn profile). Phase 4 installs `PreToolUse` on
  `Write`/`Edit`/`MultiEdit`/`Bash`; its thin CLI adapter translates the hook's stdin into
  `guardrail.gate` and exits 2 on hold/refusal or infrastructure failure, including a gate that
  has not answered within 20 s: the provider kills a hook at 30 s and then runs the tool
  unchecked (D163). A `Bash` call meets the `exec` gate and then one `write` gate per file the
  command visibly writes — redirections, `tee`, `sed -i`, `truncate`, `rm`, `cp`/`mv` and the
  like, inside `sh -c` too — with an overwrite or delete carrying the line count it removes.
  Targets built from variables or globs, and files a program opens by itself, are not seen
  (D163). Phase 5 extends the
  same local file with `SessionStart`/`PostToolUse`/`Stop`/`Notification` hooks that call
  `session.report` (§10.8), and a `statusLine` command that writes Claude's rate-limit payload
  to `${CLAUDE_CONFIG_DIR:-$HOME}/.claude/relay-usage.json` for `usage.get` to read (D64); it
  calls nothing on the bus. This is v3's `agenthooks.rs`, now a bus client instead of bespoke
  files.
- **git**: every Relay-created worktree gets a `pre-commit` hook calling `relay cmd
  guardrail.gate '{"kind":"commit"}'`; the hook exits non-zero on `refuse`/`hold`. This binds
  both providers' commits, and yours.
- **Codex**: Relay merges `PreToolUse` and lifecycle hooks into `.codex/hooks.json` (D132);
  they run only after the person trusts them once in Codex's `/hooks`, which Relay surfaces and
  never bypasses. A launch is refused (`conflict` / `session.codex_hooks_tracked`) when the
  repository tracks that file, since Relay's handlers name this machine's binary and a commit
  would share them; an untracked one is added to `info/exclude` once Relay's handlers are in
  it, never before. `Bash` is gated as for Claude: `exec`, then a `write` gate per visible target.
  `apply_patch`, Codex's edit tool, is parsed into one `write` gate per file — an added file
  with its text, an updated one with its text after the hunks are applied in memory, a deleted
  one with the lines it removes, a move's source by path alone — so protected paths, destructive
  writes, shape gates and write roots bind Codex's edits as they bind Claude's Write and Edit
  (D163). Developer instructions still carry the role instruction, the compact brief and the
  `$RELAY_BIN q <op>` shell path to the bus, and the PTY reader stamps `last_output_at` for
  every provider (D108). No post-hoc write watcher exists: a write the hooks cannot see — a
  script opening files itself — is caught only by the commit gate, which checks protected paths
  and caps, not destructive writes or shape gates.

### 9.4 Holds and confirmation

Holds live in the `holds` table with the frozen envelope. `guardrail.confirm {hold_id}` is a
**user** op that re-executes the held envelope:
- with the **confirm request's** `id` (the original id stays audited as `held` and keeps
  replaying `held` — §5.3);
- audited as the held op with `actor` = the confirmer and `on_behalf_of` = the original
  actor, in the new audit row;
- run **as the original actor and its session**, not the confirmer: the worktree, grants and
  any nested hold resolve from the held session, and every policy but the one that raised the
  hold still judges the original actor;
- skipping **only** the policy that raised the hold — a protected path still refuses, a cap
  still refuses. Allowlist and own-task checks are not run again: they passed when the
  original request was held, before the hold was written;
- returning `{hold: Hold, outcome: Response}` — the outcome is the original op's response.

`guardrail.reject {hold_id, reason?}` closes it. Either way core emits `guardrail.resolved`
`{hold_id, state, by}` and, if the original actor was an agent, drops a mailbox message to it
("your write to X was confirmed/rejected: <reason>") so a waiting agent learns the answer
through the channel it already reads. Open holds expire (`state: expired`) when their session
closes.

### 9.5 Exceptions: an agent that cannot progress asks

A refusal an agent cannot work around is not the end of the task. Every agent refusal a grant
could lift carries `hint` (printed by both hook adapters) and `details.exception {kind, value}`
naming the request to make:

- **`guardrail.request`** (mutation · agent · session, callable by *every* role regardless of the
  allowlist) — `{session, kind: "command"|"path"|"cap", value, reason, scope?: "once"|"session"}`
  → `{request: GuardrailException, created, wait_for: "guardrail.request_resolved"}`. It is
  stored as an open hold (`op: guardrail.request`, `policy: exception`), so it appears wherever
  holds do, raises a notification, and expires with its session. An identical open (or still
  active) request from the same session is returned instead of duplicated.
- The user answers with **`guardrail.confirm {hold_id, scope?}`** (no replay: it records a grant
  of `scope`, default the requested one) or **`guardrail.reject {hold_id, reason?}`**. Both emit
  `guardrail.request_resolved {request_id, session, state, scope?|reason?}` and mail the agent.
  The agent waits with `bus.wait {events: ["guardrail.request_resolved"], matching:
  {request_id}}` and, after a timeout, re-reads `guardrail.request.get`.
- A **grant** lifts exactly one rule for exactly that session, in `guardrail.gate`, every
  enforcing `file.*` / `git.commit`, and (read-only) `guardrail.check`. `command` covers that
  exact command, every word as written, quoted ones included (a `*` is the shell's glob, not a
  wildcard), and a grant of several commands (`cd dist && rm -rf *`) covers only that whole
  line; every denied command in a line needs its own. A grant ends with its session. `path` covers protected paths, write roots (absolute prefix)
  and large rewrites; never shape gates. `cap` raises the caps it names (`files=N lines=M`) or,
  naming neither, lifts them. A `once` grant is spent only when the action it let through was
  allowed, then announced as `guardrail.resolved {state: "used"}`.
  `guardrail.grant.revoke {request_id}` (user) ends one early; `guardrail.requests.list
  {project_id?, session?, state?: open|active|all}` lists them.
- **No self-approval.** The answers are user-only on the bus. An agent's `exec` gate also refuses,
  with the ungrantable `guardrail.self_approval`, any `relay q|cmd` invocation whose op (or
  `cmd` envelope) is a user-only guardrail/settings answer, the CLI commands that pair a phone
  or link the PC to a cloud engine (`relay remote approve|pair|decline|revoke|via`, `relay remote
  serve --pair`, `relay cloud connect|pair|off`, which also refuse to run with `RELAY_SESSION`
  set), any call claiming `--actor user|test`, any envelope claiming `"actor":"user"`, and any line that sheds `RELAY_SESSION`
  (`unset`, `env -u`, `env -i`, `sudo`, …) or sets `RELAY_ACTOR`. Commands inside `sh -c`,
  heredocs fed to a shell, scripts piped into one and interpreter `-c`/`-e` code are read too;
  searching for the names (`rg guardrail.confirm`) is not an invocation. Behind that, the socket
  refuses a `user`/`test` claim from any process in an agent session's tree or elsewhere below
  the engine (`actor.peer`, §4.2), so a raw envelope written with `socat` or a script fails too.
  An agent shares the user's uid and can still reach outside the engine's tree (`systemd-run
  --user`, a cron job, the store file itself), so this is a seatbelt, not a security boundary.

### 9.6 Configuration layers

Effective config is `defaults <- global <- workspace <- project <- the project's legacy
protected_paths / critical_files`. Each layer stores only what it overrides
(`guardrails.*`, `guardrails.workspaces.{id}.*`, `guardrails.projects.{id}.*`); a `null` in a
`guardrail.config.set` patch clears a key back to the inherited value, and an object a patch
empties is dropped rather than stored as `{}` (which used to replace the subtree).
`guardrail.config.layers` answers one layer with its `effective` and `inherited` configs, its raw
`overrides`, and `sources`: every leaf path mapped to the layer that decided it.

---

## 10. Op catalogue

Notation: `op — kind · audit · undo · scope` then `payload → result`, TS-ish, `?`
optional. Entity shapes are in §11. `Id = number`. Every project-scoped op takes
`project_id: Id` unless the payload names a session (sessions know their project).

### 10.1 bus

| op | attrs | payload → result |
|---|---|---|
| `bus.ping` | query · global | `{}` → `{ pong: true, instance, version, uptime_s }` |
| `bus.schema` | query · global | `{ op?: string }` → `{ schema: JsonSchema }` (whole `bus.v1.json`, or one op's `{payload, result}`) |
| `bus.ops` | query · global | `{ actor?: Actor }` → `{ ops: OpInfo[] }` — registry attributes plus the all-layers `call`/`why` verdict for the (given or calling) actor (§9.1) |
| `bus.whoami` | query · global | `{}` → `{ actor, is_agent, session?, role?, project_id?, project?, worktree?, branch?, can_call: string[], write_roots: string[] }` — identity and capability in one call, for any actor (D119) |
| `bus.wait` | query · global · socket only | `{ events?: string[], timeout_ms? = 60000, matching?: object }` → `{ event?: Event, timed_out }` — block until a matching event arrives. The wake-up an agent has instead of a poll loop; events fired before the call are not replayed, so read state first, then wait (D115). `matching` takes only an event whose payload has each given top-level key equal to the value given; anything but an object is `bus.schema` |
| `bus.subscribe` | query · global · socket only | `{ events?: string[] }` → `{ subscribed: string[] }` — then `Event` lines; `bus.lagged {dropped}` when some were dropped (§6.2) |
| `bus.unsubscribe` | query · global · socket only | `{}` → `{}` |

### 10.2 app

All `app.*` mutations except `app.resources.watch` are `user_only` (§9.1 layer 1).

| op | attrs | payload → result |
|---|---|---|
| `app.version` | query | `{}` → `{ version, instance, build: {profile, git_sha, built_at} }` |
| `app.status` | query | `{}` → `{ pid, uptime_s, store_path, socket_path, sessions_live: number, providers: ProviderInfo[], gaming: GamingStatus }` — `GamingStatus = { supported, on, auto, threads, active, game? }`: gaming mode (`settings: performance.gaming.{on, auto, threads}`, default off, off, 2). While `active` (`on`, or `auto` with a game running: Steam's launch reaper, gamescope or Lutris's wrapper), every agent and what it runs share `threads` CPU threads at the lowest CPU and disk priority — each agent is moved into a scope under the user manager's `relay-agents.slice`, and the cap is that slice's `CPUQuota`/`CPUWeight`/`IOWeight`, so the engine and the window stay uncapped. `supported` is false without a systemd user manager. The engine emits `gaming.changed` (the same object) when it changes |
| `app.quit` | mutation · always · global | `{ force?: bool }` → `{}` — refuses (`conflict`/`app.sessions_live`) unless `force` or no live sessions |
| `app.resources.get` | query | `{}` → `{ relay: {pid, rss_mb, cpu_pct}, panes: {session, pid, rss_mb, cpu_pct}[], worktrees: {path, disk_mb}[], store_mb, total_rss_mb }` |
| `app.resources.watch` | mutation · never · global | `{ on: bool }` → `{}` — while any client watches, `resource.sample` events flow (§1.3); otherwise none |
| `app.recovery.last` | query | `{}` → `{ at, reaped_pids: number[], fsck_fixes: string[], dirty_worktrees: string[], tasks_reset_offered: Id[] } \| null` — what crash recovery did at the last launch (SPEC §14) |
| `app.log.tail` | query · stream | `{ level?: "trace"\|"debug"\|"info"\|"warn"\|"error", filter?: string }` → attaches `log` stream. **Not built yet:** answers `unavailable`/`bus.not_implemented`; the engine logs to stderr only |
| `app.backup.now` | mutation · always · global | `{}` → `{ path, bytes }` |
| `app.backup.list` | query | `{}` → `{ backups: {path, bytes, created_at, reason: "manual"\|"upgrade"}[] }` |
| `app.import.v3` | mutation · always · global | `{ source: path, project_id: Id, dry_run?: bool }` — `source` is a v3 `.relay/` dir, its `relay.db`, or the repo containing it; one-time per source (`conflict`/`import.already_done`); mapping in DECISIONS D14 → `{ counts: {tasks, modules, notes, sessions}, id_map: {tasks: Record<old, new>, modules: Record<old, new>, notes: Record<old, new>}, warnings: string[] }` — the id map is also the audit row's `result_summary` |
| `app.first_run.state` | query | `{}` → `{ needed: bool, steps: {workspace, providers, project, import}: "todo"\|"done"\|"skipped" }` |
| `app.reconcile` | mutation · always · global | `{}` → `{ actions: string[] }` — the retention pass of §5.4, on demand; same action vocabulary as `app.recovery.last` |

### 10.3 audit

| op | attrs | payload → result |
|---|---|---|
| `audit.list` | query · user | `{ project_id?, actor?, session_id?, op_prefix?, parent_req?, since?, until?, limit?: ≤1000 }` → `{ rows: AuditRow[] }` — `since`/`until` take RFC 3339 with any offset, a bare date (UTC midnight) or epoch seconds/ms; anything else is `invalid`/`time.invalid` |
| `audit.get` | query · user | `{ audit_id }` → `AuditRow` (with stored payload if kept) — both reads are `user_only`: stored payloads hold frozen actions, launch prompts and private mail |
| `audit.undo` | mutation · always · undo none · global · user | `{ audit_id, force?: bool }` → `{ undone: Id, by: Id }` — `conflict`/`audit.not_undoable` if the row has no inverse or was already undone; `conflict`/`audit.stale` if the entity changed since (§5.5) |

A `design.apply` row keeps, with its undo, what the undo expects to find (`expect.nodes`): each field
the list updated at the value it held then, each moved node's parent, and each added container's
`within` (the ids it may hold: anything else added inside it since is a conflict). `audit.undo` refuses
`conflict` / `audit.stale`, with `details.conflicts`, when a later change would be overwritten: a field
that holds another value, a node in another parent, a container holding nodes added since, or an id the
undo would add back that is taken. A node removed since is not a conflict: the lenient replay skips it.
`force: true` skips the check. Rows kept before expectations existed undo as they did.

### 10.4 workspace / project

All mutations here are `user_only`. Uniqueness rules, once: session `name` is unique among
non-closed sessions; layout name is unique per project (save overwrites); project `path` is
unique; nothing else is.

| op | attrs | payload → result |
|---|---|---|
| `workspace.create` | mutation · always · global | `{ path?, name? }` → `Workspace`; blank/omitted path uses the same repository-aware default as `workspace.discover` |
| `workspace.discover` | query | `{ path? }` → `{ path, repositories: {path,name}[] }`; resolves blank to the parent of the Git checkout holding the *engine's* working directory (or that directory outside a checkout) — under `relay serve` the engine home, not the caller's directory, so a client that means "here" sends its own absolute path — and scans bounded descendants for Git roots, skipping any it cannot read |
| `workspace.list` | query | `{}` → `{ workspaces: Workspace[] }` |
| `workspace.update` | mutation · always · inverse | `{ workspace_id, name?, order? }` → `Workspace` |
| `workspace.reorder` | mutation · always · inverse · global | `{ orders: { workspace_id, order }[] }` → `{ workspaces: Workspace[] }` — a sidebar drag in one call: every named workspace takes its `order` in one transaction, unnamed ones keep theirs; one audit row whose inverse is a `workspace.reorder` back to the previous orders, and one `workspace.changed {workspaces}` carrying the rows it wrote. `not_found`/`workspace.not_found` for an unknown id (nothing written); `invalid`/`workspace.orders` for an empty list or an id named twice |
| `workspace.remove` | mutation · always · global | `{ workspace_id, force?: bool, remove_worktrees?: bool, discard_changes?: bool }` → `{ projects_removed, sessions_closed }` — `conflict` (`workspace.has_projects`, `details.projects`) if it still has projects, unless `force`, which runs `project.remove { force }` for each of them first. With `remove_worktrees`, refused `conflict`/`worktree.dirty` as `project.remove` is, across all its projects, before anything is backed up or closed |
| `project.add` | mutation · always · global | `{ workspace_id, path, name? }` → `Project` — path must be a git repo root **inside** `workspace.path` (`invalid`/`project.outside_workspace`); one project per path (`conflict`/`project.exists`); `build_cmd` starts as the test command of the build tool at its root (`cargo test`, `go test ./...`, `./gradlew test`, `npm`/`pnpm`/`yarn`/`bun` `test` when `package.json` has a real test script), else null |
| `project.clone` | mutation · always · global | `{ workspace_id, url, dest? }` → `{ project: Project }`; clones inside the workspace and registers the result |
| `project.list` | query | `{ workspace_id? }` → `{ projects: Project[] }` |
| `project.get` | query | `{ project_id }` → `Project` |
| `project.update` | mutation · always · inverse | `{ project_id, name?, build_cmd?, run_cmd?, base_branch?, protected_paths?, critical_files?, order?, pinned? }` → `Project` |
| `project.reorder` | mutation · always · inverse · global | `{ orders: { project_id, order }[] }` → `{ projects: Project[] }` — as `workspace.reorder`, for projects (`project.not_found`, `project.orders`); emits one `project.changed {projects}`. `pinned` still sorts first in `project.list` |
| `project.remove` | mutation · always · project | `{ project_id, force?: bool, remove_worktrees?: bool, discard_changes?: bool }` → `{ sessions_closed, runs_stopped }` — forgets project-owned Relay metadata but never touches repository files; `conflict` if sessions (`project.sessions_live`, `details.open_sessions`) or device runs are live, unless `force`, which closes every open session through `session.close` and stops the runs first. Closed sessions keep their worktrees and branches unless `remove_worktrees` (Relay-pool checkouts only, deleted once the store unlocks; branches always kept). A checkout `remove_worktrees` would delete that holds uncommitted work (tracked edits or untracked files) refuses the whole removal `conflict`/`worktree.dirty` (`details.worktrees: [{ worktree, sessions, changed }]`, `changed` null when git status failed) before anything is backed up or closed, unless `discard_changes: true` (RA-405); a session opened after that check is closed but its checkout kept. An integration in progress refuses even with `force` |
| `project.remove.preview` | query · user | `{ project_id? } \| { workspace_id? }` (exactly one) → `{ projects, tasks, notes, modules }` — what a removal would delete (live, untrashed rows), for the confirmation dialog |
| `project.relink` | mutation · always · inverse | `{ project_id, path }` → `Project` — the repository moved: `path` must be a git repo root (`invalid`/`project.path`), no other project's path (`conflict`/`project.exists`) and inside a workspace (`invalid`/`project.outside_workspace`); the project stays in its workspace if that still contains it, else joins the innermost one that does. Stored worktree and trash paths under the old root are rewritten and the moved checkouts get `git worktree repair` after the commit. `conflict` (`project.sessions_live` / `project.activity_live`) while sessions are open or an integration or device run is live. Undo relinks to the old path |
| `project.removed.list` | query · user | `{}` → `{ removed: [{ backup_path, created_at, reason, project_id, workspace_id, name, path }] }` — projects absent from the store that a `project-remove` / `workspace-remove` backup still holds, each from the newest such backup |
| `project.restore` | mutation · always · global | `{ backup_path, project_id }` → `{ project, tasks, notes, modules, workspace_restored }` — copies a removed project back from its removal backup (read-only, before the transaction), with its original ids, in one transaction: the project, labels, modules, tasks with their labels, relations, commits and attachments, module unlinks, notes, notifications, file-trash records, layouts, skill/plugin enablement and its `guardrails.projects.<id>` / `layout.current.<id>` settings. Sessions (and their mailbox, claims, overlaps), integrations and device runs stay gone. A removed workspace comes back with it unless its directory is a workspace again, which then takes the project. `backup_path` must be a `store-*.db` directly in the store's `backups/` (`invalid`/`project.backup_path`); `conflict`/`project.exists` if the id is live or the path is another project's; `not_found`/`project.not_in_backup` |
| `project.stats` | query | `{ project_id }` → `{ tasks_by_column, sessions_live, sessions_idle, worktrees, disk_mb }` |

### 10.5 task (SPEC §6)

| op | attrs | payload → result |
|---|---|---|
| `task.create` | mutation · always · inverse (delete) · project | `{ project_id, title, body?, column?: Column = "backlog", state?: TaskState, priority?: Priority = "medium", size?: Size, module_id?, changelog?, attachments?: AttachmentIn[], type?: TaskType = "task", parent_id?, labels?: string[], note_id? }` → `Task` — `note_id` records the note the task was made from: a live note in the same project, else `not_found` / `notes.not_found` |
| `task.get` | query | `{ task_id }` → `Task` (with attachments, commits) |
| `task.list` | query | `{ project_id?, column?, state?, module_id?, priority?, include_deleted?, sort?: "column"\|"priority"\|"updated", type?, label?, session?, note_id?, parent_id?: Id\|null, limit? (1000, ≤2000), offset?, summary? }` → `{ tasks: Task[], next_offset? }` — a page; `summary` leaves `body` and `changelog` empty. `project_id` optional so the Dashboard can ask "in review, everywhere"; `parent_id: null` is roots only, `session` is "every card this agent was ever sent", `note_id` the tasks made from that note. Each page reads its sessions, commits, attachments, labels, relations and children in one statement per table, and every task in it equals its `task.get` (RA-412) Every task names its module (`module_name`, completed modules included), so a board labels cards without `module.list` |
| `task.update` | mutation · always · inverse | `{ task_id, title?, body?, priority?, size?, module_id?: Id\|null, state?, changelog?, type? }` → `Task` |
| `task.move` | mutation · always · inverse | `{ task_id, column: Column, position?: number }` → `Task` — transitions table in §11.1; `position` is the 0-based index the task ends at in the column (the others shift around it, clamped to the end), omitted means last; agents may not move tasks (`conflict` / `task.column_transition`): `session.done` hands theirs to review |
| `task.delete` | mutation · always · inverse (restore) | `{ task_id }` → `{}` |
| `task.restore` | mutation · always · inverse (delete) | `{ task_id }` → `Task` |
| `task.link_commit` | mutation · always | `{ task_id, sha, branch? }` → `Task` |
| `task.changelog.write` | mutation · always · inverse | `{ task_id, text, expected_updated_at? }` → `Task` — `expected_updated_at` is the task's `updated_at` as the caller last read it: given and no longer current, the write is refused (`conflict` / `task.edit_conflict`, the code `task.update`'s `expected` uses); omitted, it writes as before. Nothing tracks reads (RA-413) |
| `task.comment` | mutation · always · project | `{ task_id, body }` → `TaskComment { id, task_id, author, body, created_at }` — append-only, GitHub-issue style; `author` is the agent's session name or `user`. A blank body is `invalid`/`task.comment_empty`, one over 64 KiB `invalid`/`task.comment_size`; a deleted task is `task.not_found`. `task.activity` returns every comment oldest first as `comments` and leaves `task.comment` out of `history` |
| `task.attach` | mutation · always | `{ task_id, name, mime, bytes_b64 }` or `{ task_id, path, name?, mime? }` → `Attachment` — by path, `name` and `mime` set what the copy is stored as (default: the file's own name, `application/octet-stream`) (RA-414) |
| `task.detach` | mutation · always · inverse (attachment.restore) | `{ task_id, attachment_id }` → `{}` — soft: the row gets `deleted_at` and leaves `Task.attachments`, the file stays. The retention pass removes row and file after `undo.grace_days` (RA-414, RA-409) |
| `task.attachment.restore` | mutation · always · inverse (detach) | `{ task_id, attachment_id }` → `Attachment` — brings a detached attachment back with its id and file; `task.attachment_not_found` when it is not detached (or was purged) |
| `task.parent.set` | mutation · always · inverse · project | `{ task_id, parent_id?: Id\|null, position? }` → `Task` — promote a task into a sub-task, re-parent it, or (no `parent_id`) detach it back to a root. Refuses `task.parent_cycle`, `task.depth`, `task.children_full`, `task.parent_project` |
| `task.children` | query | `{ task_id, recursive? }` → `{ tasks: Task[] }` — direct children in board order, or the whole subtree |
| `task.label.add` / `task.label.remove` | mutation · always · inverse | `{ task_id, label }` → `Task` — `add` creates the project label if the name is new; names are matched case-insensitively and the stored spelling wins |
| `task.label.list` | query | `{ project_id }` → `{ labels: Label[] }` |
| `task.relate` / `task.unrelate` | mutation · always · inverse | `{ task_id, relation: "blocked_by"\|"duplicate_of", other_id }` → `Task` — stored and rendered, never enforced; `duplicate_of` is single-valued, so a second `relate` replaces the first |
| `task.dispatch` | mutation · always · project · user | `{ task_id, session?: string, create?: SessionCreateIn, fanout?, start?: bool }` → `{ task: Task, session: Session, fanned: { task, session }[] }` — one of `session`/`create`; moves the task to `active`, sets `state: dispatched`, and appends it to the session queue. `start` defaults true; false stages assignments before the provider starts. `fanout` also dispatches every not-done descendant, one fresh session each, and therefore requires `create` (`task.fanout_target`). The sessions are created before the transaction; if it fails, they are closed with their checkouts, and one made for a sub-task finished meanwhile is closed after it |
| `task.approve` | mutation · always · inverse (unapprove) · project · user | `{ task_id, sha? }` → `Task` — any open column → `done`, links sha (defaults to the recorded session branch head, or the project checkout HEAD without a session) |
| `task.unapprove` | mutation · always · inverse (approve) · project · user | `{ task_id, column: Column, position: number, state: TaskState, sha? }` → `Task` — `task.approve`'s inverse: a `done` task goes back to `column` (not `done`) at index `position`, as `task.move` places it, with run state `state`, and the commit link `sha` is removed. The approval records `sha` only when it created that link, so a commit linked before it stays (RA-416). Refuses `task.column_transition` when the task is not done |
| `task.copy_text` | query | `{ task_id }` → `{ text }` — a task as plain text: `#id title`, then the body when it has one. Agents' copy; the desktop board's copy button still builds its own string (title, body, `#id`) and does not call it |

### 10.6 module (SPEC §7)

| op | attrs | payload → result |
|---|---|---|
| `module.create` | mutation · always · inverse | `{ project_id, name, icon?, priority? }` → `Module` |
| `module.get` | query | `{ module_id }` → `Module & { tasks_by_state }` — an agent is refused another project's module, `refused`/`actor.scope` (D106) |
| `module.list` | query | `{ project_id?, include_archived? }` → `{ modules: ModuleSummary[], header: {count, in_flight, issues, completed, completion_pct} }` — an agent sees its own project only (`project_id` may be left out; another is `refused`/`actor.scope`, D106); the person must name one (`invalid`/`module.project`) |
| `module.update` | mutation · always · inverse | `{ module_id, name?, icon?, priority?, order? }` → `Module` |
| `module.complete` | mutation · always · inverse (reopen) | `{ module_id }` → `Module` (archived, `completed_at` set) |
| `module.reopen` | mutation · always · inverse | `{ module_id }` → `Module` |
| `module.delete` / `module.restore` | mutation · always · inverse | `{ module_id }` → `{}` / `Module` — delete unlinks tasks (they keep existing, `module_id: null`) |
| `module.stats` | query | `{ project_id }` → same as `module.list.header` — an agent's own project only (D106) |
| `module.changelog.draft` | query | `{ module_id, group_by?: "priority" }` (the only grouping built; any other value is `invalid` / `module.changelog_group`) → `{ markdown, tasks: Id[] }` — an agent's own project's modules only (D106) |

### 10.7 notes / mailbox (SPEC §3, §12)

| op | attrs | payload → result |
|---|---|---|
| `notes.list` | query | `{ project_id, pinned_only?, include_deleted?, summary? }` → `{ notes: Note[] }` — `summary` cuts each `body` to its first 240 characters (`notes.get` has it whole) |
| `notes.get` | query | `{ note_id }` → `Note` |
| `notes.create` | mutation · always · inverse | `{ project_id, title?, body, pinned? }` → `Note` — a body is at most 1 MiB here, in `notes.update` and after `notes.append` (`invalid` / `notes.body`). `notes.changed` carries the note without its body, plus `body_bytes` |
| `notes.update` | mutation · always · inverse | `{ note_id, title?, body?, pinned?, expected? }` → `Note` — `expected` holds the original values of the fields being edited; one that no longer matches refuses the write (`conflict` / `notes.edit_conflict`) |
| `notes.append` | mutation · always | `{ note_id?, project_id?, target?: "standing" \| "suggestions", text, expected_updated_at? }` → `Note` — appends to an explicit note, the standing note by default, or the unpinned per-project Agent suggestions note. Suggestions require a bound agent with a current task; Relay adds timestamp, session, and task identity and never injects this note into a brief. `expected_updated_at` is the target note's `updated_at` as the caller last read it: given and no longer current, nothing is appended (`conflict` / `notes.edit_conflict`); omitted, it appends as before (RA-413). Of the other note writes an agent role can reach, `notes.create` makes a new note and `notes.update` takes `expected`; `notes.pin/delete/restore` are in no agent role |
| `notes.pin` | mutation · always · inverse | `{ note_id, pinned: bool }` → `Note` |
| `notes.delete` / `notes.restore` | mutation · always · inverse | `{ note_id }` |
| `notes.standing` | query | `{ project_id }` → `{ text }` — the project's standing note. Agents read it on request; it is not injected into briefs (D166) |
| `mailbox.send` | mutation · always · project | `{ project_id, to: string \| "*", text, re_task?: Id, priority?: bool }` → `{ message, recipients, delivery }` — priority mail produces the response `mail` hint until acknowledged. Agents may prioritize only direct task-linked mail and may have only one unread priority message outstanding per recipient; user/system sends are unrestricted. |
| `mailbox.outbox` | query | `{ project_id, since?, limit? = 100 }` → `{ sent: {message: Message, recipients: {session, state, acked_at?}[]}[] }` — what this actor sent, and where each addressee stands (D109) |
| `mailbox.list` | query | `{ project_id, session?: string, unread_only?, since?, limit? (200, ≤1000), before?: message id }` → `{ messages: Message[], next_before?, more_unread? }` — a page, oldest first: the newest page (older ones through `before: next_before`), or with `unread_only` the oldest unread, `more_unread` when more wait. Message text is at most 64 KiB; system notices are clipped to 2,000 bytes |
| `mailbox.ack` | mutation · agent_only | `{ message_id }` → `{}` |

### 10.8 session (SPEC §10)

Lifecycle ops (`create/spawn/resume/clear_restorable/park/wake/close/update/input/resize/discard_restorable`)
are `user_only` (§9.1). Agents get `done`, `report`, `attach`/`scrollback`, `bootstrap`, `brief`, `peers`,
`get`, `list`.

| op | attrs | payload → result |
|---|---|---|
| `session.create` | mutation · always · project | `SessionCreateIn = { project_id, provider: Provider, role?: Role = "builder", model?, effort?, branch?, worktree?: "new"\|"primary"\|path, task_id?, module_id?, pair_with?: string, use_api_key?: bool }` → `Session` — allocates name + worktree, spawns nothing; an omitted `worktree` is `"new"` unless a plugin on for the project sets `default_checkout: "primary"` (D160); one reviewer may share that PAIR worktree with at most two builders (`conflict`/`session.review_group_full`). `use_api_key: true` (Claude only; ignored for Codex) bills the session to an Anthropic API key instead of the signed-in subscription, on every spawn and resume, through the key proxy (docs/KEY-PROXY.md): `relay-key-proxy`, run as its own system user, holds the key, so nothing running as the person can read it. Claude's inline `--settings` gain `env.ANTHROPIC_BASE_URL: "http://127.0.0.1:7445"` and `apiKeyHelper: "/usr/bin/cat ~/.config/relay/key-proxy-token"`, the local pass token the proxy swaps for the key. Relay stores only the choice and never has the key. A spawn or resume while the token file is missing or the proxy does not answer on 127.0.0.1:7445 is `unavailable`/`session.key_proxy`. A phone cannot ask for it (`refused`/`remote.api_key`, also through `task.dispatch`'s `create`). Every Claude session's environment drops `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL`; an API-key session's also `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CODE_USE_BEDROCK`/`VERTEX`/`FOUNDRY`. Terminal output, transcript replies and audit rows have `sk-ant-` keys blanked (`redact.rs`) |
| `session.spawn` | mutation · always · session | `{ session, prompt?: string }` → `Session` — stores optional launch text, writes the inspectable brief and provider role instructions, starts the CLI with no positional argument, then types one private bootstrap/start turn when a task or launch prompt exists; `conflict`/`session.already_spawned` |
| `session.resume` | mutation · always | `{ session }` → `Session` — provider resume of a `restorable` session |
| `session.clear_restorable` | mutation · always | `{ session }` → `Session` — drop saved provider context and fresh-spawn the same restorable session/worktree |
| `session.park` | mutation · always | `{ session }` → `Session` — kill CLI, keep pane/scrollback/worktree/token |
| `session.wake` | mutation · always | `{ session }` → `Session` — respawn with provider resume |
| `session.close` | mutation · always | `{ session, remove_worktree?: bool = true, purge_build?: bool = true, discard_changes?: bool }` → `{ freed_mb }` — see SPEC §8 lifecycle. The branch stays unless branch cleanup finds its work already merged, after the close. A shared pooled review worktree is removed only when every other session on it is closed (`conflict`/`session.pair_live` otherwise, unless `remove_worktree: false`); a non-pooled checkout such as the primary is never removed, so others on it do not refuse the close. Removing a pooled worktree with uncommitted work (tracked edits or untracked files, or one git status cannot read) is refused `conflict`/`worktree.dirty` (`details.worktrees: [{ worktree, sessions, changed }]`) before the agent is stopped, unless `discard_changes: true` (RA-405) |
| `session.done` | mutation · always · agent | `{ session, summary?, sha?, status?: "completed"\|"blocked"\|"partial", blockers?: string[] }` → `Session` — self-report for `sessions.task_id`, the current task only. `completed` (default) moves that task `active → in_review`, promotes and prompts the next queued task, and notifies. `blocked`/`partial` leave only the current task in place, set it `blocked`, notify `agent_blocked`, and require `blockers` (D118). Reviewer: notifies only |
| `session.intent` | mutation · always · agent | `{ session, text }` → `Session` — one line, ≤200 chars, of what this session is doing; empty clears it. Surfaces in `session.peers` and the dashboard, where coordination happens |
| `session.claim` | mutation · always · agent | `{ paths: string[], symbol?, note?, exclusive? }` → `{ claimed, collisions: {path, symbol, session, since}[] }` — declare the files this session is taking on. Collisions are reported, not refused, so they can be negotiated over `mailbox.send`; `exclusive: true` refuses instead and records nothing. Advisory, not enforced (D116) |
| `session.release` | mutation · always · agent | `{ paths?: string[] }` → `{ released }` — drop claims; omit `paths` for all of them |
| `session.get` | query | `{ session }` → `Session` |
| `session.list` | query | `{ project_id?, state?: SessionState[], include_closed? }` → `{ sessions: Session[] }` — an agent sees its own project only; another `project_id` is `refused`/`actor.scope` (D106) |
| `session.peers` | query | `{ session }` or `{ project_id }` or `{}` → `{ peers: Peer[] }` — the live peer table (name, provider, branch, claimed files, task title, state). For an agent, neither target means "my project, minus me" |
| `session.brief` | query | `{ session }` → `{ text, compact, parts: {state, peers, adjacent, skills, plugins}: string }` — the knowledge injection, inspectable. `compact` is the half injected on every spawn: state, peers and adjacent work, naming the enabled skills and the folders they are registered in (bodies stay on disk) and the launch assignment omitted (D101, D147). Own session or PAIR partner only |
| `session.bootstrap` | query · agent | `{}` → `{ session, role, project_id, project, base_branch, worktree, branch, module?, task?, tasks: Task[], pair?, assignment?, brief_path, peers: Peer[], can_call: string[], discovery, comms, guardrails: {caps, denied_commands, write_roots, dry_run} }` — the one call every agent makes first, so it answers "who else is here and what may I do?" as well as "who am I" (D105). `discovery` points to `bus.ops`/`bus.schema` and their shell equivalents; `can_call` is the §9.1 verdict. `assignment` is the launch prompt on the session's first launch only: after a relaunch (fresh or resumed) it is absent, and the brief keeps the text labelled as the original launch assignment (RA-397). User/unbound calls are `invalid`/`bus.actor` |
| `session.update` | mutation · always · inverse · user | `{ session, branch?, model?, effort?, task_id?: Id\|null, module_id?: Id\|null, bus_writes?, allow_ui? }` → `Session` — `branch`/`model`/`effort` are `conflict`/`session.already_spawned` after first spawn; the rest any time. A branch rename is refused `conflict`/`session.branch_primary` on the project's primary checkout and `conflict`/`session.branch_shared` on a checkout another session shares (RA-402). Its undo records `expect: { fields, queued }` instead of `updated_at`, which every `session.report` moves: `audit.undo` is `audit.stale` only if one of the fields it wrote now holds something else, and it deletes the `(task, session)` queue rows the update added that are not yet completed (RA-403) |
| `session.report` | mutation · agent_only · session · agent | `{ session, kind: "session_start"\|"tool_use"\|"stop"\|"notification"\|"idle"\|"blocked", data?: object }` → `{}` — the provider's hooks calling home (§9.3); this is where `state: blocked`, `provider_ref` and "agent asking" notifications come from. Codex, having no hooks, degrades to PTY-derived idle/busy |
| `session.attach` | query · stream | `{ session, from_seq?: number }` → attaches `pty` stream (replays scrollback from `from_seq`) |
| `session.detach` | query | `{ session }` → `{}` |
| `session.input` | mutation · agent_only · session | `{ session, data: string, paste?: bool }` → `{}` — text to the PTY, as keys or as one paste |
| `session.resize` | mutation · agent_only · session | `{ session, cols, rows, until_detach?: bool }` → `{}` — `until_detach` borrows the size: on a connection door the PTY goes back to the size it had when that connection detaches from the session or closes, unless someone resized it since. A size set without it ends a borrow. The phone fits an agent's terminal to its screen this way |
| `session.scrollback` | query | `{ session, lines?: number }` → `{ text, epoch, seq, cols?, rows? }` — `cols`/`rows` are the live PTY's size, absent for a saved scrollback |
| `session.transcript` | query | `{ session, after?: number, limit?: number }` → `{ messages: {id, role, body, created_at}[], cursor, reset: bool, found: bool }` — the agent's conversation read from its provider's own log (Claude Code's `projects/<cwd>/<id>.jsonl`, Codex's rollout), in a thread message's shape: `user {text}`, `assistant {blocks: text \| tool_use {id, name, summary}}`, `tool {tool_use_id, is_error, text}` (cut to its head), `error`, `note`. `after` is a previous `cursor` (opaque): only what was written since; `reset` says the answer replaces everything read before. `found: false` until the provider's first hook names its conversation. The phone draws a terminal agent as a chat from it |
| `session.upload` | mutation | `{ session, bytes_b64 }` → `{ path, absolute, w, h }` — a PNG, JPEG, GIF or WebP picture (at most 4 MiB decoded) written into the session's checkout at `.relay/attachments/phone-<sha256 prefix>.<ext>`, which Relay keeps out of git in every checkout; the same picture twice is the same file. `path` is from the checkout, for a message to name: the phone attaches a photo to a message to the agent this way. User only (an agent is refused); a link where the folder should be is refused (`session.upload_path`) rather than written through; a session with no checkout is `session.upload_no_checkout`. Decoded and written with the store lock released (staged) |
| `session.restorable` | query | `{}` → `{ sessions: {session: Session, reason: "app_restart"\|"crash", worktree_dirty: bool}[] }` — the individual-resume list at launch |
| `session.discard_restorable` | mutation · always | `{ session, discard_changes?: bool }` → `{}` — declined at launch → cleaned. The last session on a pooled checkout deletes it, refused `conflict`/`worktree.dirty` as `session.close` is when it holds uncommitted work, unless `discard_changes: true` (RA-405) |
| `session.repo.add` | mutation · always · session | `{ session, url, name?, branch? }` → `LinkedRepo { name, path, url?, branch? }` — clones another repository into the session's own checkout at `.relay/linked/<name>` (excluded with the rest of `.relay/`, so the checkout's status never shows it), with the store unlocked and a 29-minute deadline. `url` is anything `git clone` takes, or `owner/name` for GitHub; a leading `-` is `invalid`/`session.repo_url`. `name` defaults to the URL's last part and is one plain folder name (`invalid`/`session.repo_name`); a name already linked is `conflict`/`session.repo_exists`; a failed clone `unavailable`/`session.repo_clone_failed`, leaving nothing behind. An agent links only to its own session; when the user links one, the agent gets a `system` mail naming the path. The folder is the record: nothing is stored. A brief built while one is linked carries `linked_repos:`. Emits `session.repo.changed { session, project_id }` |
| `session.repo.list` | query · session | `{ session }` → `{ repos: LinkedRepo[] }` — each with `unpushed: { changed, commits, branches, stashes, ignored }` when removing it would lose something: uncommitted or untracked files, commits no remote-tracking ref contains (as of its last fetch, so a stale one only overcounts), stashes, git-ignored keepers. A few bounded git calls per repository, store unlocked |
| `session.repo.remove` | mutation · always · user | `{ session, name, discard_changes?: bool }` → `{ freed_mb }` — deletes the clone. One holding what `unpushed` lists is refused `conflict`/`session.repo_unpushed` (`details: { name, path, unpushed }`) and kept, unless `discard_changes: true`. Every removal that deletes a checkout — `session.close`, `session.discard_restorable`, `worktree.remove`, `project.remove`, branch cleanup — checks its linked repositories the same way: `worktree.dirty` names them, `details.worktrees[].linked: [{ name, unpushed }]` |

### 10.9 knowledge injection

The brief is assembled by core at `session.spawn` / `session.wake` / `task.dispatch` and refreshed
before each provider launch. It comes in two halves, because delivering it whole meant delivering
27 KB of skill bodies around four lines of peer table (D101):

- **compact** — state (worktree, branch, project, module, task title+body+changelog), peer table
  (§10.8 `session.peers`), adjacent tasks (`task.list {module_id}` titles+states), and the
  comms hint. Notes are not part of it: an agent works from its task and reads a note
  (`notes.get`, `notes.standing`) only when it asks for one (D166). This is delivered on **every**
  spawn, appended to the provider-native role instruction — Claude's appended-system-prompt file
  or Codex `developer_instructions` — because a session spawned with no assignment is exactly the
  one that most needs to know who its peers are. It never carries the launch assignment: for
  Codex it lands in argv, and `session.bootstrap` already returns it privately. Each section has
  a budget (RA-385): peers, adjacent tasks and the skill list at most 40 lines / 8 KiB, each
  task body and changelog 4 KiB. A section that is cut ends with a line naming the op that
  returns the whole (`session.peers`, `task.list {module_id}`, `skill.list`, `task.get {task_id}`).
- **skills** — `skill.list {enabled: true}` bodies, written to `.relay/session-skills.md`. The
  compact half names each enabled skill and the folder it is materialized in
  (`.claude/skills/<name>/`, `.agents/skills/<name>/`, and the provider's own home, D147), so an
  agent knows a skill exists and can load it through its own skill mechanism instead of being
  handed a path to 27 KB of bodies it has no reason to open. Bodies remain the fallback for a
  provider with no skill mechanism of its own.

The full document, compact half plus the launch assignment, is written to the untracked worktree
file `.relay/sessions/<session>/session-brief.md` (`$RELAY_BRIEF`); `session.brief` returns `text`, `compact` and the
parts separately, for inspection. Relay still sends no positional prompt, and every role
instruction directs the first real turn to actor-bound `session.bootstrap` (§10.8), which returns
the assignment (first launch only) along with the peer table, the callable op list and the session's guardrails.
Provider-neutral Markdown; the same for both providers.

### 10.10 overlap

| op | attrs | payload → result |
|---|---|---|
| `overlap.list` | query | `{ project_id }` → `{ overlaps: Overlap[] }` — pairs of sessions sharing changed files/symbols |
| `overlap.flag` | mutation · always | `{ project_id, path, symbol?, note? }` → `Overlap` — an agent claiming "I am touching this" |
| `overlap.ack` | mutation · always | `{ overlap_id }` → `Overlap` |
| `overlap.scan` | mutation · always · project | `{ project_id }` → `{ overlaps: Overlap[] }` — force a scan (tests / reconcile) |

### 10.11 guardrail (SPEC §5)

| op | attrs | payload → result |
|---|---|---|
| `guardrail.gate` | mutation · always · session · agent | `{ session, kind: "write"\|"commit"\|"exec", path?, new_text?, diff?, command? }` → `{ verdict: "allow" }` — the enforcement door (§9.3); may create a hold. Refuse and hold arrive as typed `refused` / `held` errors, never as a verdict |
| `guardrail.holds.list` | query | `{ project_id?, session?, open_only? = true, limit? (200, ≤1000) }` → `{ holds: Hold[] }` — newest first; a string over 64 KiB in `details` is cut to its first 4 KiB. An agent sees its own project's only (another `project_id` is `actor.scope`) |
| `guardrail.hold.get` | query · user | `{ hold_id, full? }` → `{ hold, request, elided? }` — the frozen action without its auth; unless `full`, each string over 64 KiB in `request.payload` or `hold.details` is cut to its first 4 KiB and its JSON pointer listed in `elided`. `guardrail.confirm` replays the stored action whole |
| `guardrail.confirm` | mutation · always · user | `{ hold_id, scope? }` → `{ hold: Hold, outcome: Response }` (§9.4); for an exception request `scope` is `once`\|`session` and nothing is replayed (§9.5) |
| `guardrail.reject` | mutation · always · user | `{ hold_id, reason? }` → `{ hold: Hold }` |
| `guardrail.config.get` | query | `{ workspace_id? \| project_id? }` → `GuardrailConfig` (§9.6) |
| `guardrail.config.set` | mutation · always · inverse · user | `{ workspace_id? \| project_id?, patch: Partial<GuardrailConfig> }` → `GuardrailConfig` — patches that one layer's overrides |
| `guardrail.config.layers` | query | `{ workspace_id? \| project_id? }` → `{ scope, workspace_id?, project_id?, effective, inherited, overrides, sources }` (§9.6) |
| `guardrail.request` | mutation · always · session · agent (every role) | `{ session, kind, value, reason, scope? }` → `{ request: GuardrailException, created, wait_for }` (§9.5); `reason` at most 4 KiB, `value` at most 8 KiB; a `command` value with a bidi control, a zero-width or other format character (Unicode Cf) or a control character other than newline is `invalid`/`guardrail.request`, since the person approving must read exactly the bytes granted |
| `guardrail.request.get` | query | `{ request_id }` → `GuardrailException` |
| `guardrail.requests.list` | query | `{ project_id?, session?, state?: "open"\|"active"\|"all" }` → `{ requests: GuardrailException[] }`; an agent sees its own project's only, as for `guardrail.holds.list` |
| `guardrail.grant.revoke` | mutation · always · user | `{ request_id }` → `GuardrailException` |
| `guardrail.explain` | query | `{ project_id, paths?, lines?, commands? }` → `{ verdict, paths: Item[], commands: Item[], files, lines, caps, over_caps, write_roots }` where `Item = {subject, verdict, policy?, message?}` — preflight a whole plan before the first action (D117). Pure: holds nothing, writes nothing |
| `guardrail.check` | query | `{ project_id, kind: "write"\|"commit"\|"exec", path?, new_text?, diff?, command? }` → `{ verdict: "allow"\|"refuse"\|"hold", error?: BusError }` — pure dry run of `gate`: nothing created, nothing audited |

### 10.12 worktree / git / integration (SPEC §8)

| op | attrs | payload → result |
|---|---|---|
| `worktree.list` | query | `{ project_id, include_dirty? = true }` → `{ worktrees: Worktree[] }` — hot UI refreshes may skip the per-worktree status scan and project the selected worktree's dirty state from `git.status` |
| `worktree.create` | mutation · always | `{ project_id, branch, from?: string }` → `Worktree` |
| `worktree.remove` | mutation · always | `{ project_id, path, purge_build? }` → `{ freed_mb }` — `conflict` if a session owns it |
| `worktree.disk` | query | `{ project_id }` → `{ worktrees: {path, disk_mb, build_mb}[] }` |
| `worktree.drift` | query | `{ project_id }` → `{ base, head, worktrees: {path, branch, behind?, ahead?}[] }` — how far each worktree's HEAD is behind the freshest base: the base's remote-tracking branch, or the local base when it contains it. Cached refs only, never the network; `git.fetch` refreshes them |
| `git.status` | query | `{ project_id, worktree? }` → `{ branch, upstream?, ahead, behind, files: FileStatus[] }` |
|  |  | **`worktree` throughout `git.*` and `file.*`:** omitted, it is the caller's own session worktree for an agent and the project root for the user. `"@project"` asks for the project root explicitly. Defaulting an agent to the project root returned confident, well-formed, wrong answers with no error either way (D111) |
| `git.diff` | query | `{ project_id, worktree?, base?, staged? }` → `{ files: DiffFile[] }` |
| `git.diff.file` | query | `{ project_id, worktree?, path, base? }` → `{ old, new, hunks }` (for `@codemirror/merge`); refuses a binary file (`git.diff_binary`) or one whose old + new text passes 1 MiB (`git.diff_too_large`) before building the reply |
| `git.log` | query | `{ project_id, worktree?, branch?, limit? = 200 }` → `{ commits: Commit[] }` |
| `git.show` | query | `{ project_id, sha }` → `{ commit: Commit, files: DiffFile[] }` |
| `git.branches` | query | `{ project_id, worktree? }` → `{ current, branches: Branch[] }` (with merged flag and session owner; `current` follows the selected worktree) |
| `git.branch.create` | mutation · always · user | `{ project_id, worktree?, name, start_point?, checkout? = true }` → `{ name, head, worktree }` — validates with Git, conflicts on an existing branch, and creates only through the selected worktree |
| `git.branch.delete` | mutation · always · user | `{ project_id, name }` → `{}` — deletes one merged local branch; refuses the base branch, unmerged work, session-owned branches, and branches checked out in any worktree; remote refs are untouched |
| `git.stage` / `git.unstage` | mutation · agent_only | `{ project_id, worktree?, paths: string[] }` → `{}` |
| `git.commit` | mutation · always | `{ project_id, worktree?, message, all?: bool }` → `{ sha }` — caps and protected paths apply |
| `git.fetch` | mutation · agent_only | `{ project_id }` → `{ ahead, behind }` |
| `git.push` | mutation · always | `{ project_id, worktree?, set_upstream? = auto }` → `{}` — a branch without an upstream is first-pushed as `git push -u origin <branch>`; explicit `false` keeps plain-push behavior |
| `git.pr.list` | query | `{ project_id, refresh? }` → `{ pull_requests: { number, branch, draft, url, title }[] }` — GitHub PRs reported by the authenticated `gh` CLI: every open one, and the 100 most recently updated closed or merged ones (`complete: false` when that page failed). One listing, or one failure, answers for a minute per repository (D130: redraws must not repeat the network request); `refresh` asks GitHub now, and `git.push` / `git.pr.open` drop the cached one |
| `git.pr.open` | mutation · always | `{ project_id, worktree?, title?, body? }` → `{ url }` — `gh pr create` runs before the store lock, with a 25 s deadline; `git.pr_timeout` means the outcome is unknown |
| `git.branch.clean_merged` | mutation · always · user | `{ project_id, dry_run? }` → `{ deleted: string[], failed: {branch, reason}[] }` — an alias of `git.branch.cleanup` (same rules, same staged pass), answering with the branches deleted, or on a dry run that would be, and in `failed` the merged ones it could not delete (a dirty or held checkout, a rebase in progress, a failed delete) with why; unmerged work is in neither. It used to delete any merged local branch by rules of its own, which had drifted from cleanup's |
| `git.branch.cleanup` | mutation · always · user | `{ project_id, dry_run? }` → `{ branches: {branch, session?, outcome, reason, pr?, removed_worktree, deleted_remote, merged}[] }` — closed sessions' `relay/*` branches whose work is merged (an ancestor of the base, every commit already upstream by patch id, or a merged GitHub PR containing the tip) are deleted; anything else is `kept` with the reason (`merged` tells a merged branch that could not go from unmerged work). A branch checked out in the primary, by an open session or outside the pool, or being rebased or bisected, stays; a clean, unowned pooled checkout holding it is removed first. The remote branch is deleted only for a merged PR, leased on the sha just seen. The same pass runs after `session.close`, when `git.pr.list` sees a closed session's PR merged, and as a background sweep 90 s after start and every 20 min (a kept, unchanged branch is looked at again ever less often, up to weekly). Each deletion writes a `system` audit row and emits `git.changed` |
| `git.suggest_message` | query | `{ project_id, worktree? }` → `{ message }` — heuristic subject from the diff |
| `integration.request` | mutation · always | `{ project_id, sessions: string[] \| branches: string[], build?: bool = true, deploy?: DeviceRef }` → `Integration` (state `queued`; results via `integration.result` events). An agent is held to its own project and refused `deploy`; an agent's request that builds (the project's `build_cmd`, run outside any sandbox) is `held` / `integration.agent_build` for a person to confirm unless the project's `guardrails.agent_builds` is on, while a merge-only request (`build: false`) goes straight through |
| `integration.get` / `integration.list` | query | `{ integration_id }` / `{ project_id }` |
| `integration.discard` | mutation · always | `{ integration_id }` → `{}` — removes the throwaway worktree |

### 10.13 file (SPEC §8)

All paths are relative to the worktree root; `..` and absolute paths are `invalid` /
`file.path`. An omitted `worktree` is the caller's own checkout: for an agent its session's
worktree, for the user the project's primary checkout; `"@project"` names the primary checkout
explicitly (D111). A `worktree` must be the primary or a linked checkout whose slot points back
at it; one moved by hand without `git worktree move` is refused until `git worktree repair`.

| op | attrs | payload → result |
|---|---|---|
| `file.tree` | query | `{ project_id, worktree?, path? = "", depth? = 1, git_badges? = true, limit? (2000, ≤5000) }` → `{ entries: Entry[], truncated?: {path: total} }` (optional git badges; omits VCS metadata and high-churn build/cache directories). Each directory lists at most `limit` entries, folders first; one cut short is named in `truncated` with its full count |
| `file.read` | query | `{ project_id, worktree?, path, max_bytes? }` → `{ text?, bytes_b64?, mime, size, truncated }` |
| `file.write` | mutation · always · project | `{ project_id, worktree?, path, text }` → `{ bytes, removed_lines, added_lines }` — guardrails §9.2 |
| `file.create` | mutation · always | `{ project_id, worktree?, path, kind: "file"\|"dir", text? }` → `Entry` |
| `file.rename` | mutation · always | `{ project_id, worktree?, path, new_name }` → `Entry` |
| `file.move` | mutation · always | `{ project_id, worktree?, path, into: string }` → `Entry` |
| `file.delete` | mutation · always · inverse (restore) | `{ project_id, worktree?, path }` → `{ trash_id }` |
| `file.restore` | mutation · always | `{ project_id, trash_id, worktree? }` → `Entry & { worktree, fallback }` — puts it back into `worktree` (`@project` or a worktree path, an agent confined to its own checkout), by default the checkout it was deleted from; once that checkout is gone, the project's primary checkout, with `fallback: true`. Never overwrites an existing path (`file.restore_conflict`); bytes no longer on disk are `file.trash_unavailable` |
| `file.trash.list` | query · user | `{ project_id, limit? (200, ≤1000) }` → `{ entries: {id, original_path, worktree, created_at, available}[] }` — the project's trashed files not yet restored or expired, newest first; `id` is the `trash_id` for `file.restore`, `available` whether the bytes are still on disk |
| `file.import` | mutation · always | `{ project_id, worktree?, into, sources: path[] }` → `{ entries: Entry[] }` — OS drag-in |
| `file.restore_head` | mutation · always · user | `{ project_id, worktree?, path }` → `Entry` — `git checkout -- <path>`: puts a tracked file back as `HEAD` has it. Nothing calls it for you: there is no post-hoc write watcher (§9.3, D163) |
| `file.search` | query | `{ project_id, worktree?, query, glob?, regex?, limit? }` → `{ hits: {path, line, col, text, text_offset?}[] }`; `col` is the match's 1-based byte offset in the line, and `text` the line, or for one over 240 bytes a window of it around the match starting at byte `text_offset`; searches regular text files only (a symlink only when it stays inside the worktree), skipping generated trees, files over 8 MiB and any with a NUL in the first 8 KiB |

### 10.14 device (SPEC §9)

| op | attrs | payload → result |
|---|---|---|
| `device.list` | query | `{}` → `{ devices: Device[] }` (adb + AVDs later) |
| `device.watch` | mutation · never · user | `{ on: bool }` → `{}`; while device control is visible, `adb track-devices` emits `device.changed`; off tears it down |
| `device.mirror.start` | mutation · always | `{ device, max_size?, bitrate? }` → `{ mirror_id, width, height }` + `mirror` stream; the scrcpy server jar is taken from `$RELAY_SCRCPY_SERVER`, else `~/.local/share/relay-v4/scrcpy-server-v4.1`, else the checkout the engine was built from, and checked against its pinned SHA-256 before every push (`device.mirror_server_missing`, `device.mirror_server_mismatch`). `bitrate` (and the size ladder's own rate) is held to 8–50 Mbps; see *Mirror launch* below |
| `device.mirror.stop` | mutation · always | `{ mirror_id }` → `{}` |
| `device.mirror.input` | mutation · never | `{ mirror_id, event }` → `{}`; `event` is `{type: "tap", x, y}`, `{type: "swipe", x1, y1, x2, y2}` (stream coordinates) or one of scrcpy's input messages (`touch`, `scroll`, `key`, `keypress`, `text`, `setclipboard`, `back`, `home`, …). A `swipe` is instant — DOWN, one MOVE, UP in one write, so it lands as a fling — and refuses any other field (`duration_ms`); send `touch` events for a timed drag. `setclipboard` text is at most 262,130 bytes (one message, never split); longer is `device.input` — use `text`, which is split |
| `device.run` | mutation · always | `{ project_id, worktree?, device, variant?, integration_id? }` → `Run` (state `building`) + `logcat` stream; crashes surface as `run.crash` events |
| `device.build` | mutation · always | `{ project_id, worktree?, variant?, format?, publish?, integration_id? }` → `Run` (state `building`, `kind: "build"`, no device) + `logcat` stream; `variant` defaults to `release`, `format` is `apk` (default) or `bundle`, and `publish` runs Gradle Play Publisher's `publish<Variant><Format>` to upload to Google Play. A saved Relay profile overrides release signing for this invocation; otherwise Gradle's project configuration signs as before. Play credentials stay in the target project. Build rows persist `variant`, `format`, and `publish`; a finished build adds `artifact` plus `signing: signed | unsigned | unverified` after checking the APK with SDK `apksigner` or the AAB with JDK `jarsigner` |
| `device.signing.get` | query · user | `{ project_id }` → `{ configured, enabled, key_alias?, keystore? }`; returns metadata only, never a password |
| `device.signing.create` | mutation · never · user | `{ project_id, key_alias, password }` → `{ configured, enabled, key_alias, keystore }`; creates one PKCS12 upload key below Relay's private data directory and saves the password in Linux Secret Service. The op is deliberately unaudited and `user_only`, so its secret never reaches the audit log or an agent. It travels the socket like every op (the native client calls it there); from the CLI, pass the payload on stdin (`relay cmd device.signing.create -`) rather than as an argument |
| `device.signing.set_enabled` | mutation · user | `{ project_id, enabled }` → signing profile metadata; explicitly switches release builds between the saved Relay key and the Android project's Gradle signing configuration without deleting either identity |
| `device.run.stop` | mutation · always | `{ run_id }` → `{}`; also stops a build |
| `device.run.list` | query | `{ project_id }` → `{ runs: Run[] }` |
| `avd.list` | query | `{}` → `{ avds: Avd[] }`; includes a running emulator serial when available |
| `avd.catalog` | query | `{}` → `{ system_images, devices }`; installed SDK choices only |
| `avd.create` | mutation · always | `{ name, package, device? }` → `Avd` |
| `avd.boot` | mutation · always | `{ name, cold? }` → `{}`; boots headless (`-no-window`) — Relay's mirror is its screen. The emulator then appears through `device.list` and uses the normal deploy pipeline. An AVD adb already lists as running is refused `conflict`/`avd.already_running`. `avd.changed {name, state, message?}` follows it: `booting`, then `started`, or `failed` / `stopped` with the emulator's `message` when it exits early (RA-349) |
| `avd.stop` | mutation · always | `{ name }` → `{}`; `adb emu kill` on the running AVD (`avd.not_running` otherwise), since a headless emulator has no window to close |

**Mirror launch.** Each mirror start reads the device's display once (`adb shell dumpsys
display`, alongside the jar push) and boots the scrcpy server with `max_fps` at the panel's top
refresh rate — the highest of display 0's modes, rounded, at least 60 and at most 144; 60 when
the output says nothing usable. A 120 Hz phone capped at 60 showed every other frame and added
~4 ms on average. `video_bit_rate` follows the capture-size ladder (8 Mbps at 1024, up to 20 Mbps)
and never goes under 8 Mbps: at 4 Mbps busy content was measured arriving 0.7–1.3 s late, the
device queueing frames it could not encode in budget. The whole launch line is logged at INFO
(`mirror server launch`). Settings, all read at each start:

| setting | default | effect |
|---|---|---|
| `device.mirror.max_fps` | `null` | a fixed cap (1–144) instead of the device's refresh rate |
| `device.mirror.video_codec_options` | `null` | extra encoder keys passed as scrcpy's `video_codec_options`, `key[:type]=value` comma-separated (`type` is `int`, `long`, `float` or `string`), merged over Relay's own (none today; a key given here wins). Meant for vendor low-latency keys such as `vendor.qti-ext-enc-low-latency.enable:int=1` (Qualcomm); unverified on real hardware, and a key the encoder does not know is ignored by it. Keys and values are limited to letters, digits and `._-` (`+` in values) because the line goes through the device's shell; a bad entry drops the whole value with a warning, and the mirror starts without it |
| `device.mirror.pin_refresh_rate` | `false` | while a mirror runs, `settings put system min_refresh_rate <top rate>` so an adaptive (LTPO / Samsung "Adaptive") panel does not idle at a low rate and delay the first frame after a tap. The previous value is written to `mirror-refresh` beside the store *before* anything changes, and put back (`settings delete` when it was unset) when the device's last mirror ends, when the engine shuts down, at the next engine start, or at the next mirror start on that device — whichever first finds it connected. With no record file (an in-memory store) or a previous value that cannot be written back verbatim, nothing is pinned. `peak_refresh_rate` is never touched. A device that never comes back while Relay runs keeps the pin until it does: `adb shell settings delete system min_refresh_rate` (or the value the file names) undoes it by hand |

**Mirror trace.** `RELAY_MIRROR_TRACE=1` (read once at first use; off costs one load per
packet) logs each video packet at the `mirror_trace` tracing target, INFO, so the subscriber
must let it through (`RUST_LOG=mirror_trace=info` for the native client, `RELAY_LOG` for
`relay serve`). Stamps are `CLOCK_MONOTONIC` nanoseconds (`mono_ns`); `seq` is the packet's
frame `seq`, the key that joins the engine's lines to the client's:
`stage="read"` (worker, payload read off the scrcpy socket; also `header_ns`, `pts_us` — the
device's presentation time — `len`, `key`, `config`), `stage="serialize"` (socket door, frame
encoded; `recv_ns` when it left the broadcast queue, `bytes`), `stage="flush"` (socket door, the
write holding it returned; `batch_bytes`).

### 10.15 provider / usage / skill / plugin

| op | attrs | payload → result |
|---|---|---|
| `provider.list` | query | `{}` → `{ providers: ProviderInfo[] }` — installed, path, version, auth (`signed_in_as?`), spawn profile |
| `provider.refresh` | mutation · never | `{}` → `{ providers: ProviderInfo[] }` — re-detect now |
| `usage.get` | query | `{ provider?: Provider }` → `{ usage: Usage[] }` — per provider, in its own units and windows; combines stored agent reports with bounded read-only CLI state inspection and never calls a provider endpoint. Where a provider left state to read (the Claude status-line file, Codex's newest rollout), it wins over a stored report whatever their ages |
| `usage.report` | mutation · never · session · agent | `{ session, provider, payload: object }` → `{}` — metering an agent reports for its own session; core stores the latest per session as `usage.get`'s fallback. No hook sends it: the Claude status line writes a file instead (D64) |
| `skill.list` | query | `{ project_id?, enabled?, summary? }` → `{ skills: Skill[] }`; with `summary: true` each `body` is cut to its first 4 KiB (the frontmatter and opening), so a large library fits one reply; `description` is read before the cut |
| `skill.get` | query | `{ skill_id }` → `Skill` with its whole body; `skill.not_found` |
| `skill.create` / `skill.update` / `skill.delete` | mutation · always · inverse | `{ name, body }` / `{ skill_id, name?, body? }` / `{ skill_id }`; a skill nobody has enabled anywhere is enabled in every project on create/install (D147) |
| `skill.enable` | mutation · always · inverse | `{ skill_id, project_id, enabled: bool }` → `Skill`; the per-project override on top of that app-wide default |
| `skill.install` | mutation · always | `{ url, subdir?, replace_skill_id? }` → `{ skills: Skill[] }` (bodies cut as in `skill.list {summary: true}`); clones a GitHub source, finds bounded `SKILL.md` files, prefers canonical `skills/` then Relay-native `.agents/skills/` entries over provider adapters, collapses byte-identical copies with the same name, and installs or refreshes them with source revision metadata. The whole skill folder is kept (`<store dir>/skills/<id>/`), not only the `SKILL.md` body, and is materialized into every project root and session worktree as `.claude/skills/<name>/` and `.agents/skills/<name>/`, and into the provider homes `~/.claude/skills/` and `$CODEX_HOME/skills/` — Codex reads skills from nowhere else. A folder the repository or the user wrote themselves is never overwritten (D147). Same-rank different bodies return `skill.duplicate_name`. A hidden deleted name is reclaimed automatically; a visible different-source collision returns `skill.name_exists` with both identities and requires the exact conflicting id for atomic replacement |
| `github.status` | query | `{}` → `{ installed, connected, login? }` |
| `github.connect` | mutation · never | `{}` → `{ started }`; launches GitHub CLI browser auth and emits `github.changed` when it finishes; Relay stores no token |
| `github.repo.list` | query | `{}` → `{ repositories: GitHubRepo[] }`; all repositories the connected account can access |
| `plugin.list` | query | `{ project_id? }` → `{ plugins: Plugin[] }` — the plugins this build bundles (D159) with their skills, MCP servers and docs, `enabled_in` project ids, and `suggested_for` (projects whose root matches the manifest's `detect` suffixes, e.g. a `.uproject`; limited to `project_id` when given). Runs with the store lock released |
| `plugin.get` | query | `{ plugin_id, skill? }` → `{ plugin, instructions, docs: PluginDoc[], skill?: PluginDoc }` — the always-on agent rules, every documentation file, and optionally one skill's `SKILL.md`; `plugin.not_found`, `plugin.skill_not_found` |
| `plugin.enable` | mutation · always · inverse · user | `{ plugin_id, project_id, enabled: bool }` → `Plugin`; emits `plugin.changed`. Off by default. When on, every agent of the project gets the plugin's skills as folders (materialized at once into the root and live worktrees, like installed skills; an installed skill with the same folder name wins), its instructions and skill list under "Enabled plugins" in the injected brief, and its MCP servers in `.relay/relay.mcp.json` (Claude) or as `--config mcp_servers.<name>.*` (Codex) on each start and resume |

### 10.16 notify / settings

| op | attrs | payload → result |
|---|---|---|
| `notify.list` | query | `{ project_id?, unread_only?, category?, limit? (100, ≤500), count_only? }` → `{ notifications: Notification[], unread: number }` — `unread` counts every unread notification under `project_id` / `category`, whatever `limit` and `unread_only` leave out of the page; `count_only` returns it with no rows (`notifications: []`), for a badge |
| `dashboard.get` | query · global | `{}` → `{ projects: DashboardProject[], sessions_live: Peer[], in_review: Task[], holds_open: Hold[] (newest 100), notifications: Notification[], resources: {…as app.resources.get} }` — project workload counts, decisions, activity, and resource pulse in one round trip |
| `notify.ack` / `notify.ack_all` | mutation · never | `{ notification_id }` / `{ category? }` → `{}` |
| `notify.settings.get` / `notify.settings.set` | query / mutation · always · inverse | `{}` → `NotifySettings` / `{ patch }` → `NotifySettings` |
| `settings.get` | query | `{ path?: string }` → `{ value }` — dotted path into the settings tree, whole tree if absent |
| `settings.set` | mutation · always · inverse | `{ path, value }` → `{ value }` |
| `settings.reset` | mutation · always · inverse | `{ path? }` → `{ value }` |

Settings tree top-level keys (each documented where its feature lands): `appearance`,
`notifications`, `providers`, `guardrails` (its `roles` are the §9.1 allow-sets), `undo`,
`audit`, `usage`, `device`, `layout` (`layout.current.<project>`), `keybindings` (shortcut → op
envelope, SPEC §2). Unread defaults were dropped (RA-253): `parking`, `theme` and a top-level
`roles` are no longer part of the tree, though an arbitrary path can still be written.
`guardrails` is the one subtree with a fixed shape: `settings.set` under it refuses a key the
guardrail config does not know and reads the touched layers back the way `guardrail.config.set`
does (`invalid`/`guardrail.config`); a stored key this build does not know is ignored with a
warning rather than failing every guardrail read.

### 10.17 ui (core-held shell model — §6.5)

| op | attrs | payload → result |
|---|---|---|
| `ui.state` | query | `{}` → `{ project_id, page, panes: PaneInfo[], focused: PaneRef, windows: WindowInfo[] }` — the engine's model, not the native window's panes |
| `ui.page.switch` | mutation · agent_only · inverse | `{ page: "agents"\|"code"\|"board"\|"modules"\|"dashboard"\|"skills"\|"plugins"\|"settings", project_id? }` → `{}` |
| `ui.pane.open` | mutation · agent_only | `{ kind: PaneKind, target?: PaneTarget }` → `{ pane: PaneRef }` — records the pane; the native client acts only on `target.session` (focuses that terminal), so e.g. `{kind:"diff", target:{sha}}` opens nothing on screen |
| `ui.pane.close` / `ui.pane.focus` | mutation · agent_only | `{ pane: PaneRef }` → `{}` |
| `ui.pane.move` | mutation · agent_only | `{ pane: PaneRef, to: PaneRef, edge: "top"\|"bottom"\|"left"\|"right"\|"center" }` → `{}` |
| `ui.layout.list` / `ui.layout.save` / `ui.layout.apply` / `ui.layout.delete` | mutation · always · inverse (save/delete) | `{ project_id }` / `{ project_id, name, state? }` / `{ project_id, name }` / `{ project_id, name }` — opaque shell state is stored in core and an apply emits `layout.changed` for the UI. A save without `state` stores the arrangement the native client last saved (`native.layout.current.<id>`), then the older `layout.current.<id>`, then a plain grid (RA-419, D150) |
| `ui.window.popout` / `ui.window.close` | mutation · agent_only | `{ pane: PaneRef }` → `{ window_id }` / `{ window_id }` → `{}` — model only: no OS window opens or closes |
| `ui.window.list` | query | `{}` → `{ windows: WindowInfo[] }` — the model's windows |
| `ui.toast` | mutation · agent_only | `{ text, level?: "info"\|"warn"\|"error", ttl_ms? }` → `{}` — emits `ui.toast`; the native client shows the text in its notice bar |
| `os.reveal` | mutation · never | `{ path }` → `{}` — file manager, via `xdg-open` on the engine's machine |
| `os.open_url` | mutation · never | `{ url }` → `{}` — `http(s)` only, via `xdg-open` on the engine's machine |

### 10.18 design

A project's design canvases: pages of frames, shapes, text, stickies, pen paths and pictures, each
design optionally tied to one terminal (`session`). The node model, its validation and limits are
`relay_bus::design` (`DesignNode`, `DesignNodeOp` in the schema). Kinds: page, frame, group, rect, ellipse, polygon (`sides`, `star`), line (`points`, or attached `from`/`to` with a `route`), text, sticky, path, image, icon (one of `ICONS`); any node may be turned (`rotation`), shapes take `gradient`, `radii`, `dash`, `blur` and a centred `text` label. Geometry is px, `x`/`y` relative
to the parent; siblings paint in order, the last on top.

| op | attrs | payload → result |
|---|---|---|
| `design.list` | query | `{ project_id, session?, include_deleted? }` → `{ designs: Design[] }` — newest change first |
| `design.get` | query | `{ design_id, page?, pages_only?, nodes?, since_rev? }` → `{ design, nodes: DesignNode[], assets_dir, colors, styles, credits?, removed?, indexes?, left? }` — pages in their order, then each page's nodes in pre-order, siblings bottom to top; `page` (an id, or a name no other page has) keeps one page and what is on it (`not_found` / `design.page_not_found`); `pages_only` answers the pages without what is on them (with `page`, that one) and is refused with `nodes` or `since_rev` (`design.pages_only`); `nodes` (1 to 500 ids) answers those nodes and what is inside them, an unknown id is `not_found` / `design.node_not_found` naming the nearest id it holds and the rev it was removed at, when on record; `since_rev` answers what changed since that rev (below); `colors` and `styles` are always whole; `credits` is the credit of each picture added with `design.picture.add`, by `src` (at most 128, and 256 KiB). A deleted design still reads |
| `design.create` | mutation · always · inverse | `{ project_id, title? = "Untitled design", session? }` → `DesignGetOut` — one empty page, `page-1`. An agent's design is its own session's (layer 2 refuses another name); the person may name any session of the project (`invalid` / `design.session`) or none. Titles are 1–200 characters (`design.title`) |
| `design.update` | mutation · always · inverse | `{ design_id, title?, session?: string \| null }` → `Design` — bumps `rev` |
| `design.duplicate` | mutation · always · inverse | `{ design_id, title? = "<title> copy" }` → `Design` — every node, with its id, and the id counter |
| `design.delete` / `design.restore` | mutation · always · inverse | `{ design_id }` → `{}` / `Design` — soft (§5.4): the retention pass purges a deleted design past `undo.grace_days`. Any change to a deleted design is `conflict` / `design.deleted` |
| `design.purge` | mutation · always · user_only | `{ design_id }` → `{}` — a deleted one only (`conflict` / `design.not_deleted`), nodes and all; emits `design.deleted` |
| `design.apply` | mutation · always · inverse | `{ design_id, ops: DesignNodeOp[], base_rev?, lenient?, reply?, expect? }` → `ApplyOut`: short (`rev`, `prev_rev`, `ids`, `removed_count`, `audit_id`, `skipped`, `warnings`, `copied`, `rebased`), or with `reply: "full"` also `design`, `nodes`, `removed`, `inverse`, `expect`, `colors`, `styles`, `reread` — below |
| `design.asset.add` | mutation · always | `{ design_id, path? \| bytes_b64? }` → `{ src, path, w, h }` — a PNG, JPEG, GIF, WebP or SVG, by its bytes (`invalid` / `design.asset` for anything else); an absolute `path` up to 20 MiB or `bytes_b64` up to 1 MiB. Stored once by content in `design-assets/` beside the store as `<32 hex of SHA-256>.<ext>`, the `src` an image node names; `w`/`h` its natural size (an SVG's `width`/`height` in px, else its `viewBox`, else 300×150). Staged: the file work runs unlocked. Emits no event: it changes no row a query shows (§3.3) |
| `design.find` | query | `{ design_id, page?, inside?, kind?, name?, text?, fill?, stroke?, color?, any_color?, font?, with_nodes? }` → `{ ids, nodes?, cut? }` — the nodes that match every filter given, in paint order (`ids`); `kind` is any of its kinds, `name` and `text` a case-insensitive word match, `font` a family, and colours compare as drawn (a `$name` or a style counts). `inside` (not the node itself) is `not_found` / `design.node_not_found` when unknown. `with_nodes` adds the first 200 matches as stored (512 KiB at most; `cut` when more) |
| `design.fonts` | query · global | `{ name? }` → `{ families, complete }` — the families the newest connected desktop app can draw a design's text in, sorted, `name` a case-insensitive word match. With no app reporting, Geist, Geist Mono and Sora with `complete: false` |
| `design.asset.get` | query · user | `{ design_id, src, offset? }` → `{ media_type, size, bytes_b64, next }` — an image node's picture, in pieces of at most 3 MiB decoded from `offset`; `next` is where the next piece starts, null on the last |
| `design.render` | query · socket only | `{ design_id, page?, nodes?, png? = true, scale? = 1, max_side? = 1568, boxes_limit? = 200, base_rev?, timeout_ms? = 20000, at_ms?, frames? }` → `{ design_id, rev, page, area, scale, png?, issues, issue_count, warnings, boxes, timings_ms, times_ms? }` — the desktop app draws a page, or the outermost of up to 200 `nodes` on one page, and answers a PNG written beside the store (`png.path`, `w`, `h`; shown to an MCP caller as an image) and every visible node's laid-out box in page px, hugged text measured: `boxes.fields` is always `id, kind, parent, x, y, w, h, rx, ry, rot`, `rows` at most `boxes_limit` (0–500) of them, `total` and `hidden` counting the rest. `issues` (worst first, at most 100; `issue_count` counts all) are `overflow`, `overlap`, `text_overlap`, `clipped` and `contrast`, each with the ids it concerns. `scale` is 0.25–2 and is lowered to fit `max_side` (64–2560) and 4 megapixels; `area` is the page rectangle pictured. `at_ms` (0–600000) poses each node at that time of its motion, `frames` (2–24, not with `at_ms`) draws a contact sheet of that many frames over the longest timeline in scope, their times in `times_ms`; boxes and issues stay at rest. `base_rev` refuses `conflict` / `design.stale` unless the design is still at that rev. An agent may picture only its own project's designs. Waits for the app with no lock held; see §6.2 for the lease, the codes and the limits |
| `design.render.serve` | query · global · socket only · user_only | `{ fonts? }` → `{}` — the desktop app offers to draw on this connection until it closes (§6.2) |
| `design.render.answer` | query · global · user_only | `{ render_id, drawn? \| error? }` → `{ accepted }` — the app's answer to one `design.render.request`, exactly one of `drawn` and `error`; `accepted` is whether a render still waited for it. Unlocked: it decodes and checks the picture without the store |
| `design.picture.search` | query · plugin `pictures` | `{ design_id, query, source?, orientation?, page?, count? }` → `{ source, page, has_more, results }` — stock photos from Openverse (no key) or Pexels (the person's key); a hit's `id` goes to `thumb` and `add`. Agents may call it only in a project where the plugin is on |
| `design.picture.thumb` | query · plugin `pictures` | `{ design_id, id }` → `{ path, w, h }` — a search result's small preview, to look at before choosing |
| `design.picture.add` | mutation · always · plugin `pictures` | `{ design_id, id }` → `{ src, path, w, h, credit }` — stores the photo for the design with its credit beside it (`<stem>.credit.json`; the first credit for those bytes is kept); answers the `src` an image node names, as `design.asset.add` does. Emits no event, as `design.asset.add` |
| `design.picture.key_set` | mutation · never · user_only | `{ key }` → `{ saved }` — checks a Pexels API key with Pexels and keeps it in the system keyring; never audited, since the payload is the key |
| `design.picture.key_remove` | mutation · always · user_only | `{}` → `{ saved }` — forgets the saved Pexels key |

`design.apply` runs 1–2,000 `add` / `update` / `move` / `delete` / `copy` / `color` / `style` /
`delete_style` edits in order inside the request's transaction, all or none, then bumps `rev`. A
person's lenient list (an undo or a redo) may hold up to 7,000 edits; an agent's lists, lenient or
not, hold at most 2,000. An `add` without an `id` gets `{kind}-{n}`; `index` is a place among the
parent's children, 0 at the bottom, absent on top. A refusal names the edit and the field —
`ops[3] update "hero": fill: "red" is not #RGB…`, `details.op` — as `design.node_invalid`,
`design.node_not_found`, `design.node_exists`, `design.parent` (gone, or not a page, frame or group;
a page sits in nothing), `design.cycle`, `design.last_page`, `design.too_large` (5,000 nodes, 1.5 MiB
of node JSON), `design.style_invalid`, `design.style_exists` or `design.style_not_found` (a node, run
or style naming a colour or style the design lacks), `design.position` (a `keep_position` move where
either place is laid out by auto layout), or `design.node_unreadable` (an edit naming a node of a
kind this build cannot read).

`copy` takes `{id, as?, parent?, index?, dx?, dy?}` and copies a node and everything inside it, as
one edit (a page copies as a page). Each node inside the copy takes an id from the copied id: one
starting with the copied id and `-`, `_`, `.` or `:` has that start swapped for the copy's
(`card-title` becomes `card-2-title`); any other gets the copy's id and `:` in front (`text-14`
becomes `card-2:text-14`). A connector inside joins the copies of its ends; one attached outside the
copy lets go of that end where the stored boxes draw it. `color` `{name, value}` names a colour that
a colour field takes as `$name`. `style` `{name, set}` names a style or changes one. `delete_style`
`{name}` takes one out, and what used it keeps its look: the colour, or the style's fields a node did
not set itself, are written into it. A `move` with `keep_position: true` keeps the node where it is
on its page.

`base_rev` is the rev the edits were made against. A list is refused `conflict` / `design.stale`
(`details.rev`, `details.base_rev`, `details.conflicts`, each `changed`, `missing` or `inside`) only
when a node it writes, moves or deletes, or one inside a node it deletes, changed after that rev;
otherwise it applies on top, and the reply says `rebased: true`. `expect` says what the nodes must
still hold: a strict list is refused `conflict` / `design.stale` naming the fields that moved, and a
lenient list drops those fields or skips the edit.

`lenient` replays an undo. An edit whose node or parent is gone, whose node this build cannot read,
or that would overwrite a change made since (`expect`) is skipped rather than refusing the list, and
what was stored goes back even where a new edit's rules would refuse it. Each skip is in `skipped`
with its `reason`: `missing` (and the `rev` it went at), `no_parent`, `parent_skipped`, `fields` (and
the fields that moved), `parent`, `inside` (and the `ids` added inside it), `exists`, `changed`,
`too_deep` or `unreadable`. At most 100 are listed, with a warning counting the rest. An undo that
meets an id taken since skips that node and what was inside it, and never renames it. A lenient
delete of a node something was added inside since is skipped as `inside`. A list that applies
nothing changes nothing: no rev, event or undo.

`reply` `short` (an agent's default) answers the rev, each edit's node id in `ids`, `removed_count`,
`audit_id`, `skipped`, `warnings`, `copied` (each copy's new node and the node it copied) and
`rebased`. `full` (the person's clients' default) also answers the design row, each node an edit
touched as stored, the `removed` ids with what was inside them, the `inverse` edits that undo the
list in the order to apply them, the `expect` the undo needs, and `colors` and `styles` when an edit
changed them. A full answer over 1 MiB of nodes and undo leaves the nodes out (`reread: true`: read
the design again); over it without them, the undo is left out too, with a warning. The audit row's
undo is that `inverse`, applied leniently, with its `expect` kept beside it (§10.3).

A strict add of a shape, frame or icon with no size gets a default one (100, or 24 for an icon), and
a free line's or path's box is fitted to its points; a strict update refits it. Follow rule: when an
update changes a frame's stored `w` or `h`, its pinned free and absolute children follow it on each
axis the list did not set on them itself (`Node::follow`), level by level, up to 64 levels down. Each
follow is an update of its own, counted, undone and expected like any other. A connector that a
delete or a move to another page frees is drawn again from its ends' stored centres. A node, run or
style naming a font the desktop lacks gets a warning, never a refusal; `design.fonts` lists what is
there.

`design.get` with `since_rev` (0 to the design's rev) answers what changed since that rev: `nodes`
are those added, changed or moved since, each with its place among its siblings in `indexes`, and
`removed` lists the ids gone since, in the order they went, with everything a delete took. With
`page` or `nodes`, it answers what changed in what they name (a changed node with what is inside it),
and `left` lists the changed ids outside them, which a holder drops. It reaches back 1000 revs:
`conflict` / `design.since_too_old` (`details.rev`) for a rev older than that, or older than the rev
the design's removals are on record from; `invalid` / `design.since_rev` for a negative rev or one
ahead of the design. In either case, read the design whole.

Migration v30 (schema 30) adds `designs.styles` (the named colours and styles, one JSON object),
`designs.tracked_from` (the rev the removals began to be kept at), the rev each node row was last
written at (`design_nodes.rev`), and `design_removed` (the ids removed in the last 1000 revs, with the
rev each went at). It recounts `designs.bytes` so the size cap counts each node as `design.get`
returns it. Rows written before take their design's rev, so an older `base_rev` reads them as
changed; a restored v29 backup's rows are dated the same way, and its removals are kept from that rev.

---

## 11. Entity shapes (results)

Common: every row has `id: Id`, `created_at`, `updated_at` (RFC 3339 UTC); soft-deletable rows
add `deleted_at: string | null`.

### 11.1 Task

`column` is where the card sits on the board (SPEC §6's five, fixed). `state` is the agent
lifecycle *on* the task, independent of column. Values confirmed 2026-08-17 (`Priority` keeps
v3's `medium`); breaking to change from here on.

`type` is a first-class classification, separate from the free-form `labels` under it; every
pre-v16 row migrated to the neutral `"task"`. Sub-tasks are real tasks — their own card, their
own column, their own dispatch — and a parent carries a `rollup` over its whole subtree. Nesting
is capped at `TASK_DEPTH_MAX = 3` (a root plus two levels) and `TASK_CHILDREN_MAX = 100` direct
children per parent; past either, the bus refuses with `task.depth` / `task.children_full`
rather than silently flattening (D139).

```ts
type Column   = "backlog" | "in_review" | "ready" | "active" | "done";   // SPEC §6 five, fixed
type TaskState = "none" | "dispatched" | "running" | "blocked" | "failed" | "awaiting_review";
type Priority = "low" | "medium" | "high" | "urgent";
type Size     = "S" | "M" | "L";
type TaskType = "task" | "feature" | "bug" | "chore" | "spike";

interface Task {
  id; project_id; module_id: Id | null;
  module_name: string | null;           // module_id's name, completed (archived) modules included
  title: string; body: string; changelog: string;
  column: Column; position: number; state: TaskState;
  priority: Priority; size: Size | null;
  type: TaskType;
  parent_id: Id | null; depth: number;  // depth 0 = root; never above TASK_DEPTH_MAX - 1
  children: Id[];                       // direct children, in board order
  rollup: { total: number; done: number };   // the whole subtree, for the parent's n/m badge
  labels: string[];                     // sorted; free-form tags under `type`
  blocked_by: Id[]; blocks: Id[];       // `blocks` is the inverse read; only blocked_by is stored
  duplicate_of: Id | null;
  note_id: Id | null;                   // the note it was made from; cleared when that note is purged
  sessions: string[];                   // every session ever dispatched to it, in order; a PAIR adds two
  commits: { sha: string; branch: string | null; linked_at: string }[];
  attachments: Attachment[];
  created_at; updated_at; deleted_at;
}

interface Label { id; project_id; name: string; created_at }
```

Relations point one way in the store: `blocked_by` and `duplicate_of` are rows, `blocks` is the
reverse read of the same rows. A soft-deleted task drops out of both ends until it is restored.
Deleting a parent leaves its children pointing at it, so restoring the parent restores the group;
until then the board renders those children as roots, because rolled-up work is never hidden.

Column transitions by actor (anything else is `conflict` / `task.column_transition`):

| from → to | user | agent (own task) |
|---|---|---|
| any → any (drag) | ✓ | ✗ |
| `active → in_review` | ✓ | only through `session.done`, which judges the branch first |
| `* → done` | via `task.approve` (links sha) | ✗ |
| `* → active` | via `task.dispatch` or drag | ✗ |

### 11.2 Module

```ts
interface Module { id; project_id; name; icon: string | null; priority: Priority; order: number;
  completed_at: string | null; created_at; updated_at; deleted_at }
interface ModuleSummary extends Module { counts: Record<Column, number>; progress_pct: number }
```

### 11.3 Session

```ts
type Provider = "claude" | "codex";
type Role = "builder" | "reviewer" | "docs";
type SessionState = "created" | "spawning" | "running" | "idle" | "blocked" | "parked"
                  | "restorable" | "exited" | "closed";
interface Session {
  id; name: string; project_id; provider: Provider; role: Role;
  model: string | null; effort: string | null;
  branch: string; worktree: string; task_id: Id | null; module_id: Id | null;
  pair_with: string | null;                // the other half of a PAIR
  bus_writes: bool; allow_ui: bool;        // §9.1 session options
  use_api_key: bool;                       // Claude bills the key proxy's API key, see session.create
  state: SessionState; pid: number | null; exit_code: number | null;
  provider_ref: string | null;             // provider's own resume handle
  spawned_at: string | null; last_output_at: string | null;
  usage: object | null;                    // provider units, opaque
  created_at; updated_at; closed_at: string | null;
}
interface Peer { session: string; provider; role; branch; state; task_title: string | null;
  claimed: string[]; last_output_at }
```

### 11.4 Others

```ts
interface Workspace { id; path; name; order }
interface Project { id; workspace_id; path; name; base_branch; build_cmd: string | null;
  run_cmd: string | null; protected_paths: string[]; critical_files: string[]; order; pinned: bool }
interface Note { id; project_id; title: string | null; body; pinned: bool; created_at; updated_at; deleted_at }
interface Design { id; project_id; session: string | null; title; rev /* +1 per change */; pages; nodes;
  created_by: string; updated_by: string /* "user" or a session name */; created_at; updated_at; deleted_at }
interface DesignNode { id; parent?: string /* none for a page */; kind: "page" | "frame" | "group" | "rect"
  | "ellipse" | "line" | "text" | "sticky" | "path" | "image"; name?; x; y; w; h; /* optional style,
  text, layout, points and src fields: relay_bus::design::Node */ }
interface Message { id; project_id; from: string; to: string; text; re_task: Id | null; sent_at; acked_at: string | null }
interface Overlap { id; project_id; sessions: [string, string] | [string]; path; symbol: string | null;
  kind: "file" | "symbol" | "claim"; acked_by: string[]; first_seen; last_seen }
interface Hold { id; project_id; session_id: Id | null; session: string | null; actor: Actor; op: string;
  payload_hash: string; policy: string; details: object;
  state: "open" | "confirmed" | "rejected" | "expired"; created_at; resolved_at; resolved_by: Actor | null }
interface Peer { session; provider; role; branch; state: SessionState; task_title: string | null;
  claimed: string[]; last_output_at: Ts | null;
  intent: string | null /* one line, self-declared: session.intent */ }
interface GuardrailConfig {
  caps: { files: number; lines: number };
  destructive_write: { min_removed_lines: number; min_removed_pct: number; min_file_lines: number;
    allow_if_recoverable: bool /* let a big rewrite through when git can restore the file */ };
  protected_paths: string[];
  shape_gates: { path: string; validator: "non_empty" | "json" | "json_non_empty_array" | "json_non_empty_object" }[];
  denied_commands: string[];   /* matched against parsed argv, never a raw substring */
  allowed_write_roots: string[]; /* absolute (a relative or ~ entry fails the config); the process temp dir is always allowed too */
  agent_builds: boolean;  /* default false: an agent's integration build waits for a person */
  check_on_push: boolean; /* default true: an agent's `git push` first runs the project's build_cmd (else the detected tests) and is refused when it fails; a person's push never runs it */
  roles: { builder: string[]; reviewer: string[]; docs: string[] };
}
interface AuditRow { id; ts; req_id; parent_req: string | null; actor; on_behalf_of: Actor | null;
  session_id: Id | null; op; project_id: Id | null; kind: "ok" | "held" | "refused" | "error";
  code: string | null; hold_id: Id | null; payload_hash: string; payload: object | null /* null when > 64 KiB */;
  result_summary: object | null; undo_op: {op, payload, expect} | null; undo_of: Id | null; undone_by: Id | null }
interface Notification { id; category: "agent_done" | "agent_blocked" | "guardrail" | "integration"
  | "provider" | "disk" | "system"; title; body; link: {op, payload} | null; read: bool; created_at }
interface Worktree { path; branch; head: string; session: string | null; dirty: bool; disk_mb: number | null }
interface Integration { id; project_id; branches: string[]; worktree: string | null;
  state: "queued" | "merging" | "building" | "deploying" | "passed" | "failed" | "conflict" | "discarded";
  conflict: [string, string] | null; log_tail: string; started_at; finished_at }
interface ProviderInfo { provider; installed: bool; path: string | null; version: string | null;
  signed_in_as: string | null; last_seen_version: string | null; spawn_profile: object;
  guarded: bool /* true for both providers: Claude Code's hooks in .claude/settings.local.json,
                    Codex's in .codex/hooks.json once trusted in /hooks (§9.3, D132) */ }
interface Skill { id; name; description: string /* the SKILL.md front matter's `description`, else
  its first prose line, else ""; read from the whole body, so a summary's cut body keeps it */;
  body; enabled_in: Id[] }
interface Device { serial; model; kind: "usb" | "avd"; state }
```

---

## 12. Versioning and evolution

- The envelope `v` is the schema major. v1 is this document.
- **Additive changes** (new op, new optional payload field, new result field, new event, new
  error code, new optional envelope field) do not bump `v`. Clients ignore unknown result and
  envelope fields (`Response`, `Event`, frames and `BusError` are lenient); core rejects unknown
  payload fields (§5.2) — so a *new required* payload field is breaking, and a new optional one
  is not.
- **Breaking changes** (removed op, renamed op, changed field type, new required field,
  changed semantics of a code) bump `v`; core keeps serving `v-1` for one major with a
  translation shim, and logs a `bus.deprecated` warning event once per client per op.
- Every op carries `since: "1.0"` in the registry so `bus.ops` can tell an agent what exists.
- Deprecation is an attribute (`deprecated: "use x.y"`), visible in `bus.ops`, for one major.

---

## 13. Store tables owned by the bus (phase 1)

Everything else lands with its phase, but the bus itself needs:

```sql
CREATE TABLE audit (
  id INTEGER PRIMARY KEY, ts TEXT NOT NULL, req_id TEXT NOT NULL UNIQUE, parent_req TEXT,
  actor TEXT NOT NULL, on_behalf_of TEXT, session_id INTEGER, op TEXT NOT NULL, project_id INTEGER,
  kind TEXT NOT NULL CHECK (kind IN ('ok','held','refused','error')), code TEXT, hold_id INTEGER,
  payload_hash TEXT NOT NULL, payload TEXT, result_summary TEXT,
  undo_op TEXT, undo_of INTEGER REFERENCES audit(id), undone_by INTEGER REFERENCES audit(id)
);
CREATE INDEX audit_ts ON audit(ts);
CREATE INDEX audit_actor_ts ON audit(actor, ts);
CREATE INDEX audit_session_ts ON audit(session_id, ts);
CREATE INDEX audit_op_ts ON audit(op, ts);
CREATE INDEX audit_project_ts ON audit(project_id, ts);
CREATE INDEX audit_parent ON audit(parent_req);
CREATE INDEX audit_undo_of ON audit(undo_of);

CREATE TABLE holds (
  id INTEGER PRIMARY KEY, project_id INTEGER, session_id INTEGER, session TEXT, actor TEXT NOT NULL,
  op TEXT NOT NULL, envelope TEXT NOT NULL, policy TEXT NOT NULL, details TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'open', created_at TEXT NOT NULL, resolved_at TEXT, resolved_by TEXT
);
CREATE INDEX holds_state ON holds(state, created_at);
CREATE INDEX holds_session ON holds(session_id);

CREATE TABLE settings ( path TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL );
```

`PRAGMA user_version` starts at 1 with this; every later phase is a numbered migration with a
test that opens a DB at each prior version.

---

## 14. Non-goals and known trade-offs (so nobody re-argues them by accident)

- **No pagination cursors.** One user; `limit ≤ 1000`; if a list is bigger, the filter is
  wrong.
- **No per-request auth beyond tokens.** Same uid = same trust (§0.6) on the socket; over the
  phone door, `user` is whoever holds a paired device's token (§4.2). Tokens prevent
  misattribution, and the peer check on `user` (§4.2) stops an agent's own process tree from
  claiming the user; neither stops determined impersonation by a same-uid process that gets
  itself started outside that tree.
- **No RPC over the network, with one exception.** The engine's own door is the Unix socket
  only. The exception is the paired-phone door (`crates/relay-remote`, `docs/MOBILE.md`): it
  forwards bus lines from a WebSocket to this socket as actor `user`, after a per-connection
  proof of a device token. That channel is not yet sealed end to end, so an on-path attacker
  can read and inject lines; MOBILE.md §6 says so and what to do meanwhile.
- **No streaming results for ordinary ops.** Slow work returns a handle and emits events.
- **The bus is not the PTY.** §7. Anyone proposing "just send keystrokes as bus ops" is
  reinventing v3's `pty_write` storm.
- **The UI does not get private commands.** If it needs something, it becomes an op here.
- **Comments on tasks (v3) are dropped.** The body, changelog and mailbox cover the uses; the
  v3 importer folds comments into the body under a `--- comments ---` marker.
- **`merge.integration.request` (SPEC §3) is `integration.request` here.** Same op; the
  namespace is the noun. Skills written from SPEC's wording should be updated, not aliased.
- **Guardrails bind agents through hooks, not through the bus's file ops** (§9.3). Shell writes
  are gated only where the command line shows them, for every provider, and the spec says so
  rather than pretends (D163).

---

## 15. Phase-1 scope check

Phase 1 (SPEC §17.1) ships: this document · `relay-bus` crate (envelope, error, registry,
schema generation, pipeline traits) · `relay-core` engine skeleton with the store's
`audit`/`holds`/`settings` tables and migrations framework · socket door and `relay serve` ·
`relay` CLI (`cmd`, `q`, `events`, `schema`, `ops`, `ping`, `serve`) · Tauri shell **stub**
(Relay-2's door #1 — one `bus` command, `bus:event` forwarding, a ping page — no chrome; SPEC
puts the real shell in phase 9. V4 has no Tauri shell: its UI is a socket client, §6.1) · ops
actually executable in phase 1: all of
`bus.*`, `app.version`, `app.status`, `app.reconcile` (no-op list), `audit.list/get`,
`settings.*`, `workspace.*`, `project.add/list/get/update/remove` (no git yet: path must
contain `.git`). Every other op in §10 is registered with its schema and returns
`unavailable` / `bus.not_implemented` with `details.phase` — so agents and the UI can already
see the whole surface, and the schema file is complete from day one.
