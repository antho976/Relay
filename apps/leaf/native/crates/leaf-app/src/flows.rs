//! The app driven as a finger would, with no window and no GPU: taps and swipes in, routes and
//! state out.

use std::path::PathBuf;
use std::sync::Arc;

use leaf_proto::Series;

use crate::app::{App, Host, Route};
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
    let dir = std::env::temp_dir().join(format!("leaf-flows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = App::new(Arc::new(TestHost(dir.clone())), 0);
    app.shot = true;
    app.resize(1080.0, 2340.0, 0.0, 0.0);
    app.library.data = Some(vec![series(1, "One"), series(2, "Two"), series(3, "Three")]);
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

/// A tap at (x, y) in layout units.
fn tap(app: &mut App, t: &mut u64, x: f32, y: f32) {
    let u = app.canvas.u;
    app.touch(Touch { id: 1, phase: Phase::Down, x: x * u, y: y * u, t_ns: *t });
    app.touch(Touch { id: 1, phase: Phase::Up, x: x * u, y: y * u, t_ns: *t + 50_000_000 });
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

#[test]
fn a_swipe_turns_the_spotlight() {
    let (mut app, mut t, dir) = app("swipe");
    assert_eq!(app.route(), &Route::Home);
    let before = app.st.home.target;
    let u = app.canvas.u;
    let y = 200.0 * u;
    app.touch(Touch { id: 1, phase: Phase::Down, x: 300.0 * u, y, t_ns: t });
    for i in 1..=6 {
        app.touch(Touch { id: 1, phase: Phase::Move, x: (300.0 - i as f32 * 30.0) * u, y, t_ns: t + i * 10_000_000 });
        step(&mut app, &mut t);
    }
    app.touch(Touch { id: 1, phase: Phase::Up, x: 120.0 * u, y, t_ns: t + 70_000_000 });
    step(&mut app, &mut t);
    assert_eq!(app.st.home.target, before + 1.0, "a swipe to the left shows the next one");
    for _ in 0..120 {
        step(&mut app, &mut t);
    }
    assert!((app.st.home.pos - app.st.home.target).abs() < 0.01, "and settles on it");
    assert_eq!(app.route(), &Route::Home, "a swipe is not a tap");
    let _ = std::fs::remove_dir_all(dir);
}
