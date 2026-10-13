//! Installed skills as folders, not just instruction text (D147).
//!
//! A skill installed once belongs to the whole app: its folder is kept beside the store in
//! `<data>/skills/<id>/` and materialized into every project root and every session worktree
//! as a real provider skill (`.claude/skills/<name>/`, `.agents/skills/<name>/`), so an agent
//! in any project can load it. Relay only ever owns folders carrying [`MARKER`]; a skill the
//! repository checks in itself is left exactly as it is, and so is one another Relay store
//! (the dev instance beside the stable one) wrote.

use anyhow::{Context, Result};
use relay_bus::types::Id;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// Written into every skill folder Relay materializes. Its first line names the store that wrote
/// it and the rest is the freshness stamp; its absence means the folder belongs to the
/// repository and must not be touched.
pub const MARKER: &str = ".relay-skill";

/// Provider folders a materialized skill is written into, relative to a checkout root.
pub const TARGETS: [&str; 2] = [".claude/skills", ".agents/skills"];

/// Bounds on one adopted skill folder — a `SKILL.md` at a repository root would otherwise
/// pull the entire repository into the library.
const MAX_FILES: usize = 4_000;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 12;

/// One enabled skill as the materializer needs it.
struct Row {
    dir: String,
    body: String,
    stamp: String,
    source: Source,
}

/// Where a skill folder's files come from.
enum Source {
    /// An installed skill: its library folder beside the store, if it has one.
    Library(Id),
    /// A skill of a bundled plugin (D159): files relative to the skill folder.
    Plugin(Vec<(&'static str, &'static [u8])>),
}

/// `<store dir>/skills/<skill id>/` — the app-wide copy of one skill's folder.
pub fn library_dir(store: &crate::Store, id: Id) -> PathBuf {
    store.skills_dir().join(id.to_string())
}

/// Copy a downloaded skill folder into the library, replacing whatever was there. Callers
/// treat failure as "no assets": the instruction body alone still makes a usable skill. Call it
/// only once the row it belongs to is committed: a folder adopted for a transaction that then
/// rolls back is an orphan the next skill given the same id would inherit.
pub fn adopt(from: &Path, to: &Path) -> Result<()> {
    let _ = fs::remove_dir_all(to);
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    match copy_bounded(from, to) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_dir_all(to);
            Err(error)
        }
    }
}

/// Copy a folder, skipping `.git` and symlinks, refusing anything past the bounds above.
pub fn copy_bounded(from: &Path, to: &Path) -> Result<()> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    copy_into(from, to, 0, &mut files, &mut bytes)
}

fn copy_into(from: &Path, to: &Path, depth: usize, files: &mut usize, bytes: &mut u64) -> Result<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() || entry.file_name() == ".git" || entry.file_name() == MARKER {
            continue;
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_into(&entry.path(), &target, depth + 1, files, bytes)?;
            continue;
        }
        *files += 1;
        *bytes += entry.metadata()?.len();
        anyhow::ensure!(
            *files <= MAX_FILES && *bytes <= MAX_BYTES,
            "{} exceeds the skill folder bounds ({MAX_FILES} files, {} MiB)",
            from.display(),
            MAX_BYTES / (1024 * 1024)
        );
        fs::copy(entry.path(), &target)
            .with_context(|| format!("copying {}", entry.path().display()))?;
    }
    Ok(())
}

/// Folder name for a skill: the public name, reduced to something every provider can read.
pub fn folder_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars().take(64) {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "skill".into() } else { out }
}

/// The folder of every live installed skill, by id. Distinct names can reduce to one folder
/// (`Review: carefully` and `review carefully`), and two skills sharing it would rewrite each
/// other on every pass, so the oldest keeps the plain name and the others take their id as a
/// suffix. Every live skill counts, enabled or not: a skill's folder must not depend on which
/// project is asking.
pub fn folder_names(conn: &Connection) -> Result<HashMap<Id, String>> {
    let mut stmt = conn.prepare_cached("SELECT id,name FROM skills WHERE deleted_at IS NULL ORDER BY id")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, Id>(0)?, row.get::<_, String>(1)?)))?;
    let mut used = HashSet::new();
    let mut out = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        let mut dir = folder_name(&name);
        while !used.insert(dir.clone()) {
            dir = format!("{dir}-{id}");
        }
        out.insert(id, dir);
    }
    Ok(out)
}

fn enabled(conn: &Connection, project_id: Id) -> Result<Vec<Row>> {
    let dirs = folder_names(conn)?;
    let mut stmt = conn.prepare_cached(
        "SELECT s.id,s.name,s.body,COALESCE(s.revision,'') FROM skills s
         JOIN skill_projects sp ON sp.skill_id=s.id
         WHERE sp.project_id=?1 AND s.deleted_at IS NULL
         ORDER BY s.name COLLATE NOCASE,s.id",
    )?;
    let rows = stmt.query_map([project_id], |row| row_of(row, &dirs))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(with_plugins(rows, &crate::plugins::enabled_for(conn, project_id)?))
}

/// Add the skills of enabled plugins. An installed skill with the same folder name wins: the
/// user put it there on purpose, and two writers of one folder would rewrite it forever.
fn with_plugins(mut rows: Vec<Row>, plugins: &[&'static crate::plugins::Loaded]) -> Vec<Row> {
    for plugin in plugins {
        for skill in &plugin.skills {
            if rows.iter().any(|row| row.dir == skill.dir) {
                continue;
            }
            rows.push(Row {
                dir: skill.dir.clone(),
                body: skill.body.clone(),
                stamp: format!("plugin {} {} {}\n", plugin.id(), plugin.bundle.digest, skill.dir),
                source: Source::Plugin(skill.files.clone()),
            });
        }
    }
    rows
}

fn row_of(row: &rusqlite::Row, dirs: &HashMap<Id, String>) -> rusqlite::Result<Row> {
    let id: Id = row.get(0)?;
    let name: String = row.get(1)?;
    let body: String = row.get(2)?;
    let revision: String = row.get(3)?;
    // From the content, not `updated_at`: undo, `skill.enable` and every `skill.changed` move
    // that without changing a byte, and each move recopied the folder into every checkout
    // (RA-394). The library folder's version is added when it is written ([`write_skill`]).
    let mut hash = Sha256::new();
    hash.update(name.as_bytes());
    hash.update([0]);
    hash.update(body.as_bytes());
    let content = crate::hex(&hash.finalize());
    Ok(Row {
        dir: dirs.get(&id).cloned().unwrap_or_else(|| folder_name(&name)),
        body,
        stamp: format!("{id} {revision} {content}\n"),
        source: Source::Library(id),
    })
}

/// What a skill's library folder holds, without reading it: [`adopt`] puts a new directory in
/// place every time, by a rename or a fresh copy, so its inode and mtime move exactly when the
/// files do. A plugin skill's files are its bundle's, whose digest is in the stamp already.
fn assets_version(store: &crate::Store, source: &Source) -> String {
    match source {
        Source::Library(id) => match fs::metadata(library_dir(store, *id)) {
            Ok(meta) if meta.is_dir() => format!("{}.{}.{}", meta.ino(), meta.mtime(), meta.mtime_nsec()),
            _ => "none".into(),
        },
        Source::Plugin(_) => "bundled".into(),
    }
}

/// Every skill enabled in at least one project. The user-scope folders are one per machine,
/// so they cannot carry a per-project answer; a project that switched a skill off still keeps
/// it out of its own checkout, which is where a provider looks first.
fn enabled_anywhere(conn: &Connection) -> Result<Vec<Row>> {
    let dirs = folder_names(conn)?;
    let mut stmt = conn.prepare_cached(
        "SELECT s.id,s.name,s.body,COALESCE(s.revision,'') FROM skills s
         WHERE s.deleted_at IS NULL AND EXISTS(SELECT 1 FROM skill_projects sp WHERE sp.skill_id=s.id)
         ORDER BY s.name COLLATE NOCASE,s.id",
    )?;
    let rows = stmt.query_map([], |row| row_of(row, &dirs))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(with_plugins(rows, &crate::plugins::enabled_anywhere(conn)?))
}

/// The machine-wide skill folders. Codex is the reason this exists: it reads skills only from
/// `$CODEX_HOME/skills` and knows nothing about a checkout's `.claude/` or `.agents/`, so a
/// Codex session would otherwise never see an installed skill as a skill. Claude's user scope
/// is included for the same reach outside Relay's own projects.
pub fn user_bases(instance: crate::Instance) -> Vec<PathBuf> {
    // A test engine must never write into the developer's own home; `RELAY_SKILLS_HOME` gives
    // tests and scratch runs a harmless place to prove the same path end to end.
    let (home, codex) = match std::env::var_os("RELAY_SKILLS_HOME") {
        Some(path) => {
            let home = PathBuf::from(path);
            let codex = home.join(".codex");
            (home, codex)
        }
        None if instance == crate::Instance::Test => return Vec::new(),
        None => {
            let Some(dirs) = directories::BaseDirs::new() else { return Vec::new() };
            let home = dirs.home_dir().to_path_buf();
            let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
            (home, codex)
        }
    };
    vec![home.join(".claude").join("skills"), codex.join("skills")]
}

/// What one set of skill folders should hold, read under the store lock so the copying can
/// happen without it: skill folders are megabytes, and the store mutex is not where megabytes
/// belong (D144).
pub struct Plan {
    /// Directories that hold one folder per skill, each with whether plugin skills go there.
    bases: Vec<(PathBuf, bool)>,
    /// The checkout those folders sit in, when they do: its `git status` has to stay clean.
    repo: Option<PathBuf>,
    rows: Vec<Row>,
}

/// Read the skills enabled for `project_id` into a plan for the checkout at `root`.
pub fn plan(conn: &Connection, root: &Path, project_id: Id) -> Result<Plan> {
    Ok(Plan {
        bases: TARGETS.iter().map(|target| (root.join(target), true)).collect(),
        repo: Some(root.to_path_buf()),
        rows: enabled(conn, project_id)?,
    })
}

/// Read every enabled skill into a plan for the machine-wide provider folders.
///
/// A plugin is on for some projects only (D159), so its skills stay out of Claude's user scope:
/// Claude reads each checkout's `.claude/skills`, where the projects that have the plugin on
/// already carry them. Codex reads skills from its home alone, so a plugin on anywhere is
/// registered there for every Codex session on the machine — the one place it reaches further.
pub fn plan_user(conn: &Connection, instance: crate::Instance) -> Result<Plan> {
    let bases: Vec<(PathBuf, bool)> = user_bases(instance).into_iter().zip([false, true]).collect();
    let rows = if bases.is_empty() { Vec::new() } else { enabled_anywhere(conn)? };
    Ok(Plan { bases, repo: None, rows })
}

/// Write a plan out: every enabled skill becomes a provider skill folder, folders of skills
/// that are no longer enabled are removed, and results inside a checkout stay out of
/// `git status`. Best-effort per skill — one unwritable folder never costs the caller its
/// session.
///
/// One apply at a time, process-wide. A launch, a [`refresh_all`] pass and a new project's
/// first fill can run at once, and they rewrite the same machine-wide folders (remove, then
/// copy) and read-modify-write one repository's `info/exclude`; interleaved, one's removal lands
/// in the middle of the other's copy. The lock is a leaf: nothing is taken under it.
pub fn apply(plan: &Plan, store: &crate::Store) -> Result<()> {
    static APPLYING: Mutex<()> = Mutex::new(());
    let _applying = APPLYING.lock().unwrap_or_else(PoisonError::into_inner);
    let owner = owner_line(store);
    let mut written: Vec<String> = Vec::new();
    for (base, plugins) in &plan.bases {
        if let Some(repo) = plan.repo.as_ref().filter(|repo| !inside_checkout(repo, base)) {
            tracing::warn!(repo = %repo.display(), base = %base.display(), "a skills folder in this checkout is a symlink; not writing skills through it");
            continue;
        }
        let rows: Vec<&Row> = plan.rows.iter().filter(|row| *plugins || !matches!(row.source, Source::Plugin(_))).collect();
        prune(base, &rows, &owner);
        for row in rows {
            let dest = base.join(&row.dir);
            match write_skill(store, row, &dest, &owner) {
                Ok(true) => {
                    if let Some(relative) = plan.repo.as_ref().and_then(|repo| dest.strip_prefix(repo).ok()) {
                        written.push(format!("{}/", relative.display()));
                    }
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(skill = %row.dir, error = %error, "materializing skill"),
            }
        }
    }
    if let (Some(repo), false) = (plan.repo.as_ref(), written.is_empty()) {
        let entries: Vec<&str> = written.iter().map(String::as_str).collect();
        crate::worktree::exclude_paths(repo, &entries)?;
    }
    Ok(())
}

/// Serializes [`refresh_all`] for one engine and folds a burst of requests into one pass.
/// Every skill or plugin change asks for a refresh on a thread of its own; unserialized, a pass
/// that planned before the latest change could finish after the pass that saw it, and leave
/// the folders describing the older state.
#[derive(Default)]
pub struct Refresher {
    /// Passes asked for so far.
    requested: AtomicU64,
    /// The request count when the latest pass started planning: every request up to it is
    /// committed state that pass read.
    planned: Mutex<u64>,
}

/// Push every enabled skill into every project root and every session worktree that still
/// exists. This is what makes a skill installed today show up in workspaces created yesterday.
/// Passes run one at a time, and a request a later-starting pass already covers is skipped.
pub fn refresh_all(engine: &crate::engine::Engine) {
    let refresher = &engine.skill_refresh;
    let ticket = refresher.requested.fetch_add(1, Ordering::SeqCst) + 1;
    let mut planned = refresher.planned.lock().unwrap_or_else(PoisonError::into_inner);
    if *planned >= ticket {
        return;
    }
    *planned = refresher.requested.load(Ordering::SeqCst);
    let plans = {
        let conn = engine.store.lock();
        let mut roots: Vec<(Id, PathBuf)> = Vec::new();
        let mut read = |sql: &str| {
            let Ok(mut stmt) = conn.prepare(sql) else { return };
            let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, Id>(0)?, PathBuf::from(row.get::<_, String>(1)?)))
            }) else { return };
            roots.extend(rows.flatten());
        };
        read("SELECT id,path FROM projects");
        read("SELECT project_id,worktree FROM sessions WHERE worktree != '' AND state != 'closed'");
        let mut plans = match plan_user(&conn, engine.instance) {
            Ok(plan) => vec![plan],
            Err(error) => {
                tracing::warn!(error = %error, "planning user-scope skills");
                Vec::new()
            }
        };
        plans.extend(roots
            .into_iter()
            .filter(|(_, root)| root.is_dir())
            .filter_map(|(project_id, root)| match plan(&conn, &root, project_id) {
                Ok(plan) => Some(plan),
                Err(error) => {
                    tracing::warn!(root = %root.display(), error = %error, "planning skills");
                    None
                }
            })
            .collect::<Vec<_>>());
        plans
    };
    for plan in plans {
        if let Err(error) = apply(&plan, &engine.store) {
            tracing::warn!(error = %error, "materializing skills");
        }
    }
}

/// Whether `base` lies under `repo` through real directories only (those not made yet count, and
/// are made as directories). A checkout is the agent's or a cloned repository's: a `.claude` or
/// `skills` committed as a symlink would send Relay's writes and prunes outside it. The same
/// check as `hooks::real_dir`, which replaces the link instead; a skills folder is the
/// repository's to keep, so here nothing is written.
fn inside_checkout(repo: &Path, base: &Path) -> bool {
    let Ok(relative) = base.strip_prefix(repo) else { return false };
    let mut dir = repo.to_path_buf();
    for part in relative.components() {
        let std::path::Component::Normal(part) = part else { return false };
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return false,
            _ => {}
        }
    }
    true
}

/// The first line of every marker this store writes. Two instances (dev and stable) share the
/// machine-wide folders and often the same checkouts; each prunes only what it wrote.
fn owner_line(store: &crate::Store) -> String {
    format!("owner {}\n", store.skills_dir().display())
}

/// Whether a folder's marker says this store wrote it. A marker from before owners were
/// recorded counts as anyone's, which is how every store treated it then. Markers sit in the
/// agent's checkout and are read under [`apply`]'s lock: a FIFO, a link or a huge file there
/// is no marker, and is not waited on.
fn owned(marker: &Path, owner: &str) -> bool {
    crate::private::read_regular(marker).is_ok_and(|text| !text.starts_with("owner ") || text.starts_with(owner))
}

/// Returns whether the folder is Relay's to write (a checked-in skill of the same name wins).
/// A folder another store wrote is rewritten when this one has a skill there too, and from
/// then on it is this store's to prune.
fn write_skill(store: &crate::Store, row: &Row, dest: &Path, owner: &str) -> Result<bool> {
    // A symlink of the skill's name is someone else's, like a checked-in folder; writing would
    // follow it.
    if fs::symlink_metadata(dest).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Ok(false);
    }
    let marker = dest.join(MARKER);
    if dest.exists() && !marker.is_file() {
        return Ok(false);
    }
    let stamp = format!("{owner}{}assets {}\n", row.stamp, assets_version(store, &row.source));
    if crate::private::read_regular(&marker).is_ok_and(|current| current == stamp) {
        return Ok(true);
    }
    // A half-written folder must not survive: without its marker the next pass would read it
    // as a folder the repository owns and never touch it again.
    if let Err(error) = fill(store, row, dest, &stamp) {
        let _ = fs::remove_dir_all(dest);
        return Err(error);
    }
    Ok(true)
}

fn fill(store: &crate::Store, row: &Row, dest: &Path, stamp: &str) -> Result<()> {
    let _ = fs::remove_dir_all(dest);
    match &row.source {
        Source::Library(id) if library_dir(store, *id).is_dir() => copy_bounded(&library_dir(store, *id), dest)?,
        Source::Library(_) => fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?,
        Source::Plugin(files) => {
            for (relative, bytes) in files {
                let path = dest.join(relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
                }
                fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
            }
        }
    }
    // The stored body is the instructions, whatever the library folder holds: it starts as the
    // downloaded SKILL.md, and `skill.update` edits only the body.
    let instructions = dest.join("SKILL.md");
    fs::write(&instructions, &row.body).with_context(|| format!("writing {}", instructions.display()))?;
    let marker = dest.join(MARKER);
    fs::write(&marker, stamp).with_context(|| format!("writing {}", marker.display()))
}

/// Remove skill folders this store wrote that no longer correspond to an enabled skill.
fn prune(base: &Path, rows: &[&Row], owner: &str) {
    let Ok(entries) = fs::read_dir(base) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Only real folders: a link's marker would be read from wherever it points.
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) || !owned(&path.join(MARKER), owner) || rows.iter().any(|row| row.dir == name) {
            continue;
        }
        let _ = fs::remove_dir_all(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_are_provider_safe() {
        assert_eq!(folder_name("  Impeccable  "), "impeccable");
        assert_eq!(folder_name("Review Carefully!"), "review-carefully");
        assert_eq!(folder_name("///"), "skill");
    }

    /// A marker in the checkout is read under `apply`'s lock: a FIFO there is no marker, and a
    /// prune or a rewrite never waits on it.
    #[test]
    fn a_planted_fifo_marker_is_not_waited_on() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(MARKER);
        let name = std::ffi::CString::new(marker.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o644) }, 0);
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || done.send(owned(&marker, "owner x\n")).unwrap());
        let owned = finished.recv_timeout(std::time::Duration::from_secs(10)).expect("a read blocked on a planted FIFO");
        assert!(!owned, "a FIFO counted as a marker");
    }

    #[test]
    fn user_scope_covers_both_providers_and_never_touches_a_test_engine_home() {
        assert!(
            user_bases(crate::Instance::Test).is_empty(),
            "a test engine must not write into the developer's real home",
        );
        let home = tempfile::tempdir().unwrap();
        // SAFETY: the only test that touches this variable, restored before it returns.
        unsafe { std::env::set_var("RELAY_SKILLS_HOME", home.path()) };
        let bases = user_bases(crate::Instance::Test);
        assert_eq!(bases, vec![home.path().join(".claude/skills"), home.path().join(".codex/skills")]);

        // Codex reads skills from its own home and nowhere else, so this is the only place a
        // Codex session can see one.
        let store = crate::Store::open_memory().unwrap();
        store.with_tx(|tx| {
            tx.execute("INSERT INTO workspaces(path,name,ord,created_at,updated_at) VALUES ('/w','w',0,'t','t')", [])?;
            tx.execute("INSERT INTO projects(workspace_id,path,name,base_branch,ord,created_at,updated_at) VALUES (1,'/w/p','p','main',0,'t','t')", [])?;
            tx.execute("INSERT INTO skills(name,body,created_at,updated_at) VALUES ('Impeccable','design well','t','t')", [])?;
            tx.execute("INSERT INTO skill_projects(skill_id,project_id) VALUES (1,1)", [])?;
            Ok(())
        }).unwrap();
        let hand_written = home.path().join(".codex/skills/mine");
        fs::create_dir_all(&hand_written).unwrap();
        fs::write(hand_written.join("SKILL.md"), "mine").unwrap();

        // Planned under the lock, copied after it is released (D147).
        let plan = plan_user(&store.lock(), crate::Instance::Test).unwrap();
        apply(&plan, &store).unwrap();
        for base in &bases {
            assert_eq!(fs::read_to_string(base.join("impeccable/SKILL.md")).unwrap(), "design well");
        }
        // Switching it off in the only project that had it takes it out of the shared homes.
        store.with_tx(|tx| { tx.execute("DELETE FROM skill_projects", [])?; Ok(()) }).unwrap();
        // Planned under the lock, copied after it is released (D147).
        let plan = plan_user(&store.lock(), crate::Instance::Test).unwrap();
        apply(&plan, &store).unwrap();
        assert!(!bases[1].join("impeccable").exists(), "a disabled skill stayed in the codex home");
        assert_eq!(
            fs::read_to_string(hand_written.join("SKILL.md")).unwrap(),
            "mine",
            "pruning reached a skill the user wrote themselves",
        );
        unsafe { std::env::remove_var("RELAY_SKILLS_HOME") };
    }

    /// One project with the given skills enabled, in a store of its own (a memory store's
    /// library is shared by the whole process). The checkout root is `<dir>/checkout`.
    fn store_with(skills: &[&str]) -> (crate::Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::Store::open(&dir.path().join("store/store.db"), false).unwrap();
        store.with_tx(|tx| {
            tx.execute("INSERT INTO workspaces(path,name,ord,created_at,updated_at) VALUES ('/w','w',0,'t','t')", [])?;
            tx.execute("INSERT INTO projects(workspace_id,path,name,base_branch,ord,created_at,updated_at) VALUES (1,'/w/p','p','main',0,'t','t')", [])?;
            for name in skills {
                tx.execute("INSERT INTO skills(name,body,created_at,updated_at) VALUES (?1,?2,'t','t')", rusqlite::params![name, format!("body of {name}")])?;
                tx.execute("INSERT INTO skill_projects(skill_id,project_id) VALUES (last_insert_rowid(),1)", [])?;
            }
            Ok(())
        }).unwrap();
        (store, dir)
    }

    fn apply_project(store: &crate::Store, dir: &Path) {
        let mut plan = plan(&store.lock(), &dir.join("checkout"), 1).unwrap();
        plan.repo = None;
        apply(&plan, store).unwrap();
    }

    #[test]
    fn names_that_reduce_to_one_folder_get_a_folder_each() {
        let (store, root) = store_with(&["Review: carefully", "review carefully"]);
        let dirs = folder_names(&store.lock()).unwrap();
        assert_eq!(dirs[&1], "review-carefully", "the older skill keeps the plain name");
        assert_eq!(dirs[&2], "review-carefully-2");
        apply_project(&store, root.path());
        let base = root.path().join("checkout/.claude/skills");
        assert_eq!(fs::read_to_string(base.join("review-carefully/SKILL.md")).unwrap(), "body of Review: carefully");
        assert_eq!(fs::read_to_string(base.join("review-carefully-2/SKILL.md")).unwrap(), "body of review carefully");
        // A second pass finds both folders current instead of each rewriting the other.
        let stamp = fs::read_to_string(base.join("review-carefully").join(MARKER)).unwrap();
        fs::write(base.join("review-carefully/extra.md"), "kept").unwrap();
        apply_project(&store, root.path());
        assert!(base.join("review-carefully/extra.md").is_file(), "an unchanged skill was rewritten");
        assert_eq!(fs::read_to_string(base.join("review-carefully").join(MARKER)).unwrap(), stamp);
    }

    #[test]
    fn the_stored_body_wins_over_the_library_skill_md() {
        let (store, root) = store_with(&["Polish"]);
        let library = library_dir(&store, 1);
        fs::create_dir_all(library.join("reference")).unwrap();
        fs::write(library.join("SKILL.md"), "as downloaded").unwrap();
        fs::write(library.join("reference/notes.md"), "notes").unwrap();
        // `skill.update` edits only the row.
        store.with_tx(|tx| { tx.execute("UPDATE skills SET body='as edited',updated_at='t2'", [])?; Ok(()) }).unwrap();
        apply_project(&store, root.path());
        let folder = root.path().join("checkout/.agents/skills/polish");
        assert_eq!(fs::read_to_string(folder.join("SKILL.md")).unwrap(), "as edited");
        assert_eq!(fs::read_to_string(folder.join("reference/notes.md")).unwrap(), "notes");
    }

    /// RA-394: `updated_at` moving (undo, `skill.enable`, `skill.changed`) does not recopy a
    /// folder; a new body or a new library folder does.
    #[test]
    fn a_folder_is_recopied_only_when_its_content_changes() {
        let (store, root) = store_with(&["Polish"]);
        let folder = root.path().join("checkout/.claude/skills/polish");
        apply_project(&store, root.path());
        // Left by the last copy; a recopy removes the folder first.
        fs::write(folder.join("left.md"), "x").unwrap();
        store.with_tx(|tx| { tx.execute("UPDATE skills SET updated_at='t2'", [])?; Ok(()) }).unwrap();
        apply_project(&store, root.path());
        assert!(folder.join("left.md").is_file(), "an unchanged skill was recopied");

        store.with_tx(|tx| { tx.execute("UPDATE skills SET body='new body',updated_at='t3'", [])?; Ok(()) }).unwrap();
        apply_project(&store, root.path());
        assert!(!folder.join("left.md").exists(), "a changed body was not recopied");
        assert_eq!(fs::read_to_string(folder.join("SKILL.md")).unwrap(), "new body");

        fs::write(folder.join("left.md"), "x").unwrap();
        let staged = root.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("reference.md"), "ref").unwrap();
        adopt(&staged, &library_dir(&store, 1)).unwrap();
        apply_project(&store, root.path());
        assert!(!folder.join("left.md").exists(), "a new library folder was not recopied");
        assert_eq!(fs::read_to_string(folder.join("reference.md")).unwrap(), "ref");
    }

    #[test]
    fn a_store_prunes_only_the_folders_it_wrote() {
        let (store, root) = store_with(&[]);
        let base = root.path().join("checkout/.claude/skills");
        for (name, marker) in [
            ("other-instance", "owner /somewhere/else/skills\n1 t \n".to_string()),
            ("before-owners", "1 t \n".to_string()),
            ("mine", format!("{}1 t \n", owner_line(&store))),
        ] {
            fs::create_dir_all(base.join(name)).unwrap();
            fs::write(base.join(name).join(MARKER), marker).unwrap();
        }
        apply_project(&store, root.path());
        assert!(base.join("other-instance").is_dir(), "pruned a folder another Relay store wrote");
        assert!(!base.join("before-owners").exists());
        assert!(!base.join("mine").exists());
    }

    /// A crafted repository's `.claude` (or `.agents/skills`) symlinked elsewhere does not carry
    /// Relay's writes and prunes there, and neither does a skill folder that is a link.
    #[test]
    fn symlinks_in_a_checkout_do_not_redirect_skill_folders() {
        let (store, root) = store_with(&["Polish"]);
        let checkout = root.path().join("checkout");
        let outside = root.path().join("outside");
        fs::create_dir_all(outside.join("skills/victim")).unwrap();
        fs::write(outside.join("skills/victim").join(MARKER), format!("{}1 t \n", owner_line(&store))).unwrap();
        fs::create_dir_all(checkout.join(".agents")).unwrap();
        std::os::unix::fs::symlink(&outside, checkout.join(".claude")).unwrap();
        std::os::unix::fs::symlink(outside.join("skills"), checkout.join(".agents/skills")).unwrap();
        let plan = plan(&store.lock(), &checkout, 1).unwrap();
        apply(&plan, &store).unwrap();
        assert!(outside.join("skills/victim").is_dir(), "a prune followed a symlink out of the checkout");
        assert!(!outside.join("skills/polish").exists(), "a skill was written through a symlink");

        // Real folders, but the skill's own folder and a stale one are links.
        let (store, root) = store_with(&["Polish"]);
        let base = root.path().join("checkout/.claude/skills");
        let outside = root.path().join("outside");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(outside.join("stale")).unwrap();
        fs::write(outside.join("stale").join(MARKER), format!("{}1 t \n", owner_line(&store))).unwrap();
        fs::write(outside.join("stale/work.md"), "keep").unwrap();
        std::os::unix::fs::symlink(outside.join("stale"), base.join("stale")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("polish")).unwrap();
        apply_project(&store, root.path());
        assert!(outside.join("stale/work.md").is_file());
        assert!(!outside.join("SKILL.md").exists(), "a skill was written through its folder's link");
    }

    #[test]
    fn plugin_skills_stay_out_of_claude_user_scope() {
        let store = crate::Store::open_memory().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (claude, codex) = (home.path().join("claude"), home.path().join("codex"));
        let row = |dir: &str, source| Row { dir: dir.into(), body: dir.into(), stamp: format!("{dir}\n"), source };
        let plan = Plan {
            bases: vec![(claude.clone(), false), (codex.clone(), true)],
            repo: None,
            rows: vec![row("installed", Source::Library(999_999)), row("from-plugin", Source::Plugin(vec![("SKILL.md", b"from-plugin".as_slice())]))],
        };
        apply(&plan, &store).unwrap();
        assert!(claude.join("installed/SKILL.md").is_file());
        assert!(!claude.join("from-plugin").exists(), "a per-project plugin reached every Claude session");
        assert!(codex.join("from-plugin/SKILL.md").is_file());
        // One written by an earlier engine, before plugins were kept out, is taken back out.
        fs::create_dir_all(claude.join("from-plugin")).unwrap();
        fs::write(claude.join("from-plugin").join(MARKER), format!("{}x\n", owner_line(&store))).unwrap();
        apply(&plan, &store).unwrap();
        assert!(!claude.join("from-plugin").exists());
    }

    #[test]
    fn bounded_copy_skips_git_and_symlinks() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join(".git")).unwrap();
        fs::write(src.path().join(".git/config"), "x").unwrap();
        fs::create_dir_all(src.path().join("reference")).unwrap();
        fs::write(src.path().join("reference/polish.md"), "polish").unwrap();
        fs::write(src.path().join("SKILL.md"), "body").unwrap();
        std::os::unix::fs::symlink(src.path().join("SKILL.md"), src.path().join("link.md")).unwrap();
        let into = dest.path().join("out");
        copy_bounded(src.path(), &into).unwrap();
        assert_eq!(fs::read_to_string(into.join("reference/polish.md")).unwrap(), "polish");
        assert!(into.join("SKILL.md").is_file());
        assert!(!into.join(".git").exists());
        assert!(!into.join("link.md").exists());
    }
}
