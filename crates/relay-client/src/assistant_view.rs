//! The Dev space's assistant's reading of its replies (docs/THREADS.md, "The assistant"): a reply
//! cut into its Markdown and the blocks the drawer draws itself — live cards for tasks, sessions
//! and notes, and `open`, a page of the app to go to — and plain words for each tool it called.

use serde_json::Value;

/// The most cards one block shows.
pub const MAX_CARDS: usize = 6;

/// A part of a reply, in the order written.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    /// Markdown, for Threads' own drawing.
    Text(String),
    /// Cards of one kind (`task`, `session` or `note`), each by its id or name as written.
    Cards { kind: &'static str, items: Vec<String> },
    /// A page to open, as its line was written (`target` reads it).
    Open(String),
}

/// The kinds of card a fenced block may ask for, by its language.
const CARDS: [&str; 3] = ["task", "session", "note"];

/// `text` cut at its `task`, `session`, `note` and `open` blocks; every other fence, and an
/// unclosed one at the end of a reply still being written, stays Markdown.
pub fn parts(text: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut prose = String::new();
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        let lang = line.trim().strip_prefix("```").map(|l| l.trim().to_ascii_lowercase());
        let kind = lang.as_deref().and_then(|l| CARDS.iter().copied().find(|k| *k == l).or((l == "open").then_some("open")));
        let Some(kind) = kind else {
            prose.push_str(line);
            prose.push('\n');
            continue;
        };
        let mut body = Vec::new();
        let mut closed = false;
        for inner in lines.by_ref() {
            if inner.trim().starts_with("```") {
                closed = true;
                break;
            }
            body.push(inner);
        }
        if !closed {
            prose.push_str(line);
            prose.push('\n');
            for inner in body {
                prose.push_str(inner);
                prose.push('\n');
            }
            continue;
        }
        if !prose.trim().is_empty() {
            parts.push(Part::Text(std::mem::take(&mut prose)));
        }
        prose.clear();
        if kind == "open" {
            if let Some(first) = body.iter().map(|l| l.trim()).find(|l| !l.is_empty()) {
                parts.push(Part::Open(first.to_string()));
            }
        } else {
            // Tasks and notes by their number, a task's board key (`REL-12`) read as its number.
            let items: Vec<String> = body.iter()
                .flat_map(|l| l.split(|c: char| c.is_whitespace() || c == ','))
                .map(|w| w.trim_matches(|c: char| matches!(c, '#' | '.' | '`')))
                .filter_map(|w| if kind == "session" { Some(w.to_string()) } else { number(w).map(|n| n.to_string()) })
                .filter(|w| !w.is_empty())
                .take(MAX_CARDS)
                .collect();
            if !items.is_empty() {
                parts.push(Part::Cards { kind, items });
            }
        }
    }
    if !prose.trim().is_empty() {
        parts.push(Part::Text(prose.trim_end_matches('\n').to_string()));
    }
    parts
}

/// A task's or a note's number as written: `12`, `#12`, or a board key such as `REL-12`.
pub fn number(word: &str) -> Option<i64> {
    let word = word.trim_start_matches('#');
    word.parse().ok().or_else(|| {
        let (prefix, n) = word.rsplit_once('-')?;
        prefix.chars().all(|c| c.is_ascii_alphanumeric()).then(|| n.parse().ok()).flatten()
    }).filter(|n| *n > 0)
}

/// The pages an `open` block may name: (page, words for it). `code` is the Files view.
pub const PAGES: [(&str, &str); 13] = [
    ("agents", "Agents"),
    ("code", "Files"),
    ("design", "Design"),
    ("board", "the board"),
    ("modules", "Modules"),
    ("mailbox", "Mail"),
    ("guardrails", "Guardrails"),
    ("notes", "Notes"),
    ("skills", "Skills"),
    ("plugins", "Plugins"),
    ("devices", "Devices"),
    ("settings", "Settings"),
    ("notifications", "notifications"),
];

/// Where an `open` block goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A page, in the project named or the one shown.
    Page { page: &'static str, project: Option<i64> },
    Task(i64),
    Session(String),
    Note(i64),
    Project(i64),
}

/// An `open` block's line: `board`, `board 3`, `board in project 3`, `task 12`, `session
/// calm-otter`, `note 3` or `project 3`. Words a person or a small model adds (`the`, `#`, a
/// full stop) are let pass; anything else is `None`.
pub fn target(line: &str) -> Option<Target> {
    let words: Vec<String> = line.split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, '#' | ',' | '.' | '`' | '"')).to_ascii_lowercase())
        .filter(|w| !w.is_empty() && !matches!(w.as_str(), "the" | "in" | "of" | "page"))
        .collect();
    let at = |n: usize| words.get(n).and_then(|w| number(w));
    match words.first()?.as_str() {
        "task" => at(1).map(Target::Task),
        "note" => at(1).map(Target::Note),
        "project" => at(1).map(Target::Project),
        "session" | "terminal" => words.get(1).filter(|name| !name.is_empty()).map(|name| Target::Session(name.clone())),
        word => {
            let page = match word {
                "files" | "code" | "editor" => "code",
                "mail" | "mailbox" | "inbox" => "mailbox",
                "sessions" | "terminals" | "agents" | "wall" => "agents",
                "bell" | "notifications" => "notifications",
                "guardrail" | "guardrails" | "holds" => "guardrails",
                "tasks" | "board" => "board",
                "designs" | "design" => "design",
                other => PAGES.iter().map(|(page, _)| *page).find(|page| *page == other)?,
            };
            let project = words.iter().skip(1).find_map(|w| w.parse::<i64>().ok());
            Some(Target::Page { page, project })
        }
    }
}

/// Words for where `target` goes, after "Open" or "Opened": "the board", "task #12".
pub fn target_words(target: &Target) -> String {
    match target {
        Target::Page { page, project } => {
            let words = PAGES.iter().find(|(p, _)| p == page).map_or(*page, |(_, w)| *w);
            match project {
                Some(project) => format!("{words} of project {project}"),
                None => words.to_string(),
            }
        }
        Target::Task(id) => format!("task #{id}"),
        Target::Session(name) => name.clone(),
        Target::Note(id) => format!("note {id}"),
        Target::Project(id) => format!("project {id}"),
    }
}

/// A board column's name.
pub fn column_words(column: &str) -> &str {
    match column {
        "backlog" => "Backlog",
        "ready" => "Ready",
        "active" => "Active",
        "in_review" => "In review",
        "done" => "Done",
        other => other,
    }
}

/// What a tool call did, in words, from its op and what it was called with: "Moved #12 to
/// Ready", "Wrote to calm-otter".
pub fn caption(op: &str, input: &Value) -> String {
    let text = |key: &str| input.get(key).and_then(Value::as_str).map(|s| cut(s, 48)).filter(|s| !s.is_empty());
    let id = |key: &str| input.get(key).and_then(Value::as_i64);
    let task = || id("task_id").map_or_else(|| "a task".to_string(), |id| format!("#{id}"));
    let session = || text("session").unwrap_or_else(|| "a session".into());
    match op {
        "dashboard.get" => "Read the overview".into(),
        "app.status" | "app.version" => "Read Relay's status".into(),
        "session.list" | "session.peers" => "Read sessions".into(),
        "session.get" => format!("Read {}", session()),
        "session.transcript" => format!("Read {}'s conversation", session()),
        "session.scrollback" => format!("Read {}'s terminal", session()),
        "session.restorable" => "Read restorable sessions".into(),
        "task.list" | "task.children" | "task.label.list" => match input.get("column").and_then(Value::as_str) {
            Some(column) => format!("Read {}", column_words(column)),
            None => "Read the board".into(),
        },
        "task.get" => format!("Read {}", task()),
        "task.create" => match text("title") {
            Some(title) => format!("Created the task \u{201c}{title}\u{201d}"),
            None => "Created a task".into(),
        },
        "task.update" => format!("Edited {}", task()),
        "task.move" => match input.get("column").and_then(Value::as_str) {
            Some(column) => format!("Moved {} to {}", task(), column_words(column)),
            None => format!("Moved {}", task()),
        },
        "task.comment" => format!("Commented on {}", task()),
        "module.list" | "module.get" => "Read modules".into(),
        "project.list" | "project.get" | "project.stats" => "Read projects".into(),
        "workspace.list" | "workspace.discover" => "Read workspaces".into(),
        "mailbox.list" | "mailbox.outbox" => "Read mail".into(),
        "mailbox.send" => match text("to").as_deref() {
            Some("*") => "Wrote to every session".into(),
            Some(to) => format!("Wrote to {to}"),
            None => "Sent mail".into(),
        },
        "notes.list" | "notes.get" | "notes.standing" => "Read notes".into(),
        "notes.append" => "Added to a note".into(),
        "notes.create" => match text("title") {
            Some(title) => format!("Created the note \u{201c}{title}\u{201d}"),
            None => "Created a note".into(),
        },
        "notes.update" => "Edited a note".into(),
        "skill.list" | "skill.get" => "Read skills".into(),
        "skill.enable" => format!("Turned {} a skill", on_off(input)),
        "plugin.list" | "plugin.get" => "Read plugins".into(),
        "plugin.enable" => match text("plugin_id") {
            Some(plugin) => format!("Turned {} the {plugin} plugin", on_off(input)),
            None => format!("Turned {} a plugin", on_off(input)),
        },
        "guardrail.holds.list" | "guardrail.requests.list" => "Read what waits on you".into(),
        "notify.list" => "Read notifications".into(),
        "usage.get" => "Read plan usage".into(),
        "provider.list" => "Read providers".into(),
        "git.status" | "git.log" | "git.branches" | "git.pr.list" => "Read Git".into(),
        "worktree.list" | "worktree.disk" | "worktree.drift" => "Read worktrees".into(),
        "overlap.list" => "Read overlapping work".into(),
        "design.list" => "Read designs".into(),
        "video.list" => "Read videos".into(),
        "device.list" => "Read devices".into(),
        "file.search" => "Searched files".into(),
        "bus.schema" => "Checked how a tool works".into(),
        "thread.ask" => "Asked you".into(),
        "" => "Used a tool".into(),
        other => format!("Used {other}"),
    }
}

/// A finished call's words once its answer names what it acted on, where the call itself did
/// not: the skill switched, the task made. `None` keeps [`caption`].
pub fn done_caption(op: &str, input: &Value, answer: &Value) -> Option<String> {
    let name = answer.get("name").or_else(|| answer.get("title")).and_then(Value::as_str).map(|s| cut(s, 48));
    match op {
        "skill.enable" => Some(format!("Turned {} the {} skill", on_off(input), name?)),
        "plugin.enable" => Some(format!("Turned {} the {} plugin", on_off(input), name?)),
        "task.create" => Some(format!("Created #{} \u{201c}{}\u{201d}", answer.get("id")?.as_i64()?, name?)),
        _ => None,
    }
}

/// Whether a call writes, for its line's mark.
pub fn writes(op: &str) -> bool {
    matches!(op, "mailbox.send" | "notes.append" | "notes.create" | "notes.update" | "task.comment" | "task.create" | "task.update"
        | "task.move" | "skill.enable" | "plugin.enable")
}

fn on_off(input: &Value) -> &'static str {
    if input.get("enabled") == Some(&Value::Bool(false)) { "off" } else { "on" }
}

/// `text` cut to `max` characters at most, with an ellipsis when cut.
fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}\u{2026}", text[..at].trim_end()),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reply_is_cut_at_its_cards_and_open_blocks_and_keeps_every_other_fence() {
        let text = "There is one:\n\n```task\n#12, 14\n```\nOpening the board.\n```open\n  board 3\n```\n```rust\nfn main() {}\n```";
        assert_eq!(parts(text), [
            Part::Text("There is one:\n\n".into()),
            Part::Cards { kind: "task", items: vec!["12".into(), "14".into()] },
            Part::Text("Opening the board.\n".into()),
            Part::Open("board 3".into()),
            Part::Text("```rust\nfn main() {}\n```".into()),
        ]);
        assert_eq!(parts("```session\ncalm-otter brisk-otter\n```"), [Part::Cards { kind: "session", items: vec!["calm-otter".into(), "brisk-otter".into()] }]);
        assert_eq!(parts("Plain words"), [Part::Text("Plain words".into())]);
        assert_eq!(parts("```task\nREL-12 #3 soon\n```"), [Part::Cards { kind: "task", items: vec!["12".into(), "3".into()] }]);
        assert_eq!((number("REL-12"), number("#4"), number("calm-otter"), number("0")), (Some(12), Some(4), None, None));
        // A block still being written stays words until it closes; an empty one shows nothing.
        assert_eq!(parts("Here:\n```task\n12"), [Part::Text("Here:\n```task\n12".into())]);
        assert_eq!(parts("```note\n\n```"), []);
        let many = format!("```task\n{}\n```", (1..=9).map(|n| n.to_string()).collect::<Vec<_>>().join(" "));
        assert!(matches!(&parts(&many)[0], Part::Cards { items, .. } if items.len() == MAX_CARDS));
    }

    #[test]
    fn an_open_line_names_a_page_or_one_thing() {
        assert_eq!(target("board"), Some(Target::Page { page: "board", project: None }));
        assert_eq!(target("The board."), Some(Target::Page { page: "board", project: None }));
        assert_eq!(target("board in project 3"), Some(Target::Page { page: "board", project: Some(3) }));
        assert_eq!(target("files 2"), Some(Target::Page { page: "code", project: Some(2) }));
        assert_eq!(target("mail"), Some(Target::Page { page: "mailbox", project: None }));
        assert_eq!(target("settings"), Some(Target::Page { page: "settings", project: None }));
        assert_eq!(target("task #12"), Some(Target::Task(12)));
        assert_eq!(target("task REL-12"), Some(Target::Task(12)));
        assert_eq!(target("session calm-otter"), Some(Target::Session("calm-otter".into())));
        assert_eq!(target("note 3"), Some(Target::Note(3)));
        assert_eq!(target("project 4"), Some(Target::Project(4)));
        for nothing in ["", "task", "task twelve", "session", "rm -rf /", "https://example.com"] {
            assert_eq!(target(nothing), None, "{nothing}");
        }
        assert_eq!(target_words(&target("board 3").unwrap()), "the board of project 3");
        assert_eq!(target_words(&Target::Task(12)), "task #12");
    }

    #[test]
    fn a_tool_line_says_what_it_did_to_what() {
        assert_eq!(caption("task.move", &json!({"task_id": 12, "column": "in_review"})), "Moved #12 to In review");
        assert_eq!(caption("mailbox.send", &json!({"to": "calm-otter", "text": "hi"})), "Wrote to calm-otter");
        assert_eq!(caption("mailbox.send", &json!({"to": "*"})), "Wrote to every session");
        assert_eq!(caption("session.transcript", &json!({"session": "calm-otter"})), "Read calm-otter's conversation");
        assert_eq!(caption("skill.enable", &json!({"skill_id": 4, "enabled": false})), "Turned off a skill");
        assert_eq!(caption("task.list", &json!({})), "Read the board");
        assert_eq!(caption("dashboard.get", &json!({})), "Read the overview");
        assert_eq!(done_caption("skill.enable", &json!({"enabled": true}), &json!({"name": "audit"})).as_deref(), Some("Turned on the audit skill"));
        assert_eq!(done_caption("task.create", &json!({}), &json!({"id": 14, "title": "Fix login"})).as_deref(), Some("Created #14 \u{201c}Fix login\u{201d}"));
        assert_eq!(done_caption("task.get", &json!({}), &json!({"id": 1})), None);
        let long = "x".repeat(80);
        assert_eq!(caption("task.create", &json!({"title": long})).chars().count(), "Created the task \u{201c}\u{201d}".chars().count() + 49);
        assert!(writes("task.create") && !writes("task.list") && !writes("thread.ask"));
    }
}
