//! What the PC said, as the screens read it: the projects (repos), the agents, and each agent's
//! conversation turned into what the chat draws. Kept on the phone (`cache/`), so the app opens on
//! the last conversation at once and catches up after (design 44, D3). Pure rules, tested here.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub base_branch: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub project_id: i64,
    pub provider: String,
    pub role: String,
    pub state: String,
    pub intent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub branch: String,
    pub task_id: Option<i64>,
    pub last_output_at: Option<String>,
    pub updated_at: String,
}

/// How long without terminal output before a running agent reads as waiting, not working.
pub const BUSY_QUIET_MS: u64 = 8_000;

impl Agent {
    /// Held at a question or a held command: what goes under "Needs you".
    pub fn needs_you(&self) -> bool {
        self.state == "blocked"
    }

    /// At work now: running, and its terminal written to in the last few seconds. Claude Code
    /// redraws its spinner every second while it works and nothing while it waits at the prompt,
    /// and Relay's "running" also covers that wait.
    pub fn working(&self) -> bool {
        self.working_at(rc_link::link::now_ms())
    }

    pub fn working_at(&self, now_ms: u64) -> bool {
        match self.state.as_str() {
            "spawning" => true,
            "running" => self.last_output_at.as_deref().and_then(parse_ms).is_none_or(|t| now_ms.saturating_sub(t) < BUSY_QUIET_MS),
            _ => false,
        }
    }

    /// Still there to talk to (closed and exited ones leave the lists).
    pub fn alive(&self) -> bool {
        !matches!(self.state.as_str(), "closed" | "exited")
    }

    /// The line a person reads it by: what it says it is doing, else its name.
    pub fn title(&self) -> &str {
        self.intent.as_deref().filter(|i| !i.trim().is_empty()).unwrap_or(&self.name)
    }

    /// Its state in words.
    pub fn state_words(&self) -> &'static str {
        match self.state.as_str() {
            "blocked" => "asks you",
            "spawning" => "starting",
            "running" if self.working() => "working",
            "running" => "ready",
            "idle" => "ready",
            "parked" => "parked",
            "restorable" => "stopped",
            "created" => "starting",
            "closed" | "exited" => "closed",
            _ => "done",
        }
    }

    /// The badge's two letters: "umber-zebra" is UZ.
    pub fn initials(&self) -> String {
        let mut it = self.name.split(['-', '_', ' ']).filter(|p| !p.is_empty());
        let a = it.next().and_then(|p| p.chars().next());
        let b = it.next().and_then(|p| p.chars().next()).or_else(|| self.name.chars().nth(1));
        [a, b].into_iter().flatten().collect::<String>().to_uppercase()
    }

    /// The badge's colour, picked from the name so it stays the same everywhere.
    pub fn colour(&self) -> usize {
        self.name.bytes().fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % crate::theme::BADGES.len()
    }
}

pub fn projects(v: &Value) -> Vec<Project> {
    v.get("projects")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some(Project {
                        id: p.get("id")?.as_i64()?,
                        name: p.get("name")?.as_str()?.to_string(),
                        base_branch: p.get("base_branch").and_then(Value::as_str).unwrap_or("main").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn agents(v: &Value) -> Vec<Agent> {
    let str_of = |s: &Value, k: &str| s.get(k).and_then(Value::as_str).map(str::to_string);
    let mut out: Vec<Agent> = v
        .get("sessions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(Agent {
                        name: str_of(s, "name")?,
                        project_id: s.get("project_id")?.as_i64()?,
                        provider: str_of(s, "provider").unwrap_or_default(),
                        role: str_of(s, "role").unwrap_or_default(),
                        state: str_of(s, "state").unwrap_or_default(),
                        intent: str_of(s, "intent"),
                        model: str_of(s, "model"),
                        effort: str_of(s, "effort"),
                        branch: str_of(s, "branch").unwrap_or_default(),
                        task_id: s.get("task_id").and_then(Value::as_i64),
                        last_output_at: str_of(s, "last_output_at"),
                        updated_at: str_of(s, "updated_at").unwrap_or_default(),
                    })
                })
                .filter(|a| a.provider != "shell")
                .collect()
        })
        .unwrap_or_default();
    // What needs you first, then the newest.
    out.sort_by(|a, b| b.needs_you().cmp(&a.needs_you()).then_with(|| b.last_said().cmp(a.last_said())));
    out
}

impl Agent {
    fn last_said(&self) -> &str {
        self.last_output_at.as_deref().unwrap_or(&self.updated_at)
    }
}

/// "now", "6m", "3h", "2d" from an RFC 3339 time, against `now_ms`.
pub fn ago(rfc3339: &str, now_ms: u64) -> String {
    let Some(t) = parse_ms(rfc3339) else { return String::new() };
    let s = now_ms.saturating_sub(t) / 1000;
    match s {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

/// Milliseconds since 1970 from `2026-11-25T02:18:39.598Z` (or with an offset).
pub fn parse_ms(t: &str) -> Option<u64> {
    let b = t.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| t.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, s) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    let mut rest = &t[19..];
    let mut ms = 0i64;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(char::is_ascii_digit).collect();
        rest = &frac[digits.len()..];
        ms = format!("{digits:0<3}")[..3].parse().ok()?;
    }
    let offset = match rest.as_bytes().first() {
        Some(b'+') | Some(b'-') if rest.len() >= 6 => {
            let sign = if rest.starts_with('-') { -1 } else { 1 };
            sign * (rest[1..3].parse::<i64>().ok()? * 3600 + rest[4..6].parse::<i64>().ok()? * 60)
        }
        _ => 0,
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + s - offset;
    u64::try_from(secs * 1000 + ms).ok()
}

// The conversation

/// One message of `session.transcript`, as the PC sent it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Msg {
    pub id: String,
    pub role: String,
    pub body: Value,
    pub created_at: String,
}

pub fn messages(v: &Value) -> Vec<Msg> {
    v.get("messages")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect())
        .unwrap_or_default()
}

/// An agent's conversation as the phone keeps it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Convo {
    pub msgs: Vec<Msg>,
    pub cursor: Option<u64>,
    pub found: bool,
}

/// The newest this many messages are kept.
pub const KEEP: usize = 400;

impl Convo {
    /// Take an answer of `session.transcript`: what came after the cursor, or (on `reset`) all of it.
    /// Returns how many of the person's own messages came in, for [`Sending::seen`].
    pub fn take(&mut self, v: &Value) -> usize {
        let got = messages(v);
        let reset = v.get("reset").and_then(Value::as_bool) == Some(true) || self.cursor.is_none();
        self.found = v.get("found").and_then(Value::as_bool).unwrap_or(true);
        if let Some(c) = v.get("cursor").and_then(Value::as_u64) {
            self.cursor = Some(c);
        }
        let users_before: std::collections::HashSet<String> = self.msgs.iter().filter(|m| m.role == "user").map(|m| m.id.clone()).collect();
        if reset {
            self.msgs = got;
        } else {
            for m in got {
                if !self.msgs.iter().any(|o| o.id == m.id) {
                    self.msgs.push(m);
                }
            }
        }
        if self.msgs.len() > KEEP {
            self.msgs.drain(..self.msgs.len() - KEEP);
        }
        if reset {
            return 0;
        }
        self.msgs.iter().filter(|m| m.role == "user" && !users_before.contains(&m.id)).count()
    }
}

/// How long a sent slash command waits for the log before it is let go.
pub const COMMAND_WAIT_MS: u64 = 10_000;

/// What you sent that the agent's log does not hold yet. Matched by count, never by words: each
/// message of yours that the log gains (after you sent) takes the oldest one waiting. Words can be
/// logged otherwise than typed (a queued message, a picture's path), which is how the old app came
/// to draw one message twice (design 44, D6); a count cannot.
///
/// It is also the agent's send queue: a message waits here while the link is down and goes once it
/// is back, one at a time and in order, its words first and Enter only once the PC said it typed
/// them ([`Sending::next`]). Nothing is lost while the phone reconnects.
#[derive(Clone, Debug, Default)]
pub struct Sending {
    pub waiting: Vec<Pending>,
}

/// How far one message has gone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Not sent yet: the link is down, or a message before it is still going.
    #[default]
    Queued,
    /// Its words are on their way.
    Typing,
    /// The PC typed its words; Enter is next.
    Typed,
    /// Enter is on its way.
    Entering,
    /// Typed and entered: waiting for the log to hold it.
    Entered,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub text: String,
    /// When it was written, then when its words went (Enter waits [`ENTER_AFTER_MS`] after them).
    pub sent_ms: u64,
    /// The PC refused it at [`Pending::stage`]: tap to send again from there.
    pub failed: bool,
    pub stage: Stage,
    /// The link dropped under it: it goes again by itself, not before this time.
    pub not_before_ms: u64,
}

/// Enter goes as its own write at least this long after the words, so the program does not take
/// both as one paste.
pub const ENTER_AFTER_MS: u64 = 90;

/// How long a message the link dropped waits before it goes again (the link is back by then, or
/// the PC no longer busy).
pub const RESEND_AFTER_MS: u64 = 1_000;

/// How the PC answered a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Ok,
    /// The link dropped or the PC was busy for a moment (`BusError::transient`): sent again by itself.
    Dropped,
    /// The PC said no: it waits for a tap.
    Refused,
}

/// What the queue wants sent next for one agent.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Type these words.
    Type(String),
    /// Press Enter after the words already typed.
    Enter,
    /// Something is due in this many milliseconds.
    Wait(u64),
}

impl Sending {
    pub fn push(&mut self, text: &str, now_ms: u64) {
        self.waiting.push(Pending { text: text.to_string(), sent_ms: now_ms, failed: false, stage: Stage::Queued, not_before_ms: 0 });
    }

    /// What to send now, if anything; the caller sends it and reports with [`Sending::answered`].
    /// One message at a time, oldest first: nothing goes while a write is out, and a message is
    /// typed only once the one before it was entered. A refused message whose words were typed
    /// holds the ones after it: typed behind those words, one Enter would send both as one.
    pub fn next(&mut self, now_ms: u64) -> Option<Step> {
        if self.waiting.iter().any(|p| !p.failed && matches!(p.stage, Stage::Typing | Stage::Entering)) {
            return None;
        }
        let p = self.waiting.iter_mut().find(|p| (p.failed && p.stage >= Stage::Typed) || (!p.failed && matches!(p.stage, Stage::Queued | Stage::Typed)))?;
        if p.failed {
            return None;
        }
        let due = p.not_before_ms.max(if p.stage == Stage::Typed { p.sent_ms + ENTER_AFTER_MS } else { 0 });
        if now_ms < due {
            return Some(Step::Wait(due - now_ms));
        }
        if p.stage == Stage::Queued {
            p.stage = Stage::Typing;
            p.sent_ms = now_ms;
            Some(Step::Type(p.text.clone()))
        } else {
            p.stage = Stage::Entering;
            Some(Step::Enter)
        }
    }

    /// The PC answered the write that was out: on success the message moves on; when the link
    /// dropped it goes again by itself ([`RESEND_AFTER_MS`] on, while the link is up); when the PC
    /// refused it stops where it was, to be sent again by a tap ([`Sending::retry`]).
    pub fn answered(&mut self, a: Answer, now_ms: u64) {
        let Some(p) = self.waiting.iter_mut().find(|p| !p.failed && matches!(p.stage, Stage::Typing | Stage::Entering)) else { return };
        match (a, p.stage) {
            (Answer::Refused, _) => p.failed = true,
            (Answer::Dropped, s) => {
                // Its words again if they may not have gone (a drop answers before the write
                // left far more often than after), else Enter again.
                p.stage = if s == Stage::Typing { Stage::Queued } else { Stage::Typed };
                p.not_before_ms = now_ms + RESEND_AFTER_MS;
            }
            (Answer::Ok, Stage::Typing) => p.stage = Stage::Typed,
            (Answer::Ok, _) => {
                p.stage = Stage::Entered;
                // Slash commands the log never shows are let go this long after they went in.
                p.sent_ms = now_ms;
            }
        }
    }

    /// Send a failed message again, from where it stopped: its words were typed already when only
    /// Enter failed, and are not typed twice.
    pub fn retry(&mut self, i: usize) {
        if let Some(p) = self.waiting.get_mut(i).filter(|p| p.failed) {
            p.failed = false;
            p.stage = match p.stage {
                Stage::Typing => Stage::Queued,
                Stage::Entering => Stage::Typed,
                s => s,
            };
        }
    }

    /// A slash command is let go after a while: Claude Code logs some of its own commands
    /// (`/model`, `/effort`) only sometimes, and one it never logs must not wait forever.
    pub fn expire_commands(&mut self, now_ms: u64) {
        self.waiting.retain(|p| !(p.text.starts_with('/') && !p.failed && p.stage == Stage::Entered && now_ms.saturating_sub(p.sent_ms) > COMMAND_WAIT_MS));
    }

    /// `n` of your messages came into the log: the oldest entered ones are in it now. One not
    /// entered yet cannot be (a message typed at the PC itself counts, as the old app's did).
    pub fn seen(&mut self, n: usize) {
        let mut left = n;
        self.waiting.retain(|p| {
            if left > 0 && !p.failed && p.stage == Stage::Entered {
                left -= 1;
                false
            } else {
                true
            }
        });
    }
}

/// One tool call: its tool, its one-line summary, its detail (an edit's diff, `-` and `+` lines;
/// a command's whole text; a plan; a to-do list as `[x]`/`[~]`/`[ ]` lines; questions and their
/// choices), and what the tool answered (its head, matched by `tool_use_id`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub detail: Option<String>,
    /// The tool's answer, once it came: its text (the PC sends the head of it).
    pub output: Option<String>,
    pub failed: bool,
}

/// One line of a to-do list: its words and how far along it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    Done,
    Doing,
    Open,
}

/// A to-do list as the PC words it (`[x] Fix it`, `[~] Push`, `[ ] Open the PR`).
pub fn todos(detail: &str) -> Vec<(Todo, String)> {
    detail
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (mark, rest) = (l.get(..3)?, l.get(3..)?);
            let t = match mark {
                "[x]" | "[X]" => Todo::Done,
                "[~]" => Todo::Doing,
                "[ ]" => Todo::Open,
                _ => return None,
            };
            Some((t, rest.trim().to_string()))
        })
        .collect()
}

impl ToolCall {
    /// A command run in a shell: its whole command and its output are shown.
    pub fn is_shell(&self) -> bool {
        matches!(self.name.as_str(), "Bash" | "exec_command" | "shell")
    }

    /// A plan put to you (Claude Code's plan mode).
    pub fn is_plan(&self) -> bool {
        self.name == "ExitPlanMode" && self.detail.as_deref().is_some_and(|d| !d.trim().is_empty())
    }

    /// A to-do list (Claude Code's TodoWrite, Codex's plan).
    pub fn is_todos(&self) -> bool {
        matches!(self.name.as_str(), "TodoWrite" | "update_plan") && self.detail.as_deref().is_some_and(|d| !todos(d).is_empty())
    }

    /// An edit with a diff to show.
    pub fn is_edit(&self) -> bool {
        matches!(self.name.as_str(), "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "apply_patch") && self.detail.as_deref().is_some_and(|d| !d.trim().is_empty())
    }
}

/// A line of the chat, as drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    You(String),
    /// The agent's words (Markdown, drawn simply).
    Said(String),
    /// What it did, folded: "Ran `cargo test`", "Read 3 files, ran 2 commands". Each call keeps
    /// what the PC sent of it: an edit's diff is drawn under the line, never hidden.
    Did { line: String, failed: bool, calls: Vec<ToolCall> },
    Thought(String),
    /// Neither side's words: a compaction, an interruption, what Relay typed in.
    Note(String),
    Error(String),
}

/// The conversation as the chat draws it: tool calls in a row fold into one line.
pub fn items(msgs: &[Msg]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut failed = false;
    let flush = |out: &mut Vec<Item>, calls: &mut Vec<ToolCall>, failed: &mut bool| {
        if !calls.is_empty() {
            let pairs: Vec<(String, String)> = calls.iter().map(|c| (c.name.clone(), c.summary.clone())).collect();
            out.push(Item::Did { line: did_line(&pairs), failed: *failed, calls: std::mem::take(calls) });
            *failed = false;
        }
    };
    for m in msgs {
        let text = |k: &str| m.body.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        match m.role.as_str() {
            "user" => {
                flush(&mut out, &mut calls, &mut failed);
                let t = text("text");
                if !t.trim().is_empty() {
                    out.push(Item::You(t));
                }
            }
            "assistant" => {
                for b in m.body.get("blocks").and_then(Value::as_array).into_iter().flatten() {
                    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    match b.get("type").and_then(Value::as_str) {
                        Some("tool_use") => calls.push(ToolCall { id: s("id"), name: s("name"), summary: s("summary"), detail: b.get("detail").and_then(Value::as_str).map(str::to_string), ..Default::default() }),
                        Some("text") => {
                            flush(&mut out, &mut calls, &mut failed);
                            out.push(Item::Said(s("text")));
                        }
                        Some("thinking") => {
                            flush(&mut out, &mut calls, &mut failed);
                            out.push(Item::Thought(s("text")));
                        }
                        _ => {}
                    }
                }
            }
            "tool" => {
                let is_error = m.body.get("is_error").and_then(Value::as_bool) == Some(true);
                if is_error {
                    failed = true;
                }
                // The answer goes to its call: one still in the row, else one already drawn.
                let id = text("tool_use_id");
                let t = text("text");
                if id.is_empty() {
                    continue;
                }
                let here = calls.iter_mut().rev().find(|c| c.id == id);
                let found = match here {
                    Some(c) => Some(c),
                    None => out.iter_mut().rev().take(8).find_map(|it| match it {
                        Item::Did { calls, failed, .. } => calls.iter_mut().find(|c| c.id == id).inspect(|_| *failed |= is_error),
                        _ => None,
                    }),
                };
                if let Some(c) = found {
                    c.output = Some(t);
                    c.failed = is_error;
                }
            }
            "error" => {
                flush(&mut out, &mut calls, &mut failed);
                out.push(Item::Error(text("text")));
            }
            "note" => {
                flush(&mut out, &mut calls, &mut failed);
                let t = text("text");
                if !t.trim().is_empty() {
                    out.push(Item::Note(t));
                }
            }
            _ => {}
        }
    }
    flush(&mut out, &mut calls, &mut failed);
    out
}

/// One tool call in words ("Ran `cargo test`"), or several counted by kind.
pub fn did_line(calls: &[(String, String)]) -> String {
    let kind = |name: &str| match name {
        "Bash" | "exec_command" | "shell" => "ran",
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "apply_patch" => "edited",
        "Read" => "read",
        "Grep" | "Glob" | "WebSearch" | "WebFetch" => "searched",
        "TodoWrite" | "update_plan" => "planned",
        "Task" | "Agent" => "asked",
        _ => "used",
    };
    if let [(name, summary)] = calls {
        let s = summary.trim();
        return match kind(name) {
            "ran" if !s.is_empty() => format!("Ran `{}`", first_line(s)),
            "edited" if !s.is_empty() => format!("Edited {s}"),
            "read" if !s.is_empty() => format!("Read {s}"),
            "searched" if !s.is_empty() => format!("Searched {s}"),
            "planned" => "Updated the plan".into(),
            "asked" => "Asked a helper agent".into(),
            _ if !s.is_empty() => format!("{name} {s}"),
            _ => format!("Used {name}"),
        };
    }
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for (name, _) in calls {
        let k = kind(name);
        match counts.iter_mut().find(|(c, _)| *c == k) {
            Some(c) => c.1 += 1,
            None => counts.push((k, 1)),
        }
    }
    let words: Vec<String> = counts
        .iter()
        .map(|(k, n)| {
            let one = *n == 1;
            match *k {
                "ran" => format!("ran {n} command{}", if one { "" } else { "s" }),
                "edited" => format!("edited {n} file{}", if one { "" } else { "s" }),
                "read" => format!("read {n} file{}", if one { "" } else { "s" }),
                "searched" => format!("searched {n} time{}", if one { "" } else { "s" }),
                "planned" => "updated the plan".into(),
                "asked" => format!("asked {n} helper{}", if one { "" } else { "s" }),
                _ => format!("used {n} tool{}", if one { "" } else { "s" }),
            }
        })
        .collect();
    let mut line = words.join(", ");
    if let Some(f) = line.get_mut(..1) {
        f.make_ascii_uppercase();
    }
    line
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

/// A message's words in pieces: paragraphs, headings, list items, and code (fenced, or a table)
/// drawn in mono.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Prose(String),
    Heading(String),
    /// A list item: its mark ("•" or "2."), its words, how deep it is nested (0 to 2).
    Item { mark: String, text: String, depth: u8 },
    Code(String),
}

/// Markdown, drawn simply: fenced code and tables apart in mono, headings, bulleted and numbered
/// items with their nesting, paragraphs (lines joined, as markdown reads them), and `**`, `__`,
/// backticks and link targets dropped from the words.
pub fn pieces(md: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut para = String::new();
    let mut code: Option<String> = None;
    let mut table = String::new();
    // An item's words go on across lines until a blank line or another block.
    let mut in_item = false;
    let flush_para = |out: &mut Vec<Piece>, para: &mut String| {
        let t = inline(para.trim());
        if !t.is_empty() {
            out.push(Piece::Prose(t));
        }
        para.clear();
    };
    let flush_table = |out: &mut Vec<Piece>, table: &mut String| {
        if !table.is_empty() {
            out.push(Piece::Code(table.trim_end_matches('\n').to_string()));
            table.clear();
        }
    };
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            match code.take() {
                Some(c) => out.push(Piece::Code(c.trim_end_matches('\n').to_string())),
                None => {
                    flush_para(&mut out, &mut para);
                    flush_table(&mut out, &mut table);
                    in_item = false;
                    code = Some(String::new());
                }
            }
            continue;
        }
        if let Some(c) = &mut code {
            c.push_str(line);
            c.push('\n');
            continue;
        }
        let t = line.trim();
        if t.starts_with('|') {
            flush_para(&mut out, &mut para);
            in_item = false;
            // The line under a table's head (|---|:--|) says only how to align.
            if !t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                table.push_str(&inline(t));
                table.push('\n');
            }
            continue;
        }
        flush_table(&mut out, &mut table);
        if t.is_empty() {
            flush_para(&mut out, &mut para);
            in_item = false;
            continue;
        }
        if let Some(h) = heading(t) {
            flush_para(&mut out, &mut para);
            in_item = false;
            out.push(Piece::Heading(inline(h)));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some((mark, text)) = list_mark(t) {
            flush_para(&mut out, &mut para);
            out.push(Piece::Item { mark, text: inline(text), depth: (indent / 2).min(2) as u8 });
            in_item = true;
            continue;
        }
        if in_item && let Some(Piece::Item { text, .. }) = out.last_mut() {
            text.push(' ');
            text.push_str(&inline(t));
            continue;
        }
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(t);
    }
    if let Some(c) = code {
        out.push(Piece::Code(c.trim_end_matches('\n').to_string()));
    }
    flush_table(&mut out, &mut table);
    flush_para(&mut out, &mut para);
    out
}

/// The links in a message, as (the words shown, where they go): `[words](https://…)`, and bare
/// web addresses (shown as themselves, without a full stop or bracket that ends the sentence).
pub fn links(md: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut rest = md;
    while let Some(open) = rest.find("](") {
        let label_start = rest[..open].rfind('[');
        let after = &rest[open + 2..];
        if let (Some(ls), Some(end)) = (label_start, after.find(')')) {
            let label = rest[ls + 1..open].replace("**", "").replace('`', "");
            let url = after[..end].trim();
            if (url.starts_with("https://") || url.starts_with("http://")) && !label.trim().is_empty() {
                out.push((label.trim().to_string(), url.to_string()));
            }
        }
        rest = &rest[open + 2..];
    }
    for word in md.split(|c: char| c.is_whitespace() || c == '(' || c == '<' || c == '`' || c == '"') {
        let w = word.trim_end_matches(['.', ',', ';', ':', ')', '>', '!', '?', '\'', '`', '"', '*']);
        if (w.starts_with("https://") || w.starts_with("http://")) && w.len() > 8 && !w.contains("](") && !out.iter().any(|(_, u)| u == w) {
            out.push((w.to_string(), w.to_string()));
        }
    }
    out
}

/// `## Words`: the words.
fn heading(t: &str) -> Option<&str> {
    let rest = t.trim_start_matches('#');
    let n = t.len() - rest.len();
    (1..=6).contains(&n).then_some(()).and_then(|_| rest.strip_prefix(' ')).map(str::trim)
}

/// `- words`, `* words`, `+ words`, `3. words`, `3) words`: the mark to draw and the words.
fn list_mark(t: &str) -> Option<(String, &str)> {
    for b in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(b) {
            // `- [ ] task` and `- [x] task`: a box, drawn as words.
            let (mark, rest) = match rest.strip_prefix("[ ] ") {
                Some(r) => ("☐", r),
                None => match rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                    Some(r) => ("✓", r),
                    None => ("•", rest),
                },
            };
            return Some((mark.to_string(), rest.trim()));
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let rest = &t[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((format!("{}.", &t[..digits]), r.trim()));
        }
    }
    None
}

/// A line's words without markdown's marks: `**`, `__`, backticks, and `[words](target)` read as
/// its words.
fn inline(s: &str) -> String {
    let s = s.replace("**", "").replace("__", "").replace('`', "");
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| after[close + 2..].find(')').map(|end| (close, close + 2 + end))) {
            Some((close, end)) if !after[..close].contains('[') => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..open + 1]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    /// The PC takes everything: each message typed, then entered. Returns the steps sent and the
    /// clock after them.
    fn send_all(s: &mut Sending, mut now: u64) -> (Vec<Step>, u64) {
        let mut sent = Vec::new();
        while let Some(step) = s.next(now) {
            match step {
                Step::Wait(ms) => now += ms,
                step => {
                    sent.push(step);
                    s.answered(Answer::Ok, now);
                }
            }
        }
        (sent, now)
    }

    #[test]
    fn a_message_written_offline_waits_then_goes_once_in_order() {
        let mut s = Sending::default();
        // Offline: written, nothing asked of the queue, so nothing goes.
        s.push("fix the test", 0);
        s.push("then push", 0);
        assert!(s.waiting.iter().all(|p| p.stage == Stage::Queued && !p.failed));
        // Back online: the first one's words, and nothing more while they are out.
        assert_eq!(s.next(5_000), Some(Step::Type("fix the test".into())));
        assert_eq!(s.next(5_001), None, "one write at a time");
        s.answered(Answer::Ok, 5_020);
        // Enter waits for the PC's yes and then its moment, never a timer of its own.
        assert_eq!(s.next(5_020), Some(Step::Wait(5_000 + ENTER_AFTER_MS - 5_020)));
        assert_eq!(s.next(5_000 + ENTER_AFTER_MS), Some(Step::Enter));
        assert_eq!(s.next(5_200), None, "the second waits for the first's Enter");
        s.answered(Answer::Ok, 5_200);
        assert_eq!(s.waiting[0].stage, Stage::Entered);
        let (rest, _) = send_all(&mut s, 5_200);
        assert_eq!(rest, [Step::Type("then push".into()), Step::Enter]);
        assert_eq!(s.next(9_000), None, "each went exactly once");
        s.seen(2);
        assert!(s.waiting.is_empty());
    }

    #[test]
    fn a_failed_send_is_tapped_to_go_again_from_where_it_stopped() {
        let mut s = Sending::default();
        s.push("a", 0);
        assert_eq!(s.next(0), Some(Step::Type("a".into())));
        s.answered(Answer::Refused, 10);
        assert!(s.waiting[0].failed);
        assert_eq!(s.next(10), None, "a failed one does not go by itself");
        s.retry(0);
        assert_eq!(s.next(20), Some(Step::Type("a".into())), "its words never went: typed again");
        s.answered(Answer::Ok, 30);
        assert_eq!(s.next(200), Some(Step::Enter));
        s.answered(Answer::Refused, 210);
        s.retry(0);
        assert_eq!(s.next(400), Some(Step::Enter), "its words were typed: only Enter again");
        s.answered(Answer::Ok, 410);
        assert_eq!(s.waiting[0].stage, Stage::Entered);
    }

    #[test]
    fn messages_wait_behind_one_whose_words_sit_in_the_prompt() {
        let mut s = Sending::default();
        s.push("first", 0);
        assert_eq!(s.next(0), Some(Step::Type("first".into())));
        s.answered(Answer::Ok, 10);
        assert_eq!(s.next(ENTER_AFTER_MS), Some(Step::Enter));
        // The link drops under its Enter; a second message is written meanwhile.
        s.answered(Answer::Dropped, 100);
        s.push("second", 150);
        // Back online: Enter for the first by itself, then the second's words; never the second
        // typed after the first's.
        assert_eq!(s.next(500), Some(Step::Wait(100 + RESEND_AFTER_MS - 500)));
        assert_eq!(s.next(100 + RESEND_AFTER_MS), Some(Step::Enter));
        s.answered(Answer::Ok, 1_200);
        assert_eq!(s.next(1_200), Some(Step::Type("second".into())));
        // Refused at Enter: its words are in the prompt, so the next one waits for the tap.
        s.answered(Answer::Ok, 1_210);
        assert_eq!(s.next(1_400), Some(Step::Enter));
        s.answered(Answer::Refused, 1_410);
        s.push("third", 1_500);
        assert_eq!(s.next(5_000), None, "held behind the refused one");
        s.retry(1);
        assert_eq!(s.next(5_000), Some(Step::Enter));
        s.answered(Answer::Ok, 5_010);
        assert_eq!(s.next(5_010), Some(Step::Type("third".into())));
    }

    #[test]
    fn a_refused_message_whose_words_never_went_holds_nothing() {
        let mut s = Sending::default();
        s.push("a", 0);
        s.push("b", 0);
        assert_eq!(s.next(0), Some(Step::Type("a".into())));
        s.answered(Answer::Refused, 10);
        assert_eq!(s.next(10), Some(Step::Type("b".into())));
    }

    #[test]
    fn a_tool_answer_goes_to_its_call_by_id() {
        let msg = |id: &str, role: &str, body: Value| Msg { id: id.into(), role: role.into(), body, created_at: String::new() };
        let msgs = [
            msg("1", "assistant", json!({"blocks": [
                {"type": "tool_use", "id": "a", "name": "Bash", "summary": "cargo test"},
                {"type": "tool_use", "id": "b", "name": "TodoWrite", "summary": "", "detail": "[x] Read it\n[~] Fix it\n[ ] Ship it"}]})),
            msg("2", "tool", json!({"tool_use_id": "b", "is_error": false, "text": "ok"})),
            msg("3", "assistant", json!({"blocks": [{"type": "text", "text": "Running it."}]})),
            // An answer that comes after the agent's words still finds its call.
            msg("4", "tool", json!({"tool_use_id": "a", "is_error": true, "text": "1 failed"})),
        ];
        let it = items(&msgs);
        let Item::Did { failed, calls, .. } = &it[0] else { panic!("{it:?}") };
        assert!(*failed);
        assert_eq!((calls[0].output.as_deref(), calls[0].failed), (Some("1 failed"), true));
        assert_eq!((calls[1].output.as_deref(), calls[1].failed), (Some("ok"), false));
        assert!(calls[0].is_shell() && calls[1].is_todos());
        assert_eq!(todos(calls[1].detail.as_deref().unwrap()), [(Todo::Done, "Read it".into()), (Todo::Doing, "Fix it".into()), (Todo::Open, "Ship it".into())]);
    }

    #[test]
    fn the_log_takes_only_what_was_entered() {
        let mut s = Sending::default();
        s.push("one", 0);
        s.push("two", 0);
        assert_eq!(s.next(0), Some(Step::Type("one".into())));
        s.answered(Answer::Ok, 0);
        assert_eq!(s.next(ENTER_AFTER_MS), Some(Step::Enter));
        s.answered(Answer::Ok, ENTER_AFTER_MS);
        // Two messages of yours come into the log (one typed at the PC): only "one" can be there.
        s.seen(2);
        assert_eq!(s.waiting.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["two"]);
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn times_read_as_utc() {
        assert_eq!(parse_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(parse_ms("2026-11-25T02:18:39.598Z"), Some(1_795_573_119_598));
        assert_eq!(parse_ms("2026-11-25T03:18:39.598+01:00"), Some(1_795_573_119_598));
        assert_eq!(ago("2026-11-25T02:18:39Z", 1_795_573_119_000 + 6 * 60_000), "6m");
    }

    #[test]
    fn agents_that_need_you_come_first_and_shells_are_left_out() {
        let v = json!({"sessions": [
            {"name": "a-b", "project_id": 1, "provider": "claude", "state": "running", "last_output_at": "2026-11-25T02:00:00Z"},
            {"name": "c-d", "project_id": 1, "provider": "claude", "state": "blocked", "last_output_at": "2026-11-24T02:00:00Z"},
            {"name": "sh", "project_id": 1, "provider": "shell", "state": "running"},
        ]});
        let a = agents(&v);
        assert_eq!(a.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["c-d", "a-b"]);
        assert_eq!(a[0].initials(), "CD");
        assert_eq!(a[0].state_words(), "asks you");
    }

    #[test]
    fn tool_calls_in_a_row_fold_into_one_line() {
        let msgs: Vec<Msg> = serde_json::from_value(json!([
            {"id": "1", "role": "user", "body": {"text": "fix it"}, "created_at": ""},
            {"id": "2", "role": "assistant", "body": {"blocks": [
                {"type": "tool_use", "name": "Read", "summary": "src/a.rs"},
                {"type": "tool_use", "name": "Read", "summary": "src/b.rs"},
                {"type": "tool_use", "name": "Bash", "summary": "cargo test"}]}, "created_at": ""},
            {"id": "3", "role": "tool", "body": {"is_error": true, "text": "fail"}, "created_at": ""},
            {"id": "4", "role": "assistant", "body": {"blocks": [{"type": "text", "text": "Found it."}]}, "created_at": ""},
            {"id": "5", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "name": "Bash", "summary": "cargo test\n--quiet"}]}, "created_at": ""},
        ]))
        .unwrap();
        let it = items(&msgs);
        assert_eq!(it[0], Item::You("fix it".into()));
        assert!(matches!(&it[1], Item::Did { line, failed: true, .. } if line == "Read 2 files, ran 1 command"));
        assert_eq!(it[2], Item::Said("Found it.".into()));
        assert!(matches!(&it[3], Item::Did { line, failed: false, .. } if line == "Ran `cargo test`"));
    }

    #[test]
    fn what_you_sent_shows_once_matched_by_count_not_words() {
        let mut c = Convo::default();
        c.take(&json!({"messages": [{"id": "u1", "role": "user", "body": {"text": "a"}, "created_at": ""}], "cursor": 10, "reset": false, "found": true}));
        let mut s = Sending::default();
        s.push("push it when the tests pass", 0);
        s.push("and open the PR", 0);
        send_all(&mut s, 0);
        // The log names the first one otherwise than it was typed (a queued message): still one bubble.
        let n = c.take(&json!({"messages": [{"id": "u2", "role": "user", "body": {"text": "[queued] push it when the tests pass"}, "created_at": ""}], "cursor": 20, "reset": false, "found": true}));
        assert_eq!(n, 1);
        s.seen(n);
        assert_eq!(s.waiting.len(), 1);
        assert_eq!(s.waiting[0].text, "and open the PR");
        // The same message read twice does not count twice.
        let n = c.take(&json!({"messages": [{"id": "u2", "role": "user", "body": {"text": "x"}, "created_at": ""}], "cursor": 20, "reset": false, "found": true}));
        assert_eq!(n, 0);
    }

    #[test]
    fn markdown_is_drawn_simply() {
        let p = pieces("**Fixed.** Run `cargo test`:\n```rust\nfn a() {}\n```\nDone.");
        assert_eq!(p, [Piece::Prose("Fixed. Run cargo test:".into()), Piece::Code("fn a() {}".into()), Piece::Prose("Done.".into())]);
    }

    #[test]
    fn running_is_working_only_while_it_writes() {
        let a = agents(&json!({"sessions": [{"name": "a-b", "project_id": 1, "provider": "claude", "state": "running", "last_output_at": "2026-11-25T02:00:00Z"}]})).remove(0);
        let t = parse_ms("2026-11-25T02:00:00Z").unwrap();
        assert!(a.working_at(t + 3_000));
        assert!(!a.working_at(t + BUSY_QUIET_MS + 1), "quiet at its prompt: ready, not working");
    }

    #[test]
    fn links_are_found_with_their_words() {
        let l = links("See [the PR](https://github.com/a/b/pull/7), or https://example.org/x.\nNot ftp://x or [a](relative).");
        assert_eq!(l, [("the PR".to_string(), "https://github.com/a/b/pull/7".to_string()), ("https://example.org/x".to_string(), "https://example.org/x".to_string())]);
    }

    #[test]
    fn a_slash_command_the_log_never_shows_is_let_go() {
        let mut s = Sending::default();
        s.push("/effort high", 0);
        s.push("fix it", 0);
        // "/effort high" is entered at 1 000 ms: the wait counts from its Enter.
        send_all(&mut s, 1_000 - ENTER_AFTER_MS);
        s.expire_commands(1_000 + COMMAND_WAIT_MS - 1);
        assert_eq!(s.waiting.len(), 2);
        s.expire_commands(1_000 + COMMAND_WAIT_MS + 1);
        assert_eq!(s.waiting.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["fix it"]);
    }

    #[test]
    fn headings_lists_and_tables_keep_their_shape() {
        let p = pieces("## What changed\nThe chat now\nkeeps up.\n\n- one\n  - nested **bold**\n  goes on\n2. two [link](http://x)\n\n| a | b |\n|---|---|\n| 1 | 2 |\nAfter.");
        assert_eq!(
            p,
            [
                Piece::Heading("What changed".into()),
                Piece::Prose("The chat now keeps up.".into()),
                Piece::Item { mark: "•".into(), text: "one".into(), depth: 0 },
                Piece::Item { mark: "•".into(), text: "nested bold goes on".into(), depth: 1 },
                Piece::Item { mark: "2.".into(), text: "two link".into(), depth: 0 },
                Piece::Code("| a | b |\n| 1 | 2 |".into()),
                Piece::Prose("After.".into()),
            ]
        );
        assert_eq!(list_mark("2026 was"), None);
        assert_eq!(heading("#hashtag"), None);
    }
}
