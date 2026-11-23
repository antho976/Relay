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

/// Links each test's app asked the platform to open, by its data folder (tests run at once).
static OPENED: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

impl Host for TestHost {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _text: &str) {}
    fn open_url(&self, url: &str) -> bool {
        OPENED.lock().unwrap().push((self.0.clone(), url.to_string()));
        true
    }
}

/// The links this app asked the platform to open.
fn opened(app: &App) -> Vec<String> {
    OPENED.lock().unwrap().iter().filter(|(d, _)| *d == app.data).map(|(_, u)| u.clone()).collect()
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
    // The offline prompt has tests of its own: on a server that isn't there it stays out of these.
    app.st.online.off = true;
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
    // Top picks lie under the bar on a tall screen (picks already yours open their series).
    let (mut app, mut t, dir) = app_with("tabs-rows", 12, 2340.0);
    nowhere(&mut app);
    app.recs.data = Some(picks(9, Some(5)));
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
    // Picks under it, so the page is long enough to scroll.
    nowhere(&mut app);
    app.recs.data = Some(picks(9, Some(5)));
    step(&mut app, &mut t);
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
    // Three dots, centred under the card: the last one, tapped, is shown.
    let dots_y = app.canvas.inset_top + 56.0 + 4.0 + 412.0 + 14.0;
    tap(&mut app, &mut t, WIDTH / 2.0 + 18.0, dots_y + 2.0);
    assert_eq!(app.st.home.target, 2.0, "the dot tapped");
    assert_eq!(app.route(), &Route::Home, "a dot is not the picture");
    for _ in 0..90 {
        step(&mut app, &mut t);
    }
    assert_eq!(app.st.home.pos, 2.0);
    let _ = std::fs::remove_dir_all(dir);
}

/// A server nothing answers at: the app asks, nothing comes, nothing is written anywhere.
fn nowhere(app: &mut App) {
    app.server = Some(crate::net::Server { base: "http://127.0.0.1:9".into(), token: None, alt: None });
}

/// `n` made-up picks, each `in_space` when given, with two genres.
fn picks(n: i64, in_space: Option<i64>) -> leaf_proto::Recs {
    let rec = |i: i64| leaf_proto::Rec {
        anilist_id: 100 + i,
        title: format!("Pick {i}"),
        reason: "More like Series 1".into(),
        genres: vec!["Action".into(), "Murim".into()],
        chapters: Some(80),
        status: "ongoing".into(),
        hits: vec![leaf_proto::Hit { manga_id: 500 + i, source: "S".into() }],
        cover: String::new(),
        in_space,
    };
    leaf_proto::Recs { top: (1..=n).map(rec).collect(), made_at: 1, ..Default::default() }
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
    *t += crate::series::LONG_PRESS_NS + 80_000_000;
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

/// Where the series page's first chapter row starts for a series from [`detail`] (no cover, no
/// description; the status tag), and where the bottom bar's keys are.
fn first_row(app: &App) -> f32 {
    // Back row 56, the poster 6 + 212, 22 under it, the title, 12, the tag 28 + 20, the tabs 43,
    // 14, "N chapters" and 12.
    let title = crate::draw::Text::new(24.0, crate::theme::INK).brand().line(1.18).lh();
    let head = crate::draw::Text::new(crate::theme::ty::UI, crate::theme::INK).semi().lh();
    app.canvas.inset_top + 56.0 + 6.0 + 212.0 + 22.0 + title + 12.0 + 48.0 + 43.0 + 14.0 + head + 12.0
}

#[test]
fn a_long_press_on_a_chapter_selects_it_and_the_bar_marks_the_selection() {
    let (mut app, mut t, dir) = app("chapter");
    app.series.insert(1, loaded(detail(1, 10)));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let y0 = first_row(&app);
    let row = crate::series::ROW_H;
    // Newest first: chapter 10, 9, 8... Hold the third (chapter 8).
    hold(&mut app, &mut t, 200.0, y0 + row * 2.0 + row / 2.0);
    assert_eq!(app.route(), &Route::Series(1), "the long press did not open the reader");
    let sel = app.st.series.select.clone().expect("selecting");
    assert_eq!(sel.into_iter().collect::<Vec<_>>(), vec![1008]);
    // A tap now adds a chapter instead of opening it.
    tap(&mut app, &mut t, 200.0, y0 + row * 3.0 + row / 2.0);
    assert_eq!(app.route(), &Route::Series(1));
    assert_eq!(app.st.series.select.as_ref().unwrap().len(), 2, "chapter 7 too");
    // Mark read, the bar's third key: rows lie under the bar, and none of them takes the tap.
    let h = app.canvas.h;
    let ky = h - (50.0 + 24.0) + 8.0 + 25.0;
    let gw = (WIDTH - 2.0 * crate::theme::PAD - 16.0) / 3.0;
    tap(&mut app, &mut t, crate::theme::PAD + 2.0 * (gw + 8.0) + gw / 2.0, ky);
    let d = app.series[&1].data.as_ref().unwrap();
    assert!(d.chapters.iter().filter(|c| c.id == 1007 || c.id == 1008).all(|c| c.read), "both marked read at once");
    assert!(!d.chapters.iter().find(|c| c.id == 1006).unwrap().read, "and nothing else");
    assert_eq!(app.st.series.select, None, "the selection is done");
    assert_eq!(app.route(), &Route::Series(1));
    // Back stops selecting before it leaves the page.
    hold(&mut app, &mut t, 200.0, y0 + row / 2.0);
    assert!(app.st.series.select.is_some());
    assert!(app.back());
    assert_eq!((app.st.series.select.clone(), app.route().clone()), (None, Route::Series(1)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_continue_key_at_the_bottom_takes_its_tap_not_the_row_under_it() {
    let (mut app, mut t, dir) = app("continue-key");
    app.series.insert(1, loaded(detail(1, 10)));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    tap(&mut app, &mut t, 60.0, h - (50.0 + 24.0) + 8.0 + 25.0);
    assert_eq!(app.route(), &Route::Reader);
    let r = app.reader.as_ref().unwrap();
    assert!(matches!(r.pos.item, crate::strip::Item::Head(4)), "chapter 5, after the four read: {:?}", r.pos);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn paid_episodes_ahead_open_their_link_from_a_row_at_the_newest_end() {
    let (mut app, mut t, dir) = app("paid");
    app.series.insert(1, loaded(detail(1, 10)));
    app.paid.insert(1, leaf_proto::PaidAhead { from: 11.0, to: 23.0, url: "https://www.webtoons.com/x".into() });
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let y0 = first_row(&app);
    // The paid row is first; Open at its right.
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 30.0, y0 + 35.0);
    assert_eq!(opened(&app), vec!["https://www.webtoons.com/x".to_string()]);
    assert_eq!(app.route(), &Route::Series(1), "no chapter opened");
    // The chapters come after it: chapter 10 is the second row.
    tap(&mut app, &mut t, 200.0, y0 + 70.0 + crate::series::ROW_H / 2.0);
    assert_eq!(app.route(), &Route::Reader);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_picks_sheet_takes_every_tap() {
    let (mut app, mut t, dir) = app("picks");
    nowhere(&mut app);
    app.recs.data = Some(picks(6, None));
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // The first pick, under the spotlight and its heading.
    let grid_top = app.canvas.inset_top + 56.0 + 4.0 + 412.0 + 14.0 + 32.0 + 22.5 + 14.0;
    hold(&mut app, &mut t, 60.0, grid_top + 60.0);
    assert_eq!(app.st.recs.sheet.as_ref().map(|s| s.rec.anilist_id), Some(101), "held: the sheet, not a page");
    assert_eq!(app.route(), &Route::Home);
    // A tap above the sheet, on the spotlight, only closes it.
    tap(&mut app, &mut t, 200.0, 200.0);
    assert!(app.st.recs.sheet.is_none());
    assert_eq!((app.route(), app.nav.len()), (&Route::Home, 1), "the spotlight under it did not open");
    step(&mut app, &mut t);
    // Again, then Not interested, the row above the last (Fewer like Action): gone from the picks.
    hold(&mut app, &mut t, 60.0, grid_top + 60.0);
    assert!(app.st.recs.sheet.is_some());
    let h = app.canvas.h;
    tap(&mut app, &mut t, 120.0, h - 8.0 - 52.0 - 26.0);
    assert!(app.st.recs.sheet.is_none());
    let top: Vec<i64> = app.recs.data.as_ref().unwrap().top.iter().map(|r| r.anilist_id).collect();
    assert!(!top.contains(&101), "hidden: {top:?}");
    assert_eq!((app.route(), app.nav.len()), (&Route::Home, 1));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_update_sheet_holds_the_page_and_later_puts_that_build_off() {
    let (mut app, mut t, dir) = app("update");
    let build = crate::update::BUILD + 1;
    crate::update::offered(&mut app, leaf_proto::AppLatest { build, version: String::new(), size: 18_000_000, sha256: String::new(), made_at: 0 });
    assert!(app.st.update.open);
    step(&mut app, &mut t);
    // The gear under the sheet's veil takes nothing.
    tap(&mut app, &mut t, WIDTH - 6.0 - 22.0, 28.0);
    assert_eq!(app.route(), &Route::Home, "the gear under the sheet did not open Settings");
    assert!(!app.st.update.open, "a tap outside it closes it");
    crate::update::offered(&mut app, leaf_proto::AppLatest { build, version: String::new(), size: 18_000_000, sha256: String::new(), made_at: 0 });
    step(&mut app, &mut t);
    // Later, the left key at the sheet's foot.
    let h = app.canvas.h;
    tap(&mut app, &mut t, 60.0, h - 18.0 - 24.0);
    assert!(!app.st.update.open);
    assert_eq!(app.update_later.map(|l| l.0), Some(build), "that build is put off");
    crate::update::offered(&mut app, leaf_proto::AppLatest { build, version: String::new(), size: 1, sha256: String::new(), made_at: 0 });
    assert!(!app.st.update.open, "and not offered again today");
    crate::update::offered(&mut app, leaf_proto::AppLatest { build: build + 1, version: String::new(), size: 1, sha256: String::new(), made_at: 0 });
    assert!(app.st.update.open, "a newer one is");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tap_in_the_reader_only_shows_the_overlay_and_auto_scrolls_until_tapped() {
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
    // The top and bottom thirds turn no page (the owner, 2026-11-23): a tap there shows the
    // overlay, and the next one puts it away.
    for y in [h * 0.1, h * 0.85] {
        let before = at(&app);
        tap(&mut app, &mut t, 200.0, y);
        for _ in 0..30 {
            step(&mut app, &mut t);
        }
        assert_eq!(at(&app), before, "the page stays put");
        assert!(app.reader.as_ref().unwrap().overlay, "the overlay shows");
        tap(&mut app, &mut t, 200.0, h * 0.5);
        assert!(!app.reader.as_ref().unwrap().overlay, "and goes away");
    }
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
        /// Pages answered as a scanlator's promo page: `204`, `x-leaf-skip: 1`.
        pub skip_pages: HashSet<(i64, u32)>,
        /// Series answered by `/v1/series/{id}`.
        pub details: HashMap<i64, leaf_proto::SeriesDetail>,
        /// Chapters put in the server's download line (`POST /v1/downloads`): downloaded at once.
        pub queued: Vec<i64>,
        /// What a search finds (each broad source, "A" and "B", answers the same).
        pub found: Vec<leaf_proto::Candidate>,
        /// "Did you mean".
        pub meant: Option<String>,
        /// What `/v1/preview` answers (id 0, as the server sends one).
        pub preview: Option<leaf_proto::SeriesDetail>,
        /// The series a follow makes (added to `details` too), and the follows asked.
        pub follow_as: Option<leaf_proto::SeriesDetail>,
        pub follows: Vec<leaf_proto::Follow>,
        /// Bells set: (series, on).
        pub bells: Vec<(i64, bool)>,
        /// Collections' pictures, by slug (the rest are a 404).
        pub tagart: HashSet<String>,
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
            crate::net::Server { base: self.base.clone(), token: None, alt: None }
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
        let mut extra = String::new();
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
                ("GET", ["v1", "chapters", id, "pages", n]) => {
                    let page = (id.parse().unwrap_or(0), n.split('?').next().unwrap_or("").parse().unwrap_or(u32::MAX));
                    if st.skip_pages.contains(&page) {
                        extra.push_str("x-leaf-skip: 1\r\n");
                        (204, "image/png", Vec::new())
                    } else if st.fail_pages.contains(&page.0) {
                        (503, "text/plain", b"busy".to_vec())
                    } else {
                        (200, "image/png", png.to_vec())
                    }
                }
                ("GET", ["v1", "series", id]) => match st.details.get(&id.parse().unwrap_or(0)) {
                    Some(d) => (200, "application/json", serde_json::to_vec(d).unwrap()),
                    None => (404, "text/plain", b"no".to_vec()),
                },
                ("POST", ["v1", "downloads"]) => {
                    let ids = serde_json::from_slice::<leaf_proto::DownloadIds>(&body).map(|d| d.ids).unwrap_or_default();
                    st.queued.extend(&ids);
                    for d in st.details.values_mut() {
                        for c in d.chapters.iter_mut().filter(|c| ids.contains(&c.id)) {
                            c.downloaded = true;
                        }
                    }
                    (200, "application/json", format!("{{\"ok\":true,\"count\":{}}}", ids.len()).into_bytes())
                }
                ("GET", ["v1", "library"]) => {
                    let list: Vec<&leaf_proto::Series> = st.details.values().map(|d| &d.series).collect();
                    (200, "application/json", serde_json::to_vec(&list).unwrap())
                }
                ("GET", ["v1", "search", q]) if q.starts_with("sources") => (200, "application/json", br#"{"broad":["A","B"],"all":3}"#.to_vec()),
                ("GET", ["v1", "search", q]) if q.starts_with("suggest") => (200, "application/json", serde_json::to_vec(&serde_json::json!({ "title": st.meant })).unwrap()),
                ("GET", ["v1", q]) if q.starts_with("search?") => (200, "application/json", serde_json::to_vec(&st.found).unwrap()),
                ("GET", ["v1", q]) if q.starts_with("preview?") => match &st.preview {
                    Some(d) => (200, "application/json", serde_json::to_vec(d).unwrap()),
                    None => (502, "application/json", br#"{"error":"no entry answered"}"#.to_vec()),
                },
                ("POST", ["v1", "follow"]) => {
                    if let Ok(f) = serde_json::from_slice::<leaf_proto::Follow>(&body) {
                        st.follows.push(f);
                    }
                    match st.follow_as.clone() {
                        Some(d) => {
                            st.details.insert(d.series.id, d.clone());
                            (200, "application/json", serde_json::to_vec(&d.series).unwrap())
                        }
                        None => (502, "application/json", br#"{"error":"no source has chapters of it"}"#.to_vec()),
                    }
                }
                ("POST", ["v1", "series", id, "bell"]) => {
                    let on = serde_json::from_slice::<leaf_proto::Bell>(&body).map(|b| b.on).unwrap_or(false);
                    let id: i64 = id.parse().unwrap_or(0);
                    st.bells.push((id, on));
                    if let Some(d) = st.details.get_mut(&id) {
                        d.series.bell = on;
                    }
                    (200, "application/json", b"{\"ok\":true}".to_vec())
                }
                ("POST", ["v1", "series", _, "watch"]) => (200, "application/json", b"{\"ok\":true}".to_vec()),
                ("GET", ["v1", "recs", "tagart", slug]) if st.tagart.contains(*slug) => (200, "image/png", png.to_vec()),
                ("POST", ["v1", "chapters", id, "progress"]) => {
                    if let Ok(p) = serde_json::from_slice::<leaf_proto::Progress>(&body) {
                        st.progress.push((id.parse().unwrap_or(0), p));
                    }
                    (200, "application/json", b"{\"ok\":true}".to_vec())
                }
                _ => (404, "text/plain", b"no".to_vec()),
            }
        };
        let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Type: {kind}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", out.len());
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
fn a_chapter_key_lands_on_the_chapter_heading() {
    let (mut app, mut t, dir) = app("reader-jump");
    let d = comic(1, &[5, 5]);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
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

/// The places kept in a data folder, once the writer's thread has written one for `series`.
fn kept_place(dir: &std::path::Path, series: i64, done: impl Fn(&crate::places::Place) -> bool) -> Option<crate::places::Place> {
    for _ in 0..400 {
        let p = crate::places::Places::load(dir);
        if let Some(p) = p.get(series).filter(|p| done(p)) {
            return Some(*p);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}

#[test]
fn the_app_closed_mid_chapter_reopens_at_the_same_page_and_offset() {
    let (mut app, mut t, dir) = app("reader-place");
    app.shot = false;
    let d = comic(1, &[20, 20]);
    measure(&mut app, &d);
    app.series.insert(1, loaded(d.clone()));
    app.open_chapter(&d, 0, 0);
    step(&mut app, &mut t);
    // Chapter 1's heading (64), five pages (780 each) and a third of the sixth.
    scroll(&mut app, &mut t, 64.0 + 780.0 * 5.0 + 333.0);
    let at = reader(&app).pos;
    assert_eq!(at.item, crate::strip::Item::Page(0, 5), "{at:?}");
    assert!((at.off - 333.0).abs() < 0.5, "{at:?}");
    // A new page is kept at once, before anything else happens.
    assert!(kept_place(&dir, 1, |p| p.page == 5).is_some(), "kept on the page change");
    // Within the page, a little further: held back (two seconds), then the app goes to the
    // background and Android ends it.
    scroll(&mut app, &mut t, 40.0);
    let at = reader(&app).pos;
    app.paused();
    drop(app);
    let kept = kept_place(&dir, 1, |p| (p.off - at.off).abs() < 0.5).expect("the exact place is on disk");
    assert_eq!((kept.chapter, kept.page), (1001, 5));
    // Started again; the server's copy knows nothing of the page (its report never landed).
    let mut app = App::new(Arc::new(TestHost(dir.clone())), 0);
    app.resize(1080.0, 2340.0, 0.0, 0.0);
    app.library.data = Some(vec![series(1, "Series 1")]);
    app.series.insert(1, loaded(d.clone()));
    measure(&mut app, &d);
    let mut t = 1_000_000_000;
    app.frame(t);
    app.continue_series(1);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let back = reader(&app).pos;
    assert_eq!(back.item, at.item, "the same page");
    assert!((back.off - at.off).abs() < 0.5, "the same offset: {back:?} against {at:?}");
    // The overlay says where the place is kept.
    let h = app.canvas.h;
    tap(&mut app, &mut t, 200.0, h * 0.5);
    assert!(reader(&app).overlay);
    assert_eq!(reader(&app).saved.map(|p| p.page), Some(5));
    // Closing the reader keeps it too; the next Continue opens there again.
    scroll(&mut app, &mut t, 780.0);
    let moved = reader(&app).pos;
    app.pop();
    assert!(kept_place(&dir, 1, |p| p.page == 6 && (p.off - moved.off).abs() < 0.5).is_some(), "kept as the reader closed");
    let _ = std::fs::remove_dir_all(dir);
}

/// Where `item` is on screen (its top's y), if it is.
fn on_screen(app: &App, item: crate::strip::Item) -> Option<f32> {
    reader(app).visible_items(&app.images, app.canvas.h).into_iter().find(|(i, _)| *i == item).map(|(_, y)| y)
}

#[test]
fn a_promo_page_the_server_skips_leaves_the_strip_without_moving_the_place() {
    let (mut app, mut t, dir) = app("reader-skip");
    app.shot = false;
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    // Chapter 1's third page is a promo page; so is chapter 2's last.
    fake.state.lock().unwrap().skip_pages.extend([(1001, 2), (1002, 2)]);
    let d = comic(1, &[6, 3]);
    // Every other page is measured as it will be (780 tall).
    for (c, n) in [(1001, 0), (1001, 1), (1001, 3), (1001, 4), (1001, 5), (1002, 0), (1002, 1)] {
        app.images.fake_size(c, n, 800, 1600);
    }
    app.open_chapter(&d, 0, 2);
    // The screen's top 200 units down the promo page, still a placeholder: page 4 is 360 down.
    if let Some(r) = &mut app.reader {
        r.pos = crate::strip::Pos { item: crate::strip::Item::Page(0, 2), off: 200.0 };
    }
    step(&mut app, &mut t);
    let before = on_screen(&app, crate::strip::Item::Page(0, 3)).expect("page 4 on screen");
    assert!((before - 360.0).abs() < 0.5, "{before}");
    assert!(until(&mut app, &mut t, 5.0, |a| a.images.skipped(1001, 2)), "the server said to skip it");
    step(&mut app, &mut t);
    assert!(app.images.failed(1001, 2).is_none(), "not a page that didn't load");
    let after = on_screen(&app, crate::strip::Item::Page(0, 3)).expect("page 4 still on screen");
    assert!((after - before).abs() < 0.5, "nothing below it moved: {before} then {after}");
    assert_ne!(reader(&app).pos.item, crate::strip::Item::Page(0, 2), "the place is no longer in it");
    assert!(dir.join("cache").join("1001").join("2.skip").is_file(), "kept on disk, not asked for again");
    // Read on to chapter 2's end, its promo page left out: chapter 2 is finished all the same.
    for _ in 0..14 {
        scroll(&mut app, &mut t, 500.0);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(until(&mut app, &mut t, 5.0, |a| a.reported.iter().any(|(c, p)| *c == 1002 && p.done)), "chapter 2 finished: {:?}", app.reported);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_long_press_on_a_page_offers_to_hide_it_and_the_sheet_holds_every_tap() {
    let (mut app, mut t, dir) = app("reader-hide");
    nowhere(&mut app);
    let d = comic(1, &[6]);
    measure(&mut app, &d);
    app.open_chapter(&d, 0, 2);
    if let Some(r) = &mut app.reader {
        r.pos = crate::strip::Pos { item: crate::strip::Item::Page(0, 2), off: 100.0 };
    }
    step(&mut app, &mut t);
    let h = app.canvas.h;
    let at = reader(&app).pos;
    // Held on the page: the sheet, not the overlay.
    hold(&mut app, &mut t, 200.0, 300.0);
    assert_eq!(reader(&app).hide_sheet, Some((0, 2)), "the page under the finger");
    assert!(!reader(&app).overlay);
    // A tap above the sheet only closes it: no overlay, no page turn.
    tap(&mut app, &mut t, 200.0, h * 0.1);
    for _ in 0..30 {
        step(&mut app, &mut t);
    }
    let r = reader(&app);
    assert!(r.hide_sheet.is_none() && !r.overlay, "closed, and nothing under it took the tap");
    assert_eq!(r.pos, at, "the strip did not move");
    // Again, then Hide: the page leaves at once, page 4 staying where it was on screen.
    let below = on_screen(&app, crate::strip::Item::Page(0, 3)).unwrap();
    hold(&mut app, &mut t, 200.0, 300.0);
    assert!(reader(&app).hide_sheet.is_some());
    tap(&mut app, &mut t, 200.0, h - 192.0 + 76.0 + 26.0);
    assert!(reader(&app).hide_sheet.is_none());
    assert!(app.images.skipped(1001, 2), "hidden");
    let moved = on_screen(&app, crate::strip::Item::Page(0, 3)).unwrap();
    assert!((moved - below).abs() < 0.5, "{below} then {moved}");
    assert!(reader(&app).undo.is_some(), "Undo offered");
    // Undo, at the bottom (a frame on, as the screen draws after the sheet goes): the page is back.
    step(&mut app, &mut t);
    tap(&mut app, &mut t, WIDTH / 2.0, h - 16.0 - 24.0);
    assert!(!app.images.skipped(1001, 2), "shown again");
    assert!(reader(&app).undo.is_none());
    assert!(!reader(&app).overlay, "the Undo took the tap, not the strip");
    let _ = std::fs::remove_dir_all(dir);
}

// Downloads: chapters kept on this phone

/// A small page picture (a flat colour), as the fake server sends.
fn png() -> Vec<u8> {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(80, 160, image::Rgba([120, 90, 160, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}

/// A chapter downloaded to this phone, on disk as the line leaves one: `pages` pictures and its
/// manifest. Returns its bytes.
fn saved_on_disk(dir: &std::path::Path, series: i64, chapter: i64, number: f64, pages: u32, read: bool) -> u64 {
    let d = crate::offline::chapter_dir(dir, chapter);
    std::fs::create_dir_all(&d).unwrap();
    let p = png();
    for n in 0..pages {
        std::fs::write(d.join(n.to_string()), &p).unwrap();
    }
    let bytes = p.len() as u64 * pages as u64;
    let chapter = leaf_proto::Chapter { id: chapter, number, name: String::new(), group: None, pages, read, last_page: 0, downloaded: false, uploaded_at: 0, from: None };
    let m = crate::offline::Manifest { series, title: format!("Series {series}"), chapter, pages, skipped: vec![], bytes, saved_at: 1_700_000_000_000 + series * 1000 + number as i64 };
    std::fs::write(d.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    bytes
}

/// An app whose data folder already holds what `fill` puts there, on a 1080 × 2340 screen, with no
/// library and a server that isn't there; its downloads read.
fn app_on(name: &str, fill: impl Fn(&std::path::Path)) -> (App, u64, PathBuf) {
    let dir = std::env::temp_dir().join(format!("leaf-flows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    fill(&dir);
    let mut app = App::new(Arc::new(TestHost(dir.clone())), 0);
    app.shot = true;
    app.st.online.off = true;
    nowhere(&mut app);
    app.resize(1080.0, 2340.0, 0.0, 0.0);
    let mut t = 1_000_000_000;
    app.frame(t);
    assert!(until(&mut app, &mut t, 5.0, |a| a.saver.scanned()), "the downloads were read");
    (app, t, dir)
}

/// Waits (real time) until `done`.
fn eventually(secs: f32, done: impl Fn() -> bool) -> bool {
    for _ in 0..(secs * 200.0) as usize {
        if done() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

#[test]
fn a_download_survives_the_page_cache_cleanup() {
    let dir = std::env::temp_dir().join(format!("leaf-flows-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    saved_on_disk(&dir, 1, 1001, 1.0, 2, false);
    // The same chapter in the page cache, and another, both read long ago: the cleanup takes both.
    for c in [1001, 1002] {
        let cache = crate::images::chapter_dir(&dir, c);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("0"), png()).unwrap();
        std::fs::write(cache.join(".used"), "1").unwrap();
    }
    crate::images::prune_cache(dir.clone(), Vec::new());
    assert!(eventually(5.0, || !crate::images::chapter_dir(&dir, 1001).exists() && !crate::images::chapter_dir(&dir, 1002).exists()), "the cache was trimmed");
    let kept = crate::offline::chapter_dir(&dir, 1001);
    assert!(kept.join("0").is_file() && kept.join("1").is_file() && kept.join("manifest.json").is_file(), "the download is all there");
    let saver = crate::offline::Saver::with_timing(dir.clone(), crate::offline::quick());
    assert!(eventually(5.0, || saver.scanned()));
    assert_eq!(saver.state(1001), crate::offline::Kept::Saved);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn downloads_wait_their_turn_survive_a_restart_and_resume_where_they_stopped() {
    let dir = std::env::temp_dir().join(format!("leaf-flows-line-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fake = fake::Fake::start();
    {
        let mut st = fake.state.lock().unwrap();
        let mut d = comic(1, &[3, 2]);
        // The server has chapter 2 already; chapter 1 it must download first.
        d.chapters[0].downloaded = false;
        st.details.insert(1, d);
        st.counts.extend([(1001, 3), (1002, 2)]);
        // Chapter 1's second page is a scanlator's promo page.
        st.skip_pages.insert((1001, 1));
    }
    let chapters = comic(1, &[3, 2]).chapters;
    // Asked for with no server: they wait in line, kept on disk.
    let first = crate::offline::Saver::with_timing(dir.clone(), crate::offline::quick());
    assert!(eventually(5.0, || first.scanned()));
    assert_eq!(first.add(1, "Series 1", &chapters), 2);
    assert_eq!(first.add(1, "Series 1", &chapters), 0, "asked twice, in line once");
    assert_eq!(first.state(1001), crate::offline::Kept::Waiting);
    assert!(eventually(5.0, || std::fs::read_to_string(dir.join("offline").join("queue.json")).is_ok_and(|q| q.contains("1002"))), "the line is on disk");
    // Chapter 2 was half saved when the app stopped: its first page is here.
    let part = crate::offline::chapter_dir(&dir, 1002);
    std::fs::create_dir_all(&part).unwrap();
    std::fs::write(part.join("0"), png()).unwrap();
    drop(first);
    // The app starts again, with its server.
    let saver = crate::offline::Saver::with_timing(dir.clone(), crate::offline::quick());
    saver.set_server(Some(fake.server()));
    assert!(eventually(10.0, || saver.is_saved(1001) && saver.is_saved(1002)), "both saved: {:?} {:?}", saver.state(1001), saver.state(1002));
    // Chapter 1 went through the server's own line first; its pages came after.
    assert_eq!(fake.state.lock().unwrap().queued.first(), Some(&1001));
    let hits = fake.state.lock().unwrap().hits.clone();
    let queued_at = hits.iter().position(|h| h == "POST /v1/downloads").expect("the server was asked to download it");
    let first_page = hits.iter().position(|h| h.starts_with("GET /v1/chapters/1001/pages/")).expect("its pages were fetched");
    assert!(queued_at < first_page, "{hits:?}");
    // One chapter at a time: every page of chapter 1 before any of chapter 2.
    let last_of_1 = hits.iter().rposition(|h| h.starts_with("GET /v1/chapters/1001/pages/")).unwrap();
    assert!(hits.iter().position(|h| h.starts_with("GET /v1/chapters/1002/pages/")).is_some_and(|i| i > last_of_1), "{hits:?}");
    // The promo page is left out, not a failure; the page already here wasn't fetched again.
    let m = saver.manifest(1001).unwrap();
    assert_eq!((m.pages, m.skipped.clone()), (3, vec![1]));
    assert!(crate::offline::chapter_dir(&dir, 1001).join("1.skip").is_file());
    assert_eq!(fake.hits("/v1/chapters/1002/pages/0"), 0, "resumed where it stopped");
    assert_eq!(fake.hits("/v1/chapters/1002/pages/1"), 1);
    assert!(m.bytes > 0 && saver.manifest(1002).unwrap().bytes > 0);
    assert!(eventually(5.0, || !dir.join("offline").join("queue.json").exists()), "nothing left in line");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_download_that_fails_says_so_and_goes_again_on_retry() {
    let dir = std::env::temp_dir().join(format!("leaf-flows-retry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fake = fake::Fake::start();
    {
        let mut st = fake.state.lock().unwrap();
        st.details.insert(1, comic(1, &[2]));
        st.counts.insert(1001, 2);
        st.fail_pages.insert(1001);
    }
    let saver = crate::offline::Saver::with_timing(dir.clone(), crate::offline::quick());
    saver.set_server(Some(fake.server()));
    saver.add(1, "Series 1", &comic(1, &[2]).chapters);
    assert!(eventually(10.0, || matches!(saver.state(1001), crate::offline::Kept::Failed(_))), "{:?}", saver.state(1001));
    assert_eq!(saver.running().failed, 1);
    fake.state.lock().unwrap().fail_pages.clear();
    saver.retry(&[]);
    assert!(eventually(10.0, || saver.is_saved(1001)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reading_a_downloaded_chapter_asks_the_server_nothing() {
    let (mut app, mut t, dir) = app_on("offline-read", |d| {
        saved_on_disk(d, 1, 1001, 1.0, 3, false);
    });
    // A server that answers, and counts what it is asked.
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.images.set_server(app.server.clone());
    // The series as listed, its length not known yet.
    let d = comic(1, &[0]);
    app.open_chapter(&d, 0, 0);
    assert_eq!(reader(&app).count(0), Some(3), "its length is the download's");
    assert!(until(&mut app, &mut t, 5.0, |a| (0..3).all(|n| a.images.size(1001, n).is_some())), "the pages came from this phone");
    assert_eq!(fake.state.lock().unwrap().hits, Vec::<String>::new(), "no request at all");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_downloaded_chapter_reads_beside_a_server_that_hangs() {
    let (mut app, mut t, dir) = app_on("offline-hang", |d| {
        saved_on_disk(d, 1, 1002, 2.0, 3, false);
    });
    // A server that takes requests and never answers: every request worker is held by it.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    app.server = Some(crate::net::Server { base: format!("http://{}", listener.local_addr().unwrap()), token: None, alt: None });
    app.images.set_server(app.server.clone());
    for _ in 0..4 {
        app.ask(crate::net::Job::Library);
    }
    // Chapter 1 is read from the server; chapter 2, below it, is on this phone.
    let d = comic(1, &[2, 0]);
    app.open_chapter(&d, 0, 0);
    if let Some(r) = &mut app.reader {
        r.pos = crate::strip::Pos { item: crate::strip::Item::Head(1), off: 0.0 };
    }
    assert!(until(&mut app, &mut t, 2.0, |a| a.reader.as_ref().is_some_and(|r| r.count(1) == Some(3))), "its length is the download's");
    assert!(until(&mut app, &mut t, 5.0, |a| (0..3).all(|n| a.images.size(1002, n).is_some())), "and its pages come from this phone");
    drop(listener);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_series_only_webtoon_has_can_be_watched_for_another_source() {
    let (mut app, mut t, dir) = app("watch");
    let fake = fake::Fake::start();
    app.server = Some(fake.server());
    app.shot = false;
    app.series.insert(1, loaded(detail(1, 10)));
    app.watch.insert(1, false);
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let y0 = first_row(&app);
    // No paid row: the watch row is first; Watch at its right.
    tap(&mut app, &mut t, WIDTH - crate::theme::PAD - 30.0, y0 + 28.0);
    assert_eq!(app.watch.get(&1), Some(&true), "watching at once");
    assert_eq!(app.route(), &Route::Series(1), "no chapter opened");
    assert!(until(&mut app, &mut t, 5.0, |_| fake.hits("/v1/series/1/watch") == 1), "the server is told: {:?}", fake.state.lock().unwrap().hits);
    // The chapters come after it.
    tap(&mut app, &mut t, 200.0, y0 + 56.0 + crate::series::ROW_H / 2.0);
    assert_eq!(app.route(), &Route::Reader);
    let _ = std::fs::remove_dir_all(dir);
}

/// Where Downloads' first series row starts: under the tabs (104), the size line (12 + 36) and 4.
const DOWNLOADS_ROW: f32 = 104.0 + 12.0 + 36.0 + 4.0;

#[test]
fn downloads_show_and_read_with_the_server_down() {
    let (mut app, mut t, dir) = app_on("offline-list", |d| {
        saved_on_disk(d, 5, 5001, 1.0, 2, true);
        saved_on_disk(d, 5, 5002, 2.0, 2, false);
        saved_on_disk(d, 6, 6001, 1.0, 1, false);
    });
    app.library = Default::default();
    app.st.space.tab = crate::space::DOWNLOADS;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // Two series, the one saved to last first, from the folder alone.
    let groups = app.saver.groups();
    assert_eq!(groups.iter().map(|g| (g.series, g.chapters)).collect::<Vec<_>>(), vec![(6, 1), (5, 2)]);
    tap(&mut app, &mut t, 200.0, DOWNLOADS_ROW + crate::space::ROW_H + 40.0);
    assert_eq!(app.route(), &Route::Saved(5), "the row opened its series' downloads");
    step(&mut app, &mut t);
    // Newest first: chapter 2's row, under the top (60) and the line (8 + its height + 14).
    let line = crate::draw::Text::new(crate::theme::ty::UI, crate::theme::INK).lh();
    tap(&mut app, &mut t, 150.0, 60.0 + 8.0 + line + 14.0 + 35.0);
    assert_eq!(app.route(), &Route::Reader);
    let r = reader(&app);
    assert_eq!((r.series, r.chapters.iter().map(|c| c.id).collect::<Vec<_>>()), (5, vec![5001, 5002]), "the saved chapters, read offline");
    assert!(matches!(r.pos.item, crate::strip::Item::Head(1)), "{:?}", r.pos);
    assert!(until(&mut app, &mut t, 5.0, |a| (0..2).all(|n| a.images.size(5002, n).is_some())));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn selected_downloads_are_deleted_and_nothing_else() {
    let mut sizes = std::collections::HashMap::new();
    let (mut app, mut t, dir) = app_on("delete", |d| {
        saved_on_disk(d, 5, 5001, 1.0, 2, true);
        saved_on_disk(d, 5, 5002, 2.0, 3, false);
        saved_on_disk(d, 6, 6001, 1.0, 1, false);
        saved_on_disk(d, 7, 7001, 1.0, 1, true);
    });
    for id in [5001, 5002, 6001, 7001] {
        sizes.insert(id, app.saver.manifest(id).unwrap().bytes);
    }
    app.st.space.tab = crate::space::DOWNLOADS;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    // Rows: 7, 6, 5. A long press selects series 7; a tap adds 5.
    let row = crate::space::ROW_H;
    hold(&mut app, &mut t, 200.0, DOWNLOADS_ROW + row / 2.0);
    assert_eq!(app.route(), &Route::Space, "the long press opened nothing");
    tap(&mut app, &mut t, 200.0, DOWNLOADS_ROW + 2.0 * row + row / 2.0);
    assert_eq!(app.st.space.select.clone().unwrap().into_iter().collect::<Vec<_>>(), vec![5, 7]);
    // "Delete read", the bar's second key: only the read chapters of those two.
    let h = app.canvas.h;
    let key_y = h - crate::saved::BAR_H + 12.0 + 25.0;
    let kw = (WIDTH - 2.0 * crate::theme::PAD - 8.0) / 2.0;
    tap(&mut app, &mut t, crate::theme::PAD + kw + 8.0 + kw / 2.0, key_y);
    let mut asked = app.st.space.confirm.clone().expect("the confirm");
    asked.sort();
    assert_eq!(asked, vec![5001, 7001]);
    // Cancel, then the whole of both with Delete 2.
    app.st.space.confirm = None;
    step(&mut app, &mut t);
    tap(&mut app, &mut t, crate::theme::PAD + kw / 2.0, key_y);
    let mut asked = app.st.space.confirm.clone().expect("the confirm");
    asked.sort();
    assert_eq!(asked, vec![5001, 5002, 7001]);
    // What the confirm's Delete does (its key is tapped in the test after this one).
    app.st.space.confirm = None;
    let (n, freed) = app.saver.delete(&asked);
    assert_eq!((n, freed), (3, sizes[&5001] + sizes[&5002] + sizes[&7001]));
    assert!(eventually(5.0, || [5001, 5002, 7001].iter().all(|c| !crate::offline::chapter_dir(&dir, *c).exists())), "their folders are gone");
    assert!(crate::offline::chapter_dir(&dir, 6001).join("manifest.json").is_file(), "the other stays");
    assert_eq!(app.saver.groups().iter().map(|g| g.series).collect::<Vec<_>>(), vec![6]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_confirm_deletes_and_the_select_bars_hold_their_taps() {
    let (mut app, mut t, dir) = app_on("delete-bar", |d| {
        // Enough rows to lie under the bar at the bottom.
        for s in 1..=30 {
            saved_on_disk(d, s, s * 1000 + 1, 1.0, 1, false);
        }
    });
    app.st.space.tab = crate::space::DOWNLOADS;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    app.st.space.select = Some([30].into_iter().collect());
    step(&mut app, &mut t);
    let h = app.canvas.h;
    // Just above the bar's keys, inside the bar: a row lies under it, and is not toggled.
    let bar_top = h - crate::saved::BAR_H;
    tap(&mut app, &mut t, 200.0, bar_top + 4.0);
    assert_eq!(app.st.space.select.clone().unwrap().len(), 1, "the row under the bar took no tap");
    assert_eq!(app.route(), &Route::Space);
    // Delete 1, then the confirm's Delete (right half of its keys).
    tap(&mut app, &mut t, crate::theme::PAD + 40.0, bar_top + 12.0 + 25.0);
    assert_eq!(app.st.space.confirm, Some(vec![30001]));
    step(&mut app, &mut t);
    let w = WIDTH - 48.0;
    let bt = crate::draw::Text::new(crate::theme::ty::UI, crate::theme::INK2).line(1.45);
    let bytes = app.saver.manifest(30001).unwrap().bytes;
    let body = format!("That frees {} on this phone. Your server keeps its own copies as before; download them again any time.", crate::space::size_words(bytes));
    let bh = app.canvas.para_h(&body, w - 40.0, bt, 6);
    let ch = 22.0 + 24.0 + 10.0 + bh + 22.0 + 44.0 + 20.0;
    let by = (h - ch) / 2.0 + ch - 20.0 - 44.0;
    tap(&mut app, &mut t, 24.0 + w - 20.0 - 30.0, by + 22.0);
    assert!(!app.saver.is_saved(30001), "deleted");
    assert_eq!(app.st.space.select, None, "selecting is done");
    assert_eq!(app.saver.groups().len(), 29);
    // A series' own downloads: the same bar holds its taps from the rows under it.
    app.push(Route::Saved(29));
    step(&mut app, &mut t);
    app.st.saved.select = Some(std::collections::BTreeSet::new());
    step(&mut app, &mut t);
    tap(&mut app, &mut t, 200.0, bar_top + 4.0);
    assert_eq!(app.route(), &Route::Saved(29));
    assert!(app.st.saved.select.as_ref().is_some_and(|s| s.is_empty()));
    let _ = std::fs::remove_dir_all(dir);
}

// Search, previews and picks

/// An app on a fake server that writes (not a screenshot), with Search open and its field focused.
fn searching(name: &str, fake: &fake::Fake) -> (App, u64, PathBuf) {
    let (mut app, mut t, dir) = app(name);
    app.shot = false;
    app.server = Some(fake.server());
    app.reset_to(Route::Search);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    assert!(app.focus.is("search"), "the field takes the keyboard");
    (app, t, dir)
}

/// The keyboard sends the field's whole text (Android's input state), then the typing pauses.
fn type_words(app: &mut App, t: &mut u64, words: &str) {
    app.set_text(words);
    step(app, t);
}

#[test]
fn the_field_still_takes_typing_after_nothing_was_found() {
    let fake = fake::Fake::start();
    fake.state.lock().unwrap().meant = Some("The Awakening Contract".into());
    let (mut app, mut t, dir) = searching("search-type", &fake);
    type_words(&mut app, &mut t, "aweking contract");
    assert!(until(&mut app, &mut t, 5.0, |a| matches!(&a.st.search.results, Some(Ok(l)) if l.is_empty()) && !a.st.search.more), "nothing found");
    assert!(until(&mut app, &mut t, 5.0, |a| a.st.search.meant().is_some()), "AniList was asked what it meant");
    step(&mut app, &mut t);
    // The empty state and its keys are up: typing goes on reaching the field.
    for words in ["aweking contrac", "aweking contra", "awakening contra"] {
        type_words(&mut app, &mut t, words);
        assert!(app.focus.is("search"), "the field kept the keyboard");
        assert_eq!(app.st.search.query, words);
    }
    // Enter searches and lets the keyboard go; text the keyboard still sends after that (some
    // keep it up) reaches the field again, rather than nowhere.
    app.done_typing();
    step(&mut app, &mut t);
    type_words(&mut app, &mut t, "awakening contract");
    assert!(app.focus.is("search"), "focused again by what was typed");
    assert_eq!(app.st.search.query, "awakening contract");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn did_you_mean_searches_what_it_offers() {
    let fake = fake::Fake::start();
    fake.state.lock().unwrap().meant = Some("The Awakening Contract".into());
    let (mut app, mut t, dir) = searching("search-meant", &fake);
    type_words(&mut app, &mut t, "aweking contract");
    assert!(until(&mut app, &mut t, 5.0, |a| a.st.search.meant() == Some("The Awakening Contract")));
    let asked = fake.hits("/v1/search/suggest?q=aweking+contract");
    assert_eq!(asked, 1, "AniList asked once");
    // The line sits under the notice: find it by tapping down the page until the search changes.
    let mut y = 120.0;
    while app.st.search.asked.as_deref() == Some("aweking contract") && y < 500.0 {
        tap(&mut app, &mut t, WIDTH / 2.0, y);
        y += 8.0;
    }
    assert_eq!(app.st.search.asked.as_deref(), Some("The Awakening Contract"), "the title it offered, searched");
    assert_eq!(app.st.search.query, "The Awakening Contract", "and in the field");
    assert_eq!(app.route(), &Route::Search, "nothing else under it took the tap");
    let _ = std::fs::remove_dir_all(dir);
}

/// A title the fake finds: entry 77 previews with chapters 1–5 (ids 7701…); following it makes
/// series 9, whose chapters (ids 9001…) are another entry's.
fn awakening(fake: &fake::Fake) {
    let mut st = fake.state.lock().unwrap();
    st.found = vec![leaf_proto::Candidate { title: "The Awakening Contract".into(), cover_of: Some(77), hits: vec![leaf_proto::Hit { manga_id: 77, source: "A".into() }, leaf_proto::Hit { manga_id: 78, source: "B".into() }], followed: None, paid: None }];
    let mut d = comic(0, &[20; 5]);
    d.series.title = "The Awakening Contract".into();
    for c in &mut d.chapters {
        c.id += 6700;
    }
    d.series.read_upto = None;
    d.series.last_read_at = 0;
    st.preview = Some(d.clone());
    let mut f = d;
    f.series.id = 9;
    for c in &mut f.chapters {
        c.id += 9000 - 7700;
    }
    st.follow_as = Some(f);
}

#[test]
fn a_search_result_opens_a_preview_that_adds_nothing_until_a_download() {
    let fake = fake::Fake::start();
    awakening(&fake);
    let (mut app, mut t, dir) = searching("preview-get", &fake);
    let before = app.library.data.as_ref().map(|l| l.len());
    type_words(&mut app, &mut t, "awakening contract");
    assert!(until(&mut app, &mut t, 5.0, |a| matches!(&a.st.search.results, Some(Ok(l)) if l.len() == 1) && !a.st.search.more));
    // The row opens the title's page: a preview, under its first entry.
    tap(&mut app, &mut t, 200.0, 60.0 + 8.0 + 26.0 + 54.0);
    assert_eq!(app.route(), &Route::Series(-77));
    assert!(until(&mut app, &mut t, 5.0, |a| a.series.get(&-77).is_some_and(|l| l.data.as_ref().is_some_and(|d| d.chapters.len() == 5))));
    assert!(fake.hits("/v1/preview?manga_id=77,78") >= 1, "its entries, in order");
    assert!(fake.state.lock().unwrap().follows.is_empty(), "opening it added nothing");
    assert_eq!(app.library.data.as_ref().map(|l| l.len()), before);
    // The download key beside Follow, then "The next 5 chapters".
    tap(&mut app, &mut t, WIDTH - 6.0 - 88.0 + 22.0, 28.0);
    assert!(app.st.series.download, "the download choices");
    let top = app.canvas.h - (76.0 + 52.0 * 4.0 + 12.0);
    tap(&mut app, &mut t, 200.0, top + 76.0 + 52.0 * 2.0 + 26.0);
    // Added now, and its page is the series' own; the chapters go by number into the line.
    assert!(until(&mut app, &mut t, 5.0, |a| a.route() == &Route::Series(9)), "the page became the series': {:?}", app.route());
    let f = fake.state.lock().unwrap().follows.clone();
    assert_eq!(f.iter().map(|f| f.manga_ids.clone()).collect::<Vec<_>>(), vec![vec![77, 78]], "added once, with its entries");
    assert!(until(&mut app, &mut t, 5.0, |a| (9001..=9005).all(|c| a.saver.state(c) != crate::offline::Kept::None)), "the series' own chapters are in the line");
    assert!(app.saver.series_ids().contains(&9));
    assert!(!app.previews.contains_key(&-77));
    let _ = app.saver.delete(&(9001..=9005).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_preview_followed_is_added_with_its_bell_on() {
    let fake = fake::Fake::start();
    awakening(&fake);
    let (mut app, mut t, dir) = searching("preview-follow", &fake);
    app.open_preview("The Awakening Contract", &fake.state.lock().unwrap().found[0].hits.clone(), None);
    assert!(until(&mut app, &mut t, 5.0, |a| a.series.get(&-77).is_some_and(|l| l.data.is_some())));
    // Follow: the pill left of the download key.
    let ft = crate::draw::Text::new(13.5, crate::theme::INK).medium();
    let fw = app.canvas.measure("Follow", ft) + 44.0;
    tap(&mut app, &mut t, WIDTH - 6.0 - 88.0 - 4.0 - fw / 2.0, 28.0);
    assert!(until(&mut app, &mut t, 5.0, |a| a.route() == &Route::Series(9)));
    assert!(until(&mut app, &mut t, 5.0, |_| fake.state.lock().unwrap().bells == vec![(9, true)]), "its bell on: {:?}", fake.state.lock().unwrap().bells);
    let s = app.library.data.as_ref().and_then(|l| l.iter().find(|s| s.id == 9).cloned()).expect("in the library");
    assert!(s.bell && crate::logic::in_space(&s, false), "in My space by its bell");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pick_tapped_opens_its_page_and_held_opens_its_sheet() {
    let (mut app, mut t, dir) = app("pick-tap");
    nowhere(&mut app);
    app.recs.data = Some(picks(6, None));
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let grid_top = app.canvas.inset_top + 56.0 + 4.0 + 412.0 + 14.0 + 32.0 + 22.5 + 14.0;
    tap(&mut app, &mut t, 60.0, grid_top + 60.0);
    assert_eq!(app.route(), &Route::Series(-501), "its page, a preview of its first entry");
    assert!(app.st.recs.sheet.is_none());
    assert_eq!(app.previews.get(&-501).and_then(|p| p.anilist), Some(101));
    app.pop();
    step(&mut app, &mut t);
    hold(&mut app, &mut t, 60.0, grid_top + 60.0);
    assert_eq!(app.st.recs.sheet.as_ref().map(|s| s.rec.anilist_id), Some(101), "held: the sheet");
    assert_eq!(app.route(), &Route::Home);
    let _ = std::fs::remove_dir_all(dir);
}

/// A finger put down at (x, y), moved `dy` in `steps` moves `ms` apart, then lifted.
fn pull(app: &mut App, t: &mut u64, x: f32, y: f32, dy: f32, steps: u64, ms: u64) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    let start = *t;
    app.touch(Touch { id: 1, phase: Phase::Down, x: x0 + x * u, y: y * u, t_ns: start });
    step(app, t);
    for i in 1..=steps {
        let f = i as f32 / steps as f32;
        app.touch(Touch { id: 1, phase: Phase::Move, x: x0 + x * u, y: (y + dy * f) * u, t_ns: start + i * ms * 1_000_000 });
        step(app, t);
    }
    // A slow pull rests before it lifts (no fling); a flick lifts at once.
    let lift = start + steps * ms * 1_000_000 + if ms >= 50 { 200_000_000 } else { 5_000_000 };
    app.touch(Touch { id: 1, phase: Phase::Up, x: x0 + x * u, y: (y + dy) * u, t_ns: lift });
    step(app, t);
}

#[test]
fn a_sheet_follows_its_grip_down_and_closes_or_springs_back() {
    let (mut app, mut t, dir) = app_with("sheet-drag", 12, 2340.0);
    nowhere(&mut app);
    app.recs.data = Some(picks(9, None));
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    let open = |app: &mut App, t: &mut u64| {
        let r = app.recs.data.as_ref().unwrap().top[0].clone();
        crate::recs::hit(app, &r, None, true);
        step(app, t);
        step(app, t);
    };
    open(&mut app, &mut t);
    let h = app.canvas.h;
    let scrolled = app.scroll().off;
    let rail = app.st.home.rail.off;
    // A short pull from the sheet's middle: it follows, then springs back and stays open.
    pull(&mut app, &mut t, 200.0, h - 150.0, 60.0, 6, 60);
    assert!(app.st.recs.sheet.is_some(), "a short pull springs back");
    for _ in 0..60 {
        step(&mut app, &mut t);
    }
    assert!(app.st.recs.sheet.is_some());
    // Mid-pull it is drawn lower, with the finger.
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    app.touch(Touch { id: 1, phase: Phase::Down, x: x0 + 200.0 * u, y: (h - 150.0) * u, t_ns: t });
    step(&mut app, &mut t);
    app.touch(Touch { id: 1, phase: Phase::Move, x: x0 + 200.0 * u, y: (h - 90.0) * u, t_ns: t + 100_000_000 });
    step(&mut app, &mut t);
    app.touch(Touch { id: 1, phase: Phase::Cancel, x: 0.0, y: 0.0, t_ns: t + 200_000_000 });
    for _ in 0..60 {
        step(&mut app, &mut t);
    }
    assert!(app.st.recs.sheet.is_some());
    // Past a third of its height, let go slowly: it closes.
    pull(&mut app, &mut t, 200.0, h - 300.0, 280.0, 8, 60);
    assert!(app.st.recs.sheet.is_none(), "pulled far enough, it closes");
    assert_eq!(app.route(), &Route::Home, "and nothing under it opened");
    assert_eq!(app.scroll().off, scrolled, "the page under it never moved");
    assert_eq!(app.st.home.rail.off, rail);
    // A quick flick down a little way closes it too.
    open(&mut app, &mut t);
    pull(&mut app, &mut t, 200.0, h - 150.0, 50.0, 3, 8);
    assert!(app.st.recs.sheet.is_none(), "flicked, it closes");
    assert_eq!(app.scroll().off, scrolled);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_id_given_to_another_series_lets_go_of_what_was_kept_for_the_old_one() {
    let fake = fake::Fake::start();
    let (mut app, mut t, dir) = app("reused-id");
    app.shot = false;
    app.server = Some(fake.server());
    // The phone knew 172 as Invincible Romance: its cover, its place.
    let mut old = detail(172, 3);
    old.series.title = "Invincible Romance".into();
    app.known.see(&old.series, true);
    let cover = crate::images::CoverOf::Series(172).file(&app.data);
    std::fs::create_dir_all(cover.parent().unwrap()).unwrap();
    std::fs::write(&cover, png()).unwrap();
    app.places.put(crate::places::Place { series: 172, chapter: 172_002, number: 2.0, page: 4, head: false, off: 0.0, at_ms: 1 });
    app.oldest_first.insert(172);
    // The server now names 172 The Awakening Contract.
    let mut new = detail(172, 3);
    new.series.title = "The Awakening Contract".into();
    new.series.added_at += 99;
    fake.state.lock().unwrap().details.insert(172, new);
    app.library.data = None;
    app.ask(crate::net::Job::Library);
    assert!(until(&mut app, &mut t, 5.0, |a| a.library.live));
    assert!(!cover.exists(), "the old cover is gone");
    assert!(app.places.get(172).is_none(), "and its place");
    assert!(!app.oldest_first.contains(&172));
    // Told once: the new one's own cover, kept from now on, stays.
    std::fs::write(&cover, png()).unwrap();
    app.ask(crate::net::Job::Library);
    step(&mut app, &mut t);
    assert!(until(&mut app, &mut t, 5.0, |a| !a.library.loading));
    assert!(cover.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn home_shows_no_binge_and_no_because_rail_of_one() {
    let (mut app, mut t, dir) = app("home-rails");
    nowhere(&mut app);
    let mut r = picks(3, None);
    let one = r.top[0].clone();
    r.because = vec![leaf_proto::Because { series: 1, title: "Series 1".into(), recs: vec![one.clone()] }, leaf_proto::Because { series: 2, title: "Series 2".into(), recs: r.top.clone() }];
    r.binge = Some(one);
    app.recs.data = Some(r);
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    assert_eq!(app.st.home.because.len(), 1, "only the rail of two or more");
    let _ = std::fs::remove_dir_all(dir);
}

// Out of reach: the prompt

/// The offline prompt's keys at the sheet's foot: (y, the left key's x, the right key's x).
fn prompt_keys(app: &App) -> (f32, f32, f32) {
    let x = crate::theme::PAD + 4.0;
    let bw = (WIDTH - 2.0 * x - 10.0) / 2.0;
    (app.canvas.h - app.canvas.inset_bottom - 18.0 - 24.0, x + bw / 2.0, x + bw + 10.0 + bw / 2.0)
}

#[test]
fn out_of_reach_the_prompt_shows_once_a_spell_and_opens_downloads() {
    let (mut app, mut t, dir) = app_on("offline-prompt", |d| {
        saved_on_disk(d, 5, 5001, 1.0, 2, false);
        saved_on_disk(d, 5, 5002, 2.0, 2, false);
    });
    app.st.online.off = false;
    app.reset_to(Route::Home);
    assert!(until(&mut app, &mut t, 10.0, |a| a.st.online.open), "the server out of reach: the prompt");
    assert_eq!(app.route(), &Route::Home);
    // It holds every tap: the gear under its veil opens nothing (the tap only closes it).
    tap(&mut app, &mut t, WIDTH - 6.0 - 22.0, 28.0);
    assert_eq!((app.route(), app.nav.len()), (&Route::Home, 1), "the gear under the prompt did not open Settings");
    assert!(!app.st.online.open);
    // Not again in this spell, however many asks fail.
    app.ask(crate::net::Job::Library);
    assert!(until(&mut app, &mut t, 10.0, |a| !a.library.loading));
    step(&mut app, &mut t);
    assert!(!app.st.online.open, "once a spell");
    // The server answers: a quiet word, and the next loss shows it again.
    crate::online::heard(&mut app, true);
    step(&mut app, &mut t);
    assert_eq!(app.toast_words(), Some("Back online"));
    assert!(!app.st.online.open);
    crate::online::heard(&mut app, false);
    step(&mut app, &mut t);
    assert!(app.st.online.open, "lost again: shown again");
    // Open downloads goes to My space › Downloads.
    let (y, _, right) = prompt_keys(&app);
    tap(&mut app, &mut t, right, y);
    assert!(!app.st.online.open);
    assert_eq!((app.route(), app.nav.len(), app.st.space.tab), (&Route::Space, 1, crate::space::DOWNLOADS));
    // A third spell: Not now only closes it.
    crate::online::heard(&mut app, true);
    crate::online::heard(&mut app, false);
    step(&mut app, &mut t);
    assert!(app.st.online.open);
    let (y, left, _) = prompt_keys(&app);
    tap(&mut app, &mut t, left, y);
    assert!(!app.st.online.open);
    assert_eq!(app.route(), &Route::Space);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_offline_prompt_leaves_the_reader_alone_and_says_ok_with_nothing_downloaded() {
    let (mut app, mut t, dir) = app_on("offline-reader", |d| {
        saved_on_disk(d, 5, 5001, 1.0, 2, false);
    });
    assert!(until(&mut app, &mut t, 10.0, |a| a.st.online.offline()), "the server is out of reach");
    // Reading a saved chapter when it is noticed: nothing over the strip.
    crate::saved::open(&mut app, 5, 5001);
    assert_eq!(app.route(), &Route::Reader);
    app.st.online.off = false;
    for _ in 0..5 {
        step(&mut app, &mut t);
    }
    assert!(!app.st.online.open && !app.st.online.prompted, "the reader is left alone");
    while app.route() == &Route::Reader {
        app.back();
        step(&mut app, &mut t);
    }
    step(&mut app, &mut t);
    assert!(app.st.online.open, "out of the reader, it shows");
    let _ = std::fs::remove_dir_all(dir);
    // Nothing downloaded: the line says so, and its one key is OK.
    let (mut app, mut t, dir) = app_on("offline-none", |_| {});
    app.st.online.off = false;
    assert!(until(&mut app, &mut t, 10.0, |a| a.st.online.open));
    assert_eq!(crate::online::line(0, false), "Nothing is downloaded on this phone yet.");
    let (y, _, _) = prompt_keys(&app);
    tap(&mut app, &mut t, WIDTH / 2.0, y);
    assert!(!app.st.online.open);
    assert_eq!(app.route(), &Route::Home);
    let _ = std::fs::remove_dir_all(dir);
}

// Drag to select in bulk

/// A long press at (x, y), then, without lifting, the finger through each of `path` (layout units),
/// a frame every 10 units.
fn press_and_drag(app: &mut App, t: &mut u64, x: f32, y: f32, path: &[f32]) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    app.touch(Touch { id: 1, phase: Phase::Down, x: x0 + x * u, y: y * u, t_ns: *t });
    step(app, t);
    *t += crate::series::LONG_PRESS_NS + 80_000_000;
    step(app, t);
    step(app, t);
    let mut at = y;
    for &to in path {
        let n = ((to - at).abs() / 10.0).ceil().max(1.0) as usize;
        for i in 1..=n {
            let yy = at + (to - at) * i as f32 / n as f32;
            app.touch(Touch { id: 1, phase: Phase::Move, x: x0 + x * u, y: yy * u, t_ns: *t });
            step(app, t);
        }
        at = to;
    }
}

/// The finger lifts at (x, y).
fn lift(app: &mut App, t: &mut u64, x: f32, y: f32) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    app.touch(Touch { id: 1, phase: Phase::Up, x: x0 + x * u, y: y * u, t_ns: *t + 200_000_000 });
    step(app, t);
}

fn selected(app: &App) -> Vec<i64> {
    app.st.series.select.clone().unwrap_or_default().into_iter().collect()
}

/// How far [`forty`] scrolls its page, so its first rows are on screen under the fixed top.
const FORTY_OFF: f32 = 350.0;

/// A series page of 40 chapters (newest first: row `k` is chapter 40 − k, id 1040 − k), scrolled
/// [`FORTY_OFF`]: where its first row is on screen.
fn forty(name: &str) -> (App, u64, PathBuf, f32) {
    let (mut app, mut t, dir) = app(name);
    app.series.insert(1, loaded(detail(1, 40)));
    app.push(Route::Series(1));
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    app.scroll().off = FORTY_OFF;
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    let y0 = first_row(&app) - FORTY_OFF;
    (app, t, dir, y0)
}

#[test]
fn a_long_press_and_drag_selects_the_chapters_it_passes_and_dragging_back_lets_go() {
    let (mut app, mut t, dir, y0) = forty("sweep");
    let row = crate::series::ROW_H;
    let mid = |k: f32| y0 + row * k + row / 2.0;
    // From chapter 38 down to 35.
    press_and_drag(&mut app, &mut t, 200.0, mid(2.0), &[mid(5.0)]);
    assert_eq!(selected(&app), vec![1035, 1036, 1037, 1038]);
    // Back up to 37: 36 and 35 let go.
    press_and_drag_on(&mut app, &mut t, 200.0, &[mid(3.0)]);
    assert_eq!(selected(&app), vec![1037, 1038]);
    lift(&mut app, &mut t, 200.0, mid(3.0));
    assert_eq!(selected(&app), vec![1037, 1038], "lifting is not a tap");
    let off = app.scroll().off;
    assert_eq!((app.route(), off), (&Route::Series(1), FORTY_OFF), "nothing opened, nothing scrolled");
    // Taps add 34 and 33; a sweep starting on 33 (selected) lets go of 33 and 34.
    tap(&mut app, &mut t, 200.0, mid(6.0));
    tap(&mut app, &mut t, 200.0, mid(7.0));
    assert_eq!(selected(&app), vec![1033, 1034, 1037, 1038]);
    press_and_drag(&mut app, &mut t, 200.0, mid(7.0), &[mid(6.0)]);
    lift(&mut app, &mut t, 200.0, mid(6.0));
    assert_eq!(selected(&app), vec![1037, 1038], "a sweep from a selected row de-selects");
    // The bar still takes its own taps: Mark read, its third key.
    let h = app.canvas.h;
    let gw = (WIDTH - 2.0 * crate::theme::PAD - 16.0) / 3.0;
    tap(&mut app, &mut t, crate::theme::PAD + 2.0 * (gw + 8.0) + gw / 2.0, h - (50.0 + 24.0) + 8.0 + 25.0);
    let d = app.series[&1].data.as_ref().unwrap();
    assert!(d.chapters.iter().filter(|c| c.id == 1037 || c.id == 1038).all(|c| c.read));
    assert!(!d.chapters.iter().find(|c| c.id == 1036).unwrap().read);
    assert_eq!(app.st.series.select, None);
    let _ = std::fs::remove_dir_all(dir);
}

/// The finger already down moves on through `path`.
fn press_and_drag_on(app: &mut App, t: &mut u64, x: f32, path: &[f32]) {
    let u = app.canvas.u;
    let x0 = ((app.canvas.px_w - WIDTH * u) * 0.5).max(0.0);
    let mut at = app.ui.finger().expect("a finger down").1;
    for &to in path {
        let n = ((to - at).abs() / 10.0).ceil().max(1.0) as usize;
        for i in 1..=n {
            let yy = at + (to - at) * i as f32 / n as f32;
            app.touch(Touch { id: 1, phase: Phase::Move, x: x0 + x * u, y: yy * u, t_ns: *t });
            step(app, t);
        }
        at = to;
    }
}

#[test]
fn a_sweep_held_at_the_bottom_edge_scrolls_the_list_and_takes_what_comes() {
    let (mut app, mut t, dir, y0) = forty("sweep-edge");
    let row = crate::series::ROW_H;
    let edge = app.canvas.h - (50.0 + 24.0) - 10.0;
    press_and_drag(&mut app, &mut t, 200.0, y0 + row * 2.0 + row / 2.0, &[edge]);
    let fit = selected(&app).len();
    for _ in 0..40 {
        step(&mut app, &mut t);
    }
    let off = app.scroll().off;
    assert!(off > FORTY_OFF + 200.0, "the list scrolled by itself: {off}");
    let sel = selected(&app);
    assert!(sel.len() > fit + 3, "and the rows it brought were taken: {} after {fit}", sel.len());
    assert_eq!(sel.last(), Some(&1038), "from where it began");
    assert!(sel.windows(2).all(|w| w[1] == w[0] + 1), "one run: {sel:?}");
    lift(&mut app, &mut t, 200.0, edge);
    step(&mut app, &mut t);
    let after = app.scroll().off;
    step(&mut app, &mut t);
    assert_eq!(app.scroll().off, after, "it stops with the finger, and doesn't coast");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_plain_drag_on_the_chapters_scrolls_and_selects_nothing() {
    let (mut app, mut t, dir, y0) = forty("sweep-plain");
    pull(&mut app, &mut t, 200.0, y0 + crate::series::ROW_H * 4.0, -300.0, 10, 60);
    assert!(app.scroll().off > FORTY_OFF + 250.0, "scrolled: {}", app.scroll().off);
    assert_eq!(app.st.series.select, None);
    assert_eq!(app.route(), &Route::Series(1));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn downloads_and_a_series_downloads_select_by_sweeping_too() {
    let (mut app, mut t, dir) = app_on("sweep-downloads", |d| {
        for s in 1..=6 {
            saved_on_disk(d, s, s * 1000 + 1, 1.0, 1, false);
        }
        for n in 2..=5 {
            saved_on_disk(d, 6, 6000 + n, n as f64, 1, false);
        }
    });
    app.st.space.tab = crate::space::DOWNLOADS;
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    // Rows: 6, 5, 4, 3, 2, 1 (saved last first). From the first to the third.
    let row = crate::space::ROW_H;
    press_and_drag(&mut app, &mut t, 200.0, DOWNLOADS_ROW + row / 2.0, &[DOWNLOADS_ROW + 2.0 * row + row / 2.0]);
    lift(&mut app, &mut t, 200.0, DOWNLOADS_ROW + 2.0 * row + row / 2.0);
    assert_eq!(app.st.space.select.clone().unwrap().into_iter().collect::<Vec<_>>(), vec![4, 5, 6]);
    assert_eq!(app.route(), &Route::Space, "nothing opened");
    // The bar's Delete takes its tap: their chapters wait for the confirm.
    let key_y = app.canvas.h - crate::saved::BAR_H + 12.0 + 25.0;
    tap(&mut app, &mut t, crate::theme::PAD + 40.0, key_y);
    let mut asked = app.st.space.confirm.clone().expect("the confirm");
    asked.sort();
    assert_eq!(asked, vec![4001, 5001, 6001, 6002, 6003, 6004, 6005]);
    app.st.space.confirm = None;
    app.st.space.select = None;
    // Series 6's own downloads: newest first, 5 down to 1.
    app.push(Route::Saved(6));
    step(&mut app, &mut t);
    let line = crate::draw::Text::new(crate::theme::ty::UI, crate::theme::INK).lh();
    let top = 60.0 + 8.0 + line + 14.0;
    let r = 70.0;
    press_and_drag(&mut app, &mut t, 150.0, top + r / 2.0, &[top + 3.0 * r + r / 2.0]);
    lift(&mut app, &mut t, 150.0, top + 3.0 * r + r / 2.0);
    assert_eq!(app.st.saved.select.clone().unwrap().into_iter().collect::<Vec<_>>(), vec![6002, 6003, 6004, 6005]);
    assert_eq!(app.route(), &Route::Saved(6));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_newer_app_is_asked_for_on_coming_back_at_most_every_quarter_hour() {
    let (mut app, mut t, dir) = app("update-check");
    nowhere(&mut app);
    app.shot = false;
    app.update_checked_ms = 0;
    app.resumed();
    let first = app.update_checked_ms;
    assert!(first > 0, "back to the front: asked");
    step(&mut app, &mut t);
    app.update_checked_ms = first - 14 * 60_000;
    app.resumed();
    assert_eq!(app.update_checked_ms, first - 14 * 60_000, "14 minutes after: not again");
    app.update_checked_ms = first - 15 * 60_000;
    app.resumed();
    assert!(app.update_checked_ms >= first, "a quarter of an hour after: asked again");
    // Opening the app (Home's first frame) asks too.
    app.update_checked_ms = 0;
    app.reset_to(Route::Home);
    step(&mut app, &mut t);
    assert!(app.update_checked_ms > 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cancelling_downloads_empties_the_line_and_leaves_nothing_half_saved() {
    let dir = std::env::temp_dir().join(format!("leaf-flows-cancel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let chapters = comic(1, &[3, 2, 2]).chapters;
    // No server: everything waits in line.
    let saver = crate::offline::Saver::with_timing(dir.clone(), crate::offline::quick());
    assert!(eventually(5.0, || saver.scanned()));
    assert_eq!(saver.add(1, "Series 1", &chapters), 3);
    // A half-saved chapter has a page on disk.
    let part = crate::offline::chapter_dir(&dir, 1002);
    std::fs::create_dir_all(&part).unwrap();
    std::fs::write(part.join("0"), png()).unwrap();
    // One stopped from its row.
    saver.delete(&[1001]);
    assert_eq!(saver.state(1001), crate::offline::Kept::None);
    assert_eq!(saver.state(1002), crate::offline::Kept::Waiting);
    // The rest from Downloads' Cancel all.
    assert_eq!(saver.cancel_all(), 2);
    assert_eq!((saver.state(1002), saver.state(1003)), (crate::offline::Kept::None, crate::offline::Kept::None));
    assert_eq!(saver.running().waiting, 0);
    assert!(eventually(5.0, || !part.exists()), "the half-saved folder is gone");
    // Given a server now, nothing is fetched.
    let fake = fake::Fake::start();
    saver.set_server(Some(fake.server()));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(fake.state.lock().unwrap().hits.iter().all(|h| !h.contains("/pages/")), "nothing fetched after cancelling");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_chapter_opened_by_mistake_does_not_move_continue() {
    let dir = std::env::temp_dir().join(format!("leaf-flows-slip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut places = crate::places::Places::load(&dir);
    let mut d = comic(1, &[30, 30, 30, 30, 30]);
    d.chapters[0].read = true;
    d.chapters[1].read = true;
    let place = |chapter: i64, number: f64, page: u32| crate::places::Place { series: 1, chapter, number, page, head: false, off: 40.0, at_ms: i64::MAX / 2 };
    // Chapter 5 tapped by mistake and left on its first page: Continue stays on chapter 3.
    places.put(place(1005, 5.0, 0));
    assert_eq!(crate::places::resume(&places, &d).unwrap().index, 2);
    // Read into chapter 5 (its fourth page): that is where you are.
    places.put(place(1005, 5.0, 3));
    assert_eq!(crate::places::resume(&places, &d).unwrap().index, 4);
    // On chapter 3 itself, even its first page keeps the exact spot.
    places.put(place(1003, 3.0, 0));
    let c = crate::places::resume(&places, &d).unwrap();
    assert_eq!((c.index, c.page), (2, 0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_server_out_of_reach_is_asked_at_its_second_address() {
    let fake = fake::Fake::start();
    // The public address blocked (nothing listens there); the Tailscale one answers.
    let s = crate::net::Server { base: "http://127.0.0.1:9".into(), token: None, alt: Some(fake.server().base) };
    let client = crate::net::Client::new();
    assert!(client.get(&s, "/v1/library").is_ok(), "answered at the second address");
    assert!(fake.hits("/v1/library") >= 1);
    // With no second address, out of reach stays out of reach.
    let lone = crate::net::Server { alt: None, ..s.clone() };
    assert_eq!(client.get(&lone, "/v1/library").err(), Some(crate::net::Fail::Unreachable));
}
