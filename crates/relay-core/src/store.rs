//! The single `store.db` (SPEC §1): WAL, `user_version`-numbered migrations, indexes on every
//! FK and every WHERE column, one connection behind a mutex (writes are serial by design,
//! BUS.md §5.1). Later phases append migrations; every prior version stays openable
//! (SPEC §16 migration tests).

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, Transaction};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The schema version this build knows. Bump when appending to [`MIGRATIONS`].
pub const SCHEMA_VERSION: i64 = 36;

/// Numbered migrations; index 0 brings a fresh DB to `user_version = 1`.
pub const MIGRATIONS: &[&str] = &[
    // v1 — phase 1: what the bus itself needs (BUS.md §13) plus workspaces/projects, which
    // phase 1 already executes (BUS.md §15).
    r#"
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

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

    CREATE TABLE settings (path TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL);

    CREATE TABLE workspaces (
      id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, name TEXT NOT NULL, ord INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL
    );

    CREATE TABLE projects (
      id INTEGER PRIMARY KEY, workspace_id INTEGER NOT NULL REFERENCES workspaces(id),
      path TEXT NOT NULL UNIQUE, name TEXT NOT NULL, base_branch TEXT NOT NULL DEFAULT 'main',
      build_cmd TEXT, run_cmd TEXT, protected_paths TEXT NOT NULL DEFAULT '[]', critical_files TEXT NOT NULL DEFAULT '[]',
      ord INTEGER NOT NULL DEFAULT 0, pinned INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL
    );
    CREATE INDEX projects_workspace ON projects(workspace_id, ord);
    "#,
    // v2 — phase 2: the entities the v3 importer fills (BUS.md §11). Ids are AUTOINCREMENT
    // (v3's D85 lesson: a task's id names things on disk; a bare rowid recycles a deleted
    // id into the next insert). Sessions carry their token/epoch columns now so phase 3
    // does not need to ALTER.
    r#"
    CREATE TABLE modules (
      id INTEGER PRIMARY KEY AUTOINCREMENT, project_id INTEGER NOT NULL REFERENCES projects(id),
      name TEXT NOT NULL, icon TEXT, priority TEXT NOT NULL DEFAULT 'medium', ord INTEGER NOT NULL DEFAULT 0,
      completed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX modules_project ON modules(project_id, deleted_at, ord);

    CREATE TABLE tasks (
      id INTEGER PRIMARY KEY AUTOINCREMENT, project_id INTEGER NOT NULL REFERENCES projects(id),
      module_id INTEGER REFERENCES modules(id),
      title TEXT NOT NULL, body TEXT NOT NULL DEFAULT '', changelog TEXT NOT NULL DEFAULT '',
      col TEXT NOT NULL DEFAULT 'backlog' CHECK (col IN ('backlog','in_review','ready','active','done')),
      position INTEGER NOT NULL DEFAULT 0, state TEXT NOT NULL DEFAULT 'none',
      priority TEXT NOT NULL DEFAULT 'medium', size TEXT,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX tasks_project_col ON tasks(project_id, deleted_at, col, position);
    CREATE INDEX tasks_module ON tasks(module_id);
    CREATE INDEX tasks_state ON tasks(project_id, state);

    CREATE TABLE sessions (
      id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, project_id INTEGER NOT NULL REFERENCES projects(id),
      provider TEXT NOT NULL, role TEXT NOT NULL DEFAULT 'builder', model TEXT, effort TEXT,
      branch TEXT NOT NULL DEFAULT '', worktree TEXT NOT NULL DEFAULT '',
      task_id INTEGER REFERENCES tasks(id), module_id INTEGER REFERENCES modules(id), pair_with TEXT,
      bus_writes INTEGER NOT NULL DEFAULT 0, allow_ui INTEGER NOT NULL DEFAULT 0,
      state TEXT NOT NULL DEFAULT 'created', pid INTEGER, exit_code INTEGER, provider_ref TEXT,
      token TEXT NOT NULL DEFAULT '', epoch INTEGER NOT NULL DEFAULT 0,
      spawned_at TEXT, last_output_at TEXT, usage TEXT,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, closed_at TEXT
    );
    CREATE UNIQUE INDEX sessions_live_name ON sessions(name) WHERE state != 'closed';
    CREATE INDEX sessions_project_state ON sessions(project_id, state);
    CREATE INDEX sessions_task ON sessions(task_id);

    CREATE TABLE task_sessions (
      task_id INTEGER NOT NULL REFERENCES tasks(id), session_id INTEGER NOT NULL REFERENCES sessions(id),
      ord INTEGER NOT NULL, PRIMARY KEY (task_id, session_id)
    );
    CREATE INDEX task_sessions_session ON task_sessions(session_id);

    CREATE TABLE task_commits (
      id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL REFERENCES tasks(id),
      sha TEXT NOT NULL, branch TEXT, linked_at TEXT NOT NULL, UNIQUE (task_id, sha)
    );
    CREATE INDEX task_commits_task ON task_commits(task_id, id);

    CREATE TABLE attachments (
      id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL REFERENCES tasks(id),
      name TEXT NOT NULL, mime TEXT NOT NULL, bytes INTEGER NOT NULL, path TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE INDEX attachments_task ON attachments(task_id, id);

    CREATE TABLE notes (
      id INTEGER PRIMARY KEY AUTOINCREMENT, project_id INTEGER NOT NULL REFERENCES projects(id),
      title TEXT, body TEXT NOT NULL DEFAULT '', pinned INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX notes_project ON notes(project_id, deleted_at, pinned, updated_at);
    CREATE INDEX sessions_module ON sessions(module_id);

    -- v1 forgot one FK index (the migration test now checks every FK):
    CREATE INDEX audit_undone_by ON audit(undone_by);
    "#,
    // v3 — phase 4: guardrail holds are already a phase-1 bus table; notifications land
    // now because a hold/refusal must survive a restart even before the phase-9 chrome exists.
    r#"
    CREATE TABLE notifications (
      id INTEGER PRIMARY KEY AUTOINCREMENT, project_id INTEGER REFERENCES projects(id),
      category TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, link TEXT,
      read INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL
    );
    CREATE INDEX notifications_project ON notifications(project_id, read, created_at);
    CREATE INDEX notifications_category ON notifications(category, read, created_at);
    "#,
    // v4 — phase 5: project standing context, a persistent per-recipient mailbox,
    // explicit claims, and durable overlap findings.
    r#"
    ALTER TABLE notes ADD COLUMN standing INTEGER NOT NULL DEFAULT 0;
    CREATE UNIQUE INDEX notes_one_standing
      ON notes(project_id) WHERE standing = 1 AND deleted_at IS NULL;

    CREATE TABLE messages (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      from_session TEXT NOT NULL, to_spec TEXT NOT NULL, text TEXT NOT NULL,
      re_task INTEGER REFERENCES tasks(id), sent_at TEXT NOT NULL
    );
    CREATE INDEX messages_project_sent ON messages(project_id, sent_at, id);
    CREATE INDEX messages_re_task ON messages(re_task);

    CREATE TABLE message_recipients (
      message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
      session_id INTEGER NOT NULL REFERENCES sessions(id),
      session TEXT NOT NULL, acked_at TEXT,
      PRIMARY KEY (message_id, session_id)
    );
    CREATE INDEX message_recipients_session_ack
      ON message_recipients(session_id, acked_at, message_id);

    CREATE TABLE claims (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      session TEXT NOT NULL, path TEXT NOT NULL, symbol TEXT NOT NULL DEFAULT '', note TEXT,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
      UNIQUE(session_id, path, symbol)
    );
    CREATE INDEX claims_project_path ON claims(project_id, path, symbol);
    CREATE INDEX claims_session ON claims(session_id);

    CREATE TABLE overlaps (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      fingerprint TEXT NOT NULL UNIQUE, sessions TEXT NOT NULL, path TEXT NOT NULL,
      symbol TEXT, kind TEXT NOT NULL CHECK (kind IN ('file','symbol','claim')),
      acked_by TEXT NOT NULL DEFAULT '[]', active INTEGER NOT NULL DEFAULT 1,
      first_seen TEXT NOT NULL, last_seen TEXT NOT NULL
    );
    CREATE INDEX overlaps_project_active ON overlaps(project_id, active, last_seen);
    "#,
    // v5 — phase 6: cached provider discovery and lifecycle state that does not belong in
    // the public Session entity. Scrollback is snapshotted on park/app shutdown.
    r#"
    CREATE TABLE provider_cache (
      provider TEXT PRIMARY KEY CHECK (provider IN ('claude','codex')),
      path TEXT, version TEXT, signed_in_as TEXT, last_seen_version TEXT,
      spawn_profile TEXT NOT NULL, detected_at TEXT NOT NULL
    );

    ALTER TABLE sessions ADD COLUMN restore_reason TEXT;

    CREATE TABLE session_scrollback (
      session_id INTEGER PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
      text BLOB NOT NULL, epoch INTEGER NOT NULL, seq INTEGER NOT NULL, updated_at TEXT NOT NULL
    );
    CREATE INDEX session_scrollback_session ON session_scrollback(session_id);
    "#,
    // v6 — phase 7: deleting a module unlinks its board tasks, but undo must restore the
    // exact membership without clobbering tasks the user reassigned in the meantime.
    r#"
    CREATE TABLE module_unlinked_tasks (
      module_id INTEGER NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
      task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      PRIMARY KEY (module_id, task_id)
    );
    CREATE INDEX module_unlinked_tasks_task ON module_unlinked_tasks(task_id);
    "#,
    // v7 - phase 8: recoverable file deletion and durable integration runs.
    r#"
    CREATE TABLE file_trash (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      worktree TEXT NOT NULL, original_path TEXT NOT NULL, trash_path TEXT NOT NULL,
      created_at TEXT NOT NULL, restored_at TEXT
    );
    CREATE INDEX file_trash_project ON file_trash(project_id, restored_at, id);

    CREATE TABLE integrations (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      branches TEXT NOT NULL, worktree TEXT, state TEXT NOT NULL,
      conflict TEXT, log_tail TEXT NOT NULL DEFAULT '', build INTEGER NOT NULL DEFAULT 0,
      deploy TEXT, started_at TEXT, finished_at TEXT, created_at TEXT NOT NULL
    );
    CREATE INDEX integrations_project ON integrations(project_id, id DESC);
    CREATE INDEX integrations_state ON integrations(state, id);
    "#,
    // v8 — phase 9: named shell arrangements are durable per project.
    r#"
    CREATE TABLE ui_layouts (
      project_id INTEGER NOT NULL REFERENCES projects(id), name TEXT NOT NULL,
      state TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
      PRIMARY KEY(project_id, name)
    );
    CREATE INDEX ui_layouts_project_updated ON ui_layouts(project_id, updated_at);
    "#,
    // v9 — phase 10: device runs survive reloads; mirrors stay runtime-only because an
    // H.264 producer cannot survive the process that owns its ADB transport.
    r#"
    CREATE TABLE device_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      device TEXT NOT NULL, worktree TEXT NOT NULL, state TEXT NOT NULL,
      started_at TEXT NOT NULL, finished_at TEXT
    );
    CREATE INDEX device_runs_project ON device_runs(project_id, id DESC);
    CREATE INDEX device_runs_state ON device_runs(state, id);
    "#,
    // v10 - phase 11: skills are global markdown instructions with per-project enables.
    // Deleted rows stay addressable so audit undo can restore the same id and enable set.
    r#"
    CREATE TABLE skills (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL COLLATE NOCASE UNIQUE, body TEXT NOT NULL,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX skills_deleted_name ON skills(deleted_at, name);

    CREATE TABLE skill_projects (
      skill_id INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
      project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
      PRIMARY KEY(skill_id, project_id)
    );
    CREATE INDEX skill_projects_project ON skill_projects(project_id, skill_id);
    "#,
    // v11 - phase 12: GitHub is the source of truth for installed skills.
    r#"
    ALTER TABLE skills ADD COLUMN source_url TEXT;
    ALTER TABLE skills ADD COLUMN source_path TEXT;
    ALTER TABLE skills ADD COLUMN revision TEXT;
    CREATE UNIQUE INDEX skills_source ON skills(source_url, source_path) WHERE source_url IS NOT NULL;
    "#,
    // v12 - launch text is durable bootstrap data, never a positional provider prompt.
    r#"
    ALTER TABLE sessions ADD COLUMN launch_prompt TEXT;
    "#,
    // v13 — one line of self-declared intent per session, so peers can coordinate on what a
    // session is doing rather than inferring it from a branch name.
    r#"
    ALTER TABLE sessions ADD COLUMN intent TEXT;
    "#,
    // v14 - preserve per-session task queue order independently of the per-task agent order.
    // Existing assignments get a deterministic task-id order; new assignments append in the
    // order the user selected them.
    r#"
    ALTER TABLE task_sessions ADD COLUMN queue_ord INTEGER NOT NULL DEFAULT 0;
    UPDATE task_sessions SET queue_ord=task_id;
    CREATE INDEX task_sessions_queue ON task_sessions(session_id, queue_ord, task_id);
    "#,
    // v15 - priority mail is a live response hint, while suggestions are deliberately kept
    // out of the standing note that is injected into every dispatch.
    r#"
    ALTER TABLE messages ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE notes ADD COLUMN suggestions INTEGER NOT NULL DEFAULT 0;
    CREATE UNIQUE INDEX notes_one_suggestions
      ON notes(project_id) WHERE suggestions = 1 AND deleted_at IS NULL;
    "#,
    // v16 - the board grows the GitHub-issues model: a first-class type, parent/child
    // sub-tasks with a depth cap, free-form labels, and stored blocked-by / duplicate-of
    // edges. Existing rows become type 'task', parentless, unlabelled and unrelated, so a
    // v15 store reads identically after the upgrade (D139).
    r#"
    ALTER TABLE tasks ADD COLUMN kind TEXT NOT NULL DEFAULT 'task'
      CHECK (kind IN ('task','feature','bug','chore','spike'));
    ALTER TABLE tasks ADD COLUMN parent_id INTEGER REFERENCES tasks(id);
    CREATE INDEX tasks_parent ON tasks(parent_id, position, id);
    CREATE INDEX tasks_kind ON tasks(project_id, kind);

    CREATE TABLE labels (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      name TEXT NOT NULL, created_at TEXT NOT NULL
    );
    CREATE UNIQUE INDEX labels_project_name ON labels(project_id, name COLLATE NOCASE);

    CREATE TABLE task_labels (
      task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
      PRIMARY KEY(task_id, label_id)
    );
    CREATE INDEX task_labels_label ON task_labels(label_id, task_id);

    CREATE TABLE task_relations (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      from_task INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      to_task INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      rel TEXT NOT NULL CHECK (rel IN ('blocked_by','duplicate_of')),
      created_at TEXT NOT NULL
    );
    CREATE UNIQUE INDEX task_relations_edge ON task_relations(from_task, to_task, rel);
    CREATE INDEX task_relations_to ON task_relations(to_task, rel);
    "#,
    // v17 - device_runs also holds device-less release builds: `kind` separates them from
    // install-and-launch runs and `artifact` records the APK / AAB a finished build produced.
    // Existing rows become kind 'run' with no artifact, so a v16 store reads identically (D152).
    r#"
    ALTER TABLE device_runs ADD COLUMN kind TEXT NOT NULL DEFAULT 'run'
      CHECK (kind IN ('run','build'));
    ALTER TABLE device_runs ADD COLUMN artifact TEXT;
    "#,
    // v18 - keep enough release metadata to explain an active or historical build without
    // parsing its Gradle log. Signing is verified from the artifact; credentials stay in Gradle.
    r#"
    ALTER TABLE device_runs ADD COLUMN variant TEXT;
    ALTER TABLE device_runs ADD COLUMN format TEXT
      CHECK (format IS NULL OR format IN ('apk','bundle'));
    ALTER TABLE device_runs ADD COLUMN publish INTEGER NOT NULL DEFAULT 0
      CHECK (publish IN (0,1));
    ALTER TABLE device_runs ADD COLUMN signing TEXT
      CHECK (signing IS NULL OR signing IN ('signed','unsigned','unverified'));
    "#,
    // v19 - completion belongs to a participant's assignment, not the shared task alone.
    r#"
    ALTER TABLE task_sessions ADD COLUMN completed_at TEXT;
    ALTER TABLE sessions ADD COLUMN done_pending_stop INTEGER;
    "#,
    // v20 - bundled plugins are switched on per project (D159). The plugin itself lives in the
    // binary, so only the edge is stored; an id this build no longer bundles is simply ignored.
    r#"
    CREATE TABLE plugin_projects (
      plugin_id TEXT NOT NULL,
      project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
      enabled_at TEXT NOT NULL,
      PRIMARY KEY(plugin_id, project_id)
    );
    CREATE INDEX plugin_projects_project ON plugin_projects(project_id, plugin_id);
    "#,
    // v21 - the shell's global notification list (no project, newest first, optionally unread
    // only) had no index to walk, so every open was a full scan and sort under the store mutex.
    r#"
    CREATE INDEX notifications_recent ON notifications(created_at, id);
    CREATE INDEX notifications_unread_recent ON notifications(read, created_at, id);
    "#,
    // v22 - a GitHub skill remembers the branch or tag it was installed from (NULL: the
    // repository's default branch), so a refresh pulls that ref again, and the same folder
    // installed from two refs is two sources rather than one row each install overwrites.
    r#"
    ALTER TABLE skills ADD COLUMN source_ref TEXT;
    DROP INDEX skills_source;
    CREATE UNIQUE INDEX skills_source ON skills(source_url, source_path, IFNULL(source_ref, ''))
      WHERE source_url IS NOT NULL;
    "#,
    // v23 - a hold's payload hash is taken once, when it is frozen, instead of by re-parsing
    // and re-hashing every envelope a list reads (NULL for older holds: SQLite has no SHA-256,
    // so each is filled the first time it is read). A held write's large text is kept in a
    // content-addressed file beside the store, named here, instead of in the envelope (RA-102).
    r#"
    ALTER TABLE holds ADD COLUMN payload_hash TEXT;
    CREATE INDEX holds_unhashed ON holds(id) WHERE payload_hash IS NULL;
    CREATE TABLE hold_blobs (
      hold_id INTEGER NOT NULL REFERENCES holds(id) ON DELETE CASCADE,
      pointer TEXT NOT NULL, hash TEXT NOT NULL,
      PRIMARY KEY (hold_id, pointer)
    );
    CREATE INDEX hold_blobs_hash ON hold_blobs(hash);
    "#,
    // v24 - task.detach soft-deletes the attachment, keeping its id and its file, so undoing it
    // restores that row instead of copying the file in again under a new id (RA-414). Every
    // read of a task's attachments filters `deleted_at IS NULL`; the retention pass removes the
    // row and its file once the undo grace window is past. Existing rows are live.
    r#"
    ALTER TABLE attachments ADD COLUMN deleted_at TEXT;
    "#,
    // v25 - comments on a task, GitHub-issue style (`task.comment`): append-only, read oldest
    // first by `task.activity`. `author` is the session name for an agent, `user` for a person.
    r#"
    CREATE TABLE task_comments (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      author TEXT NOT NULL, body TEXT NOT NULL, created_at TEXT NOT NULL
    );
    CREATE INDEX task_comments_task ON task_comments(task_id, id);
    "#,
    // v26 - a task remembers the note it was made from (`task.create` with `note_id`), so the
    // note's thinking stays one step from the card. Purging the note clears the link instead
    // of the task; existing tasks were made from no note.
    r#"
    ALTER TABLE tasks ADD COLUMN note_id INTEGER REFERENCES notes(id) ON DELETE SET NULL;
    CREATE INDEX tasks_note ON tasks(note_id);
    "#,
    // v27 - agents' hooks write an audit row on every tool call, and allowed gate decisions and
    // session reports kept the whole hook payload (~1.4 KB): 85 % of a 260 MB store
    // (BASELINE-2026-10-14 §1.2). New rows keep what `audit::slim` keeps; this slims the rows
    // already there the same way. The payload hash still names the full payload. Rows already
    // reduced to a "truncated" marker, refusals and holds are left as they are.
    r#"
    UPDATE audit SET payload = json_object('session', json_extract(payload, '$.session'), 'kind', json_extract(payload, '$.kind'),
        'command', substr(json_extract(payload, '$.command'), 1, 200), 'path', json_extract(payload, '$.path'), 'slim', json('true'))
      WHERE op = 'guardrail.gate' AND kind = 'ok' AND json_valid(payload) AND json_extract(payload, '$.kind') IS NOT NULL;
    UPDATE audit SET payload = json_object('session', json_extract(payload, '$.session'), 'kind', json_extract(payload, '$.kind'),
        'tool', json_extract(payload, '$.data.tool_name'), 'slim', json('true'))
      WHERE op = 'session.report' AND kind = 'ok' AND json_valid(payload) AND json_extract(payload, '$.kind') IS NOT NULL;
    "#,
    // v28 - design canvases (`design.*`, BUS.md §10.18): a project's designs, each a tree of
    // nodes. A node row is its parent and paint order (`ord`, ascending is bottom to top) beside
    // the node's own JSON, so a reorder or a move rewrites two columns, not the node. `session`
    // is a name, not an FK: names outlive the sessions that held them. Ids are AUTOINCREMENT, as
    // tasks' are: audit undo rows and removal backups name a design by id, and a purged id
    // recycled into a new design would be restored over or deleted by mistake. The counters
    // (`pages`, `nodes`, `bytes` of node JSON, `next_node` for engine-assigned ids) are kept by
    // every write, so a list and the size limits never count a design's nodes.
    r#"
    CREATE TABLE designs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      session TEXT, title TEXT NOT NULL,
      rev INTEGER NOT NULL DEFAULT 0, pages INTEGER NOT NULL DEFAULT 0, nodes INTEGER NOT NULL DEFAULT 0,
      bytes INTEGER NOT NULL DEFAULT 0, next_node INTEGER NOT NULL DEFAULT 1,
      created_by TEXT NOT NULL, updated_by TEXT NOT NULL,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX designs_project ON designs(project_id, deleted_at, updated_at);

    CREATE TABLE design_nodes (
      design_id INTEGER NOT NULL REFERENCES designs(id) ON DELETE CASCADE,
      node_id TEXT NOT NULL, parent_id TEXT, kind TEXT NOT NULL, ord REAL NOT NULL, body TEXT NOT NULL,
      PRIMARY KEY (design_id, node_id)
    ) WITHOUT ROWID;
    CREATE INDEX design_nodes_parent ON design_nodes(design_id, parent_id, ord);
    "#,
    // v29 - a Claude session may run on an API key, through the key proxy, instead of the user's
    // subscription (`session.create` `use_api_key`). Only the choice is stored, never the key.
    r#"
    ALTER TABLE sessions ADD COLUMN use_api_key INTEGER NOT NULL DEFAULT 0;
    "#,
    // v30 - design canvases, round two (BUS.md §10.18). A design's named colours and styles, one
    // JSON object read whole by design.get and rewritten whole by an apply that changes them; the
    // rev at which each node row was last written, and the ids removed in the last 1000 revs
    // (design.apply's base_rev, design.get's since_rev); and `bytes` recounted to hold each node
    // as design.get returns it (its id, its parent and 21 bytes of JSON around them, besides its
    // body), so the size cap keeps a design.get under the clients' 2 MiB line (B60). Rows written
    // before take their design's rev now: they read as changed for any older base_rev. Removals
    // are on record from `tracked_from`: a since_rev before it is design.since_too_old.
    r#"
    ALTER TABLE designs ADD COLUMN styles TEXT NOT NULL DEFAULT '{}';
    ALTER TABLE designs ADD COLUMN tracked_from INTEGER NOT NULL DEFAULT 0;
    UPDATE designs SET tracked_from = rev;
    ALTER TABLE design_nodes ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
    UPDATE design_nodes SET rev = (SELECT rev FROM designs WHERE designs.id = design_nodes.design_id);
    CREATE INDEX design_nodes_rev ON design_nodes(design_id, rev);
    CREATE TABLE design_removed (
      design_id INTEGER NOT NULL REFERENCES designs(id) ON DELETE CASCADE,
      node_id TEXT NOT NULL, rev INTEGER NOT NULL,
      PRIMARY KEY (design_id, node_id)
    ) WITHOUT ROWID;
    UPDATE designs SET bytes = (SELECT COALESCE(SUM(length(CAST(body AS BLOB)) + length(CAST(node_id AS BLOB))
      + COALESCE(length(CAST(parent_id AS BLOB)), 0) + 21), 0) FROM design_nodes WHERE design_nodes.design_id = designs.id);
    "#,
    // v31 - Relay's icon set (`design.icon.*`, BUS.md §10.18): outline icons an agent or the
    // person saved, shared by every project's designs. An icon node that names one copies its
    // path into its own `icon_path`, so a design never depends on a row here to draw. Keywords
    // are one space-separated line.
    r#"
    CREATE TABLE design_icons (
      name TEXT PRIMARY KEY, path TEXT NOT NULL, keywords TEXT NOT NULL DEFAULT '',
      created_by TEXT NOT NULL, updated_by TEXT NOT NULL,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL
    ) WITHOUT ROWID;
    "#,
    // v32 - components (`component`, `instance_of` on nodes): whether a design has ever held a
    // main component, set by the write that made one and never cleared, so a design.apply of a
    // design without one skips the component rules on the row it reads anyway. Not a partial
    // index on the bodies: SQLite would test its condition on every node write, ~12,000
    // instructions each. No design held one before.
    r#"
    ALTER TABLE designs ADD COLUMN components INTEGER NOT NULL DEFAULT 0;
    "#,
    // v33 - charts that fill their laid-out parent: the ids of a design's chart frames whose width
    // or height fills, a JSON array kept by the writes that make one (and pruned as they stop), so
    // a design.apply lays out only the pages that hold one, and finds them without a pass over the
    // design (a partial index costs every node write, as v32 says). Nothing filled before.
    r#"
    ALTER TABLE designs ADD COLUMN fill_charts TEXT NOT NULL DEFAULT '[]';
    "#,
    // v34 - videos written as code (`video.*`, BUS.md §10.19, D167). A video is a folder of
    // Remotion code under `videos/` beside the store (`dir`), which agents edit with their own
    // tools; the row keeps what Relay knows of it: its compositions as the last check found them
    // (JSON), the props the person set per composition (JSON), and the newest version recorded.
    // A version is a manifest of the code's files (path, content hash, size) whose contents are
    // kept by hash in `videos/.relay/blobs/`. A render is a job and, once done, its file in the
    // folder's `out/`. Ids are AUTOINCREMENT, as designs' are, for the same reason (undo rows).
    r#"
    CREATE TABLE videos (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      session TEXT, title TEXT NOT NULL, dir TEXT NOT NULL,
      composition TEXT NOT NULL DEFAULT 'main',
      width INTEGER NOT NULL, height INTEGER NOT NULL, fps REAL NOT NULL, duration_frames INTEGER NOT NULL,
      compositions TEXT NOT NULL DEFAULT '[]', props TEXT NOT NULL DEFAULT '{}',
      version INTEGER NOT NULL DEFAULT 0, poster TEXT,
      created_by TEXT NOT NULL, updated_by TEXT NOT NULL,
      created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );
    CREATE INDEX videos_project ON videos(project_id, deleted_at, updated_at);

    CREATE TABLE video_versions (
      video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
      n INTEGER NOT NULL, digest TEXT NOT NULL, manifest TEXT NOT NULL, files INTEGER NOT NULL,
      actor TEXT NOT NULL, created_at TEXT NOT NULL,
      PRIMARY KEY (video_id, n)
    ) WITHOUT ROWID;

    CREATE TABLE video_renders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
      project_id INTEGER NOT NULL REFERENCES projects(id),
      composition TEXT NOT NULL,
      format TEXT NOT NULL CHECK (format IN ('mp4','webm','gif','prores','mp3','wav')),
      quality TEXT NOT NULL CHECK (quality IN ('preview','standard','high')),
      state TEXT NOT NULL CHECK (state IN ('queued','rendering','done','failed','cancelled')),
      progress REAL NOT NULL DEFAULT 0, version INTEGER NOT NULL,
      props TEXT NOT NULL DEFAULT '{}', range_from INTEGER, range_to INTEGER,
      path TEXT, bytes INTEGER, error TEXT, actor TEXT NOT NULL,
      created_at TEXT NOT NULL, started_at TEXT, finished_at TEXT
    );
    CREATE INDEX video_renders_video ON video_renders(video_id, id);
    CREATE INDEX video_renders_project ON video_renders(project_id);
    CREATE INDEX video_renders_state ON video_renders(state) WHERE state IN ('queued','rendering');
    "#,
    // v35 - a render's file name as asked for (`out/<name>.<ext>`), the loudness and true peak it
    // was asked to reach, and its sound as measured once done (JSON: `AudioReport`).
    r#"
    ALTER TABLE video_renders ADD COLUMN name TEXT;
    ALTER TABLE video_renders ADD COLUMN loudness REAL;
    ALTER TABLE video_renders ADD COLUMN peak REAL;
    ALTER TABLE video_renders ADD COLUMN audio TEXT;
    "#,
    // v36 - a video version's name (`video.version.label`): a take the person or an agent keeps,
    // listed however old. And the render whose file a render's now is, when a later one of the
    // same name wrote over it: filled in for those already rendered.
    r#"
    ALTER TABLE video_versions ADD COLUMN label TEXT;
    ALTER TABLE video_renders ADD COLUMN replaced_by INTEGER;
    UPDATE video_renders SET replaced_by = (SELECT MAX(r.id) FROM video_renders r
        WHERE r.video_id = video_renders.video_id AND r.path = video_renders.path AND r.id > video_renders.id AND r.state = 'done')
    WHERE state = 'done' AND path IS NOT NULL;
    "#,
];

pub struct Store {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Store {
    /// Open (creating if needed) and migrate. `exclusive` takes SQLite's EXCLUSIVE locking
    /// mode — the engine does (BUS.md §6.2); tests and tools do not.
    pub fn open(path: &Path, exclusive: bool) -> Result<Store> {
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            private_dir(dir)?;
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening {}", path.display()))?;
        // Before WAL mode: SQLite gives the -wal and -shm files the database file's mode.
        private_file(path);
        conn.pragma_update(None, "journal_mode", "WAL")?;
        for side in ["-wal", "-shm"] {
            let mut name = path.as_os_str().to_owned();
            name.push(side);
            if Path::new(&name).exists() { private_file(Path::new(&name)); }
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        if exclusive {
            conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        }
        tune(&conn)?;
        let store = Store {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        };
        store.migrate()?;
        Ok(store)
    }

    /// An in-memory store (tests).
    pub fn open_memory() -> Result<Store> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        tune(&conn)?;
        let store = Store {
            conn: Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.lock();
        let mut v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let from = v;
        anyhow::ensure!(
            v <= SCHEMA_VERSION,
            "store is schema v{v} but this build knows only v{SCHEMA_VERSION}; refusing to open"
        );
        if v > 0 && (v as usize) < MIGRATIONS.len() && self.path.is_file() {
            // SPEC §14: store.db copied on every version upgrade.
            match backup_to(&conn, &self.backup_dir(), "upgrade") {
                Ok(p) => {
                    tracing::info!(backup = %p.display(), from = v, to = SCHEMA_VERSION, "store backed up before upgrade")
                }
                Err(e) => anyhow::bail!(
                    "refusing to migrate v{v} → v{SCHEMA_VERSION}: pre-upgrade backup failed: {e}"
                ),
            }
        }
        while (v as usize) < MIGRATIONS.len() {
            let tx = conn.transaction()?;
            tx.execute_batch(MIGRATIONS[v as usize])?;
            v += 1;
            tx.pragma_update(None, "user_version", v)?;
            if v == 1 {
                tx.execute(
                    "INSERT INTO meta(key, value) VALUES ('created_at', ?1)",
                    [crate::time::now()],
                )?;
            }
            tx.commit()?;
        }
        // v27 slims every allowed hook row in place; the pages it empties stay in the file until
        // a VACUUM gives them back (260 MB -> 116 MB in 2.4 s on the store that prompted it,
        // migration included). Once, here, before the engine answers anyone.
        if from > 0 && from < 27 && v >= 27 && self.path.is_file() {
            let started = std::time::Instant::now();
            match conn.execute_batch("VACUUM") {
                Ok(()) => tracing::info!(ms = started.elapsed().as_millis() as u64, "store vacuumed after slimming hook audit rows"),
                Err(error) => tracing::warn!(error = %error, "vacuum after slimming hook audit rows failed; the space is reused, not returned"),
            }
        }
        Ok(())
    }

    pub fn version(&self) -> Result<i64> {
        Ok(self
            .lock()
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Lock the connection. Held for the duration of one request (BUS.md §5.1).
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Run `f` inside a transaction; commit on Ok, roll back on Err.
    pub fn with_tx<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// The backups directory beside the store.
    pub fn backup_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(|d| d.join("backups"))
            .unwrap_or_else(|| PathBuf::from("backups"))
    }

    /// Where installed skill folders live, beside the store: one folder per skill id, copied
    /// into every project root and session worktree by [`crate::skills`].
    pub fn skills_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("skills"),
            None => std::env::temp_dir().join(format!("relay-skills-{}", std::process::id())),
        }
    }

    /// Where `design.asset.add` keeps pictures, beside the store: one file per content hash,
    /// named as an image node's `src`, shared by every design that shows it.
    pub fn design_assets_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("design-assets"),
            None => std::env::temp_dir().join(format!("relay-design-assets-{}", std::process::id())),
        }
    }

    /// Where `design.render` writes the pictures it answers with, beside the store: the newest
    /// few, each named by its render id. Per instance, and on disk rather than in the runtime dir
    /// (tmpfs), since they can add up to more than a little memory.
    pub fn design_renders_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("design-renders"),
            None => std::env::temp_dir().join(format!("relay-design-renders-{}", std::process::id())),
        }
    }

    /// Where videos live, beside the store (D167): the kit (`package.json`, `node_modules/`,
    /// `.relay/`) and one folder per project (`p<id>/`) holding a folder of code per video, so
    /// every video resolves the kit's packages by walking up.
    pub fn videos_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("videos"),
            None => std::env::temp_dir().join(format!("relay-videos-{}", std::process::id())),
        }
    }

    /// Where `video.still` writes the pictures it answers with, beside the store: the newest
    /// few, each named by a fresh id, as `design.render`'s are.
    pub fn video_stills_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("video-stills"),
            None => std::env::temp_dir().join(format!("relay-video-stills-{}", std::process::id())),
        }
    }

    /// Where the Pictures plugin keeps the search results' thumbnails it fetched, beside the
    /// store: a cache, swept of what has not been looked at for a week.
    pub fn picture_cache_dir(&self) -> PathBuf {
        match self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            Some(dir) => dir.join("picture-cache"),
            None => std::env::temp_dir().join(format!("relay-picture-cache-{}", std::process::id())),
        }
    }

    /// Copy the live database (SQLite online backup) into `backups/`, keep the newest
    /// [`KEEP_BACKUPS`] (and what [`prune_backups`] keeps past them). Returns the new file. Takes the store lock one step at a time, so other
    /// requests run between steps rather than waiting out the whole copy — do not call from
    /// inside a handler, which already holds it; use [`Store::backup_with`] there.
    pub fn backup(&self, reason: &str) -> Result<PathBuf> {
        Ok(self.backup_then(reason, |_| Ok(()))?.0)
    }

    /// [`Store::backup`], and `at_end` read through the store's connection in the same hold of
    /// the lock as the copy's last step and its finish, so what it reads is exactly what the copy
    /// holds: a write made through this connection between two steps is in the copy (SQLite
    /// updates the pages already copied), and only one made after `at_end` is missing from it.
    pub fn backup_then<T>(&self, reason: &str, at_end: impl FnOnce(&Connection) -> Result<T>) -> Result<(PathBuf, T)> {
        let dir = self.backup_dir();
        let dest = backup_dest(&dir, reason)?;
        let mut out = Connection::open(&dest)?;
        private_file(&dest);
        let copied = {
            let guard = self.lock();
            // SAFETY: the connection lives inside `self.conn` for as long as `self`, which this
            // borrow cannot outlive, and it never moves. The connection is opened NO_MUTEX, so
            // every call that touches it — `Backup::new`, each `step` and the `finish` in
            // `Backup`'s drop — is made below with the store mutex held, exactly as if it went
            // through the guard. Between steps another request may write through the same
            // connection; SQLite then updates the backup in place rather than restarting it.
            let src: &Connection = unsafe { &*(&*guard as *const Connection) };
            let backup = rusqlite::backup::Backup::new(src, &mut out)?;
            drop(guard);
            // The lock of the last step, still held: nothing is written between it, `at_end` and
            // the finish.
            let last = (|| -> Result<MutexGuard<'_, Connection>> {
                let mut busy = 0;
                loop {
                    let guard = self.lock();
                    let step = backup.step(BACKUP_STEP_PAGES)?;
                    if step == rusqlite::backup::StepResult::Done {
                        return Ok(guard);
                    }
                    drop(guard);
                    match step {
                        rusqlite::backup::StepResult::More => busy = 0,
                        _ => {
                            busy += 1;
                            anyhow::ensure!(busy < BACKUP_BUSY_RETRIES, "the store stayed busy for the whole backup");
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                    #[cfg(test)]
                    seam::BETWEEN_STEPS.with_borrow_mut(|hook| if let Some(hook) = hook { hook(&self.lock()) });
                    // Not `yield_now`: the store mutex is not fair, and a loop that re-locks at once
                    // wins it back before a woken waiter runs, so a request waited out several
                    // batches (14 ms behind a 64 MB copy). A millisecond off the lock hands it over.
                    std::thread::sleep(BACKUP_STEP_PAUSE);
                }
            })();
            let (guard, copied) = match last {
                Ok(guard) => {
                    let read = at_end(&guard);
                    (guard, read)
                }
                Err(error) => (self.lock(), Err(error)),
            };
            drop(backup);
            drop(guard);
            copied
        };
        drop(out);
        let read = match copied {
            Ok(read) => read,
            Err(error) => {
                let _ = std::fs::remove_file(&dest);
                return Err(error);
            }
        };
        let live = live_projects(&self.lock());
        prune_backups(&dir, live.as_ref())?;
        Ok((dest, read))
    }

    /// Same, using a connection the caller already holds (a handler's transaction).
    pub fn backup_with(&self, conn: &Connection, reason: &str) -> Result<PathBuf> {
        backup_to(conn, &self.backup_dir(), reason)
    }

    /// Existing backups, newest first: (path, bytes, created_at, reason).
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        list_backups(&self.backup_dir())
    }
}

/// Read tuning applied to every connection.
///
/// Handlers reach for `prepare_cached`, which keys compiled statements on their SQL text. Relay
/// has ~50 distinct literal statements and rusqlite's default cache holds 16, so without this the
/// cache thrashes and recompiles the same query on the next call. The page cache and mmap window
/// are sized for a store measured in tens of MB — small enough to hold hot pages outright.
fn tune(conn: &Connection) -> Result<()> {
    conn.set_prepared_statement_cache_capacity(128);
    conn.pragma_update(None, "cache_size", -32_000)?; // negative = KiB, so 32 MB
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    // mmap is a read path only; writes still go through the WAL.
    conn.pragma_update(None, "mmap_size", 268_435_456i64)?;
    Ok(())
}

/// How many backups of each reason `backups/` keeps (SPEC §14).
pub const KEEP_BACKUPS: usize = 5;

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub bytes: u64,
    pub created_at: String,
    pub reason: String,
}

/// Pages copied per step of [`Store::backup`]: 1024 × 4 KiB, a few ms of work under the lock.
const BACKUP_STEP_PAGES: std::os::raw::c_int = 1024;
/// How long [`Store::backup`] stays off the store lock between batches.
const BACKUP_STEP_PAUSE: std::time::Duration = std::time::Duration::from_millis(1);
/// Busy steps in a row before a backup gives up, 10 ms apart.
const BACKUP_BUSY_RETRIES: u32 = 200;

/// The store holds the audit log (request payloads, file contents, commands), notes and session
/// tokens: other local accounts must not read it where home directories are traversable
/// (RA-342). New directories are created 0700. An existing one is left as it is — the store's
/// directory may be one the caller chose — except `backups/`, which is always Relay's own.
fn private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
        .with_context(|| format!("creating {}", dir.display()))
}

/// 0600 on a file the store wrote. A failure (a store file someone else owns) is logged, not
/// fatal: an existing install must keep opening.
fn private_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        tracing::warn!(path = %path.display(), %error, "could not make the store file private");
    }
}

fn backup_dest(dir: &Path, reason: &str) -> Result<PathBuf> {
    private_dir(dir)?;
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)) {
            tracing::warn!(dir = %dir.display(), %error, "could not make the backups directory private");
        }
    }
    let stamp = crate::time::now().replace(':', "-");
    Ok(dir.join(format!("store-{stamp}-{reason}.db")))
}

/// The reasons whose backups are what `project.restore` restores from.
const REMOVAL_REASONS: &[&str] = &["project-remove", "workspace-remove"];

/// Keep the newest [`KEEP_BACKUPS`] of each reason, so a run of removals or manual backups never
/// pushes out the one taken before a schema upgrade — and, past those, every removal backup
/// that holds the newest copy of a project still removed. A `project-remove` backup is the only
/// copy of the project it was taken for: pruned by count alone, the sixth removal dropped the
/// first project from `project.removed.list`, and the next launch's recovery swept its
/// attachment files. `live` is the store's projects now; unknown, or a backup unreadable, no
/// removal backup is pruned. A removal backup goes once the projects it alone held are
/// restored. Removals are rare and asked for one at a time, so what this keeps grows slowly.
fn prune_backups(dir: &Path, live: Option<&std::collections::HashSet<i64>>) -> Result<()> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    // Removed projects a newer removal backup already holds.
    let mut held = std::collections::HashSet::new();
    for backup in list_backups(dir)? {
        let only_copy = REMOVAL_REASONS.contains(&backup.reason.as_str()) && match (live, backup_projects(&backup.path)) {
            (Some(live), Some(projects)) => {
                let removed: Vec<i64> = projects.into_iter().filter(|id| !live.contains(id)).collect();
                let only = removed.iter().any(|id| !held.contains(id));
                held.extend(removed);
                only
            }
            _ => true,
        };
        let count = seen.entry(backup.reason.clone()).or_default();
        *count += 1;
        if *count > KEEP_BACKUPS && !only_copy {
            let _ = remove_backup(&backup.path);
        }
    }
    Ok(())
}

/// Delete backup `path` and the `-wal` and `-shm` files beside it: a backup is a copy of a store
/// in WAL mode, so reading it (as [`backup_projects`] does) leaves them, which a read-only
/// connection cannot clear.
pub(crate) fn remove_backup(path: &Path) -> std::io::Result<()> {
    for side in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(side);
        match std::fs::remove_file(&name) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    std::fs::remove_file(path)
}

/// The ids of the projects the store holds; `None` when they cannot be read.
fn live_projects(conn: &Connection) -> Option<std::collections::HashSet<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM projects").ok()?;
    let ids = stmt.query_map([], |r| r.get(0)).ok()?.collect::<rusqlite::Result<_>>().ok();
    ids
}

/// The ids of the projects a backup holds, read through a read-only connection of its own.
fn backup_projects(path: &Path) -> Option<Vec<i64>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()?;
    let mut stmt = conn.prepare("SELECT id FROM projects").ok()?;
    let ids = stmt.query_map([], |r| r.get(0)).ok()?.collect::<rusqlite::Result<_>>().ok();
    ids
}

fn backup_to(conn: &Connection, dir: &Path, reason: &str) -> Result<PathBuf> {
    let dest = backup_dest(dir, reason)?;
    let mut out = Connection::open(&dest)?;
    private_file(&dest);
    let copied = (|| -> Result<()> {
        let bk = rusqlite::backup::Backup::new(conn, &mut out)?;
        let mut busy = 0;
        loop {
            match bk.step(-1)? {
                rusqlite::backup::StepResult::Done => return Ok(()),
                rusqlite::backup::StepResult::More => {}
                // The connection being copied has written in its open transaction: SQLite
                // answers LOCKED on every step until that commits, which no retry can wait out
                // from here. `run_to_completion` would spin forever.
                rusqlite::backup::StepResult::Locked => anyhow::bail!("cannot back up a connection with uncommitted writes; back up before writing"),
                _ => {
                    busy += 1;
                    anyhow::ensure!(busy < BACKUP_BUSY_RETRIES, "the store stayed busy for the whole backup");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    })();
    drop(out);
    if let Err(error) = copied {
        let _ = std::fs::remove_file(&dest);
        return Err(error);
    }
    prune_backups(dir, live_projects(conn).as_ref())?;
    Ok(dest)
}

fn list_backups(dir: &Path) -> Result<Vec<BackupInfo>> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(v);
    };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name
            .strip_prefix("store-")
            .and_then(|n| n.strip_suffix(".db"))
        else {
            continue;
        };
        // "<stamp>-<reason>": the stamp is RFC 3339 with ':' → '-', ends in 'Z'
        let Some(zi) = rest.find('Z') else { continue };
        let stamp = &rest[..=zi];
        let reason = rest[zi + 1..].trim_start_matches('-').to_string();
        let created_at = {
            // restore the colons in the time part: 2026-08-17T19-42-21.123Z
            let (date, time) = stamp.split_at(stamp.find('T').unwrap_or(stamp.len()));
            format!("{date}{}", time.replacen('-', ":", 2))
        };
        let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
        v.push(BackupInfo {
            path,
            bytes,
            created_at,
            reason,
        });
    }
    // Newest first by instant: the stamp's fractional seconds vary in width, so its text does
    // not sort in time order (`…21Z` would sort after `…21.5Z`).
    v.sort_by_cached_key(|b| std::cmp::Reverse((b.created_at.parse::<jiff::Timestamp>().ok(), b.created_at.clone())));
    Ok(v)
}

/// Test seams of [`Store::backup_then`].
#[cfg(test)]
pub(crate) mod seam {
    use std::cell::RefCell;

    pub(crate) type Hook = Box<dyn FnMut(&rusqlite::Connection)>;

    thread_local! {
        /// Run on the backing-up thread between two steps of a copy, with the store locked: a
        /// request landing mid-copy.
        pub(crate) static BETWEEN_STEPS: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store of more than one step of pages, so a copy of it has a between.
    pub(crate) fn ballast(store: &Store) {
        store.lock().execute_batch("CREATE TABLE ballast(b BLOB); INSERT INTO ballast VALUES (zeroblob(6000000));").unwrap();
    }

    fn audit_row(conn: &Connection, op: &str) -> i64 {
        conn.execute(
            "INSERT INTO audit (ts, req_id, actor, op, kind, payload_hash) VALUES ('2026-10-20T00:00:00Z', lower(hex(randomblob(16))), 'user', ?1, 'ok', 'h')",
            [op],
        ).unwrap();
        conn.last_insert_rowid()
    }

    /// B68: what `at_end` reads is what the copy holds. A write made through the store's
    /// connection while the copy runs is in it (SQLite updates the pages already copied), and
    /// below what `at_end` reads; one made after is in neither.
    #[test]
    fn a_backup_holds_what_was_written_during_it_and_its_end_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("store.db"), true).unwrap();
        ballast(&store);
        let before = audit_row(&store.lock(), "notes.create");
        let during = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = during.clone();
        seam::BETWEEN_STEPS.set(Some(Box::new(move |conn: &Connection| {
            if seen.get() == 0 { seen.set(audit_row(conn, "task.update")); }
        })));
        let (path, mark) = store.backup_then("test", |conn| Ok(conn.query_row("SELECT MAX(id) FROM audit", [], |r| r.get::<_, i64>(0))?)).unwrap();
        seam::BETWEEN_STEPS.set(None);
        let after = audit_row(&store.lock(), "task.update");
        assert!(during.get() > before, "the copy had a between");
        assert_eq!(mark, during.get(), "the end reads the write made during the copy");
        let copy = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let ids: Vec<i64> = copy.prepare("SELECT id FROM audit ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(ids, vec![before, during.get()], "the copy holds the write made during it, not the one after");
        assert!(after > mark);
        let blob: i64 = copy.query_row("SELECT length(b) FROM ballast", [], |r| r.get(0)).unwrap();
        assert_eq!(blob, 6_000_000, "the copy is whole");
    }

    /// A backup is a copy of a store in WAL mode: reading it leaves a `-wal` and a `-shm` beside
    /// it, which go when it is pruned.
    #[test]
    fn a_pruned_backup_takes_the_files_a_read_of_it_left() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("store.db"), true).unwrap();
        let first = store.backup("manual").unwrap();
        Connection::open_with_flags(&first, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap().query_row("SELECT COUNT(*) FROM audit", [], |r| r.get::<_, i64>(0)).unwrap();
        let sides = |path: &Path| ["-wal", "-shm"].map(|side| { let mut name = path.as_os_str().to_owned(); name.push(side); PathBuf::from(name) });
        assert!(sides(&first).iter().any(|side| side.exists()), "a read leaves its files: {:?}", std::fs::read_dir(dir.path().join("backups")).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
        for _ in 0..KEEP_BACKUPS {
            store.backup("manual").unwrap();
        }
        assert!(!first.exists(), "the oldest past {KEEP_BACKUPS} is pruned");
        assert!(sides(&first).iter().all(|side| !side.exists()), "and what a read of it left");
    }
}
