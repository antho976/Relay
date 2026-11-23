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
    app.reset_to(Route::Space);
    step(&mut app, &mut t);
    step(&mut app, &mut t);
    // The + key, then the menu's second item, which lies over the first row's cover and title.
    tap(&mut app, &mut t, 390.0 - 6.0 - 88.0 + 22.0, 10.0 + 22.0);
    assert!(app.st.space.add_menu, "the + opened its menu");
    tap(&mut app, &mut t, 390.0 - 12.0 - 100.0, 56.0 + 4.0 + 48.0 + 24.0);
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
    app.st.space.tab = 0;
    tab_over_content(&mut app, &mut t, Route::Space, 0.0, 0, Route::Home);
    app.st.space.tab = 1;
    tab_over_content(&mut app, &mut t, Route::Space, 0.0, 1, Route::Search);
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
