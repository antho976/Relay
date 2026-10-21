//! A build seed end to end, on a real (tiny) workspace: build it from a commit, copy the
//! generation into a fresh worktree, and let Cargo judge the copy. The third-party dependency
//! must come back fresh and the workspace's own crate must compile from the worktree's source.
//!
//! It needs a filesystem that can share extents (btrfs, XFS) under the target directory, and
//! `itoa` 1.x in the local registry cache; without either it says so and passes. CI's ext4
//! runners take the first exit.

use relay_core::build_seed;
use std::path::Path;
use std::process::Command;

fn run(dir: &Path, program: &str, args: &[&str]) -> std::process::Output {
    let out = Command::new(program).args(args).current_dir(dir).env_remove("CARGO_TARGET_DIR").output().unwrap();
    assert!(out.status.success(), "{program} {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out
}

#[test]
fn a_seeded_worktree_rebuilds_only_its_own_crates() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("build-seed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    if !build_seed::probe_reflink(&root) {
        eprintln!("skipped: {} cannot share extents", root.display());
        return;
    }
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join("app/src")).unwrap();
    std::fs::write(repo.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\nmembers = [\"app\"]\n").unwrap();
    std::fs::write(repo.join("app/Cargo.toml"), "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nitoa = \"1\"\n").unwrap();
    std::fs::write(repo.join("app/src/lib.rs"), "pub fn show(n: u32) -> String { itoa::Buffer::new().format(n).to_string() }\n").unwrap();
    std::fs::write(repo.join(".gitignore"), "/target\n").unwrap();
    let lock = Command::new("cargo").args(["generate-lockfile", "--offline"]).current_dir(&repo).output().unwrap();
    if !lock.status.success() {
        eprintln!("skipped: no itoa in the registry cache: {}", String::from_utf8_lossy(&lock.stderr));
        return;
    }
    run(&repo, "git", &["init", "-q", "-b", "main"]);
    run(&repo, "git", &["add", "-A"]);
    run(&repo, "git", &["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-qm", "init"]);
    let commit = String::from_utf8(run(&repo, "git", &["rev-parse", "HEAD"]).stdout).unwrap().trim().to_string();

    let mut published = Vec::new();
    let count = build_seed::build_seed(&repo, &commit, |generation, step, steps| {
        published.push((generation.meta.clone(), step, steps));
        Default::default()
    }).unwrap();
    assert!(count >= 1, "nothing published: {published:?}");
    let dir = build_seed::seed_dir(&repo);
    let current = build_seed::current(&dir).expect("a current generation");
    assert_eq!(current.meta.commit, commit);
    assert_eq!(published.first().unwrap().0.step, "cargo test --no-run");
    assert!(published.first().unwrap().0.ok, "{}", std::fs::read_to_string(dir.join("last-build.log")).unwrap_or_default());
    // The workspace's own crate left no unit behind to be judged fresh.
    let fingerprints: Vec<String> = std::fs::read_dir(current.target().join("debug/.fingerprint")).unwrap()
        .flatten().map(|entry| entry.file_name().to_string_lossy().to_string()).collect();
    assert!(fingerprints.iter().any(|name| name.starts_with("itoa-")), "{fingerprints:?}");
    assert!(!fingerprints.iter().any(|name| name.starts_with("app-")), "{fingerprints:?}");
    // The export it was built from is gone; only the seed's own state remains.
    assert!(!dir.join("src").exists());

    let worktree = repo.join(".relay/worktrees/a");
    relay_core::worktree::create(&repo, &worktree, "relay/a", None).unwrap();
    assert!(build_seed::copy_into(&current.target(), &worktree, &dir).unwrap());
    // A worktree that has a target/ by then keeps it.
    assert!(!build_seed::copy_into(&current.target(), &worktree, &dir).unwrap());
    assert!(std::fs::read_dir(dir.join("copies")).unwrap().next().is_none(), "a refused copy is cleaned up");

    let build = Command::new("cargo").args(["build", "-v", "--offline"]).current_dir(&worktree).env_remove("CARGO_TARGET_DIR").output().unwrap();
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(build.status.success(), "{stderr}");
    assert!(stderr.contains("Fresh itoa"), "{stderr}");
    assert!(stderr.contains("Compiling app"), "{stderr}");
    let _ = std::fs::remove_dir_all(&root);
}

/// The real thing, by hand: `RELAY_SEED_REPO=<checkout> RELAY_SEED_INTO=<new worktree path>
/// cargo test -p relay-core --test all -- build_seed:: --ignored --nocapture` builds the seed for the
/// checkout's HEAD under its `.relay/build-seed/`, then checks HEAD out at the second path and
/// copies the seed in, for timing a first build there (docs/perf/FIXES-2026-10-16.md).
#[test]
#[ignore]
fn seed_a_real_checkout() {
    let (Ok(repo), Ok(into)) = (std::env::var("RELAY_SEED_REPO"), std::env::var("RELAY_SEED_INTO")) else {
        eprintln!("set RELAY_SEED_REPO and RELAY_SEED_INTO");
        return;
    };
    let (repo, into) = (Path::new(&repo), Path::new(&into));
    let commit = String::from_utf8(run(repo, "git", &["rev-parse", "HEAD"]).stdout).unwrap().trim().to_string();
    let started = std::time::Instant::now();
    build_seed::build_seed(repo, &commit, |generation, step, steps| {
        eprintln!("[{:>4} s] generation {} after step {}/{steps}: {} ({})", started.elapsed().as_secs(),
            generation.meta.generation, step + 1, generation.meta.step, if generation.meta.ok { "ok" } else { "failed" });
        Default::default()
    }).unwrap();
    let dir = build_seed::seed_dir(repo);
    eprintln!("{}", std::fs::read_to_string(dir.join("last-build.log")).unwrap_or_default());
    run(repo, "git", &["worktree", "add", "--detach", &into.display().to_string(), &commit]);
    let copy = std::time::Instant::now();
    assert!(build_seed::copy_into(&build_seed::current(&dir).unwrap().target(), into, &dir).unwrap());
    eprintln!("copied into {} in {} ms", into.display(), copy.elapsed().as_millis());
}
