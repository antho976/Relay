//! `file.*` (phase 8): worktree-relative IO, recoverable trash, search and git-badged trees.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::handlers::guardrail;
use crate::handlers::workspace::get_project;
use base64::Engine as _;
use relay_bus::error::BusError;
use relay_bus::ops::file::*;
use relay_bus::types::{Entry, EntryKind, GateKind, Id};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn register(e: &mut Engine) {
    e.register_unlocked::<Tree>(|ctx, p| {
        let (project, root) = root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        crate::watch::ensure_after_commit(ctx, root.clone(), project.id);
        let rel = rel(p.path.as_deref().unwrap_or(""), true)?;
        let dir = safe_join(&root, &rel, true)?;
        if !dir.is_dir() {
            return Err(BusError::not_found(
                "file.not_found",
                format!("{} is not a directory", rel.display()),
            ));
        }
        let badges = if p.git_badges.unwrap_or(true) {
            super::git::status_badges(&root).unwrap_or_default()
        } else {
            HashMap::new()
        };
        let limit = p.limit.unwrap_or(TREE_LIMIT).clamp(1, TREE_LIMIT_MAX) as usize;
        let mut truncated = BTreeMap::new();
        let shown = Shown::new(&root);
        let entries = list_dir(&root, &dir, p.depth.unwrap_or(1).min(20), &badges, &shown, limit, &mut truncated)?;
        Ok(TreeOut { entries, truncated })
    });
    e.register_unlocked::<Read>(|ctx, p| {
        let (project, root) = root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        crate::watch::ensure_after_commit(ctx, root.clone(), project.id);
        let rel = rel(&p.path, false)?;
        let path = safe_join(&root, &rel, false)?;
        let md = fs::metadata(&path).map_err(|e| io_err("file.read_failed", &rel, e))?;
        if !md.is_file() {
            return Err(BusError::invalid("file.type", "path is not a file"));
        }
        let limit = p.max_bytes.unwrap_or(4 * 1024 * 1024).min(32 * 1024 * 1024);
        use std::io::Read;
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
            .map_err(|e| io_err("file.read_failed", &rel, e))?;
        let truncated = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        let mime = mime(&rel);
        let (text, bytes_b64) = decode(bytes, truncated);
        Ok(ReadOut {
            text,
            bytes_b64,
            mime,
            size: md.len(),
            truncated,
        })
    });
    // Staged (D149): the file being replaced is read, hashed and diffed, and the guardrail's
    // probes of it — the old file again, and `git status` when the rewrite is large — are taken
    // before the transaction opens. The transaction checks the file is still the one that was
    // read; if it is not, it is read and judged again there, as before staging, so a write that
    // raced another cannot pass on what the other replaced.
    e.register_staged::<Write, PreparedWrite>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let rel = rel(&p.path, false)?;
        let (path, target_rel) = landing(&root, &rel)?;
        // Taken before anything is read, so a change made while it is read shows too.
        let stamp = stamp(&path);
        let old = read_current(&path).ok();
        let old_sha = p.expected_sha256.as_ref().and(old.as_deref()).map(sha256_hex);
        expect_sha(p.expected_sha256.as_deref(), old_sha.as_deref())?;
        let counts = line_counts(old.as_deref().unwrap_or(""), &p.text);
        drop(old);
        let gated = target_rel.as_ref().map(|t| t.to_string_lossy().to_string());
        let probes = guardrail::warm_enforce(
            ctx,
            project.id,
            &root,
            GateKind::Write,
            Some(gated.as_deref().unwrap_or(&p.path)),
            Some(&p.text),
            None,
            None,
        );
        Ok(PreparedWrite { project_id: project.id, root, rel, path, stamp, old_sha, counts, probes })
    }, |ctx: &mut Ctx, p, prepared| {
        let PreparedWrite { project_id, root, rel, path: read_at, stamp: read_as, old_sha, counts, probes } = prepared;
        let (path, target_rel) = landing(&root, &rel)?;
        // Another write, an editor or a tool changed the file since the read phase, or the path
        // now leads elsewhere: what was hashed, counted and probed is not what would be replaced.
        let fresh = path == read_at && stamp(&path) == read_as;
        let (old_sha, old) = if fresh {
            (old_sha, None)
        } else {
            let old = read_current(&path).ok();
            (p.expected_sha256.as_ref().and(old.as_deref()).map(sha256_hex), old)
        };
        expect_sha(p.expected_sha256.as_deref(), old_sha.as_deref())?;
        let probes = if fresh { probes } else { crate::guardrail::Probes::default() };
        let gated = target_rel.as_ref().map(|t| t.to_string_lossy().to_string());
        guardrail::enforce_with(
            ctx,
            project_id,
            &root,
            GateKind::Write,
            Some(gated.as_deref().unwrap_or(&p.path)),
            Some(&p.text),
            None,
            None,
            &probes,
        )?;
        let (removed, added) = if fresh { counts } else { line_counts(old.as_deref().unwrap_or(""), &p.text) };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err("file.write_failed", &rel, e))?;
        }
        let temp_dir = root.join(".relay").join("tmp");
        fs::create_dir_all(&temp_dir).map_err(|e| io_err("file.write_failed", &rel, e))?;
        let temp = temp_dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let replaced = fs::write(&temp, p.text.as_bytes())
            .and_then(|_| match fs::metadata(&path) {
                Ok(metadata) => fs::set_permissions(&temp, metadata.permissions()),
                Err(_) => Ok(()),
            })
            .and_then(|_| fs::rename(&temp, &path));
        if let Err(e) = replaced {
            // A failed save leaves no stray temp file behind (RA-360).
            let _ = fs::remove_file(&temp);
            return Err(io_err("file.write_failed", &rel, e));
        }
        ctx.set_project(project_id);
        ctx.emit(
            "file.changed",
            json!({"project_id": project_id, "worktree": root, "path": p.path}),
        );
        Ok(WriteOut {
            bytes: p.text.len() as u64,
            removed_lines: removed,
            added_lines: added,
        })
    });
    // Staged like write. Nothing is there to read, so the probes are cheap; the transaction
    // checks again that nothing is there before it gates and creates.
    e.register_staged::<Create, (Id, PathBuf, crate::guardrail::Probes)>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let rel = rel(&p.path, false)?;
        let path = safe_join(&root, &rel, true)?;
        if occupied(&path) {
            return Err(BusError::conflict(
                "file.exists",
                format!("{} already exists", p.path),
            ));
        }
        let text = p.text.as_deref().unwrap_or_default();
        let probes = guardrail::warm_enforce(ctx, project.id, &root, GateKind::Write, Some(&p.path), Some(text), None, None);
        Ok((project.id, root, probes))
    }, |ctx: &mut Ctx, p, (project_id, root, probes)| {
        let rel = rel(&p.path, false)?;
        let path = safe_join(&root, &rel, true)?;
        if occupied(&path) {
            return Err(BusError::conflict(
                "file.exists",
                format!("{} already exists", p.path),
            ));
        }
        let text = p.text.clone().unwrap_or_default();
        guardrail::enforce_with(
            ctx,
            project_id,
            &root,
            GateKind::Write,
            Some(&p.path),
            Some(&text),
            None,
            None,
            &probes,
        )?;
        match p.kind.as_str() {
            "file" => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| io_err("file.create_failed", &rel, e))?;
                }
                fs::write(&path, text).map_err(|e| io_err("file.create_failed", &rel, e))?;
            }
            "dir" => {
                fs::create_dir_all(&path).map_err(|e| io_err("file.create_failed", &rel, e))?
            }
            _ => return Err(BusError::invalid("file.kind", "kind must be file or dir")),
        }
        changed(ctx, project_id, &root, &p.path);
        entry(&root, &path, &HashMap::new())
    });
    // Rename, move and delete are staged (D149): a directory source is walked for the gated
    // paths inside it (RA-147) before the transaction opens, and only the gates and the rename
    // itself hold the store.
    e.register_staged::<Rename, PreparedPath>(|ctx, p| {
        if p.new_name.is_empty()
            || Path::new(&p.new_name).components().count() != 1
            || p.new_name == "."
            || p.new_name == ".."
        {
            return Err(BusError::invalid(
                "file.path",
                "new_name must be one path component",
            ));
        }
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let from_rel = rel(&p.path, false)?;
        let from = safe_join(&root, &from_rel, false)?;
        let into_rel = from_rel.parent().unwrap_or(Path::new("")).join(&p.new_name);
        safe_join(&root, &into_rel, true)?;
        let inner = covered_inside(ctx, project.id, &from, &[&from_rel, &into_rel])?;
        Ok(PreparedPath { project_id: project.id, root, from_rel, into_rel: Some(into_rel), inner })
    }, |ctx: &mut Ctx, _p, prepared| relocate(ctx, prepared, "file.rename_failed"));
    e.register_staged::<Move, PreparedPath>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let from_rel = rel(&p.path, false)?;
        let from = safe_join(&root, &from_rel, false)?;
        let dir_rel = rel(&p.into, true)?;
        let dir = safe_join(&root, &dir_rel, false)?;
        if !dir.is_dir() {
            return Err(BusError::invalid(
                "file.into",
                "destination is not a directory",
            ));
        }
        let name = from
            .file_name()
            .ok_or_else(|| BusError::invalid("file.path", "path has no name"))?;
        let into_rel = dir_rel.join(name);
        safe_join(&root, &into_rel, true)?;
        let inner = covered_inside(ctx, project.id, &from, &[&from_rel, &into_rel])?;
        Ok(PreparedPath { project_id: project.id, root, from_rel, into_rel: Some(into_rel), inner })
    }, |ctx: &mut Ctx, _p, prepared| relocate(ctx, prepared, "file.move_failed"));
    e.register_staged::<Delete, PreparedPath>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let rel = rel(&p.path, false)?;
        let path = safe_join(&root, &rel, false)?;
        let inner = covered_inside(ctx, project.id, &path, &[&rel])?;
        Ok(PreparedPath { project_id: project.id, root, from_rel: rel, into_rel: None, inner })
    }, |ctx: &mut Ctx, p, prepared| {
        let PreparedPath { project_id, root, from_rel: rel, inner, .. } = prepared;
        let path = root.join(&rel);
        guard_path_mutation(ctx, project_id, &root, &rel, None)?;
        for inside in &inner {
            guard_path_mutation(ctx, project_id, &root, &rel.join(inside), None)?;
        }
        // The primary checkout keeps the trash: a session's worktree is removed when the
        // session ends, and its `.relay/trash` went with it while the row still offered a
        // restore (RA-214). A worktree a rename cannot leave (another filesystem) keeps its own.
        let primary = fs::canonicalize(get_project(ctx.tx(), project_id)?.path).ok();
        let mut bases: Vec<&Path> = primary.iter().map(PathBuf::as_path).collect();
        if !bases.contains(&root.as_path()) {
            bases.push(&root);
        }
        let worktree = root.display().to_string();
        let mut slot: Option<Id> = None;
        let mut attempts = 0;
        let (id, trash) = loop {
            ctx.tx().execute(
                "INSERT INTO file_trash(id, project_id, worktree, original_path, trash_path, created_at) VALUES (?1,?2,?3,?4,'',?5)",
                params![slot, project_id, worktree, p.path, ctx.now],
            ).bus()?;
            let id = ctx.tx().last_insert_rowid();
            match trash_into(&bases, id, &path) {
                Ok(trash) => break (id, trash),
                // `.relay/trash/<id>` is already there: the store's sequence is behind the
                // directories on disk (a fresh store, a restored backup, a commit that rolled
                // back after the move). Never move onto it (RA-361); take an id past them all.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempts < 3 => {
                    attempts += 1;
                    ctx.tx().execute("DELETE FROM file_trash WHERE id=?1", [id]).bus()?;
                    slot = Some(highest_trash_slot(&bases).max(id) + 1);
                }
                Err(e) => return Err(io_err("file.delete_failed", &rel, e)),
            }
        };
        // The row that lists the payload is not durable until the commit: should anything from
        // here to the commit fail, the payload goes back where it was rather than sitting in a
        // trash nothing lists or restores.
        let untrash = Untrash { payload: trash.clone(), path, kept: false };
        ctx.tx().execute("UPDATE file_trash SET trash_path=?1 WHERE id=?2", params![trash.display().to_string(), id]).bus()?;
        ctx.set_undo("file.restore", json!({"project_id": project_id, "trash_id": id}), None);
        changed(ctx, project_id, &root, &p.path);
        ctx.after_commit(move |_| untrash.keep());
        Ok(DeleteOut { trash_id: id })
    });
    // Staged like delete: the checkout is resolved and a directory payload walked for gated paths
    // before the transaction, which then gates the restored path the way every other file
    // mutation is gated (RA-362) and moves the payload back.
    e.register_staged::<Restore, PreparedRestore>(|ctx, p| {
        let session_id = ctx.actor_session_id();
        let (project, own, row) = ctx.read(|conn| {
            let project = get_project(conn, p.project_id)?;
            let own = own_checkout(conn, &ctx.actor, session_id, project.id)?;
            Ok((project, own, open_trash(conn, p.trash_id, p.project_id)?))
        })?;
        let (root_s, rel_s, trash_s) = row;
        // A named checkout is addressed and confined like any other mutation's (RA-148). Unnamed,
        // it goes back where it was deleted from while that is still a worktree of this project;
        // a session's worktree is removed with the session, while the bytes outlive it in the
        // primary checkout's trash, so then it goes to the primary checkout (RA-214).
        let (root, fallback) = if p.worktree.is_some() {
            (root_mut_unlocked(ctx, project.id, p.worktree.as_deref())?.1, false)
        } else {
            let original = match fs::canonicalize(&root_s) {
                Ok(original) => crate::worktree::contains(Path::new(&project.path), &original)
                    .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?
                    .then_some(original),
                Err(_) => None,
            };
            let (root, fallback) = match original {
                Some(original) => (original, false),
                None => (root_verify(&project, PathBuf::from(&project.path), "file.worktree")?, true),
            };
            confine(own, &root)?;
            (root, fallback)
        };
        if trash_s.is_empty() || !occupied(Path::new(&trash_s)) {
            return Err(BusError::not_found("file.trash_unavailable", format!("trash {} is no longer on disk", p.trash_id)));
        }
        let rel = rel(&rel_s, false)?;
        safe_join(&root, &rel, true)?;
        let inner = covered_inside(ctx, project.id, Path::new(&trash_s), &[&rel])?;
        Ok(PreparedRestore { project_id: project.id, root, fallback, rel, trash: PathBuf::from(trash_s), inner })
    }, |ctx: &mut Ctx, p, prepared| {
        let PreparedRestore { project_id, root, fallback, rel, trash, inner } = prepared;
        // Restored by someone else since the read phase, or not the same payload any more.
        let (_, _, trash_s) = open_trash(ctx.tx(), p.trash_id, project_id)?;
        if Path::new(&trash_s) != trash {
            return Err(BusError::conflict("file.restore_conflict", format!("trash {} changed", p.trash_id)));
        }
        let path = safe_join(&root, &rel, true)?;
        guard_path_mutation(ctx, project_id, &root, &rel, Some(&trash))?;
        for inside in &inner {
            guard_path_mutation(ctx, project_id, &root, &rel.join(inside), Some(&trash.join(inside)))?;
        }
        if occupied(&path) { return Err(BusError::conflict("file.restore_conflict", format!("{} already exists", rel.display()))); }
        if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| io_err("file.restore_failed", &rel, e))?; }
        fs::rename(&trash, &path).map_err(|e| io_err("file.restore_failed", &rel, e))?;
        ctx.tx().execute("UPDATE file_trash SET restored_at=?1 WHERE id=?2", params![ctx.now, p.trash_id]).bus()?;
        changed(ctx, project_id, &root, &rel.to_string_lossy());
        Ok(RestoreOut { entry: entry(&root, &path, &HashMap::new())?, worktree: root.display().to_string(), fallback })
    });
    e.register_unlocked::<TrashList>(|ctx, p| {
        let limit = p.limit.unwrap_or(200).clamp(1, 1000);
        let rows = ctx.read(|conn| {
            get_project(conn, p.project_id)?;
            conn.prepare_cached(
                "SELECT id, original_path, worktree, trash_path, created_at FROM file_trash
                 WHERE project_id = ?1 AND restored_at IS NULL ORDER BY id DESC LIMIT ?2+0",
            ).bus()?
            .query_map(params![p.project_id, limit], |r| {
                Ok((r.get::<_, Id>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
            }).bus()?
            .collect::<rusqlite::Result<Vec<_>>>().bus()
        })?;
        // Whether the bytes are still there is a stat per row, so it is asked with nothing held.
        let entries = rows
            .into_iter()
            .map(|(id, original_path, worktree, trash_path, created_at)| TrashEntry {
                id, original_path, worktree, created_at,
                available: !trash_path.is_empty() && occupied(Path::new(&trash_path)),
            })
            .collect();
        Ok(TrashListOut { entries })
    });
    // Staged (D149): the checkout is a subprocess, so it runs before the transaction opens;
    // the transaction only announces the change. `--literal-pathspecs`: a path is a name, never
    // a glob, so restoring `[id].tsx` cannot also discard edits to `i.tsx` and `d.tsx` (RA-153).
    // `HEAD` is the source, so a staged change is put back too rather than surviving in the
    // index; the path is validated before git runs, and `.` (the whole tree) is refused (RA-364).
    e.register_staged::<RestoreHead, (Id, PathBuf, Entry)>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let rel = rel(&p.path, false)?;
        let rel: PathBuf = rel.components().filter(|c| !matches!(c, Component::CurDir)).collect();
        if rel.as_os_str().is_empty() {
            return Err(BusError::invalid("file.path", "restore_head needs a path inside the worktree, not the worktree itself"));
        }
        let path = safe_join(&root, &rel, true)?;
        let spec = rel.to_string_lossy();
        crate::worktree::git_mutate(&root, &["--literal-pathspecs", "checkout", "HEAD", "--", &spec])
            .map_err(|e| BusError::conflict("file.restore_head_failed", e.to_string()))?;
        Ok((project.id, root.clone(), entry(&root, &path, &HashMap::new())?))
    }, |ctx: &mut Ctx, p, (project_id, root, restored)| {
        changed(ctx, project_id, &root, &p.path);
        Ok(restored)
    });
    // Staged (D149, RA-190): the sources are copied into the worktree's `.relay/tmp` with
    // nothing locked; the transaction gates each destination and renames the copies into place.
    e.register_staged::<Import, PreparedImport>(|ctx, p| {
        let (project, root) = root_mut_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let into_rel = rel(&p.into, true)?;
        let into = safe_join(&root, &into_rel, false)?;
        if !into.is_dir() {
            return Err(BusError::invalid(
                "file.into",
                "destination is not a directory",
            ));
        }
        let staging = Staging::new(&root)?;
        let mut items = Vec::new();
        for (index, source) in p.sources.iter().enumerate() {
            let source = Path::new(source);
            if !source.is_absolute() || !source.exists() {
                return Err(BusError::invalid(
                    "file.source",
                    format!("{} is not an existing absolute path", source.display()),
                ));
            }
            let name = source
                .file_name()
                .ok_or_else(|| BusError::invalid("file.source", "source has no name"))?;
            // The copy would walk into its own staging directory and never finish.
            if fs::symlink_metadata(source).is_ok_and(|md| md.is_dir())
                && fs::canonicalize(source).is_ok_and(|source| root.starts_with(source))
            {
                return Err(BusError::invalid(
                    "file.source",
                    format!("{} contains the worktree it would be imported into", source.display()),
                ));
            }
            let dest_rel = into_rel.join(name);
            // Checked again under the lock; here it saves copying what cannot land.
            if occupied(&into.join(name)) {
                return Err(BusError::conflict(
                    "file.exists",
                    format!("{} already exists", dest_rel.display()),
                ));
            }
            let inner = covered_inside(ctx, project.id, source, &[&dest_rel])?;
            let staged = staging.0.join(index.to_string());
            copy(source, &staged).map_err(|e| io_err("file.import_failed", &dest_rel, e))?;
            items.push(StagedImport { staged, dest_rel, inner });
        }
        Ok(PreparedImport { project_id: project.id, root, items, _staging: staging })
    }, |ctx: &mut Ctx, p, prepared| {
        let PreparedImport { project_id, root, items, _staging } = prepared;
        // Every gate and conflict first, so a refusal part-way leaves nothing half-imported.
        for item in &items {
            guard_path_mutation(ctx, project_id, &root, &item.dest_rel, Some(&item.staged))?;
            for inside in &item.inner {
                guard_path_mutation(ctx, project_id, &root, &item.dest_rel.join(inside), Some(&item.staged.join(inside)))?;
            }
            if occupied(&root.join(&item.dest_rel)) {
                return Err(BusError::conflict(
                    "file.exists",
                    format!("{} already exists", item.dest_rel.display()),
                ));
            }
        }
        let mut entries = Vec::new();
        for item in &items {
            let dest = root.join(&item.dest_rel);
            fs::rename(&item.staged, &dest).map_err(|e| io_err("file.import_failed", &item.dest_rel, e))?;
            entries.push(entry(&root, &dest, &HashMap::new())?);
        }
        changed(ctx, project_id, &root, &p.into);
        Ok(ImportOut { entries })
    });
    e.register_unlocked::<Search>(|ctx, p| {
        let (project, root) = root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        crate::watch::ensure_after_commit(ctx, root.clone(), project.id);
        let regex = if p.regex.unwrap_or(false) {
            Some(
                regex::Regex::new(&p.query)
                    .map_err(|e| BusError::invalid("file.regex", e.to_string()))?,
            )
        } else {
            None
        };
        let limit = p.limit.unwrap_or(200).min(2000) as usize;
        let mut hits = Vec::new();
        let mut stack = vec![root.clone()];
        let shown = Shown::new(&root);
        // One buffer for the whole walk: `read_to_string` allocated, grew and freed a fresh
        // `String` for every file in the tree, most of which contribute no hits at all.
        let mut buffer: Vec<u8> = Vec::new();
        while let Some(dir) = stack.pop() {
            // One unreadable subdirectory costs its own hits, not the whole search.
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(e) if dir == root => return Err(io_err("file.search_failed", &dir, e)),
                Err(_) => continue,
            };
            for e in entries.flatten() {
                let path = e.path();
                let relp = path.strip_prefix(&root).unwrap_or(&path);
                // `DirEntry::file_type` does not follow symlinks, so a linked directory is
                // never walked into. Generated trees are skipped by the same rule file.tree uses.
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if !shown.hidden(&path) {
                        stack.push(path);
                    }
                    continue;
                }
                if let Some(glob) = &p.glob {
                    if !search_glob_matches(glob, relp) {
                        continue;
                    }
                }
                if !read_searchable(&root, &path, &mut buffer) {
                    continue;
                }
                // Same rule as `read_to_string`: what is not text is not searched.
                let Ok(text) = std::str::from_utf8(&buffer) else {
                    continue;
                };
                // The whole file is searched in one pass and hits are mapped back to lines,
                // one per line as before. Searching line by line built a fresh substring
                // searcher for every line of every file, a fifth of the op (PERF §1.7).
                let mut from = 0usize;
                let mut line = 1u32;
                let mut counted_to = 0usize;
                while from < text.len() {
                    let found = match &regex {
                        Some(r) => r.find_at(text, from).map(|m| m.start()),
                        None => text[from..].find(&p.query).map(|i| from + i),
                    };
                    let Some(at) = found else { break };
                    line += text[counted_to..at].bytes().filter(|b| *b == b'\n').count() as u32;
                    counted_to = at;
                    let start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let end = text[at..].find('\n').map(|i| at + i).unwrap_or(text.len());
                    let (shown, text_offset) = hit_window(&text[start..end], at - start);
                    hits.push(Hit {
                        path: relp.to_string_lossy().to_string(),
                        line,
                        col: (at - start) as u32 + 1,
                        text: shown.trim_end_matches('\r').to_string(),
                        text_offset: (text_offset > 0).then_some(text_offset as u32),
                    });
                    if hits.len() >= limit {
                        return Ok(SearchOut { hits });
                    }
                    from = end + 1;
                }
            }
        }
        Ok(SearchOut { hits })
    });
}

/// Which tree an omitted `worktree` means. For an agent it is that session's own worktree,
/// never the project root: defaulting to the root answered a confident, well-formed, wrong
/// question — "your" diff, read from a tree the agent has never touched — and returned no
/// error either way (D111). `@project` asks for the root explicitly and loudly.
///
/// The store half of worktree resolution, split out so the unlocked query context can do it in
/// one short read and check the answer against the repository with nothing held (D144).
pub(crate) fn default_worktree_in(
    conn: &Connection,
    session_id: Option<Id>,
    project: &relay_bus::types::Project,
    requested: Option<&str>,
) -> Result<PathBuf, BusError> {
    let repo = PathBuf::from(&project.path);
    match requested {
        Some(PROJECT_ROOT) => Ok(repo),
        Some(path) => Ok(PathBuf::from(path)),
        None => match session_id {
            Some(id) => {
                let row = crate::sessions::by_id(conn, id)?
                    .ok_or_else(|| BusError::actor("bound session vanished"))?;
                Ok(PathBuf::from(row.session.worktree))
            }
            None => Ok(repo),
        },
    }
}

/// The store half of [`root_unlocked`] and [`root_mut_unlocked`]: project row plus the worktree
/// the caller means, before any of it is checked against the repository. Cheap, and the only
/// part that needs the connection.
pub(crate) fn root_choice(
    conn: &Connection,
    session_id: Option<Id>,
    project_id: Id,
    requested: Option<&str>,
) -> Result<(relay_bus::types::Project, PathBuf), BusError> {
    let project = get_project(conn, project_id)?;
    let chosen = default_worktree_in(conn, session_id, &project, requested)?;
    Ok((project, chosen))
}

/// The external half: canonicalize and confirm the path really is a worktree of this repository.
/// Filesystem reads only — [`crate::worktree::contains`] reads the checkout's `.git` pointer and
/// its slot, no subprocess — so even a nested replay that holds the store may call it.
pub(crate) fn root_verify(
    project: &relay_bus::types::Project,
    chosen: PathBuf,
    code: &'static str,
) -> Result<PathBuf, BusError> {
    let chosen = fs::canonicalize(&chosen).map_err(|e| BusError::invalid(code, e.to_string()))?;
    let valid = crate::worktree::contains(Path::new(&project.path), &chosen)
        .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?;
    if !valid {
        return Err(BusError::invalid(
            code,
            format!("{} is not a worktree of this project", chosen.display()),
        ));
    }
    Ok(chosen)
}

/// The one spelling that means "the project root, deliberately".
pub(crate) const PROJECT_ROOT: &str = "@project";

/// The tree a query reads: one short read, then the check against the repository with nothing
/// held. Mutations resolve theirs through [`root_mut_unlocked`], which also confines an agent to
/// its own checkout.
pub(crate) fn root_unlocked(
    ctx: &Unlocked,
    project_id: Id,
    requested: Option<&str>,
) -> Result<(relay_bus::types::Project, PathBuf), BusError> {
    let session_id = ctx.actor_session_id();
    let (project, chosen) =
        ctx.read(|conn| root_choice(conn, session_id, project_id, requested))?;
    let chosen = root_verify(&project, chosen, "file.worktree")?;
    Ok((project, chosen))
}

fn rel(value: &str, allow_empty: bool) -> Result<PathBuf, BusError> {
    let path = Path::new(value);
    if (!allow_empty && value.is_empty())
        || path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(BusError::invalid(
            "file.path",
            format!("{value:?} must be worktree-relative and contain no .."),
        ));
    }
    Ok(path.to_path_buf())
}

/// The tree a mutation changes: an agent may change files only in its own project and its own
/// checkout (RA-148). Naming another project, or another worktree of its own, used to be
/// enough to write there. Resolved in the read phase: every file mutation is staged (D149).
fn root_mut_unlocked(
    ctx: &Unlocked,
    project_id: Id,
    requested: Option<&str>,
) -> Result<(relay_bus::types::Project, PathBuf), BusError> {
    let session_id = ctx.actor_session_id();
    let (project, chosen, own) = ctx.read(|conn| {
        let (project, chosen) = root_choice(conn, session_id, project_id, requested)?;
        let own = own_checkout(conn, &ctx.actor, session_id, project.id)?;
        Ok((project, chosen, own))
    })?;
    let chosen = root_verify(&project, chosen, "file.worktree")?;
    confine(own, &chosen)?;
    Ok((project, chosen))
}

/// The checkout an agent is confined to, or `None` for the user. Refuses another project
/// outright, the way every other project-scoped agent op does.
fn own_checkout(
    conn: &Connection,
    actor: &relay_bus::Actor,
    session_id: Option<Id>,
    project_id: Id,
) -> Result<Option<PathBuf>, BusError> {
    let Some(id) = session_id else {
        if actor.is_agent() {
            return Err(BusError::actor("agent actor is not bound to a live session"));
        }
        return Ok(None);
    };
    let row = crate::sessions::by_id(conn, id)?
        .ok_or_else(|| BusError::actor("bound session vanished"))?;
    if row.session.project_id != project_id {
        return Err(BusError::not_own("project"));
    }
    Ok(Some(PathBuf::from(row.session.worktree)))
}

/// `root` (already canonical) must be the agent's own checkout.
fn confine(own: Option<PathBuf>, root: &Path) -> Result<(), BusError> {
    match own.map(|own| fs::canonicalize(&own).unwrap_or(own)) {
        Some(own) if own != root => Err(BusError::not_own("worktree")),
        _ => Ok(()),
    }
}

/// Move `path` to `<base>/.relay/trash/<id>/payload` under the first base a rename reaches,
/// the shape purge's expiry accepts. A base that fails is left as it was found. The slot must
/// be new: an existing `<id>` belongs to an earlier delete the store no longer knows of, and
/// renaming onto its payload replaced that file or failed for good (RA-361). That case is
/// reported as `AlreadyExists`, so the caller can take another id.
fn trash_into(bases: &[&Path], id: Id, path: &Path) -> std::io::Result<PathBuf> {
    let mut failed = None;
    let mut taken = None;
    for base in bases {
        let trash = base.join(".relay").join("trash");
        let dir = trash.join(id.to_string());
        let created = fs::create_dir_all(&trash).and_then(|_| fs::create_dir(&dir));
        if let Err(e) = created {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                taken = Some(e);
            } else {
                failed = Some(e);
            }
            continue;
        }
        let payload = dir.join("payload");
        match fs::rename(path, &payload) {
            Ok(()) => return Ok(payload),
            Err(e) => {
                let _ = fs::remove_dir(&dir);
                failed = Some(e);
            }
        }
    }
    Err(taken.or(failed).unwrap_or_else(|| std::io::Error::other("no trash directory")))
}

/// A delete's payload, moved into the trash inside the transaction that records it. The engine
/// has no rollback hook, but it drops a request's after-commit work unrun when the handler
/// fails or the commit does: `file.delete` hands this to [`Ctx::after_commit`], which keeps it,
/// and dropped unkept it moves the payload back, never onto anything that took its place.
struct Untrash {
    payload: PathBuf,
    path: PathBuf,
    kept: bool,
}

impl Untrash {
    fn keep(mut self) {
        self.kept = true;
    }
}

impl Drop for Untrash {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        let back = if occupied(&self.path) {
            Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists))
        } else {
            fs::rename(&self.payload, &self.path)
        };
        match back {
            Ok(()) => {
                if let Some(dir) = self.payload.parent() {
                    let _ = fs::remove_dir(dir);
                }
            }
            Err(e) => tracing::error!(path = %self.path.display(), payload = %self.payload.display(), error = %e, "a delete that did not commit could not put its file back"),
        }
    }
}

/// The highest `<id>` directory under any base's `.relay/trash`, or 0.
fn highest_trash_slot(bases: &[&Path]) -> Id {
    bases
        .iter()
        .filter_map(|base| fs::read_dir(base.join(".relay").join("trash")).ok())
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| entry.file_name().to_str()?.parse::<Id>().ok())
        .max()
        .unwrap_or(0)
}

/// Whether anything — a dangling symlink included — already sits at `path`. `Path::exists`
/// follows links, so a dangling one read as free and was then written through (RA-146).
fn occupied(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// `root/rel`, refused when it resolves outside `root` through a symlink. A path that does not
/// exist yet is judged by its nearest existing ancestor, so a nested create, write or restore
/// can make its parents (RA-149). A symlink at the path itself must resolve: writing through
/// a dangling one creates its target wherever it points (RA-146).
fn safe_join(root: &Path, rel: &Path, may_not_exist: bool) -> Result<PathBuf, BusError> {
    let path = root.join(rel);
    let check = match fs::symlink_metadata(&path) {
        Ok(md) if md.file_type().is_symlink() => fs::canonicalize(&path).map_err(|_| {
            BusError::invalid("file.path", format!("{} is a dangling symlink", rel.display()))
        })?,
        Ok(_) => fs::canonicalize(&path).map_err(|e| io_err("file.not_found", rel, e))?,
        Err(e) if !may_not_exist => return Err(io_err("file.not_found", rel, e)),
        Err(_) => {
            let existing = path.ancestors().skip(1).find(|a| occupied(a)).unwrap_or(root);
            fs::canonicalize(existing).map_err(|e| io_err("file.not_found", rel, e))?
        }
    };
    if !check.starts_with(root) {
        return Err(BusError::invalid(
            "file.path",
            "path escapes the worktree through a symlink",
        ));
    }
    Ok(path)
}

/// `file.tree` entries per directory when the caller names no `limit`, and the most it may ask
/// for. A folder of ten thousand icons was one reply over the client's 2 MiB line cap (RA-210).
const TREE_LIMIT: u32 = 2000;
const TREE_LIMIT_MAX: u32 = 5000;

/// The first `limit` entries of `dir` in display order (folders first, then by name), each
/// directory among them listed `depth - 1` levels further. A directory cut short is recorded
/// in `truncated` with its full count. Only the entries kept are stat'ed.
/// Which paths `file.tree` and `file.search` leave out: VCS metadata, and build output or caches
/// named by [`crate::watch::generated_name`] — unless git tracks something there. The name list
/// alone hid a committed `dist/` or `scripts/build/` from the tree (RA-345, RA-638). The index is
/// read only when a generated name actually turns up.
struct Shown<'a> {
    root: &'a Path,
    index: std::cell::OnceCell<Option<gix::worktree::Index>>,
}

impl<'a> Shown<'a> {
    fn new(root: &'a Path) -> Self {
        Shown { root, index: std::cell::OnceCell::new() }
    }

    fn hidden(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(self.root) else { return true };
        if !crate::watch::is_generated_path(self.root, path) { return false; }
        if relative.starts_with(".git") { return true; }
        let index = self.index.get_or_init(|| gix::open(self.root).ok().and_then(|repo| repo.index_or_empty().ok()));
        let Some(index) = index else { return true };
        let mut prefix = String::new();
        for component in relative.components() {
            let name = component.as_os_str().to_string_lossy();
            if !prefix.is_empty() { prefix.push('/'); }
            prefix.push_str(&name);
            if crate::watch::generated_name(&name) {
                let at = gix::bstr::BStr::new(prefix.as_bytes());
                if index.entry_by_path(at).is_none() && !index.path_is_directory(at) { return true; }
            }
        }
        false
    }
}

fn list_dir(
    root: &Path,
    dir: &Path,
    depth: u32,
    badges: &HashMap<String, String>,
    shown: &Shown,
    limit: usize,
    truncated: &mut BTreeMap<String, u32>,
) -> Result<Vec<Entry>, BusError> {
    let mut found: Vec<(bool, String, PathBuf)> = fs::read_dir(dir)
        .map_err(|e| io_err("file.tree_failed", dir, e))?
        .flatten()
        .filter(|e| {
            !matches!(e.file_name().to_string_lossy().as_ref(), ".git" | ".relay")
                && !shown.hidden(&e.path())
        })
        // `DirEntry::file_type` does not follow links, the same as `entry`'s `symlink_metadata`.
        .map(|e| {
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            (!is_dir, e.file_name().to_string_lossy().to_lowercase(), e.path())
        })
        .collect();
    found.sort();
    if found.len() > limit {
        let rel = dir.strip_prefix(root).unwrap_or(dir).to_string_lossy().replace('\\', "/");
        truncated.insert(rel, found.len() as u32);
        found.truncate(limit);
    }
    let mut out = Vec::with_capacity(found.len());
    for (_, _, path) in found {
        // An entry removed since `read_dir` (an agent's temp file) is skipped, and a folder that
        // cannot be read is listed without children; either used to fail the whole tree (RA-368).
        let Ok(mut item) = entry(root, &path, badges) else { continue };
        if item.kind == EntryKind::Dir && depth > 1 {
            item.children = list_dir(root, &path, depth - 1, badges, shown, limit, truncated).ok();
        }
        out.push(item);
    }
    Ok(out)
}

fn entry(
    root: &Path,
    path: &Path,
    badges: &HashMap<String, String>,
) -> Result<Entry, BusError> {
    let md = fs::symlink_metadata(path).map_err(|e| io_err("file.stat_failed", path, e))?;
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let kind = if md.file_type().is_symlink() {
        EntryKind::Symlink
    } else if md.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let modified_at = md
        .modified()
        .ok()
        .and_then(|time| jiff::Timestamp::try_from(time).ok())
        .map(|t| t.to_string());
    Ok(Entry {
        path: rel.clone(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        kind,
        size: (kind == EntryKind::File).then_some(md.len() as i64),
        modified_at,
        badge: badges.get(&rel).cloned(),
        children: None,
    })
}

/// Gate a rename, move, delete or import by path alone (BUS.md §9.2). `content` is the file
/// that would land at `rel`; it is read only when a shape gate covers `rel` and needs the text,
/// so moving a multi-GB asset never reads it, under the lock or otherwise.
fn guard_path_mutation(
    ctx: &mut Ctx,
    project_id: Id,
    root: &Path,
    rel: &Path,
    content: Option<&Path>,
) -> Result<(), BusError> {
    let shaped = content.is_some()
        && crate::guardrail::config(ctx.tx(), Some(project_id))?
            .shape_gates
            .iter()
            .any(|gate| crate::guardrail::path_matches(&gate.path, rel));
    let text = content
        .filter(|_| shaped)
        .filter(|path| fs::metadata(path).is_ok_and(|md| md.is_file() && md.len() <= SHAPE_TEXT_MAX))
        .and_then(|path| fs::read_to_string(path).ok());
    guardrail::enforce_path_mutation(ctx, project_id, root, &rel.to_string_lossy(), text.as_deref())
}

/// Shape gates validate configuration files; anything larger is not one, and is held unread.
const SHAPE_TEXT_MAX: u64 = 4 * 1024 * 1024;

/// Every path inside the directory `dir` that a protected path or shape gate would cover once
/// it sits under one of `bases` (where it is now, where it is going), relative to `dir`.
/// Gating a directory by its own path alone let `config/` be deleted or moved with the
/// protected `config/prod.env` inside it (RA-147). A tree walk, so it runs in the read phase
/// of the staged op, with nothing locked; symlinked directories are not followed, the same
/// as the rename or copy they precede. Empty for anything that is not a directory.
fn covered_inside(
    ctx: &Unlocked,
    project_id: Id,
    dir: &Path,
    bases: &[&Path],
) -> Result<Vec<PathBuf>, BusError> {
    if !fs::symlink_metadata(dir).is_ok_and(|md| md.is_dir()) {
        return Ok(Vec::new());
    }
    let cfg = ctx.read(|conn| crate::guardrail::config(conn, Some(project_id)))?;
    let patterns: Vec<&str> = cfg
        .protected_paths
        .iter()
        .map(String::as_str)
        .chain(cfg.shape_gates.iter().map(|gate| gate.path.as_str()))
        .collect();
    let mut covered = Vec::new();
    if patterns.is_empty() {
        return Ok(covered);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current).map_err(|e| io_err("file.walk_failed", &current, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| io_err("file.walk_failed", &current, e))?;
            let path = entry.path();
            let inner = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
            if bases.iter().any(|base| {
                let at = base.join(&inner);
                patterns.iter().any(|pattern| crate::guardrail::path_matches(pattern, &at))
            }) {
                covered.push(inner);
            }
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path);
            }
        }
    }
    Ok(covered)
}

/// A scratch path under the worktree's `.relay/tmp`, removed when dropped unless its contents
/// were moved into place. A staged op that never reaches its transaction — a refused hold, a
/// failed confirm — leaves nothing behind.
struct Staging(PathBuf);

impl Staging {
    fn new(root: &Path) -> Result<Self, BusError> {
        let path = root.join(".relay").join("tmp").join(format!("import-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).map_err(|e| io_err("file.import_failed", &path, e))?;
        Ok(Self(path))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What a staged rename, move or delete settled before its transaction.
struct PreparedPath {
    project_id: Id,
    root: PathBuf,
    from_rel: PathBuf,
    /// Where a rename or move lands; `None` for a delete.
    into_rel: Option<PathBuf>,
    /// Paths inside a directory source that a gate covers, relative to it ([`covered_inside`]).
    inner: Vec<PathBuf>,
}

/// The transaction half of `file.rename` and `file.move`: gate the source and destination,
/// and every gated path inside a directory at both ends, then rename.
fn relocate(ctx: &mut Ctx, prepared: PreparedPath, code: &str) -> Result<Entry, BusError> {
    let PreparedPath { project_id, root, from_rel, into_rel, inner } = prepared;
    let into_rel = into_rel.ok_or_else(|| BusError::internal("relocate without a destination"))?;
    let (from, into) = (root.join(&from_rel), root.join(&into_rel));
    guard_path_mutation(ctx, project_id, &root, &from_rel, None)?;
    guard_path_mutation(ctx, project_id, &root, &into_rel, Some(&from))?;
    for inside in &inner {
        guard_path_mutation(ctx, project_id, &root, &from_rel.join(inside), None)?;
        guard_path_mutation(ctx, project_id, &root, &into_rel.join(inside), Some(&from.join(inside)))?;
    }
    if occupied(&into) {
        return Err(BusError::conflict(
            "file.exists",
            format!("{} already exists", into_rel.display()),
        ));
    }
    fs::rename(&from, &into).map_err(|e| io_err(code, &from_rel, e))?;
    changed(ctx, project_id, &root, &into_rel.to_string_lossy());
    entry(&root, &into, &HashMap::new())
}

/// What a staged `file.restore` settled before its transaction.
struct PreparedRestore {
    project_id: Id,
    /// The checkout it goes back to, verified as one of the project's and canonical.
    root: PathBuf,
    /// The checkout it was deleted from is gone, so `root` is the primary checkout (RA-214).
    fallback: bool,
    rel: PathBuf,
    trash: PathBuf,
    /// Paths inside a directory payload that a gate covers, relative to it.
    inner: Vec<PathBuf>,
}

/// The worktree, original path and payload of trash row `id`, if it is still unrestored.
fn open_trash(conn: &Connection, id: Id, project_id: Id) -> Result<(String, String, String), BusError> {
    conn.prepare_cached(
        "SELECT worktree, original_path, trash_path FROM file_trash WHERE id=?1 AND project_id=?2 AND restored_at IS NULL",
    ).bus()?
    .query_row(params![id, project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .optional().bus()?
    .ok_or_else(|| BusError::not_found("file.trash_not_found", format!("no open trash {id}")))
}

/// One source of a staged `file.import`, already copied into the staging directory.
struct StagedImport {
    staged: PathBuf,
    dest_rel: PathBuf,
    inner: Vec<PathBuf>,
}

struct PreparedImport {
    project_id: Id,
    root: PathBuf,
    items: Vec<StagedImport>,
    _staging: Staging,
}

fn changed(ctx: &mut Ctx, project_id: Id, root: &Path, path: &str) {
    ctx.set_project(project_id);
    ctx.emit(
        "file.changed",
        json!({"project_id": project_id, "worktree": root, "path": path}),
    );
}

/// The text `file.write` is replacing: a regular file only, and never more than [`OLD_TEXT_MAX`]
/// of it. Read before the transaction, and again under the store lock when the file changed in
/// between. `fs::read_to_string` on a FIFO there blocked every request for good, and on a huge
/// file filled memory. Anything else reads as an error, as a missing file does: it matches no
/// `expected_sha256`, and counts as empty for the line counts.
fn read_current(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;
    let md = fs::metadata(path)?;
    if !md.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let too_big = || std::io::Error::other("too large to compare");
    if md.len() > OLD_TEXT_MAX {
        return Err(too_big());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(OLD_TEXT_MAX + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > OLD_TEXT_MAX {
        return Err(too_big());
    }
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// What a staged `file.write` read before its transaction.
struct PreparedWrite {
    project_id: Id,
    root: PathBuf,
    rel: PathBuf,
    /// Where the text lands ([`landing`]), and the file there as it was when it was read.
    path: PathBuf,
    stamp: Result<[i64; 7], std::io::ErrorKind>,
    /// The old file's SHA-256, when the caller named the one it expects and the file was readable.
    old_sha: Option<String>,
    /// Lines removed and added, against the file as read.
    counts: (i64, i64),
    probes: crate::guardrail::Probes,
}

/// Where a write to `root/rel` lands, and the target's worktree-relative path when `rel` is a
/// symlink. A symlink is written through, not replaced: renaming the new text over the link
/// turned it into a regular file and left its target as it was (RA-146). [`safe_join`] has
/// confirmed the target is inside the worktree, and it is the target that is gated.
fn landing(root: &Path, rel: &Path) -> Result<(PathBuf, Option<PathBuf>), BusError> {
    let path = safe_join(root, rel, true)?;
    if fs::symlink_metadata(&path).is_ok_and(|md| md.file_type().is_symlink()) {
        let target = fs::canonicalize(&path).map_err(|e| io_err("file.write_failed", rel, e))?;
        let target_rel = target.strip_prefix(root).unwrap_or(&target).to_path_buf();
        return Ok((target, Some(target_rel)));
    }
    Ok((path, None))
}

/// Enough of what sits at `path` to tell whether it changed since it was read: replaced (a
/// rename, which is how every `file.write` lands), rewritten in place or truncated, or gone.
/// The change time cannot be set back by the writer, so a rewrite that restores the size and
/// modification time still shows. An error stands for itself: a missing file is not an
/// unreadable one.
fn stamp(path: &Path) -> Result<[i64; 7], std::io::ErrorKind> {
    use std::os::unix::fs::MetadataExt;
    let md = fs::metadata(path).map_err(|e| e.kind())?;
    Ok([md.dev() as i64, md.ino() as i64, md.size() as i64, md.mtime(), md.mtime_nsec(), md.ctime(), md.ctime_nsec()])
}

fn sha256_hex(text: &str) -> String {
    crate::hex(&Sha256::digest(text.as_bytes()))
}

/// Refuse a draft whose base is not the file there now: `old_sha` is `None` when the file is
/// missing or unreadable, which matches no expectation.
fn expect_sha(expected: Option<&str>, old_sha: Option<&str>) -> Result<(), BusError> {
    match expected {
        Some(expected) if old_sha != Some(expected) => Err(BusError::conflict(
            "file.edit_conflict",
            "File changed or was removed. Your draft has not been written.",
        )),
        _ => Ok(()),
    }
}

/// The guardrail's comparison cap: past it, the destructive-write rule does not read the old file
/// either. `file.read` returns at most half of this, so no draft is of a file this big.
const OLD_TEXT_MAX: u64 = 64 * 1024 * 1024;

/// How long `file.write` may spend diffing for its line counts. It runs before the transaction,
/// and under the store lock when the file changed since, and Myers is quadratic in the worst
/// case: a rewrite of a large generated file took seconds with every other request queued
/// behind it (RA-145). Past the deadline `similar` finishes with a coarser diff, so the counts
/// stay a true (if not minimal) account of the change.
const LINE_COUNT_DEADLINE: std::time::Duration = std::time::Duration::from_millis(50);

fn line_counts(old: &str, new: &str) -> (i64, i64) {
    let mut removed = 0;
    let mut added = 0;
    let diff = TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .timeout(LINE_COUNT_DEADLINE)
        .diff_lines(old, new);
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => removed += 1,
            ChangeTag::Insert => added += 1,
            ChangeTag::Equal => {}
        }
    }
    (removed, added)
}

fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let md = fs::symlink_metadata(from)?;
    if md.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(fs::read_link(from)?, to)?;
    } else if md.is_dir() {
        fs::create_dir_all(to)?;
        for e in fs::read_dir(from)? {
            let e = e?;
            copy(&e.path(), &to.join(e.file_name()))?;
        }
    } else {
        fs::copy(from, to)?;
    }
    Ok(())
}

/// `file.search`'s `glob` against a worktree-relative file path. The guardrail matcher keeps
/// `*` inside one segment, so `*.rs` found only root-level files; as in ripgrep, a glob with
/// no `/` also names files anywhere by their file name (RA-367).
fn search_glob_matches(glob: &str, relp: &Path) -> bool {
    crate::guardrail::path_matches(glob, relp)
        || (!glob.contains('/')
            && relp.file_name().is_some_and(|name| crate::guardrail::path_matches(glob, Path::new(name))))
}

/// Files larger than this are not searched: at that size they are assets or generated output,
/// and reading them whole on every search is the cost the editor's re-search pays repeatedly.
const SEARCH_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// How much of a file is read first to decide whether it is text at all.
const SEARCH_SNIFF_BYTES: usize = 8 * 1024;

/// Read `path` into `buffer` if it is a searchable text file: a regular file (a symlink only
/// when its target stays inside `root`, the same rule as file.read), no larger than
/// [`SEARCH_MAX_BYTES`], whose first few KB hold no NUL and valid UTF-8. A FIFO, socket or
/// device is never opened, and a binary asset costs one small read rather than its full size.
fn read_searchable(root: &Path, path: &Path, buffer: &mut Vec<u8>) -> bool {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let Ok(link) = fs::symlink_metadata(path) else { return false };
    if link.file_type().is_symlink()
        && !fs::canonicalize(path).is_ok_and(|target| target.starts_with(root))
    {
        return false;
    }
    let Ok(md) = fs::metadata(path) else { return false };
    if !md.is_file() || md.len() > SEARCH_MAX_BYTES {
        return false;
    }
    // O_NONBLOCK: a file swapped for a FIFO after the check above cannot hang the open.
    let Ok(file) = fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|md| md.is_file()) {
        return false;
    }
    buffer.clear();
    let mut file = file.take(SEARCH_MAX_BYTES);
    if (&mut file).take(SEARCH_SNIFF_BYTES as u64).read_to_end(buffer).is_err() {
        return false;
    }
    let sniff = &buffer[..];
    if sniff.contains(&0) {
        return false;
    }
    if let Err(error) = std::str::from_utf8(sniff) {
        // A multi-byte character cut by the sniff boundary is not evidence of binary.
        if error.error_len().is_some() {
            return false;
        }
    }
    if buffer.len() < SEARCH_SNIFF_BYTES {
        return true;
    }
    file.read_to_end(buffer).is_ok()
}

/// The longest hit `text` file.search returns, in bytes. Hits were whole lines, and one match
/// in a minified bundle or source map made a reply the client could not read (RA-212).
const HIT_TEXT_MAX: usize = 240;
/// How much of the line before the match a cut hit keeps.
const HIT_TEXT_BEFORE: usize = 80;

/// `line` whole when it is short, or a window of at most [`HIT_TEXT_MAX`] bytes around the match
/// at byte `at`, cut on character boundaries. Returns the text and its byte offset in `line`.
fn hit_window(line: &str, at: usize) -> (&str, usize) {
    if line.len() <= HIT_TEXT_MAX {
        return (line, 0);
    }
    let mut start = at.saturating_sub(HIT_TEXT_BEFORE);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + HIT_TEXT_MAX).min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    (&line[start..end], start)
}

/// `file.read`'s bytes as text, or as base64 when they are not text. A read cut at `max_bytes`
/// can end inside a multi-byte character; that incomplete tail is dropped rather than taken as
/// evidence of binary, which turned a large non-ASCII text file into base64 (RA-359).
fn decode(bytes: Vec<u8>, truncated: bool) -> (Option<String>, Option<String>) {
    let text = match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(e) if truncated && e.utf8_error().error_len().is_none() => {
            let valid = e.utf8_error().valid_up_to();
            let mut bytes = e.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).map_err(|e| e.into_bytes())
        }
        Err(e) => Err(e.into_bytes()),
    };
    let b64 = |bytes: &[u8]| Some(base64::engine::general_purpose::STANDARD.encode(bytes));
    match text {
        Ok(text) if !text.contains('\0') => (Some(text), None),
        Ok(text) => (None, b64(text.as_bytes())),
        Err(bytes) => (None, b64(&bytes)),
    }
}

fn mime(path: &Path) -> String {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" | "ts" | "js" | "svelte" | "kt" | "kts" | "md" | "toml" | "yaml" | "yml" | "css"
        | "html" => "text/plain",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// A filesystem error as a bus error whose kind says whose problem it is (RA-369): a missing
/// path is the caller's `not_found` and an occupied one a `conflict`, not an outage. Permission
/// errors stay `unavailable`; `refused` is the guardrail's kind and is audited as one.
fn io_err(code: &str, path: impl AsRef<Path>, e: std::io::Error) -> BusError {
    let message = format!("{}: {e}", path.as_ref().display());
    match e.kind() {
        std::io::ErrorKind::NotFound => BusError::not_found(code, message),
        std::io::ErrorKind::AlreadyExists => BusError::conflict(code, message),
        _ => BusError::unavailable(code, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RA-145: every line different is Myers' worst case, quadratic in the line count. The
    /// deadline bounds it, and the counts it settles on are still the whole change.
    #[test]
    fn line_counts_for_a_total_rewrite_return_within_the_deadline() {
        let old: String = (0..20_000).map(|i| format!("old {i}\n")).collect();
        let new: String = (0..20_000).map(|i| format!("new {i}\n")).collect();
        let started = std::time::Instant::now();
        assert_eq!(line_counts(&old, &new), (20_000, 20_000));
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
        assert_eq!(line_counts("a\nb\nc\n", "a\nc\nd\n"), (1, 1));
    }

    /// RA-212: a long line is cut to a window that starts before the match and still holds it,
    /// on character boundaries, and says where in the line it starts.
    #[test]
    fn a_long_hit_line_is_cut_to_a_window_around_the_match() {
        assert_eq!(hit_window("short line", 6), ("short line", 0));
        let line = format!("{}needle{}", "é".repeat(5000), "x".repeat(5000));
        let at = line.find("needle").unwrap();
        let (text, offset) = hit_window(&line, at);
        assert!(text.len() <= HIT_TEXT_MAX && text.len() > HIT_TEXT_MAX - 4, "{}", text.len());
        assert!(offset <= at && at - offset <= HIT_TEXT_BEFORE);
        assert_eq!(&text[at - offset..at - offset + 6], "needle");
        assert_eq!(&line[offset..offset + text.len()], text);
        // At the very start or end of a line the window runs to that edge.
        assert_eq!(hit_window(&line, 0).1, 0);
        let (tail, offset) = hit_window(&line, line.len() - 1);
        assert_eq!(offset + tail.len(), line.len());
    }

    /// RA-359: a read cut inside a multi-byte character is still text, minus the partial
    /// character; invalid UTF-8 elsewhere, or untruncated, is still binary.
    #[test]
    fn a_truncated_read_that_splits_a_character_stays_text() {
        let mut cut = "héllo é".as_bytes().to_vec();
        cut.pop();
        assert_eq!(decode(cut.clone(), true), (Some("héllo ".to_string()), None));
        assert_eq!(decode(cut, false).0, None);
        assert_eq!(decode(vec![b'a', 0xff, b'b'], true).0, None);
        assert_eq!(decode(b"a\0b".to_vec(), false).0, None);
        assert_eq!(decode(b"plain".to_vec(), false), (Some("plain".to_string()), None));
    }

    /// RA-367: a glob with no `/` names files at any depth by their file name; one with a `/`
    /// is matched against the whole path, as before.
    #[test]
    fn a_search_glob_without_a_slash_matches_by_file_name() {
        assert!(search_glob_matches("*.rs", Path::new("main.rs")));
        assert!(search_glob_matches("*.rs", Path::new("crates/core/src/lib.rs")));
        assert!(!search_glob_matches("*.rs", Path::new("crates/core/src/lib.ts")));
        assert!(search_glob_matches("src/**", Path::new("src/a/b.rs")));
        assert!(!search_glob_matches("src/*.rs", Path::new("lib/src/a.rs")));
        assert!(search_glob_matches("src", Path::new("src/a.rs")), "the prefix rule still holds");
    }

    /// RA-361: an existing `.relay/trash/<id>` is never moved onto; the move reports the slot as
    /// taken and leaves both files where they were.
    #[test]
    fn a_taken_trash_slot_is_refused_not_overwritten() {
        let base = tempfile::tempdir().unwrap();
        let stale = base.path().join(".relay/trash/1");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("payload"), "older").unwrap();
        let doomed = base.path().join("doomed.txt");
        fs::write(&doomed, "newer").unwrap();
        let error = trash_into(&[base.path()], 1, &doomed).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(stale.join("payload")).unwrap(), "older");
        assert!(doomed.exists());
        assert_eq!(highest_trash_slot(&[base.path()]), 1);
        let payload = trash_into(&[base.path()], 2, &doomed).unwrap();
        assert_eq!(fs::read_to_string(payload).unwrap(), "newer");
    }

    /// RA-345: a tracked `dist/` or `scripts/build/` is source, shown and searched; an untracked
    /// `dist/` or `target/` is build output, left out.
    #[test]
    fn generated_names_are_hidden_only_where_git_tracks_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let root = &std::fs::canonicalize(directory.path()).unwrap();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(root).args(args).status().unwrap().success());
        git(&["init", "-q", "-b", "main"]);
        for file in ["action/dist/index.js", "scripts/build/run.sh", "src/main.rs", "web/dist/out.js", "target/debug/app"] {
            std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
            std::fs::write(root.join(file), "x").unwrap();
        }
        git(&["add", "action", "scripts", "src"]);
        let shown = Shown::new(root);
        for visible in ["action/dist", "action/dist/index.js", "action/dist/new.js", "scripts/build", "src"] {
            assert!(!shown.hidden(&root.join(visible)), "{visible} is hidden");
        }
        for hidden in ["web/dist", "target", "target/debug", ".git", "action/dist/node_modules"] {
            assert!(shown.hidden(&root.join(hidden)), "{hidden} is shown");
        }
    }
}
