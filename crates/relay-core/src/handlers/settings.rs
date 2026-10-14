//! `settings.*` (BUS.md §10.16): a dotted-path tree stored as leaf rows, overlaid on defaults.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::guardrail::{self, ConfigScope};
use relay_bus::error::BusError;
use relay_bus::ops::notify::{SettingsGet, SettingsReset, SettingsSet, ValueOut};
use relay_bus::types::{GuardrailConfig, Id};
use rusqlite::{params, Connection, Transaction};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::sync::Mutex;

/// Defaults for the top-level keys BUS.md §10.16 names. Each feature fills its subtree in
/// when it lands; unknown paths are allowed (settings are a tree, not a schema). A key nothing
/// reads has no default: it would only advertise a setting that does nothing.
///
/// Guardrail evaluation reads this for every audited agent mutation, so the tree is built
/// once and handed out by reference; callers clone only the branch they overlay.
pub fn defaults() -> &'static Value {
    static DEFAULTS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        json!({
            "appearance": { "mode": "dark", "panel_alpha": 1.0, "wallpaper": null },
            "notifications": { "sound": "chime", "volume": 0.7, "categories": {} },
            "providers": { "claude": { "path": null }, "codex": { "path": null } },
            // What the status bar's limit meters show, and how often the window re-reads them
            // (minutes; 0 = only on engine events and the refresh key).
            "usage": {
                "refresh_minutes": 0,
                "claude": { "enabled": true, "five_hour": true, "weekly": true, "fable": true },
                "codex": { "enabled": true, "five_hour": true, "weekly": true }
            },
            // `mirror` (handlers/device.rs, BUS.md §10.14 "Mirror launch"): `max_fps` null follows
            // the device's refresh rate; `video_codec_options` is extra encoder keys
            // (`key[:type]=value,…`), null for none; `pin_refresh_rate` holds an adaptive panel
            // at its top rate while mirroring and puts the old value back after.
            "device": {
                "sdk_path": null, "adb_path": null, "emulator_path": null, "avdmanager_path": null,
                "mirror": { "max_fps": null, "video_codec_options": null, "pin_refresh_rate": false }
            },
            "guardrails": {
                "caps": { "files": 40, "lines": 2000 },
                "destructive_write": { "min_removed_lines": 50, "min_removed_pct": 40, "min_file_lines": 30, "allow_if_recoverable": true },
                "protected_paths": [],
                "agent_builds": false,
                "shape_gates": [],
                "denied_commands": ["rm -rf", "git reset --hard", "git clean -fd", "git push --force"],
                "allowed_write_roots": [],
                "roles": {
                    "builder": ["task.link_commit", "task.changelog.write", "task.comment", "task.update", "mailbox.*", "notes.append", "overlap.flag", "overlap.ack", "integration.request", "session.done", "session.report", "session.intent", "session.claim", "session.release", "session.repo.add", "device.claim", "device.release", "usage.report", "guardrail.gate"],
                    "reviewer": ["mailbox.*", "notes.append", "overlap.flag", "task.changelog.write", "task.comment", "session.done", "session.report", "session.intent", "session.claim", "session.release", "session.repo.add", "device.claim", "device.release", "usage.report"],
                    "docs": ["task.changelog.write", "task.comment", "task.update", "mailbox.*", "notes.append", "notes.create", "notes.update", "overlap.flag", "overlap.ack", "session.done", "session.report", "session.intent", "session.claim", "session.release", "session.repo.add", "device.claim", "device.release", "usage.report", "guardrail.gate"]
                },
                "projects": {},
            },
            // Threads' local models (local_models.rs): where their files are, llama-server
            // when it is not on PATH, and the minutes unused before it stops (0 = never).
            "threads": { "local": { "dir": null, "server": null, "idle_minutes": 15 } },
            "undo": { "grace_days": 7 },
            "audit": { "retention_days": 180 },
            // `layout.current.<project>` is the shell's last arrangement (ui.rs).
            "layout": {},
            "keybindings": {
                "palette": "Ctrl+K", "agents": "Ctrl+1", "code": "Ctrl+2", "board": "Ctrl+3",
                "new_session": "Ctrl+N", "settings": "Ctrl+,", "sidebar": "Ctrl+Shift+B"
            },
        })
    });
    &DEFAULTS
}

/// `""` is the root (whole tree); otherwise dotted segments of `[A-Za-z0-9_-]`.
fn valid_path(p: &str) -> Result<(), BusError> {
    if p.is_empty() {
        return Ok(());
    }
    if p.split('.').any(|s| s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
        return Err(BusError::invalid("settings.path", format!("bad settings path {p:?}")));
    }
    Ok(())
}

/// The leaf rows for `v` at `prefix`. Rows are keyed by dotted path, so an object key that is
/// empty or holds a `.` cannot be stored: `{"ctrl+.": …}` read back as `{"ctrl+": {"": …}}`
/// (RA-407). Any other key is kept as it is (`ctrl+k` is a keybinding).
fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) -> Result<(), BusError> {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                if k.is_empty() || k.contains('.') {
                    return Err(BusError::invalid("settings.key", format!("settings key {k:?} under {prefix:?} cannot be empty or contain '.'")));
                }
                let p = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&p, v, out)?;
            }
        }
        _ => out.push((prefix.to_string(), v.clone())),
    }
    Ok(())
}

fn set_at(root: &mut Value, path: &str, v: Value) {
    if path.is_empty() {
        *root = v;
        return;
    }
    let mut cur = root;
    let segs: Vec<&str> = path.split('.').collect();
    for (i, s) in segs.iter().enumerate() {
        if i == segs.len() - 1 {
            if !cur.is_object() {
                *cur = Value::Object(Map::new());
            }
            cur.as_object_mut().unwrap().insert(s.to_string(), v);
            return;
        }
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur.as_object_mut().unwrap().entry(s.to_string()).or_insert(Value::Object(Map::new()));
    }
}

/// RFC-7396-style object merge used by guardrail global/project configuration. Objects
/// recurse; arrays and scalars replace. `null` removes a key from the destination.
pub fn merge_value(dst: &mut Value, patch: &Value) {
    match (dst, patch) {
        (Value::Object(dst), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    dst.remove(key);
                } else {
                    merge_value(dst.entry(key.clone()).or_insert(Value::Null), value);
                }
            }
        }
        (dst, patch) => *dst = patch.clone(),
    }
}

fn get_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(root);
    }
    let mut cur = root;
    for s in path.split('.') {
        cur = cur.get(s)?;
    }
    Some(cur)
}

/// The full effective tree: defaults overlaid with stored leaves.
pub fn tree(tx: &Transaction) -> Result<Value, BusError> {
    let mut root = defaults().clone();
    let mut stmt = tx.prepare_cached("SELECT path, value FROM settings ORDER BY path").bus()?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .bus()?;
    for row in rows {
        let (p, v) = row.bus()?;
        let v: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
        set_at(&mut root, &p, v);
    }
    Ok(root)
}

/// The rows that can affect the value at `path`: the subtree under it, plus its ancestors —
/// a stored scalar at `appearance` replaces the whole default object below it, so it has to be
/// applied before `appearance.mode` is read back out.
///
/// Everything else in the table is irrelevant to `path`, and reading it is not free: a leaf read
/// of `appearance.mode` used to decode the megabytes parked in `appearance.wallpaper*` on its way
/// past. Ordering by path keeps the ancestor-then-descendant application order [`tree`] relies on
/// (a prefix always sorts before the paths it prefixes).
fn subtree(tx: &Transaction, path: &str) -> Result<Value, BusError> {
    let mut root = defaults().clone();
    let ancestors: Vec<String> = path
        .match_indices('.')
        .map(|(i, _)| path[..i].to_string())
        .collect();
    // The subtree as a case-sensitive range, as [`delete_under`] deletes it.
    let mut stmt = tx
        .prepare_cached(
            "SELECT path, value FROM settings
             WHERE path = ?1 OR (path > ?1 || '.' AND path < ?1 || '/')
                OR path IN (SELECT value FROM json_each(?2))
             ORDER BY path",
        )
        .bus()?;
    let rows = stmt
        .query_map(
            params![path, serde_json::to_string(&ancestors).bus()?],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .bus()?;
    for row in rows {
        let (p, v) = row.bus()?;
        let v: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
        set_at(&mut root, &p, v);
    }
    Ok(get_at(&root, path).cloned().unwrap_or(Value::Null))
}

pub fn get(tx: &Transaction, path: Option<&str>) -> Result<Value, BusError> {
    match path {
        None => tree(tx),
        Some("") => tree(tx),
        Some(p) => {
            valid_path(p)?;
            subtree(tx, p)
        }
    }
}

/// Delete the row at `path` and every leaf under it — the one subtree delete, for settings,
/// guardrail layers and the rows a removed project or workspace leaves. A range, not `LIKE`:
/// `LIKE` is ASCII case-insensitive, so `theme.dark` also took `theme.Dark.*` (RA-647). `.`
/// sorts just below `/`, so everything strictly between `path.` and `path/` is the subtree,
/// and the primary-key index serves it.
pub(crate) fn delete_under(conn: &Connection, path: &str) -> Result<(), BusError> {
    if path.is_empty() {
        conn.execute("DELETE FROM settings", []).bus()?;
        return Ok(());
    }
    conn.prepare_cached("DELETE FROM settings WHERE path = ?1 OR (path > ?1 || '.' AND path < ?1 || '/')")
        .bus()?
        .execute([path])
        .bus()?;
    Ok(())
}

// ---------------------------------------------------------------- guardrails

/// The guardrail layers a write at `path` stores: `(settings path, scope to read back, value)`.
/// Global keys are one layer; each `projects.<id>` and `workspaces.<id>` is its own. Empty
/// when the write does not reach `guardrails`.
fn guardrail_layers(path: &str, value: &Value) -> Result<Vec<(String, Option<ConfigScope>, Value)>, BusError> {
    if !(path.is_empty() || path == "guardrails" || path.starts_with("guardrails.")) {
        return Ok(Vec::new());
    }
    let mut tree = json!({});
    set_at(&mut tree, path, value.clone());
    let mut global = match tree.get_mut("guardrails").map(Value::take) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Object(map)) => map,
        Some(_) => return Err(BusError::invalid("guardrail.config", "guardrails must be an object")),
    };
    let mut layers = Vec::new();
    for kind in ["projects", "workspaces"] {
        let scope = |id: Id| if kind == "projects" { ConfigScope::Project(id) } else { ConfigScope::Workspace(id) };
        match global.remove(kind) {
            None | Some(Value::Null) => {}
            Some(Value::Object(by_id)) => {
                for (id, layer) in by_id {
                    // A layer holds overrides; a scalar in its place would replace the whole
                    // tree and fail every guardrail read for that scope (RA-318).
                    if !(layer.is_object() || layer.is_null()) {
                        return Err(BusError::invalid("guardrail.config", format!("guardrails.{kind}.{id} must be an object of overrides, not {layer}")));
                    }
                    layers.push((format!("guardrails.{kind}.{id}"), id.parse().ok().map(scope), layer));
                }
            }
            Some(_) => return Err(BusError::invalid("guardrail.config", format!("guardrails.{kind} must be an object keyed by id"))),
        }
    }
    layers.push(("guardrails".to_string(), Some(ConfigScope::Global), Value::Object(global)));
    Ok(layers)
}

/// `guardrails` is the one subtree with a fixed shape, and every guardrail check reads it. A key
/// the typed config does not know is refused at write time, from `settings.set` and
/// `guardrail.config.set` alike, so a typo cannot reach the store.
fn check_guardrail_keys(path: &str, value: &Value) -> Result<(), BusError> {
    for (at, _, mut layer) in guardrail_layers(path, value)? {
        let unknown = GuardrailConfig::strip_unknown(&mut layer);
        if !unknown.is_empty() {
            let keys: Vec<String> = unknown.iter().map(|key| format!("{at}.{key}")).collect();
            return Err(BusError::invalid(
                "guardrail.config",
                format!("unknown guardrail setting {}; guardrail.config.get shows the keys, guardrail.config.set writes them", keys.join(", ")),
            ));
        }
    }
    Ok(())
}

/// The read side of [`check_guardrail_keys`]: a stored key this build does not know is dropped
/// before the typed read, with one warning per key rather than one per read.
pub fn ignore_unknown_guardrail_keys(root: &mut Value) {
    static WARNED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    for key in GuardrailConfig::strip_unknown(root) {
        if WARNED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(key.clone()) {
            tracing::warn!(key = %key, "ignoring a stored guardrail setting this build does not know");
        }
    }
}

pub fn set(tx: &Transaction, path: &str, value: &Value, now: &str) -> Result<(), BusError> {
    valid_path(path)?;
    // The root is a tree of keys, never a value of its own: a row at "" is applied by a whole-tree
    // read but by no leaf read, so the two disagreed (RA-407).
    if path.is_empty() && value.as_object().is_none_or(|m| m.is_empty()) {
        return Err(BusError::invalid("settings.path", "the settings root can only be set to a non-empty object; settings.reset clears it"));
    }
    check_guardrail_keys(path, value)?;
    delete_under(tx, path)?;
    let mut leaves = Vec::new();
    flatten(path, value, &mut leaves)?;
    for (p, v) in leaves {
        tx.execute("INSERT INTO settings(path, value, updated_at) VALUES (?1, ?2, ?3)",
            params![p, serde_json::to_string(&v).bus()?, now]).bus()?;
    }
    Ok(())
}

/// Record the inverse of a settings write, unless the old value is past the audit's store
/// limit: the uncapped undo column would keep it for the whole retention window, and the only
/// settings that large are wallpaper libraries. Such a row is not undoable.
fn set_bounded_undo(ctx: &mut Ctx, path: &str, before: Value) {
    let size = serde_json::to_string(&before).map_or(usize::MAX, |s| s.len());
    if size <= crate::audit::STORE_LIMIT {
        ctx.set_undo("settings.set", json!({ "path": path, "value": before }), None);
    }
}

pub fn register(e: &mut Engine) {
    e.register::<SettingsGet>(|ctx, p| Ok(ValueOut { value: get(ctx.tx(), p.path.as_deref())? }));
    e.register::<SettingsSet>(|ctx: &mut Ctx, p| {
        let before = get(ctx.tx(), Some(&p.path))?;
        set(ctx.tx(), &p.path, &p.value, &ctx.now.clone())?;
        // Read the touched guardrail layers back, as guardrail.config.set does: a value of the
        // wrong type or out of range fails the typed read, and the transaction rolls back.
        for (_, scope, _) in guardrail_layers(&p.path, &p.value)? {
            match scope.map(|scope| guardrail::config_for(ctx.tx(), scope)) {
                Some(Err(error)) if error.code != "project.not_found" => return Err(error),
                _ => {}
            }
        }
        set_bounded_undo(ctx, &p.path, before);
        let value = get(ctx.tx(), Some(&p.path))?;
        ctx.emit("settings.changed", json!({ "path": p.path, "value": value }));
        Ok(ValueOut { value })
    });
    e.register::<SettingsReset>(|ctx: &mut Ctx, p| {
        let path = p.path.unwrap_or_default();
        valid_path(&path)?;
        let before = get(ctx.tx(), Some(&path))?;
        // No guardrail read-back here: a reset only removes overrides, and it is how a bad
        // `guardrails.*` row is cleared, so it must not depend on the rest reading cleanly.
        delete_under(ctx.tx(), &path)?;
        set_bounded_undo(ctx, &path, before);
        let value = get(ctx.tx(), Some(&path))?;
        ctx.emit("settings.changed", json!({ "path": path, "value": value }));
        Ok(ValueOut { value })
    });
}

#[cfg(test)]
mod tests {
    /// RA-647: a subtree delete and read take exactly the subtree, case-sensitively — not a
    /// sibling spelled in another case, not a sibling that merely shares the prefix.
    #[test]
    fn subtree_delete_and_read_match_the_prefix_exactly() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings (path TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL)").unwrap();
        let tx = conn.transaction().unwrap();
        for path in ["theme.dark", "theme.dark.bg", "theme.dark.fg.a", "theme.Dark.bg", "theme.dark_x.bg", "theme.darker", "theme.dark-x"] {
            tx.execute("INSERT INTO settings VALUES (?1, '1', 'now')", [path]).unwrap();
        }
        let read = super::subtree(&tx, "theme.dark").unwrap();
        assert_eq!(read, serde_json::json!({"bg": 1, "fg": {"a": 1}}));
        super::delete_under(&tx, "theme.dark").unwrap();
        let left: Vec<String> = tx.prepare("SELECT path FROM settings ORDER BY path").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(left, ["theme.Dark.bg", "theme.dark-x", "theme.dark_x.bg", "theme.darker"]);
    }

    /// A default is a promise that something reads the key (RA-253).
    #[test]
    fn defaults_carry_no_setting_that_nothing_reads() {
        let defaults = super::defaults();
        for dead in ["parking", "theme", "roles"] {
            assert!(defaults.get(dead).is_none(), "{dead} has a default but no reader");
        }
        assert!(defaults["appearance"].get("wallpaper_preview").is_none());
        assert!(defaults["layout"].is_object(), "ui.rs reads layout.current.<project>");
        assert!(defaults["guardrails"]["roles"].is_object(), "the role allow-sets live here");
    }

    /// Keys are path segments once stored, so one that would split differently is refused, as is
    /// a root that is not a tree (RA-407).
    #[test]
    fn a_value_that_would_not_read_back_is_refused() {
        let store = crate::Store::open_memory().unwrap();
        store.with_tx(|tx| {
            for (path, value) in [
                ("keybindings", serde_json::json!({"ctrl+.": {"op": "ui.toast"}})),
                ("theme", serde_json::json!({"a.b": 1, "a": {"b": 2}})),
                ("theme", serde_json::json!({"": 1})),
                ("", serde_json::json!({})),
                ("", serde_json::json!(1)),
            ] {
                let error = super::set(tx, path, &value, "t").unwrap_err();
                assert!(error.code == "settings.key" || error.code == "settings.path", "{path:?} {value}: {}", error.code);
            }
            super::set(tx, "keybindings", &serde_json::json!({"ctrl+k": {"op": "ui.page.switch"}}), "t").unwrap();
            assert_eq!(super::get(tx, Some("keybindings")).unwrap()["ctrl+k"]["op"], "ui.page.switch");
            Ok(())
        }).unwrap();
    }
}
