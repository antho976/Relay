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
