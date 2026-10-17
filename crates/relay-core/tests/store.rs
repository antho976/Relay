//! Phase 2: migrations at every prior version, upgrade backups, backup ops, and the v3 importer.

mod common;

use common::{call, call_as};
use relay_bus::Actor;
use relay_core::engine::Engine;
use relay_core::store::{MIGRATIONS, SCHEMA_VERSION};
use relay_core::{Instance, Store};
use serde_json::json;
use std::sync::Arc;

/// SHA-256 of every migration that has shipped, by version. A store in the wild was built by
/// exactly this text, so the test below rebuilds old stores from it: if a shipped migration is
/// edited, those rebuilds stop matching any real store and the test proves nothing. Never edit
/// one — append a new migration (and its hash here) instead. A version that has not shipped
/// yet may still change; update its hash with it.
const SHIPPED_MIGRATIONS: &[&str] = &[
    "3822a76b947b8b42c12f1a4927f0002f614b5c9caadeb901de7e6d849a99c67b", // v1
    "0a79d47ecb22a13afe2e0f7e67dcce3f6888642abc1ab2342bf10b69cae8a1dc", // v2
    "531a1949c26351b8b76d51e769d8ff840f49545e7e4b927feff0a504a0edec86", // v3
    "d340613c5d56e8f7827d874f730c6b5bfdb98db8ab9685473def73c8d229a181", // v4
    "33b81b0d70123844652deac1cd4866bd1dd2b25759ae2f82be61866c79cdfcdb", // v5
    "d51daa43dc93895b4b2f7039d2fcca77b2de453af457730fb7e9ef69dc023e36", // v6
    "49f0625694cf2518dd96371f3e11f4f3557e427216f1acc6c5e1ea84f1061bed", // v7
    "edfd6f65719e7a9025bd521e8cfb32b8a417a2c683a959464063b37c9092e5d1", // v8
    "808b62553c8c0cab053f02200dd89fb6090a3257005c5e76f3ba6f8c7c141c35", // v9
    "92db2e1dcb85c03714cbfe21cd65c35e2680eedcc0e0f9b032aee4b7dd50889e", // v10
    "8501046b07e691979867dc734ada0f3cb6a6b8bb93c28c9697be2543a5c1eba2", // v11
    "a57514c2ad1b9990bd5413d3a21df8a236c9c40a56d91cff9683c5bf4fd42c60", // v12
    "0a0a9507f8b057536664d7b611b0c88d472480b9d314f174978614700fbbc1d1", // v13
    "c6b5848f5977b9ff522b39e99444072b2b5f578473349f96df591e19547a367a", // v14
    "cb1d0f1dca5040b2777ffd2c26f7ecbf3147111e22638c7e1124ae3c0aa540e9", // v15
    "1531c3890ae1d6263f4b5cb112a71b98139c499578a1a7eb90d65ba84c558382", // v16
    "8cd59b0caadc346f4a2df8d8f7f0dac48219e0ac34af306ffdc08c14d956d783", // v17
    "9a57275a79ceed0b1fd7c3a1b9c36bac2ce5c66035d92ca79c0003c21cfedf4a", // v18
    "7767c3211ab0dbdbb83cc4571fccc3530ae1ffcb021a5bb0708c63fdb05214d6", // v19
    "8a0cb54342844cf8d525cf58e77f562853bdaa7bd311dc67e570a7118feb3c28", // v20
    "62681b138e192280c0e12cbb2a9abf5c7c4fde9506337682d16d091e3f46b39e", // v21
    "91c29598127eea523390db607475e48c670a6cfc4cbfd3eb3cf5e8ebb95a0e49", // v22
    "f59a2bedad6eca0af4f82d20b22ffc35a5befc24a1348f0bca8e63ce87448bbb", // v23
    "9686cbabdf0e0ed0d5a321c149cf8477480d03f05584f58daa07d85c89760c69", // v24
    "51f4a2bec4fc08130563d5421142abfa94e2ca5829160f7d9a53a4a2965eae4a", // v25
    "2f790e39553e4eb153998e34e45c2f3ccbc042455ab83893712c31853715ccea", // v26
    "4eee63135435765dc7b578d0501029fb4dca69a46c68fb26464f2b0cf82bb941", // v27
    "9953a804e621ca8b21b96fd0d8dfea578a75610f07f5f32e3af4c7656835daa5", // v28
];

#[test]
fn shipped_migrations_are_never_edited() {
    use sha2::{Digest, Sha256};
    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let hash: String = Sha256::digest(migration.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let v = i + 1;
        let Some(&shipped) = SHIPPED_MIGRATIONS.get(i) else {
            panic!("v{v} is new: add its hash to SHIPPED_MIGRATIONS ({hash})");
        };
        assert_eq!(hash, shipped, "migration v{v} was edited after it shipped; append a new migration instead");
    }
    assert_eq!(SHIPPED_MIGRATIONS.len(), MIGRATIONS.len(), "a shipped migration was removed");
}

/// Two rows in every table that exists at this version, parents before children, with every
/// foreign key pointing at a real row and every `CHECK (col IN (...))` satisfied — the shape of
/// a store that has been used, so a migration that only works on empty tables fails here.
/// Returns the row count per table.
fn seed(c: &rusqlite::Connection, k: usize) -> Vec<(String, i64)> {
    let mut st = c.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").unwrap();
    let mut pending: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
    let mut done: Vec<String> = Vec::new();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|(table, sql)| {
            let mut fk = c.prepare(&format!("PRAGMA foreign_key_list({table})")).unwrap();
            // (referenced table, from column)
            let fks: Vec<(String, String)> = fk.query_map([], |r| Ok((r.get(2)?, r.get(3)?))).unwrap().map(|r| r.unwrap()).collect();
            if fks.iter().any(|(parent, _)| parent != table && !done.contains(parent)) {
                return true;
            }
            let mut ti = c.prepare(&format!("PRAGMA table_info({table})")).unwrap();
            // (name, declared type, not null, default, pk)
            let cols: Vec<(String, String, bool, Option<String>, i64)> = ti
                .query_map([], |r| Ok((r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
                .unwrap().map(|r| r.unwrap()).collect();
            for i in 1..=2i64 {
                let mut names = Vec::new();
                let mut values: Vec<rusqlite::types::Value> = Vec::new();
                for (name, ty, not_null, default, pk) in &cols {
                    let parent = fks.iter().find(|(_, from)| from == name).map(|(parent, _)| parent);
                    let value = if let Some(parent) = parent {
                        // A self-reference (undo_of, parent_id) stays NULL: one level is enough.
                        if parent == table { continue; }
                        // Both rows share their first parent (two tasks in one project) unless
                        // it is part of the key; any later one differs, so keys stay distinct.
                        let first = cols.iter().map(|c| &c.0)
                            .find(|col| fks.iter().any(|(p, from)| p != table && from == *col)) == Some(name);
                        rusqlite::types::Value::Integer(if first && *pk == 0 { 1 } else { i })
                    } else if let Some(literal) = check_literal(sql, name, i as usize - 1) {
                        literal
                    } else if *pk > 0 && ty.eq_ignore_ascii_case("INTEGER") {
                        rusqlite::types::Value::Integer(i)
                    } else if (!*not_null || default.is_some()) && *pk == 0 {
                        continue;
                    } else if ty.eq_ignore_ascii_case("INTEGER") {
                        rusqlite::types::Value::Integer(i)
                    } else if name.ends_with("_at") || name == "ts" || name.ends_with("_seen") {
                        rusqlite::types::Value::Text(format!("2026-0{i}-01T00:00:00.000Z"))
                    } else if ty.eq_ignore_ascii_case("BLOB") {
                        rusqlite::types::Value::Blob(format!("v{k} {table} {i}").into_bytes())
                    } else {
                        rusqlite::types::Value::Text(format!("v{k}-{table}-{name}-{i}"))
                    };
                    names.push(name.clone());
                    values.push(value);
                }
                let marks = vec!["?"; names.len()].join(",");
                c.execute(&format!("INSERT INTO {table}({}) VALUES ({marks})", names.join(",")), rusqlite::params_from_iter(values))
                    .unwrap_or_else(|e| panic!("seeding {table} at v{k}: {e}"));
            }
            done.push(table.clone());
            false
        });
        assert!(pending.len() < before, "foreign keys between {pending:?} cannot be ordered at v{k}");
    }
    done.into_iter().map(|t| { let n = c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap(); (t, n) }).collect()
}

/// The `nth` literal (wrapping) a `CHECK (col IN (...))` allows, wherever the check is written.
fn check_literal(sql: &str, col: &str, nth: usize) -> Option<rusqlite::types::Value> {
    let at = sql.find(&format!("CHECK ({col} IN ("))
        .or_else(|| sql.find(&format!("{col} IS NULL OR {col} IN (")))?;
    let list = &sql[at..];
    let open = list.find("IN (")? + 4;
    let allowed: Vec<&str> = list[open..list[open..].find(')')? + open].split(',').map(str::trim).collect();
    let literal = allowed[nth % allowed.len()];
    Some(match literal.parse::<i64>() {
        Ok(n) => rusqlite::types::Value::Integer(n),
        Err(_) => rusqlite::types::Value::Text(literal.trim_matches('\'').to_string()),
    })
}

/// Build a DB at schema version `k` the way a build of that era would have — its migrations,
/// then rows in every table — and open it with the current build: it must migrate to the
/// latest version, keep every row, and pass integrity_check and foreign_key_check.
#[test]
fn every_prior_version_migrates_forward() {
    assert_eq!(MIGRATIONS.len() as i64, SCHEMA_VERSION);
    for k in 1..=MIGRATIONS.len() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.db");
        let seeded = {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.pragma_update(None, "foreign_keys", "ON").unwrap();
            for m in &MIGRATIONS[..k] {
                c.execute_batch(m).unwrap();
            }
            c.pragma_update(None, "user_version", k as i64).unwrap();
            c.execute("INSERT INTO meta(key, value) VALUES ('created_at', 'then')", []).unwrap();
            let rows = seed(&c, k);
            if (11..22).contains(&k) {
                // Skills installed from GitHub before v22 recorded no ref; seed leaves the
                // nullable source columns empty, so give both rows one.
                c.execute("UPDATE skills SET source_url='https://github.com/o/r.git', source_path='skills/'||id||'/SKILL.md'", []).unwrap();
            }
            rows
        };
        let s = Store::open(&path, false).unwrap_or_else(|e| panic!("opening a v{k} store: {e}"));
        assert_eq!(s.version().unwrap(), SCHEMA_VERSION, "v{k} did not migrate to latest");
        let ok: String = s.lock().query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
        assert_eq!(ok, "ok");
        {
            let conn = s.lock();
            let dangling: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
            assert_eq!(dangling, 0, "v{k}: the upgrade left foreign keys pointing nowhere");
            for (table, rows) in &seeded {
                let now: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap();
                assert_eq!(now, *rows, "v{k}: {table} lost rows in the upgrade");
            }
            if k < 14 {
                // v14 backfills the queue order from the task id for assignments that predate it.
                let off: i64 = conn.query_row("SELECT COUNT(*) FROM task_sessions WHERE queue_ord != task_id", [], |r| r.get(0)).unwrap();
                assert_eq!(off, 0, "v{k}: existing assignments were not given a queue order");
            }
            if (11..22).contains(&k) {
                // v22: what was cloned then was the default branch, which a NULL ref still means.
                let sourced: i64 = conn.query_row("SELECT COUNT(*) FROM skills WHERE source_url IS NOT NULL AND source_ref IS NULL", [], |r| r.get(0)).unwrap();
                assert_eq!(sourced, 2, "v{k}: installed skills lost their GitHub source");
            }
            if k < 23 {
                // v23: SQLite cannot hash, so a hold from before is hashed when first read.
                let unhashed: i64 = conn.query_row("SELECT COUNT(*) FROM holds WHERE payload_hash IS NULL", [], |r| r.get(0)).unwrap();
                assert_eq!(unhashed, 2, "v{k}: a hold from before v23 must read as not yet hashed");
            }
            if k < 24 {
                // v24: every attachment from before soft-delete existed is live.
                let gone: i64 = conn.query_row("SELECT COUNT(*) FROM attachments WHERE deleted_at IS NOT NULL", [], |r| r.get(0)).unwrap();
                assert_eq!(gone, 0, "v{k}: an attachment from before v24 must read as live");
            }
        }
        // every table has its FK indexes: no FK column without an index (SPEC §1)
        let conn = s.lock();
        let mut st = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").unwrap();
        let tables: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        for t in tables {
            let mut fk = conn.prepare(&format!("PRAGMA foreign_key_list({t})")).unwrap();
            let cols: Vec<String> = fk.query_map([], |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
            for col in cols {
                let mut il = conn.prepare(&format!("PRAGMA index_list({t})")).unwrap();
                let idx: Vec<String> = il.query_map([], |r| r.get::<_, String>(1)).unwrap().map(|r| r.unwrap()).collect();
                let covered = idx.iter().any(|i| {
                    let mut ii = conn.prepare(&format!("PRAGMA index_info({i})")).unwrap();
                    let first: Option<String> = ii.query_map([], |r| r.get::<_, String>(2)).unwrap().next().map(|r| r.unwrap());
                    first.as_deref() == Some(col.as_str())
                });
                assert!(covered, "{t}.{col} is a foreign key with no index leading on it (v{k})");
            }
        }
        if k < MIGRATIONS.len() {
            // upgrading an existing file made a backup first
            let backups = s.list_backups().unwrap();
            assert_eq!(backups.len(), 1, "one upgrade backup for v{k}");
            assert_eq!(backups[0].reason, "upgrade");
        }
    }
}

#[test]
fn backups_via_bus_keep_last_five() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("store.db"), false).unwrap();
    let e = Engine::new(Instance::Test, store);
    // Two older backups already on disk: a manual one, which retention must count with the
    // new ones and drop first, and an upgrade one, which is kept by reason (RA-678).
    let backups = e.store.backup_dir();
    std::fs::create_dir_all(&backups).unwrap();
    let old_manual = backups.join("store-2000-01-01T00-00-00Z-manual.db");
    let old_upgrade = backups.join("store-2000-01-01T00-00-00Z-upgrade.db");
    std::fs::write(&old_manual, b"").unwrap();
    std::fs::write(&old_upgrade, b"").unwrap();
    let mut made = Vec::new();
    for _ in 0..7 {
        let r = call(&e, "app.backup.now", json!({})).into_result().unwrap();
        assert!(r["bytes"].as_u64().unwrap() > 0);
        made.push(r["path"].as_str().unwrap().to_string());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let l = call(&e, "app.backup.list", json!({})).into_result().unwrap();
    let listed = |reason: &str| -> Vec<String> {
        l["backups"].as_array().unwrap().iter()
            .filter(|b| b["reason"] == reason)
            .map(|b| b["path"].as_str().unwrap().to_string())
            .collect()
    };
    // The five kept are the last five made, newest first; the planted one and the first two
    // made are gone from disk, not just from the list.
    let newest: Vec<String> = made[2..].iter().rev().cloned().collect();
    assert_eq!(listed("manual"), newest);
    for gone in [&old_manual.display().to_string(), &made[0], &made[1]] {
        assert!(!std::path::Path::new(gone).exists(), "{gone} outlived retention");
    }
    assert_eq!(listed("upgrade"), vec![old_upgrade.display().to_string()], "manual backups never push out an upgrade one");
    // an agent may not back up
    let r = call_as(&e, Actor::agent("x"), "app.backup.now", json!({}));
    assert_eq!(r.error.unwrap().code, "actor.allowlist");
}

/// RA-342: the store, its WAL and its backups hold audit payloads and session tokens. A new data
/// dir and `backups/` are 0700, the files 0600 — and an existing 0644 store is tightened on open.
#[test]
fn the_store_and_its_backups_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("relay-v4/test");
    let path = data.join("store.db");
    let store = Store::open(&path, false).unwrap();
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&path), 0o600);
    store.with_tx(|tx| { tx.execute("INSERT INTO meta(key, value) VALUES ('probe', 'x')", [])?; Ok(()) }).unwrap();
    for side in ["store.db-wal", "store.db-shm"] {
        let side = data.join(side);
        if side.exists() { assert_eq!(mode(&side), 0o600, "{}", side.display()); }
    }
    let backup = store.backup("manual").unwrap();
    assert_eq!(mode(backup.parent().unwrap()), 0o700);
    assert_eq!(mode(&backup), 0o600);
    drop(store);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    drop(Store::open(&path, false).unwrap());
    assert_eq!(mode(&path), 0o600, "an existing install's store is tightened");
}

/// A v3 `.relay/` dir the way v3 wrote it (schema copied from a real relay.db).
fn make_v3(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("attachments")).unwrap();
    let c = rusqlite::Connection::open(dir.join("relay.db")).unwrap();
    // v3 never turned foreign_keys on, so dangling module refs are possible in the wild
    c.execute_batch(r#"
        PRAGMA foreign_keys = OFF;
        CREATE TABLE columns (id INTEGER PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL);
        CREATE TABLE modules (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL);
        CREATE TABLE tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
            acceptance_criteria TEXT NOT NULL DEFAULT '', target_files TEXT NOT NULL DEFAULT '', size_hint TEXT,
            status TEXT NOT NULL DEFAULT 'idle', column_id INTEGER NOT NULL REFERENCES columns(id),
            module_id INTEGER REFERENCES modules(id), priority TEXT NOT NULL DEFAULT 'medium',
            created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
        CREATE TABLE runs (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, branch TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'running',
            session_id TEXT, cost_usd REAL, log_path TEXT, note TEXT, started_at INTEGER NOT NULL, finished_at INTEGER);
        CREATE TABLE attachments (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, path TEXT NOT NULL, original_name TEXT NOT NULL, created_at INTEGER NOT NULL);
        CREATE TABLE comments (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, body TEXT NOT NULL, created_at INTEGER NOT NULL);
        INSERT INTO columns VALUES (1,'Backlog',0),(2,'Queue',3),(3,'Ready',2),(4,'Done',4),(5,'In review',1),(6,'Someday',5);
        INSERT INTO modules VALUES (7,'Wear',1786000000),(9,'Academy',1786000100);
        INSERT INTO tasks (id,title,description,acceptance_criteria,target_files,size_hint,status,column_id,module_id,priority,created_at,updated_at)
          VALUES (10,'First','Do the thing','It works','a.kt, b.kt','large','approved',4,7,'medium',1786000000,1786000001),
                 (11,'Second','','','',NULL,'idle',1,NULL,'urgent',1786000002,1786000003),
                 (12,'Third','Body','','','small','running',2,9,'low',1786000004,1786000005),
                 (13,'Fourth','','','','medium','idle',6,42,'high',1786000006,1786000007);
        INSERT INTO comments VALUES (1,10,'looks good',1786000010),(2,10,'shipped',1786000011);
        INSERT INTO attachments VALUES (1,10,'attachments/shot.png','shot.png',1786000012),(2,11,'attachments/missing.png','missing.png',1786000013);
        INSERT INTO runs VALUES (1,10,'relay/t10','done','s1',0.5,NULL,NULL,1786000000,1786000100);
    "#).unwrap();
    std::fs::write(dir.join("attachments/shot.png"), b"\x89PNGfake").unwrap();
    std::fs::write(dir.join("notes.json"), json!([
        {"id": 1, "title": "Academy foundational rewrite", "body": "long text", "tags": [], "pinned": true, "created_at": 1786798899, "updated_at": 1786798899},
        {"id": 2, "title": null, "body": "", "tags": [], "pinned": false, "created_at": 1786838884, "updated_at": 1786838884}
    ]).to_string()).unwrap();
}

fn project_with_v3() -> (Arc<Engine>, tempfile::TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    make_v3(&repo.join(".relay"));
    let sessions = root.path().join("sessions.json");
    std::fs::write(&sessions, json!([
        {"id": "s1", "name": "Avex", "repo": std::fs::canonicalize(&repo).unwrap(), "max_files": 20, "max_lines": 1500, "agent": "claude"},
        {"id": "s2", "name": "Other", "repo": "/nowhere", "max_files": 5, "max_lines": 10}
    ]).to_string()).unwrap();
    std::env::set_var("RELAY_V3_SESSIONS_JSON", &sessions);
    let store_dir = root.path().join("store");
    let e = Engine::new(Instance::Test, Store::open(&store_dir.join("store.db"), false).unwrap());
    call(&e, "workspace.create", json!({"path": ws})).into_result().unwrap();
    call(&e, "project.add", json!({"workspace_id": 1, "path": repo})).into_result().unwrap();
    let src = repo.join(".relay").display().to_string();
    (e, root, src)
}

fn count(e: &Engine, sql: &str) -> i64 {
    e.store.lock().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn v3_import_dry_run_then_real() {
    let (e, _root, src) = project_with_v3();
    // dry run: full report, nothing written
    let r = call(&e, "app.import.v3", json!({"source": src, "project_id": 1, "dry_run": true})).into_result().unwrap();
    assert_eq!(r["counts"]["tasks"], 4);
    assert_eq!(r["counts"]["modules"], 2);
    assert_eq!(r["counts"]["notes"], 2);
    assert_eq!(r["counts"]["sessions"], 1);
    assert_eq!(count(&e, "SELECT COUNT(*) FROM tasks"), 0);
    let warnings = r["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
    assert!(warnings.contains("Someday"), "{warnings}");
    assert!(warnings.contains("missing module"), "{warnings}");
    assert!(warnings.contains("missing.png"), "{warnings}");
    assert!(warnings.contains("1 v3 runs"), "{warnings}");

    // real
    let r = call(&e, "app.import.v3", json!({"source": src, "project_id": 1})).into_result().unwrap();
    assert_eq!(r["counts"]["tasks"], 4);
    let map = &r["id_map"];
    let t10 = map["tasks"]["10"].as_i64().unwrap();
    let m7 = map["modules"]["7"].as_i64().unwrap();
    assert_eq!(count(&e, "SELECT COUNT(*) FROM tasks"), 4);
    assert_eq!(count(&e, "SELECT COUNT(*) FROM modules"), 2);
    assert_eq!(count(&e, "SELECT COUNT(*) FROM notes"), 2);
    assert_eq!(count(&e, "SELECT COUNT(*) FROM attachments"), 1);
    let conn = e.store.lock();
    let (col, state, prio, size, module_id, body): (String, String, String, Option<String>, Option<i64>, String) = conn
        .query_row("SELECT col, state, priority, size, module_id, body FROM tasks WHERE id = ?1", [t10],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).unwrap();
    assert_eq!((col.as_str(), state.as_str(), prio.as_str(), size.as_deref(), module_id), ("done", "none", "medium", Some("L"), Some(m7)));
    assert!(body.starts_with("Do the thing"), "{body}");
    assert!(body.contains("## Acceptance criteria\n\nIt works"), "{body}");
    assert!(body.contains("## Target files\n\na.kt, b.kt"), "{body}");
    assert!(body.contains("--- comments ---"), "{body}");
    assert!(body.contains("looks good") && body.contains("shipped"), "{body}");
    // column mapping: Queue → ready, Someday → backlog; priority urgent/low/high; state running
    let t12 = map["tasks"]["12"].as_i64().unwrap();
    let (col, state, prio, size): (String, String, String, Option<String>) = conn
        .query_row("SELECT col, state, priority, size FROM tasks WHERE id = ?1", [t12], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!((col.as_str(), state.as_str(), prio.as_str(), size.as_deref()), ("ready", "running", "low", Some("S")));
    let t13 = map["tasks"]["13"].as_i64().unwrap();
    let (col, module_id, prio): (String, Option<i64>, String) = conn
        .query_row("SELECT col, module_id, priority FROM tasks WHERE id = ?1", [t13], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!((col.as_str(), module_id, prio.as_str()), ("backlog", None, "high"));
    // timestamps converted
    let created: String = conn.query_row("SELECT created_at FROM tasks WHERE id = ?1", [t10], |r| r.get(0)).unwrap();
    assert!(created.starts_with("2026-08-0"), "{created}");
    // attachment copied under the store
    let path: String = conn.query_row("SELECT path FROM attachments WHERE task_id = ?1", [t10], |r| r.get(0)).unwrap();
    assert!(std::path::Path::new(&path).is_file());
    assert!(path.contains("/attachments/"));
    // notes
    let (title, pinned): (Option<String>, i64) = conn.query_row("SELECT title, pinned FROM notes ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(title.as_deref(), Some("Academy foundational rewrite"));
    assert_eq!(pinned, 1);
    drop(conn);
    // guardrail caps + project name from sessions.json
    let caps = call(&e, "settings.get", json!({"path": "guardrails.projects.1.caps"})).into_result().unwrap();
    assert_eq!(caps["value"]["files"], 20);
    assert_eq!(caps["value"]["lines"], 1500);
    let pr = call(&e, "project.get", json!({"project_id": 1})).into_result().unwrap();
    assert_eq!(pr["name"], "Avex");
    // audited with the id map in result_summary
    let rows = call(&e, "audit.list", json!({"op_prefix": "app.import"})).into_result().unwrap();
    let row = &rows["rows"][0];
    assert_eq!(row["kind"], "ok");
    assert_eq!(row["result_summary"]["id_map"]["tasks"]["10"], t10);
    // one-time
    let r = call(&e, "app.import.v3", json!({"source": src, "project_id": 1}));
    assert_eq!(r.error.unwrap().code, "import.already_done");
    // bad source / bad project
    let r = call(&e, "app.import.v3", json!({"source": "/nope", "project_id": 1}));
    assert_eq!(r.error.unwrap().code, "import.source");
    let r = call(&e, "app.import.v3", json!({"source": src, "project_id": 9}));
    assert_eq!(r.error.unwrap().code, "project.not_found");
}

/// Why every `LIMIT` in the engine is `?+0` and not `?`: SQLite re-prepares a statement whose
/// LIMIT is a bare parameter on every run, so `prepare_cached` saved nothing for it, while as an
/// expression the plan is the same and the statement is kept. If SQLite stops doing that, the
/// `+0`s can go; until then, no query here may bring a bare one back.
#[test]
fn no_limit_is_a_bare_parameter() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)").unwrap();
    let reprepared = |sql: &str| {
        let mut stmt = conn.prepare(sql).unwrap();
        for n in 1..=3 {
            stmt.query_map([n], |_| Ok(())).unwrap().for_each(drop);
        }
        stmt.get_status(rusqlite::StatementStatus::RePrepare)
    };
    assert_eq!(reprepared("SELECT id FROM t LIMIT ?"), 3, "SQLite no longer re-prepares a bare LIMIT parameter");
    assert_eq!(reprepared("SELECT id FROM t LIMIT ?+0"), 0);

    let bare = regex::Regex::new(r"LIMIT \?\d*[^\d+]").unwrap();
    let mut found = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (n, line) in text.lines().enumerate() {
                    if bare.is_match(line) && !line.trim_start().starts_with("//") {
                        found.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
    }
    assert!(found.is_empty(), "a bare LIMIT parameter: {found:?}");
}
