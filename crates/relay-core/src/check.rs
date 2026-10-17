//! The check a change has to pass before an agent pushes it (`guardrails.check_on_push`).
//!
//! A project's own `build_cmd` is the check when it has one. Without it, [`detect`] names the
//! test command of the build tool at the checkout's root, the same one for every project of
//! that kind. It is only ever a starting point: a project whose real check is stricter (lints,
//! several modules) says so in `build_cmd`.

use std::fs;
use std::path::Path;

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
    use super::{detect, pushes_commits};

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
}
