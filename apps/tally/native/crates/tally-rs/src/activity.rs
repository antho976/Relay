//! Entries, one category's or one account's entries, and Search, as the Kotlin app's
//! EntriesScreen.kt and TransactionsScreen.kt, in the premium look (DESIGN.md): every entry of a
//! month by day under its Out / In / Net, with chips for a type or a category; a category or an
//! account with what it reads this month; every month at once by what was typed. A tap on an entry
//! opens it in the editor, where it can also be deleted.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::copy;
use relay_money::model::{AccountType, CategoryKind, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::pace::PaceStatus;
use relay_money::period::BudgetPeriod;

use crate::activity_data::{self, Filter, Listing, Subject};
use crate::activity_kit::{self as ak, W};
use crate::app::App;
use crate::cx;
use crate::entry::Entry;
use crate::entry_logic;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, KeyKind, PAD, ty};
use crate::model;
use crate::nav::Route;
use crate::theme;
use crate::ui::HScroll;
use crate::widgets;

const SEARCH: &str = "activity.search";
const TX_SEARCH: &str = "activity.tx.search";
/// How far back the month switcher reaches.
const MONTHS: i64 = 12;

type EntriesKey = (u64, Date, i64, Filter, Option<i64>);
type SearchKey = (u64, Date, String, Filter);
type TxKey = (u64, Date, Option<i64>, Option<i64>, String);

#[derive(Default)]
pub struct State {
    /// Entries: 0 is the current period, -1 the one before; never ahead of today.
    pub offset: i64,
    pub filter: Filter,
    /// Entries: one category's entries only.
    pub category: Option<i64>,
    entries: Option<(EntriesKey, BudgetPeriod, Listing)>,
    /// The month switcher under the title is open, with the periods it offers.
    months: Option<Vec<(i64, String)>>,
    chips: HScroll,
    pub query: String,
    pub search_filter: Filter,
    search: Option<(SearchKey, Listing)>,
    pub tx_query: String,
    /// Each open category's or account's page's search, by (depth in `app.nav`, category,
    /// account): one page opened over another keeps both, the same subject twice on the stack too.
    tx_queries: HashMap<(usize, Option<i64>, Option<i64>), String>,
    tx: Option<(TxKey, Option<(Subject, Listing)>)>,
    /// The entry an overview shows, read as the editor reads it; `editing` none when it is gone.
    view: Option<((u64, i64), Box<Entry>)>,
    problem: Option<String>,
    /// The reading that failed, so it is not tried again every frame (it is when the ledger
    /// moves, the day turns or what is asked changes).
    entries_failed: Option<EntriesKey>,
    search_failed: Option<SearchKey>,
    tx_failed: Option<TxKey>,
}

enum Act {
    Go(Route),
    Back,
    Offset(i64),
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    match app.route().clone() {
        Route::Entries => entries(app, dt),
        Route::Search => search(app, dt),
        Route::Transactions { category, account } => transactions(app, dt, category, account),
        Route::EntryView { id } => view(app, dt, id),
        _ => false,
    }
}

fn act(app: &mut App, act: Option<Act>) {
    match act {
        Some(Act::Go(r)) => app.push(r),
        Some(Act::Back) => {
            app.pop();
        }
        Some(Act::Offset(o)) => {
            let st = &mut app.st.activity;
            if st.offset != o.min(0) {
                st.offset = o.min(0);
                st.category = None;
                st.chips = HScroll::default();
                *app.scroll() = Default::default();
            }
        }
        None => {}
    }
}

/// Back closes the month switcher when it is open.
pub fn back(app: &mut App) -> bool {
    app.st.activity.months.take().is_some()
}

/// A tapped entry opens its overview; Edit is one tap from there.
fn open_entry(id: i64) -> Act {
    Act::Go(Route::EntryView { id })
}

// Entries

fn entries(app: &mut App, dt: f32) -> bool {
    let today = model::today();
    let st = &mut app.st.activity;
    // Opened from the tab or the drawer it starts on this month with everything; back from an
    // entry it stays on the month and filter it was on.
    if app.fresh && !app.returned {
        st.offset = 0;
        st.filter = Filter::All;
        st.category = None;
        st.months = None;
        st.chips = HScroll::default();
    }
    let key = (app.ledger_gen, today, st.offset, st.filter, st.category);
    if st.entries.as_ref().is_none_or(|(k, _, _)| *k != key) && st.entries_failed.as_ref() != Some(&key) {
        let read = activity_data::period_listing_in(&app.store, today, st.offset, st.filter, st.category);
        // The chosen category left the month (its entry deleted or filed elsewhere): it has no
        // chip to light, so it stops filtering and the month reads whole again.
        let read = match (read, st.category) {
            (Ok((_, l)), Some(c)) if !l.categories.iter().any(|(id, _)| *id == c) => {
                st.category = None;
                activity_data::period_listing_in(&app.store, today, st.offset, st.filter, None)
            }
            (read, _) => read,
        };
        let key = (app.ledger_gen, today, st.offset, st.filter, st.category);
        match read {
            Ok((p, l)) => {
                st.entries = Some((key, p, l));
                st.entries_failed = None;
                st.problem = None;
            }
            Err(e) => {
                // What was listed is not this; the problem shows in its place.
                st.entries = None;
                st.entries_failed = Some(key);
                st.problem = Some(e);
            }
        }
    }
    let fmt = app.store.formatter();
    let pg = app.page_begin(dt);
    let mut todo = None;
    let mut moving = pg.moving;
    let mut open_months = false;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.activity;
        let p = cx.p;
        let held = if st.months.is_some() { widgets::hold(&mut cx) } else { None };
        let mut y = pg.top + 2.0;
        let current = st.offset >= 0;
        let mut switcher_at = y;
        match &st.entries {
            None => {
                let words = st.problem.as_deref().unwrap_or("Reading the ledger");
                y += cx.c.para(words, PAD, y, W, Text::new(ty::UI, p.ink2).line(1.4), 4) + 16.0;
                // A month that cannot be read is not where the page stays.
                if st.entries_failed.is_some() && st.offset != 0 {
                    let t = Text::new(ty::BODY, p.ink).semi();
                    let kw = cx.c.measure("Back to this month", t) + 32.0;
                    if kit::key(&mut cx, PAD, y, kw, 44.0, "Back to this month", None, KeyKind::Plain) {
                        todo = Some(Act::Offset(0));
                    }
                    y += 44.0 + 10.0;
                }
            }
            Some((_, period, l)) => {
                // The month, with a chevron that opens the switcher.
                let title = activity_data::period_label(period, today);
                let tt = Text::new(30.0, p.ink).semi().line(1.15);
                let tw = cx.c.text_fit(&title, PAD, y, W - 40.0, tt);
                cx.c.icon("chevron-down", PAD + tw + 16.0, y + tt.lh() / 2.0 + 1.0, 18.0, p.ink2);
                if cx.tap(PAD - 8.0, y - 6.0, tw + 48.0, tt.lh() + 12.0) {
                    open_months = true;
                }
                y += tt.lh() + 10.0;
                switcher_at = y - 4.0;

                y = reading(&mut cx, y, &l.totals, &fmt) + 10.0;
                // Only over spending: Income or Transfers (or a month with none) would read "$0 a
                // day". The days counted are those with an expense, not any entry.
                if let Some(pd) = activity_data::per_day(l.totals.spent, period, current, today).filter(|_| l.totals.spent > 0) {
                    let mut words = format!("{} a day", fmt.format_whole(pd));
                    let spending_days = l.days.iter().filter(|d| d.rows.iter().any(|r| r.r#type == TxType::Expense)).count();
                    if spending_days > 0 {
                        words += &format!(" · {} with spending", copy::plural(spending_days as i64, "day"));
                    }
                    cx.c.text_fit(&words, PAD, y, W, Text::new(13.0, p.ink3));
                    y += 18.0;
                }
                y += 14.0;

                // A type, or one of the month's categories.
                let mut labels: Vec<&str> = Filter::LABELS.to_vec();
                labels.extend(l.categories.iter().map(|(_, n)| n.as_str()));
                let selected = match st.category {
                    Some(c) => l.categories.iter().position(|(id, _)| *id == c).map(|i| i + Filter::LABELS.len()),
                    None => Some(st.filter.index()),
                };
                let (tapped, m) = ak::chip_row(&mut cx, &mut st.chips, y, &labels, selected, dt);
                moving |= m;
                if let Some(i) = tapped {
                    if i < Filter::LABELS.len() {
                        st.filter = Filter::ALL[i];
                        st.category = None;
                    } else {
                        let id = l.categories[i - Filter::LABELS.len()].0;
                        st.filter = Filter::All;
                        st.category = if st.category == Some(id) { None } else { Some(id) };
                    }
                }
                y += ak::CHIP_H + 8.0;

                let (b, tapped) = ak::days(&mut cx, y, &l.days, today, &fmt, &l.repeats, None);
                y = b;
                if let Some(id) = tapped {
                    todo = Some(open_entry(id));
                }
                if l.days.is_empty() {
                    let (title, body) = match (st.category.is_some() || st.filter != Filter::All, current) {
                        (true, _) => ("Nothing of this kind", "Nothing logged this month matches the chip picked. All shows every entry."),
                        (false, true) => ("Nothing logged this month", "Entries you add, import or that bills post land here."),
                        (false, false) => ("Nothing logged in this month", "Entries you add, import or that bills post land here."),
                    };
                    y += 6.0;
                    y += widgets::empty(&mut cx, PAD, y, W, "list", title, body);
                }
            }
        }
        if st.months.is_some() {
            widgets::release(&mut cx, held);
            if let Some(o) = month_switcher(&mut cx, switcher_at, st) {
                todo = Some(Act::Offset(o));
            }
        }
        bottom = y;
    }
    app.page_end(&pg, bottom);
    if open_months {
        app.st.activity.months = Some(activity_data::recent_periods(&app.store, today, MONTHS));
    }
    act(app, todo);
    moving
}

/// Out, In and Net side by side.
fn reading(cx: &mut Ctx, y: f32, t: &activity_data::Totals, fmt: &MoneyFormatter) -> f32 {
    let p = cx.p;
    let col = W / 3.0;
    let lt = Text::new(12.0, p.ink3).medium();
    let vt = Text::new(17.0, p.ink).medium();
    let cols = [("Out", fmt.format(t.spent), p.ink), ("In", fmt.format(t.income), p.live_text), ("Net", ak::signed(t.income - t.spent, fmt), p.ink)];
    for (i, (label, value, colour)) in cols.iter().enumerate() {
        let x = PAD + col * i as f32;
        cx.c.text(label, x, y, lt);
        cx.c.text_fit(value, x, y + lt.lh() + 2.0, col - 8.0, vt.color(*colour));
    }
    y + lt.lh() + 2.0 + vt.lh()
}

/// The months to switch to, as a card of chips under the title. A tap outside closes it.
/// Returns the offset picked.
fn month_switcher(cx: &mut Ctx, y: f32, st: &mut State) -> Option<i64> {
    let p = cx.p;
    let months = st.months.clone().unwrap_or_default();
    let cols = 3usize;
    let (pad, gap, kh) = (12.0, 8.0, 40.0);
    let rows = months.len().div_ceil(cols).max(1);
    let h = pad * 2.0 + rows as f32 * kh + (rows - 1) as f32 * gap;
    let (x, w) = (PAD, W);
    cx.c.shadow(x, y + 6.0, w, h, kit::CARD_R, 24.0, Rgba::hex(0x000000).a(if p.light { 0.12 } else { 0.5 }));
    cx.c.rect(x, y, w, h, kit::CARD_R, p.raised);
    cx.c.outline(x, y, w, h, kit::CARD_R, 1.0, p.line_subtle);
    let kw = (w - pad * 2.0 - gap * (cols - 1) as f32) / cols as f32;
    let mut out = None;
    for (i, (offset, name)) in months.iter().enumerate() {
        let kx = x + pad + (i % cols) as f32 * (kw + gap);
        let ky = y + pad + (i / cols) as f32 * (kh + gap);
        let on = *offset == st.offset;
        cx.c.rect(kx, ky, kw, kh, 12.0, if on { p.prim } else { p.card2 });
        if !on && cx.pressed(kx, ky, kw, kh) {
            cx.c.rect(kx, ky, kw, kh, 12.0, p.wash);
        }
        let t = Text::new(ty::UI, if on { p.on_ink } else { p.ink }).medium().center();
        cx.c.text_fit(name, kx + kw / 2.0, ky + (kh - t.lh()) / 2.0, kw - 10.0, t);
        if cx.tap(kx, ky, kw, kh) {
            out = Some(*offset);
        }
    }
    // Anything else closes it, and is not passed on.
    if out.is_some() || cx.ui.tap.take().is_some() {
        st.months = None;
    }
    out
}

// Search

fn search(app: &mut App, dt: f32) -> bool {
    let today = model::today();
    let st = &mut app.st.activity;
    // Opened from the tab or the drawer it starts empty (on every kind), with the keyboard; back
    // from a result it keeps what was typed and the chip picked.
    let opened = app.fresh && !app.returned;
    if opened {
        st.query.clear();
        st.search_filter = Filter::All;
    }
    if app.focus.is(SEARCH) {
        st.query = app.focus.text.clone();
    }
    let q = st.query.trim().to_string();
    let key = (app.ledger_gen, today, q.clone(), st.search_filter);
    if !q.is_empty() && st.search.as_ref().is_none_or(|(k, _)| *k != key) && st.search_failed.as_ref() != Some(&key) {
        match activity_data::search_listing(&app.store, today, &q, st.search_filter) {
            Ok(l) => {
                st.search = Some((key, l));
                st.search_failed = None;
                st.problem = None;
            }
            Err(e) => {
                st.search = None;
                st.search_failed = Some(key);
                st.problem = Some(e);
            }
        }
    }
    let fmt = app.store.formatter();
    let pg = app.page_begin(dt);
    let mut todo = None;
    let moving = pg.moving;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.activity;
        let p = cx.p;
        // The field and the chips stay put over the results: they take their own taps first.
        let head_h = 44.0 + 14.0 + ak::CHIP_H + 14.0;
        let head_bottom = pg.top + head_h;
        let head_tap = cx.ui.tap.filter(|&(_, ty)| ty < head_bottom);
        if head_tap.is_some() {
            cx.ui.tap = None;
        }
        let mut y = head_bottom + 2.0;
        let searching = !st.query.trim().is_empty();
        let failed = searching && st.search.is_none() && st.search_failed.is_some();
        match (&st.search, searching) {
            (None, true) if failed => {
                let words = st.problem.as_deref().unwrap_or("The search could not be read");
                y += 6.0;
                y += cx.c.para(words, PAD, y, W, Text::new(ty::UI, p.ink2).line(1.4), 4) + 12.0;
            }
            (Some((_, l)), true) => {
                let t = &l.totals;
                let lt = Text::new(13.0, p.ink2);
                cx.c.text(&copy::plural_as(t.count as i64, "entry", "entries"), PAD, y, lt);
                let total = if t.spent > 0 && t.income > 0 {
                    format!("{} out · +{} in", fmt.format(t.spent), fmt.format(t.income))
                } else if t.income > 0 {
                    format!("+{} total", fmt.format(t.income))
                } else if t.spent > 0 {
                    format!("{} total", fmt.format(t.spent))
                } else {
                    String::new()
                };
                cx.c.text(&total, PAD + W, y, lt.right());
                y += lt.lh() + 10.0;
                if !l.days.is_empty() {
                    let rows = l.days.iter().flat_map(|d| &d.rows);
                    let (b, tapped) = ak::row_card(&mut cx, y, rows, &fmt, |t| ak::short_day(&t.date, today));
                    y = b;
                    if let Some(id) = tapped {
                        todo = Some(open_entry(id));
                    }
                }
                if l.truncated {
                    let words = format!("Only the newest {} are listed. Narrow the search to see older ones.", activity_data::SEARCH_LIMIT);
                    y += 8.0;
                    y += cx.c.para(&words, PAD, y, W, Text::new(ty::CAPTION, p.ink3).line(1.4), 3) + 12.0;
                }
                if l.days.is_empty() {
                    y += 6.0;
                    y += widgets::empty(&mut cx, PAD, y, W, "search", "Nothing matches", "Search reads notes, categories and accounts, across every month.");
                }
            }
            _ => {
                y += 6.0;
                y += widgets::empty(&mut cx, PAD, y, W, "search", "Search every month", "Type a note, a payee, a category or an account. Results come from every month at once.");
            }
        }
        bottom = y;

        // The pinned head, over what scrolls.
        if head_tap.is_some() {
            cx.ui.tap = head_tap;
        }
        let dy = cx.c.dy;
        cx.c.dy = 0.0;
        let ground = p.ground[0];
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, head_bottom, 0.0, ground);
        cx.c.rect_grad(-1000.0, head_bottom, WIDTH + 2000.0, 12.0, 0.0, ground, ground.a(0.0));
        let y = pg.top;
        if opened {
            ak::focus_field(&mut cx, SEARCH, &st.query);
        }
        kit::menu_key(&mut cx, 10.0, y);
        let fx = 10.0 + 44.0 + 4.0;
        let active = cx.focus.is(SEARCH) || !st.query.is_empty();
        let ct = Text::new(15.0, p.ink).medium().right();
        let cw = if active { cx.c.measure("Cancel", ct) + 14.0 } else { 0.0 };
        ak::search_box(&mut cx, SEARCH, fx, y, PAD + W - cw - fx, "A note, a payee, a category", &mut st.query);
        if active {
            cx.c.text("Cancel", PAD + W, y + (44.0 - ct.lh()) / 2.0, ct);
            if cx.tap(PAD + W - cw + 4.0, y, cw + 12.0, 44.0) {
                st.query.clear();
                ak::drop_focus(&mut cx);
            }
        }
        let labels = ["Everything", "Spent", "Income", "Transfers"];
        if let Some(i) = ak::chips_fit(&mut cx, y + 44.0 + 14.0, &labels, Some(st.search_filter.index())) {
            st.search_filter = Filter::ALL[i];
        }
        cx.c.dy = dy;
    }
    app.page_end(&pg, bottom);
    act(app, todo);
    moving
}

// One category's or one account's entries

fn transactions(app: &mut App, dt: f32, category: Option<i64>, account: Option<i64>) -> bool {
    let today = model::today();
    let depth = app.nav.len() - 1;
    let st = &mut app.st.activity;
    // A new subject starts with no search; back to one (from one of its entries, or past another
    // category's or account's page) its own search is back.
    if app.fresh {
        // Searches of pages no longer open are let go.
        st.tx_queries.retain(|(d, c, a), _| matches!(app.nav.get(*d), Some((r, _)) if *r == Route::Transactions { category: *c, account: *a }));
        if !app.returned {
            st.tx_queries.remove(&(depth, category, account));
        }
        st.tx_query = st.tx_queries.get(&(depth, category, account)).cloned().unwrap_or_default();
    }
    if app.focus.is(TX_SEARCH) {
        st.tx_query = app.focus.text.clone();
    }
    let q = st.tx_query.trim().to_string();
    let key = (app.ledger_gen, today, category, account, q.clone());
    if st.tx.as_ref().is_none_or(|(k, _)| *k != key) && st.tx_failed.as_ref() != Some(&key) {
        match activity_data::subject_listing(&app.store, today, category, account, &q) {
            Ok(v) => {
                st.tx = Some((key, v));
                st.tx_failed = None;
                st.problem = None;
            }
            Err(e) => {
                st.tx = None;
                st.tx_failed = Some(key);
                st.problem = Some(e);
            }
        }
    }
    let fmt = app.store.formatter();
    let period = activity_data::period_at(&app.store, today, 0);
    let pg = app.page_begin(dt);
    let mut todo = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.activity;
        let p = cx.p;
        let mut y = pg.top;
        let subject = st.tx.as_ref().and_then(|(_, v)| v.as_ref());
        let name = subject.map(|(s, _)| s.name.as_str()).unwrap_or("");
        // Two keys on the right (edit, then add), so the title keeps clear of both.
        let tt = Text::new(17.0, p.ink).semi();
        let shown = cx.c.fit(name, WIDTH - 2.0 * 112.0, tt);
        let (back, add, h) = widgets::page_header(&mut cx, y, &shown, subject.map(|_| "plus"));
        let edit = subject.is_some() && kit::round_key(&mut cx, WIDTH - 10.0 - 44.0 - 46.0, y, "edit");
        y += h;
        if back {
            todo = Some(Act::Back);
        }
        match subject {
            None => {
                if st.tx.is_some() {
                    y += widgets::empty(&mut cx, PAD, y, W, "list", "This is gone", "It was deleted, here or on another of your devices.");
                } else if let Some(words) = st.problem.as_deref() {
                    y += cx.c.para(words, PAD, y + 8.0, W, Text::new(ty::UI, p.ink2).line(1.4), 4) + 20.0;
                }
            }
            Some((s, l)) => {
                if edit {
                    todo = Some(Act::Go(match (s.account_type, account) {
                        (Some(_), Some(id)) => Route::AccountEdit { id: Some(id), kind: None },
                        _ => Route::CategoryEdit { id: category, kind: s.kind },
                    }));
                }
                if add {
                    todo = Some(Act::Go(if s.is_account() {
                        Route::Entry { id: None, kind: None, account, category: None }
                    } else {
                        let kind = if s.kind == CategoryKind::Income { TxType::Income } else { TxType::Expense };
                        Route::Entry { id: None, kind: Some(kind), account: None, category }
                    }));
                }
                let (b, budget) = hero(&mut cx, y + 4.0, s, &period, &fmt);
                y = b + 12.0;
                if budget {
                    todo = Some(Act::Go(Route::BudgetEdit { category }));
                }
                y = ak::stat_grid(&mut cx, y, &stats(s, &fmt)) + 30.0;

                let (hh, _) = kit::section(&mut cx, y, "Entries", None);
                let count = copy::plural_as(s.entry_count as i64, "entry", "entries");
                let ct = Text::new(13.0, p.ink3).right();
                cx.c.text(&count, PAD + W, y + (hh - ct.lh()) / 2.0, ct);
                y += hh + 12.0;
                y += ak::search_box(&mut cx, TX_SEARCH, PAD, y, W, "Search these entries", &mut st.tx_query) + 6.0;
                // On an account's own page its transfers are signed and count in each day's net.
                let on = if s.is_account() { account } else { None };
                let (b, tapped) = ak::days(&mut cx, y, &l.days, today, &fmt, &l.repeats, on);
                y = b;
                if let Some(id) = tapped {
                    todo = Some(open_entry(id));
                }
                // The page lists at most SEARCH_LIMIT entries; say so rather than look complete.
                if l.truncated {
                    let words = format!("Only the newest {} are listed. Search these entries to see older ones.", activity_data::SEARCH_LIMIT);
                    y += 8.0;
                    y += cx.c.para(&words, PAD, y, W, Text::new(ty::CAPTION, p.ink3).line(1.4), 3) + 12.0;
                }
                if l.days.is_empty() {
                    let searching = !st.tx_query.trim().is_empty();
                    let (title, body) = if searching {
                        ("Nothing matches", "Search reads notes, categories and accounts.")
                    } else {
                        ("No entries yet", "Entries logged here land on this page.")
                    };
                    y += 6.0;
                    y += widgets::empty(&mut cx, PAD, y, W, "list", title, body);
                }
            }
        }
        bottom = y;
        // Kept for this subject, so it is still there when Back returns here past another one.
        st.tx_queries.insert((depth, category, account), st.tx_query.clone());
    }
    app.page_end(&pg, bottom);
    act(app, todo);
    pg.moving
}

/// The card on top: what the category or account reads this month. Returns its bottom and whether
/// "Set a budget" was tapped.
fn hero(cx: &mut Ctx, y: f32, s: &Subject, period: &BudgetPeriod, fmt: &MoneyFormatter) -> (f32, bool) {
    let p = cx.p;
    let tint = if s.is_account() { ak::money_hue() } else { theme::category_color(s.color) };
    let holds = s.is_account() && s.tracks_balance;
    let r = s.reading.as_ref();
    let no_budget = !s.is_account() && r.is_none() && s.kind == CategoryKind::Expense;
    let pad = 20.0;
    let fig = 34.0;
    let head = 44.0;
    let mut h = pad + head + 14.0 + fig * 1.15 + pad;
    if r.is_some() {
        h += 14.0 + 6.0 + 12.0 + 18.0;
    } else if no_budget {
        h += 12.0 + 18.0 + 12.0 + 40.0;
    }
    let set_tapped = no_budget && kit::card_button(cx, PAD, y, W, h);
    if !no_budget {
        kit::card(cx, PAD, y, W, h);
    }
    let (bx, by) = (PAD + pad, y + pad);
    match s.account_type {
        Some(t) => ak::account_badge(cx, bx, by, head, t),
        None => kit::badge(cx, bx, by, head, &s.icon, tint),
    }
    let tx = bx + head + 14.0;
    let month = activity_data::period_title(period);
    let eyebrow = if !holds {
        if s.is_account() { format!("Out in {month}") } else { format!("Spent in {month}") }
    } else if s.balance >= 0 {
        "Balance".into()
    } else if s.account_type == Some(AccountType::Credit) {
        "Owed".into()
    } else {
        "Below zero".into()
    };
    let eyebrow = if !s.is_account() && s.kind == CategoryKind::Income { format!("In in {month}") } else { eyebrow };
    let et = Text::new(12.0, p.ink3).semi();
    cx.c.text(&eyebrow.to_uppercase(), tx, by + 4.0, et);
    let kind = match s.account_type {
        Some(t) => activity_data::account_type_label(t).to_string(),
        None => copy::plural_as(s.period_count as i64, "entry", "entries") + " this month",
    };
    cx.c.text_fit(&kind, tx, by + 4.0 + et.lh() + 2.0, W - pad * 2.0 - head - 14.0, Text::new(13.0, p.ink2));
    let figure = if holds {
        s.balance.abs()
    } else if s.is_account() {
        s.money_out
    } else {
        s.period_total
    };
    let red = holds && s.balance < 0 && s.account_type != Some(AccountType::Credit);
    let fy = by + head + 14.0;
    kit::money(cx, &fmt.format(figure), PAD + pad, fy, fig, if red { p.held_text } else { p.ink }, p.ink2);
    let mut ry = fy + fig * 1.15;
    if let Some(r) = r {
        ry += 14.0;
        let colour = if model::limit_reached(r) {
            p.held
        } else if r.status == PaceStatus::OverPace {
            p.waiting
        } else {
            tint
        };
        kit::meter(cx, PAD + pad, ry, W - pad * 2.0, r.spent_fraction, Some(r.pace_fraction), colour);
        ry += 6.0 + 12.0;
        let days = if model::limit_reached(r) { "Limit reached".to_string() } else { copy::days_line(r) };
        let line = format!("{} · {days}", copy::of_budget(r.spent, r.budget, fmt));
        let tone = if r.spent > r.budget {
            p.held_text
        } else if r.status == PaceStatus::OverPace {
            p.waiting_text
        } else {
            p.ink2
        };
        cx.c.text_fit(&line, PAD + pad, ry, W - pad * 2.0, Text::new(13.0, tone).medium());
    } else if no_budget {
        ry += 12.0;
        cx.c.text("No budget for this category", PAD + pad, ry, Text::new(13.0, p.ink2));
        ry += 18.0 + 12.0;
        let t = Text::new(ty::BODY, p.ink).semi();
        let kw = cx.c.measure("Set a budget", t) + 26.0 + 32.0;
        kit::key(cx, PAD + pad, ry, kw, 40.0, "Set a budget", Some("pie"), KeyKind::Plain);
    }
    (y + h, set_tapped)
}

/// The two cards under it: this month and last for a category, in and out for an account.
fn stats<'a>(s: &Subject, fmt: &MoneyFormatter) -> Vec<ak::Stat<'a>> {
    let entries = |n: usize| copy::plural_as(n as i64, "entry", "entries");
    if s.is_account() {
        let second: ak::Stat = if s.tracks_balance {
            ("Out this month", fmt.format_whole(s.money_out), Some(entries(s.out_count)), None)
        } else {
            ("Entries this month", (s.in_count + s.out_count).to_string(), Some(format!("{} out", copy::plural(s.out_count as i64, "payment"))), None)
        };
        vec![("In this month", fmt.format_whole(s.money_in), Some(entries(s.in_count)), None), second]
    } else {
        vec![
            ("This month", fmt.format_whole(s.period_total), Some(entries(s.period_count)), None),
            ("Last month", fmt.format_whole(s.last_total), Some(copy::versus_last_line(s.period_total, s.last_same_day, fmt)), None),
        ]
    }
}

// One entry, read

fn view(app: &mut App, dt: f32, id: i64) -> bool {
    let key = (app.ledger_gen, id);
    if app.st.activity.view.as_ref().is_none_or(|(k, _)| *k != key) {
        let e = Entry::view(&app.store, id);
        app.st.activity.view = Some((key, Box::new(e)));
    }
    let fmt = app.store.formatter();
    let pg = app.page_begin(dt);
    let mut todo = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.activity;
        let mut y = pg.top;
        let e = st.view.as_ref().map(|(_, e)| e.as_ref());
        let found = e.and_then(|e| e.original.as_ref().map(|t| (e, t)));
        let title = match found.map(|(_, t)| t.r#type) {
            Some(TxType::Income) => "Income",
            Some(TxType::Transfer) => "Transfer",
            Some(TxType::Expense) => "Expense",
            None => "",
        };
        let (back, edit, h) = widgets::page_header(&mut cx, y, title, found.map(|_| "Edit"));
        y += h;
        if back {
            todo = Some(Act::Back);
        }
        let edit_route = Act::Go(Route::Entry { id: Some(id), kind: None, account: None, category: None });
        if edit {
            todo = Some(edit_route);
        }
        match found {
            None => {
                y += widgets::empty(&mut cx, PAD, y, W, "list", "This is gone", "It was deleted, here or on another of your devices.");
            }
            Some((e, t)) => {
                let (b, act) = view_body(&mut cx, y, e, t, &fmt);
                y = b;
                if let Some(a) = act {
                    todo = Some(a);
                }
            }
        }
        bottom = y;
    }
    app.page_end(&pg, bottom);
    act(app, todo);
    pg.moving
}

/// The entry's overview under the header: its badge, what it was, the amount (a tap edits it),
/// then a card of when, the category, the account (and where a transfer went) and whether it
/// repeats, and how its category stands this month. Returns the bottom and what was tapped.
fn view_body(cx: &mut Ctx, mut y: f32, e: &Entry, t: &relay_money::views::Tx, fmt: &MoneyFormatter) -> (f32, Option<Act>) {
    let p = cx.p;
    let mut todo = None;
    let mid = WIDTH / 2.0;
    let transfer = t.r#type == TxType::Transfer;
    let colour = theme::category_color(t.color);

    // The badge, the name and the amount, centred.
    y += 8.0;
    let d = 60.0;
    if transfer {
        ak::glyph_badge(cx, mid - d / 2.0, y, d, "transfer", ak::money_hue());
    } else {
        kit::badge(cx, mid - d / 2.0, y, d, t.icon.as_deref().unwrap_or("dots"), colour);
    }
    y += d + 16.0;
    let nt = Text::new(20.0, p.ink).semi().center();
    let name = if transfer && t.note.trim().is_empty() {
        format!("To {}", t.to_account.as_deref().unwrap_or("another account"))
    } else {
        activity_data::entry_name(t)
    };
    cx.c.text_fit(&name, mid, y, W, nt);
    y += nt.lh() + 6.0;
    let (amount, ink) = ak::amount_of(cx, t, fmt);
    let size = 44.0;
    let aw = kit::money_w(cx.c, &amount, size);
    let cents = if t.r#type == TxType::Expense { p.ink2 } else { ink };
    kit::money(cx, &amount, mid - aw / 2.0, y, size, ink, cents);
    if cx.pressed(mid - aw / 2.0 - 12.0, y - 4.0, aw + 24.0, size * 1.15 + 8.0) {
        cx.c.rect(mid - aw / 2.0 - 12.0, y - 4.0, aw + 24.0, size * 1.15 + 8.0, 14.0, p.wash);
    }
    if cx.tap(mid - aw / 2.0 - 12.0, y - 4.0, aw + 24.0, size * 1.15 + 8.0) {
        todo = Some(Act::Go(Route::Entry { id: Some(t.id), kind: None, account: None, category: None }));
    }
    y += size * 1.15 + 28.0;

    // The details, a hairline between each.
    let date = t.date.parse::<Date>().ok();
    let account = e.accounts.iter().find(|a| a.id == t.account_id);
    let to = t.to_account_id.and_then(|id| e.accounts.iter().find(|a| a.id == id));
    let note_row = !t.note.trim().is_empty() && name != t.note.trim();
    let repeats = entry_logic::repeats_line(e.bill.as_ref(), e.today);
    let repeat_sub = e.bill.as_ref().filter(|b| b.active).map(|b| if b.auto_post { "Posts itself" } else { "Reminds you to log it" });
    // The lines under the rows, worked out before the card so its height is the rows' own: a row
    // is taller exactly when it has a line under it.
    let account_line = |a: Option<&relay_money::views::AccountView>| a.filter(|a| a.r#type != relay_money::model::AccountType::Cash).map(|a| activity_data::account_type_label(a.r#type));
    let (from_sub, to_sub) = (account_line(account), account_line(to));
    let (word, rest) = match repeats.split_once(" · ") {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (repeats.clone(), None),
    };
    // "Repeats" | "Monthly", "Next on December 17 · posts itself".
    let value = word.strip_prefix("Repeats ").map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    });
    let sub = match (&rest, repeat_sub) {
        (Some(r), Some(how)) => Some(format!("{}{} · {}", r[..1].to_uppercase(), &r[1..], how.to_lowercase())),
        (Some(r), None) => Some(format!("{}{}", r[..1].to_uppercase(), &r[1..])),
        _ => None,
    };
    let repeat_has_sub = value.is_some() && e.bill.is_some() && sub.is_some();
    let row = |has_sub: bool| if has_sub { 68.0 } else { 56.0 };
    let rows_h = 56.0 // when
        + if transfer { 0.0 } else { 56.0 }
        + row(from_sub.is_some())
        + if transfer { row(to_sub.is_some()) } else { 0.0 }
        + row(repeat_has_sub)
        + if note_row { 56.0 } else { 0.0 };
    let ch = ak::CARD_PAD * 2.0 + rows_h;
    kit::card(cx, PAD, y, W, ch);
    let mut ry = y + ak::CARD_PAD;
    let when = date.map(|d| activity_data::full_date(d, e.today)).unwrap_or_else(|| t.date.clone());
    ry += ak::detail_row(cx, ry, true, "When", &when, None, None, false).0;
    if !transfer {
        let cat = t.category.clone().unwrap_or_else(|| "Uncategorized".into());
        let icon = t.icon.as_deref().unwrap_or("dots");
        let (h, tapped) = ak::detail_row(cx, ry, false, "Category", &cat, None, Some((icon, colour)), t.category_id.is_some());
        if tapped {
            todo = Some(Act::Go(Route::Transactions { category: t.category_id, account: None }));
        }
        ry += h;
    }
    let label = match t.r#type {
        TxType::Expense => "Paid from",
        TxType::Income => "Paid into",
        TxType::Transfer => "From",
    };
    let (h, tapped) = ak::detail_row(cx, ry, false, label, &t.account, from_sub, None, true);
    if tapped {
        todo = Some(Act::Go(Route::Transactions { category: None, account: Some(t.account_id) }));
    }
    ry += h;
    if transfer {
        let name = t.to_account.clone().unwrap_or_else(|| "Another account".into());
        let (h, tapped) = ak::detail_row(cx, ry, false, "To", &name, to_sub, None, t.to_account_id.is_some());
        if tapped {
            todo = Some(Act::Go(Route::Transactions { category: None, account: t.to_account_id }));
        }
        ry += h;
    }
    let (h, tapped) = match (&value, &e.bill) {
        (Some(v), Some(b)) => {
            let (h, tapped) = ak::detail_row(cx, ry, false, "Repeats", v, sub.as_deref(), None, true);
            (h, tapped.then_some(b.id))
        }
        _ => (ak::detail_row(cx, ry, false, "Repeats", "Does not repeat", None, None, false).0, None),
    };
    if let Some(bill) = tapped {
        todo = Some(Act::Go(Route::BillEdit { id: Some(bill) }));
    }
    ry += h;
    if note_row {
        ak::detail_row(cx, ry, false, "Note", t.note.trim(), None, None, false);
    }
    y += ch;

    // How its category stands this month.
    if let Some((words, tone)) = e.category_line(fmt) {
        y += 14.0;
        let lt = Text::new(14.0, match tone {
            model::LineTone::Quiet => p.ink,
            model::LineTone::Warn => p.waiting_text,
            model::LineTone::Held => p.held_text,
        })
        .medium()
        .line(1.4);
        let inner = W - 40.0;
        let title = format!("{} in {}", t.category.as_deref().unwrap_or("Its category"), activity_data::period_title(&e.period));
        let bh = cx.c.para_h(&words, inner, lt, 3);
        let h = 18.0 + 18.0 + 6.0 + bh + 18.0;
        if kit::card_button(cx, PAD, y, W, h) {
            todo = Some(Act::Go(Route::Transactions { category: t.category_id, account: None }));
        }
        ak::card_title(cx, PAD + 20.0, y + 18.0, inner - 20.0, &title);
        cx.c.icon("chevron-right", PAD + W - 20.0 - 6.0, y + 18.0 + 9.0, 13.0, p.ink3);
        cx.c.para(&words, PAD + 20.0, y + 18.0 + 18.0 + 6.0, inner, lt, 3);
        y += h;
    }
    (y, todo)
}
