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
    /// Why a blocked agent waits: "prompt" (a question in its terminal) or "report" (it ended its
    /// turn blocked or partial, and its blockers say what it needs); none from an older PC.
    #[serde(default)]
    pub waiting: Option<String>,
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
            "blocked" if self.waiting.as_deref() == Some("report") => "stopped · blocked",
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
                        waiting: str_of(s, "waiting"),
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// What you wrote that the PC has not taken yet, kept on the phone (`cache/outbox.json`) so a
/// restart loses none of it: each agent's draft, and each agent's send queue.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outbox {
    #[serde(default)]
    pub drafts: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub queue: std::collections::HashMap<String, Vec<Pending>>,
}

impl Outbox {
    /// What to keep: non-empty drafts, and the messages the log cannot have yet.
    pub fn of(drafts: &std::collections::HashMap<String, String>, sending: &std::collections::HashMap<String, Sending>) -> Outbox {
        Outbox {
            drafts: drafts.iter().filter(|(_, d)| !d.trim().is_empty()).map(|(k, d)| (k.clone(), d.clone())).collect(),
            queue: sending
                .iter()
                .map(|(k, s)| (k.clone(), s.waiting.iter().filter(|p| p.stage != Stage::Entered).cloned().collect::<Vec<_>>()))
                .filter(|(_, q)| !q.is_empty())
                .collect(),
        }
    }

    /// The queues as they stand after a restart. A message that was out when the app stopped may
    /// or may not have reached the terminal: it waits for a tap from where it was, never sent twice
    /// by itself.
    pub fn sending(self) -> std::collections::HashMap<String, Sending> {
        self.queue
            .into_iter()
            .map(|(k, q)| {
                let waiting = q
                    .into_iter()
                    .map(|mut p| {
                        if matches!(p.stage, Stage::Typing | Stage::Entering) {
                            p.failed = true;
                        }
                        p.not_before_ms = 0;
                        p
                    })
                    .collect();
                (k, Sending { waiting })
            })
            .collect()
    }
}

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

    /// The log was read whole again (a first read, `/clear`, a new log): every message already
    /// entered is in it, or gone with what was cleared. Neither waits to be drawn again.
    pub fn reset(&mut self) {
        self.waiting.retain(|p| p.failed || p.stage != Stage::Entered);
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

/// What a line of a diff is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Add,
    Del,
    Same,
    /// `@@`: where the next lines are (or only that the change goes on further down).
    Hunk,
    /// A patch's file line (`*** Update File: …`).
    File,
}

/// A line of a diff as drawn: its kind, its number in the file (the new one, or the old one for a
/// removed line) when a hunk header gave one, and its words without the mark and without the
/// indent every line of the change shares.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub num: Option<u32>,
    pub text: String,
}

/// `@@ -12,3 +14,4 @@`: where the old and new lines start.
fn hunk_starts(l: &str) -> Option<(u32, u32)> {
    let mut it = l.split_whitespace().skip(1);
    let num = |s: &str| s[1..].split(',').next()?.parse::<u32>().ok();
    let (a, b) = (it.next()?, it.next()?);
    (a.starts_with('-') && b.starts_with('+')).then_some(())?;
    Some((num(a)?, num(b)?))
}

/// A change as the PC sends it (`-` and `+` lines, `@@` between hunks, a patch's `***` lines)
/// read into lines to draw, its shared indent dropped (tabs count as four spaces).
pub fn diff_lines(detail: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let (mut old, mut new): (Option<u32>, Option<u32>) = (None, None);
    for l in detail.lines() {
        if l.starts_with("@@") {
            match hunk_starts(l) {
                Some((a, b)) => (old, new) = (Some(a), Some(b)),
                None => (old, new) = (None, None),
            }
            out.push(DiffLine { kind: DiffKind::Hunk, num: None, text: String::new() });
            continue;
        }
        if let Some(f) = l.strip_prefix("*** ") {
            out.push(DiffLine { kind: DiffKind::File, num: None, text: f.trim().to_string() });
            continue;
        }
        let (kind, body) = match l.chars().next() {
            Some('+') => (DiffKind::Add, &l[1..]),
            Some('-') => (DiffKind::Del, &l[1..]),
            Some(' ') => (DiffKind::Same, &l[1..]),
            _ => (DiffKind::Same, l),
        };
        let num = match kind {
            DiffKind::Del => old.inspect(|_| old = old.map(|n| n + 1)),
            DiffKind::Add => new.inspect(|_| new = new.map(|n| n + 1)),
            _ => {
                let n = new;
                old = old.map(|n| n + 1);
                new = new.map(|n| n + 1);
                n
            }
        };
        out.push(DiffLine { kind, num, text: body.replace('\t', "    ") });
    }
    // The indent every line with words shares goes: a change deep in a function reads from the left.
    let lines = || out.iter().filter(|d| matches!(d.kind, DiffKind::Add | DiffKind::Del | DiffKind::Same) && !d.text.trim().is_empty());
    let shared = lines().map(|d| d.text.len() - d.text.trim_start_matches(' ').len()).min().unwrap_or(0);
    if shared > 0 {
        for d in out.iter_mut().filter(|d| matches!(d.kind, DiffKind::Add | DiffKind::Del | DiffKind::Same)) {
            d.text = d.text.get(shared.min(d.text.len() - d.text.trim_start_matches(' ').len())..).unwrap_or_default().to_string();
        }
    }
    out
}

/// What a piece of code is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    Str,
    Comment,
    Number,
}

/// How a file's language marks comments and strings, from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// `//` comments: Rust, Kotlin, Java, JS and TS, C, Go, Swift.
    Slashes,
    /// `#` comments: Python, shell, TOML, YAML, Ruby.
    Hash,
    /// `--` comments: SQL, Lua.
    Dashes,
}

pub fn lang_of(path: &str) -> Option<Lang> {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    Some(match ext.as_str() {
        "rs" | "kt" | "kts" | "java" | "js" | "jsx" | "ts" | "tsx" | "mjs" | "c" | "h" | "cc" | "cpp" | "hpp" | "go" | "swift" | "cs" | "dart" | "scala" | "wgsl" | "gradle" => Lang::Slashes,
        "py" | "sh" | "bash" | "zsh" | "fish" | "toml" | "yaml" | "yml" | "rb" | "pl" | "r" => Lang::Hash,
        "sql" | "lua" => Lang::Dashes,
        _ => return None,
    })
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "struct", "enum", "impl", "match", "if", "else", "for", "while", "loop", "return", "self", "Self", "const", "static", "mod", "trait",
    "where", "async", "await", "true", "false", "None", "Some", "Ok", "Err", "def", "class", "import", "from", "as", "in", "not", "and", "or", "is", "lambda", "val", "var",
    "fun", "when", "object", "interface", "function", "export", "default", "new", "this", "null", "undefined", "type", "package", "func", "defer", "case", "switch",
    "break", "continue", "try", "catch", "finally", "throw", "override", "private", "internal", "suspend", "data", "elif", "with", "yield", "local", "then", "end",
    "select", "insert", "update", "delete", "create", "table", "into", "values", "set", "null", "unsafe", "dyn", "ref", "move", "crate", "super",
];

/// A line of code as coloured pieces, by a light reading: comments to the line's end, strings in
/// quotes, numbers, and the common keywords. No state across lines.
pub fn tokens(line: &str, lang: Lang) -> Vec<(Tok, String)> {
    let c: Vec<char> = line.chars().collect();
    let mut out: Vec<(Tok, String)> = Vec::new();
    let mut push = |t: Tok, s: String| match out.last_mut() {
        Some((lt, ls)) if *lt == t => ls.push_str(&s),
        _ => out.push((t, s)),
    };
    let mut i = 0;
    while i < c.len() {
        let rest: String = c[i..].iter().take(2).collect();
        let comment = match lang {
            Lang::Slashes => rest == "//",
            Lang::Hash => c[i] == '#',
            Lang::Dashes => rest == "--",
        };
        if comment {
            push(Tok::Comment, c[i..].iter().collect());
            break;
        }
        let ch = c[i];
        // A string: to its closing quote (a backslash keeps the next character). In Rust a single
        // quote is also a lifetime, so only a short '…' counts there.
        if ch == '"' || ch == '`' || (ch == '\'' && (lang != Lang::Slashes || c.get(i + 2) == Some(&'\'') || (c.get(i + 1) == Some(&'\\') && c.get(i + 3) == Some(&'\'')))) {
            let mut j = i + 1;
            while j < c.len() && c[j] != ch {
                j += if c[j] == '\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(c.len());
            push(Tok::Str, c[i..end].iter().collect());
            i = end;
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            let mut j = i;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_') {
                j += 1;
            }
            let word: String = c[i..j].iter().collect();
            let t = if ch.is_ascii_digit() {
                Tok::Number
            } else if KEYWORDS.contains(&word.as_str()) {
                Tok::Keyword
            } else {
                Tok::Plain
            };
            push(t, word);
            i = j;
            continue;
        }
        push(Tok::Plain, ch.to_string());
        i += 1;
    }
    out
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
    items_timed(msgs).0
}

/// [`items`], with when each was written (ms since 1970; 0 when the log does not say): a folded
/// line takes the time of its first call.
pub fn items_timed(msgs: &[Msg]) -> (Vec<Item>, Vec<u64>) {
    let mut out: Vec<Item> = Vec::new();
    let mut times: Vec<u64> = Vec::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    // When the calls waiting to fold began, and the folded lines' times in the order they fold.
    let mut calls_t = 0u64;
    let mut did_ts: std::collections::VecDeque<u64> = Default::default();
    let mut failed = false;
    let flush = |out: &mut Vec<Item>, calls: &mut Vec<ToolCall>, failed: &mut bool, did_ts: &mut std::collections::VecDeque<u64>, calls_t: u64| {
        if !calls.is_empty() {
            let pairs: Vec<(String, String)> = calls.iter().map(|c| (c.name.clone(), c.summary.clone())).collect();
            out.push(Item::Did { line: did_line(&pairs), failed: *failed, calls: std::mem::take(calls) });
            did_ts.push_back(calls_t);
            *failed = false;
        }
    };
    for m in msgs {
        let t_m = parse_ms(&m.created_at).unwrap_or(0);
        let text = |k: &str| m.body.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        match m.role.as_str() {
            "user" => {
                flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
                let t = text("text");
                if !t.trim().is_empty() {
                    out.push(Item::You(t));
                }
            }
            "assistant" => {
                for b in m.body.get("blocks").and_then(Value::as_array).into_iter().flatten() {
                    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    match b.get("type").and_then(Value::as_str) {
                        Some("tool_use") => {
                            if calls.is_empty() {
                                calls_t = t_m;
                            }
                            calls.push(ToolCall { id: s("id"), name: s("name"), summary: s("summary"), detail: b.get("detail").and_then(Value::as_str).map(str::to_string), ..Default::default() })
                        }
                        Some("text") => {
                            flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
                            out.push(Item::Said(s("text")));
                        }
                        Some("thinking") => {
                            flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
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
                flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
                out.push(Item::Error(text("text")));
            }
            "note" => {
                flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
                let t = text("text");
                if !t.trim().is_empty() {
                    out.push(Item::Note(t));
                }
            }
            _ => {}
        }
        // What this message added: a folded line keeps its first call's time, the rest this one's.
        while times.len() < out.len() {
            let t = if matches!(out[times.len()], Item::Did { .. }) { did_ts.pop_front().unwrap_or(t_m) } else { t_m };
            times.push(t);
        }
    }
    flush(&mut out, &mut calls, &mut failed, &mut did_ts, calls_t);
    while times.len() < out.len() {
        times.push(did_ts.pop_front().unwrap_or(0));
    }
    (out, times)
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
    /// A table's rows, each its cells (the head first), their words plain.
    Table(Vec<Vec<String>>),
}

/// Markdown in blocks: fenced code, tables (as rows of cells), headings, bulleted and numbered
/// items with their nesting, and paragraphs (lines joined, as markdown reads them). The words of
/// paragraphs, headings and items keep their inline marks, for [`spans`].
pub fn pieces(md: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut para = String::new();
    let mut code: Option<String> = None;
    let mut table: Vec<Vec<String>> = Vec::new();
    // An item's words go on across lines until a blank line or another block.
    let mut in_item = false;
    let flush_para = |out: &mut Vec<Piece>, para: &mut String| {
        let t = para.trim().to_string();
        if !t.is_empty() {
            out.push(Piece::Prose(t));
        }
        para.clear();
    };
    let flush_table = |out: &mut Vec<Piece>, table: &mut Vec<Vec<String>>| {
        if !table.is_empty() {
            out.push(Piece::Table(std::mem::take(table)));
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
                let inner = t.strip_prefix('|').unwrap_or(t);
                let inner = inner.strip_suffix('|').unwrap_or(inner);
                table.push(inner.split('|').map(|c| inline(c.trim())).collect());
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
            out.push(Piece::Heading(h.to_string()));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some((mark, text)) = list_mark(t) {
            flush_para(&mut out, &mut para);
            out.push(Piece::Item { mark, text: text.to_string(), depth: (indent / 2).min(2) as u8 });
            in_item = true;
            continue;
        }
        if in_item && let Some(Piece::Item { text, .. }) = out.last_mut() {
            text.push(' ');
            text.push_str(t);
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

/// How a run of words is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

/// A run of words with one look: plain, `**bold**`, `*italic*`, `` `code` ``, or a link
/// (`[words](https://…)`, or a bare web address shown as itself).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub marks: Marks,
    pub link: Option<String>,
}

fn is_url_start(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// A line's inline Markdown as runs. A mark only counts when it closes later on the line (an
/// unclosed `*` or `` ` `` stays as typed), and `_` only at a word's edge, so `snake_case` and
/// `2 * 3` read as written.
pub fn spans(s: &str) -> Vec<Span> {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut out: Vec<Span> = Vec::new();
    let mut marks = Marks::default();
    let mut buf = String::new();
    let push = |out: &mut Vec<Span>, buf: &mut String, marks: Marks, link: Option<String>| {
        if buf.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(l) if l.marks == marks && l.link.is_none() && link.is_none() => l.text.push_str(buf),
            _ => out.push(Span { text: buf.clone(), marks, link }),
        }
        buf.clear();
    };
    let find = |from: usize, pat: &[char]| (from..n.saturating_sub(pat.len() - 1)).find(|&j| c[j..j + pat.len()] == *pat);
    let word = |i: usize| i < n && (c[i].is_alphanumeric());
    let mut i = 0;
    while i < n {
        let ch = c[i];
        // `code`: as typed inside, no marks.
        if ch == '`'
            && let Some(end) = find(i + 1, &['`'])
            && end > i + 1
        {
            push(&mut out, &mut buf, marks, None);
            out.push(Span { text: c[i + 1..end].iter().collect(), marks: Marks { code: true, ..marks }, link: None });
            i = end + 1;
            continue;
        }
        // [words](https://…)
        if ch == '['
            && let Some(close) = find(i + 1, &[']', '('])
            && let Some(end) = find(close + 2, &[')'])
        {
            let url: String = c[close + 2..end].iter().collect();
            let label: String = c[i + 1..close].iter().collect();
            if is_url_start(url.trim()) && !label.trim().is_empty() && !label.contains('[') {
                push(&mut out, &mut buf, marks, None);
                let words = spans(&label).into_iter().map(|s| s.text).collect::<String>();
                out.push(Span { text: words, marks, link: Some(url.trim().to_string()) });
                i = end + 1;
                continue;
            }
        }
        // A bare web address, without the full stop or bracket that ends its sentence.
        if (ch == 'h') && (i == 0 || !word(i - 1)) && is_url_start(&c[i..n.min(i + 8)].iter().collect::<String>()) {
            let mut end = i;
            while end < n && !c[end].is_whitespace() && !matches!(c[end], '<' | '>' | '"' | '`') {
                end += 1;
            }
            while end > i && matches!(c[end - 1], '.' | ',' | ';' | ':' | ')' | '!' | '?' | '\'' | '*') {
                end -= 1;
            }
            if end - i > 8 {
                push(&mut out, &mut buf, marks, None);
                let url: String = c[i..end].iter().collect();
                out.push(Span { text: url.clone(), marks, link: Some(url) });
                i = end;
                continue;
            }
        }
        // **bold** and __bold__
        if (ch == '*' || ch == '_') && i + 1 < n && c[i + 1] == ch {
            let pat = [ch, ch];
            let opening = !marks.bold && i + 2 < n && !c[i + 2].is_whitespace() && find(i + 2, &pat).is_some();
            let closing = marks.bold && i > 0 && !c[i - 1].is_whitespace();
            if (opening || closing) && (ch == '*' || !(word(i.wrapping_sub(1)) && word(i + 2))) {
                push(&mut out, &mut buf, marks, None);
                marks.bold = !marks.bold;
                i += 2;
                continue;
            }
        }
        // *italic* and _italic_
        if ch == '*' || ch == '_' {
            let edge = ch == '*' || ((i == 0 || !word(i - 1)) || !word(i + 1));
            let opening = !marks.italic && i + 1 < n && !c[i + 1].is_whitespace() && (i == 0 || !word(i - 1) || ch == '*') && find(i + 1, &[ch]).is_some_and(|j| j > i + 1 && !c[j - 1].is_whitespace());
            let closing = marks.italic && i > 0 && !c[i - 1].is_whitespace() && (ch == '*' || !word(i + 1));
            if edge && (opening || closing) {
                push(&mut out, &mut buf, marks, None);
                marks.italic = !marks.italic;
                i += 1;
                continue;
            }
        }
        if ch == '\\' && i + 1 < n && c[i + 1].is_ascii_punctuation() {
            buf.push(c[i + 1]);
            i += 2;
            continue;
        }
        buf.push(ch);
        i += 1;
    }
    push(&mut out, &mut buf, marks, None);
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
    fn drafts_and_the_queue_come_back_after_a_restart() {
        let mut sending = std::collections::HashMap::new();
        let mut s = Sending::default();
        s.push("entered", 0);
        s.push("out", 0);
        s.push("waiting", 0);
        assert_eq!(s.next(0), Some(Step::Type("entered".into())));
        s.answered(Answer::Ok, 0);
        assert_eq!(s.next(ENTER_AFTER_MS), Some(Step::Enter));
        s.answered(Answer::Ok, ENTER_AFTER_MS);
        assert_eq!(s.next(200), Some(Step::Type("out".into())));
        sending.insert("a".to_string(), s);
        let drafts = std::collections::HashMap::from([("a".to_string(), "half a thought".to_string()), ("b".to_string(), " ".to_string())]);
        let saved: Outbox = serde_json::from_slice(&serde_json::to_vec(&Outbox::of(&drafts, &sending)).unwrap()).unwrap();
        assert_eq!(saved.drafts.len(), 1, "an empty draft is not kept");
        assert_eq!(saved.drafts["a"], "half a thought");
        let mut back = saved.sending();
        let q = back.get_mut("a").unwrap();
        // The entered one is the log's now; the one that was out waits for a tap; the last waits.
        assert_eq!(q.waiting.iter().map(|p| (p.text.as_str(), p.failed)).collect::<Vec<_>>(), [("out", true), ("waiting", false)]);
        assert_eq!(q.next(1_000), Some(Step::Type("waiting".into())), "words never typed hold nothing");
    }

    #[test]
    fn each_item_keeps_when_it_was_written() {
        let msg = |id: &str, role: &str, at: &str, body: Value| Msg { id: id.into(), role: role.into(), body, created_at: at.into() };
        let msgs = [
            msg("1", "user", "2026-11-25T10:00:00Z", json!({"text": "go"})),
            msg("2", "assistant", "2026-11-25T10:01:00Z", json!({"blocks": [{"type": "tool_use", "id": "a", "name": "Read", "summary": "x"}]})),
            msg("3", "assistant", "2026-11-25T10:05:00Z", json!({"blocks": [{"type": "tool_use", "id": "b", "name": "Read", "summary": "y"}, {"type": "text", "text": "done"}]})),
            msg("4", "note", "", json!({"text": "Interrupted"})),
        ];
        let (it, times) = items_timed(&msgs);
        assert_eq!(it.len(), times.len());
        let t = |s: &str| parse_ms(s).unwrap();
        // You, the folded reads (from the first call's time), the words, the note with no time.
        assert_eq!(times, [t("2026-11-25T10:00:00Z"), t("2026-11-25T10:01:00Z"), t("2026-11-25T10:05:00Z"), 0]);
    }

    #[test]
    fn a_diff_reads_with_its_marks_numbers_and_no_shared_indent() {
        let d = diff_lines("@@ -10,2 +10,3 @@\n         let a = 1;\n-        let b = 2;\n+        let b = 3;\n+            nested();\n@@\n-        x();");
        let kinds: Vec<DiffKind> = d.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, [DiffKind::Hunk, DiffKind::Same, DiffKind::Del, DiffKind::Add, DiffKind::Add, DiffKind::Hunk, DiffKind::Del]);
        assert_eq!(d[1].text, "let a = 1;", "the shared eight spaces are gone");
        assert_eq!(d[4].text, "    nested();", "what is deeper keeps its own indent");
        // Numbers from the header: the old one for a removed line, the new one otherwise.
        assert_eq!(d[1..5].iter().map(|l| l.num).collect::<Vec<_>>(), [Some(10), Some(11), Some(11), Some(12)]);
        assert_eq!(d[6].num, None, "a bare @@ says where nothing is");
        // Claude Code's edits have no header: no numbers at all.
        assert!(diff_lines("-a\n+b").iter().all(|l| l.num.is_none()));
    }

    #[test]
    fn code_is_coloured_lightly() {
        let t = tokens("let s = \"a // b\"; // done 2", Lang::Slashes);
        assert_eq!(t, [(Tok::Keyword, "let".into()), (Tok::Plain, " s = ".into()), (Tok::Str, "\"a // b\"".into()), (Tok::Plain, "; ".into()), (Tok::Comment, "// done 2".into())]);
        // A Rust lifetime is not a string; a char is.
        assert!(tokens("fn a<'a>(x: &'a str) -> char { 'x' }", Lang::Slashes).iter().filter(|(k, _)| *k == Tok::Str).map(|(_, s)| s.as_str()).eq(["'x'"]));
        assert_eq!(tokens("x = 42  # n", Lang::Hash).last(), Some(&(Tok::Comment, "# n".into())));
        assert_eq!(lang_of("src/data.rs"), Some(Lang::Slashes));
        assert_eq!(lang_of("README.md"), None);
    }

    #[test]
    fn a_reset_log_holds_what_was_entered() {
        let mut s = Sending::default();
        s.push("/clear", 0);
        s.push("after it", 0);
        assert_eq!(s.next(0), Some(Step::Type("/clear".into())));
        s.answered(Answer::Ok, 0);
        assert_eq!(s.next(ENTER_AFTER_MS), Some(Step::Enter));
        s.answered(Answer::Ok, ENTER_AFTER_MS);
        // The log comes back reset (count 0): what was entered does not wait to show twice; the
        // message not yet sent still does.
        s.reset();
        s.seen(0);
        assert_eq!(s.waiting.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["after it"]);
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
        assert_eq!(p, [Piece::Prose("**Fixed.** Run `cargo test`:".into()), Piece::Code("fn a() {}".into()), Piece::Prose("Done.".into())]);
    }

    fn sp(text: &str, bold: bool, italic: bool, code: bool, link: Option<&str>) -> Span {
        Span { text: text.into(), marks: Marks { bold, italic, code }, link: link.map(str::to_string) }
    }

    #[test]
    fn inline_marks_read_as_runs() {
        assert_eq!(
            spans("**Fixed.** Run `cargo test` and *then* push"),
            [sp("Fixed.", true, false, false, None), sp(" Run ", false, false, false, None), sp("cargo test", false, false, true, None), sp(" and ", false, false, false, None), sp("then", false, true, false, None), sp(" push", false, false, false, None)]
        );
        // Not marks: snake_case, a lone star, an unclosed backtick, 2 * 3 * 4.
        for plain in ["call snake_case_name now", "a * b", "a `b", "2 * 3 * 4"] {
            let s = spans(plain);
            assert_eq!(s, [sp(plain, false, false, false, None)], "{plain}");
        }
        assert_eq!(spans("_quiet_ one"), [sp("quiet", false, true, false, None), sp(" one", false, false, false, None)]);
        assert_eq!(spans("***both***"), [sp("both", true, true, false, None)]);
    }

    #[test]
    fn links_are_found_with_their_words() {
        let s = spans("See [the **PR**](https://github.com/a/b/pull/7), or https://example.org/x.");
        assert_eq!(
            s,
            [sp("See ", false, false, false, None), sp("the PR", false, false, false, Some("https://github.com/a/b/pull/7")), sp(", or ", false, false, false, None), sp("https://example.org/x", false, false, false, Some("https://example.org/x")), sp(".", false, false, false, None)]
        );
        assert!(spans("Not ftp://x or [a](relative).").iter().all(|s| s.link.is_none()));
    }

    #[test]
    fn running_is_working_only_while_it_writes() {
        let a = agents(&json!({"sessions": [{"name": "a-b", "project_id": 1, "provider": "claude", "state": "running", "last_output_at": "2026-11-25T02:00:00Z"}]})).remove(0);
        let t = parse_ms("2026-11-25T02:00:00Z").unwrap();
        assert!(a.working_at(t + 3_000));
        assert!(!a.working_at(t + BUSY_QUIET_MS + 1), "quiet at its prompt: ready, not working");
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
                Piece::Item { mark: "•".into(), text: "nested **bold** goes on".into(), depth: 1 },
                Piece::Item { mark: "2.".into(), text: "two [link](http://x)".into(), depth: 0 },
                Piece::Table(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]),
                Piece::Prose("After.".into()),
            ]
        );
        assert_eq!(list_mark("2026 was"), None);
        assert_eq!(heading("#hashtag"), None);
    }
}
