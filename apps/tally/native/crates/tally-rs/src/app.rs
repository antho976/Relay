//! The whole app: the ledger, the pages open, input, and one frame at a time.
//!
//! Screens are functions of the app's state that draw themselves and act on taps as they draw:
//! each module has `pub fn draw(app: &mut App, dt: f32) -> bool` (true while it is still moving)
//! and its own `State` in [`States`]. Nothing animates at rest: after an input the app draws until
//! scrolling settles, then sleeps until the next one.
//!
//! A screen's frame, in order:
//! 1. `let pg = app.page_begin(dt);` scrolls its page and gives the content's top.
//! 2. Draw with `let mut cx = cx!(app);` (and read `app.store`, `app.st.<module>` beside it).
//! 3. `app.page_end(&pg, bottom)` with the content's bottom, then act on what was tapped
//!    (navigate with `app.push`, `app.pop`, `app.replace`; write to `app.store`, then
//!    `app.invalidate()`).

use std::path::PathBuf;
use std::sync::Arc;

use crate::cx;
use crate::draw::{Canvas, Rgba, Text, WIDTH};
use crate::kit::{self, ty};
use crate::model::{self, Home, Store};
use crate::nav::{Route, Tab};
use crate::theme::{self, Palette};
use crate::ui::{Scroll, Touch, Ui};
use crate::widgets::Focus;

/// What the platform does for the app.
pub trait Host: Send + Sync {
    /// Where the ledger lives.
    fn data_dir(&self) -> PathBuf;
    fn log(&self, text: &str);
    /// Show the soft keyboard for a field holding `text` (digits only when `numeric`), or hide it.
    fn keyboard(&self, _show: bool, _text: &str, _numeric: bool) {}
    /// Open a web page in the browser (Plaid's sign-in, the server's pages).
    fn open_url(&self, _url: &str) {}
    /// Ask the person for a file to read (a CSV or a backup). The bytes arrive through
    /// [`App::file_picked`].
    fn pick_file(&self, _mime: &str) {}
    /// Hand a file to the share sheet (an export or a backup).
    fn share_file(&self, _name: &str, _mime: &str, _bytes: &[u8]) {}
    /// The phone is set to dark.
    fn system_dark(&self) -> bool {
        false
    }
    /// The app went light or dark: the system bars' icons and the launcher icon follow.
    fn theme_changed(&self, _dark: bool) {}
}

/// When the next frame is wanted.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Next {
    Now,
    OnInput,
    /// Sleep until input or this time on [`crate::clock::now_ns`], whichever comes first: a sync
    /// running in the background, or the next one due.
    At(u64),
}

pub struct Toast {
    pub words: String,
    /// The entry an Undo deletes.
    pub undo: Option<i64>,
    /// Undo puts a deleted entry back instead of deleting a new one.
    pub restore: bool,
    pub until_ns: u64,
}

/// Each screen module's own state, kept while the app runs.
#[derive(Default)]
pub struct States {
    pub overview: crate::overview::State,
    pub activity: crate::activity::State,
    pub insights: crate::insights::State,
    pub plan: crate::plan::State,
    pub accounts: crate::accounts::State,
    pub invest: crate::invest::State,
    pub categories: crate::categories::State,
    pub settings: crate::settings::State,
    pub import: crate::import::State,
    pub banks: crate::banks::State,
    pub onboarding: crate::onboarding::State,
    pub sync: crate::sync::State,
}

pub const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/geist_400.ttf"),
    include_bytes!("../assets/fonts/geist_500.ttf"),
    include_bytes!("../assets/fonts/geist_600.ttf"),
    include_bytes!("../assets/fonts/sora_600.ttf"),
];

/// Height of the top bar over a tab's page (menu, title, bills, settings).
pub const TOP_ROW: f32 = 56.0;
/// The coin's face in its picture, for a small round close-up.
pub const COIN_PART: [f32; 4] = [0.315, 0.14, 0.695, 0.52];
/// The drawer's width.
pub const DRAWER_W: f32 = 318.0;

pub struct App {
    pub canvas: Canvas,
    pub ui: Ui,
    pub host: Arc<dyn Host>,
    pub store: Store,
    /// Overview's reading, refreshed whenever the ledger changes.
    pub home: Option<Home>,
    pub palette: Palette,
    /// The pages open, the current one last, each with its scroll.
    pub nav: Vec<(Route, Scroll)>,
    /// True on the first frame a page is shown (pushed, or uncovered): reload what it reads.
    pub fresh: bool,
    /// Bumps on every change to the ledger, so a screen can cache what it read.
    pub ledger_gen: u64,
    pub entry: crate::entry::Entry,
    pub st: States,
    pub focus: Focus,
    pub toast: Option<Toast>,
    /// A problem the ledger reported, shown until the next frame that works.
    pub problem: Option<String>,
    /// The soft keyboard covers the bottom of the window (its height is in the bottom inset).
    pub keyboard_up: bool,
    /// How far the drawer is out, 0 to 1, and where it is going.
    pub drawer: f32,
    pub drawer_open: bool,
    /// The stack depth of the page the drawer opened, while it is still there.
    pub drawer_depth: Option<usize>,
    drawer_drag: bool,
    drawer_scroll: Scroll,
    /// The phone is on dark (for Appearance "System").
    pub system_dark: bool,
    /// A file the person picked, waiting for the screen that asked (`Host::pick_file`).
    pub picked: Option<(String, Vec<u8>)>,
    size: (f32, f32),
    insets: (f32, f32),
    last_ns: u64,
    pub now_ns: u64,
    stale: bool,
    posted_on: Option<relay_money::Date>,
}

impl App {
    pub fn new(host: Arc<dyn Host>, now_ns: u64) -> App {
        let path = host.data_dir().join("tally.db");
        let store = match Store::open(&path) {
            Ok(s) => s,
            Err(e) => {
                host.log(&format!("ledger at {}: {e}; using one in memory", path.display()));
                Store::in_memory().expect("an in-memory ledger opens")
            }
        };
        let canvas = Canvas::new(FONTS);
        let system_dark = host.system_dark();
        let palette = theme::for_mode(&appearance(&store), system_dark);
        theme::set_dark(!palette.light);
        let start = if store.pref("onboarded").is_some() || !store.is_empty() { Route::Overview } else { Route::Onboarding };
        App {
            canvas,
            ui: Ui::default(),
            host,
            store,
            home: None,
            palette,
            nav: vec![(start, Scroll::default())],
            fresh: true,
            ledger_gen: 0,
            entry: crate::entry::Entry::default(),
            st: States::default(),
            focus: Focus::default(),
            toast: None,
            problem: None,
            keyboard_up: false,
            drawer: 0.0,
            drawer_open: false,
            drawer_depth: None,
            drawer_drag: false,
            drawer_scroll: Scroll::default(),
            system_dark,
            picked: None,
            size: (1080.0, 2340.0),
            insets: (0.0, 0.0),
            last_ns: now_ns,
            now_ns,
            stale: true,
            posted_on: None,
        }
    }

    pub fn route(&self) -> &Route {
        &self.nav.last().expect("a page is always open").0
    }

    pub fn scroll(&mut self) -> &mut Scroll {
        &mut self.nav.last_mut().expect("a page is always open").1
    }

    /// The window changed size. Insets are the system bars (and the keyboard), in pixels.
    pub fn resize(&mut self, w: f32, h: f32, inset_top: f32, inset_bottom: f32) {
        self.size = (w, h);
        self.insets = (inset_top, inset_bottom);
    }

    pub fn touch(&mut self, t: Touch) {
        let u = self.canvas.u.max(0.01);
        let x0 = ((self.size.0 - WIDTH * u) * 0.5).max(0.0);
        self.ui.touch(t, (t.x - x0) / u, t.y / u);
    }

    /// The ledger changed: read it again before the next frame.
    pub fn invalidate(&mut self) {
        self.stale = true;
        self.ledger_gen += 1;
        crate::sync::changed(self);
    }

    // Navigation

    fn shown(&mut self) {
        if self.drawer_depth.is_some_and(|d| d >= self.nav.len()) {
            self.drawer_depth = None;
        }
        self.fresh = true;
        self.drawer_open = false;
        self.ui.reset();
        if self.focus.id.take().is_some() {
            self.host.keyboard(false, "", false);
        }
        if let Route::Entry { id, kind, account, category } = self.route().clone() {
            self.entry = crate::entry::Entry::open(&self.store, id, kind, account, category);
        }
    }

    /// Open a page. A space page replaces everything open (spaces are siblings); any other page
    /// goes on top.
    pub fn push(&mut self, route: Route) {
        if self.route() == &route {
            return;
        }
        if route.is_space() {
            self.nav.clear();
        }
        self.nav.push((route, Scroll::default()));
        self.shown();
    }

    /// Open a tab's page from inside another page (Overview's "See all"): on top of it, with Back,
    /// rather than in its place.
    pub fn push_over(&mut self, route: Route) {
        if self.route() == &route {
            return;
        }
        self.nav.push((route, Scroll::default()));
        self.shown();
    }

    /// The page showing is one the drawer opened (it gets the drawer's key, not Back).
    pub fn drawer_page(&self) -> bool {
        self.drawer_depth == Some(self.nav.len().saturating_sub(1)) && self.route().is_top_level()
    }

    /// The page showing sits on another page and goes back to it.
    pub fn is_sub_page(&self) -> bool {
        self.nav.len() > 1 && !self.drawer_page()
    }

    /// Open a page from the drawer: a tab's page as ever; any other on top of Overview alone, so
    /// Back from it goes to Overview.
    pub fn open_from_drawer(&mut self, route: Route) {
        if route.is_space() {
            self.push(route);
            return;
        }
        self.nav.clear();
        self.nav.push((Route::Overview, Scroll::default()));
        self.nav.push((route, Scroll::default()));
        self.drawer_depth = Some(1);
        self.shown();
    }

    /// Replace the page showing (an editor that saved and opens what it made).
    pub fn replace(&mut self, route: Route) {
        if self.nav.len() > 1 {
            self.nav.pop();
        }
        self.nav.push((route, Scroll::default()));
        self.shown();
    }

    /// Close the page showing. False when it was the last one.
    pub fn pop(&mut self) -> bool {
        if self.nav.len() <= 1 {
            return false;
        }
        self.nav.pop();
        self.shown();
        true
    }

    /// Back: typing stops first, then the menu closes, then the page; a space page other than
    /// Overview goes to Overview. False when there was nowhere to go back to.
    pub fn back(&mut self) -> bool {
        if self.focus.id.is_some() {
            self.finish_typing();
            return true;
        }
        if self.drawer_open {
            self.drawer_open = false;
            return true;
        }
        if self.pop() {
            return true;
        }
        if self.route() != &Route::Overview && self.route() != &Route::Onboarding {
            self.push(Route::Overview);
            return true;
        }
        false
    }

    // Typing

    /// Text the keyboard sent: the field's whole text (Android's input state).
    pub fn set_text(&mut self, text: &str) {
        if self.focus.id.is_some() {
            self.focus.text = text.chars().take(200).collect();
        }
    }

    /// The keyboard's Done, or back while typing: the field lets go and the keyboard goes.
    pub fn finish_typing(&mut self) {
        if self.focus.id.take().is_some() {
            self.focus.at = None;
            self.host.keyboard(false, "", false);
        }
    }

    /// Characters typed on a hardware keyboard (the desktop).
    pub fn type_text(&mut self, s: &str) {
        if self.focus.id.is_some() {
            self.focus.text.push_str(s);
        } else if matches!(self.route(), Route::Entry { .. }) {
            for ch in s.chars() {
                if let Some(d) = ch.to_digit(10) {
                    self.entry.amount = self.entry.amount.digit(d);
                } else if ch == '.' || ch == ',' {
                    self.entry.amount = self.entry.amount.decimal();
                }
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.focus.id.is_some() {
            self.focus.text.pop();
        } else if matches!(self.route(), Route::Entry { .. }) {
            self.entry.amount = self.entry.amount.backspace();
        }
    }

    /// A file came back from `Host::pick_file`.
    pub fn file_picked(&mut self, name: String, bytes: Vec<u8>) {
        self.picked = Some((name, bytes));
    }

    pub fn say(&mut self, words: impl Into<String>, undo: Option<i64>) {
        self.toast = Some(Toast { words: words.into(), undo, restore: false, until_ns: self.now_ns + 4_000_000_000 });
    }

    /// A toast for an entry just deleted: its Undo restores it.
    pub fn say_restore(&mut self, words: impl Into<String>, id: i64) {
        self.toast = Some(Toast { words: words.into(), undo: Some(id), restore: true, until_ns: self.now_ns + 4_000_000_000 });
    }

    pub fn set_palette(&mut self, p: Palette) {
        self.palette = p;
        theme::set_dark(!p.light);
        let _ = self.store.set_pref("palette", p.name);
    }

    /// Appearance: "system", "light" or "dark".
    pub fn appearance(&self) -> String {
        appearance(&self.store)
    }

    pub fn set_appearance(&mut self, mode: &str) {
        let _ = self.store.set_pref("appearance", mode);
        self.apply_appearance();
    }

    /// The phone switched between light and dark.
    pub fn set_system_dark(&mut self, dark: bool) {
        if self.system_dark != dark {
            self.system_dark = dark;
            self.apply_appearance();
        }
    }

    fn apply_appearance(&mut self) {
        let p = theme::for_mode(&self.appearance(), self.system_dark);
        self.palette = p;
        theme::set_dark(!p.light);
        self.host.theme_changed(!p.light);
    }

    pub fn clear_colour(&self) -> [f32; 3] {
        self.palette.ground[1].rgb_f32()
    }

    // Pages

    /// The height of the bottom bar on space pages.
    pub fn bar_h(&self) -> f32 {
        62.0 + self.canvas.inset_bottom
    }

    /// Start drawing the page's scrolling content: moves it with the finger and returns where its
    /// content starts. Space pages start under the fixed top row.
    pub fn page_begin(&mut self, dt: f32) -> Page {
        let space = self.route().is_space();
        let bottom = if space && !self.keyboard_up { self.canvas.h - self.bar_h() } else { self.canvas.h - self.canvas.inset_bottom };
        let viewport = [0.0, 0.0, WIDTH, bottom];
        let ui = &self.ui;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        let moving = sc.update(ui, viewport, dt);
        self.canvas.dy = -sc.off;
        let top = self.canvas.inset_top + if space && self.route() != &Route::Search { TOP_ROW + 6.0 } else { 10.0 };
        Page { top, bottom, moving }
    }

    /// Finish the page: `content_bottom` is where its last thing ends (content units).
    pub fn page_end(&mut self, page: &Page, content_bottom: f32) {
        self.canvas.dy = 0.0;
        let space = self.route().is_space();
        let tail = if space { 28.0 } else { 24.0 };
        let keyboard = self.keyboard_up;
        let at = self.focus.at;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        sc.max = content_bottom + tail - page.bottom;
        // Typing: keep the field above the keyboard.
        if keyboard && let Some((y, h)) = at {
            let need = y + h + 16.0 - page.bottom;
            if need > sc.off {
                sc.off = need;
            }
        }
    }

    /// Draw one frame. Returns when the next is wanted.
    pub fn frame(&mut self, now_ns: u64) -> Next {
        let dt = ((now_ns.saturating_sub(self.last_ns)) as f32 / 1e9).clamp(0.0, 0.05);
        self.last_ns = now_ns;
        self.now_ns = now_ns;
        crate::sync::tick(self, now_ns);
        let today = model::today();
        if self.posted_on != Some(today) {
            // Bills that fell due while the app was closed are posted once a day, as the Kotlin
            // app's worker does.
            if let Ok(n) = self.store.ledger.post_due(today)
                && n > 0
            {
                self.stale = true;
                self.ledger_gen += 1;
            }
            self.posted_on = Some(today);
        }
        if self.stale {
            match self.store.home(today) {
                Ok(h) => {
                    self.home = Some(h);
                    self.problem = None;
                }
                Err(e) => {
                    self.host.log(&format!("reading the ledger: {e}"));
                    self.problem = Some(e);
                }
            }
            self.stale = false;
        }
        // A tap can change what was drawn above it in the same frame (a chip fills the amount
        // drawn over it): a frame that took one is followed by one more.
        let tapped = self.ui.tap.is_some();
        let (w, h) = self.size;
        self.canvas.begin(w, h, self.insets.0, self.insets.1);
        self.focus.at = None;
        let p = self.palette;
        kit::ground(&mut self.canvas, &p);

        let moving_pictures = crate::pictures::drain(&mut self.canvas);

        // The fixed bars take their taps first, so a tap on them never lands on what scrolls under.
        let space = self.route().is_space();
        let bar_top = self.canvas.h - self.bar_h();
        let row_bottom = self.canvas.inset_top + if self.route() == &Route::Search { 0.0 } else { TOP_ROW };
        // The swipe brings the drawer out where its key shows; a page opened from another has Back.
        let drawer_moving = self.drawer_gesture(dt, (space && self.nav.len() == 1) || self.drawer_page());
        let drawer_shown = self.drawer > 0.001;
        let chrome_tap = match self.ui.tap {
            Some((_, ty)) if space && !drawer_shown && ((ty >= bar_top && !self.keyboard_up) || ty < row_bottom) => self.ui.tap.take(),
            _ => None,
        };
        // While the drawer is out it takes every tap and every drag; the page under it stays still.
        let drawer_input = if drawer_shown {
            let taken = (self.ui.tap.take(), self.ui.drag_from.take(), std::mem::take(&mut self.ui.drag_dy), self.ui.fling.take());
            Some(taken)
        } else {
            None
        };

        let route = self.route().clone();
        let mut moving = match route {
            Route::Overview => crate::overview::draw(self, dt),
            Route::Entry { .. } => crate::entry::draw(self, dt),
            Route::Entries | Route::Transactions { .. } | Route::Search | Route::EntryView { .. } => crate::activity::draw(self, dt),
            Route::Insights => crate::insights::draw(self, dt),
            Route::Budgets | Route::BudgetEdit { .. } | Route::Bills | Route::BillEdit { .. } | Route::Goals | Route::Goal { .. } | Route::GoalEdit { .. } => {
                crate::plan::draw(self, dt)
            }
            Route::Accounts | Route::AccountEdit { .. } => crate::accounts::draw(self, dt),
            Route::Investments | Route::RoomEdit { .. } => crate::invest::draw(self, dt),
            Route::Categories | Route::CategoryEdit { .. } => crate::categories::draw(self, dt),
            Route::Settings | Route::Appearance | Route::Format | Route::Backup | Route::About => crate::settings::draw(self, dt),
            Route::Sync => crate::sync::draw(self, dt),
            Route::Import => crate::import::draw(self, dt),
            Route::Banks => crate::banks::draw(self, dt),
            Route::Onboarding => crate::onboarding::draw(self, dt),
        };
        // A page that navigated during its draw keeps `fresh` for the page it opened.
        if self.route() == &route {
            self.fresh = false;
        }

        // What scrolls goes under the status bar behind a band of the sky's top colour, fading out
        // below it (and below the top row on space pages), so the clock never sits on words.
        let space = self.route().is_space();
        // Pages whose picture runs to the top edge (first run, a goal) draw their own top.
        let full_bleed = matches!(self.route(), Route::Onboarding | Route::Goal { .. });
        let band = if full_bleed { 0.0 } else { self.canvas.inset_top + if space && self.route() != &Route::Search { TOP_ROW - 4.0 } else { 0.0 } };
        if band > 0.0 {
            let top = p.ground[0];
            self.canvas.rect(-1000.0, 0.0, WIDTH + 2000.0, band, 0.0, top);
            self.canvas.rect_grad(-1000.0, band, WIDTH + 2000.0, 14.0, 0.0, top, top.a(0.0));
        }

        if space && self.route() == &route {
            self.ui.tap = chrome_tap;
            if route != Route::Search {
                self.top_bar(&route);
            }
            if !self.keyboard_up {
                self.tab_bar(&route, bar_top);
            }
        }
        if let Some((tap, from, dy, fling)) = drawer_input {
            self.ui.tap = tap;
            self.ui.drag_from = from;
            self.ui.drag_dy = dy;
            self.ui.fling = fling;
            moving |= self.draw_drawer(dt);
        }
        moving |= drawer_moving;
        // A picture asked for is decoding: the decoder wakes the loop when it lands (Android), or,
        // with no waker (the desktop), frames keep coming until it has.
        moving |= moving_pictures && !crate::pictures::has_waker();
        moving |= crate::pictures::pending() && !crate::pictures::has_waker();
        let toast_bottom = if self.route().is_space() { bar_top } else { self.canvas.h - self.canvas.inset_bottom - 8.0 };
        moving |= self.draw_toast(toast_bottom);

        self.canvas.finish();
        self.ui.end_frame();
        if moving || tapped || self.ui.busy() || self.fresh {
            Next::Now
        } else if let Some(at) = self.st.sync.wake_at(now_ns) {
            Next::At(at)
        } else {
            Next::OnInput
        }
    }

    /// Over a tab's page: the drawer's key, the page's title, bills and settings.
    fn top_bar(&mut self, route: &Route) {
        let top = self.canvas.inset_top + 6.0;
        let soon = self.home.as_ref().is_some_and(|h| h.s.bills.iter().any(|b| (0..=3).contains(&b.days_until)));
        let over = self.nav.len() > 1;
        let mut back = false;
        let mut go = None;
        let mut cx = cx!(self);
        let p = cx.p;
        // A tab's page opened from another page (Overview's "See all") goes back to it.
        if over {
            back = kit::round_key(&mut cx, 10.0, top, "arrow-left");
        } else if kit::round_key(&mut cx, 10.0, top, "menu") {
            go = Some(None);
        }
        let t = Text::new(17.0, p.ink).semi().center();
        cx.c.text(route.title(), WIDTH / 2.0, top + (44.0 - t.lh()) / 2.0, t);
        if kit::round_key(&mut cx, WIDTH - 10.0 - 44.0, top, "user") {
            go = Some(Some(Route::Settings));
        }
        let bx = WIDTH - 10.0 - 44.0 - 46.0;
        if kit::round_key(&mut cx, bx, top, "bell") {
            go = Some(Some(Route::Bills));
        }
        if soon {
            let (dx, dy) = (bx + 30.0, top + 12.0);
            cx.c.circle(dx, dy, 5.0, p.ground[0]);
            cx.c.circle(dx, dy, 3.5, p.held);
        }
        if back {
            self.pop();
        }
        match go {
            Some(None) => {
                self.drawer_open = true;
                self.drawer_scroll = Scroll::default();
            }
            Some(Some(r)) => self.push(r),
            None => {}
        }
    }

    /// Overview, Entries, + (a new entry), Insights, Search: icons only, on a frosted bar.
    fn tab_bar(&mut self, route: &Route, top: f32) {
        let h = self.bar_h();
        let current = route.tab();
        let mut cx = cx!(self);
        let p = cx.p;
        cx.c.rect(-1000.0, top, WIDTH + 2000.0, h + 40.0, 0.0, p.console);
        cx.c.rect(-1000.0, top, WIDTH + 2000.0, 1.0, 0.0, p.line_subtle);
        let items: [(&'static str, Option<Tab>); 5] =
            [("home", Some(Tab::Home)), ("history", Some(Tab::Entries)), ("plus", None), ("pie", Some(Tab::Insights)), ("search", Some(Tab::Search))];
        let slot = (WIDTH - 24.0) / 5.0;
        let mut go = None;
        for (i, (glyph, tab)) in items.iter().enumerate() {
            let x = 12.0 + slot * i as f32;
            let mid = top + 30.0;
            let Some(tab) = tab else {
                let (kw, kh) = (56.0, 42.0);
                let (kx, ky) = (x + (slot - kw) / 2.0, mid - kh / 2.0);
                cx.c.rect(kx, ky, kw, kh, 15.0, p.prim);
                if cx.pressed(x, top, slot, h) {
                    cx.c.rect(kx, ky, kw, kh, 15.0, Rgba::hex(0x808080).a(0.25));
                }
                cx.c.icon("plus", kx + kw / 2.0, ky + kh / 2.0, 22.0, p.on_ink);
                if cx.tap(x, top, slot, h) {
                    go = Some(Route::Entry { id: None, kind: None, account: None, category: None });
                }
                continue;
            };
            let selected = *tab == current;
            cx.c.icon(glyph, x + slot / 2.0, mid, 24.0, if selected { p.ink } else { p.ink3 });
            if selected {
                cx.c.rect(x + slot / 2.0 - 2.0, mid + 18.0, 4.0, 4.0, 2.0, p.ink);
            }
            if cx.tap(x, top, slot, h) {
                go = Some(tab.home());
            }
        }
        if let Some(r) = go {
            self.push(r);
        }
    }

    /// Follow a sideways swipe on a tab's page: right brings the drawer out, left puts it away.
    /// A swipe that began on a row that scrolls sideways (a carousel) is that row's.
    fn drawer_gesture(&mut self, dt: f32, space: bool) -> bool {
        let ui = &mut self.ui;
        let from_carousel = ui.drag_from.is_some_and(|(x, y)| ui.in_hzone(x, y));
        if ui.horizontal && (space || self.drawer > 0.0) && !from_carousel && !self.keyboard_up {
            if ui.drag_dx != 0.0 || ui.busy() {
                self.drawer_drag = true;
            }
            if self.drawer_drag {
                self.drawer = (self.drawer + ui.drag_dx / DRAWER_W).clamp(0.0, 1.0);
                ui.drag_dx = 0.0;
                if let Some(v) = ui.fling_x.take() {
                    self.drawer_open = if v > 350.0 { true } else if v < -350.0 { false } else { self.drawer > 0.4 };
                    self.drawer_drag = false;
                }
                if ui.busy() {
                    return true;
                }
            }
        } else {
            self.drawer_drag = false;
        }
        let target = if self.drawer_open { 1.0 } else { 0.0 };
        if (self.drawer - target).abs() < 0.002 {
            self.drawer = target;
            return false;
        }
        self.drawer += (target - self.drawer) * (1.0 - (-dt * 16.0).exp());
        true
    }

    /// The drawer: Tally and how sync stands, every place by section with a reading beside it,
    /// and this month's categories.
    fn draw_drawer(&mut self, dt: f32) -> bool {
        let d = self.drawer;
        let sync = self.st.sync.line(&self.store, crate::sync::now_ms());
        let (sync_line, sync_dot) = (sync.text, sync.tone.colour(&self.palette));
        let home = self.home.as_ref();
        let fmt = home.map(|h| h.fmt.clone());
        let money = |v: i64| fmt.as_ref().map(|f| f.format_whole(v)).unwrap_or_default();
        let over = home.map(|h| h.s.budgets.iter().filter(|b| b.spent > b.budget && b.budget > 0).count()).unwrap_or(0);
        let week_bills = home.map(|h| h.s.bills.iter().filter(|b| (0..=7).contains(&b.days_until)).count()).unwrap_or(0);
        let goals = home.map(|h| h.s.goals.len()).unwrap_or(0);
        // The count, not a figure: Accounts' own net worth also values investments, and two
        // different totals for one thing would read as a mistake.
        let net = home.map(|h| h.s.accounts.iter().filter(|a| !a.archived).count()).filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or_default();
        let invested_total: i64 = home.map(|h| h.s.accounts.iter().filter(|a| !a.archived && a.registration.is_some()).map(|a| a.balance).sum()).unwrap_or(0);
        let invested = if invested_total != 0 { money(invested_total) } else { String::new() };
        let current = self.route().clone();
        let h = self.canvas.h;
        let top_inset = self.canvas.inset_top;
        let mut go: Option<Route> = None;
        let mut close = false;
        let moving;
        {
            let mut cx = cx!(self);
            let p = cx.p;
            let x0 = -DRAWER_W * (1.0 - d);
            let dy = cx.c.dy;
            cx.c.dy = 0.0;
            cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, h, 0.0, Rgba::hex(0x000000).a(if p.light { 0.4 } else { 0.72 } * d));
            if cx.tap(x0 + DRAWER_W, 0.0, WIDTH + 1000.0, h) {
                close = true;
            }
            let panel = if p.light { p.ground[0] } else { Rgba::hex(0x0D0D0D) };
            cx.c.rect(x0 - 1000.0, 0.0, DRAWER_W + 1000.0, h, 28.0, panel);
            cx.c.rect(x0 + DRAWER_W - 1.0, 28.0, 1.0, h - 56.0, 0.0, p.line_subtle);
            // The content scrolls if it is taller than the screen.
            let viewport = [x0, 0.0, DRAWER_W, h];
            moving = self.drawer_scroll.update(cx.ui, viewport, dt);
            cx.c.push_clip(x0, 0.0, DRAWER_W, h);
            cx.c.dy = -self.drawer_scroll.off;
            let (lx, rw) = (x0 + 14.0, DRAWER_W - 28.0);
            let mut y = top_inset + 14.0;
            // Header: the coin, Tally, sync.
            kit::app_icon(&mut cx, lx + 6.0, y, 46.0);
            cx.c.text("Tally", lx + 64.0, y + 1.0, Text::new(21.0, p.ink).brand());
            let st = Text::new(13.0, p.ink2);
            cx.c.circle(lx + 67.0, y + 35.0, 3.5, sync_dot);
            cx.c.text_fit(&sync_line, lx + 76.0, y + 26.0, rw - 76.0 - 44.0, st);
            if kit::round_key(&mut cx, lx + rw - 44.0, y + 1.0, "gear") {
                go = Some(Route::Settings);
            }
            y += 46.0 + 22.0;
            let groups: [(&str, Vec<(&'static str, &str, Route, String, Option<Rgba>)>); 4] = [
                (
                    "Spend",
                    vec![
                        ("home", "Overview", Route::Overview, String::new(), None),
                        ("history", "Entries", Route::Entries, String::new(), None),
                        ("pie", "Insights", Route::Insights, String::new(), None),
                        ("search", "Search", Route::Search, String::new(), None),
                    ],
                ),
                (
                    "Plan",
                    vec![
                        ("wallet", "Budgets", Route::Budgets, if over > 0 { format!("{over} over") } else { String::new() }, Some(p.held_text)),
                        ("receipt", "Bills", Route::Bills, if week_bills > 0 { format!("{week_bills} this week") } else { String::new() }, Some(p.signal)),
                        ("target", "Goals", Route::Goals, if goals > 0 { goals.to_string() } else { String::new() }, None),
                    ],
                ),
                (
                    "Money",
                    vec![
                        ("bank", "Accounts", Route::Accounts, net.clone(), None),
                        ("trend", "Investments", Route::Investments, invested.clone(), None),
                        ("tag", "Categories", Route::Categories, String::new(), None),
                    ],
                ),
                (
                    "Setup",
                    vec![
                        ("sync", "Connected banks", Route::Banks, String::new(), None),
                        ("download", "Import a statement", Route::Import, String::new(), None),
                        ("gear", "Settings", Route::Settings, String::new(), None),
                    ],
                ),
            ];
            let eyebrow = Text::new(12.0, p.ink3).semi();
            for (title, rows) in groups.iter() {
                cx.c.text(&title.to_uppercase(), lx + 12.0, y, eyebrow);
                y += eyebrow.lh() + 8.0;
                for (glyph, label, route, value, tone) in rows {
                    let on = *route == current;
                    if on || cx.pressed(lx, y, rw, 44.0) {
                        cx.c.rect(lx, y, rw, 44.0, 12.0, if on { p.wash } else { p.wash });
                    }
                    cx.c.icon(glyph, lx + 12.0 + 10.5, y + 22.0, 21.0, if on { p.ink } else { p.ink2 });
                    let lt = if on { Text::new(15.0, p.ink).semi() } else { Text::new(15.0, p.ink).medium() };
                    cx.c.text(label, lx + 12.0 + 35.0, y + 12.0, lt);
                    if !value.is_empty() {
                        let vt = match tone {
                            Some(c) => Text::new(13.0, *c).semi().right(),
                            None => Text::new(13.0, p.ink3).medium().right(),
                        };
                        cx.c.text(value, lx + rw - 12.0, y + 13.0, vt);
                    }
                    if cx.tap(lx, y, rw, 44.0) {
                        go = Some(route.clone());
                    }
                    y += 44.0;
                }
                y += 14.0;
            }
            let bottom = y + 20.0;
            cx.c.pop_clip();
            cx.c.dy = dy;
            self.drawer_scroll.max = bottom - h;
        }
        if let Some(r) = go {
            self.drawer_open = false;
            self.drawer = 0.0;
            if &r != self.route() {
                self.open_from_drawer(r);
            }
        } else if close {
            self.drawer_open = false;
        }
        moving
    }

    /// The toast: an ink pill above `bottom`, with Undo when there is something to undo.
    fn draw_toast(&mut self, bottom: f32) -> bool {
        let Some(t) = &self.toast else { return false };
        if self.now_ns > t.until_ns {
            self.toast = None;
            return false;
        }
        let (words, undo, restore) = (t.words.clone(), t.undo, t.restore);
        let mut cx = cx!(self);
        let (x, w, h) = (16.0, WIDTH - 32.0, 48.0);
        let y = bottom - h - 12.0;
        cx.c.shadow(x, y + 3.0, w, h, 16.0, 14.0, Rgba::argb(0x331A1A2E));
        cx.c.rect(x, y, w, h, 16.0, Rgba::hex(0x1A1A2E));
        let tt = Text::new(ty::UI, Rgba::WHITE).medium();
        let room = if undo.is_some() { w - 32.0 - 60.0 } else { w - 32.0 };
        cx.c.text_fit(&words, x + 16.0, y + (h - tt.lh()) / 2.0, room, tt);
        let mut undone = None;
        if let Some(id) = undo {
            let ut = Text::new(ty::UI, Rgba::hex(0xC9B8FF)).semi().right();
            cx.c.text("Undo", x + w - 16.0, y + (h - ut.lh()) / 2.0, ut);
            if cx.tap(x + w - 80.0, y, 80.0, h) {
                undone = Some(id);
            }
        }
        if let Some(id) = undone {
            let done = if restore { self.store.restore(id) } else { self.store.delete(id) };
            match done {
                Ok(()) => {
                    self.toast = None;
                    self.invalidate();
                }
                Err(e) => self.say(format!("Could not undo: {e}"), None),
            }
        }
        // Wake again when it is due to go.
        true
    }
}

/// Where a page's content goes this frame (content units).
#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub top: f32,
    /// The bottom of what can be seen (above the bottom bar or the keyboard).
    pub bottom: f32,
    pub moving: bool,
}

/// The page title and its context line, at the top of a space page. Returns the height used.
pub fn page_title(cx: &mut kit::Ctx, top: f32, title: &str, context: &str) -> f32 {
    let t = Text::new(ty::HEADING, cx.p.ink).semi().line(1.2);
    cx.c.text(title, kit::PAD, top, t);
    if context.is_empty() {
        return t.lh();
    }
    let c = Text::new(ty::CAPTION, cx.p.ink2);
    cx.c.text(context, kit::PAD, top + t.lh() + 2.0, c);
    t.lh() + 2.0 + c.lh()
}

/// The stored appearance: "system", "light" or "dark" (a palette from the sky look counts).
fn appearance(store: &Store) -> String {
    store.pref("appearance").or_else(|| store.pref("palette")).unwrap_or_else(|| "system".into())
}
