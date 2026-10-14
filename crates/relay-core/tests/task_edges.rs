//! Edges of the task, note and v3-import handlers that the low-severity audit found open
//! (RA-381 … RA-418): undo that changed nothing, files left behind by a refused request, an
//! agent reaching past its project or its column rules. Every call crosses the bus door.

use base64::Engine as _;
use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use relay_core::{Instance, Store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}
fn call(engine: &Engine, actor: Actor, op: &str, payload: Value) -> Response {
    engine.dispatch(Request::new(actor, op, payload), Door::InProcess)
}
fn ok_as(engine: &Engine, actor: Actor, op: &str, payload: Value) -> Value {
    call(engine, actor, op, payload)
        .into_result()
        .unwrap_or_else(|e| panic!("{op}: {} {}", e.code, e.message))
}
fn ok(engine: &Engine, op: &str, payload: Value) -> Value {
    ok_as(engine, Actor::User, op, payload)
}
fn code(response: Response) -> String {
    response.error.expect("expected error").code
}
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { out.extend(files_under(&path)) } else { out.push(path) }
    }
    out
}
fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn fake_provider(dir: &Path, binary: &str) -> PathBuf {
    let path = dir.join(binary);
    std::fs::write(
        &path,
        "#!/bin/sh\ncase \"${1:-}\" in\n  --version) echo 'fixture 1.0'; exit 0;;\n  auth|login) echo '{\"loggedIn\":true}'; exit 0;;\nesac\necho hello-from-pty\nwhile IFS= read -r line; do echo \"echo:$line\"; done\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "edges@relay.test"]);
    git(path, &["config", "user.name", "Edges"]);
    std::fs::write(path.join("README.md"), "edges\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "init"]);
}

struct Fixture {
    root: tempfile::TempDir,
    engine: Arc<Engine>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        repo(&ws.join("app"));
        let engine = Engine::new(
            Instance::Test,
            Store::open(&root.path().join("store/store.db"), false).unwrap(),
        );
        ok(&engine, "workspace.create", json!({"path":ws}));
        ok(&engine, "project.add", json!({"workspace_id":1,"path":ws.join("app")}));
        let claude = fake_provider(root.path(), "claude");
        ok(&engine, "settings.set", json!({"path":"providers.claude.path","value":claude}));
        Self { root, engine }
    }
    fn task(&self, title: &str, extra: Value) -> Value {
        let mut payload = json!({"project_id":1,"title":title});
        let map = payload.as_object_mut().unwrap();
        for (k, v) in extra.as_object().unwrap() {
            map.insert(k.clone(), v.clone());
        }
        ok(&self.engine, "task.create", payload)
    }
    fn attachments(&self) -> PathBuf {
        self.root.path().join("store/attachments")
    }
    fn last_audit(&self, op: &str) -> Value {
        ok(&self.engine, "audit.list", json!({"op_prefix":op,"limit":1}))["rows"][0].clone()
    }
}

/// RA-418: re-linking an edge that is already there records no undo, so undoing it cannot
/// drop the edge; and a new duplicate_of replaces one that points at a deleted task.
#[test]
fn relate_undo_and_duplicate_replacement() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = f.task("A", json!({}));
    let b = f.task("B", json!({}));
    let c = f.task("C", json!({}));
    ok(e, "task.relate", json!({"task_id":a["id"],"relation":"blocked_by","other_id":b["id"]}));
    let first = f.last_audit("task.relate");
    ok(e, "task.relate", json!({"task_id":a["id"],"relation":"blocked_by","other_id":b["id"]}));
    let second = f.last_audit("task.relate");
    assert_ne!(first["id"], second["id"]);
    assert_eq!(code(call(e, Actor::User, "audit.undo", json!({"audit_id":second["id"]}))), "audit.not_undoable");
    assert_eq!(ok(e, "task.get", json!({"task_id":a["id"]}))["blocked_by"], json!([b["id"]]));

    ok(e, "task.relate", json!({"task_id":a["id"],"relation":"duplicate_of","other_id":b["id"]}));
    ok(e, "task.delete", json!({"task_id":b["id"]}));
    ok(e, "task.relate", json!({"task_id":a["id"],"relation":"duplicate_of","other_id":c["id"]}));
    ok(e, "task.restore", json!({"task_id":b["id"]}));
    assert_eq!(ok(e, "task.get", json!({"task_id":a["id"]}))["duplicate_of"], c["id"]);
    let edges: i64 = e.store.lock().query_row(
        "SELECT COUNT(*) FROM task_relations WHERE from_task=?1 AND rel='duplicate_of'",
        [a["id"].as_i64().unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(edges, 1);
}

/// RA-417: parent.set's position is an index in the task's column, clamped; a huge one no
/// longer breaks every later append to the column, and undo puts the order back.
#[test]
fn parent_set_position_is_a_clamped_column_index() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Parent", json!({}));
    let x = f.task("X", json!({}));
    let y = f.task("Y", json!({}));
    let column = |e: &Engine| -> Vec<i64> {
        ok(e, "task.list", json!({"project_id":1,"column":"backlog"}))["tasks"].as_array().unwrap()
            .iter().map(|t| t["id"].as_i64().unwrap()).collect()
    };
    let before = column(e);
    ok(e, "task.parent.set", json!({"task_id":y["id"],"parent_id":parent["id"],"position":0}));
    assert_eq!(column(e)[0], y["id"].as_i64().unwrap());
    ok(e, "audit.undo", json!({"audit_id":f.last_audit("task.parent.set")["id"]}));
    assert_eq!(column(e), before);
    assert!(ok(e, "task.get", json!({"task_id":y["id"]}))["parent_id"].is_null());

    ok(e, "task.parent.set", json!({"task_id":x["id"],"parent_id":parent["id"],"position":i64::MAX}));
    let z = f.task("Z", json!({}));
    assert_eq!(*column(e).last().unwrap(), z["id"].as_i64().unwrap());
}

/// RA-414 and RA-409: undoing a detach brings the attachment back under its own name and
/// type, and a create refused part-way leaves no attachment file behind.
#[test]
fn attachment_undo_and_refused_create() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Shot", json!({}));
    let attached = ok(e, "task.attach", json!({"task_id":task["id"],"name":"shot.png","mime":"image/png","bytes_b64":b64(b"png")}));
    let files = files_under(&f.attachments()).len();
    ok(e, "task.detach", json!({"task_id":task["id"],"attachment_id":attached["id"]}));
    let detached = f.last_audit("task.detach");
    assert_eq!(ok(e, "task.get", json!({"task_id":task["id"]}))["attachments"], json!([]));
    assert!(ok(e, "task.list", json!({"project_id":1}))["tasks"][0]["attachments"].as_array().unwrap().is_empty());
    assert_eq!(code(call(e, Actor::User, "task.detach", json!({"task_id":task["id"],"attachment_id":attached["id"]}))), "task.attachment_not_found");
    assert!(Path::new(attached["path"].as_str().unwrap()).is_file(), "a detach keeps the file for its undo");
    ok(e, "audit.undo", json!({"audit_id":detached["id"]}));
    // RA-414: the same row comes back — id, path and all — and no second copy of the file.
    let back = &ok(e, "task.get", json!({"task_id":task["id"]}))["attachments"][0];
    assert_eq!(back, &attached);
    assert_eq!(files_under(&f.attachments()).len(), files);
    assert_eq!(code(call(e, Actor::User, "task.attachment.restore", json!({"task_id":task["id"],"attachment_id":attached["id"]}))), "task.attachment_not_found");
    // Redo detaches it again; restore called directly brings it back too.
    ok(e, "audit.undo", json!({"audit_id":f.last_audit("audit.undo")["id"]}));
    assert_eq!(ok(e, "task.get", json!({"task_id":task["id"]}))["attachments"], json!([]));
    assert_eq!(ok(e, "task.attachment.restore", json!({"task_id":task["id"],"attachment_id":attached["id"]})), attached);

    let stored = files_under(&f.attachments()).len();
    let refused = call(e, Actor::User, "task.create", json!({"project_id":1,"title":"Two","attachments":[
        {"name":"one.txt","mime":"text/plain","bytes_b64":b64(b"one")},
        {"name":"two.txt","mime":"text/plain","bytes_b64":"not base64!"},
    ]}));
    assert_eq!(code(refused), "task.attachment_base64");
    let left: Vec<PathBuf> = files_under(&f.attachments()).into_iter().filter(|p| !p.starts_with(f.attachments().join(".staging"))).collect();
    assert_eq!(left.len(), stored, "{left:?}");
}

/// RA-410 and RA-411: an agent cannot create a task straight into active or done, and its
/// task.list and task.label.list stop at its own project.
#[test]
fn agent_task_create_and_reads_stay_in_bounds() {
    let f = Fixture::new();
    let e = &f.engine;
    repo(&f.root.path().join("ws/other"));
    ok(e, "project.add", json!({"workspace_id":1,"path":f.root.path().join("ws/other")}));
    ok(e, "task.create", json!({"project_id":2,"title":"Elsewhere","labels":["secret"]}));
    f.task("Here", json!({}));
    let session = ok(e, "session.create", json!({"project_id":1,"provider":"claude","role":"builder"}));
    let agent = Actor::agent(session["name"].as_str().unwrap());

    let listed = ok_as(e, agent.clone(), "task.list", json!({}));
    let titles: Vec<&str> = listed["tasks"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Here"]);
    assert_eq!(code(call(e, agent.clone(), "task.list", json!({"project_id":2,"include_deleted":true}))), "actor.scope");
    assert_eq!(code(call(e, agent.clone(), "task.label.list", json!({"project_id":2}))), "actor.scope");
    ok_as(e, agent.clone(), "task.label.list", json!({"project_id":1}));
    assert_eq!(ok(e, "task.list", json!({}))["tasks"].as_array().unwrap().len(), 2, "the person still sees every project");

    let mut roles = ok(e, "settings.get", json!({"path":"guardrails.roles.builder"}))["value"].clone();
    roles.as_array_mut().unwrap().push(json!("task.create"));
    ok(e, "settings.set", json!({"path":"guardrails.roles.builder","value":roles}));
    for extra in [json!({"column":"done"}), json!({"column":"active"}), json!({"state":"running"})] {
        let mut payload = json!({"project_id":1,"title":"Shortcut"});
        payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        assert_eq!(code(call(e, agent.clone(), "task.create", payload)), "task.column_transition");
    }
    ok_as(e, agent, "task.create", json!({"project_id":1,"title":"Filed","column":"ready"}));
    ok(e, "task.create", json!({"project_id":1,"title":"Escape hatch","column":"done"}));
}

/// RA-386 / RA-411: an agent's module reads stop at its own project (D106); module.list
/// without a project is its own.
#[test]
fn agent_module_reads_stay_in_its_project() {
    let f = Fixture::new();
    let e = &f.engine;
    repo(&f.root.path().join("ws/other"));
    ok(e, "project.add", json!({"workspace_id":1,"path":f.root.path().join("ws/other")}));
    let here = ok(e, "module.create", json!({"project_id":1,"name":"Here"}));
    let there = ok(e, "module.create", json!({"project_id":2,"name":"Elsewhere"}));
    let session = ok(e, "session.create", json!({"project_id":1,"provider":"claude","role":"builder"}));
    let agent = Actor::agent(session["name"].as_str().unwrap());

    let listed = ok_as(e, agent.clone(), "module.list", json!({}));
    let names: Vec<&str> = listed["modules"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Here"]);
    ok_as(e, agent.clone(), "module.list", json!({"project_id":1}));
    ok_as(e, agent.clone(), "module.get", json!({"module_id":here["id"]}));
    ok_as(e, agent.clone(), "module.stats", json!({"project_id":1}));
    ok_as(e, agent.clone(), "module.changelog.draft", json!({"module_id":here["id"]}));
    for (op, payload) in [
        ("module.list", json!({"project_id":2})),
        ("module.stats", json!({"project_id":2})),
        ("module.get", json!({"module_id":there["id"]})),
        ("module.changelog.draft", json!({"module_id":there["id"]})),
    ] {
        assert_eq!(code(call(e, agent.clone(), op, payload)), "actor.scope", "{op}");
    }
    // The person reaches every project, and names the one they mean.
    ok(e, "module.get", json!({"module_id":there["id"]}));
    ok(e, "module.stats", json!({"project_id":2}));
    assert_eq!(ok(e, "module.list", json!({"project_id":2}))["modules"][0]["name"], "Elsewhere");
    assert_eq!(code(call(e, Actor::User, "module.list", json!({}))), "module.project");
}

/// RA-415: a fanned sub-task does not inherit the parent's branch or prompt.
#[test]
fn fanout_children_get_their_own_branch_and_prompt() {
    let f = Fixture::new();
    let e = &f.engine;
    let parent = f.task("Body of work", json!({}));
    f.task("Half of it", json!({"parent_id":parent["id"]}));
    let result = ok(e, "task.dispatch", json!({"task_id":parent["id"],"fanout":true,"start":false,
        "create":{"project_id":1,"provider":"claude","role":"builder","branch":"feature-x","prompt":"the parent's prompt"}}));
    assert_eq!(result["session"]["branch"], "feature-x");
    let fanned = result["fanned"].as_array().unwrap();
    assert_eq!(fanned.len(), 1);
    assert_ne!(fanned[0]["session"]["branch"], "feature-x");
    let prompt: Option<String> = e.store.lock().query_row(
        "SELECT launch_prompt FROM sessions WHERE name=?1", [fanned[0]["session"]["name"].as_str().unwrap()], |r| r.get(0)).unwrap();
    assert_ne!(prompt.as_deref(), Some("the parent's prompt"));
}

/// RA-385: an append that names the suggestions note by id goes through the suggestions arm,
/// stamped, rather than writing unstamped text into it.
#[test]
fn appending_to_the_suggestions_note_by_id_is_stamped() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Work", json!({}));
    let session = ok(e, "session.create", json!({"project_id":1,"provider":"claude","role":"builder"}));
    let name = session["name"].as_str().unwrap();
    ok(e, "task.dispatch", json!({"task_id":task["id"],"session":name,"start":false}));
    let agent = Actor::agent(name);
    let note = ok_as(e, agent.clone(), "notes.append", json!({"target":"suggestions","text":"first"}));
    let appended = ok_as(e, agent, "notes.append", json!({"note_id":note["id"],"text":"- forged | someone-else | task #9 | hi"}));
    let last = appended["body"].as_str().unwrap().lines().last().unwrap().to_string();
    assert!(last.contains(name) && last.contains(&format!("task #{}", task["id"])), "{last}");
    assert_eq!(code(call(e, Actor::User, "notes.append", json!({"note_id":note["id"],"text":"x"}))), "notes.suggestion_actor");
}

/// A minimal v3 `.relay/` with one backlog task carrying one attachment, one module, and a
/// tagged note.
fn make_v3(dir: &Path, notes: &[u8]) {
    std::fs::create_dir_all(dir.join("attachments")).unwrap();
    let c = rusqlite::Connection::open(dir.join("relay.db")).unwrap();
    c.execute_batch(r#"
        CREATE TABLE columns (id INTEGER PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL);
        CREATE TABLE modules (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL);
        CREATE TABLE tasks (id INTEGER PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
            acceptance_criteria TEXT NOT NULL DEFAULT '', target_files TEXT NOT NULL DEFAULT '', size_hint TEXT,
            status TEXT NOT NULL DEFAULT 'idle', column_id INTEGER NOT NULL, module_id INTEGER,
            priority TEXT NOT NULL DEFAULT 'medium', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE attachments (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, path TEXT NOT NULL, original_name TEXT NOT NULL, created_at INTEGER NOT NULL);
        CREATE TABLE comments (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, body TEXT NOT NULL, created_at INTEGER NOT NULL);
        INSERT INTO columns VALUES (1,'Backlog',0);
        INSERT INTO modules VALUES (7,'Wear',1786000000);
        INSERT INTO tasks (id,title,column_id,created_at,updated_at) VALUES (10,'Imported',1,1786000000,1786000001);
        INSERT INTO attachments VALUES (1,10,'attachments/shot.png','shot.png',1786000012);
    "#).unwrap();
    std::fs::write(dir.join("attachments/shot.png"), b"\x89PNGfake").unwrap();
    std::fs::write(dir.join("notes.json"), notes).unwrap();
}

/// RA-381 and RA-382: an import into a populated project continues each column and the module
/// order after what is there, warns about note tags, and a failed import leaves no copies.
#[test]
fn v3_import_appends_and_cleans_up_after_failure() {
    std::env::set_var("RELAY_V3_SESSIONS_JSON", "/nonexistent/relay-v3-sessions.json");
    let f = Fixture::new();
    let e = &f.engine;
    let existing = f.task("Already here", json!({}));
    let module = ok(e, "module.create", json!({"project_id":1,"name":"Core"}));

    let broken = f.root.path().join("ws/app/broken");
    make_v3(&broken, b"\xff\xfe not utf-8");
    assert!(call(e, Actor::User, "app.import.v3", json!({"source":broken,"project_id":1})).error.is_some());
    assert!(files_under(&f.attachments()).is_empty(), "{:?}", files_under(&f.attachments()));

    let good = f.root.path().join("ws/app/good");
    make_v3(&good, json!([{"id":1,"title":"Tagged","body":"","tags":["ux"],"pinned":false}]).to_string().as_bytes());
    let out = ok(e, "app.import.v3", json!({"source":good,"project_id":1}));
    assert_eq!(files_under(&f.attachments()).len(), 1);
    let warnings = out["warnings"].to_string();
    assert!(warnings.contains("ux"), "{warnings}");
    let imported = ok(e, "task.get", json!({"task_id":out["id_map"]["tasks"]["10"]}));
    assert!(imported["position"].as_i64().unwrap() > existing["position"].as_i64().unwrap());
    let ord: i64 = e.store.lock().query_row("SELECT ord FROM modules WHERE id=?1", [out["id_map"]["modules"]["7"].as_i64().unwrap()], |r| r.get(0)).unwrap();
    assert!(ord > module["order"].as_i64().unwrap());
}

/// RA-413: `expected_updated_at` on the agent-reachable writes with no other optimistic check.
/// Given and current, the write lands; given and stale, it is refused with the code
/// task.update / notes.update use; omitted, nothing changes from before.
#[test]
fn changelog_and_append_take_an_optional_expected_updated_at() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Changelog", json!({}));
    let read = ok(e, "task.get", json!({"task_id":task["id"]}));
    let written = ok(e, "task.changelog.write", json!({"task_id":task["id"],"text":"first","expected_updated_at":read["updated_at"]}));
    assert_eq!(written["changelog"], "first");
    let stale = call(e, Actor::User, "task.changelog.write", json!({"task_id":task["id"],"text":"second","expected_updated_at":"2000-01-01T00:00:00Z"}));
    assert_eq!(code(stale), "task.edit_conflict");
    assert_eq!(ok(e, "task.get", json!({"task_id":task["id"]}))["changelog"], "first");
    assert_eq!(ok(e, "task.changelog.write", json!({"task_id":task["id"],"text":"third"}))["changelog"], "third");

    let note = ok(e, "notes.create", json!({"project_id":1,"title":"Log","body":"a"}));
    let appended = ok(e, "notes.append", json!({"note_id":note["id"],"text":"b","expected_updated_at":note["updated_at"]}));
    assert_eq!(appended["body"], "a\nb");
    let stale = call(e, Actor::User, "notes.append", json!({"note_id":note["id"],"text":"c","expected_updated_at":"2000-01-01T00:00:00Z"}));
    assert_eq!(code(stale), "notes.edit_conflict");
    assert_eq!(ok(e, "notes.get", json!({"note_id":note["id"]}))["body"], "a\nb");
    assert_eq!(ok(e, "notes.append", json!({"note_id":note["id"],"text":"d"}))["body"], "a\nb\nd");
    // The standing note, found from the project, is checked the same way.
    let standing = ok(e, "notes.append", json!({"project_id":1,"text":"rule"}));
    let stale = call(e, Actor::User, "notes.append", json!({"project_id":1,"text":"x","expected_updated_at":"2000-01-01T00:00:00Z"}));
    assert_eq!(code(stale), "notes.edit_conflict");
    ok(e, "notes.append", json!({"project_id":1,"text":"rule two","expected_updated_at":standing["updated_at"]}));
}

/// RA-409: the retention pass removes a detached attachment's row and file once its undo
/// window is past, and not before.
#[test]
fn retention_reclaims_detached_attachments_past_the_window() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Shots", json!({}));
    let old = ok(e, "task.attach", json!({"task_id":task["id"],"name":"old.png","mime":"image/png","bytes_b64":b64(b"old")}));
    let recent = ok(e, "task.attach", json!({"task_id":task["id"],"name":"new.png","mime":"image/png","bytes_b64":b64(b"new")}));
    let live = ok(e, "task.attach", json!({"task_id":task["id"],"name":"live.png","mime":"image/png","bytes_b64":b64(b"live")}));
    for a in [&old, &recent] {
        ok(e, "task.detach", json!({"task_id":task["id"],"attachment_id":a["id"]}));
    }
    e.store.lock().execute("UPDATE attachments SET deleted_at='2000-01-01T00:00:00Z' WHERE id=?1", [old["id"].as_i64().unwrap()]).unwrap();
    let out = ok(e, "app.reconcile", json!({}));
    assert!(out["actions"].to_string().contains("detached attachment"), "{out}");
    let path = |a: &Value| PathBuf::from(a["path"].as_str().unwrap());
    // The files go on a worker after the commit.
    for _ in 0..200 {
        if !path(&old).exists() { break }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!path(&old).exists());
    assert!(path(&recent).is_file() && path(&live).is_file());
    let rows: i64 = e.store.lock().query_row("SELECT COUNT(*) FROM attachments", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 2);
    ok(e, "task.attachment.restore", json!({"task_id":task["id"],"attachment_id":recent["id"]}));
    assert_eq!(code(call(e, Actor::User, "task.attachment.restore", json!({"task_id":task["id"],"attachment_id":old["id"]}))), "task.attachment_not_found");
}

/// RA-409: recovery removes attachment files nothing references — but not one a row (live or
/// detached) names, one an undoable pre-v24 detach would bring back, or one a removed project's
/// backup still holds for project.restore.
#[test]
fn recovery_sweeps_orphaned_attachment_files() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Shots", json!({}));
    let live = ok(e, "task.attach", json!({"task_id":task["id"],"name":"live.png","mime":"image/png","bytes_b64":b64(b"live")}));
    let detached = ok(e, "task.attach", json!({"task_id":task["id"],"name":"gone.png","mime":"image/png","bytes_b64":b64(b"gone")}));
    ok(e, "task.detach", json!({"task_id":task["id"],"attachment_id":detached["id"]}));
    let dir = f.attachments().join(task["id"].to_string());
    let orphan = dir.join("99-orphan.png");
    std::fs::write(&orphan, b"orphan").unwrap();
    let stray = f.attachments().join("424242");
    std::fs::create_dir_all(&stray).unwrap();
    std::fs::write(stray.join("1-stray.png"), b"stray").unwrap();
    let legacy = dir.join("98-legacy.png");
    std::fs::write(&legacy, b"legacy").unwrap();
    e.store.lock().execute(
        "INSERT INTO audit(ts, req_id, actor, op, project_id, kind, payload_hash, undo_op) VALUES (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'legacy-detach', 'user', 'task.detach', 1, 'ok', 'x', ?1)",
        [json!({"op":"task.attach","payload":{"task_id":task["id"],"path":legacy,"name":"legacy.png","mime":"image/png"}}).to_string()],
    ).unwrap();
    // A fresh staged copy may belong to an attach in flight.
    let staged = f.attachments().join(".staging/in-flight");
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    std::fs::write(&staged, b"x").unwrap();

    let report = relay_core::recovery::run(e).unwrap();
    assert!(report.fsck_fixes.iter().any(|x| x == "removed 2 orphaned attachment file(s)"), "{:?}", report.fsck_fixes);
    assert!(!orphan.exists() && !stray.exists(), "orphans and their empty directory go");
    for kept in [PathBuf::from(live["path"].as_str().unwrap()), PathBuf::from(detached["path"].as_str().unwrap()), legacy.clone(), staged.clone()] {
        assert!(kept.is_file(), "{kept:?} was removed");
    }

    // A removed project's files wait for its backup: project.restore needs them.
    ok(e, "project.remove", json!({"project_id":1}));
    let report = relay_core::recovery::run(e).unwrap();
    assert!(!report.fsck_fixes.iter().any(|x| x.contains("attachment")), "{:?}", report.fsck_fixes);
    assert!(PathBuf::from(live["path"].as_str().unwrap()).is_file());
    for backup in e.store.list_backups().unwrap() {
        std::fs::remove_file(backup.path).unwrap();
    }
    relay_core::recovery::run(e).unwrap();
    assert!(!PathBuf::from(live["path"].as_str().unwrap()).exists());
    assert!(!PathBuf::from(detached["path"].as_str().unwrap()).exists());
    assert!(legacy.is_file(), "the undoable audit row still names it");
}

/// RA-412: task.list hydrates its page in one statement per side table. Every task it returns
/// must read exactly as task.get returns it: sessions, commits, attachments (not detached
/// ones), labels, relations, children and roll-up.
#[test]
fn task_list_hydrates_each_task_as_task_get_does() {
    let f = Fixture::new();
    let e = &f.engine;
    let a = f.task("A", json!({"labels":["ux","Backend"],"column":"ready"}));
    let b = f.task("B", json!({"labels":["ux"],"parent_id":a["id"]}));
    let c = f.task("C", json!({"parent_id":b["id"]}));
    let d = f.task("D", json!({"column":"in_review"}));
    let plain = f.task("Plain", json!({}));
    let session = ok(e, "session.create", json!({"project_id":1,"provider":"claude","worktree":"primary","task_id":a["id"]}));
    ok(e, "task.dispatch", json!({"task_id":d["id"],"session":session["name"],"start":false}));
    ok(e, "task.link_commit", json!({"task_id":a["id"],"sha":"aaa","branch":"main"}));
    ok(e, "task.link_commit", json!({"task_id":a["id"],"sha":"bbb"}));
    ok(e, "task.approve", json!({"task_id":c["id"],"sha":"ccc"}));
    for (task, name) in [(&a, "one.png"), (&a, "two.png"), (&b, "three.png")] {
        ok(e, "task.attach", json!({"task_id":task["id"],"name":name,"mime":"image/png","bytes_b64":b64(name.as_bytes())}));
    }
    let gone = ok(e, "task.attach", json!({"task_id":d["id"],"name":"gone.png","mime":"image/png","bytes_b64":b64(b"gone")}));
    ok(e, "task.detach", json!({"task_id":d["id"],"attachment_id":gone["id"]}));
    ok(e, "task.relate", json!({"task_id":a["id"],"relation":"blocked_by","other_id":d["id"]}));
    ok(e, "task.relate", json!({"task_id":plain["id"],"relation":"blocked_by","other_id":d["id"]}));
    ok(e, "task.relate", json!({"task_id":b["id"],"relation":"duplicate_of","other_id":plain["id"]}));

    let listed = ok(e, "task.list", json!({"project_id":1}))["tasks"].as_array().unwrap().clone();
    assert_eq!(listed.len(), 5);
    for task in &listed {
        assert_eq!(task, &ok(e, "task.get", json!({"task_id":task["id"]})));
    }
    let get = |t: &Value| ok(e, "task.get", json!({"task_id":t["id"]}));
    let a = get(&a);
    assert!(!a["sessions"].as_array().unwrap().is_empty(), "{a}");
    assert_eq!(a["commits"].as_array().unwrap().len(), 2);
    assert_eq!(a["attachments"].as_array().unwrap().len(), 2);
    assert_eq!(a["labels"], json!(["Backend", "ux"]));
    assert_eq!(a["children"], json!([b["id"]]));
    assert_eq!(a["rollup"], json!({"total":2,"done":1}));
    assert_eq!(get(&d)["blocks"], json!([a["id"], plain["id"]]));
    assert_eq!(get(&d)["attachments"], json!([]));
    assert!(!get(&d)["sessions"].as_array().unwrap().is_empty());
    assert_eq!(get(&b)["duplicate_of"], plain["id"]);
    assert_eq!(get(&c)["depth"], 2);
    // A page is hydrated as a whole too.
    let page = ok(e, "task.list", json!({"project_id":1,"limit":2,"offset":1}))["tasks"].as_array().unwrap().clone();
    assert_eq!(page, listed[1..3]);
}

/// RA-416: undoing task.approve is task.unapprove. The task returns to its column, slot and
/// state, and the commit link the approval added goes; a link that was there before stays.
#[test]
fn undoing_approve_restores_the_task_and_drops_only_its_own_link() {
    let f = Fixture::new();
    let e = &f.engine;
    let ids = |e: &Engine| -> Vec<i64> {
        ok(e, "task.list", json!({"project_id":1,"column":"in_review"}))["tasks"].as_array().unwrap()
            .iter().map(|t| t["id"].as_i64().unwrap()).collect()
    };
    let a = f.task("A", json!({"column":"in_review"}));
    let b = f.task("B", json!({"column":"in_review"}));
    let c = f.task("C", json!({"column":"in_review"}));
    ok(e, "task.update", json!({"task_id":b["id"],"state":"awaiting_review"}));
    let before = ids(e);
    let done = ok(e, "task.approve", json!({"task_id":b["id"],"sha":"cafe"}));
    assert_eq!(done["column"], "done");
    assert_eq!(done["state"], "none");
    let row = f.last_audit("task.approve");
    assert_eq!(row["undo_op"]["op"], "task.unapprove");
    ok(e, "audit.undo", json!({"audit_id":row["id"]}));
    let back = ok(e, "task.get", json!({"task_id":b["id"]}));
    assert_eq!(back["column"], "in_review");
    assert_eq!(back["state"], "awaiting_review");
    assert_eq!(back["commits"], json!([]));
    assert_eq!(ids(e), before);
    assert_eq!(before, [a["id"].as_i64().unwrap(), b["id"].as_i64().unwrap(), c["id"].as_i64().unwrap()]);
    // Undoing the undo is a redo: back to done with the same commit.
    ok(e, "audit.undo", json!({"audit_id":f.last_audit("audit.undo")["id"]}));
    let again = ok(e, "task.get", json!({"task_id":b["id"]}));
    assert_eq!(again["column"], "done");
    assert_eq!(again["commits"][0]["sha"], "cafe");

    // A commit linked before the approval is not the approval's to remove.
    ok(e, "task.link_commit", json!({"task_id":a["id"],"sha":"beef","branch":"relay/a"}));
    ok(e, "task.approve", json!({"task_id":a["id"],"sha":"beef"}));
    ok(e, "audit.undo", json!({"audit_id":f.last_audit("task.approve")["id"]}));
    let kept = ok(e, "task.get", json!({"task_id":a["id"]}));
    assert_eq!(kept["column"], "in_review");
    assert_eq!(kept["commits"].as_array().unwrap().len(), 1);
    assert_eq!(kept["commits"][0]["sha"], "beef");

    // Called directly it refuses a task that is not done.
    assert_eq!(code(call(e, Actor::User, "task.unapprove", json!({"task_id":c["id"],"column":"ready","position":0,"state":"none"}))), "task.column_transition");
}

/// D166: a task remembers the note it was made from. The note must be live and in the task's
/// project; `task.list {note_id}` finds the tasks made from it, and purging the note clears the
/// link rather than the task.
#[test]
fn a_task_made_from_a_note_keeps_the_link() {
    let f = Fixture::new();
    let e = &f.engine;
    let note = ok(e, "notes.create", json!({"project_id":1,"title":"Idea","body":"thinking"}));
    let task = f.task("From the note", json!({"note_id":note["id"]}));
    assert_eq!(task["note_id"], note["id"]);
    assert_eq!(ok(e, "task.get", json!({"task_id":task["id"]}))["note_id"], note["id"]);
    let plain = f.task("Unrelated", json!({}));
    assert!(plain["note_id"].is_null());

    let made: Vec<Value> = ok(e, "task.list", json!({"project_id":1,"note_id":note["id"]}))["tasks"].as_array().unwrap().clone();
    assert_eq!(made.len(), 1);
    assert_eq!(made[0]["id"], task["id"]);
    assert_eq!(made[0]["note_id"], note["id"]);

    // A missing note, one in another project, and a deleted one are refused.
    repo(&f.root.path().join("ws/other"));
    ok(e, "project.add", json!({"workspace_id":1,"path":f.root.path().join("ws/other")}));
    let elsewhere = ok(e, "notes.create", json!({"project_id":2,"title":"Theirs","body":""}));
    let gone = ok(e, "notes.create", json!({"project_id":1,"title":"Gone","body":""}));
    ok(e, "notes.delete", json!({"note_id":gone["id"]}));
    for note_id in [json!(9999), elsewhere["id"].clone(), gone["id"].clone()] {
        let refused = call(e, Actor::User, "task.create", json!({"project_id":1,"title":"Nope","note_id":note_id}));
        assert_eq!(code(refused), "notes.not_found", "note {note_id}");
    }
    assert_eq!(ok(e, "task.list", json!({"project_id":1}))["tasks"].as_array().unwrap().len(), 2, "a refused create left a task");

    // Purging the note past the undo window keeps the task and clears its link.
    ok(e, "notes.delete", json!({"note_id":note["id"]}));
    e.store.lock().execute("UPDATE notes SET deleted_at='2000-01-01T00:00:00Z' WHERE id=?1", [note["id"].as_i64().unwrap()]).unwrap();
    ok(e, "app.reconcile", json!({}));
    let after = ok(e, "task.get", json!({"task_id":task["id"]}));
    assert!(after["note_id"].is_null(), "{after}");
    assert!(after["deleted_at"].is_null());
}

/// Rows store an attachment's whole path, and the store's directory can be reached by more than
/// one spelling (a moved or symlinked data directory). Under another one no row matched any
/// file, and recovery deleted every attachment. A row outside the folder scanned now stops the
/// sweep; under the folder's own spelling, files are matched by `<task>/<file>`.
#[test]
fn recovery_keeps_attachments_whose_rows_spell_the_store_directory_differently() {
    let f = Fixture::new();
    let e = &f.engine;
    let task = f.task("Shots", json!({}));
    let live = ok(e, "task.attach", json!({"task_id":task["id"],"name":"live.png","mime":"image/png","bytes_b64":b64(b"live")}));
    let path = PathBuf::from(live["path"].as_str().unwrap());
    let orphan = path.parent().unwrap().join("99-orphan.png");
    std::fs::write(&orphan, b"orphan").unwrap();
    let alias = f.root.path().join("alias");
    std::os::unix::fs::symlink(f.root.path().join("store"), &alias).unwrap();
    let elsewhere = alias.join("attachments").join(path.strip_prefix(f.attachments()).unwrap());
    let set_path = |to: &Path| {
        e.store.lock().execute("UPDATE attachments SET path=?1 WHERE id=?2", rusqlite::params![to.display().to_string(), live["id"].as_i64().unwrap()]).unwrap();
    };
    set_path(&elsewhere);
    let report = relay_core::recovery::run(e).unwrap();
    assert!(!report.fsck_fixes.iter().any(|x| x.contains("attachment")), "{:?}", report.fsck_fixes);
    assert!(path.is_file() && orphan.is_file(), "a row under another spelling had its files swept");

    set_path(&path);
    let report = relay_core::recovery::run(e).unwrap();
    assert!(report.fsck_fixes.iter().any(|x| x == "removed 1 orphaned attachment file(s)"), "{:?}", report.fsck_fixes);
    assert!(path.is_file() && !orphan.exists());
}

/// A fanned dispatch whose transaction fails after its prepare made the sessions — here a
/// sub-task deleted in between — closes them and their checkouts, as a failed prepare does.
/// The deletion lands on the first session's event, while the prepare still makes the other
/// two; should the dispatch win that race anyway, the round is run again.
#[test]
fn a_dispatch_that_fails_to_commit_closes_the_sessions_it_made() {
    let mut failed = false;
    for _ in 0..5 {
        let f = Fixture::new();
        let e = &f.engine;
        let parent = f.task("Body of work", json!({}));
        f.task("First half", json!({"parent_id":parent["id"]}));
        let second = f.task("Second half", json!({"parent_id":parent["id"]}));
        let mut rx = e.subscribe();
        let engine = e.clone();
        let second_id = second["id"].as_i64().unwrap();
        let deleter = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while std::time::Instant::now() < deadline {
                match rx.try_recv() {
                    Ok(event) if event.ev == "session.changed" => {
                        engine.store.lock().execute("UPDATE tasks SET deleted_at='2026-01-01T00:00:00Z' WHERE id=?1", [second_id]).unwrap();
                        return;
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            }
        });
        let out = call(e, Actor::User, "task.dispatch", json!({"task_id":parent["id"],"fanout":true,"start":false,
            "create":{"project_id":1,"provider":"claude","role":"builder"}}));
        deleter.join().unwrap();
        let open = || -> Vec<(String, String)> {
            let conn = e.store.lock();
            let mut stmt = conn.prepare("SELECT name, worktree FROM sessions WHERE state != 'closed'").unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
        };
        if out.ok {
            // The dispatch read its sub-tasks before the deletion: every session it made has a task.
            assert_eq!(open().len(), 3);
            continue;
        }
        failed = true;
        let made: i64 = e.store.lock().query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(made, 3, "the prepare made all three before the transaction failed");
        let worktrees: Vec<String> = e.store.lock().prepare("SELECT worktree FROM sessions").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        let gone = || open().is_empty() && worktrees.iter().all(|worktree| !Path::new(worktree).exists());
        // Closed on a thread of their own, once the failed transaction lets go of the store.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !gone() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(open(), vec![], "no session outlives the dispatch that made it");
        for worktree in &worktrees {
            assert!(!Path::new(worktree).exists(), "{worktree} goes with its session");
        }
        assert_eq!(ok(e, "task.get", json!({"task_id":parent["id"]}))["column"], "backlog");
        break;
    }
    assert!(failed, "the deletion never landed between the prepare and the transaction");
}
