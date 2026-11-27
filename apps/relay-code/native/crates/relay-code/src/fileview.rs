//! A file, read only: from a row in Changes or a path in an agent's words. `file.read` from the
//! PC, line numbers down the side, code lightly coloured (`data::tokens`), long lines wrapped once
//! when they come, and only the rows on screen drawn. A path may end in `:line` (or
//! `:line:col`), which opens the file at that line; an absolute path is matched against the
//! repo's worktrees.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::App;
use crate::data::{self, Lang, Tok};
use crate::draw::{Rgba, Text, WIDTH, Weight};
use crate::theme::{self as t, ty};

/// The most of a file the phone reads.
const MAX_BYTES: u64 = 2 << 20;
/// The most lines wrapped and kept.
const MAX_LINES: usize = 20_000;

/// Open `path` of `repo` (in `worktree`, or the repo's own checkout when None) on a page of its
/// own. `path` is relative to that checkout or absolute, and may end in `:line` or `:line:col`.
pub fn open(app: &mut App, repo: i64, worktree: Option<String>, path: &str) {
    app.push(crate::app::Route::File { repo, worktree, path: path.to_string() });
}

/// Where a path asks to be opened: the path itself and the line (1-based), if it names one.
pub fn split_line(path: &str) -> (&str, Option<usize>) {
    let mut p = path;
    let mut nums: Vec<usize> = Vec::new();
    for _ in 0..2 {
        match p.rsplit_once(':') {
            Some((head, n)) if !head.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                nums.push(n.parse().unwrap_or(0));
                p = head;
            }
            _ => break,
        }
    }
    // path:line:col gives [col, line]; path:line gives [line].
    (p, nums.last().copied().filter(|n| *n > 0))
}

/// An absolute `path` as one of `roots` (worktree paths) and the path inside it: the longest root
/// it is under. None when it is under none of them.
pub fn inside(path: &str, roots: &[String]) -> Option<(String, String)> {
    roots
        .iter()
        .filter_map(|r| {
            let r = r.trim_end_matches('/');
            let rest = path.strip_prefix(r)?.strip_prefix('/')?;
            (!rest.is_empty()).then(|| (r.to_string(), rest.to_string()))
        })
        .max_by_key(|(r, _)| r.len())
}

#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    /// An absolute path: which worktree it is in.
    Finding(Call),
    Reading(Call),
    Ready,
    /// Why there is nothing to show: outside the repo, binary, or the PC's refusal.
    Note(String),
}

/// One line as drawn: wrapped pieces after the first carry no number.
struct Row {
    num: Option<usize>,
    indent: u16,
    pieces: Vec<(Tok, String)>,
}

#[derive(Default)]
pub struct State {
    /// The route it shows: (repo, worktree, path as given).
    key: Option<(i64, Option<String>, String)>,
    /// Where it is read from, once known.
    worktree: Option<String>,
    rel: String,
    line: Option<usize>,
    stage: Stage,
    text: Vec<String>,
    truncated: bool,
    rows: Vec<Row>,
    /// The row each line starts on, to open at a line.
    starts: Vec<usize>,
    jumped: bool,
}

impl State {
    fn busy(&self) -> bool {
        matches!(self.stage, Stage::Finding(_) | Stage::Reading(_))
    }
}

fn read(app: &mut App, repo: i64) {
    if !app.online() {
        return;
    }
    let st = &mut app.st.fileview;
    let payload = json!({"project_id": repo, "worktree": st.worktree, "path": st.rel, "max_bytes": MAX_BYTES});
    st.stage = Stage::Reading(app.link.call("file.read", payload));
}

/// Start on the route's file: work out where it is, then read it.
fn begin(app: &mut App, repo: i64, worktree: Option<String>, path: &str) {
    let (p, line) = split_line(path);
    let mut st = State { key: Some((repo, worktree.clone(), path.to_string())), line, ..State::default() };
    if p.starts_with('/') {
        // Under the worktree given: the rest of it. Otherwise ask which worktree it is under.
        if let Some((w, rel)) = worktree.as_ref().and_then(|w| inside(p, std::slice::from_ref(w))) {
            st.worktree = Some(w);
            st.rel = rel;
        } else {
            st.rel = p.to_string();
            if app.online() {
                st.stage = Stage::Finding(app.link.call("worktree.list", json!({"project_id": repo})));
            }
            app.st.fileview = st;
            return;
        }
    } else {
        st.worktree = worktree;
        st.rel = p.trim_start_matches("./").to_string();
    }
    app.st.fileview = st;
    read(app, repo);
}

fn take(app: &mut App, repo: i64) {
    let st = &mut app.st.fileview;
    let (finding, answer) = match &st.stage {
        Stage::Finding(c) => (true, c.poll()),
        Stage::Reading(c) => (false, c.poll()),
        _ => return,
    };
    let Some(r) = answer else { return };
    if finding { found(app, repo, r) } else { got(app, r) }
}

/// The repo's worktrees came: the absolute path is read from the one it is under, or not at all.
fn found(app: &mut App, repo: i64, r: Result<Value, rc_link::BusError>) {
    let st = &mut app.st.fileview;
    match r {
        Ok(v) => {
            let roots: Vec<String> = crate::changes::worktrees(&v).into_iter().map(|w| w.path).collect();
            match inside(&st.rel, &roots) {
                Some((w, rel)) => {
                    st.worktree = Some(w);
                    st.rel = rel;
                    read(app, repo);
                }
                None => st.stage = Stage::Note("This file is not in this repo, so the phone does not read it.".into()),
            }
        }
        Err(e) => st.stage = Stage::Note(e.message),
    }
}

/// The file came: its lines, or why there are none.
fn got(app: &mut App, r: Result<Value, rc_link::BusError>) {
    let st = &mut app.st.fileview;
    match r {
        Ok(v) => {
            st.truncated = v.get("truncated").and_then(Value::as_bool) == Some(true);
            match v.get("text").and_then(Value::as_str) {
                Some(text) => {
                    st.text = text.lines().take(MAX_LINES).map(str::to_string).collect();
                    st.rows.clear();
                    st.stage = Stage::Ready;
                }
                None => {
                    let size = v.get("size").and_then(Value::as_u64).unwrap_or(0);
                    st.stage = Stage::Note(format!("A binary file ({}): there is no text to show.", human(size)));
                }
            }
        }
        Err(e) => st.stage = Stage::Note(e.message),
    }
}

fn human(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{} KB", bytes / 1024),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// A piece of code's colour, as the chat colours code: few, quiet.
fn tok_ink(k: Tok) -> Rgba {
    match k {
        Tok::Plain => t::INK2,
        Tok::Keyword => t::BADGES[1],
        Tok::Str => Rgba::hex(0xD9B77E),
        Tok::Number => t::BADGES[4],
        Tok::Comment => t::INK3,
    }
}

/// The text as rows `w` wide (mono, `col` a column): each line's indent kept (a tab as four, at
/// most a third of the width), its number on its first row.
fn wrap(app: &mut App, w: f32, col: f32, st: Text) {
    let lang: Option<Lang> = data::lang_of(&app.st.fileview.rel);
    let text = std::mem::take(&mut app.st.fileview.text);
    let most = ((w / 3.0) / col.max(1.0)) as u16;
    let mut rows = Vec::new();
    let mut starts = Vec::with_capacity(text.len());
    for (i, l) in text.iter().enumerate() {
        starts.push(rows.len());
        let body = l.trim_start_matches([' ', '\t']);
        let lead = &l[..l.len() - body.len()];
        let indent = lead.chars().map(|c| if c == '\t' { 4u16 } else { 1 }).sum::<u16>().min(most);
        let segs = if body.is_empty() { vec![String::new()] } else { app.canvas.wrap(body, w - f32::from(indent) * col, st, 40) };
        for (k, seg) in segs.into_iter().enumerate() {
            let pieces = match lang {
                Some(lang) => data::tokens(&seg, lang),
                None => vec![(Tok::Plain, seg)],
            };
            rows.push(Row { num: (k == 0).then_some(i + 1), indent, pieces });
        }
    }
    let fv = &mut app.st.fileview;
    fv.text = text;
    fv.rows = rows;
    fv.starts = starts;
}

pub fn draw(app: &mut App, repo: i64, worktree: Option<String>, path: &str, dt: f32) -> bool {
    let key = (repo, worktree.clone(), path.to_string());
    if app.st.fileview.key.as_ref() != Some(&key) {
        begin(app, repo, worktree, path);
    } else if app.fresh && matches!(app.st.fileview.stage, Stage::Idle) {
        // Opened while the link was down: try again now.
        begin(app, repo, worktree, path);
    }
    take(app, repo);
    let page = app.page_begin(dt, 56.0, 0.0);
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let mut y = page.top + 8.0;
    // Its name, its folder under it.
    let rel = app.st.fileview.rel.clone();
    let (dir, name) = rel.rsplit_once('/').map_or(("", rel.as_str()), |(d, n)| (d, n));
    app.canvas.text_fit(name, x, y, w, Text::new(ty::SECTION, t::INK).semi().face(Weight::Mono));
    y += 30.0;
    if !dir.is_empty() {
        let st = Text::new(ty::SMALL, t::INK3);
        let dir = app.canvas.fit_start(dir, w, st);
        app.canvas.text(&dir, x, y, st);
        y += 24.0;
    }
    y += 10.0;
    let st = Text::new(ty::SMALL, t::INK2).face(Weight::Mono).line(1.5);
    match &app.st.fileview.stage {
        Stage::Ready => {
            let digit = app.canvas.measure("0", st);
            let n = app.st.fileview.text.len().max(1);
            let gutter = digit * (n.to_string().len() as f32) + 12.0;
            if app.st.fileview.rows.is_empty() && !app.st.fileview.text.is_empty() {
                wrap(app, w - 24.0 - gutter, digit, st);
            }
            y = code(app, &page, x, y, w, gutter, digit, st);
        }
        Stage::Note(words) => {
            let words = words.clone();
            y += app.canvas.para(&words, x, y, w, Text::new(ty::UI, t::INK3).line(1.45), 5) + 12.0;
        }
        _ => {
            let words = if app.online() { "Reading the file…" } else { "The file shows here once the PC answers." };
            app.canvas.text(words, x, y, Text::new(ty::UI, t::INK3));
            y += 32.0;
        }
    }
    app.page_end(&page, y + 12.0);
    crate::board::back_bar(app, "");
    page.moving || app.st.fileview.busy() && !crate::wake::has()
}

/// The code on its dark ground, the rows on screen only. Returns where it ends.
#[allow(clippy::too_many_arguments)]
fn code(app: &mut App, page: &crate::app::Page, x: f32, y: f32, w: f32, gutter: f32, col: f32, st: Text) -> f32 {
    let lh = st.lh();
    let fv = &app.st.fileview;
    let n = fv.rows.len();
    let h = n as f32 * lh + 16.0;
    // Open at the line asked for, once: its row near the top.
    if !fv.jumped
        && let Some(line) = fv.line
        && let Some(&row) = fv.starts.get(line.saturating_sub(1))
    {
        let target = y + 8.0 + row as f32 * lh - page.top - 3.0 * lh;
        app.st.fileview.jumped = true;
        let sc = app.scroll();
        sc.max = sc.max.max(target);
        sc.off = target.max(0.0);
    }
    let fv = &app.st.fileview;
    app.canvas.rect(x, y, w, h, 10.0, t::CODE);
    let (lo, hi) = (page.top - app.canvas.dy, page.bottom - app.canvas.dy);
    let first = (((lo - y - 8.0) / lh).floor().max(0.0)) as usize;
    let last = ((((hi - y - 8.0) / lh).ceil().max(0.0)) as usize).min(n);
    let target = fv.line;
    for (i, r) in fv.rows.iter().enumerate().take(last).skip(first) {
        let ly = y + 8.0 + i as f32 * lh;
        if r.num.is_some() && r.num == target {
            app.canvas.rect(x + 1.0, ly, w - 2.0, lh, 0.0, t::WASH);
        }
        if let Some(num) = r.num {
            app.canvas.text(&num.to_string(), x + 4.0 + gutter - 8.0, ly, st.color(t::INK3).right());
        }
        let mut px = x + 12.0 + gutter + f32::from(r.indent) * col;
        for (k, s) in &r.pieces {
            px += app.canvas.text(s, px, ly, st.color(tok_ink(*k)));
        }
    }
    let mut end = y + h;
    if fv.truncated {
        app.canvas.text(&format!("Shown up to {}.", human(MAX_BYTES)), x, end + 10.0, Text::new(ty::SMALL, t::INK3));
        end += 34.0;
    }
    end
}

/// A sample file for screenshots: opened at a line.
pub fn sample(app: &mut App) {
    let text = "use crate::app::App;\n\n/// The finger, on the layer it landed on.\npub fn tapped(&mut self, x: f32, y: f32, w: f32, h: f32) -> bool {\n    // A tap under a sheet reaches nothing.\n    if self.hidden((x, y)) {\n        return false;\n    }\n    let hit = Self::inside(self.tap, x, y, w, h);\n    hit && self.layer >= self.top_at(x, y)\n}\n";
    app.st.fileview = State {
        key: Some((1, None, "apps/relay-code/native/crates/relay-code/src/ui.rs:6".into())),
        rel: "apps/relay-code/native/crates/relay-code/src/ui.rs".into(),
        line: Some(6),
        stage: Stage::Ready,
        text: text.lines().map(str::to_string).collect(),
        jumped: true,
        ..State::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames drawn at rest on `route`, after `warm` to settle: µs a frame (release, this machine).
    fn frame_cost(a: &mut App, warm: usize, n: usize) -> f64 {
        let mut clock = 2_000_000_000u64;
        for _ in 0..warm {
            clock += 16_666_666;
            a.frame(clock);
        }
        let t = std::time::Instant::now();
        for _ in 0..n {
            clock += 16_666_666;
            a.frame(clock);
        }
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    }

    fn bench_app(name: &str) -> App {
        struct NoHost(std::path::PathBuf);
        impl crate::app::Host for NoHost {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _text: &str) {}
        }
        let dir = std::env::temp_dir().join(format!("relay-code-bench-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a
    }

    /// A 3000-line file open, at rest:
    /// `cargo test --release fileview_frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fileview_frame_cost() {
        let mut a = bench_app("fileview");
        let text: Vec<String> = (1..=3000).map(|i| format!("    let value_{i} = compute(&input[{i}..], \"{i}\"); // line {i}")).collect();
        a.st.fileview = State { key: Some((1, None, "src/big.rs".into())), rel: "src/big.rs".into(), stage: Stage::Ready, text, ..State::default() };
        open(&mut a, 1, None, "src/big.rs");
        println!("file view frame, 3000 lines: {:.0} µs", frame_cost(&mut a, 10, 200));
    }

    #[test]
    fn a_path_names_its_line_and_column_or_not() {
        assert_eq!(split_line("src/app.rs"), ("src/app.rs", None));
        assert_eq!(split_line("src/app.rs:120"), ("src/app.rs", Some(120)));
        assert_eq!(split_line("src/app.rs:120:9"), ("src/app.rs", Some(120)));
        assert_eq!(split_line("C:/x"), ("C:/x", None));
        assert_eq!(split_line(":12"), (":12", None));
        assert_eq!(split_line("a.rs:0"), ("a.rs", None));
    }

    #[test]
    fn an_absolute_path_finds_its_worktree_the_longest_first() {
        let roots = ["/home/a/RelayV4".to_string(), "/home/a/RelayV4/.relay/worktrees/bold-puffin".to_string()];
        assert_eq!(inside("/home/a/RelayV4/src/app.rs", &roots), Some(("/home/a/RelayV4".into(), "src/app.rs".into())));
        assert_eq!(inside("/home/a/RelayV4/.relay/worktrees/bold-puffin/src/app.rs", &roots), Some(("/home/a/RelayV4/.relay/worktrees/bold-puffin".into(), "src/app.rs".into())));
        assert_eq!(inside("/home/a/RelayV4x/src/app.rs", &roots), None, "a name that only starts the same");
        assert_eq!(inside("/etc/passwd", &roots), None);
    }

    struct NoHost(std::path::PathBuf);

    impl crate::app::Host for NoHost {
        fn data_dir(&self) -> std::path::PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    fn app(name: &str) -> (App, u64) {
        let dir = std::env::temp_dir().join(format!("relay-code-fileview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        (a, 2_000_000_000)
    }

    #[test]
    fn a_long_file_opens_at_its_line_and_draws_only_what_shows() {
        let (mut a, mut clock) = app("long");
        let text: Vec<String> = (1..=3000).map(|i| format!("    let value_{i} = compute(input_{i}, {i}); // line {i}")).collect();
        a.st.fileview = State {
            key: Some((1, None, "src/big.rs:2000".into())),
            rel: "src/big.rs".into(),
            line: Some(2000),
            stage: Stage::Ready,
            text,
            ..State::default()
        };
        open(&mut a, 1, None, "src/big.rs:2000");
        for _ in 0..3 {
            clock += 16_666_666;
            a.frame(clock);
        }
        let fv = &a.st.fileview;
        assert!(fv.rows.len() >= 3000 && fv.jumped);
        let row = fv.starts[1999] as f32;
        let off = a.scroll().off;
        assert!(off > row * 15.0, "opened down at line 2000: {off}");
        // A frame draws a screenful, not 3000 rows.
        let t0 = std::time::Instant::now();
        for _ in 0..20 {
            clock += 16_666_666;
            a.frame(clock);
        }
        assert!(t0.elapsed().as_millis() < 2000);
    }

    #[test]
    fn an_absolute_path_is_read_from_its_worktree_and_one_outside_is_not() {
        let list = json!({"worktrees": [{"path": "/home/a/RelayV4", "branch": "main"}, {"path": "/home/a/RelayV4/.relay/worktrees/x", "branch": "relay/x"}]});
        let (mut a, _) = app("outside");
        a.st.fileview = State { key: Some((1, None, "/etc/passwd".into())), rel: "/etc/passwd".into(), ..State::default() };
        found(&mut a, 1, Ok(list.clone()));
        assert!(matches!(&a.st.fileview.stage, Stage::Note(w) if w.contains("not in this repo")));
        a.st.fileview = State { key: Some((1, None, "/home/a/RelayV4/.relay/worktrees/x/src/a.rs".into())), rel: "/home/a/RelayV4/.relay/worktrees/x/src/a.rs".into(), ..State::default() };
        found(&mut a, 1, Ok(list));
        assert_eq!((a.st.fileview.worktree.as_deref(), a.st.fileview.rel.as_str()), (Some("/home/a/RelayV4/.relay/worktrees/x"), "src/a.rs"));
        // A binary file, and a text one.
        got(&mut a, Ok(json!({"bytes_b64": "AAAA", "mime": "image/png", "size": 2048, "truncated": false})));
        assert!(matches!(&a.st.fileview.stage, Stage::Note(w) if w.contains("binary") && w.contains("2 KB")));
        got(&mut a, Ok(json!({"text": "a\nb\n", "mime": "text/plain", "size": 4, "truncated": true})));
        assert!(matches!(a.st.fileview.stage, Stage::Ready) && a.st.fileview.text == ["a", "b"] && a.st.fileview.truncated);
    }
}
