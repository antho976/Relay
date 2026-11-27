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
    behind: i64,
    /// The repo's pull requests (`git.pr.list`): the open branch's is shown and opened.
    prs: Vec<Pr>,
    /// The sheet that opens a pull request, with its title and body (pub, as `picking`, for
    /// flows.rs: no tap falls through either).
    pub pr_sheet: bool,
    pub pr_title: String,
    pub pr_body: String,
    pub files: Vec<File>,
    loaded: bool,
    /// The project's base branch: an agent's work is read against where it left it.
    pub base: String,
    /// The branch's walk back from its tip, and the base's, until both are in: the commits the
    /// branch has of its own are the ones before the first the base also has (the fork).
    log: Option<Vec<Commit>>,
    base_log: Option<Vec<String>>,
    pub commits: Vec<Commit>,
    fork: Option<String>,
    /// The whole change since the fork, committed or not (`git.diff` with `base`).
    pub branch_files: Vec<File>,
    branch_loaded: bool,
    open: Option<(Group, String)>,
    diff: Diff,
    /// The commit opened in the list, and what `git.show` said of it: its body and files.
    open_commit: Option<String>,
    shown: Option<(String, String, Vec<File>)>,
    pub message: String,
    pub picking: bool,
    /// The branch picker's list, when it holds more than fits.
    pick_scroll: crate::ui::Scroll,
    /// The foot's menu (Commit without pushing), and the file whose discard waits for a yes.
    pub menu: bool,
    pub discard: Option<File>,
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

/// Which list a file is in: the branch's whole change since the base, or what is not committed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
    Branch,
    Work,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pr {
    pub number: u64,
    pub branch: String,
    pub title: String,
    pub url: String,
    /// "open", "closed" or "merged", as GitHub says.
    pub state: String,
    pub draft: bool,
}

pub fn prs(v: &Value) -> Vec<Pr> {
    v.get("pull_requests")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    Some(Pr { number: p.get("number")?.as_u64()?, branch: s("branch"), title: s("title"), url: s("url"), state: s("state").to_lowercase(), draft: p.get("draft").and_then(Value::as_bool) == Some(true) })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The branch's pull request: an open one first, else the latest closed or merged.
pub fn pr_for<'a>(prs: &'a [Pr], branch: &str) -> Option<&'a Pr> {
    let mine = || prs.iter().filter(move |p| p.branch == branch);
    mine().find(|p| p.state == "open").or_else(|| mine().max_by_key(|p| p.number))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub at: String,
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
    /// The branch's commits, then the base's, to find where they part.
    Log,
    BaseLog,
    /// The whole change since the fork.
    BranchDiff,
    Show(String),
    File(Group, String),
    /// Staging or unstaging one path: one at a time per path, so a second tick on another file
    /// is never dropped as a repeat.
    Stage(String),
    /// Put a file back as HEAD has it (or, new, into Relay's trash).
    Discard(String),
    Suggest,
    /// `set_upstream` is the push's, read when the commit was asked, for the worktree it commits.
    Commit { push: bool, set_upstream: bool },
    Push,
    Pr,
    /// The repo's pull requests, and a fetch: both for the whole repo, not one worktree.
    Prs,
    Fetch,
}

impl Job {
    /// About the repo, not the open worktree: its answer is kept across a branch switch.
    fn repo_wide(&self) -> bool {
        matches!(self, Job::Worktrees | Job::Prs | Job::Fetch)
    }

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
        matches!(self, Job::Stage(_) | Job::Discard(_) | Job::Commit { .. } | Job::Push | Job::Pr)
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

/// The files a `git.diff` or `git.show` lists, as rows.
pub fn diff_files(v: &Value) -> Vec<File> {
    v.get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let n = |k: &str| f.get(k).and_then(Value::as_i64).unwrap_or(0);
                    Some(File {
                        path: f.get("path")?.as_str()?.to_string(),
                        index: f.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                        added: n("added"),
                        removed: n("removed"),
                        old_path: f.get("old_path").and_then(Value::as_str).map(str::to_string),
                        binary: f.get("binary").and_then(Value::as_bool) == Some(true),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn commits(v: &Value) -> Vec<Commit> {
    v.get("commits")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    Some(Commit { sha: c.get("sha")?.as_str()?.to_string(), subject: s("subject"), author: s("author"), at: s("at") })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The branch's own commits, newest first, and the commit where it left the base: the first in
/// its walk the base has too. None when the walks never meet (the branch's own go past the read).
pub fn since_base(log: &[Commit], base: &[String]) -> (Vec<Commit>, Option<String>) {
    match log.iter().position(|c| base.contains(&c.sha)) {
        Some(i) => (log[..i].to_vec(), Some(log[i].sha.clone())),
        None => (log.to_vec(), None),
    }
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
        self.behind = 0;
        self.pr_sheet = false;
        self.log = None;
        self.base_log = None;
        self.commits.clear();
        self.fork = None;
        self.branch_files.clear();
        self.branch_loaded = false;
        self.open_commit = None;
        self.shown = None;
    }

    /// What the branch's whole change is read against: the fork, else the base's tip.
    fn since(&self) -> String {
        self.fork.clone().unwrap_or_else(|| self.base.clone())
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
    let base = app.projects.iter().find(|p| p.id == repo).map(|p| p.base_branch.clone()).unwrap_or_default();
    let st = &mut app.st.changes;
    st.base = base;
    st.ask_for(&link, repo, None, Job::Worktrees, "worktree.list", json!({"project_id": repo, "include_dirty": true}));
    st.ask_for(&link, repo, None, Job::Prs, "git.pr.list", json!({"project_id": repo}));
    if st.worktree.is_some() {
        let w = wt(st);
        st.ask(&link, repo, Job::Status, "git.status", json!({"project_id": repo, "worktree": w}));
        // Against HEAD: the whole change, staged or not.
        st.ask(&link, repo, Job::Diff, "git.diff", json!({"project_id": repo, "worktree": w}));
        // An agent commits its own work: what the branch did since it left the base.
        st.ask(&link, repo, Job::Log, "git.log", json!({"project_id": repo, "worktree": w, "limit": 60}));
        if !st.base.is_empty() {
            let base = st.base.clone();
            st.ask(&link, repo, Job::BaseLog, "git.log", json!({"project_id": repo, "worktree": w, "branch": base, "limit": 400}));
        }
    }
}

/// Both walks are in: the branch's own commits, and its whole change since the fork.
fn fork_point(app: &mut App, repo: i64) {
    let st = &mut app.st.changes;
    let (Some(log), Some(base)) = (&st.log, &st.base_log) else { return };
    let (own, fork) = since_base(log, base);
    st.commits = own;
    st.fork = fork;
    if st.commits.is_empty() {
        // Nothing committed of its own: the uncommitted files are the whole change.
        st.branch_files.clear();
        st.branch_loaded = true;
        return;
    }
    let link = app.link.clone();
    let w = wt(st);
    let since = st.since();
    st.ask(&link, repo, Job::BranchDiff, "git.diff", json!({"project_id": repo, "worktree": w, "base": since}));
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
    let here = app.st.changes.repo == Some(repo) && (job.repo_wide() || app.st.changes.worktree == worktree);
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
                st.behind = v.get("behind").and_then(Value::as_i64).unwrap_or(0);
                status = Some(v);
            }
            (Job::Diff, Ok(v)) => diffs.push(v),
            (Job::Log, r) => {
                app.st.changes.log = Some(r.map(|v| commits(&v)).unwrap_or_default());
                fork_point(app, repo);
            }
            // No base to read (renamed, or never fetched): the branch shows its uncommitted files.
            (Job::BaseLog, r) => {
                app.st.changes.base_log = Some(r.map(|v| commits(&v).into_iter().map(|c| c.sha).collect()).unwrap_or_default());
                fork_point(app, repo);
            }
            (Job::BranchDiff, Ok(v)) => {
                app.st.changes.branch_files = diff_files(&v);
                app.st.changes.branch_loaded = true;
            }
            // Said where the commit opens, not in a toast that leaves "Reading the commit…" up.
            (Job::Show(sha), Err(e)) => {
                if app.st.changes.open_commit.as_deref() == Some(sha.as_str()) {
                    app.st.changes.shown = Some((sha, e.message, Vec::new()));
                }
            }
            (Job::BranchDiff, Err(e)) => {
                app.st.changes.branch_loaded = true;
                app.toast(&e.message);
            }
            (Job::Show(sha), Ok(v)) => {
                let st = &mut app.st.changes;
                if st.open_commit.as_deref() == Some(sha.as_str()) {
                    let body = v.pointer("/commit/body").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                    st.shown = Some((sha, body, diff_files(&v)));
                }
            }
            (Job::File(g, p), r) => {
                let st = &mut app.st.changes;
                if st.open.as_ref() == Some(&(g, p.clone())) {
                    let list = if g == Group::Branch { &st.branch_files } else { &st.files };
                    st.diff = match r {
                        Ok(v) => {
                            let lines = diff_lines(&v);
                            if lines.is_empty() { Diff::Note(empty_note(list.iter().find(|f| f.path == p))) } else { Diff::Lines(lines, Vec::new()) }
                        }
                        Err(e) if e.code == "git.diff_too_large" => Diff::Note("Too large to show here: over 1 MB of text.".into()),
                        Err(e) if e.code == "git.diff_binary" => Diff::Note("A binary file: there is no text to show.".into()),
                        Err(e) => Diff::Note(e.message),
                    };
                }
            }
            (Job::Stage(_), Ok(_)) => {
                if here {
                    reload(app);
                }
            }
            (Job::Discard(p), Ok(_)) => {
                app.toast("Discarded");
                if here {
                    if app.st.changes.open.as_ref() == Some(&(Group::Work, p.clone())) {
                        app.st.changes.open = None;
                    }
                    app.st.changes.files.retain(|f| f.path != p);
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
                if !url.is_empty() {
                    app.host.open_url(&url);
                }
                app.toast("Pull request opened");
                let st = &mut app.st.changes;
                st.pr_title.clear();
                st.pr_body.clear();
                // The PC dropped its cached list when it opened it: read the new one.
                let link = app.link.clone();
                app.st.changes.ask_for(&link, repo, None, Job::Prs, "git.pr.list", json!({"project_id": repo}));
            }
            (Job::Prs, Ok(v)) => app.st.changes.prs = prs(&v),
            // No GitHub (not connected, no gh): the page simply shows no pull request.
            (Job::Prs, Err(_)) => {}
            (Job::Fetch, Ok(_)) => {
                let link = app.link.clone();
                app.st.changes.ask_for(&link, repo, None, Job::Prs, "git.pr.list", json!({"project_id": repo, "refresh": true}));
                reload(app);
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
    let foot_h = 132.0 + message_extra(app);
    let page = app.page_begin(dt, crate::home::TOP_H, foot_h);
    let mut y = page.top + 4.0;

    // The branch chip and the totals.
    let st = &app.st.changes;
    let branch = if st.branch.is_empty() { st.worktrees.iter().find(|w| Some(&w.path) == st.worktree.as_ref()).map(|w| w.branch.clone()).unwrap_or_default() } else { st.branch.clone() };
    // The whole change: since the base when the branch has commits of its own.
    let whole = if st.commits.is_empty() { &st.files } else { &st.branch_files };
    let (add, rem): (i64, i64) = whole.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
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
    y += 44.0;
    y = branch_line(app, y);

    if !app.st.changes.loaded {
        let words = if app.online() { "Reading what changed…" } else { "Changes show here once the PC answers." };
        app.canvas.text(words, t::PAD, y + 10.0, Text::new(ty::UI, t::INK3));
        y += 40.0;
    } else {
        let mut open: Option<(Group, String)> = None;
        let mut toggle_stage: Option<File> = None;
        // An agent's branch: everything it changed since it left the base, file by file, then
        // its commits. Agents commit their own work, so this is what there is to review.
        let commits = app.st.changes.commits.clone();
        if !commits.is_empty() {
            let base = app.st.changes.base.clone();
            let list = app.st.changes.branch_files.clone();
            let label = if app.st.changes.branch_loaded { format!("SINCE {} · {}", base.to_uppercase(), plural(list.len(), "FILE")) } else { format!("SINCE {}", base.to_uppercase()) };
            y = section(app, y, &label);
            if !app.st.changes.branch_loaded {
                app.canvas.text("Reading the branch…", t::PAD, y + 4.0, Text::new(ty::SMALL, t::INK3));
                y += 30.0;
            }
            for f in &list {
                let (_, tapped) = file_row(app, y, f, false);
                if tapped {
                    open = Some((Group::Branch, f.path.clone()));
                }
                y += 52.0;
                if app.st.changes.open.as_ref() == Some(&(Group::Branch, f.path.clone())) {
                    y += hunks(app, y, &page);
                }
            }
            y = section(app, y + 10.0, &plural(commits.len(), "COMMIT"));
            y = commit_list(app, y, &commits);
        }
        if nfiles > 0 {
            let label = if commits.is_empty() { plural(nfiles, "FILE") } else { format!("UNCOMMITTED · {}", plural(nfiles, "FILE")) };
            y = section(app, y + if commits.is_empty() { 0.0 } else { 10.0 }, &label);
            let list = app.st.changes.files.clone();
            for f in &list {
                let (ticked, tapped) = file_row(app, y, f, true);
                if ticked {
                    toggle_stage = Some(f.clone());
                }
                if tapped {
                    open = Some((Group::Work, f.path.clone()));
                }
                y += 52.0;
                if app.st.changes.open.as_ref() == Some(&(Group::Work, f.path.clone())) {
                    y += hunks(app, y, &page);
                    y += discard_key(app, y, f);
                }
            }
        } else if commits.is_empty() {
            let ahead = app.st.changes.ahead;
            let words = if ahead > 0 { format!("Nothing to commit. {} not pushed yet.", plural(ahead as usize, "commit").to_lowercase()) } else { "Nothing changed on this branch.".to_string() };
            app.canvas.para(&words, t::PAD, y + 10.0, WIDTH - 2.0 * t::PAD, Text::new(ty::UI, t::INK3).line(1.45), 3);
            y += 50.0;
        }
        if let Some(f) = toggle_stage
            && let Some(repo) = app.repo
        {
            let link = app.link.clone();
            let st = &mut app.st.changes;
            let w = wt(st);
            let op = if f.staged() { "git.unstage" } else { "git.stage" };
            st.ask(&link, repo, Job::Stage(f.path.clone()), op, json!({"project_id": repo, "worktree": w, "paths": [f.path]}));
        }
        if let Some(key) = open {
            open_file(app, key);
        }
    }
    app.page_end(&page, y);
    crate::home::top(app, "Changes");
    foot(app, foot_h, nfiles);
    if app.st.changes.picking {
        picker(app, dt);
    } else if app.st.changes.pr_sheet {
        pr_sheet(app);
    } else if app.st.changes.discard.is_some() {
        confirm_discard(app);
    }
    page.moving || !app.st.changes.pending.is_empty() && !crate::wake::has()
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "S" })
}

/// A list's small caps label. Returns where the list starts.
fn section(app: &mut App, y: f32, label: &str) -> f32 {
    app.canvas.text(label, t::PAD, y + 4.0, Text::new(ty::SMALL, t::INK3).semi());
    y + 26.0
}

/// One file: the box (when it can be ticked), its name over its folder, its counts. Returns
/// whether the box and whether the row were tapped.
fn file_row(app: &mut App, y: f32, f: &File, tick: bool) -> (bool, bool) {
    let h = 52.0;
    let mut cx = crate::cx!(app);
    let left = if tick { 44.0 } else { 0.0 };
    if cx.pressed(left, y, WIDTH - left, h) {
        cx.c.rect(8.0, y, WIDTH - 16.0, h, 8.0, t::WASH);
    }
    let bx = t::PAD;
    let mut ticked = false;
    // The box: in the next commit or not.
    if tick {
        if f.staged() {
            cx.c.rect(bx, y + 17.0, 18.0, 18.0, 5.0, t::ACCENT);
            cx.c.icon("check", bx + 9.0, y + 26.0, 12.0, t::ON_ACCENT);
        } else {
            cx.c.outline(bx, y + 17.0, 18.0, 18.0, 5.0, 1.5, t::INK3);
        }
        ticked = cx.tap(0.0, y, 44.0, h);
    }
    let tx = if tick { bx + 30.0 } else { bx };
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
    let tw = WIDTH - tx - cw - 26.0;
    cx.c.text_fit(name, tx, y + 8.0, tw, Text::new(ty::MONO, t::INK).face(Weight::Mono));
    let st = Text::new(ty::SMALL, t::INK3);
    if let Some(old) = &f.old_path {
        // A rename says where it came from in place of its folder: the old name alone when the
        // folder is the same.
        let old = old.trim_end_matches('/');
        let (odir, oname) = old.rsplit_once('/').map_or(("", old), |(d, n)| (d, n));
        let from = format!("renamed from {}", if odir == dir { oname } else { old });
        cx.c.text_fit(&from, tx, y + 28.0, tw, st);
    } else if !dir.is_empty() {
        let dir = cx.c.fit_start(dir, tw, st);
        cx.c.text(&dir, tx, y + 28.0, st);
    }
    cx.c.text(&counts, WIDTH - t::PAD, y + 9.0, ct.right());
    let tapped = cx.tap(left, y, WIDTH - left, h);
    (ticked, tapped)
}

/// Open a file's diff, or close it when it is the open one. Binary files and folders have no
/// text diff: they say so without asking.
fn open_file(app: &mut App, key: (Group, String)) {
    let st = &mut app.st.changes;
    if st.open.as_ref() == Some(&key) {
        st.open = None;
        return;
    }
    let (group, p) = key.clone();
    st.open = Some(key);
    let list = if group == Group::Branch { &st.branch_files } else { &st.files };
    let file = list.iter().find(|f| f.path == p).cloned();
    st.diff = file.as_ref().and_then(no_lines).map_or(Diff::Reading, Diff::Note);
    let Some(repo) = app.repo else { return };
    if st.diff != Diff::Reading {
        return;
    }
    let link = app.link.clone();
    let w = wt(st);
    let mut payload = json!({"project_id": repo, "worktree": w, "path": p});
    if let Some(old) = file.and_then(|f| f.old_path) {
        payload["old_path"] = json!(old);
    }
    // The branch's diff is against the fork: everything the agent changed in this file.
    if group == Group::Branch {
        payload["base"] = json!(st.since());
    }
    st.ask(&link, repo, Job::File(group, p.clone()), "git.diff.file", payload);
}

/// The branch's commits, newest first: a tap opens one, its message and its files.
fn commit_list(app: &mut App, mut y: f32, commits: &[Commit]) -> f32 {
    let now = rc_link::link::now_ms();
    let mut tapped: Option<String> = None;
    for c in commits {
        let h = 56.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(0.0, y, WIDTH, h) {
            cx.c.rect(8.0, y, WIDTH - 16.0, h, 8.0, t::WASH);
        }
        let w = WIDTH - 2.0 * t::PAD;
        cx.c.text_fit(&c.subject, t::PAD, y + 8.0, w, Text::new(ty::UI, t::INK));
        // The short sha in mono, then who and when.
        let sha: String = c.sha.chars().take(7).collect();
        let mono = Text::new(ty::SMALL, t::INK3).face(Weight::Mono);
        cx.c.text(&sha, t::PAD, y + 31.0, mono);
        let sw = cx.c.measure(&sha, mono);
        let sub = [c.author.clone(), crate::data::ago(&c.at, now)].into_iter().filter(|s| !s.is_empty()).map(|s| format!(" · {s}")).collect::<String>();
        cx.c.text_fit(&sub, t::PAD + sw, y + 31.0, w - sw, Text::new(ty::SMALL, t::INK3));
        if cx.tap(0.0, y, WIDTH, h) {
            tapped = Some(c.sha.clone());
        }
        y += h;
        if app.st.changes.open_commit.as_deref() == Some(c.sha.as_str()) {
            y = commit_detail(app, y, &c.sha);
        }
    }
    if let Some(sha) = tapped {
        let st = &mut app.st.changes;
        if st.open_commit.as_deref() == Some(sha.as_str()) {
            st.open_commit = None;
        } else {
            st.open_commit = Some(sha.clone());
            if st.shown.as_ref().is_none_or(|s| s.0 != sha)
                && let Some(repo) = app.repo
            {
                let link = app.link.clone();
                st.ask(&link, repo, Job::Show(sha.clone()), "git.show", json!({"project_id": repo, "sha": sha}));
            }
        }
    }
    y
}

/// An open commit: its message's body and the files it touched, with their counts.
fn commit_detail(app: &mut App, mut y: f32, sha: &str) -> f32 {
    let x = t::PAD + 14.0;
    let w = WIDTH - t::PAD - x;
    let Some((_, body, files)) = app.st.changes.shown.clone().filter(|s| s.0 == sha) else {
        app.canvas.text("Reading the commit…", x, y, Text::new(ty::SMALL, t::INK3));
        return y + 30.0;
    };
    let top = y;
    if !body.is_empty() {
        y += app.canvas.para(&body, x, y, w, Text::new(ty::SMALL, t::INK2).line(1.45), 8) + 10.0;
    }
    let mono = Text::new(ty::SMALL, t::INK2).face(Weight::Mono);
    let ct = Text::new(ty::SMALL, t::INK3).face(Weight::Mono);
    for f in &files {
        let counts = if f.binary { "binary".to_string() } else { format!("+{} −{}", f.added, f.removed) };
        let cw = app.canvas.measure(&counts, ct);
        let name = f.path.trim_end_matches('/').rsplit('/').next().unwrap_or(&f.path).to_string();
        let name = app.canvas.fit_start(&name, w - cw - 12.0, mono);
        app.canvas.text(&name, x, y, mono);
        app.canvas.text(&counts, WIDTH - t::PAD, y, ct.right());
        y += 24.0;
    }
    // A rule down the side ties the detail to its commit.
    app.canvas.rect(t::PAD, top, 2.0, (y - top - 4.0).max(0.0), 1.0, t::S3);
    y + 10.0
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
    let box_h = 46.0 + message_extra(app);
    app.canvas.rect(x, my, w, box_h, 12.0, t::S1);
    let mut message = app.st.changes.message.clone();
    message_box(app, x + 14.0, my, w - 60.0, box_h, &mut message);
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
    let ky = my + box_h + 12.0;
    let pr_w = 120.0;
    let st = &app.st.changes;
    let ahead = st.ahead;
    // A detached HEAD has no branch to push to: it commits, and that is all.
    let detached = st.branch == "HEAD";
    let (label, can) = if nfiles > 0 {
        (if detached { "Commit" } else { "Commit and push" }, !message.trim().is_empty())
    } else if detached {
        ("Detached HEAD", false)
    } else {
        ("Push", ahead > 0 || st.upstream.is_none() && !st.branch.is_empty())
    };
    let label = busy.unwrap_or(label);
    // The branch's pull request opens on GitHub; with none, the key opens one.
    let open_pr = pr_for(&st.prs, &st.branch).filter(|p| p.state == "open").map(|p| p.url.clone());
    let can_pr = !detached && !st.branch.is_empty() && st.branch != st.base;
    let mut cx = crate::cx!(app);
    // More: commit without pushing. Only when there is something to commit and somewhere to push.
    let more_w = if nfiles > 0 && !detached { 46.0 + 10.0 } else { 0.0 };
    let go_w = w - pr_w - 10.0 - more_w;
    let go = if can && busy.is_none() { kit::primary(&mut cx, x, ky, go_w, 46.0, label, None) } else { kit::plain(&mut cx, x, ky, go_w, 46.0, label, None) && false };
    let more = more_w > 0.0 && kit::plain(&mut cx, x + go_w + 10.0, ky, 46.0, 46.0, "", Some("more"));
    let pr = kit::plain(&mut cx, x + w - pr_w, ky, pr_w, 46.0, if open_pr.is_some() { "View PR" } else { "Pull request" }, None);
    if more && busy.is_none() {
        app.st.changes.menu = true;
    }
    let mut commit_only = false;
    if app.st.changes.menu {
        let items = ["Commit only"];
        let mtop = ky - 8.0 - (48.0 * items.len() as f32 + 8.0);
        match kit::menu(&mut crate::cx!(app), mtop, &items) {
            Some(0) => {
                app.st.changes.menu = false;
                commit_only = can;
                if !can {
                    app.toast("Write a message first");
                }
            }
            Some(_) => app.st.changes.menu = false,
            None => {}
        }
    }
    let go = go || commit_only;
    if pr {
        if let Some(url) = &open_pr {
            app.host.open_url(url);
        } else if can_pr && busy.is_none() {
            app.finish_typing();
            let st = &mut app.st.changes;
            // The title starts as the branch's one commit, when it has one; the PC fills in the
            // rest from the commits when left empty.
            if st.pr_title.is_empty() && st.commits.len() == 1 {
                st.pr_title = st.commits[0].subject.clone();
            }
            st.pr_sheet = true;
        }
    }
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
            st.ask(&link, repo, Job::Commit { push: !detached && !commit_only, set_upstream }, "git.commit", json!({"project_id": repo, "worktree": w, "message": message.trim(), "all": all}));
        } else {
            let worktree = st.worktree.clone();
            push_to(app, repo, worktree, set_upstream);
        }
    }
}

/// The commit message's style and width in the foot's box.
fn message_style() -> Text {
    Text::new(ty::UI, t::INK).line(1.4)
}

const MESSAGE_LINES: usize = 5;

/// The message's text as the box shows it: the keyboard's while it is being typed.
fn message_text(app: &App) -> String {
    if app.focus.is("commit-message") { app.focus.text.clone() } else { app.st.changes.message.clone() }
}

/// How much taller than one line the box is: a message of several lines (a subject, a blank
/// line, a body, as git.suggest_message writes them) shows up to five of them.
fn message_extra(app: &mut App) -> f32 {
    let text = message_text(app);
    if text.is_empty() {
        return 0.0;
    }
    let st = message_style();
    let w = WIDTH - 2.0 * t::PAD - 60.0;
    let n = app.canvas.wrap_spans(&text, w, st).len().clamp(1, MESSAGE_LINES);
    (n - 1) as f32 * st.lh()
}

/// The commit message: every line, wrapped, with the caret at its end while typing. A tap gives it
/// the keyboard, as kit::inline does.
fn message_box(app: &mut App, x: f32, y: f32, w: f32, h: f32, value: &mut String) {
    let id = "commit-message";
    let mut cx = crate::cx!(app);
    let focused = cx.focus.is(id);
    if focused {
        *value = cx.focus.text.clone();
        cx.focus.at = Some((y, h));
    }
    let st = message_style();
    if value.is_empty() {
        cx.c.text("Commit message", x, y + (46.0 - st.lh()) / 2.0, st.color(t::INK3));
        if focused {
            cx.c.rect(x, y + 12.0, 2.0, 22.0, 1.0, t::ACCENT);
        }
    } else {
        let chars: Vec<char> = value.chars().collect();
        let spans = cx.c.wrap_spans(value, w, st);
        // Typing: the last lines, where the caret is. Otherwise the first.
        let first = if focused { spans.len().saturating_sub(MESSAGE_LINES) } else { 0 };
        let mut ly = y + (46.0 - st.lh()) / 2.0;
        let mut end = (x, ly);
        for (i, (a, b)) in spans.iter().enumerate().skip(first).take(MESSAGE_LINES) {
            let line: String = chars[*a..*b].iter().collect();
            let line = if !focused && i + 1 == first + MESSAGE_LINES && spans.len() > first + MESSAGE_LINES { format!("{}…", line.trim_end()) } else { line };
            let tw = cx.c.text(&line, x, ly, st);
            end = (x + tw, ly);
            ly += st.lh();
        }
        if focused {
            cx.c.rect(end.0 + 1.0, end.1 + (st.lh() - 22.0) / 2.0, 2.0, 22.0, 1.0, t::ACCENT);
        }
    }
    if cx.tap(x - 14.0, y, w + 28.0, h) && (!focused || !cx.focus.keyboard_up) {
        if !focused {
            cx.focus.id = Some(id);
            cx.focus.text = value.clone();
            cx.focus.kind = Kb::Text;
        }
        cx.host.keyboard(true, value, Kb::Text);
    }
}

/// Under an open uncommitted file's diff: Discard, which asks first. Returns its height.
fn discard_key(app: &mut App, y: f32, f: &File) -> f32 {
    let label = if f.index == "?" { "Delete this new file" } else { "Discard changes" };
    let small = Text::new(ty::SMALL, t::DANGER).medium();
    let lw = app.canvas.measure(label, small) + 24.0;
    let x = WIDTH - t::PAD - lw;
    let mut cx = crate::cx!(app);
    if cx.pressed(x, y - 4.0, lw, 40.0) {
        cx.c.rect(x, y - 2.0, lw, 32.0, 8.0, t::WASH);
    }
    cx.c.text(label, x + 12.0, y + 5.0, small);
    if cx.tap(x, y - 6.0, lw, 44.0) {
        app.st.changes.discard = Some(f.clone());
    }
    40.0
}

/// Discard asks before it goes: a file put back as HEAD has it loses its edits, staged or not; a
/// new file goes to Relay's trash.
fn confirm_discard(app: &mut App) {
    let Some(f) = app.st.changes.discard.clone() else { return };
    let name = f.path.trim_end_matches('/').rsplit('/').next().unwrap_or(&f.path).to_string();
    let (title, body, act) = if f.index == "?" {
        (format!("Delete {name}?"), "Git does not track it yet. It goes to Relay's trash on the PC.".to_string(), "Delete")
    } else {
        (format!("Discard {name}?"), "It goes back to the last commit. Its edits, staged or not, are lost.".to_string(), "Discard")
    };
    let answer = kit::confirm(&mut crate::cx!(app), &title, &body, act);
    let Some(yes) = answer else { return };
    app.st.changes.discard = None;
    if !yes {
        return;
    }
    let Some(repo) = app.repo else { return };
    let link = app.link.clone();
    let st = &mut app.st.changes;
    let w = wt(st);
    let op = if f.index == "?" { "file.delete" } else { "file.restore_head" };
    st.ask(&link, repo, Job::Discard(f.path.clone()), op, json!({"project_id": repo, "worktree": w, "path": f.path}));
}

/// Under the branch chip: how it stands against its upstream, and Fetch; then its pull request,
/// which opens on GitHub. Returns where the page goes on.
fn branch_line(app: &mut App, mut y: f32) -> f32 {
    let st = &app.st.changes;
    if !st.loaded {
        return y + 4.0;
    }
    let words = if st.branch == "HEAD" {
        "Detached HEAD: commits here are on no branch.".to_string()
    } else {
        match &st.upstream {
            None => "Not pushed yet.".to_string(),
            Some(up) => match (st.ahead, st.behind) {
                (0, 0) => format!("Up to date with {up}."),
                (a, 0) => format!("{a} to push to {up}."),
                (0, b) => format!("{b} behind {up}."),
                (a, b) => format!("{a} to push, {b} behind {up}."),
            },
        }
    };
    let fetching = st.pending.iter().any(|p| p.job == Job::Fetch);
    let pr = pr_for(&st.prs, &st.branch).cloned();
    let small = Text::new(ty::SMALL, t::INK3);
    let key = if fetching { "Fetching…" } else { "Fetch" };
    let kw = app.canvas.measure(key, small.color(t::INK2)) + 24.0;
    app.canvas.text_fit(&words, t::PAD + 2.0, y + 9.0, WIDTH - 2.0 * t::PAD - kw - 8.0, small);
    let fetch = {
        let mut cx = crate::cx!(app);
        let kx = WIDTH - t::PAD - kw;
        if cx.pressed(kx, y - 6.0, kw, 44.0) {
            cx.c.rect(kx, y, kw, 32.0, 8.0, t::WASH);
        }
        cx.c.text(key, kx + kw / 2.0, y + 9.0, small.color(t::INK2).center());
        cx.tap(kx, y - 6.0, kw, 44.0)
    };
    if fetch
        && !fetching
        && let Some(repo) = app.repo
    {
        let link = app.link.clone();
        app.st.changes.ask_for(&link, repo, None, Job::Fetch, "git.fetch", json!({"project_id": repo}));
    }
    y += 40.0;
    if let Some(p) = pr {
        let (x, w, h) = (t::PAD, WIDTH - 2.0 * t::PAD, 58.0);
        let mut cx = crate::cx!(app);
        cx.c.rect(x, y, w, h, 12.0, if cx.pressed(x, y, w, h) { t::S2 } else { t::S1 });
        cx.c.icon("merge", x + 22.0, y + h / 2.0, 16.0, if p.state == "open" { t::ACCENT } else { t::INK3 });
        let state = match (p.state.as_str(), p.draft) {
            ("open", true) => "Draft",
            ("open", false) => "Open",
            ("merged", _) => "Merged",
            _ => "Closed",
        };
        cx.c.text_fit(&p.title, x + 44.0, y + 10.0, w - 80.0, Text::new(ty::UI, t::INK));
        cx.c.text(&format!("#{} · {state}", p.number), x + 44.0, y + 33.0, Text::new(ty::SMALL, t::INK3));
        cx.c.icon("external", x + w - 20.0, y + h / 2.0, 14.0, t::INK3);
        if cx.tap(x, y, w, h) {
            app.host.open_url(&p.url);
        }
        y += h + 14.0;
    }
    y
}

/// The sheet that opens a pull request for the branch: its title and body, both optional (the PC
/// writes them from the commits when left empty).
fn pr_sheet(app: &mut App) {
    let h = 330.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside {
        app.finish_typing();
        app.st.changes.pr_sheet = false;
        return;
    }
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let st = &app.st.changes;
    let into = format!("{} into {}", st.branch, if st.base.is_empty() { "the base" } else { &st.base });
    app.canvas.text("Pull request", x + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    app.canvas.text_fit(&into, x + 4.0, top + 58.0, w - 8.0, Text::new(ty::SMALL, t::INK3).face(Weight::Mono));
    let mut title = app.st.changes.pr_title.clone();
    let mut body = app.st.changes.pr_body.clone();
    let mut y = top + 88.0;
    y += kit::field(&mut crate::cx!(app), "pr-title", x, y, w, "Title", "From the commits", &mut title, Kb::Text) + 12.0;
    kit::field(&mut crate::cx!(app), "pr-body", x, y, w, "Description", "Optional", &mut body, Kb::Text);
    app.st.changes.pr_title = title.clone();
    app.st.changes.pr_body = body.clone();
    let ky = top + h - 62.0;
    let go = kit::primary(&mut crate::cx!(app), x, ky, w, 46.0, "Open pull request", None);
    if go && let Some(repo) = app.repo {
        app.finish_typing();
        let link = app.link.clone();
        let st = &mut app.st.changes;
        st.pr_sheet = false;
        let w = wt(st);
        let some = |s: &str| if s.trim().is_empty() { Value::Null } else { json!(s.trim()) };
        st.ask(&link, repo, Job::Pr, "git.pr.open", json!({"project_id": repo, "worktree": w, "title": some(&title), "body": some(&body)}));
    }
}

/// The branches of the repo, one per worktree, each with its agent.
fn picker(app: &mut App, dt: f32) {
    let list = app.st.changes.worktrees.clone();
    // Eight rows show; more scroll inside the sheet.
    let h = 64.0 + list.len().clamp(1, 8) as f32 * 56.0 + 12.0;
    let (top, outside) = kit::sheet(&mut crate::cx!(app), h);
    if outside {
        app.st.changes.picking = false;
        app.st.changes.pick_scroll = crate::ui::Scroll::default();
        return;
    }
    app.canvas.text("Branch", t::PAD + 4.0, top + 28.0, Text::new(ty::SECTION, t::INK).semi());
    let list_top = top + 60.0;
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let viewport = [0.0, list_top, WIDTH, bottom - list_top];
    app.st.changes.pick_scroll.update(&app.ui, viewport, dt);
    app.canvas.push_clip(0.0, list_top, WIDTH, bottom - list_top);
    let mut y = list_top - app.st.changes.pick_scroll.off;
    let mut pick = None;
    for w in &list {
        let visible = y + 56.0 > list_top && y < bottom;
        if !visible {
            y += 56.0;
            continue;
        }
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
    app.canvas.pop_clip();
    app.st.changes.pick_scroll.max = y + app.st.changes.pick_scroll.off - bottom + 12.0;
    if let Some(p) = pick {
        let st = &mut app.st.changes;
        st.picking = false;
        st.pick_scroll = crate::ui::Scroll::default();
        if st.worktree.as_ref() != Some(&p) && st.busy().is_none() {
            st.switch(p);
            reload(app);
        }
    }
}

/// Sample changes for screenshots.
pub fn sample(app: &mut App) {
    let src = "apps/relay-code/native/crates/relay-code/src";
    let f = |name: &str, index: &str, added: i64, removed: i64| File { path: format!("{src}/{name}"), index: index.into(), added, removed, ..File::default() };
    let branch_files = vec![
        f("chat.rs", "M", 214, 61),
        f("ui.rs", "M", 38, 4),
        f("screen.rs", "A", 179, 0),
        File { old_path: Some(format!("{src}/git.rs")), ..f("changes.rs", "R", 12, 3) },
        File { path: "apps/relay-code/native/android/res/icon.png".into(), index: "M".into(), binary: true, ..File::default() },
    ];
    let files = vec![f("ui.rs", " ", 6, 1), File { path: "apps/relay-code/native/android/res/icon.png".into(), index: " ".into(), binary: true, ..File::default() }];
    let at = |mins: u64| {
        let ms = rc_link::link::now_ms().saturating_sub(mins * 60_000) / 1000;
        let (d, s) = (ms / 86_400, ms % 86_400);
        // Days since 1970 to a date (civil from days, Howard Hinnant's).
        let z = d as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
    };
    let commits = vec![
        Commit { sha: "3b5b460e4f1c".into(), subject: "Chat: live status, mode and questions from session.screen".into(), author: "bold-puffin".into(), at: at(12) },
        Commit { sha: "9c0d7aa21e55".into(), subject: "Changes: rename git.rs, read diffs against the fork".into(), author: "bold-puffin".into(), at: at(95) },
        Commit { sha: "1f8369210b3d".into(), subject: "Ui: a tap under a sheet reaches nothing".into(), author: "bold-puffin".into(), at: at(260) },
    ];
    let shown = ("3b5b460e4f1c".to_string(), "The terminal's own status line says what the agent is doing.".to_string(), vec![f("chat.rs", "M", 120, 40), f("screen.rs", "A", 179, 0)]);
    app.st.changes = State {
        repo: app.repo,
        worktrees: vec![Worktree { path: "/w/bold-puffin".into(), branch: "relay/bold-puffin".into(), session: Some("bold-puffin".into()), dirty: true }],
        worktree: Some("/w/bold-puffin".into()),
        branch: "relay/bold-puffin".into(),
        upstream: Some("origin/relay/bold-puffin".into()),
        ahead: 2,
        behind: 1,
        prs: vec![Pr { number: 209, branch: "relay/bold-puffin".into(), title: "Relay Code: the phone app rewritten in Rust + wgpu".into(), url: "https://github.com/antho976/Relay/pull/209".into(), state: "open".into(), draft: false }],
        files,
        loaded: true,
        base: "main".into(),
        commits,
        fork: Some("3549249d40c9".into()),
        branch_files,
        branch_loaded: true,
        open_commit: Some("3b5b460e4f1c".into()),
        shown: Some(shown),
        message: "Ui: keep the tap on the sheet\n\nA tap on a sheet no longer reaches the row under it.".into(),
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
        add(&mut a, Job::File(Group::Work, "x.rs".into()), "/w/b");
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
        let path = a.st.changes.branch_files[1].path.clone();
        a.st.changes.open_commit = None;
        a.st.changes.open = Some((Group::Branch, path.clone()));
        let w = a.st.changes.worktree.clone();
        answered(&mut a, Job::File(Group::Branch, path), repo, w, Ok(json!({"hunks": [{"text": text}]})));
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
    fn a_branch_has_the_commits_before_it_meets_the_base() {
        let c = |s: &str| Commit { sha: s.into(), ..Commit::default() };
        let log = [c("c3"), c("c2"), c("c1"), c("m1"), c("m0")];
        let base = ["m2".to_string(), "m1".into(), "m0".into()];
        let (own, fork) = since_base(&log, &base);
        assert_eq!(own.iter().map(|c| c.sha.as_str()).collect::<Vec<_>>(), ["c3", "c2", "c1"]);
        assert_eq!(fork.as_deref(), Some("m1"));
        // On the base itself: nothing of its own.
        assert_eq!(since_base(&log[3..], &base), (vec![], Some("m1".into())));
        // Walks that never meet: all its commits, and the base's tip to diff against.
        assert_eq!(since_base(&log[..2], &base).1, None);
    }

    #[test]
    fn an_agents_branch_reads_its_whole_change_since_the_fork() {
        let mut a = app_on_b("branch");
        a.st.changes.base = "main".into();
        let log = json!({"commits": [{"sha": "c2", "subject": "Two"}, {"sha": "c1", "subject": "One"}, {"sha": "m1"}]});
        answered(&mut a, Job::Log, 1, Some("/w/b".into()), Ok(log));
        assert!(a.st.changes.commits.is_empty(), "waits for the base's walk");
        answered(&mut a, Job::BaseLog, 1, Some("/w/b".into()), Ok(json!({"commits": [{"sha": "m2"}, {"sha": "m1"}]})));
        assert_eq!(a.st.changes.commits.len(), 2);
        assert_eq!(a.st.changes.since(), "m1");
        assert!(a.st.changes.pending.iter().any(|p| p.job == Job::BranchDiff && p.worktree.as_deref() == Some("/w/b")));
        let diff = json!({"files": [{"path": "a.rs", "status": "M", "added": 5, "removed": 1, "binary": false}, {"path": "n.rs", "old_path": "o.rs", "status": "R", "added": 0, "removed": 0, "binary": false}]});
        answered(&mut a, Job::BranchDiff, 1, Some("/w/b".into()), Ok(diff));
        assert_eq!(a.st.changes.branch_files.len(), 2);
        assert_eq!(a.st.changes.branch_files[1].old_path.as_deref(), Some("o.rs"));
        // Opening one of its files reads it against the fork, in its own group.
        a.st.changes.pending.clear();
        open_file(&mut a, (Group::Branch, "a.rs".into()));
        assert_eq!(a.st.changes.pending.first().map(|p| p.job.clone()), Some(Job::File(Group::Branch, "a.rs".into())));
        // A commit opens what git.show says of it.
        a.st.changes.open_commit = Some("c1".into());
        answered(&mut a, Job::Show("c1".into()), 1, Some("/w/b".into()), Ok(json!({"commit": {"sha": "c1", "body": "Why.\n"}, "files": [{"path": "a.rs", "status": "M", "added": 5, "removed": 1, "binary": false}]})));
        let (sha, body, files) = a.st.changes.shown.clone().unwrap();
        assert_eq!((sha.as_str(), body.as_str(), files.len()), ("c1", "Why.", 1));
    }

    #[test]
    fn a_branch_with_no_base_to_read_shows_its_uncommitted_files() {
        let mut a = app_on_b("nobase");
        a.st.changes.base = "trunk".into();
        answered(&mut a, Job::Log, 1, Some("/w/b".into()), Ok(json!({"commits": [{"sha": "c1"}]})));
        let gone = rc_link::BusError { kind: "not_found".into(), code: "git.revision".into(), message: "no trunk".into(), hint: None };
        answered(&mut a, Job::BaseLog, 1, Some("/w/b".into()), Err(gone));
        // Its commits are listed, and the change is read against the base's name as asked.
        assert_eq!(a.st.changes.commits.len(), 1);
        assert_eq!(a.st.changes.since(), "trunk");
    }

    #[test]
    fn the_branchs_pull_request_is_its_open_one_else_its_latest() {
        let p = |n: u64, b: &str, st: &str| Pr { number: n, branch: b.into(), state: st.into(), ..Pr::default() };
        let list = [p(3, "relay/x", "closed"), p(7, "relay/x", "merged"), p(5, "relay/x", "open"), p(9, "relay/y", "open")];
        assert_eq!(pr_for(&list, "relay/x").map(|p| p.number), Some(5));
        assert_eq!(pr_for(&list[..2], "relay/x").map(|p| p.number), Some(7));
        assert_eq!(pr_for(&list, "relay/z"), None);
        let v = json!({"pull_requests": [{"number": 209, "branch": "relay/x", "draft": true, "url": "u", "title": "T", "state": "OPEN", "same_repository": true}], "complete": true});
        assert_eq!(prs(&v), [Pr { number: 209, branch: "relay/x".into(), title: "T".into(), url: "u".into(), state: "open".into(), draft: true }]);
    }

    /// Press and lift a finger at (x, y), in the app's units, with frames between.
    fn press(a: &mut App, clock: &mut u64, x: f32, y: f32) {
        use crate::ui::{Phase, Touch};
        let u = a.canvas.u.max(0.01);
        let (px, py) = (x * u + ((390.0 - WIDTH * u) * 0.5).max(0.0), y * u);
        for phase in [Phase::Down, Phase::Up] {
            *clock += 30_000_000;
            a.touch(Touch { id: 1, phase, x: px, y: py, t_ns: *clock });
            a.frame(*clock);
        }
        for _ in 0..20 {
            *clock += 16_666_666;
            a.frame(*clock);
        }
    }

    #[test]
    fn a_tap_on_the_pull_request_sheet_reaches_nothing_under_it() {
        let mut a = app_on_b("prsheet");
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Changes);
        a.st.changes.open_commit = None;
        a.st.changes.pr_sheet = true;
        let mut clock = 2_000_000_000;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        // On the sheet's title, over the page's file rows.
        let y = a.canvas.h - 330.0 + 40.0;
        press(&mut a, &mut clock, 40.0, y);
        assert!(a.st.changes.pr_sheet, "a tap on the sheet keeps it");
        assert_eq!(a.st.changes.open, None, "no file under it opened");
        assert!(a.st.changes.pending.is_empty(), "no key under it was pressed");
        // A tap above it closes it, and still reaches nothing.
        press(&mut a, &mut clock, 40.0, 150.0);
        assert!(!a.st.changes.pr_sheet);
        assert_eq!(a.st.changes.open, None);
    }

    #[test]
    fn a_detached_head_commits_without_pushing() {
        let mut a = app_on_b("detached");
        a.st.changes.branch = "HEAD".into();
        answered(&mut a, Job::Commit { push: false, set_upstream: false }, 1, Some("/w/b".into()), Ok(json!({})));
        assert!(a.st.changes.pending.is_empty(), "no push after the commit");
    }

    /// A tap, looked at on the frame it lands: offline, what it asked fails on the next frame.
    fn lift(a: &mut App, clock: &mut u64, x: f32, y: f32) {
        use crate::ui::{Phase, Touch};
        for phase in [Phase::Down, Phase::Up] {
            *clock += 30_000_000;
            a.touch(Touch { id: 1, phase, x, y, t_ns: *clock });
            a.frame(*clock);
        }
    }

    /// Changes open on the sample, settled, with nothing opened.
    fn on_changes(name: &str) -> (App, u64) {
        let mut a = app_on_b(name);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(Route::Changes);
        a.st.changes.open_commit = None;
        let mut clock = 2_000_000_000;
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        (a, clock)
    }

    #[test]
    fn ticking_two_files_stages_both() {
        let mut a = app_on_b("stage");
        let link = a.link.clone();
        for p in ["a.rs", "b.rs", "a.rs"] {
            a.st.changes.ask(&link, 1, Job::Stage(p.into()), "git.stage", json!({}));
        }
        let jobs: Vec<Job> = a.st.changes.pending.iter().map(|p| p.job.clone()).collect();
        assert_eq!(jobs, [Job::Stage("a.rs".into()), Job::Stage("b.rs".into())], "a repeat on one path waits; another path goes");
    }

    #[test]
    fn discard_asks_first_and_a_no_reaches_nothing() {
        let (mut a, mut clock) = on_changes("discard");
        let f = a.st.changes.files[0].clone();
        a.st.changes.discard = Some(f.clone());
        clock += 16_666_666;
        a.frame(clock);
        // Outside the dialog, over the page's rows: it closes, nothing is discarded or opened.
        press(&mut a, &mut clock, 200.0, 140.0);
        assert!(a.st.changes.discard.is_none() && a.st.changes.pending.is_empty() && a.st.changes.open.is_none());
        // Yes: the dialog's right key, in the middle of the screen.
        a.st.changes.discard = Some(f.clone());
        clock += 16_666_666;
        a.frame(clock);
        let mut found = None;
        for y in (300..560).step_by(4) {
            a.st.changes.discard = Some(f.clone());
            clock += 16_666_666;
            a.frame(clock);
            lift(&mut a, &mut clock, 290.0, y as f32);
            if a.st.changes.pending.iter().any(|p| p.job == Job::Discard(f.path.clone())) {
                found = Some(y);
                break;
            }
        }
        assert!(found.is_some(), "the dialog's Discard key sends the discard");
    }

    #[test]
    fn the_menu_commits_without_pushing() {
        let (mut a, mut clock) = on_changes("menu");
        a.st.changes.message = "One line".into();
        a.st.changes.menu = true;
        clock += 16_666_666;
        a.frame(clock);
        // The menu sits above the foot's keys, at the right.
        let ky = 844.0 - 132.0 + 12.0 + 46.0 + 12.0;
        lift(&mut a, &mut clock, 300.0, ky - 8.0 - 56.0 + 4.0 + 24.0);
        assert!(!a.st.changes.menu);
        let jobs: Vec<Job> = a.st.changes.pending.iter().map(|p| p.job.clone()).collect();
        assert!(jobs.contains(&Job::Commit { push: false, set_upstream: false }), "{jobs:?}");
    }

    #[test]
    fn a_long_message_grows_the_box_to_five_lines_and_many_branches_scroll() {
        let (mut a, mut clock) = on_changes("grow");
        a.st.changes.message = "Subject".into();
        assert_eq!(message_extra(&mut a), 0.0);
        a.st.changes.message = "Subject\n\nOne\nTwo".into();
        let lh = message_style().lh();
        assert!((message_extra(&mut a) - 3.0 * lh).abs() < 0.5);
        a.st.changes.message = (0..20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        assert!((message_extra(&mut a) - 4.0 * lh).abs() < 0.5, "five lines at most");
        a.st.changes.worktrees = (0..14).map(|i| Worktree { path: format!("/w/{i}"), branch: format!("relay/b{i}"), ..Worktree::default() }).collect();
        a.st.changes.picking = true;
        for _ in 0..2 {
            clock += 16_666_666;
            a.frame(clock);
        }
        assert!(a.st.changes.pick_scroll.max > 5.0 * 56.0, "the rows past eight scroll: {}", a.st.changes.pick_scroll.max);
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
        a.st.changes.open = Some((Group::Work, "same.sh".into()));
        answered(&mut a, Job::File(Group::Work, "same.sh".into()), 1, Some("/w/b".into()), Ok(json!({"old": "x", "new": "x", "hunks": []})));
        assert!(note(&a).contains("mode"));
        a.st.changes.open = Some((Group::Work, "empty.txt".into()));
        answered(&mut a, Job::File(Group::Work, "empty.txt".into()), 1, Some("/w/b".into()), Ok(json!({"old": "", "new": "", "hunks": []})));
        assert!(note(&a).contains("empty"));
        let too = rc_link::BusError { kind: "refused".into(), code: "git.diff_too_large".into(), message: "big".into(), hint: None };
        answered(&mut a, Job::File(Group::Work, "empty.txt".into()), 1, Some("/w/b".into()), Err(too));
        assert!(note(&a).contains("Too large"));
        // Binary files and new folders are known from git.diff and git.status: nothing to ask.
        assert!(no_lines(&File { path: "a.png".into(), binary: true, ..File::default() }).is_some_and(|w| w.contains("binary")));
        assert!(no_lines(&File { path: "new/".into(), index: "?".into(), ..File::default() }).is_some_and(|w| w.contains("folder")));
    }
}
