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
use crate::widgets::{Focus, Kb};

/// What the platform does for the app.
pub trait Host: Send + Sync {
    /// Where the ledger lives.
    fn data_dir(&self) -> PathBuf;
    fn log(&self, text: &str);
    /// Show the soft keyboard for a field holding `text` (digits only when `numeric`), or hide it.
    fn keyboard(&self, show: bool, text: &str, numeric: bool) {
        self.keyboard_kind(show, text, Kb::numeric(numeric));
    }
    /// [`Host::keyboard`] for a field that wants a particular keyboard (a web address gets no
    /// autocorrect). What a platform implements.
    fn keyboard_kind(&self, _show: bool, _text: &str, _kind: Kb) {}
    /// Open a web page in the browser (Plaid's sign-in, the server's pages).
    fn open_url(&self, _url: &str) {}
    /// Ask the person for a file to read (a CSV or a backup). The bytes arrive through
    /// [`crate::settings_files::deliver`]. A platform starts each pick with
    /// [`crate::settings_files::begin_pick`], so a file an earlier pick left is never taken for it.
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
    /// Any other Undo: what puts it back (a goal, a bill, a budget, a category).
    pub action: Option<UndoFn>,
    pub until_ns: u64,
}

/// What a toast's Undo does, for anything other than an entry.
pub type UndoFn = Box<dyn FnOnce(&mut App) -> Result<(), String> + Send>;

/// The most a field holds, in characters.
const MAX_TYPED: usize = 200;

/// How long a toast stays.
const TOAST_NS: u64 = 4_000_000_000;

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
    /// With `fresh`: the page was uncovered (Back from a page over it), not opened. What was typed
    /// on it stays.
    pub returned: bool,
    /// Bumps on every change to the ledger, so a screen can cache what it read.
    pub ledger_gen: u64,
    pub entry: crate::entry::Entry,
    pub st: States,
    pub focus: Focus,
    pub toast: Option<Toast>,
    /// A problem the ledger reported, shown until the next frame that works.
    pub problem: Option<String>,
    /// The ledger on disk could not be opened, and why. Until it opens ([`App::retry_ledger`]),
    /// the app shows only that (`store` is an empty stand-in in memory that nothing reads or
    /// writes), so nothing typed meanwhile can vanish at the next launch.
    pub ledger_error: Option<String>,
    /// Tries since it failed, to say a Try again did not work either.
    ledger_tries: u32,
    /// What the system kept of the pages ([`App::restore`]) while the ledger would not open:
    /// opened once it does, and kept again meanwhile in place of the stand-in's pages.
    kept: Option<Saved>,
    /// The keyboard sent text that no frame has drawn yet (no field has copied it).
    typed_unseen: bool,
    /// The soft keyboard covers the bottom of the window (its height is in the bottom inset).
    pub keyboard_up: bool,
    /// `keyboard_up` as the last frame saw it, to notice the system taking the keyboard away.
    keyboard_was_up: bool,
    /// The keyboard went away under this focused field at this time; it lets go if the keyboard
    /// stays away (switching between a digits and a words keyboard can dip it for a moment).
    keyboard_gone: Option<(u64, &'static str)>,
    /// The soonest a screen asked to be drawn again without input (a notice timing out).
    wake: Option<u64>,
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
    /// The pick `picked` answers ([`crate::settings_files::begin_pick`]'s number): a page takes it
    /// only while that pick is still the latest one.
    pub picked_for: u64,
    size: (f32, f32),
    insets: (f32, f32),
    last_ns: u64,
    pub now_ns: u64,
    stale: bool,
    posted_on: Option<relay_money::Date>,
    /// Editors' drafts [`App::restore`] put back, each waiting for its page to open
    /// ([`App::take_draft`]). One whose page is no longer where it was is let go.
    drafts: Vec<PageDraft>,
}

impl App {
    pub fn new(host: Arc<dyn Host>, now_ns: u64) -> App {
        let path = host.data_dir().join("tally.db");
        let (store, ledger_error) = match Store::open(&path) {
            Ok(s) => (s, None),
            Err(e) => {
                // Never a ledger in memory in its place: what was typed would vanish at the next
                // launch. The app says so and waits for the real one.
                host.log(&format!("ledger at {}: {e}", path.display()));
                (Store::in_memory().expect("an in-memory ledger opens"), Some(e))
            }
        };
        let canvas = Canvas::new(FONTS);
        let system_dark = host.system_dark();
        let palette = theme::for_mode(&appearance(&store), system_dark);
        theme::set_dark(!palette.light);
        let start = start_route(&store);
        App {
            canvas,
            ui: Ui::default(),
            host,
            store,
            home: None,
            palette,
            nav: vec![(start, Scroll::default())],
            fresh: true,
            returned: false,
            ledger_gen: 0,
            entry: crate::entry::Entry::default(),
            st: States::default(),
            focus: Focus::default(),
            toast: None,
            problem: None,
            ledger_error,
            ledger_tries: 0,
            kept: None,
            typed_unseen: false,
            keyboard_up: false,
            keyboard_was_up: false,
            keyboard_gone: None,
            wake: None,
            drawer: 0.0,
            drawer_open: false,
            drawer_depth: None,
            drawer_drag: false,
            drawer_scroll: Scroll::default(),
            system_dark,
            picked: None,
            picked_for: 0,
            size: (1080.0, 2340.0),
            insets: (0.0, 0.0),
            last_ns: now_ns,
            now_ns,
            stale: true,
            posted_on: None,
            drafts: Vec::new(),
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

    /// Read the ledger again before the next frame, though nothing here changed it (the app came
    /// back to the front, the shade was pulled: another day, a sync's news). Unlike
    /// [`App::invalidate`] it starts no sync: those follow their own schedule (every five
    /// minutes by the wall clock too, so a return after longer, a night asleep included, meets it
    /// on its first frame).
    pub fn refresh(&mut self) {
        self.stale = true;
        self.ledger_gen += 1;
    }

    // Navigation

    /// A page is showing: opened (`returned` false), or uncovered by Back (`returned` true).
    fn shown(&mut self, returned: bool) {
        if self.drawer_depth.is_some_and(|d| d >= self.nav.len()) {
            self.drawer_depth = None;
        }
        self.fresh = true;
        self.returned = returned;
        self.drawer_open = false;
        self.ui.reset();
        // A restored draft is for the page at its place on the stack: gone from there (Back past
        // it, the drawer opened elsewhere), it is not put into some later editor.
        let nav = &self.nav;
        self.drafts.retain(|d| nav.get(d.depth).is_some_and(|(r, _)| r == &d.route));
        // An Undo belongs to the page it was made on and the one Back leads to: a new page drops
        // it, so its Undo can't be hit there over something unrelated. Words alone stay.
        if !returned && self.toast.as_ref().is_some_and(|t| t.undo.is_some() || t.action.is_some()) {
            self.toast = None;
        }
        if self.focus.id.take().is_some() {
            self.host.keyboard(false, "", false);
        }
        if let Route::Entry { id, kind, account, category } = self.route().clone() {
            if returned {
                // Back from a page the editor opened (a new account or category, a bill): the
                // draft stays, and what was just made is picked.
                self.entry.came_back(&self.store);
            } else {
                self.entry = crate::entry::Entry::open(&self.store, id, kind, account, category);
            }
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
        self.shown(false);
    }

    /// Open a tab's page from inside another page (Overview's "See all"): on top of it, with Back,
    /// rather than in its place.
    pub fn push_over(&mut self, route: Route) {
        if self.route() == &route {
            return;
        }
        self.nav.push((route, Scroll::default()));
        self.shown(false);
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
        self.shown(false);
    }

    /// Replace the page showing (an editor that saved and opens what it made).
    pub fn replace(&mut self, route: Route) {
        if self.nav.len() > 1 {
            self.nav.pop();
        }
        self.nav.push((route, Scroll::default()));
        self.shown(false);
    }

    /// Close the page showing. False when it was the last one.
    pub fn pop(&mut self) -> bool {
        if self.nav.len() <= 1 {
            return false;
        }
        self.nav.pop();
        self.shown(true);
        true
    }

    /// Back: typing stops first, then the menu closes, then a sheet or dialog open on the page,
    /// then the page; a space page other than Overview goes to Overview. False when there was
    /// nowhere to go back to.
    pub fn back(&mut self) -> bool {
        // No ledger, no pages: Back leaves.
        if self.ledger_error.is_some() {
            return false;
        }
        if self.focus.id.is_some() {
            self.finish_typing();
            return true;
        }
        if self.drawer_open {
            self.drawer_open = false;
            return true;
        }
        if self.close_over_page() {
            self.ui.reset();
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

    /// Close what is open over the page showing (a sheet, a dialog, a switcher), as its own
    /// module knows it. True when something closed.
    fn close_over_page(&mut self) -> bool {
        match self.route() {
            Route::Entry { .. } => crate::entry::back(self),
            Route::Entries | Route::Transactions { .. } | Route::Search | Route::EntryView { .. } => crate::activity::back(self),
            Route::Budgets | Route::BudgetEdit { .. } | Route::Bills | Route::BillEdit { .. } | Route::Goals | Route::Goal { .. } | Route::GoalEdit { .. } => {
                crate::plan::back(self)
            }
            Route::Accounts | Route::AccountEdit { .. } => crate::accounts::back(self),
            Route::Categories | Route::CategoryEdit { .. } => crate::categories::back(self),
            Route::Banks => crate::banks::back(self),
            Route::Import => crate::import::back(self),
            Route::Settings | Route::Appearance | Route::Format | Route::Backup | Route::About => crate::settings::back(self),
            Route::Sync => crate::sync::back(self),
            Route::Onboarding => crate::onboarding::back(self),
            _ => false,
        }
    }

    // Typing

    /// Text the keyboard sent: the field's whole text (Android's input state).
    pub fn set_text(&mut self, text: &str) {
        if self.focus.id.is_some() {
            self.focus.text = text.chars().take(MAX_TYPED).collect();
            self.typed_unseen = true;
            // Cut short: the keyboard's copy is cut too, so the two never drift apart.
            if text.chars().count() > MAX_TYPED {
                self.host.keyboard_kind(true, &self.focus.text, self.focus.kb());
            }
        }
    }

    /// The keyboard's Done, or back while typing: the field lets go and the keyboard goes.
    pub fn finish_typing(&mut self) {
        if self.focus.id.take().is_some() {
            self.focus.at = None;
            self.host.keyboard(false, "", false);
        }
    }

    /// The keyboard's Done (or Back while typing) from the platform, between frames. Text that
    /// came with it, in the same batch of input, has reached no field yet (a field copies what
    /// the keyboard holds as it draws, and only while it is focused): one frame draws first so
    /// the field takes it, then the field lets go.
    pub fn done_typing(&mut self, now_ns: u64) {
        if self.typed_unseen && self.focus.id.is_some() {
            let _ = self.frame(now_ns);
        }
        self.finish_typing();
    }

    /// Characters typed on a hardware keyboard (the desktop).
    pub fn type_text(&mut self, s: &str) {
        if self.focus.id.is_some() {
            self.focus.text.push_str(s);
            self.typed_unseen = true;
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
            self.typed_unseen = true;
        } else if matches!(self.route(), Route::Entry { .. }) {
            self.entry.amount = self.entry.amount.backspace();
        }
    }

    /// Open the ledger again after it failed to open (Try again, or the app coming back to the
    /// front). True once it is open; the app then starts as a fresh launch would.
    pub fn retry_ledger(&mut self) -> bool {
        if self.ledger_error.is_none() {
            return true;
        }
        let path = self.host.data_dir().join("tally.db");
        match Store::open(&path) {
            Ok(store) => {
                self.store = store;
                self.ledger_error = None;
                self.ledger_tries = 0;
                self.home = None;
                self.apply_appearance();
                self.nav = vec![(start_route(&self.store), Scroll::default())];
                self.drawer_depth = None;
                self.shown(false);
                // The pages and draft kept from before the process was stopped, held while the
                // ledger was in the way.
                if let Some(kept) = self.kept.take() {
                    self.restore(&kept);
                }
                self.invalidate();
                true
            }
            Err(e) => {
                self.host.log(&format!("ledger at {}: {e}", path.display()));
                self.ledger_error = Some(e);
                self.ledger_tries += 1;
                false
            }
        }
    }

    /// The pages open, to keep across the process being stopped (Android's saved state).
    pub fn saved(&self) -> Saved {
        // While the ledger is in the way, what was kept before is kept again, not the stand-in's
        // welcome page in its place.
        if let Some(kept) = &self.kept {
            return Saved { picking: None, ..kept.clone() };
        }
        // The editor's draft, while the editor is open (on top, or under a page it opened).
        let entry = self.nav.iter().any(|(r, _)| matches!(r, Route::Entry { .. })).then(|| self.entry.saved_draft());
        // The other editors' drafts (a bill, a goal, a budget, a goal's Add money, an account, a
        // category, a contribution room) whose pages are open, and those restored but not opened yet.
        let mut drafts = crate::plan::saved_drafts(self);
        drafts.extend(crate::accounts::saved_draft(self));
        drafts.extend(crate::categories::saved_draft(self));
        drafts.extend(crate::invest::saved_draft(self));
        for d in &self.drafts {
            if !drafts.iter().any(|x| x.depth == d.depth) {
                drafts.push(d.clone());
            }
        }
        // A field half typed: what it holds, so it is typing again, with that, once restored.
        let typing = self.focus.id.map(|id| SavedTyping { id: id.to_string(), text: self.focus.text.clone(), numeric: self.focus.numeric });
        Saved { nav: self.nav.iter().map(|(r, _)| r.clone()).collect(), drawer_depth: self.drawer_depth, picking: None, entry, drafts, typing }
    }

    /// The draft [`App::restore`] kept for the page showing, if it is the page it was kept for:
    /// its editor takes it once, just after it opens afresh.
    pub fn take_draft(&mut self) -> Option<Draft> {
        let depth = self.nav.len().checked_sub(1)?;
        let i = self.drafts.iter().position(|d| d.depth == depth && &d.route == self.route())?;
        Some(self.drafts.remove(i).draft)
    }

    /// Where the newest page for `route` sits on the stack, to keep a draft for it.
    pub fn depth_of(&self, route: &Route) -> Option<usize> {
        self.nav.iter().rposition(|(r, _)| r == route)
    }

    /// Open again the pages [`App::saved`] kept, as the system brings the app back after
    /// stopping it. Nothing happens on a first run, without a ledger, or with nothing sensible
    /// kept. The editor opens again with what was typed in it. While the ledger will not open,
    /// it is held, and opened once [`App::retry_ledger`] opens the ledger.
    pub fn restore(&mut self, saved: &Saved) {
        if self.ledger_error.is_some() {
            self.kept = Some(saved.clone());
            return;
        }
        let usable = self.ledger_error.is_none()
            && self.route() != &Route::Onboarding
            && !saved.nav.is_empty()
            && !saved.nav.contains(&Route::Onboarding)
            // A space page replaces everything open, so it is only ever first, except the one
            // Overview's "See all" opens over Overview with Back (`push_over`): second, on
            // Overview.
            && saved.nav.iter().enumerate().skip(1).all(|(i, r)| {
                !r.is_space() || (i == 1 && saved.nav[0] == Route::Overview && r != &Route::Overview)
            });
        if !usable {
            return;
        }
        self.nav = saved.nav.iter().map(|r| (r.clone(), Scroll::default())).collect();
        self.drawer_depth = saved.drawer_depth.filter(|d| *d < self.nav.len());
        self.shown(false);
        // After `shown`, which opens the editor afresh on its route. Under a page the editor
        // opened, it is opened here, so coming back to it finds the draft.
        if let Some(draft) = &saved.entry
            && let Some(Route::Entry { id, kind, account, category }) = self.nav.iter().rev().map(|(r, _)| r).find(|r| matches!(r, Route::Entry { .. })).cloned()
        {
            if !matches!(self.route(), Route::Entry { .. }) {
                self.entry = crate::entry::Entry::open(&self.store, id, kind, account, category);
            }
            self.entry.restore_draft(draft);
        }
        // After `shown`, which lets go of drafts not for the pages open. Each editor takes its
        // own as it opens (`take_draft`), after reading what is stored, so the reading does not
        // wipe what was typed.
        self.drafts = saved.drafts.iter().filter(|d| self.nav.get(d.depth).is_some_and(|(r, _)| r == &d.route)).cloned().collect();
        // The field that was typing types again, keyboard down: its page copies the text into
        // itself as it draws the field, and a tap on it brings the keyboard back. Field names are
        // fixed words in the code; the few bytes kept for this one live as long as the app.
        if let Some(t) = &saved.typing
            && !t.id.is_empty()
            && t.id.len() <= 64
        {
            self.focus.id = Some(Box::leak(t.id.clone().into_boxed_str()));
            self.focus.text = t.text.chars().take(MAX_TYPED).collect();
            self.focus.numeric = t.numeric;
            self.focus.kind = None;
        }
    }

    /// A file in hand for the page asking now (one shared to Tally, one a stopped process asked
    /// for, a test): it answers the latest pick.
    pub fn file_picked(&mut self, name: String, bytes: Vec<u8>) {
        self.file_delivered(crate::settings_files::current_pick(), name, bytes);
    }

    /// A file came back from `Host::pick_file`, for the pick numbered `pick`.
    pub fn file_delivered(&mut self, pick: u64, name: String, bytes: Vec<u8>) {
        self.picked = Some((name, bytes));
        self.picked_for = pick;
    }

    pub fn say(&mut self, words: impl Into<String>, undo: Option<i64>) {
        self.toast = Some(Toast { words: words.into(), undo, restore: false, action: None, until_ns: self.now_ns + TOAST_NS });
    }

    /// A toast for an entry just deleted: its Undo restores it.
    pub fn say_restore(&mut self, words: impl Into<String>, id: i64) {
        self.toast = Some(Toast { words: words.into(), undo: Some(id), restore: true, action: None, until_ns: self.now_ns + TOAST_NS });
    }

    /// A toast whose Undo does `undo` (anything but an entry: a goal, a bill, a category).
    pub fn say_undo(&mut self, words: impl Into<String>, undo: impl FnOnce(&mut App) -> Result<(), String> + Send + 'static) {
        self.toast = Some(Toast { words: words.into(), undo: None, restore: false, action: Some(Box::new(undo)), until_ns: self.now_ns + TOAST_NS });
    }

    /// Draw again at `at` (on [`crate::clock::now_ns`]) even with no input: something on the
    /// page changes then (a notice goes).
    pub fn wake_at(&mut self, at: u64) {
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
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

    /// The launcher icon's look: dark or light while Appearance is set to Dark or Light, none
    /// while it follows the phone. The icon switches only on such a choice: turning a launcher
    /// alias off makes Pixel Launcher and Launcher3 drop its home-screen shortcut, which would
    /// happen at every light/dark change of the phone if the icon followed it too.
    pub fn icon_dark(&self) -> Option<bool> {
        let mode = self.appearance();
        let (light, dark) = (theme::for_mode(&mode, false), theme::for_mode(&mode, true));
        (light.light == dark.light).then_some(!light.light)
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
        // Under a sheet or dialog (`widgets::hold`) the page holds still: the drag is the sheet's.
        // Under the drawer too: a finger put down on the drawer is the drawer's, and must not go
        // to stopping the page coasting under it.
        let moving = if ui.was_covered() || self.drawer > 0.001 { false } else { sc.update(ui, viewport, dt) };
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
                // This frame drew with the old offset: draw again now, or the field stays under
                // the keyboard until the next key.
                self.wake_at(self.now_ns);
            }
        }
    }

    /// Draw one frame. Returns when the next is wanted.
    pub fn frame(&mut self, now_ns: u64) -> Next {
        let dt = ((now_ns.saturating_sub(self.last_ns)) as f32 / 1e9).clamp(0.0, 0.05);
        self.last_ns = now_ns;
        self.now_ns = now_ns;
        self.wake = None;
        // The focused field copies what was typed as it draws, below.
        self.typed_unseen = false;
        if self.ledger_error.is_some() {
            return self.frame_blocked();
        }
        self.follow_keyboard(now_ns);
        let today = model::today();
        // Bills due today post before this frame's sync can start (the first frame, or the day
        // turning while the app runs): a sync started first would find them moving the ledger
        // under it, which for a whole pull asks the person about entries nobody made.
        self.post_bills(today);
        crate::sync::tick(self, now_ns);
        // Whether a bank wants a sign-in soon, for Overview: a listing every few hours.
        crate::banks::watch(self);
        // A whole ledger a sync took in place of this one brings bills of its own, and rows a sync
        // brought can be a bill made, resumed or moved to today on another device: those due
        // today post now, not tomorrow. (`|`, not `||`: both are said once, and both are taken.)
        if self.st.sync.took_ledger() | self.st.sync.received_rows() {
            self.posted_on = None;
            self.post_bills(today);
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

        // The toast sits over everything: it takes a tap on it before the page (or the drawer)
        // under it can, wherever it is.
        let toast_tap = match (self.ui.tap, self.toast_rect()) {
            (Some((tx, ty_)), Some([x, y, w, h])) if tx >= x && tx < x + w && ty_ >= y && ty_ < y + h => self.ui.tap.take(),
            _ => None,
        };

        // The fixed bars take their taps first, so a tap on them never lands on what scrolls under.
        let space = self.route().is_space();
        let bar_top = self.canvas.h - self.bar_h();
        let row_bottom = self.canvas.inset_top + if self.route() == &Route::Search { 0.0 } else { TOP_ROW };
        // The swipe brings the drawer out where its key shows; a page opened from another has Back.
        let drawer_moving = self.drawer_gesture(dt, (space && self.nav.len() == 1) || self.drawer_page());
        let drawer_shown = self.drawer > 0.001;
        // Under a sheet or dialog (`widgets::hold`, the Entries month switcher) the tap is the
        // sheet's, bars and all: its veil covers them.
        let chrome_tap = match self.ui.tap {
            Some((_, ty)) if space && !drawer_shown && !self.ui.was_covered() && ((ty >= bar_top && !self.keyboard_up) || ty < row_bottom) => {
                self.ui.tap.take()
            }
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
            if chrome_tap.is_some() {
                self.ui.tap = chrome_tap;
            }
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
        // A picture asked for is decoding. With a waker (Android) the decoder wakes the loop when it
        // lands, and the platform loop draws once more for pictures that landed late; with none
        // (the desktop, a test) frames keep coming until it has.
        moving |= moving_pictures && !crate::pictures::has_waker();
        moving |= crate::pictures::pending() && !crate::pictures::has_waker();
        // Only the tap it claimed reaches the toast: what the page left of the frame's is spent.
        self.ui.tap = toast_tap;
        if let Some(until) = self.draw_toast() {
            // Wake again when it is due to go.
            self.wake_at(until + 1);
        }

        self.canvas.finish();
        self.ui.end_frame();
        let wake = [self.st.sync.wake_at(now_ns), self.wake].into_iter().flatten().min();
        if moving || tapped || self.ui.busy() || self.fresh {
            Next::Now
        } else if let Some(at) = wake {
            Next::At(at)
        } else {
            Next::OnInput
        }
    }

    /// Bills were saved, resumed, put back or replaced: those now due post on the next frame, not
    /// tomorrow. Without it a bill made today, due today, would wait for tomorrow's post while the
    /// person logged it by hand, and then post a second time.
    pub fn bills_changed(&mut self) {
        self.posted_on = None;
    }

    /// Bills that fell due while the app was closed are posted once a day, as the Kotlin app's
    /// worker does, and again after [`App::bills_changed`].
    fn post_bills(&mut self, today: relay_money::Date) {
        if self.posted_on == Some(today) {
            return;
        }
        // Failed, it is not marked posted: the next frame tries again, not tomorrow.
        if let Ok(n) = self.store.ledger.post_due(today) {
            if n > 0 {
                // Posted bills are changes like any other: they go to the server too.
                self.invalidate();
            }
            self.posted_on = Some(today);
        }
    }

    /// A frame without a ledger: what happened, why, and Try again. Nothing else is drawn or run
    /// (no page, no bills posted, no sync), so nothing can be written to the stand-in.
    fn frame_blocked(&mut self) -> Next {
        let tapped = self.ui.tap.is_some();
        let (w, h) = self.size;
        self.canvas.begin(w, h, self.insets.0, self.insets.1);
        self.focus.at = None;
        let p = self.palette;
        kit::ground(&mut self.canvas, &p);
        let why = self.ledger_error.clone().unwrap_or_default();
        let tries = self.ledger_tries;
        let retry;
        {
            let mut cx = cx!(self);
            let x = kit::PAD;
            let tw = WIDTH - 2.0 * kit::PAD;
            let bt = Text::new(ty::UI, p.ink2).line(1.4);
            let body = "Tally could not open the ledger on this phone. Until it does, it shows nothing and saves nothing, so nothing you type now can be lost. Try again, or close Tally and open it again.";
            let rt = Text::new(ty::CAPTION, p.ink3).line(1.4);
            let reason = if tries > 0 { format!("Still could not open it. {why}") } else { why };
            let content_h = 48.0 + 20.0 + 30.0 + cx.c.para_h(body, tw, bt, 6) + 14.0 + cx.c.para_h(&reason, tw, rt, 6) + 28.0 + 52.0;
            let mut y = ((cx.c.h - content_h) / 2.0).max(cx.c.inset_top + 24.0);
            cx.c.rect(x, y, 48.0, 48.0, 15.0, p.card2);
            cx.c.icon("file-lock", x + 24.0, y + 24.0, 22.0, p.ink2);
            y += 48.0 + 20.0;
            cx.c.text("Your ledger could not be opened", x, y, Text::new(ty::TITLE, p.ink).semi());
            y += 30.0;
            y += cx.c.para(body, x, y, tw, bt, 6) + 14.0;
            y += cx.c.para(&reason, x, y, tw, rt, 6) + 28.0;
            retry = kit::key(&mut cx, x, y, tw, 52.0, "Try again", None, kit::KeyKind::Primary);
        }
        if retry {
            self.retry_ledger();
        }
        self.canvas.finish();
        self.ui.end_frame();
        if tapped || self.ui.busy() || self.ledger_error.is_none() { Next::Now } else { Next::OnInput }
    }

    /// The bottom of the toast's place: over the bottom bar on a tab's page, over the editor's
    /// keys on an entry or a budget, over the footer's key on Import and Banks, above the welcome's
    /// words and keys (never on them: a tap on the pill is the toast's, so it would eat 0 and ⌫,
    /// Undo the last entry, take the Done an import just put there, or Start fresh), else over the
    /// bottom edge.
    fn toast_bottom(&self) -> f32 {
        // Where `settings_kit::footer_top` puts a footer of one row of keys.
        let footer = self.canvas.h - self.canvas.inset_bottom - crate::settings_kit::FOOTER_KEYS - 28.0;
        match self.route() {
            r if r.is_space() => self.canvas.h - self.bar_h(),
            Route::Entry { .. } if self.entry.footer_top > 0.0 => self.entry.footer_top - 4.0,
            Route::BudgetEdit { .. } => self.canvas.h - crate::plan_kit::KEYPAD_H - self.canvas.inset_bottom - 4.0,
            // Import's footer shows on its preview and when done, while the keyboard is down.
            Route::Import if matches!(crate::import::stage(self), "preview" | "done") && !self.keyboard_up => footer - 4.0,
            // Banks' footer comes and goes with what its page reads; the toast keeps clear of
            // its place either way.
            Route::Banks => footer - 4.0,
            // The welcome's keys fill its foot (Start fresh sits on the bottom edge): the toast goes
            // above its words and keys, over the picture, and stays on screen when they scroll.
            Route::Onboarding if self.st.onboarding.block_top > 0.0 => (self.st.onboarding.block_top - 4.0).max(self.canvas.inset_top + 72.0),
            _ => self.canvas.h - self.canvas.inset_bottom - 8.0,
        }
    }

    /// The toast waits unseen (it stays live and shows again once what hid it goes, until it runs
    /// out), and takes no tap meanwhile:
    /// - while a sheet or dialog is over the page, or the drawer is out: it would sit over their
    ///   rows, and a tap meant for a row (a day, the veil that closes the drawer) would Undo;
    /// - on an entry while the keyboard is up: its place is just above the keyboard, over the
    ///   note's chips, where a tap meant for a chip would Undo the entry just saved;
    /// - an Undo on a new entry (the next one after "Save and add another"), as long as it shows:
    ///   the keypad folds as soon as a category or a chip is picked, and the toast would then sit
    ///   over the account and day chips, Undo under the day. Words alone still show there (why
    ///   Save did nothing).
    fn toast_hidden(&self) -> bool {
        if self.ui.was_covered() || self.drawer > 0.001 {
            return true;
        }
        if !matches!(self.route(), Route::Entry { .. }) {
            return false;
        }
        let undo = self.toast.as_ref().is_some_and(|t| t.undo.is_some() || t.action.is_some());
        self.keyboard_up || self.entry.picking.is_some() || (self.entry.editing.is_none() && undo)
    }

    /// Where the toast is (screen units), while there is one and it shows.
    fn toast_rect(&self) -> Option<[f32; 4]> {
        let t = self.toast.as_ref()?;
        if self.now_ns > t.until_ns || self.toast_hidden() {
            return None;
        }
        let (h, bottom) = (48.0, self.toast_bottom());
        Some([16.0, bottom - h - 12.0, WIDTH - 32.0, h])
    }

    /// The system took the keyboard away (Back on Android's keyboard is the keyboard's own): the
    /// field lets go too, so nothing waits on a keyboard that is gone and a tap on the field
    /// brings it back.
    fn follow_keyboard(&mut self, now_ns: u64) {
        const SETTLE_NS: u64 = 300_000_000;
        if self.keyboard_was_up && !self.keyboard_up {
            self.keyboard_gone = self.focus.id.map(|id| (now_ns, id));
        }
        if self.keyboard_up {
            self.keyboard_gone = None;
        }
        if let Some((at, id)) = self.keyboard_gone {
            if self.focus.id != Some(id) {
                self.keyboard_gone = None;
            } else if now_ns.saturating_sub(at) >= SETTLE_NS {
                self.keyboard_gone = None;
                self.finish_typing();
            } else {
                self.wake_at(at + SETTLE_NS);
            }
        }
        self.keyboard_was_up = self.keyboard_up;
        self.focus.keyboard_up = self.keyboard_up;
    }

    /// Over a tab's page: the drawer's key, the page's title, bills and settings.
    fn top_bar(&mut self, route: &Route) {
        let top = self.canvas.inset_top + 6.0;
        // Due within three days, or overdue: a reminder past its date still wants paying.
        let soon = self.home.as_ref().is_some_and(|h| h.s.bills.iter().any(|b| b.days_until <= 3));
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
        // Not over a sheet or dialog: the swipe is theirs, and the drawer would take the person
        // elsewhere with the question still open.
        let covered = ui.was_covered();
        if ui.horizontal && (space || self.drawer > 0.0) && !from_carousel && !self.keyboard_up && !covered {
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
        // Banks that want a sign-in, as last listed (none shown on a phone not paired).
        let to_sign = if self.st.sync.paired(&self.store) { crate::banks::renewals(self).len() } else { 0 };
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
                        ("sync", "Connected banks", Route::Banks, if to_sign > 0 { format!("{to_sign} to sign in") } else { String::new() }, Some(p.waiting_text)),
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
                    // The page showing is a filled row, as a chosen key is; a press washes over it.
                    if on {
                        cx.c.rect(lx, y, rw, 44.0, 12.0, p.card2);
                    }
                    if cx.pressed(lx, y, rw, 44.0) {
                        cx.c.rect(lx, y, rw, 44.0, 12.0, p.wash);
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

    /// The toast: an ink pill in its place ([`App::toast_bottom`]), with Undo when there is
    /// something to undo. Returns when it is due to go, while it shows.
    fn draw_toast(&mut self) -> Option<u64> {
        if self.toast_hidden()
            && let Some(until) = self.toast.as_ref().map(|t| t.until_ns).filter(|t| self.now_ns <= *t)
        {
            // Hidden under a sheet or the drawer: nothing else may draw once it goes (closing a
            // sheet is one tap and one frame), so look again soon rather than at the toast's end.
            if self.ui.was_covered() || self.drawer > 0.001 {
                self.wake_at(self.now_ns + 50_000_000);
            }
            return Some(until);
        }
        let Some([x, y, w, h]) = self.toast_rect() else {
            self.toast = None;
            return None;
        };
        let t = self.toast.as_ref()?;
        let (words, undo, restore, until) = (t.words.clone(), t.undo, t.restore, t.until_ns);
        let can_undo = undo.is_some() || t.action.is_some();
        let mut cx = cx!(self);
        let dy = cx.c.dy;
        cx.c.dy = 0.0;
        cx.c.shadow(x, y + 3.0, w, h, 16.0, 14.0, Rgba::argb(0x331A1A2E));
        cx.c.rect(x, y, w, h, 16.0, Rgba::hex(0x1A1A2E));
        let tt = Text::new(ty::UI, Rgba::WHITE).medium();
        let room = if can_undo { w - 32.0 - 60.0 } else { w - 32.0 };
        cx.c.text_fit(&words, x + 16.0, y + (h - tt.lh()) / 2.0, room, tt);
        let mut undone = false;
        if can_undo {
            let ut = Text::new(ty::UI, Rgba::hex(0xC9B8FF)).semi().right();
            cx.c.text("Undo", x + w - 16.0, y + (h - ut.lh()) / 2.0, ut);
            undone = cx.tap(x + w - 80.0, y, 80.0, h);
        }
        // A tap on the rest of the pill goes nowhere.
        let _ = cx.tap(x, y, w, h);
        cx.c.dy = dy;
        if !undone {
            return Some(until);
        }
        let t = self.toast.take()?;
        let done = match (t.action, undo) {
            (Some(f), _) => f(self),
            (None, Some(id)) if restore => self.store.restore(id),
            (None, Some(id)) => self.store.delete(id),
            (None, None) => Ok(()),
        };
        match done {
            Ok(()) => self.invalidate(),
            Err(e) => self.say(format!("Could not undo: {e}"), None),
        }
        self.toast.as_ref().map(|t| t.until_ns)
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

/// What the app keeps of itself while the system has stopped it: the pages open, the entry being
/// typed, and the kind of file a picker was out for (the platform fills that in).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Saved {
    pub nav: Vec<Route>,
    pub drawer_depth: Option<usize>,
    #[serde(default)]
    pub picking: Option<String>,
    /// The entry editor's draft, while it is open.
    #[serde(default)]
    pub entry: Option<crate::entry::SavedDraft>,
    /// The other editors' drafts, each for the page at its place on the stack.
    #[serde(default)]
    pub drafts: Vec<PageDraft>,
    /// The field that was typing, and what it held.
    #[serde(default)]
    pub typing: Option<SavedTyping>,
}

/// An editor's draft kept for the page at `depth` on the stack, which shows `route`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageDraft {
    pub depth: usize,
    pub route: Route,
    pub draft: Draft,
}

/// What was typed in an editor, by kind of editor. Ids in it are checked again as it is put back.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Draft {
    Plan(crate::plan::PlanDraft),
    Account(crate::accounts::AccountDraft),
    Category(crate::categories::CategoryDraft),
    /// A contribution room's figure as typed on its keypad.
    Room { input: String },
}

/// The field typing as the system stopped Tally.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTyping {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub numeric: bool,
}

/// The first page: Overview, or the first run's welcome on a new, empty ledger.
fn start_route(store: &Store) -> Route {
    if store.pref("onboarded").is_some() || !store.is_empty() { Route::Overview } else { Route::Onboarding }
}

/// The stored appearance: "system", "light" or "dark" (a palette from the sky look counts).
fn appearance(store: &Store) -> String {
    store.pref("appearance").or_else(|| store.pref("palette")).unwrap_or_else(|| "system".into())
}
