//! Linked repositories: another repository cloned into one session's checkout at
//! `.relay/linked/<name>` for as long as that session needs it (`session.repo.*`). `.relay/` is
//! in `.git/info/exclude`, which every worktree of the repository shares, so the clone never
//! shows up in the session's own status, and it is deleted with the checkout. The folder is the
//! record: nothing about a linked repository lives in the store.
//!
//! What removing one would lose is [`unpushed`]: everything a fresh clone of its remote would
//! not bring back. Every removal that would delete a checkout checks its linked repositories
//! too (`handlers::session::dirty_checkouts`).

use relay_bus::ops::session::{LinkedRepo, LinkedRepoUnpushed};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Where a checkout keeps its linked repositories, relative to the checkout.
pub const DIR: &str = ".relay/linked";

const GIT_TIMEOUT: Duration = Duration::from_secs(10);

/// A clone is network work of unbounded length; `project.clone` allows as long.
pub const CLONE_TIMEOUT: Duration = Duration::from_secs(29 * 60);

/// Where a clone is made before it is renamed into place, beside the linked repositories so the
/// rename never crosses a filesystem. [`list`] skips these.
const SCRATCH_PREFIX: &str = ".clone-";

pub fn dir(worktree: &Path) -> PathBuf {
    worktree.join(DIR)
}

/// The linked repositories of a checkout, by name: every folder under [`DIR`] with a `.git`.
/// One `read_dir`, no subprocess.
pub fn list(worktree: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir(worktree)) else { return Vec::new() };
    let mut repos: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            (!name.starts_with(SCRATCH_PREFIX) && entry.path().join(".git").exists()).then(|| (name, entry.path()))
        })
        .collect();
    repos.sort();
    repos
}

/// The URL `git clone` is given: `owner/name` is a GitHub repository, anything else is taken
/// as written. A leading `-` is refused, so the URL can never read as an option.
pub fn clone_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("a repository URL is required".into());
    }
    if url.starts_with('-') {
        return Err(format!("{url:?} is not a repository URL"));
    }
    let shorthand = url.split('/').count() == 2
        && url.split('/').all(|part| !part.is_empty() && part != "." && part != ".."
            && part.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)));
    if shorthand {
        return Ok(format!("https://github.com/{url}.git"));
    }
    Ok(url.to_string())
}

/// The folder name a repository is linked under: `requested`, or the last part of the URL
/// without `.git`. One plain directory name, which does not start with `.`.
pub fn name_for(url: &str, requested: Option<&str>) -> Result<String, String> {
    let name = match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some(name) => name.to_string(),
        None => url.trim().trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or_default().trim_end_matches(".git").to_string(),
    };
    if name.is_empty() || name.starts_with('.') || !name.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err(format!("{name:?} cannot name a linked repository: letters, digits, '-', '_' and '.', not starting with '.'"));
    }
    Ok(name)
}

/// Clone `url` into `worktree`'s [`DIR`] as `name`, optionally on `branch`. The clone is made
/// in a scratch folder and renamed into place, so a failed or concurrent one never leaves a
/// half-made repository under the name. Network work: run it with the store unlocked.
pub fn clone(worktree: &Path, url: &str, name: &str, branch: Option<&str>) -> Result<PathBuf, String> {
    let parent = dir(worktree);
    let destination = parent.join(name);
    if destination.exists() {
        return Err(format!("{} already exists", destination.display()));
    }
    std::fs::create_dir_all(&parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let scratch = parent.join(format!("{SCRATCH_PREFIX}{}", uuid::Uuid::new_v4().simple()));
    let mut command = Command::new("git");
    command.current_dir(&parent);
    crate::proc::quiet_network_git(&mut command);
    command.arg("clone");
    if let Some(branch) = branch.map(str::trim).filter(|branch| !branch.is_empty()) {
        command.arg(format!("--branch={branch}"));
    }
    command.arg("--").arg(url).arg(&scratch);
    let failed = |message: String| {
        let _ = std::fs::remove_dir_all(&scratch);
        Err(message)
    };
    match crate::proc::output_with_timeout(&mut command, CLONE_TIMEOUT) {
        Err(error) => return failed(format!("cannot start git: {error}")),
        Ok(None) => return failed(format!("git clone did not finish within {} minutes", CLONE_TIMEOUT.as_secs() / 60)),
        Ok(Some(output)) if !output.status.success() => return failed(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Ok(Some(_)) => {}
    }
    if destination.exists() {
        return failed(format!("{} already exists", destination.display()));
    }
    if let Err(error) = std::fs::rename(&scratch, &destination) {
        return failed(format!("moving the clone into place: {error}"));
    }
    Ok(destination)
}

/// Name, path, origin and branch, read through gix: no subprocess.
pub fn describe(name: &str, path: &Path) -> LinkedRepo {
    let repo = gix::open(path).ok();
    let url = repo.as_ref().and_then(|repo| {
        repo.config_snapshot().string("remote.origin.url").map(|url| url.to_string())
    });
    let branch = repo.as_ref().and_then(|repo| repo.head_name().ok().flatten()).map(|name| name.shorten().to_string());
    LinkedRepo { name: name.to_string(), path: path.display().to_string(), url, branch, unpushed: None }
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let output = crate::proc::output_with_timeout(Command::new("git").arg("-C").arg(repo).args(args), GIT_TIMEOUT).ok()??;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What deleting the clone at `path` would lose that its remote does not hold: uncommitted
/// and untracked files, commits no remote-tracking ref contains, stashes, and git-ignored
/// keepers. Unpushed commits are judged against the last fetch, so a stale one can only
/// overcount. A few bounded git calls: run it with the store unlocked.
pub fn unpushed(path: &Path) -> LinkedRepoUnpushed {
    let changed = crate::worktree::status_files_with(path, crate::worktree::Untracked::Directories).ok().map(|files| files.len() as i64);
    // `--ignore-missing` lets an unborn HEAD (an empty repository) through.
    let commits = git(path, &["rev-list", "--ignore-missing", "--branches", "HEAD", "--not", "--remotes"]);
    let mut branches = Vec::new();
    if let Some(listed) = &commits {
        let unpushed: std::collections::HashSet<&str> = listed.lines().collect();
        // A branch holds an unpushed commit exactly when its tip is one: a tip some remote
        // contains brings every commit under it along.
        if !unpushed.is_empty() {
            for line in git(path, &["for-each-ref", "--format=%(objectname) %(refname:short)", "refs/heads"]).unwrap_or_default().lines() {
                if let Some((tip, branch)) = line.split_once(' ') {
                    if unpushed.contains(tip) {
                        branches.push(branch.to_string());
                    }
                }
            }
        }
    }
    let stashes = std::fs::read_to_string(path.join(".git").join("logs").join("refs").join("stash"))
        .map(|log| log.lines().filter(|line| !line.trim().is_empty()).count() as i64)
        .unwrap_or(0);
    let ignored = crate::branch_cleanup::ignored_keepers(path).unwrap_or_default();
    LinkedRepoUnpushed {
        changed,
        commits: commits.map(|listed| listed.lines().filter(|line| !line.is_empty()).count() as i64),
        branches,
        stashes,
        ignored,
    }
}

/// Whether [`unpushed`] found anything, a scan that failed included.
pub fn holds_work(found: &LinkedRepoUnpushed) -> bool {
    found.changed != Some(0) || found.commits != Some(0) || found.stashes > 0 || !found.ignored.is_empty()
}

/// One line naming what [`unpushed`] found, for a refusal.
pub fn summary(found: &LinkedRepoUnpushed) -> String {
    let plural = |n: i64| if n == 1 { "" } else { "s" };
    let mut what = Vec::new();
    match found.changed {
        Some(0) => {}
        Some(n) => what.push(format!("{n} uncommitted change{}", plural(n))),
        None => what.push("changes that could not be checked".to_string()),
    }
    match found.commits {
        Some(0) => {}
        Some(n) if found.branches.is_empty() => what.push(format!("{n} unpushed commit{}", plural(n))),
        Some(n) => what.push(format!("{n} unpushed commit{} (on {})", plural(n), found.branches.join(", "))),
        None => what.push("commits that could not be checked against its remote".to_string()),
    }
    if found.stashes > 0 {
        what.push(format!("{} stash{}", found.stashes, if found.stashes == 1 { "" } else { "es" }));
    }
    if !found.ignored.is_empty() {
        let n = found.ignored.len() as i64;
        let named = found.ignored.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        what.push(format!("{n} git-ignored file{} ({named}{})", plural(n), if n > 3 { ", …" } else { "" }));
    }
    what.join(", ")
}

/// The linked repositories of `worktree` that hold work, each with what [`unpushed`] found.
pub fn holding_work(worktree: &Path) -> Vec<(String, LinkedRepoUnpushed)> {
    list(worktree)
        .into_iter()
        .map(|(name, path)| (name, unpushed(&path)))
        .filter(|(_, found)| holds_work(found))
        .collect()
}

/// Delete one linked repository; returns the bytes freed. `path` must be a direct child of
/// `worktree`'s [`DIR`], checked here rather than trusted.
pub fn remove(worktree: &Path, name: &str) -> Result<u64, String> {
    let path = dir(worktree).join(name);
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." || path.parent() != Some(dir(worktree).as_path()) {
        return Err(format!("{name:?} is not a linked repository"));
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|_| format!("no linked repository {name}"))?;
    if !meta.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    let freed = crate::worktree::dir_size(&path);
    std::fs::remove_dir_all(&path).map_err(|error| format!("deleting {}: {error}", path.display()))?;
    Ok(freed)
}

/// The linked repository `path` (absolute, or relative to `worktree`) is in, and `path` relative
/// to it: git of the checkout cannot answer for a file there, since `.relay/` is excluded.
pub fn containing(worktree: &Path, path: &Path) -> Option<(PathBuf, PathBuf)> {
    let rest = worktree.join(path).strip_prefix(dir(worktree)).ok()?.to_path_buf();
    let mut parts = rest.components();
    let std::path::Component::Normal(name) = parts.next()? else { return None };
    let repo = dir(worktree).join(name);
    repo.join(".git").exists().then(|| (repo, parts.as_path().to_path_buf()))
}

/// The brief's line: each linked repository and where it is.
pub fn brief_line(worktree: &Path) -> String {
    let repos = list(worktree);
    if repos.is_empty() {
        return "none".into();
    }
    repos.iter().map(|(name, path)| format!("{name} at {}", path.display())).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_is_github() {
        assert_eq!(clone_url("antho976/relay").unwrap(), "https://github.com/antho976/relay.git");
        assert_eq!(clone_url(" https://example.com/a/b.git ").unwrap(), "https://example.com/a/b.git");
        assert_eq!(clone_url("git@github.com:a/b.git").unwrap(), "git@github.com:a/b.git");
        assert!(clone_url("--upload-pack=touch x").is_err());
        assert!(clone_url("  ").is_err());
        // Not a two-part name: taken as written.
        assert_eq!(clone_url("../x").unwrap(), "../x");
    }

    #[test]
    fn names_are_one_plain_folder() {
        assert_eq!(name_for("https://github.com/a/tally.git", None).unwrap(), "tally");
        assert_eq!(name_for("git@github.com:a/tally.git/", None).unwrap(), "tally");
        assert_eq!(name_for("x", Some(" docs ")).unwrap(), "docs");
        for bad in ["..", ".hidden", "a/b", "a b"] {
            assert!(name_for("x", Some(bad)).is_err(), "{bad:?}");
        }
        assert!(name_for("https://example.com/.git", None).is_err());
    }
}
