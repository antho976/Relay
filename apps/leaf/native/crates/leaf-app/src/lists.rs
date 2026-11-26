//! Your own lists (the owner, 2026-11-26: "album/category creation where I can put names and say
//! this is the wait list for the next series"). A list has a name and series in your order; it
//! may show on Home as a rail of its own, under Continue reading, and every list is in My space's
//! Lists tab whether on Home or not.
//!
//! - **Making one**: My space › Lists › New list, or "Add to list…" on a series' ⋯ (a search
//!   result or a pick too: it is added to your library first, and nothing more: not followed, not
//!   in Recent until read).
//! - **A list's page** ([`draw`]): its series with where you are in each, Add series (a picker of
//!   your library, [`draw_add`]), and ⋯: Show on Home or Remove from Home, Rename, Delete list. A
//!   long press on a series: Move to top, Remove from this list.
//! - **On Home** ([`rail`]): the list's name opens its page; a long press on it offers Remove from
//!   Home, and on a cover, that series' own two.
//!
//! The server keeps them (`/v1/lists`, `leaf-server/src/lists.rs`); an edit shows here at once and
//! is sent whole ([`App::edit_lists`]). The sheets are drawn over any page ([`draw_sheet`]).

use std::collections::HashMap;

use leaf_proto::{List, Series};

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit::{self, Ctx, Kb};
use crate::logic;
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};
use crate::ui::HScroll;

/// The sheets of lists, over whatever page opened them.
#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    /// Which lists a series is on (its id, its title); "New list" last.
    Pick(i64, String),
    /// A list's name being typed: a new one (with a series to put on it), or one renamed.
    Name { rename: Option<i64>, add: Option<i64> },
    /// A list's ⋯.
    Menu(i64),
    /// Delete this list?
    Delete(i64),
    /// A list's rail on Home, held: open it, or take it off Home.
    Home(i64),
    /// A series on a list, held (list, series).
    Row(i64, i64),
}

#[derive(Default)]
pub struct State {
    pub sheet: Option<Sheet>,
    /// The name being typed.
    pub name: String,
    /// What the picker is narrowed to.
    pub query: String,
    /// Home's rails, by list.
    pub rails: HashMap<i64, HScroll>,
}

/// The field a list's name is typed in.
const NAME_FIELD: &str = "list-name";
/// A row of a list's page, and of the Lists tab: a square cover as tall.
const ROW_H: f32 = crate::space::ROW_H;
/// A row of the picker.
const PICK_H: f32 = 64.0;

/// Open the name sheet with the keyboard up: for a new list (with `add` to put on it) or to rename.
pub fn ask_name(app: &mut App, rename: Option<i64>, add: Option<i64>) {
    let name = rename.and_then(|id| app.lists.data.iter().flatten().find(|l| l.id == id)).map(|l| l.name.clone()).unwrap_or_default();
    app.st.lists.name = name.clone();
    app.st.lists.sheet = Some(Sheet::Name { rename, add });
    app.focus.id = Some(NAME_FIELD);
    app.focus.text = name.clone();
    app.focus.kind = Kb::Text;
    app.host.keyboard(true, &name, Kb::Text);
}

/// "Add to list…" for a series (a preview's id too): the lists with it checked, or straight to
/// naming one when there are none yet.
pub fn pick(app: &mut App, series: i64, title: &str) {
    app.want_lists();
    if app.lists.data.as_ref().is_some_and(|v| v.is_empty()) {
        ask_name(app, None, Some(series));
    } else {
        app.st.lists.sheet = Some(Sheet::Pick(series, title.to_string()));
    }
}

/// The series of `l` that are in your library, in its order.
pub fn series_of<'a>(l: &List, library: &'a [Series]) -> Vec<&'a Series> {
    l.series.iter().filter_map(|id| library.iter().find(|s| s.id == *id)).collect()
}

/// "4 series", "1 series · On Home".
pub fn count_words(n: usize, home: bool) -> String {
    let n = if n == 1 { "1 series".to_string() } else { format!("{n} series") };
    if home { format!("{n} · On Home") } else { n }
}

/// Where you are in a series, short: "Ch. 12 · 30 to go", "Read", or "120 chapters" when not
/// started.
pub fn where_words(s: &Series, d: Option<&leaf_proto::SeriesDetail>) -> String {
    if logic::finished(s, d) {
        "Read".into()
    } else if s.read_upto.is_some() {
        let at = logic::at_chapter(s, d).map_or("Ch. 1".into(), |n| format!("Ch. {}", logic::num(n)));
        format!("{at} · {} to go", logic::to_go(s, d))
    } else if s.chapters == 0 {
        "No chapters yet".into()
    } else {
        logic::chapters_words(s.chapters as usize)
    }
}

/// A list's picture: up to four of its covers, two by two (one fills it alone).
#[allow(clippy::too_many_arguments)]
pub fn collage(cx: &mut Ctx, im: &mut crate::images::Images, ids: &[i64], x: f32, y: f32, side: f32, r: f32, now: u64) {
    cx.c.rect(x, y, side, side, r, t::S1);
    let shown: Vec<i64> = ids.iter().take(4).copied().collect();
    if shown.is_empty() {
        cx.c.icon("list", x + side / 2.0, y + side / 2.0, side * 0.3, t::INK3);
        return;
    }
    let cells: Vec<(f32, f32, f32, f32)> = if shown.len() < 4 {
        vec![(x, y, side, side)]
    } else {
        let h = side / 2.0;
        vec![(x, y, h, h), (x + h, y, h, h), (x, y + h, h, h), (x + h, y + h, h, h)]
    };
    for (id, (cx_, cy, w, h)) in shown.iter().zip(cells) {
        let of = CoverOf::Series(*id);
        if im.cover(cx.c, of, now) {
            cx.c.image_cover(&of.key(), cx_, cy, w, h, if shown.len() < 4 { r } else { 0.0 }, Rgba::WHITE);
        }
    }
}

fn held_on(held: Option<(f32, f32)>, y: f32, h: f32, dy: f32) -> bool {
    held.is_some_and(|(_, hy)| hy >= y + dy && hy < y + h + dy)
}

/// What the Lists tab asks for.
pub enum TabAct {
    Open(i64),
    Home(i64, bool),
    New,
}

/// My space's Lists tab, from `y`: how many, New list, then a row per list (its covers, name, how
/// many and whether on Home, the first titles, and On Home at the right). Returns where it ends.
#[allow(clippy::too_many_arguments)]
pub fn tab(cx: &mut Ctx, im: &mut crate::images::Images, lists: &Loaded, library: &[Series], query: &str, mut y: f32, now: u64, act: &mut Option<TabAct>) -> f32 {
    let x0 = PAD;
    let w = WIDTH - 2.0 * PAD;
    let all = lists.0.unwrap_or(&[]);
    let shown: Vec<&List> = all.iter().filter(|l| logic::title_matches(&l.name, query) || series_of(l, library).iter().any(|s| logic::title_matches(&s.title, query))).collect();
    y += 10.0;
    let head_h = 40.0;
    let ht = Text::new(ty::UI, t::INK2);
    let words = match (all.len(), query.trim().is_empty()) {
        (1, true) => "1 list".to_string(),
        (n, true) => format!("{n} lists"),
        (n, false) => format!("{} of {n} found", shown.len()),
    };
    cx.c.text(&words, x0, y + (head_h - ht.lh()) / 2.0, ht);
    if !all.is_empty() {
        let (_, tapped) = kit::pill(cx, WIDTH - PAD, y + (head_h - 32.0) / 2.0, "New list", false);
        if tapped {
            *act = Some(TabAct::New);
        }
    }
    y += head_h + 6.0;
    if all.is_empty() {
        y += 30.0;
        if lists.1 {
            let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
            cx.c.arc(WIDTH / 2.0, y + 10.0, 9.0, 2.0, turn, turn + 0.7, t::INK3, false);
            cx.c.text("Asking your server", WIDTH / 2.0, y + 32.0, Text::new(ty::UI, t::INK2).center());
            return y + 60.0;
        }
        let (h, new) = kit::notice(cx, x0 + 16.0, y, w - 32.0, "No lists yet", Some("Make one for what to read next, a wait list, anything you like. A list can show on Home too."), Some("New list"));
        if new {
            *act = Some(TabAct::New);
        }
        return y + h;
    }
    let rh = ROW_H;
    for l in shown {
        let on = series_of(l, library);
        if cx.pressed(0.0, y, WIDTH, rh) {
            cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
        }
        let ids: Vec<i64> = on.iter().map(|s| s.id).collect();
        collage(cx, im, &ids, 0.0, y, rh, 0.0, now);
        let tx = rh + 14.0;
        let (pw, home_tap) = kit::pill(cx, WIDTH - PAD, y + (rh - 32.0) / 2.0, if l.home { "On Home" } else { "Show on Home" }, l.home);
        let tw = WIDTH - PAD - pw - 12.0 - tx;
        let title_t = Text::new(15.5, t::INK).semi();
        let line_t = Text::new(13.0, t::INK2);
        let last_t = Text::new(12.0, t::INK3);
        let block = title_t.lh() + 3.0 + line_t.lh() + 2.0 + last_t.lh();
        let ty0 = y + (rh - block) / 2.0;
        cx.c.text_fit(&l.name, tx, ty0, tw, title_t);
        cx.c.text_fit(&count_words(on.len(), false), tx, ty0 + title_t.lh() + 3.0, tw, line_t);
        let titles = if on.is_empty() { "Nothing on it yet".to_string() } else { on.iter().map(|s| s.title.as_str()).collect::<Vec<_>>().join(", ") };
        cx.c.text_fit(&titles, tx, ty0 + title_t.lh() + 3.0 + line_t.lh() + 2.0, tw, last_t);
        if home_tap {
            *act = Some(TabAct::Home(l.id, !l.home));
        } else if cx.tap(0.0, y, WIDTH, rh) {
            *act = Some(TabAct::Open(l.id));
        }
        y += rh;
    }
    y
}

/// The lists (when known) and whether they are being asked for.
pub type Loaded<'a> = (Option<&'a [List]>, bool);

/// A list's page.
pub fn draw(app: &mut App, id: i64, dt: f32) -> bool {
    if app.fresh {
        app.want_lists();
        if app.library.data.is_none() && !app.library.loading {
            app.ask(Job::Library);
        }
    }
    let list = app.lists.data.iter().flatten().find(|l| l.id == id).cloned();
    // Deleted (here, or on the server): back to where it was opened from.
    if list.is_none() && app.lists.data.is_some() && !app.lists.loading {
        app.pop();
        return true;
    }
    let library: Vec<Series> = app.library.data.clone().unwrap_or_default();
    let on: Vec<Series> = list.as_ref().map(|l| series_of(l, &library).into_iter().cloned().collect()).unwrap_or_default();
    for s in &on {
        if !app.series.contains_key(&s.id) {
            app.ask(Job::Series(s.id));
        }
    }
    let now = app.now_ns;
    let now_ms = crate::shelf::now_ms();
    let top_h = 60.0;
    let pg = app.page_begin(dt, top_h);
    let mut cx = cx!(app);
    let (x0, w) = (PAD, WIDTH - 2.0 * PAD);
    let mut y = pg.top + 8.0;
    let mut open: Option<i64> = None;
    let mut go: Option<i64> = None;
    let mut add = false;
    let mut held_row: Option<i64> = None;
    let held_at = cx.ui.resting(now).filter(|(_, ns)| *ns >= crate::series::LONG_PRESS_NS).map(|(p, _)| p);
    if let Some(l) = &list {
        let head_h = 40.0;
        let ht = Text::new(ty::UI, t::INK2);
        cx.c.text(&count_words(on.len(), l.home), x0, y + (head_h - ht.lh()) / 2.0, ht);
        if !on.is_empty() {
            let (_, tapped) = kit::pill(&mut cx, WIDTH - PAD, y + (head_h - 32.0) / 2.0, "Add series", false);
            add = tapped;
        }
        y += head_h + 6.0;
        if on.is_empty() {
            y += 30.0;
            let (h, tapped) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Nothing on this list yet", Some("Add series from your library here, or from any series' page: ⋯ › Add to list."), Some("Add series"));
            add |= tapped;
            y += h;
        }
        let rh = ROW_H;
        let (view_top, view_bottom) = (-cx.c.dy - 120.0, -cx.c.dy + cx.c.h + 120.0);
        for s in &on {
            if y + rh < view_top || y > view_bottom {
                y += rh;
                continue;
            }
            let d = app.series.get(&s.id).and_then(|l| l.data.as_ref());
            if cx.pressed(0.0, y, WIDTH, rh) {
                cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
            }
            collage(&mut cx, &mut app.images, &[s.id], 0.0, y, rh, 0.0, now);
            let tx = rh + 14.0;
            let done = logic::finished(s, d);
            let key = if done {
                "Open"
            } else if s.read_upto.is_some() || s.last_read_at > 0 {
                "Continue"
            } else {
                "Start"
            };
            let (kw, key_tap) = kit::pill(&mut cx, WIDTH - PAD, y + (rh - 32.0) / 2.0, key, false);
            let tw = WIDTH - PAD - kw - 12.0 - tx;
            let title_t = Text::new(15.5, t::INK).semi();
            let line_t = Text::new(13.0, t::INK2);
            let last_t = Text::new(12.0, t::INK3);
            let block = title_t.lh() + 3.0 + line_t.lh() + 2.0 + last_t.lh();
            let ty0 = y + (rh - block) / 2.0;
            cx.c.text_fit(&s.title, tx, ty0, tw, title_t);
            let line = if done { "Read".to_string() } else if s.read_upto.is_some() { logic::place_words(s, d) } else { "Not started".to_string() };
            cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 3.0, tw, line_t);
            let last = if s.last_read_at > 0 {
                format!("Last read {}", logic::ago(s.last_read_at, now_ms))
            } else {
                let mut parts = vec![logic::chapters_words(s.chapters as usize)];
                parts.extend(logic::status_word(&s.status));
                parts.join(" · ")
            };
            cx.c.text_fit(&last, tx, ty0 + title_t.lh() + 3.0 + line_t.lh() + 2.0, tw, last_t);
            if held_on(held_at, y, rh, cx.c.dy) {
                cx.ui.swallow();
                held_row = Some(s.id);
            } else if key_tap {
                go = Some(s.id);
            } else if cx.tap(0.0, y, WIDTH, rh) {
                open = Some(s.id);
            }
            y += rh;
        }
    } else {
        y += 30.0;
        let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
        cx.c.arc(WIDTH / 2.0, y + 10.0, 9.0, 2.0, turn, turn + 0.7, t::INK3, false);
        cx.c.text("Asking your server", WIDTH / 2.0, y + 32.0, Text::new(ty::UI, t::INK2).center());
        y += 60.0;
    }
    // The fixed top: Back, the name, ⋯.
    let (back, more) = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        kit::fixed_top(&mut cx, top_h);
        let name = list.as_ref().map_or("List", |l| l.name.as_str());
        let (b, key, _) = kit::top_bar(&mut cx, top + 8.0, name, if list.is_some() { &["more"] } else { &[] });
        (b, key == Some(0))
    };
    app.page_end(&pg, y + 32.0);
    if back {
        app.pop();
    } else if more {
        app.st.lists.sheet = Some(Sheet::Menu(id));
    } else if let Some(s) = held_row {
        app.st.lists.sheet = Some(Sheet::Row(id, s));
    } else if add {
        app.st.lists.query.clear();
        app.push(Route::ListAdd(id));
    } else if let Some(s) = go {
        app.continue_series(s);
    } else if let Some(s) = open {
        app.push(Route::Series(s));
    }
    pg.moving || app.lists.loading
}

/// Series put on a list from your library: a field to narrow them, then each with a check; a tap
/// puts it on or takes it off at once. Not in your library: Search, and a result's ⋯.
pub fn draw_add(app: &mut App, id: i64, dt: f32) -> bool {
    if app.fresh && app.library.data.is_none() && !app.library.loading {
        app.ask(Job::Library);
    }
    let Some(list) = app.lists.data.iter().flatten().find(|l| l.id == id).cloned() else {
        app.pop();
        return true;
    };
    let query = app.st.lists.query.clone();
    let mut library: Vec<Series> = app.library.data.clone().unwrap_or_default();
    library.retain(|s| logic::title_matches(&s.title, &query));
    // What you read last first, then the rest by title.
    library.sort_by(|a, b| b.last_read_at.cmp(&a.last_read_at).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    let now = app.now_ns;
    let top_h = 60.0 + 54.0;
    let pg = app.page_begin(dt, top_h);
    let mut cx = cx!(app);
    let mut y = pg.top + 6.0;
    let mut toggle: Option<i64> = None;
    let mut to_search = false;
    let rh = PICK_H;
    let (view_top, view_bottom) = (-cx.c.dy - 80.0, -cx.c.dy + cx.c.h + 80.0);
    if library.is_empty() && !query.trim().is_empty() {
        y += 24.0;
        let (h, _) = kit::notice(&mut cx, PAD + 16.0, y, WIDTH - 2.0 * PAD - 32.0, &format!("Nothing in your library with \u{201C}{}\u{201D}", query.trim()), None, None);
        y += h + 12.0;
    }
    for s in &library {
        if y + rh < view_top || y > view_bottom {
            y += rh;
            continue;
        }
        let on = list.series.contains(&s.id);
        if cx.pressed(0.0, y, WIDTH, rh) {
            cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
        }
        collage(&mut cx, &mut app.images, &[s.id], PAD, y + 8.0, rh - 16.0, 6.0, now);
        let tx = PAD + rh - 16.0 + 12.0;
        let (cxx, cyy) = (WIDTH - PAD - 11.0, y + rh / 2.0);
        if on {
            cx.c.circle(cxx, cyy, 11.0, t::ACCENT);
            cx.c.icon("check", cxx, cyy, 13.0, t::ON_ACCENT);
        } else {
            cx.c.outline(cxx - 11.0, cyy - 11.0, 22.0, 22.0, 11.0, 1.5, t::INK3);
        }
        let tw = WIDTH - PAD - 34.0 - tx;
        let title_t = Text::new(14.5, t::INK).semi();
        let line_t = Text::new(12.5, t::INK3);
        let ty0 = y + (rh - title_t.lh() - 3.0 - line_t.lh()) / 2.0;
        cx.c.text_fit(&s.title, tx, ty0, tw, title_t);
        let d = app.series.get(&s.id).and_then(|l| l.data.as_ref());
        cx.c.text_fit(&where_words(s, d), tx, ty0 + title_t.lh() + 3.0, tw, line_t);
        if cx.tap(0.0, y, WIDTH, rh) {
            toggle = Some(s.id);
        }
        y += rh;
    }
    y += 20.0;
    let bt = Text::new(ty::UI, t::INK3);
    y += cx.c.para("Not in your library? Find it in Search, then ⋯ › Add to list on its page.", PAD, y, WIDTH - 2.0 * PAD, bt.line(1.4), 3) + 12.0;
    if kit::plain(&mut cx, PAD, y, WIDTH - 2.0 * PAD, 46.0, "Search your sources", Some("search")) {
        to_search = true;
    }
    y += 46.0;
    // The fixed top: Back and the list's name, then the field.
    let back = {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + top_h, 0.0, t::BG);
        kit::fixed_top(&mut cx, top_h);
        let (b, _, _) = kit::top_bar(&mut cx, top + 8.0, &format!("Add to {}", list.name), &[]);
        let (fx, fy, fw, fh) = (PAD, top + 60.0 + 6.0, WIDTH - 2.0 * PAD, 40.0);
        cx.c.rect(fx, fy, fw, fh, t::KEY_R, t::S1);
        cx.c.icon("search", fx + 18.0, fy + fh / 2.0, 15.0, t::INK3);
        let st = &mut app.st.lists;
        let mut q = st.query.clone();
        let clear_w = if q.is_empty() { 0.0 } else { 40.0 };
        kit::inline(&mut cx, "list-filter", fx + 36.0, fy, fw - 36.0 - 14.0 - clear_w, fh, "Find in your library", &mut q, Kb::Text, Text::new(ty::UI, t::INK));
        if !q.is_empty() && kit::icon_key_ink(&mut cx, fx + fw - 42.0, fy - 2.0, "close", t::INK2) {
            q.clear();
            if cx.focus.is("list-filter") {
                cx.focus.text.clear();
                cx.host.keyboard(true, "", Kb::Text);
            }
        }
        st.query = q;
        b
    };
    app.page_end(&pg, y + 32.0);
    if back {
        app.finish_typing();
        app.pop();
    } else if to_search {
        app.finish_typing();
        app.go_tab(1);
    } else if let Some(s) = toggle {
        app.toggle_in_list(id, s);
    }
    pg.moving
}

/// What Home's list rails ask for.
pub enum RailAct {
    Open(i64),
    Series(i64),
    Add(i64),
    Sheet(Sheet),
}

/// A list on Home, from `y`: its name (a tap opens it, a long press offers Remove from Home), then
/// its covers in a row as Continue reading's, each with where you are; an empty list keeps one
/// cell, Add series. Returns its height.
#[allow(clippy::too_many_arguments)]
pub fn rail(
    cx: &mut Ctx,
    im: &mut crate::images::Images,
    hs: &mut HScroll,
    l: &List,
    on: &[(Series, String)],
    y0: f32,
    dt: f32,
    now: u64,
    covered: bool,
    act: &mut Option<RailAct>,
) -> (f32, bool) {
    let mut y = y0;
    let held_at = if covered { None } else { cx.ui.resting(now).filter(|(_, ns)| *ns >= crate::series::LONG_PRESS_NS).map(|(p, _)| p) };
    let head_t = Text::new(18.0, t::INK).semi();
    cx.c.text_fit(&l.name, PAD, y, WIDTH - 2.0 * PAD - 28.0, head_t);
    cx.c.icon("chevron-right", WIDTH - PAD - 8.0, y + head_t.lh() / 2.0, 18.0, t::INK2);
    let hh = head_t.lh();
    if held_on(held_at, y - 10.0, hh + 20.0, cx.c.dy) {
        cx.ui.swallow();
        *act = Some(RailAct::Sheet(Sheet::Home(l.id)));
    } else if !covered && cx.tap(0.0, y - 10.0, WIDTH, hh + 20.0) {
        *act = Some(RailAct::Open(l.id));
    }
    y += hh + 14.0;
    let side = 128.0;
    let title_t = Text::new(13.5, t::INK).semi();
    let line_t = Text::new(12.5, t::INK3);
    let cell_h = side + 8.0 + title_t.lh() + 3.0 + line_t.lh();
    let area = [0.0, y + cx.c.dy, WIDTH, cell_h];
    cx.ui.claim_h(area);
    let moving = hs.update(cx.ui, area, dt);
    let mut x = PAD - hs.off;
    if on.is_empty() {
        cx.c.outline(x, y, side, side, COVER_R, 1.0, t::INK3.a(0.5));
        cx.c.icon("plus", x + side / 2.0, y + side / 2.0, 22.0, t::INK2);
        cx.c.text_fit("Add series", x, y + side + 8.0, side, title_t);
        cx.c.text_fit("Nothing on it yet", x, y + side + 8.0 + title_t.lh() + 3.0, side, line_t);
        if !covered && cx.tap(x, y, side, cell_h) {
            *act = Some(RailAct::Add(l.id));
        }
        x += side + 10.0;
    }
    for (s, words) in on {
        if x + side > -10.0 && x < WIDTH + 10.0 {
            collage(cx, im, &[s.id], x, y, side, COVER_R, now);
            if cx.pressed(x, y, side, cell_h) {
                cx.c.rect(x, y, side, side, COVER_R, t::WASH);
            }
            cx.c.text_fit(&s.title, x, y + side + 8.0, side, title_t);
            cx.c.text_fit(words, x, y + side + 8.0 + title_t.lh() + 3.0, side, line_t);
            let over = held_at.is_some_and(|(hx, _)| hx >= x && hx < x + side);
            if over && held_on(held_at, y, cell_h, cx.c.dy) {
                cx.ui.swallow();
                *act = Some(RailAct::Sheet(Sheet::Row(l.id, s.id)));
            } else if !covered && cx.tap(x, y, side, cell_h) {
                *act = Some(RailAct::Series(s.id));
            }
        }
        x += side + 10.0;
    }
    hs.max = x + hs.off - 10.0 + PAD - WIDTH;
    y += cell_h;
    (y - y0, moving)
}

/// Do what a list rail on Home asked.
pub fn rail_act(app: &mut App, a: RailAct) {
    match a {
        RailAct::Open(id) => app.push(Route::List(id)),
        RailAct::Series(id) => app.push(Route::Series(id)),
        RailAct::Add(id) => {
            app.st.lists.query.clear();
            app.push(Route::ListAdd(id));
        }
        RailAct::Sheet(s) => app.st.lists.sheet = Some(s),
    }
}

/// Do what the Lists tab asked.
pub fn tab_act(app: &mut App, a: TabAct) {
    match a {
        TabAct::Open(id) => app.push(Route::List(id)),
        TabAct::Home(id, on) => set_home(app, id, on),
        TabAct::New => ask_name(app, None, None),
    }
}

fn set_home(app: &mut App, id: i64, on: bool) {
    let mut name = String::new();
    app.edit_lists(|v| {
        if let Some(l) = v.iter_mut().find(|l| l.id == id) {
            l.home = on;
            name = l.name.clone();
        }
    });
    app.toast(&if on { format!("{name} shows on Home") } else { format!("{name} is off Home; it stays in My space") });
}

enum Out {
    Close,
    Toggle(i64, i64),
    NewName(Option<i64>),
    Named(String, Option<i64>, Option<i64>),
    Add(i64),
    Home(i64, bool),
    Rename(i64),
    AskDelete(i64),
    Delete(i64),
    Open(i64),
    Top(i64, i64),
    Off(i64, i64),
}

/// The sheet open, over the page (the page takes no tap meanwhile: [`crate::app`]).
pub fn draw_sheet(app: &mut App) {
    let Some(sheet) = app.st.lists.sheet.clone() else { return };
    let lists: Vec<List> = app.lists.data.clone().unwrap_or_default();
    let name_of = |id: i64| lists.iter().find(|l| l.id == id).map(|l| l.name.clone()).unwrap_or_else(|| "this list".into());
    let typed_done = app.typed_done;
    let title_of = |id: i64| app.series_title(id);
    let mut out: Option<Out> = None;
    match &sheet {
        Sheet::Pick(series, title) => {
            let names: Vec<(String, bool)> = lists.iter().map(|l| (l.name.clone(), l.series.contains(series))).collect();
            let mut items: Vec<(&str, bool)> = names.iter().map(|(n, on)| (n.as_str(), *on)).collect();
            items.push(("New list…", false));
            let mut cx = cx!(app);
            match kit::choices(&mut cx, "Add to a list", Some(title), &items) {
                Some(usize::MAX) => out = Some(Out::Close),
                Some(i) if i < lists.len() => out = Some(Out::Toggle(lists[i].id, *series)),
                Some(_) => out = Some(Out::NewName(Some(*series))),
                None => {}
            }
        }
        Sheet::Name { rename, add } => {
            let mut cx = cx!(app);
            let (top, outside) = kit::sheet(&mut cx, 224.0);
            let x = PAD + 4.0;
            let w = WIDTH - 2.0 * x;
            cx.c.text(if rename.is_some() { "Rename list" } else { "New list" }, x, top + 26.0, Text::new(ty::SECTION, t::INK).semi());
            let st = &mut app.st.lists;
            let fh = kit::field(&mut cx, NAME_FIELD, x, top + 64.0, w, "Name", "Next up, Wait list, Weekend…", &mut st.name, Kb::Text);
            let ready = !st.name.trim().is_empty();
            let words = if rename.is_some() { "Save" } else { "Make list" };
            let save = kit::primary(&mut cx, x, top + 64.0 + fh + 16.0, w, 48.0, words, None);
            cx.ui.tap = None;
            if outside {
                out = Some(Out::Close);
            } else if (save || typed_done == Some(NAME_FIELD)) && ready {
                out = Some(Out::Named(st.name.trim().to_string(), *rename, *add));
            }
        }
        Sheet::Menu(id) => {
            let home = lists.iter().any(|l| l.id == *id && l.home);
            let mut cx = cx!(app);
            let top = cx.c.inset_top + 56.0;
            match kit::menu(&mut cx, top, &["Add series", if home { "Remove from Home" } else { "Show on Home" }, "Rename", "Delete list"]) {
                Some(0) => out = Some(Out::Add(*id)),
                Some(1) => out = Some(Out::Home(*id, !home)),
                Some(2) => out = Some(Out::Rename(*id)),
                Some(3) => out = Some(Out::AskDelete(*id)),
                Some(_) => out = Some(Out::Close),
                None => {}
            }
        }
        Sheet::Delete(id) => {
            let name = name_of(*id);
            let mut cx = cx!(app);
            match kit::confirm(&mut cx, &format!("Delete {name}?"), "The list goes. The series on it stay in your library, and in My space where they were.", "Delete") {
                Some(true) => out = Some(Out::Delete(*id)),
                Some(false) => out = Some(Out::Close),
                None => {}
            }
        }
        Sheet::Home(id) => {
            let name = name_of(*id);
            let mut cx = cx!(app);
            match kit::choices(&mut cx, &name, Some("It stays in My space › Lists"), &[("Open list", false), ("Remove from Home", false)]) {
                Some(0) => out = Some(Out::Open(*id)),
                Some(1) => out = Some(Out::Home(*id, false)),
                Some(_) => out = Some(Out::Close),
                None => {}
            }
        }
        Sheet::Row(list, series) => {
            let (name, title) = (name_of(*list), title_of(*series));
            let mut cx = cx!(app);
            match kit::choices(&mut cx, &title, Some(&format!("On {name}")), &[("Move to top", false), ("Remove from this list", false)]) {
                Some(0) => out = Some(Out::Top(*list, *series)),
                Some(1) => out = Some(Out::Off(*list, *series)),
                Some(_) => out = Some(Out::Close),
                None => {}
            }
        }
    }
    let Some(out) = out else { return };
    app.ui.reset();
    match out {
        Out::Close => close(app),
        Out::Toggle(list, series) => {
            app.toggle_in_list(list, series);
            // A preview is being added: its id changes, so the sheet goes.
            if series < 0 {
                close(app);
            }
        }
        Out::NewName(add) => ask_name(app, None, add),
        Out::Named(name, rename, add) => {
            close(app);
            match rename {
                Some(id) => app.edit_lists(|v| {
                    if let Some(l) = v.iter_mut().find(|l| l.id == id) {
                        l.name = name;
                    }
                }),
                None => {
                    let id = app.new_list(&name, add);
                    // Made from My space: its page, to fill it.
                    if add.is_none() {
                        app.push(Route::List(id));
                    } else if add.is_some_and(|s| s > 0) {
                        app.toast(&format!("On {name}"));
                    }
                }
            }
        }
        Out::Add(id) => {
            close(app);
            app.st.lists.query.clear();
            app.push(Route::ListAdd(id));
        }
        Out::Home(id, on) => {
            close(app);
            set_home(app, id, on);
        }
        Out::Rename(id) => ask_name(app, Some(id), None),
        Out::AskDelete(id) => app.st.lists.sheet = Some(Sheet::Delete(id)),
        Out::Delete(id) => {
            close(app);
            let name = name_of(id);
            app.edit_lists(|v| v.retain(|l| l.id != id));
            app.toast(&format!("{name} deleted"));
        }
        Out::Open(id) => {
            close(app);
            app.push(Route::List(id));
        }
        Out::Top(list, series) => {
            close(app);
            app.edit_lists(|v| {
                if let Some(l) = v.iter_mut().find(|l| l.id == list) {
                    l.series.retain(|s| *s != series);
                    l.series.insert(0, series);
                }
            });
        }
        Out::Off(list, series) => {
            close(app);
            app.toggle_in_list(list, series);
        }
    }
}

/// Close the sheet, and the keyboard with it.
pub fn close(app: &mut App) {
    app.st.lists.sheet = None;
    if app.focus.id == Some(NAME_FIELD) {
        app.focus.id = None;
        app.focus.at = None;
        app.host.keyboard(false, "", Kb::Text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_says_how_many_and_where_it_shows() {
        assert_eq!(count_words(1, false), "1 series");
        assert_eq!(count_words(4, true), "4 series · On Home");
    }
}
