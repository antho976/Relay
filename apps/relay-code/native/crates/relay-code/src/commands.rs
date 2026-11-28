//! The commands the chat's `/` and `!` list offers (backlog D3), fitted to the repo: Claude
//! Code's own commands, then the repo's (`.claude/commands`) and its enabled skills (`skill.list`);
//! for `!`, the shell commands you last sent this agent, then the ones its kind of repo runs
//! (`cargo test` only where there is a `Cargo.toml`, as the PC's file tree says), then git's.
//!
//! The chat draws the list ([`list`]); this keeps what the PC said, asking once per repo through
//! `Link::call` and polling, so calling it every frame the list shows costs only a filter.

use std::collections::HashMap;

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::App;

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub text: String,
    pub what: String,
}

/// The most a list shows.
pub const SHOWN: usize = 8;
/// The `!` commands kept per agent.
const KEPT: usize = 6;

/// Claude Code's own commands used most: first, before the repo's.
const BUILT_IN: &[(&str, &str)] = &[("/compact", "Free memory, keep the gist"), ("/clear", "Start fresh"), ("/model", "Change model"), ("/review", "Review the changes")];
/// The rest of Claude Code's own: after the repo's commands and skills.
const BUILT_IN_MORE: &[(&str, &str)] = &[
    ("/context", "What fills its memory"),
    ("/cost", "What this conversation cost"),
    ("/effort", "Change effort"),
    ("/init", "Write a CLAUDE.md for the repo"),
    ("/memory", "Edit its memory files"),
    ("/status", "Version, model and account"),
];

/// Every repo has git.
const GIT: &[(&str, &str)] = &[("!git status", "What changed"), ("!git diff --stat", "Changed lines per file"), ("!git log --oneline -10", "The last ten commits")];

/// A file at the repo's root, and what it says the repo runs.
const KINDS: &[(&str, &[(&str, &str)])] = &[
    ("Cargo.toml", &[("!cargo test", "Run the tests"), ("!cargo clippy --all-targets -- -D warnings", "Lints"), ("!cargo build", "Build it")]),
    ("package.json", &[("!npm test", "Run the tests"), ("!npm run build", "Build it"), ("!npm run lint", "Lints")]),
    ("gradlew", &[("!./gradlew test", "Run the tests"), ("!./gradlew assembleDebug", "Build the debug app")]),
    ("go.mod", &[("!go test ./...", "Run the tests"), ("!go vet ./...", "Lints")]),
    ("pyproject.toml", &[("!pytest", "Run the tests")]),
    ("requirements.txt", &[("!pytest", "Run the tests")]),
    ("Makefile", &[("!make", "Build it"), ("!make test", "Run the tests")]),
];

#[derive(Default)]
pub struct State {
    /// The repo what follows was read for.
    repo: Option<i64>,
    /// The names at the repo's root (`Cargo.toml`, `package.json`, ...).
    root: Vec<String>,
    /// The repo's own commands and its enabled skills, as `/name`.
    own: Vec<Command>,
    skills: Vec<Command>,
    calls: Vec<(Part, Call)>,
    /// The `!` commands sent to each agent, newest first, and whether the phone's copy was read.
    history: HashMap<String, Vec<String>>,
    loaded: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Root,
    Own,
    Skills,
}

fn history_file(app: &App) -> std::path::PathBuf {
    app.data.join("cache").join("commands.json")
}

/// A `!` command was sent to `agent`: it heads that agent's list next time.
pub fn remember(app: &mut App, agent: &str, text: &str) {
    let text = text.trim();
    if !text.starts_with('!') || text.len() < 2 || text.contains('\n') {
        return;
    }
    load(app);
    let h = app.st.commands.history.entry(agent.to_string()).or_default();
    h.retain(|c| c != text);
    h.insert(0, text.to_string());
    h.truncate(KEPT);
    let file = history_file(app);
    crate::app::write_json(file, &app.st.commands.history);
}

fn load(app: &mut App) {
    if !app.st.commands.loaded {
        app.st.commands.loaded = true;
        // A few hundred bytes, read once.
        if let Some(h) = crate::app::read_json::<HashMap<String, Vec<String>>>(history_file(app)) {
            app.st.commands.history = h;
        }
    }
}

/// Ask the PC about this repo once, and take its answers as they come.
fn keep_up(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if app.st.commands.repo != Some(repo) && app.online() {
        let s = &mut app.st.commands;
        s.repo = Some(repo);
        s.root.clear();
        s.own.clear();
        s.skills.clear();
        s.calls = vec![
            (Part::Root, app.link.call("file.tree", json!({"project_id": repo, "depth": 1, "git_badges": false}))),
            (Part::Own, app.link.call("file.tree", json!({"project_id": repo, "path": ".claude/commands", "depth": 1, "git_badges": false}))),
            (Part::Skills, app.link.call("skill.list", json!({"project_id": repo, "enabled": true, "summary": true}))),
        ];
    }
    let mut done = Vec::new();
    app.st.commands.calls.retain(|(part, call)| match call.poll() {
        Some(r) => {
            done.push((*part, r));
            false
        }
        None => true,
    });
    for (part, r) in done {
        // A repo with no `.claude/commands` answers with an error: it has none.
        let Ok(v) = r else { continue };
        let s = &mut app.st.commands;
        match part {
            Part::Root => s.root = names(&v),
            Part::Own => s.own = own(&v),
            Part::Skills => s.skills = skills(&v),
        }
    }
}

/// The names at the top of a `file.tree` answer.
fn names(v: &Value) -> Vec<String> {
    v.get("entries")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|e| e.get("path").and_then(Value::as_str)).filter(|p| !p.contains('/')).map(str::to_string).collect())
        .unwrap_or_default()
}

/// `.claude/commands/fix-issue.md` is `/fix-issue`.
fn own(v: &Value) -> Vec<Command> {
    v.get("entries")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| e.get("name").and_then(Value::as_str))
                .filter_map(|n| n.strip_suffix(".md"))
                .map(|n| Command { text: format!("/{n}"), what: "This repo's command".into() })
                .collect()
        })
        .unwrap_or_default()
}

fn skills(v: &Value) -> Vec<Command> {
    v.get("skills")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let name = s.get("name")?.as_str()?.trim();
                    if name.is_empty() || name.contains(char::is_whitespace) {
                        return None;
                    }
                    let what = s.get("description").and_then(Value::as_str).unwrap_or("A skill").lines().next().unwrap_or_default().to_string();
                    Some(Command { text: format!("/{name}"), what })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Everything this agent's list may hold for `prefix` (`/` or `!`), in order, each once.
fn all(s: &State, agent: &str, prefix: char) -> Vec<Command> {
    let c = |(t, w): &(&str, &str)| Command { text: t.to_string(), what: w.to_string() };
    let mut out: Vec<Command> = Vec::new();
    if prefix == '/' {
        out.extend(BUILT_IN.iter().map(c));
        out.extend(s.own.iter().cloned());
        out.extend(s.skills.iter().cloned());
        out.extend(BUILT_IN_MORE.iter().map(c));
    } else {
        out.extend(s.history.get(agent).into_iter().flatten().map(|t| Command { text: t.clone(), what: "You ran this".into() }));
        for (file, cmds) in KINDS {
            if s.root.iter().any(|r| r == file) {
                out.extend(cmds.iter().map(c));
            }
        }
        out.extend(GIT.iter().map(c));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|x| seen.insert(x.text.clone()));
    out
}

/// What the list shows for `typed` in `agent`'s message box: the commands that start with it
/// (then those that hold it), at most [`SHOWN`]; empty unless it starts with `/` or `!`.
pub fn list(app: &mut App, agent: &str, typed: &str) -> Vec<Command> {
    let Some(prefix) = typed.chars().next().filter(|c| *c == '/' || *c == '!') else { return Vec::new() };
    load(app);
    keep_up(app);
    pick(all(&app.st.commands, agent, prefix), typed)
}

fn pick(all: Vec<Command>, typed: &str) -> Vec<Command> {
    let typed = typed.trim_end().to_lowercase();
    if typed.chars().count() <= 1 {
        return all.into_iter().take(SHOWN).collect();
    }
    let (mut first, mut then): (Vec<Command>, Vec<Command>) = (Vec::new(), Vec::new());
    for c in all {
        let t = c.text.to_lowercase();
        if t.starts_with(&typed) {
            first.push(c);
        } else if t.contains(&typed[1..]) {
            then.push(c);
        }
    }
    first.extend(then);
    // Typed in full, a command has nothing left to offer: the list goes.
    if first.len() == 1 && first[0].text.to_lowercase() == typed {
        return Vec::new();
    }
    first.truncate(SHOWN);
    first
}

/// What a PC might say about RelayV4, for screenshots.
pub fn sample(app: &mut App) {
    let s = &mut app.st.commands;
    s.repo = app.repo;
    s.loaded = true;
    s.root = ["Cargo.toml", "apps", "crates", "README.md"].map(String::from).to_vec();
    s.own = own(&json!({"entries": [{"name": "ship-notes.md"}]}));
    s.skills = skills(&json!({"skills": [{"name": "house-style", "description": "The owner's design taste and fundamentals for any interface"}, {"name": "perf-rust", "description": "Rust performance"}]}));
    s.history.insert("bold-puffin".into(), vec!["!cargo test -p rc-link".into()]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(root: &[&str]) -> State {
        State { root: root.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    fn texts(v: &[Command]) -> Vec<&str> {
        v.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn no_cargo_without_a_cargo_toml() {
        let none = all(&state(&["README.md", "src"]), "a", '!');
        assert!(texts(&none).iter().all(|t| !t.contains("cargo")), "{:?}", texts(&none));
        assert!(texts(&none).contains(&"!git status"));
        let rust = all(&state(&["Cargo.toml", "src"]), "a", '!');
        assert_eq!(texts(&rust)[0], "!cargo test");
        let web = all(&state(&["package.json"]), "a", '!');
        assert_eq!(texts(&web)[0], "!npm test");
    }

    #[test]
    fn your_own_bang_commands_come_first_once_each() {
        let mut s = state(&["Cargo.toml"]);
        s.history.insert("a".into(), vec!["!cargo test -p rc-link".into(), "!git status".into()]);
        let l = all(&s, "a", '!');
        assert_eq!(texts(&l)[..3], ["!cargo test -p rc-link", "!git status", "!cargo test"]);
        assert_eq!(texts(&l).iter().filter(|t| **t == "!git status").count(), 1);
        // Another agent's history is not this one's.
        assert_eq!(texts(&all(&s, "b", '!'))[0], "!cargo test");
    }

    #[test]
    fn slash_lists_built_ins_then_the_repos_commands_and_skills() {
        let mut s = state(&[]);
        s.own = own(&json!({"entries": [{"name": "fix-issue.md", "path": ".claude/commands/fix-issue.md"}, {"name": "notes.txt"}]}));
        s.skills = skills(&json!({"skills": [{"name": "house-style", "description": "The owner's taste\nmore"}, {"name": "two words"}]}));
        let l = all(&s, "a", '/');
        // The most used built-ins, then the repo's own and its skills within the first eight.
        assert_eq!(texts(&l)[..6], ["/compact", "/clear", "/model", "/review", "/fix-issue", "/house-style"]);
        let hs = l.iter().find(|c| c.text == "/house-style").expect("the skill");
        assert_eq!(hs.what, "The owner's taste");
        assert!(!texts(&l).iter().any(|t| t.contains(' ')));
    }

    #[test]
    fn typing_narrows_starts_first_then_holds_and_a_full_match_closes() {
        let l = all(&state(&["Cargo.toml"]), "a", '!');
        assert_eq!(texts(&pick(l.clone(), "!c")), ["!cargo test", "!cargo clippy --all-targets -- -D warnings", "!cargo build"]);
        // "!stat" starts nothing, but git status holds it.
        assert_eq!(texts(&pick(l.clone(), "!stat")), ["!git status", "!git diff --stat"]);
        assert!(pick(l.clone(), "!cargo build").is_empty());
        assert_eq!(pick(l, "!").len(), SHOWN.min(6));
        let s = all(&state(&[]), "a", '/');
        assert_eq!(texts(&pick(s, "/co")), ["/compact", "/context", "/cost"]);
    }

    #[test]
    fn the_root_names_are_the_top_entries_only() {
        let v = json!({"entries": [{"path": "Cargo.toml"}, {"path": "src"}, {"path": "src/main.rs"}]});
        assert_eq!(names(&v), ["Cargo.toml", "src"]);
    }
}
