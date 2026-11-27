//! Changes (design 44, E5): what changed on one branch of the repo. Each agent works on its own
//! branch in its own worktree, so the chip picks a branch, not a project; it starts on the open
//! chat's agent's branch, else the first one with changes. Files with their counts; a tap opens a
//! file's diff, the box stages or unstages it; the foot writes the message (or asks the PC for
//! one) and commits and pushes, and opens the pull request.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::{App, Route};
use crate::draw::{Text, WIDTH, Weight};
use crate::kit::{self, Kb};
use crate::theme::{self as t, ty};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
    pub session: Option<String>,
    pub dirty: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    pub path: String,
    /// Porcelain index letter: staged when not blank or `?`.
    pub index: String,
    pub added: i64,
    pub removed: i64,
    /// A rename's source: its diff is read against the old name.
    pub old_path: Option<String>,
    /// Git calls it binary (or too large to count): it has no text diff.
    pub binary: bool,
}

impl File {
    pub fn staged(&self) -> bool {
        !matches!(self.index.as_str(), "" | " " | "?" | "!")
    }
}

#[derive(Default)]
pub struct State {
    repo: Option<i64>,
    pub worktrees: Vec<Worktree>,
    pub worktree: Option<String>,
    pub branch: String,
    upstream: Option<String>,
    ahead: i64,
    pub files: Vec<File>,
    loaded: bool,
    open: Option<String>,
    diff: Diff,
    pub message: String,
    picking: bool,
    pending: Vec<Pending>,
}

/// A request in flight, with the repo and worktree it is about: an answer for one the page has
/// left is dropped (a read) or finished where it belongs (a commit, a push).
struct Pending {
    job: Job,
    repo: i64,
    worktree: Option<String>,
    call: Call,
}

/// The open file's diff.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Diff {
    #[default]
    Reading,
    /// Its lines, and the rows they wrap to on screen (measured once, on the first frame).
    Lines(Vec<Line>, Vec<Row>),
    /// Why there are no lines to show: binary, too large, a folder, only the mode.
    Note(String),
}

/// One line of a unified diff: `@` for a hunk's header, `+`, `-`, ` `, or `\` for git's notes.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub mark: char,
    /// Its number in the new file (the old one for a removed line), for the gutter.
    pub num: Option<i64>,
    pub text: String,
}

/// A line as drawn: wrapped pieces after the first carry no number.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    mark: char,
    num: Option<i64>,
    /// Leading columns: the code's indent, kept on every row the line wraps to.
    indent: u16,
    text: String,
}

/// The most lines of one file the diff wraps and keeps.
const MAX_LINES: usize = 3000;

#[derive(Clone, Debug, PartialEq)]
enum Job {
    Worktrees,
    Status,
    Diff,
    File(String),
    Stage,
    Suggest,
    /// `set_upstream` is the push's, read when the commit was asked, for the worktree it commits.
    Commit { push: bool, set_upstream: bool },
    Push,
    Pr,
}

impl Job {
    /// What the foot says while it runs; None for the quiet reads.
    fn label(&self) -> Option<&'static str> {
        match self {
            Job::Suggest => Some("Writing a message…"),
            Job::Commit { .. } => Some("Committing…"),
            Job::Push => Some("Pushing…"),
            Job::Pr => Some("Opening the pull request…"),
            _ => None,
        }
    }

    /// Done to the worktree whatever page is open: these finish even after a switch.
    fn acts(&self) -> bool {
        matches!(self, Job::Stage | Job::Commit { .. } | Job::Push | Job::Pr)
    }
}

pub fn worktrees(v: &Value) -> Vec<Worktree> {
    v.get("worktrees")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    Some(Worktree {
                        path: w.get("path")?.as_str()?.to_string(),
                        branch: w.get("branch").and_then(Value::as_str).unwrap_or_default().to_string(),
                        session: w.get("session").and_then(Value::as_str).map(str::to_string),
                        dirty: w.get("dirty").and_then(Value::as_bool) == Some(true),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One file's counts from `git.diff`: added, removed, binary.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Count {
    pub path: String,
    pub added: i64,
    pub removed: i64,
    pub binary: bool,
}

/// The files `git.status` lists, with the counts `git.diff` gives. That diff is HEAD against the
/// worktree, the whole change, staged or not: one count per file, never a sum.
pub fn files(status: &Value, counts: &[Count]) -> Vec<File> {
    status
        .get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let path = f.get("path")?.as_str()?.to_string();
                    let c = counts.iter().find(|c| c.path == path).cloned().unwrap_or_default();
                    Some(File {
                        index: f.get("index").and_then(Value::as_str).unwrap_or_default().to_string(),
                        old_path: f.get("renamed_from").and_then(Value::as_str).map(str::to_string),
                        path,
                        added: c.added,
                        removed: c.removed,
                        binary: c.binary,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn counts(v: &Value) -> Vec<Count> {
    v.get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let n = |k: &str| f.get(k).and_then(Value::as_i64).unwrap_or(0);
                    Some(Count { path: f.get("path")?.as_str()?.to_string(), added: n("added"), removed: n("removed"), binary: f.get("binary").and_then(Value::as_bool) == Some(true) })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One file's hunks as lines with their numbers, each hunk led by its own `@@` header as git
/// wrote it (the reply's one hunk spans the whole file; the text holds the real ones).
pub fn diff_lines(v: &Value) -> Vec<Line> {
    let mut out = Vec::new();
    for h in v.get("hunks").and_then(Value::as_array).into_iter().flatten() {
        let (mut old, mut new) = (1, 1);
        for l in h.get("text").and_then(Value::as_str).unwrap_or_default().lines() {
            let mark = l.chars().next().unwrap_or(' ');
            let num = match mark {
                '@' => {
                    // @@ -a,b +c,d @@
                    let mut it = l.split_whitespace().skip(1);
                    let start = |s: Option<&str>| s.and_then(|s| s[1..].split(',').next()?.parse::<i64>().ok()).unwrap_or(1);
                    old = start(it.next());
                    new = start(it.next());
                    None
                }
                '-' => {
                    old += 1;
                    Some(old - 1)
                }
                '+' => {
                    new += 1;
                    Some(new - 1)
                }
                '\\' => None,
                _ => {
                    old += 1;
                    new += 1;
                    Some(new - 1)
                }
            };
            let text = if mark == '@' || mark == '\\' { l.to_string() } else { l.get(1..).unwrap_or_default().to_string() };
            out.push(Line { mark: if matches!(mark, '@' | '-' | '+' | '\\') { mark } else { ' ' }, num, text });
        }
    }
    out
}

/// Why a file has no lines to show, said before asking when `git.diff` already knows.
fn no_lines(f: &File) -> Option<String> {
    if f.binary {
        return Some("A binary file: there is no text to show.".into());
    }
    if f.path.ends_with('/') {
        return Some("A new folder. Tick it to add its files; each then shows here.".into());
    }
    None
}

/// What an empty diff means for this file: its text is the same on both sides.
fn empty_note(f: Option<&File>) -> String {
    match f {
        Some(f) if f.path.ends_with('/') => no_lines(f).unwrap_or_default(),
        Some(f) if matches!(f.index.as_str(), "?" | "A") => "A new, empty file.".into(),
        _ => "Only the file's mode changed: its text is the same.".into(),
    }
}

impl State {
    /// Ask about the open worktree, once per kind of question for it at a time.
    fn ask(&mut self, link: &rc_link::Link, repo: i64, job: Job, op: &str, payload: Value) {
        let worktree = self.worktree.clone();
        self.ask_for(link, repo, worktree, job, op, payload);
    }

    fn ask_for(&mut self, link: &rc_link::Link, repo: i64, worktree: Option<String>, job: Job, op: &str, payload: Value) {
        if self.pending.iter().any(|p| p.job == job && p.repo == repo && p.worktree == worktree) {
            return;
        }
        self.pending.push(Pending { job, repo, worktree, call: link.call(op, payload) });
    }

    /// What the foot says while a commit, a push, a pull request or a message for the open
    /// worktree is on its way.
    pub fn busy(&self) -> Option<&'static str> {
        self.pending.iter().filter(|p| Some(p.repo) == self.repo && p.worktree == self.worktree).find_map(|p| p.job.label())
    }

    /// Another worktree is open: what was being read for the last one is no longer wanted.
    fn switch(&mut self, worktree: String) {
        self.worktree = Some(worktree);
        self.pending.retain(|p| p.job.acts());
        self.files.clear();
        self.loaded = false;
        self.open = None;
        self.diff = Diff::Reading;
        self.branch.clear();
        self.upstream = None;
        self.ahead = 0;
    }
}

fn wt(st: &State) -> Value {
    st.worktree.as_ref().map_or(Value::Null, |w| json!(w))
}

/// Read the open worktree again. Only the page reads: one that is not open reads when it opens.
fn reload(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if !app.online() || *app.route() != Route::Changes {
        return;
    }
    let link = app.link.clone();
    let st = &mut app.st.changes;
    st.ask_for(&link, repo, None, Job::Worktrees, "worktree.list", json!({"project_id": repo, "include_dirty": true}));
    if st.worktree.is_some() {
        let w = wt(st);
        st.ask(&link, repo, Job::Status, "git.status", json!({"project_id": repo, "worktree": w}));
        // Against HEAD: the whole change, staged or not.
        st.ask(&link, repo, Job::Diff, "git.diff", json!({"project_id": repo, "worktree": w}));
    }
}

/// Take what the PC answered, on every page: a commit chains its push from here, so the push goes
/// out wherever you are when the commit lands.
pub fn keep_up(app: &mut App) {
    if app.st.changes.pending.is_empty() {
        return;
    }
    let mut done = Vec::new();
    app.st.changes.pending.retain(|p| match p.call.poll() {
        Some(r) => {
            done.push((p.job.clone(), p.repo, p.worktree.clone(), r));
            false
        }
        None => true,
    });
    for (job, repo, worktree, r) in done {
        answered(app, job, repo, worktree, r);
    }
}

/// One answer. A read for a repo or worktree that is no longer open is dropped: the old branch's
/// files never fill the new branch's list.
fn answered(app: &mut App, job: Job, repo: i64, worktree: Option<String>, r: Result<Value, rc_link::BusError>) {
    let here = app.st.changes.repo == Some(repo) && (job == Job::Worktrees || app.st.changes.worktree == worktree);
    if !here && !job.acts() {
        return;
    }
    let mut status: Option<Value> = None;
    let mut diffs: Vec<Value> = Vec::new();
    {
        match (job, r) {
            (Job::Worktrees, Ok(v)) => {
                let list = worktrees(&v);
                let agent = app.open_chat_agent_branch();
                let st = &mut app.st.changes;
                if st.worktree.as_ref().is_none_or(|w| !list.iter().any(|x| &x.path == w)) {
                    // The open chat's agent's branch first, else the first with changes, else the first.
                    let pick = agent.and_then(|b| list.iter().find(|w| w.branch == b)).or_else(|| list.iter().find(|w| w.dirty && w.session.is_some())).or_else(|| list.first());
                    if let Some(p) = pick.map(|w| w.path.clone()) {
                        st.switch(p);
                    }
                    st.worktrees = list;
                    reload(app);
                } else {
                    st.worktrees = list;
                }
            }
            (Job::Status, Ok(v)) => {
                let st = &mut app.st.changes;
                st.branch = v.get("branch").and_then(Value::as_str).unwrap_or_default().to_string();
                st.upstream = v.get("upstream").and_then(Value::as_str).map(str::to_string);
                st.ahead = v.get("ahead").and_then(Value::as_i64).unwrap_or(0);
                status = Some(v);
            }
            (Job::Diff, Ok(v)) => diffs.push(v),
            (Job::File(p), r) => {
                let st = &mut app.st.changes;
                if st.open.as_deref() == Some(p.as_str()) {
                    st.diff = match r {
                        Ok(v) => {
                            let lines = diff_lines(&v);
                            if lines.is_empty() { Diff::Note(empty_note(st.files.iter().find(|f| f.path == p))) } else { Diff::Lines(lines, Vec::new()) }
                        }
                        Err(e) if e.code == "git.diff_too_large" => Diff::Note("Too large to show here: over 1 MB of text.".into()),
                        Err(e) if e.code == "git.diff_binary" => Diff::Note("A binary file: there is no text to show.".into()),
                        Err(e) => Diff::Note(e.message),
                    };
                }
            }
            (Job::Stage, Ok(_)) => {
                if here {
                    reload(app);
                }
            }
            (Job::Suggest, Ok(v)) => {
                if let Some(m) = v.get("message").and_then(Value::as_str) {
                    app.st.changes.message = m.trim().to_string();
                }
            }
            (Job::Commit { push, set_upstream }, Ok(_)) => {
                if here {
                    app.st.changes.message.clear();
                }
                if push {
                    push_to(app, repo, worktree, set_upstream);
                } else {
                    app.toast("Committed");
                }
                if here {
                    reload(app);
                }
            }
            (Job::Push, Ok(_)) => {
                app.toast("Pushed");
                if here {
                    reload(app);
                }
            }
            (Job::Pr, Ok(v)) => {
                let url = v.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
                app.host.copy_text(&url);
                app.toast("Pull request opened · link copied");
            }
            (_, Err(e)) => app.toast(&e.message),
        }
    }
    if let Some(s) = status {
        // Keep the last counts when only the status came back.
        let c: Vec<Count> = match diffs.last() {
            Some(d) => counts(d),
            None => app.st.changes.files.iter().map(|f| Count { path: f.path.clone(), added: f.added, removed: f.removed, binary: f.binary }).collect(),
        };
        app.st.changes.files = files(&s, &c);
        app.st.changes.loaded = true;
    } else if let Some(d) = diffs.last() {
        let c = counts(d);
        for f in &mut app.st.changes.files {
            let n = c.iter().find(|c| c.path == f.path).cloned().unwrap_or_default();
            (f.added, f.removed, f.binary) = (n.added, n.removed, n.binary);
        }
    }
}

/// Push the worktree a commit was made on, not the one open now.
fn push_to(app: &mut App, repo: i64, worktree: Option<String>, set_upstream: bool) {
    let link = app.link.clone();
    let w = worktree.as_ref().map_or(Value::Null, |w| json!(w));
    app.st.changes.ask_for(&link, repo, worktree, Job::Push, "git.push", json!({"project_id": repo, "worktree": w, "set_upstream": set_upstream}));
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.st.changes.repo != app.repo {
        // A commit or a push already asked for still finishes on the repo it was for.
        let mut pending = std::mem::take(&mut app.st.changes.pending);
        pending.retain(|p| p.job.acts());
        app.st.changes = State { repo: app.repo, pending, ..State::default() };
    }
    if app.fresh || app.take_changed("git") || app.take_changed("worktree") {
        reload(app);
    }
    let foot_h = 132.0;
    let page = app.page_begin(dt, crate::home::TOP_H, foot_h);
    let mut y = page.top + 4.0;

    // The branch chip and the totals.
    let st = &app.st.changes;
    let branch = if st.branch.is_empty() { st.worktrees.iter().find(|w| Some(&w.path) == st.worktree.as_ref()).map(|w| w.branch.clone()).unwrap_or_default() } else { st.branch.clone() };
    let (add, rem): (i64, i64) = st.files.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    let nfiles = st.files.len();
    let busy = st.busy();
    {
        let mut cx = crate::cx!(app);
        let mono = Text::new(ty::MONO, t::INK).face(Weight::Mono);
        let label = if branch.is_empty() { "Pick a branch".to_string() } else { branch.clone() };
        let bw = cx.c.measure(&label, mono).min(220.0) + 58.0;
        let x = t::PAD;
        cx.c.rect(x, y, bw, 34.0, 10.0, if cx.pressed(x, y, bw, 34.0) { t::S2 } else { t::S1 });
        cx.c.icon("branch", x + 18.0, y + 17.0, 14.0, t::INK2);
        cx.c.text_fit(&label, x + 32.0, y + 9.0, bw - 58.0, mono);
        cx.c.icon("chevron-down", x + bw - 16.0, y + 17.0, 11.0, t::INK3);
        // No switching while a commit or push for this branch is on its way.
        if cx.tap(x, y - 4.0, bw, 42.0) && busy.is_none() {
            app.st.changes.picking = true;
        }
        if add + rem > 0 {
            let at = Text::new(ty::MONO, t::OK).face(Weight::Mono);
            let a = format!("+{add}");
            let r = format!("−{rem}");
            let rw = app.canvas.measure(&r, at);
            app.canvas.text(&r, WIDTH - t::PAD, y + 9.0, at.color(t::DANGER).right());
            app.canvas.text(&a, WIDTH - t::PAD - rw - 10.0, y + 9.0, at.right());
        }
    }
    y += 48.0;

    if !app.st.changes.loaded {
        let words = if app.online() { "Reading what changed…" } else { "Changes show here once the PC answers." };
        app.canvas.text(words, t::PAD, y + 10.0, Text::new(ty::UI, t::INK3));
        y += 40.0;
    } else if nfiles == 0 {
        let ahead = app.st.changes.ahead;
        let words = if ahead > 0 { format!("Nothing to commit. {ahead} commit{} not pushed yet.", if ahead == 1 { "" } else { "s" }) } else { "Nothing changed on this branch.".to_string() };
        app.canvas.para(&words, t::PAD, y + 10.0, WIDTH - 2.0 * t::PAD, Text::new(ty::UI, t::INK3).line(1.45), 3);
        y += 50.0;
    } else {
        app.canvas.text(&format!("{nfiles} FILE{}", if nfiles == 1 { "" } else { "S" }), t::PAD, y + 4.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 26.0;
        let list = app.st.changes.files.clone();
        let mut toggle_stage: Option<File> = None;
        let mut open: Option<String> = None;
        for f in &list {
            let h = 52.0;
            let mut cx = crate::cx!(app);
            if cx.pressed(44.0, y, WIDTH - 44.0, h) {
                cx.c.rect(8.0, y, WIDTH - 16.0, h, 8.0, t::WASH);
            }
            // The box: in the next commit or not.
            let bx = t::PAD;
            if f.staged() {
                cx.c.rect(bx, y + 17.0, 18.0, 18.0, 5.0, t::ACCENT);
                cx.c.icon("check", bx + 9.0, y + 26.0, 12.0, t::ON_ACCENT);
            } else {
                cx.c.outline(bx, y + 17.0, 18.0, 18.0, 5.0, 1.5, t::INK3);
            }
            if cx.tap(0.0, y, 44.0, h) {
                toggle_stage = Some(f.clone());
            }
            // A file git does not track yet has no counts to show: it is new.
            let counts = if f.binary {
                "binary".to_string()
            } else if f.index == "?" && f.added == 0 && f.removed == 0 {
                "new".to_string()
            } else {
                format!("+{} −{}", f.added, f.removed)
            };
            let ct = Text::new(ty::SMALL, t::INK3).face(Weight::Mono);
            let cw = cx.c.measure(&counts, ct);
            // The file's name first, its folder under it: a long path never hides the name.
            let trimmed = f.path.trim_end_matches('/');
            let (dir, name) = trimmed.rsplit_once('/').map_or(("", trimmed), |(d, n)| (d, n));
            let tw = WIDTH - bx - 30.0 - cw - 26.0;
            cx.c.text_fit(name, bx + 30.0, y + 8.0, tw, Text::new(ty::MONO, t::INK).face(Weight::Mono));
            let st = Text::new(ty::SMALL, t::INK3);
            if let Some(old) = &f.old_path {
                // A rename says where it came from in place of its folder: the old name alone
                // when the folder is the same.
                let old = old.trim_end_matches('/');
                let (odir, oname) = old.rsplit_once('/').map_or(("", old), |(d, n)| (d, n));
                let from = format!("renamed from {}", if odir == dir { oname } else { old });
                cx.c.text_fit(&from, bx + 30.0, y + 28.0, tw, st);
            } else if !dir.is_empty() {
                let dir = cx.c.fit_start(dir, tw, st);
                cx.c.text(&dir, bx + 30.0, y + 28.0, st);
            }
            cx.c.text(&counts, WIDTH - t::PAD, y + 9.0, ct.right());
            if cx.tap(44.0, y, WIDTH - 44.0, h) {
                open = Some(f.path.clone());
            }
            y += h;
            if app.st.changes.open.as_deref() == Some(f.path.as_str()) {
                y += hunks(app, y, &page);
            }
        }
        if let Some(f) = toggle_stage
            && let Some(repo) = app.repo
        {
            let link = app.link.clone();
            let st = &mut app.st.changes;
            let w = wt(st);
            let op = if f.staged() { "git.unstage" } else { "git.stage" };
            st.ask(&link, repo, Job::Stage, op, json!({"project_id": repo, "worktree": w, "paths": [f.path]}));
        }
        if let Some(p) = open {
            let st = &mut app.st.changes;
            if st.open.as_deref() == Some(p.as_str()) {
                st.open = None;
            } else {
                st.open = Some(p.clone());
                let file = st.files.iter().find(|f| f.path == p).cloned();
                // Binary files and folders have no text diff: say so without asking.
                st.diff = file.as_ref().and_then(no_lines).map_or(Diff::Reading, Diff::Note);
                if st.diff == Diff::Reading
                    && let Some(repo) = app.repo
                {
                    let link = app.link.clone();
                    let st = &mut app.st.changes;
                    let w = wt(st);
                    let mut payload = json!({"project_id": repo, "worktree": w, "path": p});
                    if let Some(old) = file.and_then(|f| f.old_path) {
                        payload["old_path"] = json!(old);
                    }
                    st.ask(&link, repo, Job::File(p.clone()), "git.diff.file", payload);
                }
            }
        }
    }
    app.page_end(&page, y);
    crate::home::top(app, "Changes");
    foot(app, foot_h, nfiles);
    if app.st.changes.picking {
        picker(app);
    }
    page.moving || !app.st.changes.pending.is_empty() && !crate::wake::has()
}

/// The open file's diff on red and green, with a gutter of line numbers. Its lines wrap once,
/// when they come; each frame draws only the rows on screen. Returns its height.
fn hunks(app: &mut App, y: f32, page: &crate::app::Page) -> f32 {
    let st = Text::new(ty::SMALL, t::INK2).face(Weight::Mono).line(1.5);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let note = |app: &mut App, words: &str| {
        let h = app.canvas.para(words, x + 30.0, y + 4.0, w - 30.0, Text::new(ty::SMALL, t::INK3).line(1.4), 3);
        h.max(20.0) + 12.0
    };
    let mut diff = std::mem::take(&mut app.st.changes.diff);
    let h = match &mut diff {
        Diff::Reading => note(app, "Reading the diff…"),
        Diff::Note(words) => note(app, words),
        Diff::Lines(lines, rows) => {
            let digit = app.canvas.measure("0", st);
            let most = lines.iter().filter_map(|l| l.num).max().unwrap_or(1);
            let gutter = digit * (most.to_string().len() as f32) + 10.0;
            if rows.is_empty() {
                *rows = wrap_rows(app, lines, w - 24.0 - gutter, digit, st);
            }
            let lh = st.lh();
            let h = rows.len() as f32 * lh + 16.0;
            app.canvas.rect(x, y, w, h, 10.0, t::CODE);
            // Only what is on screen: a long diff costs a page of rows, not all of them.
            let (lo, hi) = (page.top - app.canvas.dy, page.bottom - app.canvas.dy);
            let first = (((lo - y - 8.0) / lh).floor().max(0.0)) as usize;
            let last = ((((hi - y - 8.0) / lh).ceil().max(0.0)) as usize).min(rows.len());
            for (i, r) in rows.iter().enumerate().take(last).skip(first) {
                let ly = y + 8.0 + i as f32 * lh;
                let (bg, ink) = match r.mark {
                    '+' => (Some(t::OK.a(0.12)), t::OK),
                    '-' => (Some(t::DANGER.a(0.12)), t::DANGER),
                    '@' | '\\' => (None, t::INK3),
                    _ => (None, t::INK2),
                };
                if let Some(bg) = bg {
                    app.canvas.rect(x + 1.0, ly, w - 2.0, lh, 0.0, bg);
                }
                if let Some(n) = r.num {
                    app.canvas.text(&n.to_string(), x + 6.0 + gutter - 8.0, ly, st.color(t::INK3).right());
                }
                app.canvas.text(&r.text, x + 12.0 + gutter + f32::from(r.indent) * digit, ly, st.color(ink));
            }
            h + 10.0
        }
    };
    app.st.changes.diff = diff;
    h
}

/// The lines as rows of at most `w` wide, the number on each line's first row only. The font is
/// mono, `col` one column: a line's indent is kept (a tab as four), up to a third of the width.
fn wrap_rows(app: &mut App, lines: &[Line], w: f32, col: f32, st: Text) -> Vec<Row> {
    let mut rows = Vec::new();
    let most = ((w / 3.0) / col.max(1.0)) as u16;
    for l in lines.iter().take(MAX_LINES) {
        let body = l.text.trim_start_matches([' ', '\t']);
        let lead = &l.text[..l.text.len() - body.len()];
        let indent = (lead.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum::<u16>()).min(most);
        let segs = if body.is_empty() { vec![String::new()] } else { app.canvas.wrap(body, w - f32::from(indent) * col, st, 8) };
        for (i, seg) in segs.into_iter().enumerate() {
            rows.push(Row { mark: l.mark, num: if i == 0 { l.num } else { None }, indent, text: seg });
        }
    }
    if lines.len() > MAX_LINES {
        rows.push(Row { mark: '\\', num: None, indent: 0, text: format!("{} more lines not shown", lines.len() - MAX_LINES) });
    }
    rows
}

/// The foot: the message, then Commit and push (the one filled key) and Pull request.
fn foot(app: &mut App, foot_h: f32, nfiles: usize) {
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - foot_h;
    app.canvas.dy = 0.0;
    let band = [-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0, 0.0, t::BG);
    app.canvas.rect(-1000.0, top, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let my = top + 12.0;
    app.canvas.rect(x, my, w, 46.0, 12.0, t::S1);
    let mut message = app.st.changes.message.clone();
    kit::inline(&mut crate::cx!(app), "commit-message", x + 14.0, my, w - 60.0, 46.0, "Commit message", &mut message, Kb::Text, Text::new(ty::UI, t::INK));
    app.st.changes.message = message.clone();
    // Ask the PC for a message from the diff.
    let busy = app.st.changes.busy();
    {
        let mut cx = crate::cx!(app);
        if kit::icon_key_ink(&mut cx, x + w - 44.0, my + 1.0, "sparkle", t::INK2)
            && busy.is_none()
            && nfiles > 0
            && let Some(repo) = app.repo
        {
            {
                let link = app.link.clone();
                let st = &mut app.st.changes;
                let wv = wt(st);
                st.ask(&link, repo, Job::Suggest, "git.suggest_message", json!({"project_id": repo, "worktree": wv}));
            }
        }
    }
    let ky = my + 46.0 + 12.0;
    let pr_w = 120.0;
    let ahead = app.st.changes.ahead;
    let (label, can) = if nfiles > 0 {
        ("Commit and push", !message.trim().is_empty())
    } else {
        ("Push", ahead > 0 || app.st.changes.upstream.is_none() && !app.st.changes.branch.is_empty())
    };
    let label = busy.unwrap_or(label);
    let mut cx = crate::cx!(app);
    let go = if can && busy.is_none() { kit::primary(&mut cx, x, ky, w - pr_w - 10.0, 46.0, label, None) } else { kit::plain(&mut cx, x, ky, w - pr_w - 10.0, 46.0, label, None) && false };
    let pr = kit::plain(&mut cx, x + w - pr_w, ky, pr_w, 46.0, "Pull request", None);
    let Some(repo) = app.repo else { return };
    let link = app.link.clone();
    if go {
        app.finish_typing();
        let st = &mut app.st.changes;
        let w = wt(st);
        // The commit holds its worktree, what it takes and how it pushes, read now: a branch
        // switched while it runs changes none of them.
        let set_upstream = st.upstream.is_none();
        if nfiles > 0 {
            // Nothing ticked: the whole change goes, as "Include all".
            let all = !st.files.iter().any(File::staged);
            st.ask(&link, repo, Job::Commit { push: true, set_upstream }, "git.commit", json!({"project_id": repo, "worktree": w, "message": message.trim(), "all": all}));
        } else {
            let worktree = st.worktree.clone();
            push_to(app, repo, worktree, set_upstream);
        }
    }
    if pr && busy.is_none() {
        let st = &mut app.st.changes;
        let w = wt(st);
        st.ask(&link, repo, Job::Pr, "git.pr.open", json!({"project_id": repo, "worktree": w}));
    }
}

/// The branches of the repo, one per worktree, each with its agent.
fn picker(app: &mut App) {
    let list = app.st.changes.worktrees.clone();
    let h = 64.0 + list.len().clamp(1, 8) as f32 * 56.0 + 12.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside {
        app.st.changes.picking = false;
        return;
    }
    app.canvas.text("Branch", t::PAD + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    let mut y = top + 60.0;
    let mut pick = None;
    for w in list.iter().take(8) {
        let mut cx = crate::cx!(app);
        if cx.pressed(0.0, y, WIDTH, 56.0) {
            cx.c.rect(8.0, y, WIDTH - 16.0, 56.0, 10.0, t::WASH);
        }
        cx.c.icon("branch", t::PAD + 14.0, y + 28.0, 16.0, t::INK2);
        cx.c.text_fit(&w.branch, t::PAD + 36.0, y + 9.0, WIDTH - 110.0, Text::new(ty::BODY, t::INK).semi());
        let sub = format!("{}{}", w.session.as_deref().unwrap_or("no agent"), if w.dirty { " · has changes" } else { "" });
        cx.c.text_fit(&sub, t::PAD + 36.0, y + 31.0, WIDTH - 110.0, Text::new(ty::CAPTION, t::INK3));
        if Some(&w.path) == app.st.changes.worktree.as_ref() {
            cx.c.icon("check", WIDTH - t::PAD - 14.0, y + 28.0, 18.0, t::ACCENT);
        }
        if cx.tap(0.0, y, WIDTH, 56.0) {
            pick = Some(w.path.clone());
        }
        y += 56.0;
    }
    if let Some(p) = pick {
        let st = &mut app.st.changes;
        st.picking = false;
        if st.worktree.as_ref() != Some(&p) && st.busy().is_none() {
            st.switch(p);
            reload(app);
        }
    }
}

/// Sample changes for screenshots.
pub fn sample(app: &mut App) {
    let files = vec![
        File { path: "apps/relay-code/native/crates/relay-code/src/chat.rs".into(), index: "M".into(), added: 214, removed: 61, ..File::default() },
        File { path: "apps/relay-code/native/crates/relay-code/src/ui.rs".into(), index: " ".into(), added: 38, removed: 4, ..File::default() },
        File { path: "apps/relay-code/native/crates/relay-code/src/screen.rs".into(), index: "?".into(), added: 0, removed: 0, ..File::default() },
        File { path: "apps/relay-code/native/crates/relay-code/src/changes.rs".into(), index: "R".into(), added: 12, removed: 3, old_path: Some("apps/relay-code/native/crates/relay-code/src/git.rs".into()), binary: false },
        File { path: "apps/relay-code/native/android/res/icon.png".into(), index: " ".into(), binary: true, ..File::default() },
    ];
    let text = "@@ -195,9 +195,10 @@ impl Ui {\n     fn inside(p: Option<(f32, f32)>, x: f32, y: f32, w: f32, h: f32) -> bool {\n-        Self::inside(self.tap, x, y, w, h)\n+        Self::inside(self.tap, x, y, w, h)\n+            && !self.tap.is_some_and(|p| self.hidden(p))\n     }\n@@ -240,3 +241,3 @@ impl Ui {\n     pub fn reset(&mut self) {\n-        self.tap = None;\n+        self.tap.take();\n";
    let diff = Diff::Lines(diff_lines(&json!({"hunks": [{"text": text}]})), Vec::new());
    app.st.changes = State {
        repo: app.repo,
        worktrees: vec![Worktree { path: "/w/bold-puffin".into(), branch: "relay/bold-puffin".into(), session: Some("bold-puffin".into()), dirty: true }],
        worktree: Some("/w/bold-puffin".into()),
        branch: "relay/bold-puffin".into(),
        upstream: None,
        ahead: 0,
        files,
        loaded: true,
        open: Some("apps/relay-code/native/crates/relay-code/src/ui.rs".into()),
        diff,
        message: "Chat: live status, mode and diffs".into(),
        ..State::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct NoHost(PathBuf);

    impl crate::app::Host for NoHost {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    /// An app on repo 1 with `/w/b` open in Changes.
    fn app_on_b(name: &str) -> App {
        let dir = std::env::temp_dir().join(format!("relay-code-changes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(Arc::new(NoHost(dir)), 1_000_000_000);
        a.repo = Some(1);
        a.st.changes.repo = Some(1);
        a.st.changes.worktree = Some("/w/b".into());
        a
    }

    fn add(a: &mut App, job: Job, worktree: &str) {
        // Not connected: the call fails at once, which is all a test of the bookkeeping needs.
        let call = a.link.call("git.status", json!({}));
        a.st.changes.pending.push(Pending { job, repo: 1, worktree: Some(worktree.into()), call });
    }

    #[test]
    fn the_branch_lefts_status_never_fills_the_new_branchs_list() {
        let mut a = app_on_b("left");
        let status = json!({"branch": "relay/a", "files": [{"path": "a.rs", "index": "?"}]});
        answered(&mut a, Job::Status, 1, Some("/w/a".into()), Ok(status.clone()));
        assert!(a.st.changes.files.is_empty() && !a.st.changes.loaded && a.st.changes.branch.is_empty());
        answered(&mut a, Job::Status, 1, Some("/w/b".into()), Ok(status));
        assert_eq!(a.st.changes.files.len(), 1);
        assert_eq!(a.st.changes.branch, "relay/a");
    }

    #[test]
    fn switching_branch_drops_the_reads_and_keeps_the_commit() {
        let mut a = app_on_b("switch");
        add(&mut a, Job::Status, "/w/b");
        add(&mut a, Job::File("x.rs".into()), "/w/b");
        add(&mut a, Job::Commit { push: true, set_upstream: false }, "/w/b");
        assert_eq!(a.st.changes.busy(), Some("Committing…"));
        a.st.changes.switch("/w/c".into());
        let left: Vec<_> = a.st.changes.pending.iter().map(|p| (p.job.clone(), p.worktree.clone())).collect();
        assert_eq!(left, [(Job::Commit { push: true, set_upstream: false }, Some("/w/b".to_string()))]);
        // The commit is for the branch left: the open one is free.
        assert_eq!(a.st.changes.busy(), None);
    }

    #[test]
    fn a_commit_pushes_the_worktree_it_committed_not_the_open_one() {
        let mut a = app_on_b("push");
        a.st.changes.message = "for b".into();
        answered(&mut a, Job::Commit { push: true, set_upstream: true }, 1, Some("/w/a".into()), Ok(json!({})));
        let push: Vec<_> = a.st.changes.pending.iter().map(|p| (p.job.clone(), p.repo, p.worktree.clone())).collect();
        assert_eq!(push, [(Job::Push, 1, Some("/w/a".to_string()))]);
        assert_eq!(a.st.changes.message, "for b", "the open branch's message stays");
        // A commit for a repo since left still pushes there.
        a.st.changes.pending.clear();
        a.st.changes.repo = Some(2);
        answered(&mut a, Job::Commit { push: true, set_upstream: false }, 1, Some("/w/a".into()), Ok(json!({})));
        assert_eq!(a.st.changes.pending.first().map(|p| (p.repo, p.job.clone())), Some((1, Job::Push)));
    }

    #[test]
    fn a_commit_lands_on_any_page() {
        let mut a = app_on_b("page");
        assert_ne!(*a.route(), Route::Changes);
        add(&mut a, Job::Commit { push: true, set_upstream: false }, "/w/b");
        a.frame(1_016_666_666);
        assert!(a.st.changes.pending.is_empty(), "the frame took the commit's answer off the Changes page");
    }

    #[test]
    fn a_staged_file_is_counted_once() {
        let status = json!({"branch": "relay/x", "files": [
            {"path": "a.rs", "index": "M", "worktree": "M"},
            {"path": "b.rs", "index": " ", "worktree": "M"},
            {"path": "new.rs", "index": "R", "worktree": " ", "renamed_from": "old.rs"},
            {"path": "logo.png", "index": " ", "worktree": "M"},
            {"path": "new/", "index": "?", "worktree": "?"}]});
        // git.diff against HEAD: a.rs staged and then edited again is one whole change.
        let diff = json!({"files": [
            {"path": "a.rs", "status": "M", "added": 4, "removed": 2, "binary": false},
            {"path": "b.rs", "status": "M", "added": 10, "removed": 0, "binary": false},
            {"path": "new.rs", "old_path": "old.rs", "status": "R", "added": 1, "removed": 1, "binary": false},
            {"path": "logo.png", "status": "M", "added": 0, "removed": 0, "binary": true}]});
        let f = files(&status, &counts(&diff));
        assert_eq!(f.len(), 5);
        assert_eq!((f[0].added, f[0].removed, f[0].staged()), (4, 2, true));
        assert!(!f[1].staged() && !f[4].staged());
        assert_eq!(f[2].old_path.as_deref(), Some("old.rs"));
        assert!(f[3].binary && !f[0].binary);
    }

    #[test]
    fn a_diff_keeps_gits_own_headers_and_numbers_its_lines() {
        // The reply's one hunk spans the file; its text holds the real headers.
        let text = "@@ -3,3 +3,3 @@ fn main() {\n a\n-b\n+c\n d\n@@ -20,2 +20,3 @@\n x\n+y\n z\n\\ No newline at end of file";
        let v = json!({"old": "", "new": "", "hunks": [{"old_start": 1, "old_lines": 30, "new_start": 1, "new_lines": 31, "text": text}]});
        let got: Vec<(char, Option<i64>, String)> = diff_lines(&v).into_iter().map(|l| (l.mark, l.num, l.text)).collect();
        let want = [
            ('@', None, "@@ -3,3 +3,3 @@ fn main() {"),
            (' ', Some(3), "a"),
            ('-', Some(4), "b"),
            ('+', Some(4), "c"),
            (' ', Some(5), "d"),
            ('@', None, "@@ -20,2 +20,3 @@"),
            (' ', Some(20), "x"),
            ('+', Some(21), "y"),
            (' ', Some(22), "z"),
            ('\\', None, "\\ No newline at end of file"),
        ];
        assert_eq!(got, want.map(|(m, n, t)| (m, n, t.to_string())));
    }

    /// One Changes frame with a 2,000-line diff open, on this machine's CPU:
    /// `cargo test --release changes_frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn changes_frame_cost() {
        let mut a = app_on_b("bench");
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Changes);
        let repo = a.repo.unwrap_or(1);
        let text = std::iter::once("@@ -1,1000 +1,1000 @@".to_string())
            .chain((0..1000).flat_map(|i| [format!("-    let old_{i} = users.iter().any(|u| u.client_id == mine.id);"), format!("+    let new_{i} = users.iter().any(|u| u.id == mine.id && u.text == mine.text);")]))
            .collect::<Vec<_>>()
            .join("\n");
        let path = a.st.changes.open.clone().unwrap();
        let w = a.st.changes.worktree.clone();
        answered(&mut a, Job::File(path), repo, w, Ok(json!({"hunks": [{"text": text}]})));
        let mut clock = 2_000_000_000;
        for _ in 0..5 {
            clock += 16_666_666;
            a.frame(clock);
        }
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            clock += 16_666_666;
            a.frame(clock);
        }
        println!("changes frame, 2000-line diff open: {:.0} µs", t.elapsed().as_secs_f64() * 1e6 / n as f64);
        assert!(matches!(&a.st.changes.diff, Diff::Lines(_, rows) if rows.len() >= 2000), "the diff was drawn");
    }

    #[test]
    fn files_with_no_hunks_say_why() {
        let mut a = app_on_b("nohunks");
        a.st.changes.files = vec![
            File { path: "same.sh".into(), index: " ".into(), ..File::default() },
            File { path: "empty.txt".into(), index: "?".into(), ..File::default() },
        ];
        let note = |a: &App| match &a.st.changes.diff {
            Diff::Note(w) => w.clone(),
            d => panic!("no note: {d:?}"),
        };
        a.st.changes.open = Some("same.sh".into());
        answered(&mut a, Job::File("same.sh".into()), 1, Some("/w/b".into()), Ok(json!({"old": "x", "new": "x", "hunks": []})));
        assert!(note(&a).contains("mode"));
        a.st.changes.open = Some("empty.txt".into());
        answered(&mut a, Job::File("empty.txt".into()), 1, Some("/w/b".into()), Ok(json!({"old": "", "new": "", "hunks": []})));
        assert!(note(&a).contains("empty"));
        let too = rc_link::BusError { kind: "refused".into(), code: "git.diff_too_large".into(), message: "big".into(), hint: None };
        answered(&mut a, Job::File("empty.txt".into()), 1, Some("/w/b".into()), Err(too));
        assert!(note(&a).contains("Too large"));
        // Binary files and new folders are known from git.diff and git.status: nothing to ask.
        assert!(no_lines(&File { path: "a.png".into(), binary: true, ..File::default() }).is_some_and(|w| w.contains("binary")));
        assert!(no_lines(&File { path: "new/".into(), index: "?".into(), ..File::default() }).is_some_and(|w| w.contains("folder")));
    }
}
