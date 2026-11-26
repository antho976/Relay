//! The desktop build: a phone-shaped window for working on Leaf with the mouse and keyboard, and a
//! screenshot mode that needs no window.
//!
//! - `--shot out.png --screen NAME` draws one screen to a file, against a real server
//!   (`--server`, default `http://127.0.0.1:4580`; `--token`), on a data folder of its own that
//!   is removed after. Screens: `connect` (the first run, the address checked), `connect-empty`,
//!   `home` (`--hero N` for the spotlight's Nth, `--rail UNITS|end` Continue reading moved sideways, `--all-read-but N` every series finished on the shot's copy but the N read last), `library-empty` (the list answered empty),
//!   `space` (`--tab updates|recent|following|downloads`, `--sort 0..2` for Following, `--filter
//!   WORDS` typed in its field, `--shelf 0..4` for one shelf, `--shelve ID:dropped` to put a
//!   series on a shelf in the shot's copy, `--fake-updates 1` for a made-up Updates list built from
//!   the library, `--all 1` for Updates' All, `--sheet shelf` with `--id` for the shelf sheet,
//!   `--sheet select` for Downloads selecting its first two series), `saved` (`--id`: a series'
//!   downloads), `search` (`--query`, default "nano"; `--fake-search N`
//!   answers with N works made from the library instead of asking the server's sources; `--meant
//!   TITLE` offers "Did you mean TITLE?"), `series` (`--id`, default 1; `--preview 1` shows it as a
//!   title not added yet; `--tab also`; `--sheet shelf|wrong|menu|info|range|rec`; `--select N` selects N
//!   chapters from where you are; `--paid FROM-TO` shows paid episodes ahead on the shot's copy),
//!   `picks` (`--of top|binge|tag:<tag>|because:<id>`), `reader` (`--id`, `--chapter` number, default 1; `--scroll UNITS`
//!   down the strip; `--overlay 1`; `--auto 1` auto-scroll on; `--dim 0..0.85`; `--fake-pages 1`
//!   draws made-up pages instead of asking the server for any; `--sheet hide` the long press's
//!   sheet on the page at the top), `settings` (the
//!   Settings page, `--scroll UNITS`, `--backup 1` to export the latest backup, `--fold server,anilist` the address and AniList steps
//!   folded out).
//!   `--fake-recs 1` makes Home's picks (and a series' Readers also like) from the library, for the
//!   days the server has none (covers are the series' own, fetched once into `--data`);
//!   `--sheet rec` on `home` opens the first pick's sheet (`--sheet drag`: pulled down by a finger
//!   still on it); `--tagart DIR` takes the collections' pictures from `DIR/<slug>.webp`;
//!   `--update 1` offers a made-up newer app. `--sweep N` (`series`, `saved`, `space --tab
//!   downloads`): a long press on the first row on screen dragged down over N more, the finger
//!   still down. Pointed at a server that isn't there (`--server http://127.0.0.1:9`, with a
//!   `--data` folder an earlier shot filled), a page shows the offline prompt.
//!   `--fake-saved N` shows chapters as downloaded to this phone on the shot's copy (N series' in
//!   `space`; a few of `--id`'s on `series` and `saved`), their manifests written to the shot's
//!   folder with no page, so a later shot with `--data` and no server finds them;
//!   `--fake-saving 1` puts the next few in line, the first part way.
//!   Lists: `--fake-lists "*Wait list=100,98;Weekend=23"` (a * puts it on Home), screens `list`
//!   and `list-add` (`--id` the list), `space --tab lists`, `--list-sheet pick:ID|name|menu:ID|home:ID|row:LIST:ID`.
//!   `--size WxH` (default 1080x2340). `--bell ID` and `--seen ID` mark a series on the shot's own
//!   copy only (its bell on; read up to two chapters before its newest). Nothing is written to the server: no progress is reported.
//! - `--data DIR` keeps Leaf's files somewhere else than `~/.local/share/leaf` (and, with
//!   `--shot`, keeps the shot's files there instead of removing them).
//! - Books: `--books a.epub,b.txt` imports those files first (into the shot's own folder), and
//!   `--seed 1` adds highlights from the first book's own text, a note, reading time this week and
//!   a place a third of the way in (for shots of Notes, Review, Stats and the library's hero).
//!   Screens: `books` (the library), `imports`, `book` (`--at 0.0..1.0` through the book,
//!   `--scroll UNITS`, `--chrome 1`, `--sheet type|contents`, `--select 1`, `--popover 1`, `--theme
//!   0..3`, `--size-pt N`), `in-search` (`--query`), `notes`, `book-notes`, `review`, `find`
//!   (`--query`), `stats`.
//! - A window picks books from `LEAF_PICK` (paths separated by `:`), as Tally RS's `TALLY_PICK`.
//!
//! Keys: Escape goes back, Enter is the keyboard's Done, Backspace deletes.

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::app::{App, Host, Next, Route};
use crate::clock::now_ns;
use crate::gfx::{Gfx, Screen};
use crate::net::Server;
use crate::ui::{Phase, Touch};

struct DesktopHost {
    data: PathBuf,
}

impl Host for DesktopHost {
    fn data_dir(&self) -> PathBuf {
        self.data.clone()
    }
    fn log(&self, text: &str) {
        eprintln!("leaf: {text}");
    }
    /// The files named by `LEAF_PICK` (paths separated by `:`) are the ones picked.
    fn pick_books(&self) {
        let Some(list) = std::env::var_os("LEAF_PICK") else {
            self.log("pick books: set LEAF_PICK to their paths, separated by ':'");
            return;
        };
        let inbox = self.data.join("inbox");
        let list = list.to_string_lossy().into_owned();
        // Copied into the inbox off this thread, as the phone's picker does.
        let _ = std::thread::Builder::new().name("leaf-pick".into()).spawn(move || {
            let _ = std::fs::create_dir_all(&inbox);
            for (i, p) in list.split(':').filter(|p| !p.is_empty()).enumerate() {
                let src = PathBuf::from(p);
                let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
                let dest = inbox.join(format!("{}{i}-{name}", crate::shelf::now_ms()));
                if std::fs::copy(&src, &dest).is_ok() {
                    crate::shelf::deliver(dest, name);
                } else {
                    eprintln!("leaf: couldn't read {p}");
                }
            }
        });
    }
    fn copy_text(&self, text: &str) {
        eprintln!("leaf: copied {} characters", text.chars().count());
    }
    /// Saved under the data folder's `exports/`.
    fn share_file(&self, name: &str, _mime: &str, bytes: &[u8]) -> Option<String> {
        let dir = self.data.join("exports");
        let path = dir.join(name.replace('/', "-"));
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => Some(format!("Saved to {}", path.display())),
            Err(e) => Some(format!("Couldn't save it: {e}")),
        }
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn data_dir(args: &[String]) -> PathBuf {
    arg(args, "--data").map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("leaf")
    })
}

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(out) = arg(&args, "--shot") {
        if let Err(e) = shot(&args, &out) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let event_loop = EventLoop::<()>::with_user_event().build().expect("an event loop");
    let proxy: EventLoopProxy<()> = event_loop.create_proxy();
    crate::wake::set(Box::new(move || {
        let _ = proxy.send_event(());
    }));
    let mut window = Windowed { app: None, gfx: None, window: None, next: Next::Now, cursor: (0.0, 0.0), mouse_down: false, data: data_dir(&args) };
    event_loop.run_app(&mut window).expect("the event loop ran");
}

/// Picture bytes sent to the GPU while settling, reported with the shot: a number far above what
/// the screen shows means pictures are being evicted and sent again.
static UPLOADED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Draw frames until the work the screen asked for has landed (or `secs` pass).
fn settle(app: &mut App, gfx: &Gfx, clock: &mut u64, secs: f32) {
    let frames = (secs * 50.0) as usize;
    let mut quiet = 0;
    for _ in 0..frames {
        *clock += 16_666_666;
        app.frame(*clock);
        gfx.upload_glyphs(&mut app.canvas.uploads);
        UPLOADED.fetch_add(app.canvas.image_uploads.iter().map(|u| u.pixels.len()).sum::<usize>(), std::sync::atomic::Ordering::Relaxed);
        gfx.upload_images(&mut app.canvas.image_uploads);
        gfx.flush();
        let busy = app.jobs.running() > 0 || app.images.pending() || app.shelf.busy() || app.canvas.take_late_pictures() || app.book.as_ref().is_some_and(|b| b.doc.is_none() && b.error.is_none());
        quiet = if busy { 0 } else { quiet + 1 };
        if quiet >= 4 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

const BOOK_SCREENS: [&str; 10] = ["books", "imports", "book", "in-search", "notes", "book-notes", "review", "find", "stats", "books-empty"];

/// The book screens: import `--books`, seed them, and open the screen asked for.
fn books_shot(app: &mut App, gfx: &Gfx, clock: &mut u64, args: &[String], screen: &str) -> Result<(), String> {
    settle(app, gfx, clock, 10.0);
    let host = app.host.clone();
    if let Some(list) = arg(args, "--books") {
        let files: Vec<(PathBuf, String, bool)> = list
            .split(',')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let path = PathBuf::from(p);
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                (path, name, false)
            })
            .collect();
        app.shelf.import(files, host);
        if screen == "imports" {
            app.push(Route::Imports);
        }
        settle(app, gfx, clock, 60.0);
    }
    let first = app.shelf.books.iter().min_by_key(|b| b.id).map(|b| b.id);
    if arg(args, "--seed").is_some()
        && let Some(id) = first
    {
        seed(app, gfx, clock, id)?;
    }
    if let Some(theme) = arg(args, "--theme").and_then(|v| v.parse::<u8>().ok()) {
        let mut ty_ = crate::book::saved_type(app);
        ty_.theme = theme.min(3);
        if let Some(sz) = arg(args, "--size-pt").and_then(|v| v.parse::<f32>().ok()) {
            ty_.size = sz;
        }
        app.shelf.set_setting("type", serde_json::to_string(&ty_).unwrap_or_default());
    }
    let view_h = app.canvas.h;
    match screen {
        "books" | "books-empty" => app.reset_to(Route::Home),
        "imports" => {}
        "notes" => {
            app.reset_to(Route::Space);
            app.push(Route::Notes);
        }
        "stats" => {
            app.reset_to(Route::Space);
            app.push(Route::Stats);
        }
        "review" => {
            app.reset_to(Route::Space);
            app.push(Route::Notes);
            app.push(Route::Review);
        }
        "find" => {
            app.reset_to(Route::Home);
            app.push(Route::Find);
            app.frame(*clock);
            let q = arg(args, "--query").unwrap_or_else(|| "nature".into());
            app.focus.text = q.clone();
            app.st.find.query = q;
            app.finish_typing();
        }
        "book-notes" => {
            let id = first.ok_or("no book imported")?;
            app.reset_to(Route::Home);
            app.push(Route::BookNotes(id));
        }
        "book" | "in-search" => {
            let id = first.ok_or("no book imported")?;
            app.reset_to(Route::Home);
            app.open_book(id);
            settle(app, gfx, clock, 30.0);
            let r = app.book.as_mut().ok_or("the book didn't open")?;
            if let Some(f) = arg(args, "--at").and_then(|v| v.parse::<f32>().ok()) {
                r.shot_at(&mut app.canvas, &app.images, f, view_h);
            }
            // `--at hl`: a little above the third highlight, so it shows among its paragraphs.
            if arg(args, "--at").as_deref() == Some("hl")
                && let Some(h) = app.shelf.highlights.iter().filter(|h| h.book == id).nth(2)
            {
                let p = crate::shelf::Place { section: h.from.section, block: h.from.block, byte: 0 };
                r.go(&mut app.canvas, &app.images, p, view_h);
                r.scroll_by(&mut app.canvas, &app.images, -view_h * 0.3, view_h);
            }
            if let Some(units) = arg(args, "--scroll").and_then(|v| v.parse::<f32>().ok()) {
                r.scroll_by(&mut app.canvas, &app.images, units, view_h);
            }
            settle(app, gfx, clock, 10.0);
            let r = app.book.as_mut().ok_or("the book didn't open")?;
            if arg(args, "--chrome").is_some() {
                r.chrome = true;
            }
            if let Some(s) = arg(args, "--sheet") {
                r.open_sheet(&s);
            }
            if arg(args, "--select").is_some() {
                r.shot_select(&mut app.canvas, &app.images, view_h);
            }
            if arg(args, "--popover").is_some() && !r.shot_popover(&mut app.canvas, &app.images, view_h) {
                return Err("this book has no footnote links".into());
            }
            if screen == "in-search" {
                app.push(Route::InSearch);
                app.frame(*clock);
                let q = arg(args, "--query").unwrap_or_else(|| "nature".into());
                app.focus.text = q.clone();
                if let Some(r) = &mut app.book {
                    r.found.query = q;
                }
                app.finish_typing();
            }
        }
        _ => {}
    }
    settle(app, gfx, clock, 20.0);
    Ok(())
}

/// Highlights from the first book's own text, one with a note, reading time this week and last,
/// and its place a third of the way in: what a few days of reading leave.
fn seed(app: &mut App, gfx: &Gfx, clock: &mut u64, id: i64) -> Result<(), String> {
    let view_h = app.canvas.h;
    app.open_book(id);
    settle(app, gfx, clock, 30.0);
    let doc = app.book.as_ref().and_then(|b| b.doc.clone()).ok_or("the book didn't open")?;
    let now = crate::shelf::now_ms();
    let day = crate::shelf::DAY_MS;
    let mut made = 0;
    for (i, frac) in [0.06f32, 0.11, 0.17, 0.24, 0.29, 0.33, 0.41].iter().enumerate() {
        let target = (doc.chars as f32 * frac) as u64;
        let found = doc.sections.iter().enumerate().flat_map(|(s, sec)| sec.blocks.iter().enumerate().map(move |(b, bl)| (s, b, bl))).find(|(_, _, bl)| bl.at >= target && bl.kind == crate::epub::Kind::Para && bl.text.len() > 90);
        let Some((s, b, bl)) = found else { continue };
        let from = bl.text.char_indices().map(|(i, _)| i).find(|i| *i >= 40).unwrap_or(bl.text.len());
        let end = bl.text[from..].find(". ").map_or(bl.text.floor_char_boundary(160), |e| from + e + 1);
        let (chapter, ..) = doc.chapter_span(s as u32, b as u32);
        let created = now - (9 - i as i64) * day;
        let note = if i == 0 { "The whole book in one line. Worth rereading before each part.".to_string() } else if i == 3 { "Compare with Seneca on the same point.".to_string() } else { String::new() };
        app.shelf.add_highlight(crate::shelf::Highlight {
            id: 0,
            book: id,
            from: crate::shelf::Place { section: s as u32, block: b as u32, byte: 0 },
            to: crate::shelf::Place { section: s as u32, block: b as u32, byte: end as u32 },
            text: bl.text[..end].to_string(),
            colour: [0u8, 1, 2, 0, 3, 0, 2][i],
            note,
            chapter,
            created_ms: created,
            due_ms: if i < 5 { now - day } else { now + 5 * day },
            interval_d: 0,
            retired: false,
        });
        made += 1;
    }
    if made == 0 {
        return Err("no paragraphs long enough to highlight".into());
    }
    // This week and last, minutes a day.
    let today = crate::shelf::day_of(now);
    let monday = today - crate::shelf::weekday(today);
    for (d, mins) in [(-7, 22), (-6, 35), (-4, 18), (-3, 41), (-1, 30), (0, 26), (1, 12), (2, 44), (3, 31), (4, 52), (5, 38), (6, 47)] {
        if monday + d <= today {
            app.shelf.add_time(id, monday + d, mins * 60);
        }
    }
    if let Some(b) = app.shelf.book_mut(id) {
        b.cpm = 1150.0;
    }
    app.shelf.send(crate::shelf::Op::Speed { id, cpm: 1150.0 });
    if let Some(r) = &mut app.book {
        r.shot_at(&mut app.canvas, &app.images, 0.34, view_h);
    }
    settle(app, gfx, clock, 5.0);
    // Closing keeps the place (shots write nothing to the server, but the shelf is theirs).
    app.shot = false;
    app.pop();
    app.shot = true;
    settle(app, gfx, clock, 10.0);
    Ok(())
}

/// Updates made up from the library for a shot (the server only records arrivals from the day it
/// learnt to): each series' newest chapters, spread over the last few days, the older ones read.
/// `--fake-search N`: the search answered with N works named after the library's series (none
/// of the server's sources asked), the broad sources' answer, so "Search all sources" shows.
fn fake_search(app: &mut App, n: usize, gfx: &Gfx, clock: &mut u64) {
    // Asked already, as far as the page knows: it sends nothing while the library comes.
    let q = app.st.search.query.trim().to_string();
    app.st.search.asked = Some(q.clone());
    app.ask(crate::net::Job::Library);
    settle(app, gfx, clock, 20.0);
    // Titles with the words searched first, as the server would keep them.
    let mut lib = app.library.data.clone().unwrap_or_default();
    lib.sort_by_key(|s| !leaf_proto::title_has_words(&s.title, &q));
    let list: Vec<leaf_proto::Candidate> = lib
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, s)| leaf_proto::Candidate { title: s.title.clone(), cover_of: None, hits: (0..(i % 3 + 1)).map(|k| leaf_proto::Hit { manga_id: s.id * 10 + k as i64, source: format!("S{k}") }).collect(), followed: (i == 0).then_some(s.id), paid: (i == 1).then(|| leaf_proto::PaidAhead { from: 9.0, to: 23.0, url: String::new() }) })
        .collect();
    app.st.search.asked = Some(q.clone());
    app.st.search.all = false;
    app.st.search.more = true;
    app.st.search.answered(q, false, Ok(list), true);
}

/// Chapters shown as downloaded to this phone, on the shot's copy: `n` series' a few each (or, with
/// `only`, four of that one's, under its newest). Their manifests go to the shot's folder (no
/// pages), so a shot after with no server finds them.
fn fake_saved(app: &mut App, dir: &std::path::Path, n: usize, only: Option<i64>) {
    let ids: Vec<i64> = match only {
        Some(id) => vec![id],
        None => app.library.data.iter().flatten().map(|s| s.id).filter(|id| app.series.get(id).is_some_and(|l| l.data.is_some())).take(n).collect(),
    };
    let now = crate::shelf::now_ms();
    let mut list = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let Some(d) = app.series.get(id).and_then(|l| l.data.clone()) else { continue };
        let k = d.chapters.len();
        let per = if only.is_some() { 4 } else { 3 + (i * 5) % 9 };
        let from = k.saturating_sub(1 + per);
        for (j, c) in d.chapters[from..k.saturating_sub(1)].iter().enumerate() {
            let pages = if c.pages > 0 { c.pages } else { 30 + (c.number as u32 * 7) % 25 };
            let bytes = 4_200_000 + (c.number * 137_000.0) as u64 % 3_100_000;
            list.push(crate::offline::Manifest { series: *id, title: d.series.title.clone(), chapter: c.clone(), pages, skipped: vec![], bytes, saved_at: now - (i as i64) * 3 * 3_600_000 - j as i64 * 60_000 });
        }
    }
    for m in &list {
        let _ = crate::net::write_atomic(&crate::offline::chapter_dir(dir, m.chapter.id).join("manifest.json"), &serde_json::to_vec(m).unwrap_or_default());
    }
    app.saver.shot_saved(list);
}

/// The three chapters under the saved ones in line on the shot's copy, the first part way.
fn fake_saving(app: &mut App, id: i64) {
    let Some(d) = app.series.get(&id).and_then(|l| l.data.clone()) else { return };
    let items: Vec<crate::offline::Item> = d
        .chapters
        .iter()
        .rev()
        .filter(|c| !app.saver.is_saved(c.id))
        .skip(1)
        .take(3)
        .map(|c| crate::offline::Item { series: id, title: d.series.title.clone(), chapter: c.clone(), asked: true, failed: None })
        .collect();
    app.saver.shot_line(items, 12, 40);
}

fn fake_updates(app: &mut App) {
    let now = crate::shelf::now_ms();
    let hour = 3_600_000;
    let mut list = Vec::new();
    let mut lib = app.library.data.clone().unwrap_or_default();
    lib.sort_by_key(|s| std::cmp::Reverse(s.last_read_at));
    for (k, s) in lib.iter().filter(|s| app.series.get(&s.id).is_some_and(|l| l.data.is_some())).take(6).enumerate() {
        let Some(d) = app.series.get(&s.id).and_then(|l| l.data.as_ref()) else { continue };
        for (j, c) in d.chapters.iter().rev().take(3).enumerate() {
            let ago = (k as i64 * 7 + j as i64 * 19) * hour + 25 * 60_000;
            list.push(leaf_proto::Update { series: s.id, title: s.title.clone(), chapter: c.id, number: c.number, name: c.name.clone(), at: now - ago, read: j >= 2 || c.read, bell: s.bell });
        }
    }
    list.sort_by_key(|u| std::cmp::Reverse(u.at));
    app.updates.data = Some(list);
    app.updates.loading = false;
}

/// Picks made up from the library for a shot (the server makes its own once a day, from its
/// first day on): each a series you have, shown as if it were not yours, its cover copied from
/// the series' own (fetched once into the data folder). Readers also like for `series` too, two
/// of them marked yours.
fn fake_recs(app: &mut App, series: i64) -> Result<(), String> {
    let lib = app.library.data.clone().ok_or("the library did not load")?;
    let server = app.server.clone().ok_or("no server")?;
    let client = crate::net::Client::new();
    let mut lib: Vec<leaf_proto::Series> = lib.into_iter().filter(|s| s.id != series).collect();
    lib.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
    let reasons = ["More like Nano Machine", "3 of yours point here", "Murim · finished", "More like Return of the Mount Hua Sect", "2 of yours point here", "Regression · ongoing"];
    let genres = [["Action", "Murim"], ["Fantasy", "Regression"], ["Action", "Martial Arts"], ["Drama", "Romance"], ["Action", "Hunters"], ["Fantasy", "Magic"]];
    let mut recs = Vec::new();
    for (i, s) in lib.iter().take(18).enumerate() {
        let anilist = 900_000 + s.id;
        let file = crate::images::CoverOf::Rec(anilist).file(&app.data);
        if !file.exists() {
            let own = crate::images::CoverOf::Series(s.id).file(&app.data);
            let bytes = match std::fs::read(&own) {
                Ok(b) => b,
                Err(_) => client.get(&server, &format!("/v1/series/{}/cover", s.id)).map_err(|f| f.words("the server"))?,
            };
            crate::net::write_atomic(&file, &bytes).map_err(|e| e.to_string())?;
        }
        recs.push(leaf_proto::Rec {
            anilist_id: anilist,
            title: s.title.clone(),
            reason: reasons[i % reasons.len()].into(),
            genres: genres[i % genres.len()].iter().map(|g| g.to_string()).collect(),
            chapters: Some(s.chapters),
            status: s.status.clone(),
            hits: vec![leaf_proto::Hit { manga_id: -1, source: String::new() }; 1 + i % 3],
            cover: format!("/v1/recs/cover/{anilist}"),
            in_space: None,
            description: None,
        });
    }
    let pick = |from: usize, n: usize| -> Vec<leaf_proto::Rec> { recs.iter().skip(from).take(n).cloned().collect() };
    let read: Vec<&leaf_proto::Series> = app.library.data.as_ref().map(|l| l.iter().filter(|s| s.last_read_at > 0).collect()).unwrap_or_default();
    let because: Vec<leaf_proto::Because> = read.iter().take(2).enumerate().map(|(k, s)| leaf_proto::Because { series: s.id, title: s.title.clone(), recs: pick(6 + k * 4, 5), finished: false }).collect();
    let collections = ["Murim", "Magic", "Hunters", "Necromancy"].iter().enumerate().map(|(k, tag)| leaf_proto::Collection { tag: tag.to_string(), recs: pick(k * 3, 6) }).collect();
    let mut binge = recs.iter().find(|r| r.status == "completed").cloned().or_else(|| recs.last().cloned());
    if let Some(b) = &mut binge {
        b.reason = "Finished, and 9 in 10 of its readers rate it well".into();
    }
    app.recs.data = Some(leaf_proto::Recs { top: pick(0, 9), because, collections, binge, made_at: crate::shelf::now_ms() });
    app.recs.loading = false;
    let mut also = pick(3, 9);
    for (k, r) in also.iter_mut().enumerate() {
        if k == 1 || k == 4 {
            r.in_space = Some(r.anilist_id - 900_000);
        }
    }
    app.also.insert(series, crate::app::Loaded { data: Some(also), live: true, error: None, loading: false });
    Ok(())
}

/// One frame of a screen, drawn with no window.
fn shot(args: &[String], out: &str) -> Result<(), String> {
    let (w, h) = arg(args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))))
        .unwrap_or((1080, 2340));
    let screen = arg(args, "--screen").unwrap_or_else(|| "library".into());
    // A folder of its own, removed after, unless `--data` names one to keep (to shoot Offline:
    // once with the server, then again with `--server` pointing nowhere).
    let kept = arg(args, "--data").map(PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("leaf-shot-{}", std::process::id())));
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut gfx = Gfx::headless()?;
    let mut clock = 1_000_000_000u64;
    let mut app = App::new(Arc::new(DesktopHost { data: dir.clone() }), clock);
    app.shot = true;
    app.canvas.set_images_size(gfx.images_size());
    // Roughly a phone's status bar and gesture bar.
    app.resize(w as f32, h as f32, w as f32 * 0.085, w as f32 * 0.04);
    let base = arg(args, "--server").unwrap_or_else(|| "http://127.0.0.1:4580".into());
    let server = Server { base: crate::logic::normalize_server(&base).map_err(|e| e.to_string())?, token: arg(args, "--token"), alt: None };
    let id = arg(args, "--id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(1);
    // `--fake-lists "*Wait list=100,98;Weekend=23"`: lists on the shot's own copy (a * shows it on
    // Home); `--list-sheet pick:ID|name|menu:ID|row:LIST:ID` opens a list's sheet over the page.
    if let Some(spec) = arg(args, "--fake-lists") {
        let lists = spec
            .split(';')
            .enumerate()
            .map(|(i, part)| {
                let (name, ids) = part.split_once('=').unwrap_or((part, ""));
                let home = name.starts_with('*');
                let series = ids.split(',').filter_map(|v| v.trim().parse::<i64>().ok()).collect();
                leaf_proto::List { id: i as i64 + 1, name: name.trim_start_matches('*').to_string(), series, home, made_at: 0 }
            })
            .collect();
        app.lists.data = Some(lists);
    }
    if let Some(sheet) = arg(args, "--list-sheet") {
        let parts: Vec<&str> = sheet.split(':').collect();
        let n = |i: usize| parts.get(i).and_then(|v| v.parse::<i64>().ok()).unwrap_or(1);
        app.st.lists.sheet = Some(match parts[0] {
            "pick" => crate::lists::Sheet::Pick(n(1), "This series".into()),
            "name" => crate::lists::Sheet::Name { rename: None, add: None },
            "menu" => crate::lists::Sheet::Menu(n(1)),
            "home" => crate::lists::Sheet::Home(n(1)),
            _ => crate::lists::Sheet::Row(n(1), n(2)),
        });
    }
    match screen.as_str() {
        "connect-empty" => {}
        "connect" => {
            app.st.connect.address = crate::logic::server_label(&server.base);
            app.st.connect.token = server.token.clone().unwrap_or_default();
            crate::connect::start(&mut app);
        }
        "home" | "library" | "library-empty" => {
            app.set_server(server);
            app.reset_to(Route::Home);
            app.st.home.shot_spot = arg(args, "--hero").and_then(|v| v.parse::<usize>().ok());
        }
        "settings" => {
            app.set_server(server);
            app.reset_to(Route::Home);
            app.push(Route::Connect);
            app.st.connect.load(app.server.as_ref());
            // `--fold server,anilist`: the server's fields and AniList's steps folded out.
            let fold = arg(args, "--fold").unwrap_or_default();
            app.st.connect.editing = fold.contains("server");
            app.st.connect.anilist_open = fold.contains("anilist");
        }
        "saved" => {
            app.set_server(server);
            app.st.space.tab = crate::space::DOWNLOADS;
            app.reset_to(Route::Space);
        }
        "space" => {
            app.set_server(server);
            app.st.space.tab = match arg(args, "--tab").as_deref() {
                Some("updates") => crate::space::UPDATES,
                Some("following") => crate::space::FOLLOWING,
                Some("downloads") => crate::space::DOWNLOADS,
                Some("lists") => crate::space::LISTS,
                _ => crate::space::RECENT,
            };
            app.st.space.query = arg(args, "--filter").unwrap_or_default();
            app.st.space.shelf = arg(args, "--shelf").and_then(|v| v.parse::<usize>().ok()).and_then(|i| crate::logic::Shelf::ALL.get(i).copied());
            app.updates_all = arg(args, "--all").is_some();
            app.following_sort = arg(args, "--sort").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0).min(2);
            app.reset_to(Route::Space);
        }
        "list" | "list-add" => {
            app.set_server(server);
            app.reset_to(Route::Space);
            app.push(if screen == "list" { Route::List(id) } else { Route::ListAdd(id) });
        }
        "search" => {
            app.set_server(server);
            app.reset_to(Route::Search);
            app.frame(clock);
            app.focus.id = Some("search");
            app.st.search.query = arg(args, "--query").unwrap_or_else(|| "nano".into());
            app.focus.text = app.st.search.query.clone();
            match arg(args, "--fake-search").and_then(|v| v.parse::<usize>().ok()) {
                Some(n) => fake_search(&mut app, n, &gfx, &mut clock),
                None => app.finish_typing(),
            }
            // `--meant TITLE`: "Did you mean" offers it (with `--fake-search 0`).
            if let Some(m) = arg(args, "--meant") {
                settle(&mut app, &gfx, &mut clock, 5.0);
                let q = app.st.search.asked.clone().unwrap_or_default();
                app.st.search.suggest = Some((q, Some(m)));
            }
        }
        "series" | "reader" => {
            app.set_server(server);
            app.reset_to(Route::Home);
            app.push(Route::Series(id));
            app.frame(clock);
            app.st.series.also = arg(args, "--tab").as_deref() == Some("also");
        }
        "picks" => {
            app.set_server(server);
            app.reset_to(Route::Home);
            app.push(Route::Picks(arg(args, "--of").unwrap_or_else(|| "top".into())));
        }
        // `--period month:2026-11|year:2026 --card N`: one of Wrapped's cards.
        "wrapped" => {
            app.set_server(server);
            app.reset_to(Route::Home);
            let key = arg(args, "--period").unwrap_or_else(|| "month:2026-11".into());
            app.st.wrapped.period = crate::wrapped_stats::Period::parse(&key);
            app.st.wrapped.at = arg(args, "--card").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            app.push(Route::Wrapped(key));
        }
        _ if BOOK_SCREENS.contains(&screen.as_str()) => {
            books_shot(&mut app, &gfx, &mut clock, args, &screen)?;
        }
        other => return Err(format!("unknown screen: {other}")),
    }
    settle(&mut app, &gfx, &mut clock, 30.0);
    // Shot-only marks, made on the phone's copy and never sent: `--bell ID` shows a series' bell
    // on; `--seen ID` makes it read up to two chapters before its newest, a moment before they came
    // (to see NEW and the spotlight's news).
    let marks = [arg(args, "--bell").and_then(|v| v.parse::<i64>().ok()), arg(args, "--seen").and_then(|v| v.parse::<i64>().ok())];
    if marks.iter().any(Option::is_some) {
        let mark = |s: &mut leaf_proto::Series, d: Option<&mut leaf_proto::SeriesDetail>| {
            if Some(s.id) == marks[0] {
                s.bell = true;
            }
            if Some(s.id) == marks[1] {
                let latest = s.latest.unwrap_or(0.0);
                s.read_upto = Some(latest - 2.0);
                s.unread = 2;
                s.last_read_at = s.updated_at - 60_000;
                s.added_at = s.added_at.min(s.updated_at - 120_000);
                if let Some(d) = d {
                    for c in &mut d.chapters {
                        c.read = c.number <= latest - 2.0;
                        if c.number > latest - 2.0 {
                            c.uploaded_at = s.updated_at;
                        }
                    }
                }
            }
        };
        if let Some(list) = &mut app.library.data {
            for s in list.iter_mut() {
                let d = app.series.get_mut(&s.id).and_then(|l| l.data.as_mut());
                if let Some(d) = d {
                    mark(s, Some(d));
                    d.series = s.clone();
                } else {
                    mark(s, None);
                }
            }
        }
        settle(&mut app, &gfx, &mut clock, 5.0);
    }
    // `--shelve ID:shelf`, several separated by commas: the shot's copy only.
    if let Some(list) = arg(args, "--shelve") {
        for pair in list.split(',') {
            if let Some((sid, sh)) = pair.split_once(':')
                && let (Ok(sid), Some(sh)) = (sid.parse::<i64>(), crate::logic::Shelf::from_key(sh))
                && let Some(s) = app.library.data.as_mut().and_then(|l| l.iter_mut().find(|s| s.id == sid))
            {
                s.shelf = Some(sh.key().into());
            }
        }
        settle(&mut app, &gfx, &mut clock, 10.0);
    }
    if arg(args, "--fake-recs").is_some() {
        fake_recs(&mut app, id)?;
        // `--tagart DIR`: the collections' pictures from a folder (`<slug>.webp`), as if the
        // server had sent them; a tag with none there shows its collage.
        if let Some(src) = arg(args, "--tagart") {
            let tags: Vec<String> = app.recs.data.iter().flat_map(|r| r.collections.iter().map(|c| c.tag.clone())).collect();
            for tag in tags {
                let slug = crate::images::tag_slug(&tag);
                if let Ok(b) = std::fs::read(std::path::Path::new(&src).join(format!("{slug}.webp"))) {
                    crate::net::write_atomic(&dir.join("covers").join(format!("tag-{slug}.webp")), &b).map_err(|e| e.to_string())?;
                }
            }
        }
        settle(&mut app, &gfx, &mut clock, 30.0);
    }
    if let Some((from, to)) = arg(args, "--paid").and_then(|v| v.split_once('-').and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)))) {
        app.paid.insert(id, leaf_proto::PaidAhead { from, to, url: "https://www.webtoons.com/".into() });
    }
    if arg(args, "--update").is_some() {
        let build = crate::update::BUILD + 3;
        crate::update::offered(&mut app, leaf_proto::AppLatest { build, version: crate::update::version_of(build), size: 18_400_000, sha256: String::new(), made_at: 0 });
    }
    if screen == "home"
        && matches!(arg(args, "--sheet").as_deref(), Some("rec" | "drag"))
        && let Some(r) = app.recs.data.as_ref().and_then(|r| r.top.first().cloned())
    {
        crate::recs::hit(&mut app, &r, None, true);
        // `--sheet drag`: a finger pulling it down 140 units, still down as the shot is taken.
        if arg(args, "--sheet").as_deref() == Some("drag") {
            settle(&mut app, &gfx, &mut clock, 5.0);
            let u = app.canvas.u;
            let x0 = ((w as f32 - crate::draw::WIDTH * u) * 0.5).max(0.0);
            let (fx, fy) = (x0 + 200.0 * u, app.canvas.h - 200.0);
            app.touch(Touch { id: 1, phase: Phase::Down, x: fx, y: fy * u, t_ns: clock });
            for i in 1..=8u64 {
                clock += 16_666_666;
                app.touch(Touch { id: 1, phase: Phase::Move, x: fx, y: (fy + 140.0 * i as f32 / 8.0) * u, t_ns: clock });
                app.frame(clock);
            }
        }
    }
    // `--preview 1` on `series`: the series shown as a title not added yet (its page from
    // Search or a pick), from the shot's copy: nothing read, no bell, Follow and the download key.
    if screen == "series" && arg(args, "--preview").is_some() {
        let mut d = app.series.get(&id).and_then(|l| l.data.clone()).ok_or("the series did not load")?;
        let pid = -(1_000_000 + id);
        d.series.id = pid;
        d.series.bell = false;
        d.series.read_upto = None;
        d.series.last_read_at = 0;
        d.series.shelf = None;
        for c in &mut d.chapters {
            c.read = false;
            c.last_page = 0;
        }
        if let Ok(b) = std::fs::read(crate::images::CoverOf::Series(id).file(&dir)) {
            let _ = crate::net::write_atomic(&crate::images::CoverOf::Series(pid).file(&dir), &b);
        }
        app.previews.insert(pid, crate::app::Preview { title: d.series.title.clone(), hits: vec![leaf_proto::Hit { manga_id: 1_000_000 + id, source: String::new() }], anilist: None, ..Default::default() });
        app.series.insert(pid, crate::app::Loaded { data: Some(d), live: true, error: None, loading: false });
        app.push(Route::Series(pid));
        settle(&mut app, &gfx, &mut clock, 20.0);
        // The server this shot reads has no preview of a made-up entry: its answer is not shown.
        if let Some(l) = app.series.get_mut(&pid) {
            l.error = None;
            l.live = true;
        }
        settle(&mut app, &gfx, &mut clock, 5.0);
    }
    if let Some(n) = arg(args, "--select").and_then(|v| v.parse::<usize>().ok()) {
        let d = app.series.get(&id).and_then(|l| l.data.clone()).ok_or("the series did not load")?;
        let from = crate::logic::continue_point(&d.chapters).map_or(0, |c| c.index);
        app.st.series.select = Some(d.chapters.iter().rev().skip(d.chapters.len().saturating_sub(from + 1)).take(n).map(|c| c.id).collect());
    }
    if let Some(n) = arg(args, "--fake-saved").and_then(|v| v.parse::<usize>().ok()) {
        let only = matches!(screen.as_str(), "series" | "saved").then_some(id);
        fake_saved(&mut app, &dir, n, only);
        if arg(args, "--fake-saving").is_some() {
            fake_saving(&mut app, id);
        }
        settle(&mut app, &gfx, &mut clock, 5.0);
    }
    if screen == "saved" {
        app.push(Route::Saved(id));
        settle(&mut app, &gfx, &mut clock, 10.0);
    }
    if arg(args, "--fake-updates").is_some() {
        fake_updates(&mut app);
        settle(&mut app, &gfx, &mut clock, 20.0);
    }
    // `--backup 1`: Settings exports the latest backup (a read; the desktop saves it under the
    // shot's own folder).
    if screen == "settings" && arg(args, "--backup").is_some() {
        app.backing_up = true;
        app.ask(crate::net::Job::Backup(false));
        settle(&mut app, &gfx, &mut clock, 30.0);
    }
    if screen == "space" {
        match arg(args, "--sheet").as_deref() {
            Some("shelf") => app.st.space.shelf_for = Some(id),
            Some("select") => app.st.space.select = Some(app.saver.groups().iter().take(2).map(|g| g.series).collect()),
            _ => {}
        }
    }
    if screen == "series"
        && let Some(sheet) = arg(args, "--sheet")
    {
        let st = &mut app.st.series;
        match sheet.as_str() {
            "menu" => st.menu = true,
            "shelf" => st.shelf = true,
            "wrong" => {
                st.alternatives = true;
                app.ask(crate::net::Job::Alternatives(id));
            }
            "info" => st.info = true,
            "zoom" => st.zoom = Some(crate::zoom::Zoom::new(crate::images::CoverOf::Series(id))),
            "range" => {
                st.range = true;
                st.range_from = "12".into();
                st.range_to = "30".into();
            }
            "rec" => {
                if let Some(r) = app.also.get(&id).and_then(|l| l.data.as_ref()).and_then(|l| l.iter().find(|r| r.in_space.is_none()).cloned()) {
                    crate::recs::hit(&mut app, &r, None, true);
                }
            }
            other => return Err(format!("unknown sheet: {other}")),
        }
        settle(&mut app, &gfx, &mut clock, 30.0);
    }
    if screen == "library-empty" {
        // What a first library looks like, before anything is followed (the owner's server
        // follows two series already).
        app.library.data = Some(Vec::new());
    }
    if screen == "reader" {
        let d = app.series.get(&id).and_then(|l| l.data.clone()).ok_or("the series did not load")?;
        let number = arg(args, "--chapter").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
        let index = d.chapters.iter().position(|c| c.number == number).ok_or(format!("no chapter {number}"))?;
        let fake = arg(args, "--fake-pages").is_some();
        app.images.set_fake_pages(fake);
        let mut r = crate::reader::Reader::open(&d, index, 0);
        if fake {
            r.shot_counts(12);
        }
        // Next up at the strip's end, from the library (as `App::open_chapter` does).
        if app.library.data.is_none() {
            app.ask(crate::net::Job::Library);
            settle(&mut app, &gfx, &mut clock, 30.0);
        }
        if let Some(list) = &app.library.data {
            r.next_up = crate::logic::next_up(list, id, |s| crate::logic::in_space(s, false)).map(|(s, n)| (s.clone(), n));
        }
        app.reader = Some(r);
        app.push(Route::Reader);
        settle(&mut app, &gfx, &mut clock, 30.0);
        if let Some(units) = arg(args, "--scroll").and_then(|s| s.parse::<f32>().ok()) {
            // Down in steps, as a finger would, so the pages on the way are decoded and measured.
            let mut left = units;
            while left > 0.0 {
                let step = left.min(400.0);
                let hgt = app.canvas.h;
                if let Some(r) = &mut app.reader {
                    r.scroll_by(&app.images, step, hgt);
                }
                left -= step;
                settle(&mut app, &gfx, &mut clock, 10.0);
            }
        }
        if arg(args, "--overlay").is_some()
            && let Some(r) = &mut app.reader
        {
            r.overlay = true;
        }
        if arg(args, "--auto").is_some()
            && let Some(r) = &mut app.reader
        {
            r.auto = true;
        }
        if arg(args, "--sheet").as_deref() == Some("hide")
            && let Some(r) = &mut app.reader
            && let crate::strip::Item::Page(c, n) = r.pos.item
        {
            r.hide_sheet = Some((c, n));
        }
        if let Some(d) = arg(args, "--dim").and_then(|v| v.parse::<f32>().ok()) {
            app.prefs.dim = d.clamp(0.0, crate::app::DIM_MAX);
        }
        settle(&mut app, &gfx, &mut clock, 10.0);
    }
    if screen != "reader"
        && screen != "book"
        && let Some(units) = arg(args, "--scroll").and_then(|s| s.parse::<f32>().ok())
    {
        app.scroll().off = units;
        settle(&mut app, &gfx, &mut clock, 10.0);
    }
    // `--all-read-but N`: on the shot's copy only, every series read to its end but the N read
    // last (to see finished ones fill Continue reading, labelled Read).
    if let Some(keep) = arg(args, "--all-read-but").and_then(|v| v.parse::<usize>().ok()) {
        // Every series' chapters in first, so none arrives after to undo the marks.
        settle(&mut app, &gfx, &mut clock, 20.0);
        let mut list = app.library.data.clone().unwrap_or_default();
        list.sort_by_key(|s| std::cmp::Reverse(s.last_read_at));
        let shot_now = crate::shelf::now_ms();
        for (i, s) in list.iter_mut().filter(|s| s.read_upto.is_some() || s.last_read_at > 0).enumerate().skip(keep) {
            s.read_upto = s.latest;
            // Read lately, an hour apart, so they line up after the ones still going.
            if s.last_read_at == 0 {
                s.last_read_at = shot_now - (i as i64) * 3_600_000 - 86_400_000;
            }
            s.unread = 0;
            if let Some(d) = app.series.get_mut(&s.id).and_then(|l| l.data.as_mut()) {
                d.chapters.iter_mut().for_each(|c| c.read = true);
                d.series = s.clone();
            }
        }
        app.library.data = Some(list);
        clock += 16_666_666;
        app.frame(clock);
    }
    // `--rail UNITS`: Home's Continue reading moved sideways (`end` for its last covers).
    if screen == "home"
        && let Some(v) = arg(args, "--rail")
    {
        for _ in 0..3 {
            clock += 16_666_666;
            app.frame(clock);
        }
        let max = app.st.home.rail.max.max(0.0);
        app.st.home.rail.off = if v == "end" { max } else { v.parse::<f32>().unwrap_or(0.0).min(max) };
        for _ in 0..3 {
            clock += 16_666_666;
            app.frame(clock);
        }
    }
    // `--sweep N`: a long press on the first row on screen of a list that selects, dragged down
    // over N more without lifting (the finger is still down in the shot).
    if let Some(n) = arg(args, "--sweep").and_then(|v| v.parse::<usize>().ok()) {
        let off = app.scroll().off;
        let rows = app.sweep.rows.clone();
        let top = app.canvas.inset_top + 120.0;
        let k = rows.iter().position(|r| r.y - off >= top).ok_or("no row to sweep on this screen")?;
        let (from, to) = (rows[k], rows[(k + n).min(rows.len() - 1)]);
        let u = app.canvas.u;
        let x = ((app.canvas.px_w - crate::draw::WIDTH * u) * 0.5).max(0.0) + 200.0 * u;
        let at = |r: crate::sweep::Row| (r.y - off + r.h / 2.0) * u;
        let finger = |app: &mut App, phase: crate::ui::Phase, y: f32, clock: &mut u64, wait: u64| {
            app.touch(crate::ui::Touch { id: 1, phase, x, y, t_ns: *clock });
            *clock += 16_666_666 + wait;
            app.frame(*clock);
        };
        finger(&mut app, crate::ui::Phase::Down, at(from), &mut clock, 0);
        finger(&mut app, crate::ui::Phase::Move, at(from), &mut clock, crate::series::LONG_PRESS_NS);
        for i in 1..=12 {
            let y = at(from) + (at(to) - at(from)) * i as f32 / 12.0;
            finger(&mut app, crate::ui::Phase::Move, y, &mut clock, 0);
        }
    }
    clock += 16_666_666;
    app.frame(clock);
    gfx.upload_glyphs(&mut app.canvas.uploads);
    gfx.upload_images(&mut app.canvas.image_uploads);
    let pixels = gfx.render_to_pixels(w, h, &app.canvas.globals, app.clear_colour(), &app.canvas.insts, &app.canvas.batches);
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&pixels).map_err(|e| e.to_string())?;
    println!("{out}: {screen}, {w}x{h}, {} shapes in {} batches, {} MB of pictures uploaded, adapter {}", app.canvas.insts.len(), app.canvas.batches.len(), UPLOADED.load(std::sync::atomic::Ordering::Relaxed) >> 20, gfx.adapter_name());
    Ok(())
}

struct Windowed {
    app: Option<App>,
    gfx: Option<(Gfx, Screen<'static>)>,
    window: Option<Arc<Window>>,
    next: Next,
    cursor: (f32, f32),
    mouse_down: bool,
    data: PathBuf,
}

impl Windowed {
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler<()> for Windowed {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Leaf").with_inner_size(LogicalSize::new(412.0, 900.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("a window"));
        let size = window.inner_size();
        let (gfx, screen) = Gfx::with_surface(window.clone(), size.width, size.height).expect("graphics");
        let mut app = App::new(Arc::new(DesktopHost { data: self.data.clone() }), now_ns());
        app.canvas.set_images_size(gfx.images_size());
        app.resize(size.width as f32, size.height as f32, 0.0, 0.0);
        self.app = Some(app);
        self.gfx = Some((gfx, screen));
        self.window = Some(window);
    }

    /// A worker finished something: draw it.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        self.next = Next::Now;
        self.redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(app) = &mut self.app else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // Another window took focus: the reader's place is kept, as on the phone.
            WindowEvent::Focused(false) => app.keep_place(),
            WindowEvent::Focused(true) => app.resumed(),
            WindowEvent::Resized(size) => {
                if let Some((gfx, screen)) = &mut self.gfx {
                    screen.resize(gfx, size.width, size.height);
                }
                app.resize(size.width as f32, size.height as f32, 0.0, 0.0);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if self.mouse_down {
                    app.touch(Touch { id: 1, phase: Phase::Move, x: self.cursor.0, y: self.cursor.1, t_ns: now_ns() });
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_down = state == ElementState::Pressed;
                let phase = if self.mouse_down { Phase::Down } else { Phase::Up };
                app.touch(Touch { id: 1, phase, x: self.cursor.0, y: self.cursor.1, t_ns: now_ns() });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                app.wheel(-dy);
            }
            WindowEvent::Touch(t) => {
                let phase = match t.phase {
                    TouchPhase::Started => Phase::Down,
                    TouchPhase::Moved => Phase::Move,
                    TouchPhase::Ended => Phase::Up,
                    TouchPhase::Cancelled => Phase::Cancel,
                };
                app.touch(Touch { id: 10 + t.id, phase, x: t.location.x as f32, y: t.location.y as f32, t_ns: now_ns() });
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => match &event.logical_key {
                Key::Named(NamedKey::Enter) => app.done_typing(),
                Key::Named(NamedKey::Escape) => {
                    if !app.back() {
                        event_loop.exit();
                    }
                }
                Key::Named(NamedKey::Backspace) => app.backspace(),
                Key::Named(NamedKey::Space) => app.type_text(" "),
                _ => {
                    if let Some(text) = &event.text {
                        app.type_text(text);
                    }
                }
            },
            WindowEvent::RedrawRequested => {
                let Some((gfx, screen)) = &mut self.gfx else { return };
                self.next = app.frame(now_ns());
                if app.canvas.take_late_pictures() {
                    self.next = Next::Now;
                }
                gfx.upload_glyphs(&mut app.canvas.uploads);
                gfx.upload_images(&mut app.canvas.image_uploads);
                let (globals, clear) = (app.canvas.globals, app.clear_colour());
                if !screen.frame(gfx, &globals, clear, &app.canvas.insts, &app.canvas.batches) {
                    self.next = Next::At(now_ns() + 16_000_000);
                }
                return;
            }
            _ => return,
        }
        self.next = Next::Now;
        self.redraw();
    }

    /// The reader's place goes to the server before the window closes.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = &mut self.app {
            app.reset_to(Route::Home);
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.next {
            Next::Now => {
                event_loop.set_control_flow(ControlFlow::Poll);
                self.redraw();
            }
            Next::OnInput => event_loop.set_control_flow(ControlFlow::Wait),
            Next::At(at) => {
                let now = now_ns();
                if at <= now {
                    self.redraw();
                    event_loop.set_control_flow(ControlFlow::Poll);
                } else {
                    let wait = std::time::Duration::from_nanos(at - now);
                    event_loop.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + wait));
                }
            }
        }
    }
}
