//! The desktop build: a phone-shaped window for working on Leaf with the mouse and keyboard, and a
//! screenshot mode that needs no window.
//!
//! - `--shot out.png --screen NAME` draws one screen to a file, against a real server
//!   (`--server`, default `http://127.0.0.1:4580`; `--token`), on a data folder of its own that
//!   is removed after. Screens: `connect` (the first run, the address checked), `connect-empty`,
//!   `library`, `library-empty` (the list answered empty), `search` (`--query`, default "nano"), `series` (`--id`, default 1), `reader`
//!   (`--id`, `--chapter` number, default 1; `--scroll UNITS` down the strip; `--overlay 1`).
//!   `--size WxH` (default 1080x2340). Nothing is written to the server: no progress is reported.
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
        "books" | "books-empty" => app.reset_to(Route::Library),
        "imports" => {}
        "notes" => app.reset_to(Route::Notes),
        "stats" => app.reset_to(Route::Stats),
        "review" => {
            app.reset_to(Route::Notes);
            app.push(Route::Review);
        }
        "find" => {
            app.reset_to(Route::Library);
            app.push(Route::Find);
            app.frame(*clock);
            let q = arg(args, "--query").unwrap_or_else(|| "nature".into());
            app.focus.text = q.clone();
            app.st.find.query = q;
            app.finish_typing();
        }
        "book-notes" => {
            let id = first.ok_or("no book imported")?;
            app.reset_to(Route::Notes);
            app.push(Route::BookNotes(id));
        }
        "book" | "in-search" => {
            let id = first.ok_or("no book imported")?;
            app.reset_to(Route::Library);
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
    let server = Server { base: crate::logic::normalize_server(&base).map_err(|e| e.to_string())?, token: arg(args, "--token") };
    let id = arg(args, "--id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(1);
    match screen.as_str() {
        "connect-empty" => {}
        "connect" => {
            app.st.connect.address = crate::logic::server_label(&server.base);
            app.st.connect.token = server.token.clone().unwrap_or_default();
            crate::connect::start(&mut app);
        }
        "library" | "library-empty" => {
            app.set_server(server);
            app.reset_to(Route::Library);
        }
        "search" => {
            app.set_server(server);
            app.reset_to(Route::Library);
            app.push(Route::Search);
            app.frame(clock);
            app.st.search.query = arg(args, "--query").unwrap_or_else(|| "nano".into());
            app.focus.text = app.st.search.query.clone();
            app.finish_typing();
        }
        "series" | "reader" => {
            app.set_server(server);
            app.reset_to(Route::Library);
            app.push(Route::Series(id));
        }
        _ if BOOK_SCREENS.contains(&screen.as_str()) => {
            books_shot(&mut app, &gfx, &mut clock, args, &screen)?;
        }
        other => return Err(format!("unknown screen: {other}")),
    }
    settle(&mut app, &gfx, &mut clock, 30.0);
    if screen == "library-empty" {
        // What a first library looks like, before anything is followed (the owner's server
        // follows two series already).
        app.library.data = Some(Vec::new());
    }
    if screen == "reader" {
        let d = app.series.get(&id).and_then(|l| l.data.clone()).ok_or("the series did not load")?;
        let number = arg(args, "--chapter").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
        let index = d.chapters.iter().position(|c| c.number == number).ok_or(format!("no chapter {number}"))?;
        app.reader = Some(crate::reader::Reader::open(&d, index, 0));
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
        settle(&mut app, &gfx, &mut clock, 10.0);
    }
    if screen != "reader"
        && screen != "book"
        && let Some(units) = arg(args, "--scroll").and_then(|s| s.parse::<f32>().ok())
    {
        app.scroll().off = units;
        settle(&mut app, &gfx, &mut clock, 10.0);
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
            app.reset_to(Route::Library);
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
