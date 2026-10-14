//! Linked repositories (`session.repo.*`): another repository cloned into one session's
//! checkout, invisible to its status, listed with what removing it would lose, and never
//! deleted with unpushed work unless the caller says to — by `session.repo.remove` or by a
//! close that would take the checkout. Through the bus, against a local "remote".

mod common;

use common::{call_as as call, committed_repo, engine_with_project, git, ok_as as ok};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn refusal(engine: &Engine, actor: Actor, op: &str, payload: Value) -> relay_bus::error::BusError {
    call(engine, actor, op, payload).into_result().expect_err(&format!("{op} was expected to refuse"))
}

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
    /// A bare repository standing in for GitHub.
    remote: PathBuf,
    me: String,
    worktree: PathBuf,
    peer: String,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        let ws = base.join("ws");
        let repo = ws.join("app");
        committed_repo(&repo, &[("src/lib.rs", "pub fn one() -> i32 { 1 }\n")]);
        let upstream = base.join("upstream");
        committed_repo(&upstream, &[("README.md", "the other repo\n")]);
        let remote = base.join("other.git");
        git(&base, &["clone", "-q", "--bare", upstream.to_str().unwrap(), remote.to_str().unwrap()]);
        let engine = engine_with_project(&base, &ws, &repo);
        let a = ok(&engine, Actor::User, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}));
        let b = ok(&engine, Actor::User, "session.create", json!({"project_id": 1, "provider": "claude", "role": "builder"}));
        Self {
            _root: root,
            engine,
            remote,
            me: a["name"].as_str().unwrap().into(),
            worktree: PathBuf::from(a["worktree"].as_str().unwrap()),
            peer: b["name"].as_str().unwrap().into(),
        }
    }
    fn agent(&self) -> Actor { Actor::agent(&self.me) }
    fn link(&self, actor: Actor) -> Value {
        ok(&self.engine, actor, "session.repo.add", json!({"session": self.me, "url": self.remote}))
    }
    fn list(&self) -> Vec<Value> {
        ok(&self.engine, self.agent(), "session.repo.list", json!({"session": self.me}))["repos"].as_array().unwrap().clone()
    }
}

fn commit_in(repo: &Path, file: &str) {
    std::fs::write(repo.join(file), "work\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", file]);
}

#[test]
fn an_agent_links_a_repository_into_its_own_checkout_out_of_its_status() {
    let f = Fixture::new();
    let linked = f.link(f.agent());
    assert_eq!(linked["name"], "other");
    let path = PathBuf::from(linked["path"].as_str().unwrap());
    assert_eq!(path, f.worktree.join(".relay/linked/other"));
    assert!(path.join("README.md").exists(), "the clone is checked out");
    assert_eq!(linked["url"], f.remote.to_str().unwrap());
    assert_eq!(linked["branch"], "main");
    assert!(git(&f.worktree, &["status", "--porcelain"]).is_empty(), "the session's own status never sees it");

    let repos = f.list();
    assert_eq!(repos.len(), 1);
    assert!(repos[0].get("unpushed").is_none(), "a fresh clone loses nothing: {}", repos[0]);

    // The brief names it, so a resumed agent still knows.
    let brief = ok(&f.engine, f.agent(), "session.brief", json!({"session": f.me}));
    assert!(brief["compact"].as_str().unwrap().contains(&format!("linked_repos: other at {}", path.display())), "{}", brief["compact"]);

    // One name, one repository.
    let again = refusal(&f.engine, f.agent(), "session.repo.add", json!({"session": f.me, "url": f.remote}));
    assert_eq!(again.code, "session.repo_exists");
    // Under another name it is a second clone.
    ok(&f.engine, f.agent(), "session.repo.add", json!({"session": f.me, "url": f.remote, "name": "other-2"}));
    assert_eq!(f.list().len(), 2);
}

#[test]
fn only_the_session_itself_or_the_user_links_and_only_the_user_removes() {
    let f = Fixture::new();
    let peer = refusal(&f.engine, Actor::agent(&f.peer), "session.repo.add", json!({"session": f.me, "url": f.remote}));
    assert_eq!(peer.code, "actor.scope", "{}", peer.message);
    assert!(!f.worktree.join(".relay/linked/other").exists());

    // The user links one for the agent, and the agent is told where it is.
    f.link(Actor::User);
    let inbox = ok(&f.engine, f.agent(), "mailbox.list", json!({}));
    let told = inbox["messages"].as_array().unwrap().iter()
        .any(|m| m["text"].as_str().unwrap_or("").contains(".relay/linked/other"));
    assert!(told, "the agent learns of a repository linked for it: {inbox}");

    let removing = call(&f.engine, f.agent(), "session.repo.remove", json!({"session": f.me, "name": "other"})).into_result();
    assert!(removing.is_err(), "removal is the user's");
    assert!(f.worktree.join(".relay/linked/other").exists());
}

#[test]
fn bad_urls_and_names_are_refused_before_anything_runs() {
    let f = Fixture::new();
    for (payload, code) in [
        (json!({"session": f.me, "url": "--upload-pack=touch /tmp/x"}), "session.repo_url"),
        (json!({"session": f.me, "url": f.remote, "name": "../escape"}), "session.repo_name"),
        (json!({"session": f.me, "url": f.remote, "name": ".hidden"}), "session.repo_name"),
    ] {
        assert_eq!(refusal(&f.engine, f.agent(), "session.repo.add", payload).code, code);
    }
    let failed = refusal(&f.engine, f.agent(), "session.repo.add", json!({"session": f.me, "url": f.remote.with_file_name("missing.git")}));
    assert_eq!(failed.code, "session.repo_clone_failed");
    assert!(f.list().is_empty(), "a failed clone leaves nothing under the name");
    let leftovers: Vec<_> = std::fs::read_dir(f.worktree.join(".relay/linked")).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "nor a scratch folder: {leftovers:?}");
}

#[test]
fn unpushed_work_holds_the_repository_until_pushed_or_discarded() {
    let f = Fixture::new();
    let path = PathBuf::from(f.link(f.agent())["path"].as_str().unwrap());
    commit_in(&path, "a.txt");
    std::fs::write(path.join("draft.txt"), "not committed\n").unwrap();

    let found = &f.list()[0]["unpushed"];
    assert_eq!(found["commits"], 1, "{found}");
    assert_eq!(found["branches"], json!(["main"]));
    assert_eq!(found["changed"], 1);

    let refused = refusal(&f.engine, Actor::User, "session.repo.remove", json!({"session": f.me, "name": "other"}));
    assert_eq!(refused.code, "session.repo_unpushed");
    assert!(refused.message.contains("1 unpushed commit (on main)") && refused.message.contains("1 uncommitted change"), "{}", refused.message);
    assert!(path.exists());

    // Closing the session would delete the checkout, and the clone with it: refused the same way.
    let close = refusal(&f.engine, Actor::User, "session.close", json!({"session": f.me}));
    assert_eq!(close.code, "worktree.dirty");
    assert!(close.message.contains("linked repo other"), "{}", close.message);
    assert_eq!(close.details.as_ref().unwrap()["worktrees"][0]["linked"][0]["name"], "other");
    assert!(path.exists());

    // Committed and pushed, nothing is lost.
    std::fs::remove_file(path.join("draft.txt")).unwrap();
    git(&path, &["push", "-q", "origin", "main"]);
    assert!(f.list()[0].get("unpushed").is_none());
    let removed = ok(&f.engine, Actor::User, "session.repo.remove", json!({"session": f.me, "name": "other"}));
    assert!(removed["freed_mb"].as_f64().unwrap() > 0.0);
    assert!(!path.exists());
    assert!(f.list().is_empty());

    // Stashes and a branch never pushed count; discard_changes deletes them anyway.
    let path = PathBuf::from(f.link(f.agent())["path"].as_str().unwrap());
    git(&path, &["checkout", "-q", "-b", "local-only"]);
    commit_in(&path, "b.txt");
    std::fs::write(path.join("README.md"), "edited\n").unwrap();
    git(&path, &["stash", "-q"]);
    let found = &f.list()[0]["unpushed"];
    assert_eq!(found["stashes"], 1, "{found}");
    assert_eq!(found["branches"], json!(["local-only"]));
    assert_eq!(refusal(&f.engine, Actor::User, "session.repo.remove", json!({"session": f.me, "name": "other"})).code, "session.repo_unpushed");
    ok(&f.engine, Actor::User, "session.repo.remove", json!({"session": f.me, "name": "other", "discard_changes": true}));
    assert!(!path.exists());
    assert_eq!(refusal(&f.engine, Actor::User, "session.repo.remove", json!({"session": f.me, "name": "other"})).code, "session.repo_not_found");
}

#[test]
fn a_clean_linked_repository_goes_with_the_session_s_checkout() {
    let f = Fixture::new();
    f.link(f.agent());
    ok(&f.engine, Actor::User, "session.close", json!({"session": f.me}));
    assert!(!f.worktree.exists(), "the checkout and the clone in it are gone");
}
