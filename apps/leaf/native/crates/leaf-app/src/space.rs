//! My space (design 42, page 14): what you read, in four tabs.
//!
//! A series is in My space once a chapter of it is read, its bell is on or a chapter is downloaded
//! ([`logic::in_space`]): one added and left alone shows nowhere here (the owner, 2026-11-23).
//!
//! - **Updates**: chapters that came for series in My space over the last two weeks, by day
//!   (Today, Yesterday, then dates), newest first: a small cover, the series, "Chapter 132" and the
//!   time; unread in full ink, read dimmed. A tap opens the reader at that chapter. Belled only
//!   or All, remembered.
//! - **Recent** (artboard A): every series and book, last read first, each with the chapter you
//!   are on out of how many, NEW when chapters came since, when you last read it, and Continue or
//!   Start.
//! - **Following**: webtoon's Subscribed list, the owner's reference. Every series in My space with
//!   its bell, which is the follow: on (filled, the accent), new chapters are announced; off, they
//!   are not. "New chapter" on a cover when some came since you read it. Sorted by Recently read,
//!   Recently updated or Recently followed (when it was added: the server keeps no time for the
//!   bell).
//! - **Downloads**: what is downloaded to this phone for reading offline, per series (the cover,
//!   "12 chapters · 84 MB", the newest), with the room it all takes and a line while chapters are
//!   being saved. Built from the folder alone ([`crate::offline`]): it shows, and opens, with no
//!   server. A row opens the series' saved chapters ([`crate::saved`]). Select (a long press, the
//!   header's Select, or ⋯) picks series, and the bar at the bottom deletes them, or only their read
//!   chapters; ⋯ → "Delete read chapters" does so everywhere. Only this phone's copies go.
//!
//! - **Lists**: your own lists ([`crate::lists`]): each with its covers, how many, the first titles
//!   and On Home; New list. A list shown on Home is here too.
//!
//! Updates, Recent, Following and Lists have a field under the tabs that narrows the list to titles
//! holding what is typed ([`logic::title_matches`]: any case, any accents, inside words). Following
//! has a row of shelves (Reading, Plan to read, On hold, Dropped, Done) with their counts; a long
//! press on a series' row in any list puts it on another shelf.
//!
//! The + menu adds books from the phone or a series from Search; ⋯ holds Highlights and Reading
//! time, which are no longer tabs. While a menu is open the page under it gets no tap (the + menu's
//! tap went through to the cover under it, 2026-11-22).

use std::collections::{BTreeSet, HashMap};

use leaf_proto::{Series, Update};

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::home::new_tag;
use crate::images::CoverOf;
use crate::kit::{self, Kb};
use crate::logic::{self, Shelf};
use crate::net::Job;
use crate::shelf::{self, Book};
use crate::theme::{self as t, PAD, ty};

/// A row of Recent: finished, chapters left, its place in the order, and what it is.
type RecentRow<'a> = (bool, Option<u32>, (u8, i64, i64), Result<&'a Series, &'a Book>);

/// The fixed top: the title and its keys, then the tabs.
const TOP: f32 = 104.0;
/// Genres offered as chips in Following.
const GENRE_CHIPS: usize = 12;

/// The filter field under the tabs (Updates, Recent, Following).
const FIELD: f32 = 54.0;
const TABS: [&str; 5] = ["UPDATES", "RECENT", "FOLLOWING", "DOWNLOADS", "LISTS"];
pub const UPDATES: u8 = 0;
pub const RECENT: u8 = 1;
pub const FOLLOWING: u8 = 2;
pub const DOWNLOADS: u8 = 3;
/// Your own lists ([`crate::lists`]), on Home or not (the owner, 2026-11-26).
pub const LISTS: u8 = 4;
/// How far back Updates looks, in days.
pub const UPDATE_DAYS: u32 = 14;
/// A series' row in Recent, Following and Downloads: its square cover flush left is as tall. The
/// owner found the drawn 112 too big (2026-11-23): more series fit at 88.
pub const ROW_H: f32 = 88.0;

#[derive(Default)]
pub struct State {
    pub tab: u8,
    pub add_menu: bool,
    pub more_menu: bool,
    pub sort_menu: bool,
    /// Where the sort menu drops from (screen units).
    sort_top: f32,
    /// What the list is narrowed to.
    pub query: String,
    /// Following shows one shelf only.
    pub shelf: Option<Shelf>,
    chips: crate::ui::HScroll,
    /// The shelf sheet is open for this series (a long press on its row).
    pub shelf_for: Option<i64>,
    /// Downloads: selecting series (their ids). None when not selecting.
    pub select: Option<BTreeSet<i64>>,
    /// Downloads: chapters to delete, waiting for the confirm.
    pub confirm: Option<Vec<i64>>,
    /// The sheet of series in My space twice is open.
    pub dupes: bool,
    /// Following shows one genre only (as its sources name it, any case).
    pub genre: Option<String>,
    genre_chips: crate::ui::HScroll,
}

impl State {
    /// Close a menu, the shelf sheet or the confirm, if one is open; else stop selecting.
    pub fn close_over(&mut self) -> bool {
        let open = self.covered();
        self.add_menu = false;
        self.more_menu = false;
        self.sort_menu = false;
        self.shelf_for = None;
        self.confirm = None;
        self.dupes = false;
        open || self.select.take().is_some()
    }

    fn covered(&self) -> bool {
        self.add_menu || self.more_menu || self.sort_menu || self.shelf_for.is_some() || self.confirm.is_some() || self.dupes
    }
}

/// "61 MB", "1.3 GB".
pub fn size_words(bytes: u64) -> String {
    let mb = bytes as f64 / 1e6;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else if mb >= 1.0 {
        format!("{} MB", mb.round() as u64)
    } else if bytes > 0 {
        "under 1 MB".into()
    } else {
        "0 MB".into()
    }
}

enum Act {
    Series(i64),
    Continue(i64),
    Book(i64),
    Bell(i64, bool),
    /// A row in Downloads: that series' saved chapters.
    Saved(i64),
    /// Downloads: a tap while selecting (a long press sweeps, [`crate::sweep`]).
    Toggle(i64),
    /// An update: its chapter in the reader.
    Chapter(i64, i64),
    /// A long press on a series' row: the shelf sheet.
    Shelve(i64),
}

/// "Cancel all" at the right of Downloads' saving line: its (height, width).
fn cancel_key(cx: &mut kit::Ctx, y: f32, hit: &mut bool, off: bool) -> (f32, f32) {
    let words = "Cancel all";
    let tt = Text::new(12.5, t::INK).semi().right();
    let w = cx.c.measure(words, tt);
    if !off && cx.pressed(WIDTH - PAD - w - 12.0, y - 10.0, w + 24.0, 38.0) {
        cx.c.rect(WIDTH - PAD - w - 8.0, y - 6.0, w + 16.0, 30.0, 8.0, t::WASH);
    }
    cx.c.text(words, WIDTH - PAD, y, tt);
    if !off && cx.tap(WIDTH - PAD - w - 12.0, y - 10.0, w + 24.0, 38.0) {
        *hit = true;
    }
    (tt.lh(), w)
}

/// A finger held on (x, y) since long enough to open a row's sheet: the row under it is asked
/// with [`held_on`].
fn held_on(held: Option<(f32, f32)>, y: f32, h: f32, dy: f32) -> bool {
    held.is_some_and(|(_, hy)| hy >= y + dy && hy < y + h + dy)
}

#[allow(clippy::too_many_arguments)]
fn cover(cx: &mut kit::Ctx, im: &mut crate::images::Images, s: &Series, x: f32, y: f32, w: f32, h: f32, r: f32, now: u64) {
    let of = CoverOf::Series(s.id);
    cx.c.rect(x, y, w, h, r, t::S1);
    if im.cover(cx.c, of, now) {
        cx.c.image_cover(&of.key(), x, y, w, h, r, Rgba::WHITE);
    }
}

/// What the sheet of series in My space twice asked for.
enum DupeAct {
    Merge(i64, i64),
    NotSame(i64, i64),
}

/// Pairs of series that look like one: each with its cover, title and where you are, then Merge
/// (the one read further is kept, [`logic::merge_keep`]) or Not the same. Some(None) when it
/// closes.
fn dupes_sheet(cx: &mut kit::Ctx, im: &mut crate::images::Images, pairs: &[(Series, Series)], now: u64) -> Option<Option<DupeAct>> {
    const SHOWN: usize = 3;
    let row_h = 60.0;
    let pair_h = 2.0 * row_h + 8.0 + 44.0 + 24.0;
    let n = pairs.len().min(SHOWN);
    let (top, outside) = kit::sheet(cx, 30.0 + 26.0 + 48.0 + n as f32 * pair_h + 8.0);
    if outside {
        return Some(None);
    }
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    cx.c.text("Followed twice", x, top + 26.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.para("Merge keeps the one read further, with the other's chapters, bell and progress.", x, top + 56.0, w, Text::new(ty::CAPTION, t::INK3).line(1.4), 2);
    let mut y = top + 30.0 + 26.0 + 48.0;
    let mut out = None;
    for (a, b) in pairs.iter().take(SHOWN) {
        for s in [a, b] {
            let c = 44.0;
            cover(cx, im, s, x, y + (row_h - c) / 2.0, c, c, 6.0, now);
            let tx = x + c + 12.0;
            let title_t = Text::new(14.5, t::INK).semi();
            let line_t = Text::new(12.5, t::INK3);
            let ty0 = y + (row_h - title_t.lh() - 2.0 - line_t.lh()) / 2.0;
            cx.c.text_fit(&s.title, tx, ty0, x + w - tx, title_t);
            let line = match (logic::at_chapter(s, None), s.latest) {
                (Some(n), Some(l)) => format!("Chapter {} of {}", logic::num(n), logic::num(l)),
                (None, Some(l)) => format!("Not started · {} chapters", logic::num(l)),
                _ => "No chapters yet".to_string(),
            };
            cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 2.0, x + w - tx, line_t);
            y += row_h;
        }
        y += 8.0;
        let kw = (w - 10.0) / 2.0;
        if kit::primary(cx, x, y, kw, 44.0, "Merge", None) {
            out = Some(Some(DupeAct::Merge(a.id, b.id)));
        }
        if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Not the same", None) {
            out = Some(Some(DupeAct::NotSame(a.id, b.id)));
        }
        y += 44.0 + 24.0;
    }
    cx.ui.tap = None;
    out
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let tab = app.st.space.tab.min(LISTS);
    if app.fresh {
        if !app.library.loading && app.server.is_some() {
            app.ask(Job::Library);
        }
        if tab == UPDATES && !app.updates.loading && app.server.is_some() {
            app.ask(Job::Updates(UPDATE_DAYS));
        }
        if tab == LISTS {
            app.want_lists();
        }
        // Series followed twice: asked once a session (and after a merge).
        if app.duplicates.data.is_none() && !app.duplicates.loading && app.server.is_some() && !app.shot {
            app.ask(Job::Duplicates);
        }
    }
    // Only what you have done something with: read, followed or downloaded (`logic::in_space`).
    let downloaded = app.saver.series_ids();
    let series: Vec<Series> = app.library.data.iter().flatten().filter(|s| logic::in_space(s, downloaded.contains(&s.id))).cloned().collect();
    let mine: std::collections::HashSet<i64> = series.iter().map(|s| s.id).collect();
    let updates: Vec<Update> = app.updates.data.iter().flatten().filter(|u| mine.contains(&u.series)).cloned().collect();
    let updates_state = (app.updates.loading && app.updates.data.is_none(), app.updates.error.clone().filter(|_| app.updates.data.is_none()));
    let belled_only = !app.updates_all;
    let query = app.st.space.query.clone();
    let filtered = tab != DOWNLOADS;
    let top_h = if filtered { TOP + FIELD } else { TOP };
    let today = shelf::day_of(now_ms_());
    let offset = shelf::zone_offset();
    let books: Vec<Book> = app.shelf.books.clone();
    // Downloads need every series' chapters; Recent and Following count new ones with them.
    for s in &series {
        if !app.series.contains_key(&s.id) {
            app.ask(Job::Series(s.id));
        }
    }
    let new_of: HashMap<i64, u32> = series.iter().map(|s| (s.id, logic::new_chapters(s, app.series.get(&s.id).and_then(|l| l.data.as_ref())))).collect();
    let now = app.now_ns;
    let now_ms = shelf::now_ms();
    let loading = app.library.loading && app.library.data.is_none();
    let offline = app.library.offline();
    let error = app.library.error.clone().filter(|_| !offline);
    let sort = app.following_sort;
    // Pairs of series in My space that look like one, with both still here.
    let pairs: Vec<(Series, Series)> = app
        .duplicates
        .data
        .iter()
        .flatten()
        .filter_map(|[a, b]| Some((series.iter().find(|s| s.id == *a)?.clone(), series.iter().find(|s| s.id == *b)?.clone())))
        .collect();
    if pairs.is_empty() {
        app.st.space.dupes = false;
    }
    let covered = app.st.space.covered();
    let pg = app.page_begin(dt, top_h);
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let st = &mut app.st.space;
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let mut y = pg.top;
    let mut act: Option<Act> = None;
    let mut to_search = false;
    let view_top = -cx.c.dy - 140.0;
    let view_bottom = -cx.c.dy + cx.c.h + 140.0;
    let mut retry_updates = false;
    let mut retry_saves = false;
    let mut cancel_saves = false;
    let mut start_select = false;
    let mut list_act: Option<crate::lists::TabAct> = None;
    let selecting = st.select.clone().filter(|_| tab == DOWNLOADS);
    let groups = if tab == DOWNLOADS { app.saver.groups() } else { Vec::new() };
    let mut set_all: Option<bool> = None;
    let mut rows: Vec<crate::sweep::Row> = Vec::new();
    let held_at = if covered { None } else { cx.ui.resting(now).filter(|(_, ns)| *ns >= crate::series::LONG_PRESS_NS).map(|(p, _)| p) };
    // Series followed twice: one row above Recent and Following, a tap opens the pairs.
    if (tab == RECENT || tab == FOLLOWING) && !pairs.is_empty() && query.trim().is_empty() {
        y += 8.0;
        let h = 56.0;
        if !covered && cx.pressed(0.0, y, WIDTH, h) {
            cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
        }
        cx.c.icon("copy", x0 + 12.0, y + h / 2.0, 18.0, t::INK2);
        let words = if pairs.len() == 1 { "1 series is in My space twice".to_string() } else { format!("{} series are in My space twice", pairs.len()) };
        let (kw, tapped) = kit::pill(&mut cx, WIDTH - PAD, y + (h - 32.0) / 2.0, "Review", false);
        cx.c.text_fit(&words, x0 + 36.0, y + (h - Text::new(ty::UI, t::INK).lh()) / 2.0, WIDTH - PAD - kw - 12.0 - x0 - 36.0, Text::new(ty::UI, t::INK));
        if !covered && (tapped || cx.tap(0.0, y, WIDTH, h)) {
            st.dupes = true;
        }
        y += h;
    }
    match tab {
        UPDATES => {
            let groups = logic::group_updates(&updates, belled_only, &query, shelf::day_of);
            let shown: usize = groups.iter().map(|g| g.1.len()).sum();
            let unread = groups.iter().flat_map(|g| g.1.iter()).filter(|&&i| !updates[i].read).count();
            y += 10.0;
            let head_h = 40.0;
            let ht = Text::new(ty::UI, t::INK2);
            let words = match (shown, unread) {
                (0, _) => format!("Last {UPDATE_DAYS} days"),
                (n, 0) => format!("{} · all read", logic::chapters_words(n)),
                (n, u) => format!("{} · {u} unread", logic::chapters_words(n)),
            };
            let seg_w = 176.0;
            cx.c.text_fit(&words, x0, y + (head_h - ht.lh()) / 2.0, w - seg_w - 12.0, ht);
            if let Some(i) = kit::segmented(&mut cx, WIDTH - PAD - seg_w, y + 2.0, seg_w, &["Belled only", "All"], if belled_only { 0 } else { 1 }) {
                set_all = Some(i == 1);
            }
            y += head_h + 8.0;
            // Due today: a series whose usual day is today (or whose usual gap ends today) and
            // whose chapter hasn't come yet ([`crate::rhythm`]).
            let now_ms = crate::shelf::now_ms();
            let due: Vec<(&Series, String)> = series
                .iter()
                .filter(|s| (!belled_only || s.bell) && logic::title_matches(&s.title, &query) && s.status != "completed")
                .filter_map(|s| {
                    let m = app.more.get(&s.id)?;
                    let u = crate::rhythm::usual(&m.uploads, offset)?;
                    crate::rhythm::due_today(&u, now_ms, offset).then(|| (s, crate::rhythm::usual_words(&u)))
                })
                .collect();
            if !due.is_empty() {
                cx.c.text("DUE TODAY", x0, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
                y += 34.0;
                let rh = 56.0;
                for (s, words) in &due {
                    if cx.pressed(0.0, y, WIDTH, rh) {
                        cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                    }
                    cover(&mut cx, &mut app.images, s, 0.0, y, rh, rh, 0.0, now);
                    let tx = rh + 14.0;
                    let title_t = Text::new(ty::BODY, t::INK).medium();
                    let line_t = Text::new(12.5, t::INK3);
                    let ty0 = y + (rh - title_t.lh() - 3.0 - line_t.lh()) / 2.0;
                    cx.c.text_fit(&s.title, tx, ty0, WIDTH - PAD - tx, title_t);
                    let line = match s.latest {
                        Some(l) => format!("{words} · chapter {} next", logic::num((l + 1.0).floor())),
                        None => words.clone(),
                    };
                    cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 3.0, WIDTH - PAD - tx, line_t);
                    if cx.tap(0.0, y, WIDTH, rh) {
                        act = Some(Act::Series(s.id));
                    }
                    y += rh;
                }
                y += 10.0;
            }
            if groups.is_empty() {
                y += 30.0;
                if updates_state.0 {
                    let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
                    cx.c.arc(WIDTH / 2.0, y + 10.0, 9.0, 2.0, turn, turn + 0.7, t::INK3, false);
                    cx.c.text("Asking your server", WIDTH / 2.0, y + 32.0, Text::new(ty::UI, t::INK2).center());
                    y += 60.0;
                } else if let Some(e) = &updates_state.1 {
                    let (h, r) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, e, None, Some("Retry"));
                    retry_updates = r;
                    y += h;
                } else if !query.trim().is_empty() {
                    let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &format!("No chapters of a title with \u{201C}{}\u{201D}", query.trim()), None, None);
                    y += h;
                } else if belled_only && !updates.is_empty() {
                    let (h, all) = kit::notice(
                        &mut cx,
                        x0 + 16.0,
                        y,
                        w - 32.0,
                        "Nothing new for belled series",
                        Some(&format!("{} came for series with their bell off in the last {UPDATE_DAYS} days.", logic::chapters_words(updates.len()))),
                        Some("Show all"),
                    );
                    if all {
                        set_all = Some(true);
                    }
                    y += h;
                } else {
                    let (h, _) = kit::notice(
                        &mut cx,
                        x0 + 16.0,
                        y,
                        w - 32.0,
                        &format!("No new chapters in the last {UPDATE_DAYS} days"),
                        Some("Chapters your server finds for series in My space show here, newest first."),
                        None,
                    );
                    y += h;
                }
            }
            let rh = 76.0;
            for (day, idx) in &groups {
                if y + 34.0 >= view_top && y <= view_bottom {
                    cx.c.text(&logic::day_words(*day, today).to_uppercase(), x0, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
                }
                y += 34.0;
                for run in logic::fold_updates(&updates, idx) {
                    let i = run[run.len() - 1];
                    let u = &updates[i];
                    let folded = run.len() > 1;
                    let read = run.iter().all(|&j| updates[j].read);
                    if y + rh < view_top || y > view_bottom {
                        y += rh;
                        continue;
                    }
                    if cx.pressed(0.0, y, WIDTH, rh) {
                        cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                    }
                    // A square cover flush left, as every row of My space (design 42, page 16).
                    let cw = rh;
                    let of = CoverOf::Series(u.series);
                    cx.c.rect(0.0, y, cw, rh, 0.0, t::S1);
                    if app.images.cover(cx.c, of, now) {
                        cx.c.image_cover(&of.key(), 0.0, y, cw, rh, 0.0, Rgba::WHITE.a(if read { 0.5 } else { 1.0 }));
                    }
                    let tx = cw + 14.0;
                    let time = logic::clock_words(run.iter().map(|&j| updates[j].at).max().unwrap_or(u.at), offset);
                    let tt_ = Text::new(12.5, t::INK3).right();
                    let tw_ = cx.c.measure(&time, tt_);
                    let title_t = Text::new(ty::BODY, if read { t::INK3 } else { t::INK }).medium();
                    let line_t = Text::new(ty::UI, if read { t::INK3 } else { t::INK2 });
                    let ty0 = y + (rh - title_t.lh() - 4.0 - line_t.lh()) / 2.0;
                    let room = WIDTH - PAD - tw_ - 12.0 - tx;
                    cx.c.text_fit(&u.title, tx, ty0, room, title_t);
                    cx.c.text(&time, WIDTH - PAD, ty0 + 2.0, tt_);
                    let line = match logic::chapter_title(&u.name, u.number) {
                        _ if folded => logic::run_words(&updates, &run),
                        Some(n) => format!("Chapter {} · {n}", logic::num(u.number)),
                        None => format!("Chapter {}", logic::num(u.number)),
                    };
                    cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 4.0, room, line_t);
                    if held_on(held_at, y, rh, cx.c.dy) {
                        cx.ui.swallow();
                        act = Some(Act::Shelve(u.series));
                    } else if cx.tap(0.0, y, WIDTH, rh) {
                        // A folded row opens the series: one chapter of 86 is not the one meant.
                        act = Some(if folded { Act::Series(u.series) } else { Act::Chapter(u.series, u.chapter) });
                    }
                    y += rh;
                }
                y += 6.0;
            }
        }
        RECENT => {
            let with_new = series.iter().filter(|s| new_of[&s.id] > 0).count();
            let mut words = vec![if series.len() == 1 { "1 series".to_string() } else { format!("{} series", series.len()) }];
            if with_new > 0 {
                words.push(format!("{with_new} with new chapters"));
            }
            if !books.is_empty() {
                words.push(if books.len() == 1 { "1 book".into() } else { format!("{} books", books.len()) });
            }
            let found = series.iter().filter(|s| logic::title_matches(&s.title, &query)).count() + books.iter().filter(|b| logic::title_matches(&b.title, &query) || logic::title_matches(&b.author, &query)).count();
            if !query.trim().is_empty() {
                words = vec![format!("{found} of {} found", series.len() + books.len())];
            }
            y += 12.0;
            cx.c.text_fit(&words.join(" · ").to_uppercase(), x0, y, w, Text::new(ty::SMALL, t::INK3).semi());
            y += 30.0;
            // What is being read here, last read first (a belled series' new chapter as of when it
            // came); then what is partway through but never opened here, fewest chapters left
            // first; then what is caught up; then what is not started ([`logic::recent_key`]).
            let detail = |s: &Series| app.series.get(&s.id).and_then(|l| l.data.as_ref());
            let mut rows: Vec<RecentRow> = series
                .iter()
                .filter(|s| logic::title_matches(&s.title, &query))
                .map(|s| {
                    let done = logic::finished(s, detail(s));
                    let left = logic::left_after(s, detail(s));
                    let last = if s.bell && new_of[&s.id] > 0 && s.last_read_at > 0 { s.last_read_at.max(s.updated_at) } else { s.last_read_at };
                    (done, left, logic::recent_key(last, s.added_at, done, left), Ok(s))
                })
                .collect();
            rows.extend(books.iter().filter(|b| logic::title_matches(&b.title, &query) || logic::title_matches(&b.author, &query)).map(|b| {
                let done = b.progress >= 0.995;
                (done, None, logic::recent_key(b.opened_ms, b.added_ms, done, None), Err(b))
            }));
            rows.sort_by_key(|r| r.2);
            if rows.is_empty() && !query.trim().is_empty() && !(series.is_empty() && books.is_empty()) {
                y += 40.0;
                let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &format!("Nothing with \u{201C}{}\u{201D} in its title", query.trim()), None, None);
                y += h;
            } else if rows.is_empty() && !loading {
                y += 40.0;
                let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Nothing here yet", Some("Books you add, and series you read, follow or download, show here, last read first."), None);
                y += h + 20.0;
                if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Find a series", Some("search")) {
                    to_search = true;
                }
                y += 46.0;
            }
            let rh = ROW_H;
            for (done, left, _, r) in rows {
                if y + rh < view_top || y > view_bottom {
                    y += rh;
                    continue;
                }
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let cw = rh;
                let tx = cw + 14.0;
                let (title, line, new, last, key, f, row_act, key_act) = match r {
                    Ok(s) => {
                        cover(&mut cx, &mut app.images, s, 0.0, y, cw, rh, 0.0, now);
                        let line = logic::place_words(s, app.series.get(&s.id).and_then(|l| l.data.as_ref()));
                        // Progress that came with it but never read here: how much is left, not "Not started".
                        let last = match left {
                            _ if s.last_read_at > 0 => format!("Last read {}", logic::ago(s.last_read_at, now_ms)),
                            Some(1) => "1 chapter left".into(),
                            Some(n) if n > 0 => format!("{n} chapters left"),
                            Some(_) => "Caught up".into(),
                            None => "Not started".into(),
                        };
                        if done {
                            // Finished: "Read", and the key opens the series rather than its last chapter again.
                            (s.title.as_str(), "Read".to_string(), 0, last, "Open", 0.0, Act::Series(s.id), Act::Series(s.id))
                        } else {
                            let key = if s.last_read_at > 0 || left.is_some() { "Continue" } else { "Start" };
                            (s.title.as_str(), line, new_of[&s.id], last, key, logic::read_fraction(s), Act::Series(s.id), Act::Continue(s.id))
                        }
                    }
                    Err(b) => {
                        crate::book::cover(cx.c, &mut app.images, b, 0.0, y, cw, rh, now);
                        let last = if b.opened_ms > 0 { format!("Last read {}", logic::ago(b.opened_ms, now_ms)) } else { "Not started".into() };
                        let key = if b.progress > 0.0 { "Continue" } else { "Start" };
                        (b.title.as_str(), crate::library::book_line(b), 0, last, key, b.progress, Act::Book(b.id), Act::Book(b.id))
                    }
                };
                if f > 0.0 {
                    cx.c.rect(0.0, y + rh - 3.0, cw * f.min(1.0), 3.0, 0.0, t::ACCENT);
                }
                let kt = Text::new(13.0, t::INK).semi();
                let (kw, kh) = (cx.c.measure(key, kt) + 24.0, 34.0);
                let (kx, ky) = (WIDTH - PAD - kw, y + (rh - kh) / 2.0);
                if cx.pressed(kx, ky, kw, kh) {
                    cx.c.rect(kx, ky, kw, kh, 8.0, t::WASH);
                }
                cx.c.outline(kx, ky, kw, kh, 8.0, 1.0, t::INK.a(0.28));
                cx.c.text(key, kx + 12.0, ky + (kh - kt.lh()) / 2.0, kt);
                let tw = kx - 12.0 - tx;
                let title_t = Text::new(15.5, t::INK).semi();
                let line_t = Text::new(13.0, t::INK2);
                let last_t = Text::new(12.0, t::INK3);
                let block = title_t.lh() + 3.0 + 18.0 + 2.0 + last_t.lh();
                let ty0 = y + (rh - block) / 2.0;
                cx.c.text_fit(title, tx, ty0, tw, title_t);
                let ly = ty0 + title_t.lh() + 3.0;
                // NEW beside the chapter, the chapter said shorter when both don't fit.
                let tag = format!("{new} NEW");
                let tag_w = cx.c.measure(&tag, Text::new(10.5, t::ON_ACCENT).semi()) + 18.0;
                let room = if new > 0 { tw - tag_w } else { tw };
                let line = if cx.c.measure(&line, line_t) > room { line.replacen("Chapter ", "Ch. ", 1) } else { line };
                let lw = cx.c.text_fit(&line, tx, ly + (18.0 - line_t.lh()) / 2.0, room, line_t);
                if new > 0 {
                    new_tag(cx.c, tx + lw + 8.0, ly, &tag);
                }
                cx.c.text_fit(&last, tx, ly + 18.0 + 2.0, tw, last_t);
                if let (true, Act::Series(id)) = (held_on(held_at, y, rh, cx.c.dy), &row_act) {
                    cx.ui.swallow();
                    act = Some(Act::Shelve(*id));
                } else if cx.tap(kx - 6.0, ky - 8.0, kw + 12.0, kh + 16.0) {
                    act = Some(key_act);
                } else if cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(row_act);
                }
                y += rh;
            }
        }
        FOLLOWING => {
            y += 10.0;
            let ht = Text::new(ty::UI, t::INK2);
            let head_h = 40.0;
            let genre_now = st.genre.clone();
            let mut pick_genre: Option<Option<String>> = None;
            let genre_of = |s: &Series| genre_now.as_ref().is_none_or(|g| app.more.get(&s.id).is_some_and(|m| m.genres.iter().any(|x| x.eq_ignore_ascii_case(g))));
            let shown = series.iter().filter(|s| st.shelf.is_none_or(|sh| logic::shelf_of(s) == sh) && genre_of(s) && logic::title_matches(&s.title, &query)).count();
            let count = match (shown == series.len(), series.len()) {
                (true, 1) => "1 series".to_string(),
                (true, n) => format!("{n} series"),
                (false, n) => format!("{shown} of {n} series"),
            };
            cx.c.text(&count, x0, y + (head_h - ht.lh()) / 2.0, ht);
            let sw = logic::FOLLOWING_SORTS[sort as usize];
            let stt = Text::new(ty::UI, t::INK).medium().right();
            let sww = cx.c.measure(sw, stt);
            cx.c.text(sw, WIDTH - PAD - 22.0, y + (head_h - stt.lh()) / 2.0, stt);
            cx.c.icon("chevron-down", WIDTH - PAD - 8.0, y + head_h / 2.0, 15.0, t::INK2);
            if cx.tap(WIDTH - PAD - 30.0 - sww, y, sww + 30.0 + PAD, head_h) {
                st.sort_menu = true;
                st.sort_top = y + head_h + cx.c.dy;
            }
            y += head_h + 4.0;
            // The shelves, with how many series each holds.
            let counts = logic::shelf_counts(&series);
            let mut chips: Vec<(String, bool, Option<Rgba>)> = vec![(format!("All {}", series.len()), st.shelf.is_none(), None)];
            chips.extend(Shelf::ALL.iter().zip(counts).map(|(sh, n)| (format!("{} {n}", sh.words()), st.shelf == Some(*sh), None)));
            if let Some(i) = kit::chip_row(&mut cx, &mut st.chips, y, &chips, dt) {
                st.shelf = if i == 0 { None } else { Some(Shelf::ALL[i - 1]) };
            }
            y += 32.0 + 10.0;
            // The genres your series have most, as their sources name them.
            let genres = logic::top_genres(series.iter().filter_map(|s| app.more.get(&s.id)).map(|m| m.genres.as_slice()), GENRE_CHIPS);
            if !genres.is_empty() {
                let mut chips: Vec<(String, bool, Option<Rgba>)> = vec![("Any genre".to_string(), st.genre.is_none(), None)];
                chips.extend(genres.iter().map(|(g, n)| (format!("{g} {n}"), st.genre.as_ref().is_some_and(|x| x.eq_ignore_ascii_case(g)), None)));
                if let Some(i) = kit::chip_row(&mut cx, &mut st.genre_chips, y, &chips, dt) {
                    pick_genre = Some(if i == 0 { None } else { Some(genres[i - 1].0.clone()) });
                }
                y += 32.0 + 14.0;
            } else {
                y += 4.0;
            }
            let mut list: Vec<Series> = series.iter().filter(|s| st.shelf.is_none_or(|sh| logic::shelf_of(s) == sh) && genre_of(s) && logic::title_matches(&s.title, &query)).cloned().collect();
            if let Some(g) = pick_genre.take() {
                st.genre = g;
            }
            logic::sort_following(&mut list, sort);
            if list.is_empty() && !series.is_empty() {
                y += 30.0;
                let why = match (st.shelf, query.trim().is_empty()) {
                    (Some(sh), true) => format!("Nothing on {} yet", sh.words()),
                    (Some(sh), false) => format!("Nothing on {} with \u{201C}{}\u{201D} in its title", sh.words(), query.trim()),
                    (None, _) => format!("Nothing with \u{201C}{}\u{201D} in its title", query.trim()),
                };
                let body = st.shelf.map(|_| "Hold a series' row, or use ⋯ on its page, to put it on a shelf.");
                let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &why, body, None);
                y += h;
            } else if list.is_empty() && !loading {
                y += 40.0;
                let (h, _) = kit::notice(
                    &mut cx,
                    x0 + 16.0,
                    y,
                    w - 32.0,
                    "No series yet",
                    Some("Open a series from Search and tap Follow to hear when new chapters come."),
                    None,
                );
                y += h + 20.0;
                if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Find a series", Some("search")) {
                    to_search = true;
                }
                y += 46.0;
            }
            let rh = ROW_H;
            for s in &list {
                if y + rh < view_top || y > view_bottom {
                    y += rh;
                    continue;
                }
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let cw = rh;
                cover(&mut cx, &mut app.images, s, 0.0, y, cw, rh, 0.0, now);
                if new_of[&s.id] > 0 {
                    new_tag(cx.c, 5.0, y + 5.0, "NEW");
                }
                let bx = WIDTH - 6.0 - 44.0;
                let by = y + (rh - 44.0) / 2.0;
                let bell = if s.bell { kit::icon_key_ink(&mut cx, bx, by, "bell-on", t::ACCENT) } else { kit::icon_key_ink(&mut cx, bx, by, "bell", t::INK3) };
                if bell {
                    act = Some(Act::Bell(s.id, !s.bell));
                }
                let tx = cw + 14.0;
                let tw = bx - 4.0 - tx;
                let title_t = Text::new(15.5, t::INK).semi();
                let line_t = Text::new(12.5, t::INK3);
                let block = title_t.lh() + 5.0 + line_t.lh();
                let ty0 = y + (rh - block) / 2.0;
                cx.c.text_fit(&s.title, tx, ty0, tw, title_t);
                // On a break (six weeks without a chapter): said instead of the date.
                let paused = app.more.get(&s.id).filter(|m| crate::rhythm::on_break(&m.uploads, &s.status, crate::shelf::now_ms())).and_then(|m| crate::rhythm::words(&m.uploads, &s.status, crate::shelf::now_ms(), offset));
                let updated = match paused {
                    Some(w) => w,
                    None if s.updated_at > 0 => format!("{} Updated", logic::local_date(s.updated_at)),
                    None => format!("Added {}", logic::local_date(s.added_at)),
                };
                cx.c.text_fit(&updated, tx, ty0 + title_t.lh() + 5.0, tw, line_t);
                if held_on(held_at, y, rh, cx.c.dy) {
                    cx.ui.swallow();
                    act = Some(Act::Shelve(s.id));
                } else if !bell && cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(Act::Series(s.id));
                }
                y += rh;
            }
        }
        LISTS => {
            let all: Vec<Series> = app.library.data.clone().unwrap_or_default();
            let lists = (app.lists.data.as_deref(), app.lists.loading && app.lists.data.is_none());
            y = crate::lists::tab(&mut cx, &mut app.images, &lists, &all, &query, y, now, &mut list_act);
        }
        _ => {
            // What is on this phone, from its own folder: no server needed.
            let total: u64 = groups.iter().map(|g| g.bytes).sum();
            let running = app.saver.running();
            y += 12.0;
            let ht = Text::new(ty::UI, t::INK2);
            let head_h = 36.0;
            cx.c.text(&format!("On this phone: {}", size_words(total)), x0, y + (head_h - ht.lh()) / 2.0, ht);
            if selecting.is_none() && !groups.is_empty() && crate::saved::text_key(&mut cx, y - 4.0, "Select") {
                start_select = true;
            }
            y += head_h;
            // While chapters are being saved: which, how far, and how many wait.
            if let Some((title, number, done, all)) = &running.now {
                let more = running.waiting.saturating_sub(1);
                let mut words = if *all > 0 { format!("Saving {title} ch. {} · {done} of {all} pages", logic::num(*number)) } else { format!("Waiting for your server: {title} ch. {}", logic::num(*number)) };
                if more > 0 {
                    words.push_str(&format!(" · {more} more waiting"));
                }
                let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
                cx.c.arc(x0 + 7.0, y + 9.0, 6.0, 1.8, turn, turn + 0.7, t::ACCENT, false);
                let (kt, kw) = cancel_key(&mut cx, y, &mut cancel_saves, covered || selecting.is_some());
                y += cx.c.para(&words, x0 + 22.0, y, w - 22.0 - kw - 16.0, Text::new(12.5, t::INK2).line(1.4), 2).max(kt) + 10.0;
            } else if running.waiting > 0 {
                let (_, kw) = cancel_key(&mut cx, y, &mut cancel_saves, covered || selecting.is_some());
                cx.c.text_fit(&format!("{} waiting to download", logic::chapters_words(running.waiting)), x0, y, w - kw - 16.0, Text::new(12.5, t::INK3));
                y += 26.0;
            }
            if running.failed > 0 {
                let words = format!("{} didn't save · Retry", logic::chapters_words(running.failed));
                cx.c.icon("flag", x0 + 7.0, y + 9.0, 12.0, t::WARN);
                cx.c.text_fit(&words, x0 + 22.0, y, w - 22.0, Text::new(12.5, t::WARN));
                if !covered && selecting.is_none() && cx.tap(x0, y - 8.0, w, 34.0) {
                    retry_saves = true;
                }
                y += 26.0;
            }
            y += 4.0;
            if groups.is_empty() && app.saver.scanned() {
                y += 20.0;
                let (h, _) = kit::notice(
                    &mut cx,
                    x0 + 16.0,
                    y,
                    w - 32.0,
                    "Nothing downloaded",
                    Some("Chapters you download from a series page are kept on this phone, to read with no connection."),
                    None,
                );
                y += h;
            }
            let rh = ROW_H;
            for g in &groups {
                rows.push(crate::sweep::Row { id: g.series, y, h: rh });
                if y + rh < view_top || y > view_bottom {
                    y += rh;
                    continue;
                }
                let picked = selecting.as_ref().is_some_and(|s| s.contains(&g.series));
                if picked {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::ACCENT_DIM);
                }
                if cx.pressed(0.0, y, WIDTH, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                let cw = rh;
                let of = CoverOf::Series(g.series);
                cx.c.rect(0.0, y, cw, rh, 0.0, t::S1);
                if app.images.cover(cx.c, of, now) {
                    cx.c.image_cover(&of.key(), 0.0, y, cw, rh, 0.0, Rgba::WHITE);
                }
                let tx = cw + 14.0;
                let right = if selecting.is_some() { WIDTH - PAD - 34.0 } else { WIDTH - PAD };
                let tw = right - tx;
                if selecting.is_some() {
                    let (cxx, cyy) = (WIDTH - PAD - 11.0, y + rh / 2.0);
                    if picked {
                        cx.c.circle(cxx, cyy, 11.0, t::ACCENT);
                        cx.c.icon("check", cxx, cyy, 13.0, t::ON_ACCENT);
                    } else {
                        cx.c.outline(cxx - 11.0, cyy - 11.0, 22.0, 22.0, 11.0, 1.5, t::INK3);
                    }
                }
                let title_t = Text::new(15.5, t::INK).semi();
                let line_t = Text::new(13.0, t::INK2);
                let last_t = Text::new(12.0, t::INK3);
                let block = title_t.lh() + 4.0 + line_t.lh() + 2.0 + last_t.lh();
                let ty0 = y + (rh - block) / 2.0;
                cx.c.text_fit(&g.title, tx, ty0, tw, title_t);
                cx.c.text_fit(&format!("{} · {}", logic::chapters_words(g.chapters), size_words(g.bytes)), tx, ty0 + title_t.lh() + 4.0, tw, line_t);
                let newest = format!("Up to chapter {} · saved {}", logic::num(g.newest), logic::ago(g.saved_at, now_ms));
                cx.c.text_fit(&newest, tx, ty0 + title_t.lh() + 4.0 + line_t.lh() + 2.0, tw, last_t);
                if cx.tap(0.0, y, WIDTH, rh) {
                    act = Some(if selecting.is_some() { Act::Toggle(g.series) } else { Act::Saved(g.series) });
                }
                y += rh;
            }
        }
    }
    if let Some(e) = &error {
        y += 12.0;
        y += cx.c.para(e, x0, y, w, Text::new(ty::CAPTION, t::DANGER), 3);
    }
    let bottom = y + if selecting.is_some() { crate::saved::BAR_H } else { kit::TABS_H };
    // The fixed top: the title, + and ⋯, then the tabs.
    let mut set_tab = None;
    let mut stop_select = false;
    let mut select_all = false;
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        kit::fixed_top(&mut cx, top_h);
        match &selecting {
            Some(sel) => {
                if kit::icon_key(&mut cx, 6.0, top + 10.0, "close") {
                    stop_select = true;
                }
                let tt = Text::new(ty::SECTION, t::INK).semi();
                let words = if sel.is_empty() { "Tap series to select".to_string() } else { format!("{} selected", sel.len()) };
                cx.c.text_fit(&words, 56.0, top + 10.0 + (44.0 - tt.lh()) / 2.0, WIDTH - 56.0 - 90.0, tt);
                if crate::saved::text_key(&mut cx, top + 10.0, "All") {
                    select_all = true;
                }
            }
            None => {
                cx.c.text("My space", PAD, top + 14.0, Text::new(ty::TITLE, t::INK).brand());
                if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0 * 2.0, top + 10.0, "plus") {
                    st.add_menu = true;
                }
                if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 10.0, "more") {
                    st.more_menu = true;
                }
            }
        }
        let ty_ = top + 62.0;
        // Spread across the width, never closer than 14 apart; the words a little smaller only when
        // the five don't fit so (a narrow phone).
        let room = WIDTH - 2.0 * PAD - 14.0 * (TABS.len() - 1) as f32;
        let tt = [13.5, 13.0, 12.5, 12.0]
            .iter()
            .map(|&size| Text::new(size, t::INK3).medium())
            .find(|&tt| TABS.iter().map(|w| cx.c.measure(w, tt)).sum::<f32>() <= room)
            .unwrap_or(Text::new(11.5, t::INK3).medium());
        let mut x = PAD;
        let words_w: f32 = TABS.iter().map(|w| cx.c.measure(w, tt)).sum();
        let gap = ((WIDTH - 2.0 * PAD - words_w) / (TABS.len() - 1) as f32).clamp(14.0, 28.0);
        for (i, words) in TABS.iter().enumerate() {
            let on = i as u8 == tab;
            let tw = cx.c.text(words, x, ty_ + (38.0 - tt.lh()) / 2.0, if on { tt.color(t::INK) } else { tt });
            if on {
                cx.c.rect(x, ty_ + 38.0 - 2.0, tw, 2.0, 1.0, t::ACCENT);
            }
            if cx.ui.tapped(x - gap / 2.0, ty_, tw + gap, 38.0) {
                set_tab = Some(i as u8);
            }
            x += tw + gap;
        }
        cx.c.rect(-1000.0, ty_ + 38.0, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
        // The filter: narrows the list to titles holding what is typed.
        if filtered {
            let (fx, fy, fw, fh) = (PAD, ty_ + 38.0 + 8.0, WIDTH - 2.0 * PAD, 40.0);
            cx.c.rect(fx, fy, fw, fh, t::KEY_R, t::S1);
            cx.c.icon("search", fx + 18.0, fy + fh / 2.0, 15.0, t::INK3);
            let mut q = st.query.clone();
            let clear_w = if q.is_empty() { 0.0 } else { 40.0 };
            let hint = match tab {
                UPDATES => "Find in updates",
                RECENT => "Find in Recent",
                LISTS => "Find in Lists",
                _ => "Find in Following",
            };
            kit::inline(&mut cx, "space-filter", fx + 36.0, fy, fw - 36.0 - 14.0 - clear_w, fh, hint, &mut q, Kb::Text, Text::new(ty::UI, t::INK));
            if !q.is_empty() && kit::icon_key_ink(&mut cx, fx + fw - 42.0, fy - 2.0, "close", t::INK2) {
                q.clear();
                if cx.focus.is("space-filter") {
                    cx.focus.text.clear();
                    cx.host.keyboard(true, "", Kb::Text);
                }
            }
            st.query = q;
        }
    }
    // Selecting downloads: the bar to delete them takes the tabs' place.
    let (tab_hit, bar_hit) = match &selecting {
        Some(sel) => {
            let live = !sel.is_empty();
            let words = if live { format!("Delete {}", sel.len()) } else { "Delete".into() };
            (None, crate::saved::select_bar(&mut cx, &[(words.as_str(), live, true), ("Delete read", live, false)]))
        }
        None => (kit::tabs(&mut cx, 2), None),
    };
    // Over the page: the menus.
    let mut add_books = false;
    let mut to_notes = false;
    let mut to_stats = false;
    let mut to_wrapped = false;
    let mut new_sort = None;
    let mut new_shelf: Option<(i64, Shelf)> = None;
    let mut dupe_act: Option<DupeAct> = None;
    let mut delete_read_all = false;
    let mut confirmed: Option<Vec<i64>> = None;
    if covered {
        kit::release(&mut cx, held);
        let top = cx.c.inset_top + 56.0;
        if st.add_menu {
            match kit::menu(&mut cx, top, &["Books from this phone", "A series from Search"]) {
                Some(0) => {
                    add_books = true;
                    st.add_menu = false;
                }
                Some(1) => {
                    to_search = true;
                    st.add_menu = false;
                }
                Some(_) => st.add_menu = false,
                None => {}
            }
        } else if st.more_menu {
            // On Downloads, its own two first.
            let items: &[&str] = if tab == DOWNLOADS { &["Select", "Delete read chapters", "Highlights", "Reading time", "Last month's Wrapped"] } else { &["Highlights", "Reading time", "Last month's Wrapped"] };
            let skip = if tab == DOWNLOADS { 2 } else { 0 };
            match kit::menu(&mut cx, top, items) {
                Some(0) if skip == 2 => {
                    start_select = true;
                    st.more_menu = false;
                }
                Some(1) if skip == 2 => {
                    delete_read_all = true;
                    st.more_menu = false;
                }
                Some(i) if i == skip => {
                    to_notes = true;
                    st.more_menu = false;
                }
                Some(i) if i == skip + 1 => {
                    to_stats = true;
                    st.more_menu = false;
                }
                Some(i) if i == skip + 2 => {
                    to_wrapped = true;
                    st.more_menu = false;
                }
                Some(_) => st.more_menu = false,
                None => {}
            }
        } else if st.sort_menu {
            match kit::menu(&mut cx, st.sort_top, &logic::FOLLOWING_SORTS) {
                Some(i) if i < 3 => {
                    new_sort = Some(i as u8);
                    st.sort_menu = false;
                }
                Some(_) => st.sort_menu = false,
                None => {}
            }
        } else if let Some(ids) = st.confirm.clone() {
            match crate::saved::confirm(&mut cx, &app.saver, &ids) {
                Some(true) => {
                    st.confirm = None;
                    confirmed = Some(ids);
                }
                Some(false) => st.confirm = None,
                None => {}
            }
        } else if st.dupes {
            match dupes_sheet(&mut cx, &mut app.images, &pairs, now) {
                Some(Some(a)) => dupe_act = Some(a),
                Some(None) => st.dupes = false,
                None => {}
            }
        } else if let Some(id) = st.shelf_for {
            match series.iter().find(|s| s.id == id) {
                Some(s) => {
                    if let Some(picked) = crate::series::shelf_sheet(&mut cx, &s.title, logic::shelf_of(s)) {
                        st.shelf_for = None;
                        new_shelf = picked.filter(|p| *p != logic::shelf_of(s)).map(|p| (id, p));
                    }
                }
                None => st.shelf_for = None,
            }
        }
    }
    app.page_end(&pg, bottom);
    // Downloads: a long press and drag selects (or lets go of) every row it passes.
    let swept = tab == DOWNLOADS && {
        let band = (app.canvas.inset_top + top_h, app.canvas.h - app.canvas.inset_bottom - crate::saved::BAR_H);
        app.sweep_rows(rows, band, covered, dt, |st| &mut st.space.select)
    };
    // Downloads: what selecting asked for.
    if !covered {
        if stop_select {
            app.st.space.select = None;
        } else if select_all {
            app.st.space.select = Some(groups.iter().map(|g| g.series).collect());
        } else if let (Some(k), Some(sel)) = (bar_hit, &selecting) {
            let ids = saved_chapters(app, |s| sel.contains(&s), k == 1);
            if ids.is_empty() {
                app.toast("None of those chapters is read yet");
            } else {
                app.st.space.confirm = Some(ids);
            }
        }
    }
    if start_select {
        app.st.space.select = Some(BTreeSet::new());
    }
    if delete_read_all {
        let ids = saved_chapters(app, |_| true, true);
        if ids.is_empty() {
            app.toast("No read chapters are on this phone");
        } else {
            app.st.space.confirm = Some(ids);
        }
    }
    if let Some(ids) = confirmed {
        crate::saved::delete(app, &ids);
        app.st.space.select = None;
    }
    if retry_saves {
        app.saver.retry(&[]);
    }
    if cancel_saves {
        let n = app.saver.cancel_all();
        app.toast(&format!("{} stopped", if n == 1 { "1 download".to_string() } else { format!("{n} downloads") }));
    }
    if let Some(s) = new_sort {
        app.set_following_sort(s);
    }
    if let Some((id, sh)) = new_shelf {
        app.set_shelf(id, sh);
    }
    if let Some(all) = set_all
        && all != app.updates_all
    {
        app.updates_all = all;
        app.save_config();
    }
    if retry_updates {
        app.ask(Job::Updates(UPDATE_DAYS));
    }
    if let Some(t_) = set_tab
        && t_ != tab
    {
        app.st.space.tab = t_;
        app.st.space.select = None;
        *app.scroll() = Default::default();
        if t_ == UPDATES && !app.updates.loading {
            app.ask(Job::Updates(UPDATE_DAYS));
        }
        if t_ == LISTS {
            app.want_lists();
        }
    }
    if add_books {
        app.host.pick_books();
    }
    if let Some(a) = list_act.filter(|_| !covered) {
        crate::lists::tab_act(app, a);
        return true;
    }
    match dupe_act {
        Some(DupeAct::Merge(a, b)) => app.merge(a, b),
        Some(DupeAct::NotSame(a, b)) => app.not_same(a, b),
        None => {}
    }
    match act {
        Some(Act::Series(id)) => app.push(Route::Series(id)),
        Some(Act::Continue(id)) => app.continue_series(id),
        Some(Act::Book(id)) => app.open_book(id),
        Some(Act::Bell(id, on)) => app.set_bell(id, on),
        Some(Act::Saved(id)) => app.push(Route::Saved(id)),
        Some(Act::Toggle(id)) => {
            if let Some(sel) = &mut app.st.space.select
                && !sel.remove(&id)
            {
                sel.insert(id);
            }
        }
        Some(Act::Chapter(series, chapter)) => app.open_chapter_of(series, chapter),
        Some(Act::Shelve(id)) => app.st.space.shelf_for = Some(id),
        None => {
            if to_search {
                app.go_tab(1);
            } else if to_notes {
                app.push(Route::Notes);
            } else if to_stats {
                app.push(Route::Stats);
            } else if to_wrapped {
                let (y, m, _) = crate::wrapped_stats::date_of_day(shelf::day_of(shelf::now_ms()));
                crate::wrapped::open(app, crate::wrapped_stats::Period::month(y, m).before());
            } else if let Some(i) = tab_hit {
                app.go_tab(i);
            }
        }
    }
    if tab == UPDATES && updates_state.0 {
        app.wake_at(now + 50_000_000);
    }
    pg.moving || swept || (app.library.loading || app.updates.loading) && !crate::wake::has()
}

fn now_ms_() -> i64 {
    shelf::now_ms()
}

/// The chapters on this phone of the series `of` picks (only the read ones when `read`).
fn saved_chapters(app: &App, of: impl Fn(i64) -> bool, read: bool) -> Vec<i64> {
    let mut ids = Vec::new();
    for g in app.saver.groups().into_iter().filter(|g| of(g.series)) {
        for m in app.saver.saved_of(g.series) {
            if !read || crate::saved::is_read(app, &m) {
                ids.push(m.chapter.id);
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_shortly() {
        assert_eq!(size_words(0), "0 MB");
        assert_eq!(size_words(400_000), "under 1 MB");
        assert_eq!(size_words(61_200_000), "61 MB");
        assert_eq!(size_words(1_340_000_000), "1.3 GB");
    }
}
