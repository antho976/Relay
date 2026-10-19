//! The check a change has to pass before an agent pushes it (`guardrails.check_on_push`).
//!
//! A project's own `build_cmd` is the check when it has one. Without it, [`detect`] names the
//! test command of the build tool at the checkout's root, the same one for every project of
//! that kind. It is only ever a starting point: a project whose real check is stricter (lints,
//! several modules) says so in `build_cmd`.

use std::fs;
use std::path::Path;
use std::process::Command;

/// What git exports to a hook that points at *its* repository (`git rev-parse
/// --local-env-vars`). A check inheriting them sends every `git` it starts there: a test's
/// `git -C <tempdir> config user.name Fixture` writes the pushing repository's config instead.
/// On 2026-10-19 that left `core.bare = true` and a Fixture user in the shared `.git/config`
/// and broke git in every worktree. [`check_command`] clears them, and so does the runner in
/// `.cargo/config.toml` for every test binary, whatever runs it.
pub const GIT_LOCAL_ENV: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// The pre-push check `command`, run by `sh` in `worktree` with git's repository variables
/// ([`GIT_LOCAL_ENV`]) cleared and its output on one stream, in the order it was written.
pub fn check_command(command: &str, worktree: &Path) -> Command {
    let mut check = Command::new("sh");
    check.arg("-c").arg(format!("exec 2>&1\n{command}")).current_dir(worktree).stdin(std::process::Stdio::null());
    for name in GIT_LOCAL_ENV {
        check.env_remove(name);
    }
    check
}

/// The test command for the build tool at `root`, or `None` when nothing there names one.
/// Reads a handful of files at the root and never walks the tree, so it is cheap enough to
/// run inside `project.add`'s transaction.
pub fn detect(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").is_file() {
        return Some("cargo test".into());
    }
    if root.join("go.mod").is_file() {
        return Some("go test ./...".into());
    }
    if root.join("gradlew").is_file() {
        return Some("./gradlew test".into());
    }
    if root.join("package.json").is_file() && has_test_script(&root.join("package.json")) {
        let runner = if root.join("pnpm-lock.yaml").is_file() {
            "pnpm"
        } else if root.join("yarn.lock").is_file() {
            "yarn"
        } else if root.join("bun.lockb").is_file() || root.join("bun.lock").is_file() {
            "bun run"
        } else {
            "npm"
        };
        return Some(format!("{runner} test"));
    }
    None
}

/// npm writes `echo "Error: no test specified" && exit 1` into every new package.json; a
/// project that never replaced it has no tests, and running it would refuse every push.
fn has_test_script(package: &Path) -> bool {
    let Ok(text) = fs::read_to_string(package) else { return false };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
    value["scripts"]["test"].as_str().is_some_and(|script| !script.trim().is_empty() && !script.contains("no test specified"))
}

/// Whether a pre-push hook's stdin (`<local ref> <local sha> <remote ref> <remote sha>` per
/// line) sends any commits: a push that only deletes remote branches, as the merged-branch
/// sweep does, has nothing to check.
pub fn pushes_commits(refs: &str) -> bool {
    refs.lines().any(|line| {
        line.split_whitespace().nth(1).is_some_and(|sha| !sha.bytes().all(|byte| byte == b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::{check_command, detect, pushes_commits, GIT_LOCAL_ENV};

    #[test]
    fn each_build_tool_names_its_tests() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(detect(root), None, "an empty checkout has no check");

        std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#).unwrap();
        assert_eq!(detect(root), None, "npm's placeholder is not a test suite");
        std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"vitest run"}}"#).unwrap();
        assert_eq!(detect(root).as_deref(), Some("npm test"));
        std::fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect(root).as_deref(), Some("pnpm test"));

        std::fs::write(root.join("gradlew"), "").unwrap();
        assert_eq!(detect(root).as_deref(), Some("./gradlew test"));
        std::fs::write(root.join("go.mod"), "").unwrap();
        assert_eq!(detect(root).as_deref(), Some("go test ./..."));
        std::fs::write(root.join("Cargo.toml"), "").unwrap();
        assert_eq!(detect(root).as_deref(), Some("cargo test"), "a Cargo workspace wins over the tools beside it");
    }

    #[test]
    fn a_push_that_only_deletes_sends_nothing_to_check() {
        let zero = "0".repeat(40);
        let sha = "a".repeat(40);
        assert!(!pushes_commits(""));
        assert!(!pushes_commits(&format!("(delete) {zero} refs/heads/old {sha}\n")));
        assert!(pushes_commits(&format!("refs/heads/main {sha} refs/heads/main {zero}\n")));
    }

    /// Every variable this git says points at a repository is cleared, by the push check and by
    /// the test runner alike. A git that adds one fails here, not in someone's `.git/config`.
    #[test]
    fn the_check_and_the_test_runner_clear_every_repository_variable() {
        let listed = std::process::Command::new("git").args(["rev-parse", "--local-env-vars"]).output().unwrap();
        assert!(listed.status.success());
        let listed = String::from_utf8(listed.stdout).unwrap();
        for name in listed.lines().map(str::trim).filter(|name| !name.is_empty()) {
            assert!(GIT_LOCAL_ENV.contains(&name), "git points {name} at the pushing repository, and nothing clears it");
        }

        let check = check_command("true", std::path::Path::new("."));
        let cleared: Vec<_> = check.get_envs().filter(|(_, value)| value.is_none()).map(|(name, _)| name.to_str().unwrap().to_owned()).collect();
        let runner = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.cargo/config.toml")).unwrap();
        for name in GIT_LOCAL_ENV {
            assert!(cleared.iter().any(|c| c == name), "the push check keeps {name}");
            assert!(runner.contains(&format!("\"-u\", \"{name}\"")), "the test runner keeps {name}");
        }
        // And the runner applies: this test binary was started without them.
        for name in GIT_LOCAL_ENV {
            assert!(std::env::var_os(name).is_none(), "{name} reached a test binary");
        }
    }
}
