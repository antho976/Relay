//! The app driven as a finger would, with no window and no GPU: taps and swipes in, routes and
//! state out.

use std::path::PathBuf;
use std::sync::Arc;

use leaf_proto::Series;

use crate::app::{App, Host, Route};
use crate::draw::WIDTH;
use crate::kit;
use crate::ui::{Phase, Touch};

struct TestHost(PathBuf);

impl Host for TestHost {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _text: &str) {}
}

fn series(id: i64, title: &str) -> Series {
    Series {
        id,
        title: title.into(),
        author: None,
        source: "S".into(),
        status: "ongoing".into(),
        chapters: 10,
        read_upto: Some(4.0),
        unread: 6,
        latest: Some(10.0),
        gaps: vec![],
        updated_at: 1_700_000_000_000 + id,
        note: None,
        bell: false,
        added_at: 1_700_000_000_000,
        last_read_at: 1_700_000_000_000 + id * 1000,
        shelf: None,
    }
}

/// An app on a 1080 × 2340 screen with three series and no server, its first frame drawn.
fn app(name: &str) -> (App, u64, PathBuf) {
    app_with(name, 3, 2340.0)
}

/// An app on a screen 1080 px wide and `h_px` tall with `n` series and no server.
fn app_with(name: &str, n: i64, h_px: f32) -> (App, u64, PathBuf) {
    let dir = std::env::temp_dir().join(format!("leaf-flows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = App::new(Arc::new(TestHost(dir.clone())), 0);
    app.shot = true;
    app.resize(1080.0, h_px, 0.0, 0.0);
    app.library.data = Some((1..=n).map(|i| series(i, &format!("Series {i}"))).collect());
    let mut t = 1_000_000_000;
    app.frame(t);
    t += 16_000_000;
    app.frame(t);
    (app, t, dir)
}

/// A frame later.
fn step(app: &mut App, t: &mut u64) {
    *t += 16_000_000;
    app.frame(*t);
}

/// A tap at (x, y) in layout units (the layout is centred on a screen wider than it).
fn tap(app: &mut App, t: &mut u64, x: f32, y: f32) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    app.touch(Touch { id: 1, phase: Phase::Down, x: x0 + x * u, y: y * u, t_ns: *t });
    app.touch(Touch { id: 1, phase: Phase::Up, x: x0 + x * u, y: y * u, t_ns: *t + 50_000_000 });
    step(app, t);
}

#[test]
fn a_tap_on_the_plus_menu_never_reaches_the_row_under_it() {
    let (mut app, mut t, dir) = app("menu");
    app.st.space.tab = crate::space::RECENT;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // The + key, then the menu's second item low down, where it lies over the first row (rows
    // start at 146; the item spans 108 to 156).
    tap(&mut app, &mut t, 390.0 - 6.0 - 88.0 + 22.0, 10.0 + 22.0);
    assert!(app.st.space.add_menu, "the + opened its menu");
    tap(&mut app, &mut t, 390.0 - 12.0 - 100.0, 56.0 + 4.0 + 48.0 + 42.0);
    assert_eq!(app.route(), &Route::Search, "the menu's own item took the tap");
    // And a tap outside it, on a row, only closes it.
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    app.st.space.add_menu = true;
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 60.0, 300.0);
    assert!(!app.st.space.add_menu);
    assert_eq!(app.route(), &Route::Space, "the row under the menu was not opened");
    let _ = std::fs::remove_dir_all(dir);
}

/// The tab bar's middle, in layout units: (x of tab `i`, y).
fn tab_at(app: &App, i: usize) -> (f32, f32) {
    let (y, _) = kit::tabs_area(&app.canvas);
    (WIDTH / 3.0 * (i as f32 + 0.5), y + kit::TABS_H / 2.0)
}

/// Tapping tab `to` from `from` (scrolled `off` units) lands on the tab only. First proves the
/// page has something tappable right under the tab bar there: a tap just above the bar opens it.
fn tab_over_content(app: &mut App, t: &mut u64, from: Route, off: f32, to: usize, want: Route) {
    let (x, y) = tab_at(app, to);
    let (top, _) = kit::tabs_area(&app.canvas);
    let go = |app: &mut App, t: &mut u64| {
        app.reset_to(from.clone());
        step(app, t);
        app.scroll().off = off;
        step(app, t);
        step(app, t);
    };
    go(app, t);
    tap(app, t, x, top - 3.0);
    assert_ne!(app.route(), &from, "something under the tab bar at {x},{} opens", top - 3.0);
    go(app, t);
    tap(app, t, x, y);
    assert_eq!(app.route(), &want, "the tab took the tap");
    assert_eq!(app.nav.len(), 1, "and nothing under it opened");
    // Deep in the bar too, by the gesture bar.
    go(app, t);
    tap(app, t, x, top + kit::TABS_H - 4.0);
    assert_eq!(app.route(), &want);
    assert_eq!(app.nav.len(), 1);
}

#[test]
fn a_tab_tap_never_reaches_the_page_under_the_tab_bar() {
    // Recently updated's rows lie under the bar on a tall screen.
    let (mut app, mut t, dir) = app_with("tabs-rows", 12, 2340.0);
    tab_over_content(&mut app, &mut t, Route::Home, 0.0, 2, Route::Space);
    tab_over_content(&mut app, &mut t, Route::Home, 120.0, 1, Route::Search);
    // My space's Recent list and Following's series rows.
    app.st.space.tab = crate::space::RECENT;
    tab_over_content(&mut app, &mut t, Route::Space, 0.0, 0, Route::Home);
    app.st.space.tab = crate::space::FOLLOWING;
    tab_over_content(&mut app, &mut t, Route::Space, 0.0, 1, Route::Search);
    // Updates' rows: each opens the reader once its series is at hand.
    app.updates.data = Some(updates(12));
    for i in 1..=12 {
        app.series.insert(i, loaded(detail(i, 10)));
    }
    app.st.space.tab = crate::space::UPDATES;
    tab_over_content(&mut app, &mut t, Route::Space, 0.0, 0, Route::Home);
    let _ = std::fs::remove_dir_all(dir);
    // Continue reading's rail of covers lies under it on a short one.
    let (mut app, mut t, dir) = app_with("tabs-rail", 12, 1500.0);
    tab_over_content(&mut app, &mut t, Route::Home, 0.0, 2, Route::Space);
    tab_over_content(&mut app, &mut t, Route::Home, 0.0, 1, Route::Search);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tap_on_a_fixed_top_never_reaches_what_scrolled_under_it() {
    let (mut app, mut t, dir) = app_with("top", 12, 2340.0);
    // Scrolled so the rail's covers pass under Home's fixed top, then the gear over one.
    app.scroll().off = 476.0;
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 200.0, 40.0);
    assert_eq!(app.route(), &Route::Home, "the band over the rail takes a tap for nothing");
    tap(&mut app, &mut t, WIDTH - 6.0 - 22.0, 28.0);
    assert_eq!(app.route(), &Route::Connect, "the gear, not the cover under it");
    assert_eq!(app.nav.len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_spotlight_cannot_be_dragged_and_a_vertical_drag_on_it_scrolls_the_page() {
    let (mut app, mut t, dir) = app_with("swipe", 12, 2340.0);
    assert_eq!(app.route(), &Route::Home);
    let (pos, target) = (app.st.home.pos, app.st.home.target);
    let u = app.canvas.u;
    let drag = |app: &mut App, t: &mut u64, from: (f32, f32), by: (f32, f32)| {
        app.touch(Touch { id: 1, phase: Phase::Down, x: from.0 * u, y: from.1 * u, t_ns: *t });
        for i in 1..=6 {
            let f = i as f32 / 6.0;
            app.touch(Touch { id: 1, phase: Phase::Move, x: (from.0 + by.0 * f) * u, y: (from.1 + by.1 * f) * u, t_ns: *t + i * 10_000_000 });
            step(app, t);
        }
        app.touch(Touch { id: 1, phase: Phase::Up, x: (from.0 + by.0) * u, y: (from.1 + by.1) * u, t_ns: *t + 70_000_000 });
        step(app, t);
    };
    drag(&mut app, &mut t, (300.0, 200.0), (-180.0, 0.0));
    for _ in 0..90 {
        step(&mut app, &mut t);
    }
    assert_eq!((app.st.home.pos, app.st.home.target), (pos, target), "a sideways swipe does not turn it");
    assert_eq!(app.route(), &Route::Home, "nor is it a tap");
    drag(&mut app, &mut t, (200.0, 320.0), (0.0, -200.0));
    assert!(app.scroll().off > 100.0, "a drag up from the spotlight scrolls the page: {}", app.scroll().off);
    assert_eq!((app.st.home.pos, app.st.home.target), (pos, target));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_spotlight_turns_slowly_by_itself_and_a_dot_chooses_one() {
    let (mut app, mut t, dir) = app("turn");
    app.shot = false;
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    assert_eq!(app.st.home.target, 0.0);
    t += 17_000_000_000;
    app.frame(t);
    assert_eq!(app.st.home.target, 0.0, "not before eighteen seconds");
    t += 1_100_000_000;
    app.frame(t);
    assert_eq!(app.st.home.target, 1.0, "then the next one");
    step(&mut app, &mut t);
    assert!(app.st.home.pos > 0.0 && app.st.home.pos < 0.2, "sliding gently: {}", app.st.home.pos);
    for _ in 0..90 {
        step(&mut app, &mut t);
    }
    assert_eq!(app.st.home.pos, 1.0, "and settles on it");
    // Three dots, right-aligned under the spotlight's words: the last one, tapped, is shown.
    let hero_h = app.canvas.inset_top + 436.0;
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 4.0 - 2.0, hero_h - 50.0 + 2.0);
    assert_eq!(app.st.home.target, 2.0, "the dot tapped");
    assert_eq!(app.route(), &Route::Home, "a dot is not the picture");
    for _ in 0..90 {
        step(&mut app, &mut t);
    }
    assert_eq!(app.st.home.pos, 2.0);
    let _ = std::fs::remove_dir_all(dir);
}

/// A series' chapters, 1 to `n`, the first four read, as the series page has them.
fn detail(id: i64, n: usize) -> leaf_proto::SeriesDetail {
    let chapters = (1..=n)
        .map(|k| leaf_proto::Chapter { id: id * 1000 + k as i64, number: k as f64, name: format!("Chapter {k}"), group: None, pages: 30, read: k <= 4, last_page: 0, downloaded: true, uploaded_at: 0, from: None })
        .collect();
    leaf_proto::SeriesDetail { series: series(id, &format!("Series {id}")), description: None, chapters }
}

fn loaded<T>(data: T) -> crate::app::Loaded<T> {
    crate::app::Loaded { data: Some(data), live: true, error: None, loading: false }
}

/// One update per series 1..=n, each its chapter 10, newest first, belled: all from today, around
/// local noon (whatever the hour the test runs at, they stay one day).
fn updates(n: i64) -> Vec<leaf_proto::Update> {
    let today = crate::shelf::day_of(crate::shelf::now_ms());
    let noon = (today * 86_400 - crate::shelf::zone_offset() + 12 * 3600) * 1000;
    (1..=n).map(|i| leaf_proto::Update { series: i, title: format!("Series {i}"), chapter: i * 1000 + 10, number: 10.0, name: String::new(), at: noon - i * 600_000, read: false, bell: true }).collect()
}

/// A finger held at (x, y) past a long press, then lifted.
fn hold(app: &mut App, t: &mut u64, x: f32, y: f32) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    app.touch(Touch { id: 1, phase: Phase::Down, x: x0 + x * u, y: y * u, t_ns: *t });
    step(app, t);
    std::thread::sleep(std::time::Duration::from_millis(crate::series::LONG_PRESS_NS / 1_000_000 + 80));
    step(app, t);
    step(app, t);
    app.touch(Touch { id: 1, phase: Phase::Up, x: x0 + x * u, y: y * u, t_ns: *t + 600_000_000 });
    step(app, t);
}

#[test]
fn updates_come_first_and_a_row_opens_its_chapter() {
    let (mut app, mut t, dir) = app("updates");
    app.updates.data = Some(updates(3));
    app.series.insert(2, loaded(detail(2, 10)));
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    assert_eq!(app.st.space.tab, crate::space::UPDATES, "Updates is My space's first tab");
    // Under the tabs, the field, the count and Belled only / All: the day's heading at 214, then
    // rows of 76 from 248. The second is series 2.
    tap(&mut app, &mut t, 200.0, 248.0 + 76.0 + 38.0);
    assert_eq!(app.route(), &Route::Reader, "the update opened the reader");
    let r = app.reader.as_ref().unwrap();
    assert_eq!((r.series, r.chapters.len()), (2, 10));
    assert!(matches!(r.pos.item, crate::strip::Item::Head(9)), "at chapter 10: {:?}", r.pos.item);
    // A series not at hand yet: asked for, opened once it comes.
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 200.0, 248.0 + 38.0);
    assert_eq!(app.route(), &Route::Space);
    assert_eq!(app.open_after, Some((1, 1010)));
    app.series.insert(1, loaded(detail(1, 10)));
    step(&mut app, &mut t);
    assert_eq!(app.route(), &Route::Reader);
    // Belled only hides series whose bell is off.
    app.reset_to(Route::Space);
    let mut list = updates(3);
    list[0].bell = false;
    app.updates.data = Some(list);
    app.updates_all = false;
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 200.0, 248.0 + 38.0);
    assert_eq!(app.reader.as_ref().map(|r| r.series), Some(2), "series 1 is hidden: its bell is off");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_filter_takes_its_tap_and_narrows_the_list() {
    let (mut app, mut t, dir) = app_with("filter", 12, 2340.0);
    app.library.data.as_mut().unwrap()[10].title = "Pokémon Adventures".into();
    app.st.space.tab = crate::space::FOLLOWING;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    // Scrolled so rows pass under the field, then the field tapped: it takes the keyboard, no row
    // opens.
    app.scroll().off = 300.0;
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 150.0, 104.0 + 8.0 + 20.0);
    assert_eq!(app.route(), &Route::Space, "the field is not the row under it");
    assert!(app.focus.is("space-filter"));
    app.set_text("POKEMON adv");
    step(&mut app, &mut t);
    assert_eq!(app.st.space.query, "POKEMON adv");
    app.finish_typing();
    *app.scroll() = Default::default();
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // One row is left: the first under the chips (rows start at 158 + 54 + 46 = 258).
    tap(&mut app, &mut t, 200.0, 258.0 + 40.0);
    assert_eq!(app.route(), &Route::Series(11));
    app.pop();
    step(&mut app, &mut t);
    // The clear key empties it.
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 20.0, 104.0 + 8.0 + 20.0);
    assert_eq!(app.st.space.query, "");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_long_press_on_a_row_puts_it_on_a_shelf_and_the_chips_filter_by_it() {
    let (mut app, mut t, dir) = app("shelves");
    app.st.space.tab = crate::space::FOLLOWING;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    // Following sorts by last read: series 3 first, its row at 258.
    hold(&mut app, &mut t, 200.0, 258.0 + 50.0);
    assert_eq!(app.st.space.shelf_for, Some(3), "the long press opened the shelf sheet");
    assert_eq!(app.route(), &Route::Space, "and did not open the series");
    // The sheet: title and line (76), then Reading, Plan to read, On hold, Dropped, Done; 52 each.
    let h = app.canvas.h;
    let top = h - (76.0 + 52.0 * 5.0 + 12.0);
    tap(&mut app, &mut t, 120.0, top + 76.0 + 52.0 * 3.0 + 26.0);
    assert_eq!(app.st.space.shelf_for, None);
    assert_eq!(app.library.data.as_ref().unwrap().iter().find(|s| s.id == 3).unwrap().shelf.as_deref(), Some("dropped"));
    assert_eq!(app.route(), &Route::Space, "the row under the sheet was not opened");
    // Dropped is the fifth chip: only series 3 is left.
    app.st.space.shelf = Some(crate::logic::Shelf::Dropped);
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 200.0, 258.0 + 50.0);
    assert_eq!(app.route(), &Route::Series(3));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_long_press_on_a_chapter_opens_its_sheet_and_marks_it() {
    let (mut app, mut t, dir) = app("chapter");
    app.series.insert(1, loaded(detail(1, 10)));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // Newest first: chapter 10's row, then 9's (rows 84 tall from about 509).
    hold(&mut app, &mut t, 200.0, 640.0);
    let c = app.st.series.chapter.expect("the chapter's sheet is open");
    assert_eq!(app.route(), &Route::Series(1), "the long press did not open the reader");
    // Mark as read, the sheet's first row (no date: no line under its title).
    let h = app.canvas.h;
    let top = h - (58.0 + 52.0 * 3.0 + 12.0);
    tap(&mut app, &mut t, 120.0, top + 58.0 + 26.0);
    let d = app.series[&1].data.as_ref().unwrap();
    assert!(d.chapters.iter().find(|x| x.id == c).unwrap().read, "marked read at once");
    assert_eq!(app.st.series.chapter, None);
    assert_eq!(app.route(), &Route::Series(1), "the row under the sheet was not opened");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_reader_turns_pages_by_thirds_and_auto_scrolls_until_tapped() {
    let (mut app, mut t, dir) = app("reader");
    let d = detail(1, 3);
    for c in &d.chapters {
        for n in 0..30 {
            app.images.fake_size(c.id, n, 800, 1600);
        }
    }
    app.open_chapter(&d, 1, 0);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    let at = |app: &App| app.reader.as_ref().map(|r| r.pos).unwrap();
    // The bottom third: most of a screen down, gliding there.
    let before = at(&app);
    tap(&mut app, &mut t, 200.0, h * 0.85);
    for _ in 0..60 {
        step(&mut app, &mut t);
    }
    assert_ne!(at(&app), before, "the page turned");
    assert!(!app.reader.as_ref().unwrap().overlay, "and no overlay");
    // A volume key the same way, then back up with the top third.
    let mid = at(&app);
    assert!(app.volume(true));
    for _ in 0..60 {
        step(&mut app, &mut t);
    }
    tap(&mut app, &mut t, 200.0, h * 0.1);
    for _ in 0..60 {
        step(&mut app, &mut t);
    }
    assert_eq!(at(&app).item, mid.item, "down and up again");
    // The middle shows the overlay; its play key starts auto-scroll and puts the overlay away.
    tap(&mut app, &mut t, 200.0, h * 0.5);
    assert!(app.reader.as_ref().unwrap().overlay);
    let bar_top = h - 112.0;
    tap(&mut app, &mut t, WIDTH / 2.0, bar_top + 10.0 + 22.0);
    let r = app.reader.as_ref().unwrap();
    assert!(r.auto && !r.overlay, "playing");
    let from = at(&app);
    for _ in 0..30 {
        step(&mut app, &mut t);
    }
    assert_ne!(at(&app), from, "it moves by itself");
    // Any tap stops it and shows the overlay.
    tap(&mut app, &mut t, 200.0, h * 0.85);
    let r = app.reader.as_ref().unwrap();
    assert!(!r.auto && r.overlay);
    // − and + set the series' speed, kept.
    let speed = app.speed_of(1);
    tap(&mut app, &mut t, WIDTH / 2.0 + 26.0 + 22.0, bar_top + 10.0 + 22.0);
    assert_eq!(app.speed_of(1), speed + 1);
    let _ = std::fs::remove_dir_all(dir);
}

// The comics reader

/// A Leaf server on loopback for the reader's flows: chapter lengths, pages (a small picture),
/// progress kept; anything else is a 404. Chapters in `fail_counts` or `fail_pages` answer 503.
mod fake {
    use std::collections::{HashMap, HashSet};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct State {
        /// Every request, as "GET /v1/...".
        pub hits: Vec<String>,
        pub progress: Vec<(i64, leaf_proto::Progress)>,
        pub counts: HashMap<i64, u32>,
        pub fail_counts: HashSet<i64>,
        pub fail_pages: HashSet<i64>,
    }

    pub struct Fake {
        pub base: String,
        pub state: Arc<Mutex<State>>,
    }

    impl Fake {
        pub fn start() -> Fake {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let state = Arc::new(Mutex::new(State::default()));
            // 80 × 160, a flat colour: drawn 390 wide it is 780 tall, nothing for the trim to cut.
            let mut png = Vec::new();
            image::RgbaImage::from_pixel(80, 160, image::Rgba([120, 90, 160, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
            let png = Arc::new(png);
            let st = state.clone();
            std::thread::spawn(move || {
                for conn in listener.incoming() {
                    let (Ok(mut s), st, png) = (conn, st.clone(), png.clone()) else { continue };
                    std::thread::spawn(move || answer(&mut s, &st, &png));
                }
            });
            Fake { base, state }
        }

        pub fn server(&self) -> crate::net::Server {
            crate::net::Server { base: self.base.clone(), token: None }
        }

        pub fn hits(&self, path: &str) -> usize {
            self.state.lock().unwrap().hits.iter().filter(|h| h.ends_with(&format!(" {path}"))).count()
        }
    }

    fn answer(s: &mut std::net::TcpStream, st: &Mutex<State>, png: &[u8]) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            let Ok(n) = s.read(&mut chunk) else { return };
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
        while buf.len() < head_end + len {
            let Ok(n) = s.read(&mut chunk) else { return };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let body = buf[head_end..].to_vec();
        let first = head.lines().next().unwrap_or("").to_string();
        let mut parts = first.split(' ');
        let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("").to_string());
        let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let (code, kind, out): (u16, &str, Vec<u8>) = {
            let mut st = st.lock().unwrap();
            st.hits.push(format!("{method} {path}"));
            match (method, seg.as_slice()) {
                ("GET", ["v1", "chapters", id, "pages"]) => {
                    let id: i64 = id.parse().unwrap_or(0);
                    match st.counts.get(&id) {
                        Some(n) if !st.fail_counts.contains(&id) => (200, "application/json", format!("{{\"chapter_id\":{id},\"count\":{n}}}").into_bytes()),
                        _ => (503, "text/plain", b"busy".to_vec()),
                    }
                }
                ("GET", ["v1", "chapters", id, "pages", _]) => {
                    if st.fail_pages.contains(&id.parse().unwrap_or(0)) {
                        (503, "text/plain", b"busy".to_vec())
                    } else {
                        (200, "image/png", png.to_vec())
                    }
                }
                ("POST", ["v1", "chapters", id, "progress"]) => {
                    if let Ok(p) = serde_json::from_slice::<leaf_proto::Progress>(&body) {
                        st.progress.push((id.parse().unwrap_or(0), p));
                    }
                    (200, "application/json", b"{\"ok\":true}".to_vec())
                }
                _ => (404, "text/plain", b"no".to_vec()),
            }
        };
        let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
        let _ = s.write_all(&out);
    }
}

/// A series of chapters 1.. with these page counts as the server lists them (0: not known yet),
/// none read, ids 1000 + number.
fn comic(id: i64, pages: &[u32]) -> leaf_proto::SeriesDetail {
    let chapters = pages
        .iter()
        .enumerate()
        .map(|(i, &p)| leaf_proto::Chapter { id: 1000 + i as i64 + 1, number: i as f64 + 1.0, name: String::new(), group: None, pages: p, read: false, last_page: 0, downloaded: true, uploaded_at: 0, from: None })
        .collect();
    leaf_proto::SeriesDetail { series: series(id, &format!("Series {id}")), description: None, chapters }
}

/// Every page of these chapters measured as 800 × 1600 (780 units tall on the strip).
fn measure(app: &mut App, d: &leaf_proto::SeriesDetail) {
    for c in &d.chapters {
        for n in 0..c.pages {
            app.images.fake_size(c.id, n, 800, 1600);
        }
    }
}

/// Frames, with the workers given time, until `done` or about `secs` of real time.
fn until(app: &mut App, t: &mut u64, secs: f32, done: impl Fn(&App) -> bool) -> bool {
    for _ in 0..(secs * 200.0) as usize {
        step(app, t);
        if done(app) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

fn reader(app: &App) -> &crate::reader::Reader {
    app.reader.as_ref().expect("the reader is open")
}

/// The strip moved by `dy` units, as a finger would, then a few frames.
fn scroll(app: &mut App, t: &mut u64, dy: f32) {
    let h = app.canvas.h;
    if let Some(r) = &mut app.reader {
        r.scroll_by(&app.images, dy, h);
    }
    step(app, t);
    step(app, t);
}

#[test]
fn chapters_read_through_are_finished_and_a_peek_at_the_one_before_is_not() {
    let (mut app, mut t, dir) = app("reader-done");
    app.shot = false;
    // Three pages, one page, three pages.
    let d = comic(1, &[3, 1, 3]);
    measure(&mut app, &d);
    // Opened at chapter 3's heading, then pulled down a little: chapter 2's single page shows its
    // bottom above the heading. It was not read.
    app.open_chapter(&d, 2, 0);
    step(&mut app, &mut t);
    scroll(&mut app, &mut t, -300.0);
    assert!(matches!(reader(&app).pos.item, crate::strip::Item::Page(1, 0)), "{:?}", reader(&app).pos);
    assert!(!reader(&app).chapters[1].read, "a glance at a chapter's last page does not finish it");
    assert!(app.reported.iter().all(|(_, p)| !p.done), "{:?}", app.reported);
    app.pop();
    // Read from chapter 1's heading down into chapter 3: chapters 1 and 2 are finished, each once,
    // and chapter 3 is not.
    app.reported.clear();
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    for _ in 0..12 {
        scroll(&mut app, &mut t, 300.0);
    }
    assert!(matches!(reader(&app).pos.item, crate::strip::Item::Page(2, _)), "{:?}", reader(&app).pos);
    let done: Vec<i64> = app.reported.iter().filter(|(_, p)| p.done).map(|(c, _)| *c).collect();
    assert_eq!(done, vec![1001, 1002], "{:?}", app.reported);
    let r = reader(&app);
    assert!(r.chapters[0].read && r.chapters[1].read && !r.chapters[2].read);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_reader_never_reports_a_chapter_it_has_left() {
    let (mut app, mut t, dir) = app("reader-order");
    app.shot = false;
    let d = comic(1, &[20, 20]);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    // A few pages down chapter 1 within the debounce: the last of them is held back.
    for _ in 0..4 {
        scroll(&mut app, &mut t, 780.0);
    }
    let held = app.reported.len();
    // The overlay's next key: chapter 2's heading. The held report for chapter 1 goes first.
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
    assert!(reader(&app).overlay);
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 20.0, h - 112.0 + 10.0 + 22.0);
    assert!(matches!(reader(&app).pos.item, crate::strip::Item::Head(1)), "{:?}", reader(&app).pos);
    for _ in 0..3 {
        scroll(&mut app, &mut t, 780.0);
    }
    t += 10_000_000_000;
    step(&mut app, &mut t);
    app.pop();
    let after: Vec<i64> = app.reported[held..].iter().map(|(c, _)| *c).collect();
    assert!(!after.is_empty() && after[0] == 1001, "chapter 1's last word goes as chapter 2 opens: {:?}", app.reported);
    assert!(after.iter().skip_while(|c| **c == 1001).all(|c| *c == 1002), "nothing for chapter 1 after it: {:?}", app.reported);
    assert!(app.reported.iter().all(|(_, p)| !p.done), "skipped, not finished: {:?}", app.reported);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_chapter_key_lands_on_the_chapter_heading_even_mid_page_turn() {
    let (mut app, mut t, dir) = app("reader-jump");
    let d = comic(1, &[5, 5]);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
    // A volume key turns the page as the next key is tapped.
    assert!(app.volume(true));
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 20.0, h - 112.0 + 10.0 + 22.0);
    for _ in 0..40 {
        step(&mut app, &mut t);
    }
    let r = reader(&app);
    assert_eq!(r.pos, crate::strip::Pos { item: crate::strip::Item::Head(1), off: 0.0 }, "at chapter 2's heading");
    assert!(!r.overlay);
    // And the previous key, back to chapter 1's.
    tap(&mut app, &mut t, 200.0, h * 0.5);
    tap(&mut app, &mut t, crate::theme::PAD + 20.0, h - 112.0 + 10.0 + 22.0);
    assert_eq!(reader(&app).pos, crate::strip::Pos { item: crate::strip::Item::Head(0), off: 0.0 });
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn back_puts_the_reader_overlay_away_before_leaving() {
    let (mut app, mut t, dir) = app("reader-back");
    let d = comic(1, &[5, 5]);
    measure(&mut app, &d);
    app.series.insert(1, loaded(d.clone()));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
    assert!(reader(&app).overlay);
    assert!(app.back());
    assert_eq!(app.route(), &Route::Reader, "Back closed the overlay only");
    assert!(!reader(&app).overlay);
    assert!(app.back());
    assert_eq!(app.route(), &Route::Series(1), "then left, to the series");
    assert!(app.reader.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_update_waiting_for_its_series_does_not_open_after_you_moved_on() {
    let (mut app, mut t, dir) = app("reader-late");
    app.shot = false;
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    // An update tapped for series 1, not at hand: it waits for the series to load.
    app.open_chapter_of(1, 1001);
    assert_eq!(app.open_after, Some((1, 1001)));
    // Meanwhile the person reads series 2, a few pages in (the last held back by the debounce).
    let d2 = comic(2, &[20]);
    let d2 = leaf_proto::SeriesDetail { chapters: d2.chapters.into_iter().map(|mut c| { c.id += 1000; c }).collect(), ..d2 };
    measure(&mut app, &d2);
    app.open_chapter(&d2, 0, 0);
    step(&mut app, &mut t);
    scroll(&mut app, &mut t, 780.0 * 2.0);
    scroll(&mut app, &mut t, 780.0 * 2.0);
    // Series 1 arrives: the reader stays on series 2.
    app.series.insert(1, loaded(comic(1, &[5])));
    step(&mut app, &mut t);
    assert_eq!(reader(&app).series, 2, "the late update did not take over the reader");
    // Opening another chapter while the reader is open closes it properly first: its held-back
    // place is sent, not lost.
    let held = reader(&app).pos;
    assert!(matches!(held.item, crate::strip::Item::Page(0, n) if n >= 3), "{held:?}");
    let before = app.reported.len();
    app.open_chapter(&comic(1, &[5]), 0, 0);
    assert_eq!(reader(&app).series, 1);
    assert!(app.reported[before..].iter().any(|(c, p)| *c == 2001 && p.page >= 3), "{:?}", app.reported);
    assert_eq!(app.nav.iter().filter(|(r, _)| *r == Route::Reader).count(), 1, "one reader on the stack");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn back_from_the_reader_shows_what_was_read_at_once() {
    let (mut app, mut t, dir) = app("reader-marks");
    app.shot = false;
    let fake = fake::Fake::start();
    let d = comic(1, &[3, 3, 3]);
    // The copy on disk is the one from before reading (the cached answer comes first).
    crate::net::write_atomic(&dir.join("series").join("1.json"), &serde_json::to_vec(&d).unwrap()).unwrap();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    app.series.insert(1, loaded(d.clone()));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    // Chapter 1 read to its end and a page into chapter 2.
    for _ in 0..13 {
        scroll(&mut app, &mut t, 300.0);
    }
    assert!(matches!(reader(&app).pos.item, crate::strip::Item::Page(1, 1)), "{:?}", reader(&app).pos);
    assert!(app.back());
    assert_eq!(app.route(), &Route::Series(1));
    // The server is slow to answer (here it never does: 404): the page shows the place now.
    assert!(until(&mut app, &mut t, 3.0, |a| a.series.get(&1).is_some_and(|l| !l.loading)));
    let got = app.series[&1].data.as_ref().unwrap();
    assert!(got.chapters[0].read, "chapter 1 shows read");
    assert_eq!(got.chapters[1].last_page, 1, "chapter 2 shows its page");
    assert_eq!(crate::logic::continue_point(&got.chapters).map(|c| (c.index, c.page)), Some((1, 1)), "Continue goes on in chapter 2");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_chapter_whose_length_will_not_come_is_not_asked_for_every_frame() {
    let (mut app, mut t, dir) = app("reader-count");
    let fake = fake::Fake::start();
    fake.state.lock().unwrap().fail_counts.insert(1001);
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    // Listed without a length.
    let d = comic(1, &[0, 0]);
    fake.state.lock().unwrap().counts.insert(1002, 4);
    app.open_chapter(&d, 0, 0);
    for _ in 0..150 {
        step(&mut app, &mut t);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let asked = fake.hits("/v1/chapters/1001/pages");
    assert!((1..=2).contains(&asked), "asked {asked} times in 2.4 s");
    // Later it answers: the reader asks again by itself, and the chapter opens.
    fake.state.lock().unwrap().fail_counts.clear();
    fake.state.lock().unwrap().counts.insert(1001, 3);
    t += 60_000_000_000;
    assert!(until(&mut app, &mut t, 3.0, |a| a.reader.as_ref().is_some_and(|r| r.count(0) == Some(3))), "the length came");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_page_whose_early_fetch_failed_still_loads_when_reached() {
    let (mut app, mut t, dir) = app("reader-prefetch");
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    // Chapter 2's pages are kept ahead on disk while chapter 1 is read; the server can't yet.
    fake.state.lock().unwrap().fail_pages.insert(1002);
    let d = comic(1, &[6, 3]);
    app.open_chapter(&d, 0, 0);
    assert!(until(&mut app, &mut t, 5.0, |_| fake.hits("/v1/chapters/1002/pages/2") > 0 && !fake.state.lock().unwrap().hits.is_empty()));
    assert!(until(&mut app, &mut t, 5.0, |a| !a.images.pending()));
    // Now it can, and the person reaches chapter 2.
    fake.state.lock().unwrap().fail_pages.clear();
    if let Some(r) = &mut app.reader {
        r.pos = crate::strip::Pos { item: crate::strip::Item::Head(1), off: 0.0 };
    }
    assert!(until(&mut app, &mut t, 5.0, |a| (0..3).all(|n| a.images.size(1002, n).is_some())), "its pages load: {:?}", app.images.failed(1002, 0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failed_page_is_tried_again_when_the_chapter_is_opened_again() {
    let (mut app, mut t, dir) = app("reader-reopen");
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    fake.state.lock().unwrap().fail_pages.insert(1001);
    let d = comic(1, &[2]);
    app.open_chapter(&d, 0, 0);
    assert!(until(&mut app, &mut t, 5.0, |a| a.images.failed(1001, 0).is_some()), "the page failed");
    app.pop();
    fake.state.lock().unwrap().fail_pages.clear();
    app.open_chapter(&d, 0, 0);
    assert!(until(&mut app, &mut t, 5.0, |a| a.images.size(1001, 0).is_some()), "and loads when the chapter opens again");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn only_the_tiles_near_the_screen_go_to_the_gpu() {
    let (mut app, mut t, dir) = app("reader-tiles");
    // A webtoon strip: 80 × 16 320 pixels, sixteen tiles, each nearly five thousand units tall.
    let d = comic(1, &[1]);
    app.images.fake_page(1001, 0, 80, 16 * crate::logic::TILE_H);
    app.canvas.image_uploads.clear();
    app.open_chapter(&d, 0, 0);
    for _ in 0..10 {
        step(&mut app, &mut t);
    }
    let sent = app.canvas.image_uploads.len();
    assert!((1..=2).contains(&sent), "{sent} tiles uploaded for a screen that shows one");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pages_trimmed_anew_are_not_drawn_from_their_old_tiles() {
    let (mut app, mut t, dir) = app("reader-retrim");
    let d = comic(1, &[1]);
    app.images.fake_page(1001, 0, 80, 400);
    app.open_chapter(&d, 0, 0);
    for _ in 0..5 {
        step(&mut app, &mut t);
    }
    app.pop();
    // Settings: trimming turned off. The page decodes taller now.
    app.images.retrim(false);
    app.images.fake_page(1001, 0, 80, 600);
    app.canvas.image_uploads.clear();
    app.open_chapter(&d, 0, 0);
    for _ in 0..5 {
        step(&mut app, &mut t);
    }
    assert!(app.canvas.image_uploads.iter().any(|u| u.h == 600), "the new page went up, not the old tile");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn auto_scroll_waits_at_a_chapter_still_opening_and_goes_on_into_it() {
    let (mut app, mut t, dir) = app("reader-auto");
    let d = comic(1, &[2, 0]);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
    tap(&mut app, &mut t, WIDTH / 2.0, h - 112.0 + 10.0 + 22.0);
    assert!(reader(&app).auto);
    // At full speed for a while: it reaches chapter 2's heading and waits there.
    app.set_speed(1, crate::logic::SPEEDS);
    if let Some(r) = &mut app.reader {
        r.speed = crate::logic::SPEEDS;
    }
    for _ in 0..600 {
        step(&mut app, &mut t);
    }
    let r = reader(&app);
    assert!(r.auto && r.waiting, "waiting, still on: {:?}", r.pos);
    let at = r.pos;
    for _ in 0..30 {
        step(&mut app, &mut t);
    }
    assert_eq!(reader(&app).pos, at, "still while it waits");
    // Chapter 2's length and pages come: it goes on into them, then stops at the end.
    if let Some(r) = &mut app.reader {
        r.page_count(1002, Some(2), t);
    }
    app.images.fake_size(1002, 0, 800, 1600);
    app.images.fake_size(1002, 1, 800, 1600);
    for _ in 0..900 {
        step(&mut app, &mut t);
    }
    let r = reader(&app);
    assert!(!r.auto, "stopped at the end of the last chapter");
    assert!(matches!(r.pos.item, crate::strip::Item::Page(1, _)), "in chapter 2, its end on screen: {:?}", r.pos);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn back_to_the_series_at_the_end_goes_to_the_series_wherever_the_reader_was_opened() {
    let (mut app, mut t, dir) = app("reader-end");
    let d = comic(1, &[2, 2]);
    measure(&mut app, &d);
    app.series.insert(1, loaded(d.clone()));
    // Continue from Home: the series page is not under the reader.
    app.continue_series(1);
    assert_eq!(app.route(), &Route::Reader);
    step(&mut app, &mut t);
    scroll(&mut app, &mut t, 1e5);
    let h = app.canvas.h;
    tap(&mut app, &mut t, WIDTH / 2.0, h - 240.0 + 124.0 + 22.0);
    assert_eq!(app.route(), &Route::Series(1), "the end's key went to the series");
    assert_eq!(app.nav.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(), vec![Route::Home, Route::Series(1)], "and Back from there goes Home");
    assert!(app.reader.is_none());
    // From the series page itself it only goes back to it.
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    scroll(&mut app, &mut t, 1e5);
    tap(&mut app, &mut t, WIDTH / 2.0, h - 240.0 + 124.0 + 22.0);
    assert_eq!(app.nav.len(), 2);
    assert_eq!(app.route(), &Route::Series(1));
    let _ = std::fs::remove_dir_all(dir);
}
