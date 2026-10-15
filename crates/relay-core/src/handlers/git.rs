//! `worktree.*` (phase 3). `git.*` and `integration.*` land in phase 8.

use crate::engine::{Ctx, Engine, IntoBus, Unlocked};
use crate::handlers::workspace::get_project;
use crate::worktree;
use relay_bus::error::BusError;
use relay_bus::ops::app::WorktreeDisk as WorktreeDiskRow;
use relay_bus::ops::git::*;
use relay_bus::types::{Branch, Commit, DiffFile, FileStatus, GateKind, Hunk, Worktree};
use rusqlite::{Connection, Transaction};
use serde::Deserialize;
use serde_json::json;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Live sessions of a project as (branch, name), for `git.branches`.
fn worktree_owners_by_branch(
    conn: &Connection,
    project_id: relay_bus::types::Id,
) -> Result<Vec<(String, String)>, BusError> {
    let mut stmt = conn
        .prepare_cached("SELECT branch,name FROM sessions WHERE project_id=?1 AND state!='closed'")
        .bus()?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>();
    rows.bus()
}

/// Live sessions of a project as (name, worktree). The store half of [`list_with_owners`].
pub fn worktree_owners(
    conn: &Connection,
    project_id: relay_bus::types::Id,
) -> Result<Vec<(String, String)>, BusError> {
    let mut st = conn
        .prepare_cached(
            "SELECT name, worktree FROM sessions WHERE project_id = ?1 AND state != 'closed'",
        )
        .bus()?;
    let owners = st
        .query_map([project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .bus()?
        .collect::<rusqlite::Result<Vec<_>>>();
    owners.bus()
}

/// `git worktree list` plus the owning session names already read out of the store. The
/// subprocess belongs outside the lock, so callers read `owners` first and pass it in (D144).
pub fn worktrees_owned_by(
    owners: &[(String, String)],
    repo: &Path,
    include_dirty: bool,
) -> Result<Vec<Worktree>, BusError> {
    let mut wts = worktree::list_with_dirty(repo, include_dirty)
        .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?;
    for w in &mut wts {
        w.session = owners
            .iter()
            .find(|(_, path)| *path == w.path)
            .map(|(n, _)| n.clone());
    }
    Ok(wts)
}

/// Worktrees of a project with the owning session filled in from the store.
pub fn list_with_owners(
    tx: &Transaction,
    project_id: relay_bus::types::Id,
    repo: &Path,
    include_dirty: bool,
) -> Result<Vec<Worktree>, BusError> {
    let owners = worktree_owners(tx, project_id)?;
    worktrees_owned_by(&owners, repo, include_dirty)
}

/// How long the GitHub lookup may take before Relay stops waiting on the network.
const PR_LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// How long one listing answers `git.pr.list` for a repository. The Source Control panel asks
/// on every refresh, which while agents work is about once a second; asking GitHub each time
/// spends the user's GitHub REST quota (D130).
const PR_LIST_TTL: std::time::Duration = std::time::Duration::from_secs(60);
/// A failed lookup answers for this long too: offline, `gh` waits out its whole timeout, and
/// signed out it still costs a process and a round trip, on every refresh. `refresh` (the
/// panel's Refresh key) asks again at once.
const PR_LIST_FAILURE_TTL: std::time::Duration = std::time::Duration::from_secs(60);
/// The closed and merged PRs listed beside every open one: the most recently updated, one
/// page. Older ones are what a branch long merged or abandoned had; `branch_cleanup` asks
/// GitHub about those branch by branch.
const PR_LIST_RECENT: u32 = 100;
/// `gh pr create` makes several GitHub round trips. Under the desktop client's 30 s request
/// timeout, so the engine reports a timeout before the client gives up on its own.
const PR_OPEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

type PrListed = Result<PrListOut, BusError>;
type PrCache = std::collections::HashMap<PathBuf, (std::time::Instant, PrListed)>;
static PR_LISTS: std::sync::Mutex<Option<PrCache>> = std::sync::Mutex::new(None);

fn pr_ttl(listed: &PrListed) -> std::time::Duration {
    if listed.is_ok() { PR_LIST_TTL } else { PR_LIST_FAILURE_TTL }
}

/// The listing, or the failure, this repository's last lookup answered within its TTL.
fn cached_prs(repo: &Path) -> Option<PrListed> {
    let cache = PR_LISTS.lock().unwrap_or_else(|p| p.into_inner());
    cache.as_ref()?.get(repo).filter(|(at, listed)| at.elapsed() < pr_ttl(listed)).map(|(_, listed)| listed.clone())
}

fn remember_prs(repo: &Path, listed: &PrListed) {
    let mut cache = PR_LISTS.lock().unwrap_or_else(|p| p.into_inner());
    let cache = cache.get_or_insert_with(Default::default);
    cache.retain(|_, (at, listed)| at.elapsed() < pr_ttl(listed));
    cache.insert(repo.to_path_buf(), (std::time::Instant::now(), listed.clone()));
}

/// A push or a new PR changes what GitHub would say; the next listing asks again.
fn forget_prs(repo: &Path) {
    if let Some(cache) = PR_LISTS.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        cache.remove(repo);
    }
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<WorktreeList>(|ctx, p| {
        let (project, owners) = ctx.read(|conn| {
            let project = get_project(conn, p.project_id)?;
            let owners = worktree_owners(conn, project.id)?;
            Ok((project, owners))
        })?;
        Ok(WorktreeListOut {
            worktrees: worktrees_owned_by(
                &owners,
                Path::new(&project.path),
                p.include_dirty.unwrap_or(true),
            )?,
        })
    });
    // `git worktree add` checks a tree out; the store has nothing to do while it runs (D144).
    e.register_staged::<WorktreeCreate, _>(
        |ctx, p| {
            let project = ctx.read(|conn| get_project(conn, p.project_id))?;
            let repo = Path::new(&project.path);
            let name = p.branch.trim_start_matches("relay/").replace('/', "-");
            let path = worktree::pooled_path(repo, &name);
            if path.exists() {
                return Err(BusError::conflict(
                    "worktree.exists",
                    format!("{} already exists", path.display()),
                ));
            }
            let start = new_worktree_start(repo, &p.branch, &project.base_branch, p.from.as_deref())?;
            let wt = worktree::create_at(repo, &path, &p.branch, &start)
                .map_err(|e| BusError::conflict("worktree.create_failed", e.to_string()))?;
            Ok((project, wt))
        },
        |ctx: &mut Ctx, _p, (project, wt)| {
            ctx.set_project(project.id);
            ctx.emit("worktree.changed", json!({ "project_id": project.id }));
            Ok(wt)
        },
    );
    // The build purge, the size walk and `git worktree remove` are seconds of disk work for a
    // checkout with real build output: done before the transaction, which only announces it.
    // `git worktree remove --force` deletes whatever the checkout holds, so a dirty one is
    // refused first, as `session.close` refuses it (RA-405); a detached HEAD on no branch is
    // pinned under `refs/relay/rescue/` and named in the answer.
    e.register_staged::<WorktreeRemove, _>(
        |ctx, p| {
            // A relative path would resolve against the engine's cwd ($HOME under `relay serve`).
            if !Path::new(&p.path).is_absolute() {
                return Err(BusError::invalid("worktree.path", format!("{:?} must be an absolute path", p.path)));
            }
            let want = std::fs::canonicalize(&p.path).map(|c| c.display().to_string()).unwrap_or(p.path.clone());
            let (project, owner) = ctx.read(|conn| {
                let project = get_project(conn, p.project_id)?;
                let owner: Option<String> = conn.query_row(
                    "SELECT name FROM sessions WHERE project_id = ?1 AND worktree = ?2 AND state != 'closed'",
                    rusqlite::params![project.id, want], |r| r.get(0)).ok();
                Ok((project, owner))
            })?;
            if let Some(name) = owner {
                return Err(BusError::conflict("worktree.owned", format!("session {name} owns {want}")).with_hint("session.close it first"));
            }
            let refuse = |e: anyhow::Error| BusError::conflict("worktree.remove_failed", e.to_string());
            worktree::removable(Path::new(&project.path), Path::new(&want)).map_err(refuse)?;
            if !p.discard_changes.unwrap_or(false) {
                super::session::refuse_dirty(&[(std::path::PathBuf::from(&want), Vec::new())])?;
            }
            let removed = worktree::remove_rescuing(Path::new(&project.path), Path::new(&want), p.purge_build.unwrap_or(true)).map_err(refuse)?;
            Ok((project.id, removed.freed, removed.rescued))
        },
        |ctx: &mut Ctx, _p, (project_id, freed, rescued): (relay_bus::types::Id, u64, Option<String>)| {
            ctx.set_project(project_id);
            ctx.emit("worktree.changed", json!({ "project_id": project_id }));
            Ok(FreedOut { freed_mb: freed as f64 / (1024.0 * 1024.0), rescued })
        },
    );
    e.register_unlocked::<WorktreeDisk>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        let wts = worktree::list_with_dirty(Path::new(&project.path), false)
            .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?;
        let worktrees = wts
            .iter()
            .map(|w| {
                // One walk for both figures; the build dirs used to be walked a second time (RA-629).
                let (total, build) = worktree::disk_usage(Path::new(&w.path));
                WorktreeDiskRow {
                    path: w.path.clone(),
                    disk_mb: total as f64 / (1024.0 * 1024.0),
                    build_mb: Some(build as f64 / (1024.0 * 1024.0)),
                }
            })
            .collect();
        Ok(WorktreeDiskOut { worktrees })
    });
    // Rev walks over cached refs: no network, and nothing the store has to wait for.
    e.register_unlocked::<WorktreeDrift>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        drift(Path::new(&project.path), &project.base_branch)
    });
    e.register_unlocked::<Status>(|ctx, p| {
        let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        crate::watch::ensure_after_commit(ctx, root.clone(), project.id);
        let repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        let branch = repo
            .head_name()
            .ok()
            .flatten()
            .map(|n| n.shorten().to_string())
            .unwrap_or_else(|| "HEAD".into());
        let (upstream, ahead, behind) = upstream_metrics(&repo, &branch);
        let files = worktree::status_files_with(&root, worktree::Untracked::Directories)
            .map_err(git_mutation("git.status_failed"))?;
        let total = files.len() as u64;
        let files = cap_status(files);
        Ok(StatusOut {
            branch,
            upstream,
            ahead,
            behind,
            truncated: (files.len() as u64) < total,
            total,
            files,
        })
    });
    e.register_unlocked::<Diff>(|ctx, p| {
        let (_project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        // One repository, one comparison tree and one index for every file, not one of each
        // per file (RA-157).
        let mut repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        repo.object_cache_size_if_unset(16 * 1024 * 1024);
        let old_tree = revision_tree(&repo, p.base.as_deref().unwrap_or("HEAD"))?;
        let mut files = match (&p.base, &old_tree) {
            (Some(_), Some(old_tree)) => {
                let new_tree = repo
                    .head_commit()
                    .map_err(gix_err("git.diff_failed"))?
                    .tree()
                    .map_err(gix_err("git.diff_failed"))?;
                tree_diff_files(&repo, old_tree, &new_tree, "git.diff_failed")?
            }
            _ => Vec::new(),
        };
        let staged = p.staged == Some(true);
        let index = if staged {
            Some(repo.index_or_empty().map_err(gix_err("git.index"))?)
        } else {
            None
        };
        for status in status_files(&root)? {
            if staged && status.index.is_empty() {
                continue;
            }
            // A rename's old side is its source, not an empty file at its new name.
            let source = status.renamed_from.as_deref().unwrap_or(&status.path);
            let old = tree_side(&repo, old_tree.as_ref(), source, DIFF_COUNT_MAX)?;
            let new = match &index {
                Some(index) => index_side(&repo, index, &status.path, DIFF_COUNT_MAX)?,
                None => work_side(&root, &status.path, DIFF_COUNT_MAX)?,
            };
            let file = diff_file(
                status.path,
                status.renamed_from,
                if !status.index.is_empty() {
                    status.index
                } else {
                    status.worktree
                },
                &old,
                &new,
            );
            if let Some(existing) = files.iter_mut().find(|existing| existing.path == file.path) {
                *existing = file;
            } else {
                files.push(file);
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(DiffOut { files })
    });
    e.register_unlocked::<DiffFileOp>(|ctx, p| {
        let (_project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        validate_path(&p.path)?;
        if let Some(old_path) = &p.old_path {
            validate_path(old_path)?;
        }
        // The reply carries both sides and the diff on one line; stop at the editor's limit
        // before reading, rather than build a reply larger than a client can read. Neither
        // side is read past that limit, and a symlink is its target's name, as git stores it,
        // never the file it points at (RA-156).
        let repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        let old_tree = revision_tree(&repo, p.base.as_deref().unwrap_or("HEAD"))?;
        let max = DIFF_FILE_MAX as u64;
        // A staged row is what the commit would record, not the edits made since (RA-206).
        let new = if p.staged == Some(true) {
            let index = repo.index_or_empty().map_err(gix_err("git.index"))?;
            index_side(&repo, &index, &p.path, max)?
        } else {
            work_side(&root, &p.path, max)?
        };
        // A rename's old side is its source, not an empty file at its new name.
        let old = tree_side(&repo, old_tree.as_ref(), p.old_path.as_deref().unwrap_or(&p.path), max)?;
        let (old, new) = match (old, new) {
            (Side::TooLarge, _) | (_, Side::TooLarge) => return Err(diff_too_large()),
            (Side::Text(old), Side::Text(new)) if old.len() + new.len() <= DIFF_FILE_MAX => (old, new),
            (Side::Text(_), Side::Text(_)) => return Err(diff_too_large()),
            // Same test git.diff uses for its `binary` flag.
            _ => return Err(BusError::refused("git.diff_binary", "This file is binary; there is no text diff to show.")),
        };
        let hunks = if old == new {
            Vec::new()
        } else {
            vec![Hunk {
                old_start: 1,
                old_lines: old.lines().count() as i64,
                new_start: 1,
                new_lines: new.lines().count() as i64,
                text: TextDiff::configure()
                    .deadline(std::time::Instant::now() + DIFF_DEADLINE)
                    .diff_lines(&old, &new)
                    .unified_diff()
                    .context_radius(3)
                    .to_string(),
            }]
        };
        Ok(DiffFileOut { old, new, hunks })
    });
    e.register_unlocked::<Log>(|ctx, p| {
        let (_project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let mut repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        repo.object_cache_size_if_unset(16 * 1024 * 1024);
        let revision = p.branch.as_deref().unwrap_or("HEAD");
        let tip = repo.rev_parse_single(revision).map_err(revision_err(revision))?;
        // `git log --topo-order`, as `--graph` uses: no parent before all of its children. The
        // default breadth-first walk put a parent above a child on another line of history,
        // and the graph drew a lane for each such inversion (RA-150). The commit-graph file,
        // when there is one, keeps the walk from reading all of history first.
        let walk = gix::traverse::commit::topo::Builder::from_iters(&repo.objects, [tip.detach()], None::<Vec<gix::ObjectId>>)
            .sorting(gix::traverse::commit::topo::Sorting::TopoOrder)
            .with_commit_graph(repo.commit_graph_if_enabled().ok().flatten())
            .build()
            .map_err(gix_err("git.log_failed"))?;
        let mut commits = Vec::new();
        for info in walk.take(p.limit.unwrap_or(50).min(500) as usize) {
            let info = info.map_err(gix_err("git.log_failed"))?;
            commits.push(commit_from_gix(
                repo.find_commit(info.id).map_err(gix_err("git.log_failed"))?,
            )?);
        }
        Ok(LogOut { commits })
    });
    e.register_unlocked::<Show>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        let repo = gix::open(&project.path).map_err(gix_err("git.open_failed"))?;
        let id = repo
            .rev_parse_single(p.sha.as_str())
            .map_err(revision_err(&p.sha))?;
        let object = id
            .object()
            .map_err(gix_err("git.show_failed"))?
            .peel_to_commit()
            .map_err(gix_err("git.show_failed"))?;
        let files = commit_diff_files(&repo, &object)?;
        let commit = commit_from_gix(object)?;
        Ok(ShowOut { commit, files })
    });
    e.register_unlocked::<Branches>(|ctx, p| {
        let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let owner_rows = ctx.read(|conn| {
            let owners = worktree_owners_by_branch(conn, project.id)?;
            Ok(owners)
        })?;
        let owners: HashMap<String, String> = owner_rows.into_iter().collect();
        // The same single walk and object cache as branch.delete (RA-151).
        branches_with(&root, &project.base_branch, &owners, true)
    });
    // `git switch -c <name> <start>` checks out whatever differs from the start point, through
    // any LFS or clean/smudge filters: never under the store lock (D149).
    e.register_staged::<BranchCreate, _>(|ctx, p| {
        let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        // Checking the new branch out moves the checkout's branch, which in a live session's
        // checkout is its agent's call — the same rule git.branch.switch keeps (RA-152).
        if p.checkout.unwrap_or(true) {
            refuse_session_checkout(ctx, project.id, &root)?;
        }
        let name = validate_branch_name(&root, &p.name)?;
        let repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        let full_name = format!("refs/heads/{name}");
        if repo.find_reference(full_name.as_str()).is_ok() {
            return Err(BusError::conflict(
                "git.branch_exists",
                format!("branch {name} already exists"),
            ));
        }
        let start = p
            .start_point
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(revision) = start {
            repo.rev_parse_single(revision).map_err(|_| {
                BusError::invalid("git.start_point", format!("unknown start point {revision}"))
            })?;
        }
        let mut args = if p.checkout.unwrap_or(true) {
            vec!["switch", "-c", name.as_str()]
        } else {
            vec!["branch", name.as_str()]
        };
        if let Some(revision) = start {
            args.extend(["--end-of-options", revision]);
        }
        worktree::git_mutate(&root, &args).map_err(git_mutation("git.branch_create_failed"))?;
        // The new branch's tip: `git branch` leaves HEAD where it was (RA-371).
        let repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
        let head = repo
            .find_reference(full_name.as_str())
            .map_err(gix_err("git.head"))?
            .peel_to_id()
            .map_err(gix_err("git.head"))?
            .to_string();
        Ok((project, root, name, head))
    }, |ctx: &mut Ctx, _p, (project, root, name, head)| {
        changed(ctx, project.id, &root);
        Ok(BranchCreateOut {
            name,
            head,
            worktree: root.display().to_string(),
        })
    });
    // `git status` and `git switch` both walk the checkout; neither needs the store (D144).
    e.register_staged::<BranchSwitch, _>(
        |ctx, p| {
            let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
            refuse_session_checkout(ctx, project.id, &root)?;
            let target = switch_target(&root, &p.name)?;
            let current = gix::open(&root)
                .ok()
                .and_then(|repo| repo.head_name().ok().flatten().map(|n| n.shorten().to_string()));
            if current.as_deref() == Some(target.local.as_str()) {
                return Ok((project, root, target.local, false));
            }
            // Untracked files travel with any switch and git refuses one that would overwrite
            // them, so only tracked changes count as a dirty checkout.
            let tracked: Vec<_> = worktree::status_files_with(&root, worktree::Untracked::Directories)
                .map_err(git_mutation("git.status_failed"))?
                .into_iter()
                .filter(|file| !(file.index.is_empty() && file.worktree == "?"))
                .collect();
            if tracked.iter().any(|file| file.index == "U" || file.worktree == "U") {
                return Err(BusError::conflict(
                    "git.checkout_unmerged",
                    "This checkout is in the middle of a merge or rebase with unresolved conflicts; resolve them before switching branches",
                ));
            }
            if !tracked.is_empty() && !p.carry_changes.unwrap_or(false) {
                return Err(BusError::conflict(
                    "git.checkout_dirty",
                    format!(
                        "{} uncommitted change{} ({}). Commit them first, or bring them along to {}.",
                        tracked.len(),
                        if tracked.len() == 1 { "" } else { "s" },
                        sample_paths(tracked.iter().map(|file| file.path.as_str())),
                        target.local
                    ),
                ));
            }
            let args: Vec<&str> = match &target.remote {
                Some(remote) => vec!["switch", "--no-overwrite-ignore", "--track", "-c", &target.local, "--end-of-options", remote],
                None => vec!["switch", "--no-overwrite-ignore", "--", &target.local],
            };
            run_switch(&root, &args, &target.local)?;
            Ok((project, root, target.local, target.remote.is_some()))
        },
        |ctx: &mut Ctx, _p, (project, root, branch, created)| {
            changed(ctx, project.id, &root);
            Ok(BranchSwitchOut { branch, created })
        },
    );
    // Deciding "merged" walks the base branch's history, and listing worktrees opens each:
    // with the store unlocked, as `git.branches` already does (D149).
    e.register_staged::<BranchDelete, _>(|ctx, p| {
        let (project, owners) = ctx.read(|conn| {
            let project = get_project(conn, p.project_id)?;
            let owners = branch_owners(conn, project.id)?;
            Ok((project, owners))
        })?;
        let root = Path::new(&project.path);
        let name = validate_branch_name(root, &p.name)?;
        if name == project.base_branch {
            return Err(BusError::conflict(
                "git.branch_protected",
                format!("cannot delete the base branch {name}"),
            ));
        }
        let branches = branches_with(root, &project.base_branch, &owners, false)?;
        let branch = branches
            .branches
            .iter()
            .find(|branch| branch.name == name)
            .ok_or_else(|| {
                BusError::not_found("git.branch_not_found", format!("no local branch {name}"))
            })?;
        if let Some(session) = &branch.session {
            return Err(BusError::conflict(
                "git.branch_session_owned",
                format!("branch {name} belongs to session {session}"),
            ));
        }
        if !branch.merged {
            return Err(BusError::conflict(
                "git.branch_unmerged",
                format!("branch {name} is not merged into {}", project.base_branch),
            ));
        }
        if worktree::list_with_dirty(root, false)
            .map_err(|error| BusError::unavailable("worktree.list_failed", error.to_string()))?
            .iter()
            .any(|worktree| worktree.branch == name)
        {
            return Err(BusError::conflict(
                "git.branch_checked_out",
                format!("branch {name} is checked out in a worktree"),
            ));
        }
        worktree::git_mutate(root, &["branch", "-d", &name])
            .map_err(git_mutation("git.branch_delete_failed"))?;
        Ok(project)
    }, |ctx: &mut Ctx, _p, project| {
        changed(ctx, project.id, Path::new(&project.path));
        Ok(relay_bus::Empty {})
    });
    // `git add` runs clean filters and hashes every file it stages (a multi-GB asset, through
    // LFS); `git reset` rewrites the index. Neither needs the store (D149).
    e.register_staged::<Stage, _>(|ctx, p| {
        let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        if p.paths.is_empty() {
            return Err(BusError::invalid(
                "git.paths",
                "at least one path is required",
            ));
        }
        for path in &p.paths {
            validate_path(path)?;
        }
        // A path is a file name, never a pattern: without this, staging `a[1].txt` also staged
        // `a1.txt` (RA-153).
        let mut args = vec!["--literal-pathspecs", "add", "--"];
        args.extend(p.paths.iter().map(String::as_str));
        worktree::git_mutate(&root, &args).map_err(git_mutation("git.stage_failed"))?;
        Ok((project, root))
    }, |ctx: &mut Ctx, _p, (project, root)| {
        changed(ctx, project.id, &root);
        Ok(relay_bus::Empty {})
    });
    e.register_staged::<Unstage, _>(|ctx, p| {
        let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        if p.paths.is_empty() {
            return Err(BusError::invalid(
                "git.paths",
                "at least one path is required",
            ));
        }
        for path in &p.paths {
            validate_path(path)?;
        }
        let unborn = gix::open(&root)
            .map(|repo| repo.head_id().is_err())
            .unwrap_or(true);
        let mut args = if unborn {
            vec!["--literal-pathspecs", "rm", "--cached", "--"]
        } else {
            vec!["--literal-pathspecs", "reset", "HEAD", "--"]
        };
        args.extend(p.paths.iter().map(String::as_str));
        worktree::git_mutate(&root, &args).map_err(git_mutation("git.unstage_failed"))?;
        Ok((project, root))
    }, |ctx: &mut Ctx, _p, (project, root)| {
        changed(ctx, project.id, &root);
        Ok(relay_bus::Empty {})
    });
    // Staging, hook refresh and the user's own pre-commit hook are all subprocesses of unbounded
    // length — a slow `pre-commit` used to hold the one store connection for its whole run, so
    // every keystroke in every terminal waited on it. They happen before anything is locked; the
    // transaction only sees the gate and the commit itself (D144).
    e.register_staged::<CommitOp, _>(
        |ctx, p| {
            if p.message.trim().is_empty() {
                return Err(BusError::invalid(
                    "git.message",
                    "commit message cannot be empty",
                ));
            }
            let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
            // `add -A` would stage a conflicted file, markers and all, as resolved; without
            // `all`, write-tree refuses with an error that names nothing (RA-204).
            refuse_unmerged(&root)?;
            if p.all.unwrap_or(false) {
                worktree::git_mutate(&root, &["add", "-A"])
                    .map_err(git_mutation("git.stage_failed"))?;
            }
            crate::hooks::refresh_git(
                Path::new(&project.path),
                &root,
                ctx.instance(),
                &crate::hooks::relay_bin(),
            )
            .map_err(|error| BusError::unavailable("git.hook_refresh_failed", error.to_string()))?;
            crate::hooks::run_user_pre_commit(Path::new(&project.path), &root)
                .map_err(|error| BusError::conflict("git.pre_commit_failed", error.to_string()))?;
            // The commit is made without git's own hooks, so the user's prepare-commit-msg and
            // commit-msg hooks run here and may refuse or rewrite the message (RA-110), in git's
            // order — concluding a merge or the like too, since `git commit` no longer does.
            let message = crate::hooks::run_user_prepare_commit_msg(Path::new(&project.path), &root, &p.message)
                .map_err(|error| BusError::conflict("git.prepare_commit_msg_failed", error.to_string()))?;
            let message = crate::hooks::run_user_commit_msg(Path::new(&project.path), &root, &message)
                .map_err(|error| BusError::conflict("git.commit_msg_failed", error.to_string()))?;
            // The commit object itself — and a signing prompt, if commit.gpgSign asks for one —
            // is made here too, a merge's or cherry-pick's included. The transaction gates exactly
            // that object's tree and publishes it; the numstat is of that tree, not of an index
            // read a moment earlier (RA-154).
            let (staged, numstat) = stage_commit(&root, &message)?;
            Ok((project, root, numstat, staged))
        },
        |ctx: &mut Ctx, _p, (project, root, numstat, staged)| {
            super::guardrail::enforce(
                ctx,
                project.id,
                &root,
                GateKind::Commit,
                None,
                None,
                Some(&numstat),
                None,
            )?;
            // The object was made with the merge then pending as its parents; one started, ended
            // or moved on since would publish it wrong.
            let StagedCommit { sha, parent, reflog, git_dir, pending } = staged;
            if read_merge_head(&git_dir) != pending.merge {
                return Err(BusError::conflict(
                    "git.commit_raced",
                    "a merge started or ended while this commit was being made; nothing was committed",
                ));
            }
            // Compare-and-swap, in one ref transaction: the gate and its grants judged this object
            // against this parent and this pick or revert (its author, its sequencer), so it lands
            // only if none of them has moved since, and the pick it concludes is cleared with it.
            // Nothing here waits on a person: a signature, if any, was made before the store was
            // locked (D144).
            let zero = "0".repeat(sha.len());
            let mut refs = format!("update HEAD {sha} {}\n", parent.as_deref().unwrap_or(&zero));
            for (name, seen) in [("CHERRY_PICK_HEAD", &pending.pick), ("REVERT_HEAD", &pending.revert)] {
                match seen {
                    Some(oid) => refs.push_str(&format!("delete {name} {oid}\n")),
                    None => refs.push_str(&format!("verify {name} {zero}\n")),
                }
            }
            refs.push_str("delete AUTO_MERGE\n");
            git_with(&root, &["update-ref", "--stdin", "-m", &reflog], &[], Some(refs.as_bytes()), "git.commit_raced")
                .map_err(|error| BusError::conflict(
                    "git.commit_raced",
                    format!("the branch, or the cherry-pick or revert in progress, moved while this commit was being made; nothing was committed ({})", error.message),
                ))?;
            clear_concluded(&git_dir, &pending);
            let hook_root = root.clone();
            ctx.after_commit(move |_| run_post_commit(hook_root));
            changed(ctx, project.id, &root);
            Ok(CommitOut { sha })
        },
    );
    // `git fetch` is the network; it must not be holding the store while it waits (D144).
    e.register_staged::<Fetch, _>(
        |ctx, p| {
            let project = ctx.read(|conn| get_project(conn, p.project_id))?;
            fetch_remote(Path::new(&project.path))?;
            let repo = gix::open(&project.path).map_err(gix_err("git.open_failed"))?;
            let branch = repo
                .head_name()
                .ok()
                .flatten()
                .map(|n| n.shorten().to_string())
                .unwrap_or_default();
            let (_, ahead, behind) = upstream_metrics(&repo, &branch);
            Ok((project, ahead, behind))
        },
        |ctx: &mut Ctx, _p, (project, ahead, behind)| {
            changed(ctx, project.id, Path::new(&project.path));
            Ok(FetchOut { ahead, behind })
        },
    );
    // Same for `git push`: the whole op is network, and none of it needs the store.
    e.register_staged::<Push, _>(
        |ctx, p| {
            let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
            let repo = gix::open(&root).map_err(gix_err("git.open_failed"))?;
            let branch = repo
                .head_name()
                .ok()
                .flatten()
                .map(|n| n.shorten().to_string())
                .ok_or_else(|| BusError::conflict("git.detached", "cannot push a detached HEAD"))?;
            // Anyone who can run git in the checkout names its branch, `--mirror` or `+main`
            // included, and `git push origin <that>` would read it as an option or a forced
            // push: refused, and pushed only ever as an explicit refspec after the options.
            if branch.starts_with(['-', '+']) {
                return Err(BusError::invalid(
                    "git.branch_name",
                    format!("refusing to push branch {branch:?}: git push would read a name starting with '-' or '+' as an option or a forced push. Rename the branch first."),
                ));
            }
            let branch = validate_branch_name(&root, &branch)?;
            let has_upstream = upstream_metrics(&repo, &branch).0.is_some();
            let set_upstream = p.set_upstream.unwrap_or(!has_upstream);
            let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
            let args = if set_upstream {
                vec!["push", "-u", "origin", "--end-of-options", refspec.as_str()]
            } else {
                vec!["push"]
            };
            run_push(&root, &args)?;
            forget_prs(Path::new(&project.path));
            Ok((project, root))
        },
        |ctx: &mut Ctx, _p, (project, root)| {
            changed(ctx, project.id, &root);
            Ok(relay_bus::Empty {})
        },
    );
    e.register_unlocked::<PrList>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        let repo = Path::new(&project.path);
        if !p.refresh.unwrap_or(false) {
            if let Some(listed) = cached_prs(repo) {
                return listed;
            }
        }
        let gh = crate::github::gh_path()?;
        let listed = list_pull_requests(&gh, repo);
        remember_prs(repo, &listed);
        let listed = listed?;
        // A PR merged for a branch a closed session left behind: clean that branch up now
        // rather than at the next sweep (branch_cleanup).
        let merged: Vec<String> = listed.pull_requests.iter()
            .filter(|pr| pr.state == "merged" && pr.same_repository).map(|pr| pr.branch.clone()).collect();
        if !merged.is_empty() {
            let leftover: Vec<String> = ctx.read(|conn| {
                let (candidates, _) = crate::branch_cleanup::candidates(conn, Some(project.id), Some(&merged))?;
                Ok(candidates.into_iter().map(|candidate| candidate.branch).collect())
            })?;
            // Closed rows keep their branch name forever; only a branch that still exists is
            // anything to clean up, or every listing would start another cleanup for it.
            let local = gix::open(repo).ok();
            let leftover: Vec<String> = leftover.into_iter()
                .filter(|branch| local.as_ref().is_some_and(|repo| repo.find_reference(format!("refs/heads/{branch}").as_str()).is_ok()))
                .collect();
            if !leftover.is_empty() && ctx.engine().instance != crate::Instance::Test {
                let project_id = project.id;
                ctx.after_commit(move |engine| crate::branch_cleanup::after_merged_prs(engine, project_id, leftover));
            }
        }
        Ok(listed)
    });
    // Every git and gh call is a subprocess; the transaction only attributes the result (D149).
    e.register_staged::<BranchCleanup, _>(
        |ctx, p| branch_cleanup(ctx, p.project_id, p.dry_run),
        |ctx: &mut Ctx, _p, (project, rows)| {
            Ok(BranchCleanupOut { branches: branch_cleaned(ctx, &project, rows) })
        },
    );
    // `gh pr create` is GitHub round trips: never under the store lock, never unbounded.
    e.register_staged::<PrOpen, _>(
        |ctx, p| {
            let (project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
            let gh = crate::github::gh_path()?;
            let mut cmd = std::process::Command::new(gh);
            cmd.current_dir(&root).env("GH_PROMPT_DISABLED", "1").args(["pr", "create"]);
            if let Some(title) = &p.title {
                cmd.args(["--title", title]);
            }
            if let Some(body) = &p.body {
                cmd.args(["--body", body]);
            } else {
                cmd.arg("--fill");
            }
            let out = crate::proc::output_with_timeout(&mut cmd, PR_OPEN_TIMEOUT)
                .map_err(|e| BusError::unavailable("git.gh_unavailable", e.to_string()))?
                .ok_or_else(|| BusError::unavailable(
                    "git.pr_timeout",
                    format!("gh did not finish within {}s; the pull request may or may not have been opened", PR_OPEN_TIMEOUT.as_secs()),
                ).with_hint("check GitHub, or retry: gh refuses a second pull request for the same branch"))?;
            forget_prs(Path::new(&project.path));
            if !out.status.success() {
                return Err(BusError::conflict(
                    "git.pr_failed",
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                ));
            }
            Ok((project, root, String::from_utf8_lossy(&out.stdout).trim().to_string()))
        },
        |ctx: &mut Ctx, _p, (project, root, url)| {
            changed(ctx, project.id, &root);
            Ok(PrOpenOut { url })
        },
    );
    // The older name for git.branch.cleanup, kept for the clients that call it. It had rules of
    // its own that had drifted from cleanup's (any merged branch, by any name); now it is the
    // same cleanup, answering with the branches deleted, or on a dry run that would be (RA-740),
    // and the merged ones it found but could not delete, with why (RA-373).
    e.register_staged::<CleanMerged, _>(
        |ctx, p| branch_cleanup(ctx, p.project_id, p.dry_run),
        |ctx: &mut Ctx, _p, (project, rows)| {
            let (mut deleted, mut failed) = (Vec::new(), Vec::new());
            for row in branch_cleaned(ctx, &project, rows) {
                match row.outcome.as_str() {
                    "kept" if row.merged => failed.push(CleanMergedFailed { branch: row.branch, reason: row.reason }),
                    "kept" => {}
                    _ => deleted.push(row.branch),
                }
            }
            Ok(CleanMergedOut { deleted, failed })
        },
    );
    e.register_unlocked::<SuggestMessage>(|ctx, p| {
        let (_project, root) = resolve_root_unlocked(ctx, p.project_id, p.worktree.as_deref())?;
        let files = status_files(&root)?;
        let message = match files.as_slice() {
            [] => "No changes".into(),
            [one] => format!("Update {}", one.path),
            many => format!("Update {} files", many.len()),
        };
        Ok(SuggestOut { message })
    });
}

/// The most entries `git.status` returns. A client draws a row for each, and the whole reply
/// is one line on the wire (RA-205).
const STATUS_MAX_FILES: usize = 5000;
/// The most path bytes `git.status` returns, well inside a client's 2 MiB line.
const STATUS_MAX_BYTES: usize = 1024 * 1024;

/// At most [`STATUS_MAX_FILES`] entries and [`STATUS_MAX_BYTES`] of paths, in path order. When
/// some must go, conflicts are kept first and untracked files last: those are the changes a
/// commit would otherwise trip over or make unseen.
fn cap_status(mut files: Vec<FileStatus>) -> Vec<FileStatus> {
    let size = |file: &FileStatus| file.path.len() + file.renamed_from.as_ref().map_or(0, String::len) + 64;
    if files.len() <= STATUS_MAX_FILES && files.iter().map(size).sum::<usize>() <= STATUS_MAX_BYTES {
        return files;
    }
    let rank = |file: &FileStatus| {
        if file.index == "U" || file.worktree == "U" || matches!((file.index.as_str(), file.worktree.as_str()), ("A", "A") | ("D", "D")) {
            0
        } else if file.worktree == "?" {
            2
        } else {
            1
        }
    };
    // Stable: path order within each rank.
    files.sort_by_key(rank);
    let mut bytes = 0;
    let mut kept = 0;
    for file in &files {
        bytes += size(file);
        if kept == STATUS_MAX_FILES || bytes > STATUS_MAX_BYTES {
            break;
        }
        kept += 1;
    }
    files.truncate(kept);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

pub fn status_badges(root: &Path) -> Result<HashMap<String, String>, BusError> {
    Ok(status_files(root)?
        .into_iter()
        .map(|s| {
            let badge = if !s.worktree.is_empty() {
                s.worktree.clone()
            } else {
                s.index.clone()
            };
            (s.path, badge)
        })
        .collect())
}

/// What `git.commit`'s unlocked phase prepared: a commit object for the staged tree, not yet on
/// any branch, and what publishing it needs.
struct StagedCommit {
    sha: String,
    /// HEAD when the object was made: the compare-and-swap's expected value.
    parent: Option<String>,
    /// The reflog line `git commit` would write, `commit (merge): …` and the like.
    reflog: String,
    /// This checkout's own git dir, where a pending merge or pick keeps its state.
    git_dir: PathBuf,
    /// The merge, cherry-pick or revert the object concludes, as it stood then.
    pending: Pending,
}

/// What says a merge, cherry-pick or revert is waiting for its commit: it decides the commit's
/// parents and author, so the transaction checks it is unchanged before publishing.
struct Pending {
    /// `MERGE_HEAD`'s lines. Always a file, as git keeps it, reftable or not.
    merge: Option<String>,
    /// `CHERRY_PICK_HEAD` and `REVERT_HEAD`: refs, which a reftable repository keeps in its
    /// tables rather than as files, so they are read and cleared through git.
    pick: Option<String>,
    revert: Option<String>,
}

impl Pending {
    fn read(root: &Path, git_dir: &Path) -> Pending {
        Pending { merge: read_merge_head(git_dir), pick: ref_oid(root, "CHERRY_PICK_HEAD"), revert: ref_oid(root, "REVERT_HEAD") }
    }
}

fn read_merge_head(git_dir: &Path) -> Option<String> {
    std::fs::read_to_string(git_dir.join("MERGE_HEAD")).ok()
}

/// What `name` (a ref such as `CHERRY_PICK_HEAD`) points at, if it exists.
fn ref_oid(root: &Path, name: &str) -> Option<String> {
    worktree::git_mutate(root, &["rev-parse", "-q", "--verify", name]).ok().map(|oid| oid.trim().to_string())
}

/// Refuse while the index still has unmerged entries: a conflict not yet marked resolved.
fn refuse_unmerged(root: &Path) -> Result<(), BusError> {
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    let index = repo.index_or_empty().map_err(gix_err("git.index"))?;
    let mut paths: Vec<String> = Vec::new();
    for entry in index.entries().iter().filter(|entry| entry.stage_raw() != 0) {
        let path = entry.path(&index).to_string();
        // A conflict is up to three entries of one path, next to each other.
        if paths.last() != Some(&path) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Ok(());
    }
    Err(BusError::conflict(
        "git.unmerged",
        format!(
            "{} file{} still {} merge conflicts ({}). Resolve and stage {} first; nothing was committed.",
            paths.len(),
            if paths.len() == 1 { "" } else { "s" },
            if paths.len() == 1 { "has" } else { "have" },
            sample_paths(paths.iter().map(String::as_str)),
            if paths.len() == 1 { "it" } else { "them" },
        ),
    ))
}

/// Build the commit `git commit -m message` would make, without moving any ref, and the
/// numstat of exactly its tree for the commit gate. Concluding a merge, cherry-pick or revert,
/// it is the commit `git commit` would make of that too — the merged heads as parents, a picked
/// commit's author kept — so that publishing it is the transaction's one ref update, and a
/// signing prompt never waits with the store locked.
fn stage_commit(root: &Path, message: &str) -> Result<(StagedCommit, String), BusError> {
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    // Per-worktree state: a linked checkout's own git dir, not the shared one.
    let git_dir = repo.path().to_path_buf();
    let pending = Pending::read(root, &git_dir);
    let message = cleanup_message(root, message)?;
    if message.is_empty() {
        return Err(BusError::invalid("git.message", "commit message cannot be empty"));
    }
    let tree = worktree::git_mutate(root, &["write-tree"]).map_err(git_mutation("git.commit_failed"))?.trim().to_string();
    // Through git, not gix: a reftable repository's HEAD is in its tables.
    let parent = ref_oid(root, "HEAD");
    let mut parents: Vec<String> = parent.iter().cloned().collect();
    let mut author = None;
    // `git commit`'s own choices, in its order: an unborn branch takes no parents at all, a merge
    // takes the heads it merges and may be as empty as it likes, anything else must change
    // something, and a cherry-pick keeps the author of the commit it picked.
    let kind = match (&parent, &pending.merge) {
        (None, _) => "commit (initial)",
        (Some(_), Some(merged)) => {
            // `git merge v1` of an annotated tag writes the tag's own id; `git commit` peels it
            // to the commit, and so must this, or commit-tree refuses a tag as a parent and
            // `--independent` drops it for its commit, losing the merge.
            for head in merged.split_whitespace() {
                let peeled = worktree::git_mutate(root, &["rev-parse", "--verify", "--end-of-options", &format!("{head}^{{commit}}")])
                    .map_err(git_mutation("git.commit_failed"))?;
                parents.push(peeled.trim().to_string());
            }
            let mode = std::fs::read_to_string(git_dir.join("MERGE_MODE")).unwrap_or_default();
            if mode.trim() != "no-ff" && parents.len() > 1 {
                parents = independent(root, parents)?;
            }
            "commit (merge)"
        }
        (Some(parent), None) => {
            let parent_tree = worktree::git_mutate(root, &["rev-parse", &format!("{parent}^{{tree}}")])
                .map_err(git_mutation("git.commit_failed"))?;
            if parent_tree.trim() == tree {
                return Err(BusError::conflict("git.commit_failed", "nothing to commit: no staged changes"));
            }
            match &pending.pick {
                Some(picked) => {
                    author = Some(author_of(root, picked)?);
                    if git_dir.join("rebase-merge").is_dir() && ref_oid(root, "REBASE_HEAD").as_deref() == Some(picked.as_str()) {
                        "commit (rebase)"
                    } else {
                        "commit (cherry-pick)"
                    }
                }
                None => "commit",
            }
        }
    };
    let numstat = tree_numstat(root, &repo, parent.as_deref(), &tree)?;
    // `commit-tree` ignores commit.gpgSign, so honour it here the way `git commit` would.
    let sign = worktree::git_mutate(root, &["config", "--bool", "--get", "commit.gpgsign"])
        .is_ok_and(|value| value.trim() == "true");
    let mut args = vec!["commit-tree", tree.as_str()];
    for parent in &parents {
        args.extend(["-p", parent.as_str()]);
    }
    if sign {
        args.push("-S");
    }
    args.extend(["-m", message.as_str()]);
    let sha = commit_tree(root, &args, author)?;
    let reflog = format!("{kind}: {}", message.lines().next().unwrap_or_default());
    Ok((StagedCommit { sha, parent, reflog, git_dir, pending }, numstat))
}

/// `git commit`'s `reduce_heads` for a merge that may fast-forward: drop every parent another
/// one already contains, keeping the rest in order.
fn independent(root: &Path, parents: Vec<String>) -> Result<Vec<String>, BusError> {
    let mut args = vec!["merge-base", "--independent"];
    args.extend(parents.iter().map(String::as_str));
    let keep = worktree::git_mutate(root, &args).map_err(git_mutation("git.commit_failed"))?;
    let keep: Vec<&str> = keep.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    for parent in parents {
        if keep.contains(&parent.as_str()) && !out.contains(&parent) {
            out.push(parent);
        }
    }
    Ok(out)
}

/// The author of commit `oid` as `GIT_AUTHOR_NAME`, `_EMAIL` and `_DATE` take it, read from the
/// commit's own header: nothing a log format or log.showSignature adds can get into it.
fn author_of(root: &Path, oid: &str) -> Result<[String; 3], BusError> {
    let raw = worktree::git_mutate(root, &["cat-file", "commit", oid]).map_err(git_mutation("git.commit_failed"))?;
    let author = raw.lines().take_while(|line| !line.is_empty()).find_map(|line| line.strip_prefix("author "));
    // `Name <email> 1600000000 +0200`
    author
        .and_then(|line| {
            let (ident, date) = line.rsplit_once('>')?;
            let (name, email) = ident.rsplit_once('<')?;
            Some([name.trim_end().to_string(), email.to_string(), date.trim().to_string()])
        })
        .ok_or_else(|| BusError::internal(format!("reading the author of {oid}")))
}

/// `git commit-tree args`, as `author` when one is given.
fn commit_tree(root: &Path, args: &[&str], author: Option<[String; 3]>) -> Result<String, BusError> {
    let sha = match &author {
        Some([name, email, date]) => {
            let env = [("GIT_AUTHOR_NAME", name.as_str()), ("GIT_AUTHOR_EMAIL", email.as_str()), ("GIT_AUTHOR_DATE", date.as_str())];
            git_with(root, args, &env, None, "git.commit_failed")?
        }
        None => worktree::git_mutate(root, args).map_err(git_mutation("git.commit_failed"))?,
    };
    Ok(sha.trim().to_string())
}

/// Git with what `worktree::git_mutate` cannot pass — `env`, and `input` on stdin — and the same
/// bound as every other git call here: a signing prompt may be behind it. Its stdout, or a
/// conflict under `code` naming what failed.
fn git_with(root: &Path, args: &[&str], env: &[(&str, &str)], input: Option<&[u8]>, code: &'static str) -> Result<String, BusError> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(root).args(args).envs(env.iter().copied());
    let out = match input {
        Some(input) => crate::proc::output_with_input(&mut command, input, TIMEOUT),
        None => crate::proc::output_with_timeout(&mut command, TIMEOUT),
    }
    .map_err(|error| BusError::conflict(code, format!("running git {}: {error}", args.join(" "))))?
    .ok_or_else(|| BusError::conflict(code, format!("git {} did not finish within {} s", args.join(" "), TIMEOUT.as_secs())))?;
    if !out.status.success() {
        return Err(BusError::conflict(
            code,
            format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The files `git commit` clears once its commit lands, a few unlinks in the transaction: a
/// concluded merge's and a squash's always, and — once the last pick of a sequence is concluded
/// — the sequencer. These are files whatever the ref storage; CHERRY_PICK_HEAD, REVERT_HEAD and
/// AUTO_MERGE are refs, cleared in the commit's own ref transaction.
fn clear_concluded(git_dir: &Path, pending: &Pending) {
    for name in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "SQUASH_MSG"] {
        let _ = std::fs::remove_file(git_dir.join(name));
    }
    if pending.pick.is_none() && pending.revert.is_none() {
        return;
    }
    // git's `have_finished_the_last_pick`: the todo's one remaining line was this commit's.
    let sequencer = git_dir.join("sequencer");
    let Ok(todo) = std::fs::read_to_string(sequencer.join("todo")) else { return };
    if todo.find('\n').is_none_or(|eol| eol + 1 == todo.len()) {
        let _ = std::fs::remove_dir_all(&sequencer);
    }
}

/// The commit gate's numstat for `tree` against `parent` (or nothing), as git's own
/// `diff-tree` counts it — submodule bumps, binaries and big files included (RA-155, RA-157) —
/// in the `added\tremoved\tpath` lines the gate reads. A rename is its destination's real
/// change plus a `0\t0` line for its source, so a protected file cannot be moved out
/// unchallenged and a move does not count as a whole new file (RA-158).
fn tree_numstat(root: &Path, repo: &gix::Repository, parent: Option<&str>, tree: &str) -> Result<String, BusError> {
    let empty = repo.empty_tree().id.to_string();
    let from = parent.unwrap_or(&empty);
    let raw = worktree::git_mutate(
        root,
        &["diff-tree", "-r", "-z", "-M", "--numstat", "--no-ext-diff", "--no-textconv", from, tree],
    )
    .map_err(git_mutation("git.commit_failed"))?;
    Ok(numstat_lines(&raw))
}

/// `diff-tree -z --numstat` records — `a\tr\tpath\0`, or `a\tr\t\0source\0dest\0` for a
/// rename — as the gate's lines.
fn numstat_lines(raw: &str) -> String {
    let mut out = String::new();
    let mut fields = raw.split('\0');
    while let Some(record) = fields.next() {
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if path.is_empty() {
            let (Some(source), Some(dest)) = (fields.next(), fields.next()) else { break };
            out.push_str(&format!("0\t0\t{source}\n{added}\t{removed}\t{dest}\n"));
        } else {
            out.push_str(&format!("{added}\t{removed}\t{path}\n"));
        }
    }
    out
}

/// The message as `git commit -m` leaves it under commit.cleanup, which with no editor open is
/// `whitespace` for the default and for `scissors`; `strip` drops comment lines as well, as
/// `git stripspace` does with core.commentChar, and `verbatim` changes nothing.
fn cleanup_message(root: &Path, message: &str) -> Result<String, BusError> {
    let mode = worktree::git_mutate(root, &["config", "--get", "commit.cleanup"]).unwrap_or_default();
    match mode.trim() {
        "" | "default" | "whitespace" | "scissors" => Ok(clean_message(message)),
        "verbatim" => Ok(message.to_string()),
        "strip" => {
            let stripped = git_with(root, &["stripspace", "--strip-comments"], &[], Some(message.as_bytes()), "git.commit_failed")?;
            Ok(stripped.trim_end_matches('\n').to_string())
        }
        other => Err(BusError::conflict("git.commit_failed", format!("invalid commit.cleanup mode {other}"))),
    }
}

/// `git commit -m`'s default cleanup: trailing whitespace off every line, runs of blank lines
/// collapsed to one, and none at either end.
fn clean_message(message: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in message.lines().map(str::trim_end) {
        if line.is_empty() && lines.last().is_none_or(|last| last.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// `git commit` runs post-commit itself; a commit published with `update-ref` runs it here,
/// after the store is unlocked and on its own thread, with the rest of what `git commit` does
/// after every commit, in its order: rerere records conflict resolutions (a no-op unless it is
/// enabled), and a stash a `merge --autostash` set aside comes back last.
fn run_post_commit(root: PathBuf) {
    let _ = std::thread::Builder::new().name("post-commit".into()).spawn(move || {
        let run = |args: &[&str]| {
            let mut command = std::process::Command::new("git");
            command.arg("-C").arg(&root).args(args);
            match crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(120)) {
                Ok(Some(out)) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
                Ok(Some(out)) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
                Ok(None) => Err("did not finish within 120 s".to_string()),
                Err(error) => Err(error.to_string()),
            }
        };
        if let Err(error) = run(&["rerere"]) {
            tracing::warn!(root = %root.display(), %error, "recording conflict resolutions after a commit");
        }
        if let Err(error) = run(&["hook", "run", "--ignore-missing", "post-commit"]) {
            tracing::warn!(root = %root.display(), %error, "running the post-commit hook");
        }
        // git's `apply_autostash_ref`: apply it, or keep it in the stash list if it conflicts.
        let Ok(stash) = run(&["rev-parse", "-q", "--verify", "MERGE_AUTOSTASH"]) else { return };
        if run(&["stash", "apply", &stash]).is_err() {
            if let Err(error) = run(&["stash", "store", "-m", "autostash", &stash]) {
                tracing::warn!(root = %root.display(), %error, "keeping a merge's autostash");
                return;
            }
            tracing::warn!(root = %root.display(), "a merge's autostash conflicted; it is kept in the stash list");
        }
        let _ = run(&["update-ref", "-d", "MERGE_AUTOSTASH"]);
    });
}

/// The unlocked half of git.branch.cleanup (and its alias clean_merged): every git and gh call.
fn branch_cleanup(
    ctx: &Unlocked,
    project_id: relay_bus::types::Id,
    dry_run: Option<bool>,
) -> Result<(relay_bus::types::Project, Vec<BranchCleanupRow>), BusError> {
    let project = ctx.read(|conn| get_project(conn, project_id))?;
    let options = crate::branch_cleanup::Options {
        dry_run: dry_run.unwrap_or(false),
        gh: crate::branch_cleanup::gh(),
        audit_kept: false,
        use_gh_cache: false,
    };
    let rows = crate::branch_cleanup::run(ctx.engine(), Some(project.id), None, &options)?;
    Ok((project, rows))
}

/// The transaction half: announce what [`branch_cleanup`] changed.
fn branch_cleaned(ctx: &mut Ctx, project: &relay_bus::types::Project, rows: Vec<BranchCleanupRow>) -> Vec<BranchCleanupRow> {
    changed(ctx, project.id, Path::new(&project.path));
    if rows.iter().any(|row| row.removed_worktree) {
        ctx.emit("worktree.changed", json!({ "project_id": project.id }));
    }
    rows
}

fn status_files(root: &Path) -> Result<Vec<FileStatus>, BusError> {
    worktree::status_files(root).map_err(git_mutation("git.status_failed"))
}

/// The project and the checkout a git op means: one short read for the project row and the
/// session's worktree, then [`crate::worktree::contains`] confirms it is a checkout of this
/// repository. That reads `.git` pointer files, no subprocess; the git op that follows is what
/// keeps the store lock released (D144).
fn resolve_root_unlocked(
    ctx: &Unlocked,
    project_id: relay_bus::types::Id,
    requested: Option<&str>,
) -> Result<(relay_bus::types::Project, PathBuf), BusError> {
    let session_id = ctx.actor_session_id();
    let (project, chosen) = ctx
        .read(|conn| crate::handlers::file::root_choice(conn, session_id, project_id, requested))?;
    let chosen = crate::handlers::file::root_verify(&project, chosen, "git.worktree")?;
    Ok((project, chosen))
}

fn validate_path(path: &str) -> Result<(), BusError> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        Err(BusError::invalid(
            "git.path",
            "path must be worktree-relative and contain no ..",
        ))
    } else {
        Ok(())
    }
}

/// The largest combined old + new text git.diff.file returns: the desktop editor's limit,
/// and well inside every client's 2 MiB line.
const DIFF_FILE_MAX: usize = 1024 * 1024;

fn diff_too_large() -> BusError {
    BusError::refused("git.diff_too_large", "This diff is larger than the 1 MiB editor limit.")
}

/// The most bytes of one side git.diff and git.show read to count its lines. Git itself calls
/// a file over `core.bigFileThreshold` binary; past this a whole-file line diff is not worth
/// its memory or its time (RA-157).
const DIFF_COUNT_MAX: u64 = 8 * 1024 * 1024;
/// How long one file's line diff may run before it settles for a coarser, still valid edit
/// script instead of the minimal one (RA-157).
const DIFF_DEADLINE: std::time::Duration = std::time::Duration::from_millis(200);

/// One side of a file's diff.
enum Side {
    Text(String),
    /// A NUL byte: git's own test for binary content.
    Binary,
    /// Over the reader's limit, and never read whole.
    TooLarge,
}

impl Side {
    fn empty() -> Self {
        Side::Text(String::new())
    }
}

fn side_from(bytes: Vec<u8>) -> Side {
    if bytes.contains(&0) {
        return Side::Binary;
    }
    Side::Text(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// What git diffs for a submodule: the commit it points at, as one line.
fn gitlink_side(id: impl std::fmt::Display) -> Side {
    Side::Text(format!("Subproject commit {id}\n"))
}

/// An object as git diffs it. A gitlink names a commit in another repository, not an object
/// in this one (RA-155); a blob's size comes from its header, before it is inflated.
fn object_side(
    repo: &gix::Repository,
    id: gix::ObjectId,
    mode: gix::object::tree::EntryMode,
    max: u64,
) -> Result<Side, BusError> {
    if mode.is_commit() {
        return Ok(gitlink_side(id));
    }
    if mode.is_tree() {
        return Ok(Side::empty());
    }
    if repo.find_header(id).map_err(gix_err("git.object"))?.size() > max {
        return Ok(Side::TooLarge);
    }
    let object = repo.find_object(id).map_err(gix_err("git.object"))?;
    Ok(match object.try_into_blob() {
        Ok(mut blob) => side_from(std::mem::take(&mut blob.data)),
        Err(_) => Side::empty(),
    })
}

/// The tree of `revision`; none for the unborn HEAD of a repository with no commits yet.
fn revision_tree<'repo>(repo: &'repo gix::Repository, revision: &str) -> Result<Option<gix::Tree<'repo>>, BusError> {
    let id = match repo.rev_parse_single(revision) {
        Ok(id) => id,
        Err(_) if revision == "HEAD" => return Ok(None),
        Err(error) => return Err(revision_err(revision)(error)),
    };
    let tree = id
        .object()
        .map_err(gix_err("git.object"))?
        .peel_to_commit()
        .map_err(gix_err("git.object"))?
        .tree()
        .map_err(gix_err("git.object"))?;
    Ok(Some(tree))
}

fn tree_side(repo: &gix::Repository, tree: Option<&gix::Tree<'_>>, path: &str, max: u64) -> Result<Side, BusError> {
    let Some(tree) = tree else {
        return Ok(Side::empty());
    };
    match tree.lookup_entry_by_path(path).map_err(gix_err("git.object"))? {
        Some(entry) => object_side(repo, entry.object_id(), entry.mode(), max),
        None => Ok(Side::empty()),
    }
}

fn index_side(repo: &gix::Repository, index: &gix::index::State, path: &str, max: u64) -> Result<Side, BusError> {
    use gix::bstr::ByteSlice;
    let Some(entry) = index.entry_by_path(path.as_bytes().as_bstr()) else {
        return Ok(Side::empty());
    };
    match entry.mode.to_tree_entry_mode() {
        Some(mode) => object_side(repo, entry.id, mode, max),
        None => Ok(Side::empty()),
    }
}

/// The file at `path` as git sees it in the checkout. A symlink is the name of its target,
/// never what it points at, and a path under a symlinked directory is not in the checkout at
/// all; a FIFO, socket or device is never opened, and no file is read past `max` (RA-156).
fn work_side(root: &Path, path: &str, max: u64) -> Result<Side, BusError> {
    use std::io::Read;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::OpenOptionsExt;
    validate_path(path)?;
    let relative = Path::new(path);
    let read_failed = |e: std::io::Error| BusError::unavailable("git.read_failed", e.to_string());
    let through_link = relative
        .ancestors()
        .skip(1)
        .filter(|dir| !dir.as_os_str().is_empty())
        .any(|dir| std::fs::symlink_metadata(root.join(dir)).is_ok_and(|md| md.file_type().is_symlink()));
    if through_link {
        return Ok(Side::empty());
    }
    let full = root.join(relative);
    let md = match std::fs::symlink_metadata(&full) {
        Ok(md) => md,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => {
            return Ok(Side::empty())
        }
        Err(e) => return Err(read_failed(e)),
    };
    let kind = md.file_type();
    if kind.is_symlink() {
        return Ok(side_from(std::fs::read_link(&full).map_err(read_failed)?.into_os_string().into_vec()));
    }
    if kind.is_dir() {
        // A submodule's checkout, or a nested repository: its HEAD, as git diffs a gitlink.
        return Ok(gix::open(&full)
            .ok()
            .and_then(|repo| repo.head_id().ok().map(gitlink_side))
            .unwrap_or_else(Side::empty));
    }
    if !kind.is_file() {
        return Ok(Side::empty());
    }
    if md.len() > max {
        return Ok(Side::TooLarge);
    }
    // O_NOFOLLOW and O_NONBLOCK: a file swapped for a link or a FIFO since the check above
    // cannot redirect or hang the read.
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&full)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Side::empty()),
        Err(e) => return Err(read_failed(e)),
    };
    if !file.metadata().is_ok_and(|md| md.is_file()) {
        return Ok(Side::empty());
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes).map_err(read_failed)?;
    if bytes.len() as u64 > max {
        return Ok(Side::TooLarge);
    }
    Ok(side_from(bytes))
}

/// Lines added and removed; none when either side is not text to count.
fn counts(old: &Side, new: &Side) -> Option<(i64, i64)> {
    let (Side::Text(old), Side::Text(new)) = (old, new) else {
        return None;
    };
    let (mut a, mut r) = (0, 0);
    let diff = TextDiff::configure()
        .deadline(std::time::Instant::now() + DIFF_DEADLINE)
        .diff_lines(old, new);
    for c in diff.iter_all_changes() {
        match c.tag() {
            ChangeTag::Insert => a += 1,
            ChangeTag::Delete => r += 1,
            _ => {}
        }
    }
    Some((a, r))
}

/// A changed file's row: binary (or too large to count, as git's big-file rule) has no counts.
fn diff_file(path: String, old_path: Option<String>, status: String, old: &Side, new: &Side) -> DiffFile {
    let counted = counts(old, new);
    let (added, removed) = counted.unwrap_or((0, 0));
    DiffFile { path, old_path, status, added, removed, binary: counted.is_none() }
}

fn commit_from_gix(c: gix::Commit<'_>) -> Result<Commit, BusError> {
    let author = c.author().map_err(gix_err("git.commit_decode"))?;
    let message = c.message().map_err(gix_err("git.commit_decode"))?;
    let title = message.title.to_string().trim_end().to_string();
    let body = message
        .body
        .map(|b| b.to_string().trim_end().to_string())
        .unwrap_or_default();
    // git accepts dates jiff cannot represent (past year 9999); one such commit in an imported
    // history must not fail the whole page, so its date is left unknown (RA-374).
    let at = jiff::Timestamp::from_second(author.seconds())
        .map(|at| at.to_string())
        .unwrap_or_default();
    Ok(Commit {
        sha: c.id.to_string(),
        parents: c.parent_ids().map(|p| p.to_string()).collect(),
        author: author.name.to_string(),
        email: author.email.to_string(),
        at,
        subject: title,
        body,
        refs: Vec::new(),
    })
}
fn upstream_metrics(
    repo: &gix::Repository,
    branch: &str,
) -> (Option<String>, Option<i64>, Option<i64>) {
    // The full name: a short one is probed as `refs/<name>` and `refs/tags/<name>` first, so a
    // tag `v2` or `refs/stash` would shadow the branch of the same name (RA-375).
    let full = format!("refs/heads/{}", branch.trim_start_matches("refs/heads/"));
    let Ok(reference) = repo.find_reference(full.as_str()) else {
        return (None, None, None);
    };
    let Some(Ok(name)) = reference.remote_tracking_ref_name(gix::remote::Direction::Fetch) else {
        return (None, None, None);
    };
    let display = name
        .as_bstr()
        .to_string()
        .trim_start_matches("refs/remotes/")
        .to_string();
    let Ok(upstream) = repo.find_reference(name.as_bstr()) else {
        return (Some(display), None, None);
    };
    let (Some(local), Some(remote)) = (reference.try_id(), upstream.try_id()) else {
        return (Some(display), None, None);
    };
    let counts = divergence(repo, local.detach(), remote.detach());
    (Some(display), counts.map(|v| v.0), counts.map(|v| v.1))
}

fn divergence(
    repo: &gix::Repository,
    local: gix::ObjectId,
    remote: gix::ObjectId,
) -> Option<(i64, i64)> {
    let count = |from, hidden| {
        repo.rev_walk([from])
            .with_hidden([hidden])
            .all()
            .ok()?
            .try_fold(0_i64, |count, item| item.ok().map(|_| count + 1))
    };
    Some((count(local, remote)?, count(remote, local)?))
}

/// One lock per repository (keyed by its common git dir, so every worktree of a clone shares
/// it), holding when its last successful fetch finished. Two `git fetch --all --prune` in one
/// repository race on the same remote-tracking ref locks and one of them fails (RA-376).
type FetchLock = std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>;
fn fetch_lock(root: &Path) -> FetchLock {
    static LOCKS: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, FetchLock>>> = std::sync::OnceLock::new();
    let key = gix::open(root)
        .map(|repo| repo.common_dir().to_path_buf())
        .unwrap_or_else(|_| root.to_path_buf());
    let key = std::fs::canonicalize(&key).unwrap_or(key);
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|p| p.into_inner());
    locks.entry(key).or_default().clone()
}

/// Bounded network preflight, called outside the store transaction. Fetches of one repository
/// run one at a time, and a caller that waited on another's fetch reuses it when it succeeded:
/// it finished after this caller asked, so it is at most one fetch's duration staler than its own.
pub fn fetch_remote(root: &Path) -> Result<(), BusError> {
    let asked = std::time::Instant::now();
    let lock = fetch_lock(root);
    let mut last = lock.lock().unwrap_or_else(|p| p.into_inner());
    if last.is_some_and(|done| done >= asked) {
        return Ok(());
    }
    fetch_remote_now(root)?;
    *last = Some(std::time::Instant::now());
    Ok(())
}

fn fetch_remote_now(root: &Path) -> Result<(), BusError> {
    let mut command = std::process::Command::new("git");
    command
        .current_dir(root)
        .args(["fetch", "--all", "--prune"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    let output = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(20))
        .map_err(|error| BusError::unavailable("git.fetch_failed", error.to_string()))?
        .ok_or_else(|| {
            BusError::unavailable(
                "git.fetch_timeout",
                "Fetch timed out; cached refs are still available",
            )
        })?;
    if !output.status.success() {
        return Err(BusError::unavailable(
            "git.fetch_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

pub(super) fn existing_worktree_branch(root: &Path, branch: Option<&str>) -> Result<bool, BusError> {
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    Ok(branch.is_some_and(|branch| repo.find_reference(format!("refs/heads/{branch}").as_str()).is_ok()))
}

/// Where a new worktree on `branch` starts, for worktree.create and session.create alike
/// (RA-640). An existing branch is reattached as it is, preserving its work, offline sessions
/// included. A new one starts at `from` when the caller names it, and otherwise at the freshest
/// `base` after a fetch: a failed fetch must never silently launch from stale refs.
pub(super) fn new_worktree_start(root: &Path, branch: &str, base: &str, from: Option<&str>) -> Result<worktree::Start, BusError> {
    if existing_worktree_branch(root, Some(branch))? {
        return Ok(worktree::Start::Existing);
    }
    if let Some(from) = from {
        return Ok(worktree::Start::New(Some(from.to_string())));
    }
    fetch_remote(root)?;
    Ok(worktree::Start::New(new_worktree_base(root, base)?))
}

/// Pin the freshest base commit without moving the primary checkout. Preserve local-only
/// commits; refuse divergent histories instead of silently omitting remote changes.
pub fn new_worktree_base(root: &Path, base: &str) -> Result<Option<String>, BusError> {
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    let local_name = format!("refs/heads/{}", base.trim_start_matches("refs/heads/"));
    let (local, remote) = base_tips(&repo, base)?;
    let (local, remote) = (local.map(|(_, id)| id), remote.map(|(_, id)| id));
    // A repository before its first commit has a valid unborn base, but no commit to pin.
    if local.is_none() && remote.is_none()
        && repo.head().is_ok_and(|head| head.is_unborn())
        && repo.head_name().ok().flatten().is_some_and(|name| *name.as_bstr() == local_name)
    {
        return Ok(None);
    }
    let selected = match (local, remote) {
        (Some(local), Some(remote)) if local != remote => {
            match repo.merge_base(local, remote).ok().map(|id| id.detach()) {
                Some(common) if common == local => remote,
                Some(common) if common == remote => local,
                _ => return Err(BusError::conflict("git.base_diverged", format!(
                    "Base branch {base} and its upstream have diverged; reconcile them before creating a new agent branch"
                ))),
            }
        }
        (Some(id), _) | (_, Some(id)) => id,
        _ => return Err(BusError::conflict("git.base_missing", format!("Base branch {base:?} does not exist"))),
    };
    Ok(Some(selected.to_string()))
}

/// A branch tip with the short name a person reads (`main`, `origin/main`).
type Tip = (String, gix::ObjectId);

/// The base branch's local tip and its remote-tracking tip (its configured upstream, else
/// `origin/<base>`).
fn base_tips(repo: &gix::Repository, base: &str) -> Result<(Option<Tip>, Option<Tip>), BusError> {
    let short = base.trim_start_matches("refs/heads/");
    let local = repo.find_reference(format!("refs/heads/{short}").as_str()).ok();
    let upstream = local.as_ref().and_then(|reference| {
        reference.remote_tracking_ref_name(gix::remote::Direction::Fetch)
    }).transpose().map_err(gix_err("git.base_invalid"))?;
    let remote_name = upstream.map(|name| name.as_bstr().to_string()).or_else(|| {
        let name = format!("refs/remotes/origin/{short}");
        repo.find_reference(name.as_str()).ok().map(|_| name)
    });
    let remote = remote_name.map(|name| {
        repo.rev_parse_single(name.as_str()).map(|id| (name.trim_start_matches("refs/remotes/").to_string(), id.detach()))
            .map_err(gix_err("git.base_upstream_missing"))
    }).transpose()?;
    let local = local.and_then(|reference| reference.try_id().map(|id| (short.to_string(), id.detach())));
    Ok((local, remote))
}

/// The newest commit a session could start from: whichever of the local base and its remote
/// tip contains the other, and the remote one (what GitHub has) when they have diverged.
fn freshest_base(repo: &gix::Repository, base: &str) -> Result<Tip, BusError> {
    match base_tips(repo, base)? {
        (Some(local), Some(remote)) if local.1 != remote.1 => {
            let common = repo.merge_base(local.1, remote.1).ok().map(|id| id.detach());
            Ok(if common == Some(remote.1) { local } else { remote })
        }
        (_, Some(tip)) | (Some(tip), None) => Ok(tip),
        _ => Err(BusError::conflict("git.base_missing", format!("Base branch {base:?} does not exist"))),
    }
}

/// How far each worktree of `root` is behind (and ahead of) the freshest `base`, from the refs
/// already on disk: `worktree.drift`.
pub fn drift(root: &Path, base: &str) -> Result<DriftOut, BusError> {
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    let (name, tip) = freshest_base(&repo, base)?;
    let worktrees = worktree::list_with_dirty(root, false)
        .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?
        .into_iter()
        .map(|w| {
            let counts = gix::ObjectId::from_hex(w.head.as_bytes()).ok()
                .and_then(|head| divergence(&repo, head, tip));
            DriftRow { path: w.path, branch: w.branch, ahead: counts.map(|c| c.0), behind: counts.map(|c| c.1) }
        })
        .collect();
    Ok(DriftOut { base: name, head: tip.to_string(), worktrees })
}

/// Cached Git context only: generating a brief must never make a network request.
pub fn briefing_state(root: &Path, branch: &str, base: &str) -> String {
    let Ok(repo) = gix::open(root) else {
        return "git: unavailable".into();
    };
    let (upstream, ahead, behind) = upstream_metrics(&repo, branch);
    let (base_upstream, base_ahead, base_behind) = upstream_metrics(&repo, base);
    let count = |value: Option<i64>| {
        value
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".into())
    };
    let base_counts = repo.rev_parse_single(branch).ok().and_then(|local| {
        repo.rev_parse_single(base)
            .ok()
            .and_then(|remote| divergence(&repo, local.detach(), remote.detach()))
    });
    let fetched = std::fs::metadata(repo.common_dir().join("FETCH_HEAD"))
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.elapsed().ok())
        .map(|age| {
            format!(
                "FETCH_HEAD updated {}s ago; cached snapshot, not proof of a successful fetch",
                age.as_secs()
            )
        })
        .unwrap_or_else(|| "unknown; offline cached refs only".into());
    format!(
        "git_upstream: {}\ngit_ahead: {}\ngit_behind: {}\ngit_base: {}\ngit_base_ahead: {}\ngit_base_behind: {}\ngit_base_upstream: {}\ngit_base_upstream_ahead: {}\ngit_base_upstream_behind: {}\ngit_last_fetch: {}",
        upstream.as_deref().unwrap_or("none"),
        count(ahead),
        count(behind),
        base,
        count(base_counts.map(|v| v.0)),
        count(base_counts.map(|v| v.1)),
        base_upstream.as_deref().unwrap_or("none"),
        count(base_ahead),
        count(base_behind),
        fetched
    )
}

#[derive(Deserialize)]
struct GhPullRequest {
    number: u64,
    head: GhBranch,
    base: GhBranch,
    #[serde(default)]
    draft: bool,
    html_url: String,
    title: String,
    state: String,
    merged_at: Option<String>,
}
#[derive(Deserialize)]
struct GhBranch {
    #[serde(rename = "ref")]
    branch: String,
    repo: Option<GhRepo>,
}
#[derive(Deserialize)]
struct GhRepo {
    full_name: String,
}

/// Every open PR (each page), and one page of the most recently updated closed and merged
/// ones: the two asked of GitHub at once, so the lookup costs one round trip. Every PR the
/// repository ever had, page by page, grew with its history and made a slow network slower.
fn list_pull_requests(gh: &Path, root: &Path) -> Result<PrListOut, BusError> {
    let recent = format!("repos/{{owner}}/{{repo}}/pulls?state=closed&per_page={PR_LIST_RECENT}&sort=updated&direction=desc");
    let (open, closed) = std::thread::scope(|scope| {
        let closed = scope.spawn(|| gh_api(gh, root, &[recent.as_str()]));
        let open = gh_api(gh, root, &["--paginate", "--slurp", "repos/{owner}/{repo}/pulls?state=open&per_page=100"]);
        (open, closed.join().unwrap_or_else(|_| Err(BusError::internal("gh lookup thread panicked"))))
    });
    let mut pull_requests = decode_pull_requests(&open?)?;
    // Without the closed page the open list still stands; `complete` says what is missing.
    let closed = closed.and_then(|bytes| decode_page(&bytes));
    let complete = closed.is_ok();
    pull_requests.extend(closed.unwrap_or_default());
    Ok(PrListOut { pull_requests, complete })
}

/// One `gh api` call's output, bounded by [`PR_LIST_TIMEOUT`].
fn gh_api(gh: &Path, root: &Path, args: &[&str]) -> Result<Vec<u8>, BusError> {
    let mut command = std::process::Command::new(gh);
    command.current_dir(root).env("GH_PROMPT_DISABLED", "1").arg("api").args(args);
    // `gh` talks to github.com. An unbounded wait on a flaky network used to be an unbounded
    // wait for every other bus op behind it; the lock is gone now, but the caller still gets
    // an answer either way (D144).
    let out = crate::proc::output_with_timeout(&mut command, PR_LIST_TIMEOUT)
        .map_err(|error| BusError::unavailable("git.pr_list_failed", error.to_string()))?
        .ok_or_else(|| {
            BusError::unavailable(
                "git.pr_list_timeout",
                format!("gh did not answer within {}s", PR_LIST_TIMEOUT.as_secs()),
            )
        })?;
    if !out.status.success() {
        return Err(BusError::unavailable(
            "git.pr_list_failed",
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(out.stdout)
}

/// One page of `gh api`'s pull requests, as GitHub returns it without `--slurp`.
fn decode_page(bytes: &[u8]) -> Result<Vec<PullRequest>, BusError> {
    let page: Vec<GhPullRequest> = serde_json::from_slice(bytes)
        .map_err(|error| BusError::internal(format!("decoding GitHub pull requests: {error}")))?;
    Ok(page.into_iter().map(pull_request).collect())
}

fn decode_pull_requests(bytes: &[u8]) -> Result<Vec<PullRequest>, BusError> {
    let pages: Vec<Vec<GhPullRequest>> = serde_json::from_slice(bytes)
        .map_err(|error| BusError::internal(format!("decoding GitHub pull requests: {error}")))?;
    Ok(pages.into_iter().flatten().map(pull_request).collect())
}

fn pull_request(row: GhPullRequest) -> PullRequest {
    let same_repository = row
        .head
        .repo
        .as_ref()
        .zip(row.base.repo.as_ref())
        .is_some_and(|(head, base)| head.full_name == base.full_name);
    PullRequest {
        number: row.number,
        branch: row.head.branch,
        draft: row.draft,
        url: row.html_url,
        title: row.title,
        state: if row.merged_at.is_some() {
            "merged".into()
        } else {
            row.state
        },
        same_repository,
    }
}

fn commit_diff_files(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
) -> Result<Vec<DiffFile>, BusError> {
    let new_tree = commit.tree().map_err(gix_err("git.show_failed"))?;
    let old_tree = match commit.parent_ids().next() {
        Some(parent) => parent
            .object()
            .map_err(gix_err("git.show_failed"))?
            .into_commit()
            .tree()
            .map_err(gix_err("git.show_failed"))?,
        None => repo.empty_tree(),
    };
    tree_diff_files(repo, &old_tree, &new_tree, "git.show_failed")
}

fn tree_diff_files(
    repo: &gix::Repository,
    old_tree: &gix::Tree<'_>,
    new_tree: &gix::Tree<'_>,
    code: &'static str,
) -> Result<Vec<DiffFile>, BusError> {
    // Bounded like git.diff's sides; an unreadable object is an empty side, as it always was.
    let side = |id: gix::Id<'_>, mode: gix::object::tree::EntryMode| {
        object_side(repo, id.detach(), mode, DIFF_COUNT_MAX).unwrap_or_else(|_| Side::empty())
    };
    let mut files = Vec::new();
    old_tree
        .changes()
        .map_err(gix_err(code))?
        .for_each_to_obtain_tree(new_tree, |change| {
            use gix::object::tree::diff::Change;
            // A directory is not a changed file; the entries under it are reported on their
            // own (RA-159).
            let (path, old_path, status, old, new) = match change {
                Change::Addition { entry_mode, .. } | Change::Deletion { entry_mode, .. } if entry_mode.is_tree() => {
                    return Ok(std::ops::ControlFlow::Continue(()));
                }
                Change::Modification { previous_entry_mode, entry_mode, .. }
                    if previous_entry_mode.is_tree() && entry_mode.is_tree() =>
                {
                    return Ok(std::ops::ControlFlow::Continue(()));
                }
                Change::Addition { location, entry_mode, id, .. } => {
                    (location.to_string(), None, "A", Side::empty(), side(id, entry_mode))
                }
                Change::Deletion { location, entry_mode, id, .. } => {
                    (location.to_string(), None, "D", side(id, entry_mode), Side::empty())
                }
                Change::Modification { location, previous_entry_mode, previous_id, entry_mode, id, .. } => (
                    location.to_string(),
                    None,
                    "M",
                    side(previous_id, previous_entry_mode),
                    side(id, entry_mode),
                ),
                Change::Rewrite {
                    source_location,
                    source_entry_mode,
                    source_id,
                    location,
                    entry_mode,
                    id,
                    copy,
                    ..
                } => (
                    location.to_string(),
                    Some(source_location.to_string()),
                    if copy { "C" } else { "R" },
                    side(source_id, source_entry_mode),
                    side(id, entry_mode),
                ),
            };
            if !path.is_empty() {
                files.push(diff_file(path, old_path, status.into(), &old, &new));
            }
            Ok::<_, std::io::Error>(std::ops::ControlFlow::Continue(()))
        })
        .map_err(gix_err(code))?;
    Ok(files)
}
fn changed(ctx: &mut Ctx, project_id: relay_bus::types::Id, root: &Path) {
    ctx.set_project(project_id);
    ctx.emit(
        "git.changed",
        json!({"project_id":project_id,"worktree":root}),
    );
}
fn validate_branch_name(root: &Path, value: &str) -> Result<String, BusError> {
    let name = value.trim();
    if name.is_empty() {
        return Err(BusError::invalid(
            "git.branch_name",
            "branch name cannot be empty",
        ));
    }
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(root).args(["check-ref-format", "--branch", name]);
    let out = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(10))
        .map_err(|e| BusError::unavailable("git.unavailable", e.to_string()))?
        .ok_or_else(|| BusError::unavailable("git.unavailable", "git check-ref-format did not finish within 10 s"))?;
    if !out.status.success() {
        return Err(BusError::invalid(
            "git.branch_name",
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(name.to_string())
}
fn git_mutation(code: &'static str) -> impl Fn(anyhow::Error) -> BusError {
    move |e| BusError::conflict(code, e.to_string())
}
/// The tips among `tips` that the base's ancestry reaches, from a single walk that ends as soon
/// as the last of them is found — or when history runs out, which is the cost `git branch
/// --merged` pays too.
fn merged_tips(
    repo: &gix::Repository,
    head: Option<gix::ObjectId>,
    tips: impl IntoIterator<Item = gix::ObjectId>,
) -> std::collections::HashSet<gix::ObjectId> {
    let mut pending: std::collections::HashSet<gix::ObjectId> = tips.into_iter().collect();
    let mut merged = std::collections::HashSet::new();
    let Some(head) = head else { return merged };
    let Ok(walk) = repo.rev_walk([head]).all() else { return merged };
    for info in walk {
        let Ok(info) = info else { break };
        if pending.remove(&info.id) {
            merged.insert(info.id);
        }
        if pending.is_empty() {
            break;
        }
    }
    merged
}

/// Refuse to change the branch of a checkout a live session works in.
fn refuse_session_checkout(ctx: &Unlocked, project_id: relay_bus::types::Id, root: &Path) -> Result<(), BusError> {
    let checkout = root.to_string_lossy().to_string();
    let owner = ctx.read(|conn| {
        let mut st = conn
            .prepare_cached("SELECT name FROM sessions WHERE project_id=?1 AND worktree=?2 AND state!='closed' LIMIT 1")
            .bus()?;
        let mut rows = st.query(rusqlite::params![project_id, checkout]).bus()?;
        rows.next().bus()?.map(|row| row.get::<_, String>(0)).transpose().bus()
    })?;
    match owner {
        Some(session) => Err(BusError::conflict(
            "git.checkout_session_owned",
            format!("This checkout belongs to the live session {session}; its agent decides its branch. Select another checkout, or end the session first."),
        )),
        None => Ok(()),
    }
}

/// What `git.branch.switch` checks out: a local branch, or a new local tracking branch for a
/// remote one.
struct SwitchTarget {
    local: String,
    remote: Option<String>,
}

/// A local branch of that name wins; otherwise `origin/feature` names a remote-tracking branch,
/// checked out as `feature` — or as the existing local `feature`, if there is one.
fn switch_target(root: &Path, requested: &str) -> Result<SwitchTarget, BusError> {
    let name = requested.trim();
    let repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    if repo.find_reference(format!("refs/heads/{name}").as_str()).is_ok() {
        return Ok(SwitchTarget { local: validate_branch_name(root, name)?, remote: None });
    }
    if repo.find_reference(format!("refs/remotes/{name}").as_str()).is_ok() {
        let branch = repo
            .remote_names()
            .iter()
            .map(|remote| remote.to_string())
            .filter_map(|remote| name.strip_prefix(&format!("{remote}/")).map(str::to_owned))
            .min_by_key(String::len)
            .or_else(|| name.split_once('/').map(|(_, rest)| rest.to_owned()))
            .filter(|branch| !branch.is_empty() && branch != "HEAD")
            .ok_or_else(|| BusError::invalid("git.branch_name", format!("{name} is not a remote branch")))?;
        let local = validate_branch_name(root, &branch)?;
        if repo.find_reference(format!("refs/heads/{local}").as_str()).is_ok() {
            return Ok(SwitchTarget { local, remote: None });
        }
        return Ok(SwitchTarget { local, remote: Some(name.to_owned()) });
    }
    validate_branch_name(root, name)?;
    Err(BusError::not_found("git.branch_not_found", format!("no local or remote branch {name}")))
}

/// `git push`, bounded, and never prompting for credentials on whatever terminal the engine was
/// started from: a push that needs them fails instead of waiting for a person who is not there
/// (RA-372). As generous as any other git mutation, since an LFS upload can be large.
fn run_push(root: &Path, args: &[&str]) -> Result<(), BusError> {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(root).args(args).env("GIT_TERMINAL_PROMPT", "0").stdin(std::process::Stdio::null());
    let output = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(600))
        .map_err(|error| BusError::conflict("git.push_failed", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("git.push_failed", "git push did not finish within 10 minutes; check the remote before pushing again"))?;
    if !output.status.success() {
        return Err(BusError::conflict(
            "git.push_failed",
            format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim()),
        ));
    }
    Ok(())
}

/// `git switch`, bounded, with its refusals turned into errors a person can act on. Git never
/// discards anything on these paths: every refusal leaves the checkout as it was.
fn run_switch(root: &Path, args: &[&str], branch: &str) -> Result<(), BusError> {
    let mut command = std::process::Command::new("git");
    // Its refusals are told apart by their English text below, which gettext would translate.
    command.arg("-C").arg(root).args(args).env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C").env("LANGUAGE", "C");
    let output = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(120))
        .map_err(|error| BusError::unavailable("git.branch_switch_failed", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("git.branch_switch_failed", "git switch did not finish within 2 minutes"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let listed = || {
        sample_paths(
            stderr
                .lines()
                .filter(|line| line.starts_with('\t'))
                .map(str::trim),
        )
    };
    if let Some(at) = stderr
        .split("is already checked out at ")
        .nth(1)
        .or_else(|| stderr.split("is already used by worktree at ").nth(1))
    {
        let path = at.lines().next().unwrap_or("").trim().trim_matches(['\'', '"']);
        return Err(BusError::conflict(
            "git.branch_checked_out",
            format!("{branch} is already checked out in {path}. Select that checkout to work on it."),
        ));
    }
    if stderr.contains("local changes to the following files would be overwritten") {
        return Err(BusError::conflict(
            "git.checkout_conflict",
            format!("Your uncommitted changes to {} would be overwritten by {branch}. Commit them first; nothing was changed.", listed()),
        ));
    }
    if stderr.contains("untracked working tree files would be overwritten") {
        return Err(BusError::conflict(
            "git.branch_switch_failed",
            format!("{branch} has its own copy of {}, which is untracked or ignored here. Move it aside first; nothing was changed.", listed()),
        ));
    }
    Err(BusError::conflict("git.branch_switch_failed", format!("git {} failed: {stderr}", args.join(" "))))
}

/// Up to three paths, then a count, for one-line messages.
fn sample_paths<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let paths: Vec<&str> = paths.collect();
    let shown = paths.iter().take(3).copied().collect::<Vec<_>>().join(", ");
    match paths.len() {
        0 => String::from("some files"),
        n if n > 3 => format!("{shown} and {} more", n - 3),
        _ => shown,
    }
}

/// `refs/remotes/*` as remote branches, each linked to the local branch of the same name.
fn remote_branches(
    repo: &gix::Repository,
    platform: &gix::reference::iter::Platform<'_>,
    local: &[Branch],
) -> Result<Vec<RemoteBranch>, BusError> {
    let remotes: Vec<String> = repo.remote_names().iter().map(|r| r.to_string()).collect();
    let mut out = Vec::new();
    for reference in platform.remote_branches().map_err(gix_err("git.branches_failed"))? {
        let reference = reference.map_err(gix_err("git.branches_failed"))?;
        // Symbolic refs — `origin/HEAD` — point at a branch already listed.
        let Some(id) = reference.try_id() else {
            continue;
        };
        let name = reference.name().as_bstr().to_string();
        let Some(name) = name.strip_prefix("refs/remotes/").map(str::to_owned) else {
            continue;
        };
        let (remote, branch) = remotes
            .iter()
            .filter_map(|remote| name.strip_prefix(&format!("{remote}/")).map(|b| (remote.clone(), b.to_owned())))
            .min_by_key(|(_, branch)| branch.len())
            .or_else(|| name.split_once('/').map(|(r, b)| (r.to_owned(), b.to_owned())))
            .unwrap_or_else(|| (String::new(), name.clone()));
        if branch == "HEAD" {
            continue;
        }
        out.push(RemoteBranch {
            local: local.iter().find(|b| b.name == branch).map(|b| b.name.clone()),
            head: id.to_string(),
            name,
            remote,
            branch,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn gix_err<E: std::fmt::Display>(code: &'static str) -> impl Fn(E) -> BusError {
    move |e| BusError::unavailable(code, e.to_string())
}

/// A revision that did not resolve is the caller's to fix, not an outage (RA-369): one git
/// cannot parse is `invalid`, a well-formed one naming nothing is `not_found`, and only a
/// failure to read the repository on the way stays `unavailable`.
fn revision_err(spec: &str) -> impl Fn(gix::revision::spec::parse::single::Error) -> BusError + '_ {
    move |error| {
        use gix::revision::spec::parse::single::Error;
        let message = format!("{spec}: {error}");
        let Error::Parse(source) = &error else {
            return BusError::invalid("git.revision", message);
        };
        let unreadable = source.sources().any(|e| {
            e.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() != std::io::ErrorKind::NotFound)
        });
        if unreadable {
            BusError::unavailable("git.revision", message)
        } else if revision_syntax_ok(spec) {
            BusError::not_found("git.revision", message)
        } else {
            BusError::invalid("git.revision", message)
        }
    }
}

/// Whether `spec` is a well-formed revspec, by git's grammar alone: every lookup succeeds.
fn revision_syntax_ok(spec: &str) -> bool {
    use gix::bstr::BStr;
    use gix::revision::plumbing::spec::parse::{delegate, Delegate};
    use gix::Exn;
    struct Grammar;
    impl delegate::Revision for Grammar {
        fn find_ref(&mut self, _: &BStr) -> Result<(), Exn> { Ok(()) }
        fn disambiguate_prefix(&mut self, _: gix::hash::Prefix, _: Option<delegate::PrefixHint<'_>>) -> Result<(), Exn> { Ok(()) }
        fn reflog(&mut self, _: delegate::ReflogLookup) -> Result<(), Exn> { Ok(()) }
        fn nth_checked_out_branch(&mut self, _: usize) -> Result<(), Exn> { Ok(()) }
        fn sibling_branch(&mut self, _: delegate::SiblingBranch) -> Result<(), Exn> { Ok(()) }
    }
    impl delegate::Navigate for Grammar {
        fn traverse(&mut self, _: delegate::Traversal) -> Result<(), Exn> { Ok(()) }
        fn peel_until(&mut self, _: delegate::PeelTo<'_>) -> Result<(), Exn> { Ok(()) }
        fn find(&mut self, _: &BStr, _: bool) -> Result<(), Exn> { Ok(()) }
        fn index_lookup(&mut self, _: &BStr, _: u8) -> Result<(), Exn> { Ok(()) }
    }
    impl delegate::Kind for Grammar {
        fn kind(&mut self, _: gix::revision::plumbing::spec::Kind) -> Result<(), Exn> { Ok(()) }
    }
    impl Delegate for Grammar {
        fn done(&mut self) -> Result<(), Exn> { Ok(()) }
    }
    gix::revision::plumbing::spec::parse(spec.into(), &mut Grammar).is_ok()
}

/// Which open session owns each branch of a project: the one store read behind a listing.
fn branch_owners(conn: &rusqlite::Connection, project_id: relay_bus::types::Id) -> Result<HashMap<String, String>, BusError> {
    let mut owners = HashMap::new();
    let mut st = conn
        .prepare_cached("SELECT branch,name FROM sessions WHERE project_id=?1 AND state!='closed'")
        .bus()?;
    for row in st
        .query_map([project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .bus()?
    {
        let (b, n) = row.bus()?;
        owners.insert(b, n);
    }
    Ok(owners)
}

/// The branch listing, merged state included: history walks, so never under the store lock.
/// `current` is `root`'s branch; remote branches only when `with_remotes`.
fn branches_with(
    root: &Path,
    base_branch: &str,
    owners: &HashMap<String, String>,
    with_remotes: bool,
) -> Result<BranchesOut, BusError> {
    let mut repo = gix::open(root).map_err(gix_err("git.open_failed"))?;
    // Every walk below decompresses commits; without a cache each branch paid for its own.
    repo.object_cache_size_if_unset(16 * 1024 * 1024);
    let current = repo
        .head_name()
        .ok()
        .flatten()
        .map(|n| n.shorten().to_string())
        .unwrap_or_default();
    let head = repo
        .rev_parse_single(base_branch)
        .ok()
        .map(|id| id.detach());
    let mut tips = Vec::new();
    let platform = repo.references().map_err(gix_err("git.branches_failed"))?;
    let refs = platform
        .local_branches()
        .map_err(gix_err("git.branches_failed"))?;
    for reference in refs {
        let reference = reference.map_err(gix_err("git.branches_failed"))?;
        let name = reference
            .name()
            .as_bstr()
            .to_string()
            .trim_start_matches("refs/heads/")
            .to_string();
        if let Some(id) = reference.try_id() {
            tips.push((name, id.detach()));
        }
    }
    // One walk from the base, stopping once every tip has been seen, decides "merged" for all
    // branches. A merge-base per branch was branches × distance to the base, with every
    // commit on the way inflated once per branch: 520 ms and 545 MB for ~150 branches and 65
    // commits (PERF §1.3).
    let merged_tips = merged_tips(&repo, head, tips.iter().map(|(_, id)| *id));
    let mut branches = Vec::new();
    for (name, id) in tips {
        let merged = merged_tips.contains(&id);
        let (upstream, ahead, behind) = upstream_metrics(&repo, &name);
        branches.push(Branch {
            name: name.clone(),
            head: id.to_string(),
            upstream,
            ahead,
            behind,
            merged,
            session: owners.get(&name).cloned(),
            current: name == current,
        });
    }
    branches.sort_by_key(|b| (!b.current, b.name.clone()));
    let remote_branches = if with_remotes { remote_branches(&repo, &platform, &branches)? } else { Vec::new() };
    Ok(BranchesOut { current, branches, remote_branches })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_pull_requests_cover_all_pages_and_lifecycle_states() {
        let row = |number, state: &str, merged_at: serde_json::Value, repository: &str| {
            json!({
                "number":number,"head":{"ref":"feature","repo":{"full_name":repository}},
                "base":{"ref":"main","repo":{"full_name":"owner/repo"}},
                "draft":false,"html_url":"https://example.test/pr","title":"Changes",
                "state":state,"merged_at":merged_at
            })
        };
        let fixture = json!([
            [row(1, "open", serde_json::Value::Null, "owner/repo")],
            [
                row(2, "closed", json!("2026-09-05T00:00:00Z"), "owner/repo"),
                row(3, "closed", serde_json::Value::Null, "other/fork")
            ]
        ]);
        let rows = decode_pull_requests(&serde_json::to_vec(&fixture).unwrap()).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|pr| pr.state.as_str()).collect::<Vec<_>>(),
            ["open", "merged", "closed"]
        );
        assert!(rows[1].same_repository);
        assert!(!rows[2].same_repository);
        assert!(decode_pull_requests(b"[[]").is_err());
    }

    #[test]
    fn github_cli_lists_every_open_pr_and_one_page_of_closed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let pr = |number: u64, state: &str, merged: bool| {
            format!(
                r#"{{"number":{number},"head":{{"ref":"b{number}","repo":{{"full_name":"o/r"}}}},"base":{{"ref":"main","repo":{{"full_name":"o/r"}}}},"html_url":"u","title":"t","state":"{state}","merged_at":{}}}"#,
                if merged { "\"2026-09-05T00:00:00Z\"" } else { "null" }
            )
        };
        let gh = dir.path().join("gh");
        write_script(&gh, &format!(
            "#!/usr/bin/env python3\nimport sys\nargs = sys.argv[1:]\nif args == ['--probe']: sys.exit(0)\nassert args[0] == 'api' and 'state=all' not in args[-1]\nif '--paginate' in args:\n    assert args[1:3] == ['--paginate','--slurp'] and 'state=open' in args[3]\n    print('[[{}], []]')\nelse:\n    assert len(args) == 2 and 'state=closed' in args[1] and 'per_page=100' in args[1] and 'sort=updated' in args[1]\n    print('[{}, {}]')\n",
            pr(1, "open", false), pr(2, "closed", true), pr(3, "closed", false)
        ));
        let result = list_pull_requests(&gh, dir.path()).unwrap();
        assert!(result.complete);
        assert_eq!(
            result.pull_requests.iter().map(|pr| (pr.number, pr.state.as_str())).collect::<Vec<_>>(),
            [(1, "open"), (2, "merged"), (3, "closed")]
        );
        let failing = dir.path().join("gh-failing");
        write_script(&failing, "#!/usr/bin/env python3\nimport sys\nprint('[[]]')\nsys.exit(1)\n");
        assert!(list_pull_requests(&failing, dir.path()).is_err(), "a failure is not an empty list");
        // The open PRs stand without the closed page; the listing says it is not complete.
        let partial = dir.path().join("gh-partial");
        write_script(&partial, &format!(
            "#!/usr/bin/env python3\nimport sys\nif '--paginate' in sys.argv: print('[[{}]]')\nelse: sys.exit(1)\n",
            pr(1, "open", false)
        ));
        let result = list_pull_requests(&partial, dir.path()).unwrap();
        assert!(!result.complete);
        assert_eq!(result.pull_requests.len(), 1);
    }

    /// Writes an executable script and waits until it can be run. A test thread that forks while
    /// this one still holds the file open for writing gives its child a copy of that descriptor
    /// until the child execs, and until then running the script fails with ETXTBSY.
    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match std::process::Command::new(path).arg("--probe").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status() {
                Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) && std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(error) => panic!("{}: {error}", path.display()),
                Ok(_) => return,
            }
        }
    }

    #[test]
    fn commit_messages_are_cleaned_like_git_commit_m() {
        assert_eq!(clean_message("\n\nSubject  \n\n\n\nBody line \t\n\n"), "Subject\n\nBody line");
        assert_eq!(clean_message("one"), "one");
        assert_eq!(clean_message(" \n \n"), "");
    }

    #[test]
    fn pull_request_listings_are_reused_until_a_push_or_new_pr() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        assert!(cached_prs(repo).is_none());
        let listed = Ok(PrListOut { pull_requests: Vec::new(), complete: true });
        remember_prs(repo, &listed);
        assert_eq!(cached_prs(repo), Some(listed));
        assert!(cached_prs(&repo.join("other")).is_none(), "a listing answers only its own repository");
        forget_prs(repo);
        assert!(cached_prs(repo).is_none());
        // Offline, gh waits out its timeout: the failure answers the next refreshes too, rather
        // than every one of them waiting again.
        let failed = Err(BusError::unavailable("git.pr_list_timeout", "gh did not answer within 10s"));
        remember_prs(repo, &failed);
        assert_eq!(cached_prs(repo), Some(failed));
        forget_prs(repo);
        assert!(cached_prs(repo).is_none());
    }

    /// RA-160: a timed-out child gets SIGTERM, which is when git deletes its `index.lock`,
    /// before anything is SIGKILLed.
    #[test]
    fn a_timed_out_child_may_clean_up_its_lock_before_it_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("index.lock");
        let mut command = std::process::Command::new("sh");
        command.current_dir(dir.path()).args([
            "-c",
            "trap 'rm -f index.lock; exit 143' TERM; : > index.lock; while :; do sleep 0.05; done",
        ]);
        let started = std::time::Instant::now();
        let out = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_millis(300)).unwrap();
        assert!(out.is_none(), "the child outlived its deadline");
        assert!(!lock.exists(), "the lock outlived the timed-out child");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    #[test]
    fn a_capped_status_keeps_conflicts_and_tracked_changes_first() {
        let file = |path: String, index: &str, worktree: &str| FileStatus {
            path, index: index.into(), worktree: worktree.into(), renamed_from: None,
        };
        let mut files: Vec<FileStatus> = (0..STATUS_MAX_FILES + 10).map(|n| file(format!("a/new{n:05}"), "", "?")).collect();
        files.push(file("z/conflict".into(), "U", "U"));
        files.push(file("z/both-added".into(), "A", "A"));
        files.push(file("z/edited".into(), "", "M"));
        let kept = cap_status(files);
        assert_eq!(kept.len(), STATUS_MAX_FILES);
        for path in ["z/conflict", "z/both-added", "z/edited"] {
            assert!(kept.iter().any(|f| f.path == path), "{path} was dropped");
        }
        assert!(kept.windows(2).all(|pair| pair[0].path < pair[1].path), "kept in path order");
        // Long paths stop at the byte budget instead.
        let long: Vec<FileStatus> = (0..1000).map(|n| file(format!("{n:04}{}", "x".repeat(4000)), "", "M")).collect();
        let kept = cap_status(long);
        assert!(kept.len() < 1000 && kept.iter().map(|f| f.path.len()).sum::<usize>() <= STATUS_MAX_BYTES);
        let few = vec![file("one".into(), "M", "")];
        assert_eq!(cap_status(few.clone()), few);
    }

    #[test]
    fn numstat_records_become_gate_lines_with_a_rename_source_of_its_own() {
        let raw = "1\t2\tsrc/a.rs\0-\t-\tlogo.png\0\
                   0\t1\t\0secret/key.txt\0public/key.txt\0";
        assert_eq!(
            numstat_lines(raw),
            "1\t2\tsrc/a.rs\n-\t-\tlogo.png\n0\t0\tsecret/key.txt\n0\t1\tpublic/key.txt\n"
        );
        assert_eq!(numstat_lines(""), "");
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn local_remote_fetch_refreshes_new_worktree_without_moving_primary() {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        let checkout = dir.path().join("checkout");
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "-b", "main"]);
        git(&origin, &["config", "user.name", "Fixture"]);
        git(&origin, &["config", "user.email", "fixture@example.test"]);
        git(&origin, &["commit", "--allow-empty", "-m", "initial"]);
        git(
            dir.path(),
            &[
                "clone",
                origin.to_str().unwrap(),
                checkout.to_str().unwrap(),
            ],
        );
        let original = git(&checkout, &["rev-parse", "HEAD"]);
        git(
            &origin,
            &["commit", "--allow-empty", "-m", "remote advance"],
        );
        let advanced = git(&origin, &["rev-parse", "HEAD"]);
        fetch_remote(&checkout).unwrap();
        let from = new_worktree_base(&checkout, "main").unwrap().unwrap();
        let new_path = dir.path().join("new-session");
        worktree::create(&checkout, &new_path, "relay/fixture", Some(&from)).unwrap();
        assert_eq!(git(&new_path, &["rev-parse", "HEAD"]), advanced);
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), original);
        let brief = briefing_state(&new_path, "relay/fixture", "main");
        assert!(brief.contains("git_base_upstream_behind: 1"));
        assert!(brief.contains("cached snapshot"));
    }

    #[test]
    fn drift_counts_each_worktree_against_the_freshest_base() {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        let checkout = dir.path().join("checkout");
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "-b", "main"]);
        git(&origin, &["config", "user.name", "Fixture"]);
        git(&origin, &["config", "user.email", "fixture@example.test"]);
        git(&origin, &["commit", "--allow-empty", "-m", "initial"]);
        git(dir.path(), &["clone", origin.to_str().unwrap(), checkout.to_str().unwrap()]);
        git(&checkout, &["config", "user.name", "Fixture"]);
        git(&checkout, &["config", "user.email", "fixture@example.test"]);
        let linked = dir.path().join("linked");
        git(&checkout, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()]);
        git(&linked, &["commit", "--allow-empty", "-m", "side work"]);
        for n in 0..3 {
            git(&origin, &["commit", "--allow-empty", "-m", &format!("merged {n}")]);
        }
        let row = |out: &DriftOut, path: &Path| {
            let want = std::fs::canonicalize(path).unwrap().display().to_string();
            let row = out.worktrees.iter().find(|w| w.path == want).unwrap();
            (row.behind, row.ahead)
        };
        // Cached refs only: nothing is behind until a fetch has seen the new commits.
        let before = drift(&checkout, "main").unwrap();
        assert_eq!(before.base, "origin/main");
        assert_eq!(row(&before, &linked), (Some(0), Some(1)));
        fetch_remote(&checkout).unwrap();
        let after = drift(&checkout, "main").unwrap();
        assert_eq!(after.head, git(&origin, &["rev-parse", "HEAD"]));
        assert_eq!(row(&after, &checkout), (Some(3), Some(0)));
        assert_eq!(row(&after, &linked), (Some(3), Some(1)));
        // A local base ahead of its remote is the freshest one.
        git(&checkout, &["merge", "-q", "--ff-only", "origin/main"]);
        git(&checkout, &["commit", "--allow-empty", "-m", "local only"]);
        let local = drift(&checkout, "main").unwrap();
        assert_eq!(local.base, "main");
        assert_eq!(row(&local, &linked), (Some(4), Some(1)));
    }

    #[test]
    fn missing_upstream_is_unknown_and_fetched_base_preserves_local_commits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Fixture"]);
        git(root, &["config", "user.email", "fixture@example.test"]);
        git(root, &["commit", "--allow-empty", "-m", "base"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        let metrics = || upstream_metrics(&gix::open(root).unwrap(), "main");
        assert_eq!(metrics(), (None, None, None));
        git(root, &["config", "branch.main.remote", "origin"]);
        git(root, &["config", "branch.main.merge", "refs/heads/main"]);
        git(
            root,
            &["config", "remote.origin.url", "/nonexistent/relay-fixture"],
        );
        git(
            root,
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        );
        assert_eq!(metrics(), (Some("origin/main".into()), None, None));
        assert!(briefing_state(root, "main", "main").contains("git_ahead: unknown"));
        git(root, &["update-ref", "refs/remotes/origin/main", &base]);
        assert_eq!(metrics(), (Some("origin/main".into()), Some(0), Some(0)));
        git(root, &["checkout", "-b", "remote-fixture"]);
        git(root, &["commit", "--allow-empty", "-m", "remote"]);
        let remote = git(root, &["rev-parse", "HEAD"]);
        git(root, &["update-ref", "refs/remotes/origin/main", &remote]);
        git(root, &["checkout", "main"]);
        assert_eq!(
            new_worktree_base(root, "main").unwrap(),
            Some(remote)
        );
        assert_eq!(git(root, &["rev-parse", "main"]), base);
        git(root, &["commit", "--allow-empty", "-m", "local"]);
        assert_eq!(metrics(), (Some("origin/main".into()), Some(1), Some(1)));
        assert_eq!(new_worktree_base(root, "main").unwrap_err().code, "git.base_diverged");
        let local = git(root, &["rev-parse", "main"]);
        assert!(fetch_remote(root).is_err());
        assert_eq!(git(root, &["rev-parse", "main"]), local);
        assert_eq!(new_worktree_base(root, "main").unwrap_err().code, "git.base_diverged");
    }

    /// RA-375: a tag (or `refs/<name>`) with a branch's name does not hide its upstream.
    #[test]
    fn upstream_metrics_finds_a_branch_shadowed_by_a_tag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Fixture"]);
        git(root, &["config", "user.email", "fixture@example.test"]);
        git(root, &["commit", "--allow-empty", "-m", "base"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        git(root, &["config", "branch.main.remote", "origin"]);
        git(root, &["config", "branch.main.merge", "refs/heads/main"]);
        git(root, &["config", "remote.origin.url", "/nonexistent/relay-fixture"]);
        git(root, &["config", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"]);
        git(root, &["update-ref", "refs/remotes/origin/main", &base]);
        git(root, &["tag", "main"]);
        let metrics = upstream_metrics(&gix::open(root).unwrap(), "main");
        assert_eq!(metrics, (Some("origin/main".into()), Some(0), Some(0)));
    }

    /// RA-376: fetches into one repository wait for each other instead of racing on its
    /// remote-tracking ref locks, where one of them used to fail.
    #[test]
    fn concurrent_fetches_of_one_repository_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        let checkout = dir.path().join("checkout");
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "-b", "main"]);
        git(&origin, &["config", "user.name", "Fixture"]);
        git(&origin, &["config", "user.email", "fixture@example.test"]);
        git(&origin, &["commit", "--allow-empty", "-m", "initial"]);
        git(dir.path(), &["clone", origin.to_str().unwrap(), checkout.to_str().unwrap()]);
        let linked = dir.path().join("linked");
        git(&checkout, &["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
        for round in 0..3 {
            git(&origin, &["commit", "--allow-empty", "-m", &format!("advance {round}")]);
            let results: Vec<_> = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..4)
                    .map(|n| {
                        let root = if n % 2 == 0 { checkout.clone() } else { linked.clone() };
                        scope.spawn(move || fetch_remote(&root))
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for result in results {
                result.unwrap();
            }
            assert_eq!(git(&checkout, &["rev-parse", "origin/main"]), git(&origin, &["rev-parse", "HEAD"]));
        }
    }

    /// RA-369: a revision that names nothing is not_found, one git cannot parse is invalid.
    #[test]
    fn unresolved_revisions_are_the_callers_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Fixture"]);
        git(root, &["config", "user.email", "fixture@example.test"]);
        git(root, &["commit", "--allow-empty", "-m", "base"]);
        let repo = gix::open(root).unwrap();
        let kind = |spec: &str| repo.rev_parse_single(spec).map(|_| ()).map_err(|e| revision_err(spec)(e).kind);
        assert_eq!(kind("main"), Ok(()));
        for unknown in ["no-such-branch", "main~5", "deadbeefdeadbeef"] {
            assert_eq!(kind(unknown), Err(relay_bus::ErrorKind::NotFound), "{unknown}");
        }
        for malformed in ["main@{", "main^{nonsense}", "main..main"] {
            assert_eq!(kind(malformed), Err(relay_bus::ErrorKind::Invalid), "{malformed}");
        }
    }
}
