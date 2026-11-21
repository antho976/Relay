//! Investments and the room editor (Kotlin: ui/invest/InvestScreens.kt, RoomEditViewModel.kt),
//! in the premium look.
//!
//! Investments reads relay-money's portfolio beside the investment accounts' balances, so an
//! account valued by hand still counts: the total on the glass-bars picture with the year's
//! change, the last twelve months as a brass line (each month's end read again from the rows on
//! or before it), this year's contribution room, cash and income, the accounts, where it sits by
//! account kind and by kind of holding, and the holdings. The room editor takes the CRA's figure
//! for one registration on a keypad; Tally subtracts what goes in.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::amount_input::AmountInput;
use relay_money::invest::Portfolio;
use relay_money::model::Registration;
use relay_money::money::MoneyFormatter;

use crate::accounts_kit::{self as mk, W};
use crate::accounts_store::{self as accounts, AccountRow};
use crate::app::{App, Draft, PageDraft};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::invest_logic::{self as logic, InvestView};
use crate::kit::{self, Ctx, KeyKind, PAD, PicCard, ty};
use crate::model;
use crate::nav::Route;
use crate::theme;
use crate::widgets;

#[derive(Default)]
pub struct State {
    /// The portfolio and the accounts as read at a ledger generation.
    read: Option<(u64, Read)>,
    /// The kind the page is narrowed to: none for every account.
    lens: Option<Option<Registration>>,
    room: Option<RoomEditor>,
}

struct Read {
    portfolio: Option<Portfolio>,
    rows: Vec<AccountRow>,
    problem: Option<String>,
    /// What each investment account was worth at the end of each of the last twelve months
    /// (today last).
    history: Vec<(Date, HashMap<i64, i64>)>,
    /// The same on the last day of last year.
    year_start: HashMap<i64, i64>,
}

fn read(app: &App, today: Date) -> Read {
    let rows = accounts::rows(&app.store).unwrap_or_default();
    let input = app.store.ledger.portfolio_input(today).ok();
    // Each month's end as the Accounts page reads last month's (accounts_store::worth_back): an
    // account not priced from holdings by `d` and with nowhere to start by then (no opening, no
    // value recorded) reads as worth what it is now less what was moved into it since, so a first
    // holdings import is not a gain this year or on the chart, and money moved in is not either.
    let now = accounts::worth_today(&rows, input.as_ref(), today);
    // With no portfolio input (it failed to read), still by balances, as the Accounts page does:
    // nothing at all would draw the chart from $0 and read the whole worth as this year's gain.
    let worth_on = |d: Date| -> HashMap<i64, i64> { accounts::worth_back(&app.store, &rows, input.as_ref(), d, &now).unwrap_or_default() };
    let history = logic::month_ends(today).into_iter().map(|d| (d, worth_on(d))).collect();
    let year_start = Date::new(today.year() - 1, 12, 31).map(worth_on).unwrap_or_default();
    match app.store.ledger.portfolio(today) {
        Ok(mut p) => {
            // A report older than a value recorded by hand is set aside, and what moved in or out
            // after an account's report counts there, as net worth reads it.
            if let Some(i) = &input {
                logic::set_aside_older_reports(&mut p, i, &rows);
                logic::count_moves(&mut p, i);
            }
            Read { portfolio: Some(p), rows, problem: None, history, year_start }
        }
        Err(_) => Read {
            portfolio: None,
            rows,
            problem: Some("These investments could not be added up. Importing the newest holdings report again usually settles it.".into()),
            history,
            year_start,
        },
    }
}

enum Act {
    Go(Route),
    Lens(Option<Option<Registration>>),
    Back,
    SaveRoom,
    RemoveRoom,
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let today = model::today();
    let at = app.ledger_gen;
    if app.fresh || app.st.invest.read.as_ref().is_none_or(|(g, _)| *g != at) {
        let r = read(app, today);
        app.st.invest.read = Some((at, r));
    }
    match app.route().clone() {
        Route::RoomEdit { kind } => room_editor(app, dt, &kind, today),
        _ => page(app, dt, today),
    }
}

// The page

fn page(app: &mut App, dt: f32, today: Date) -> bool {
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &app.st.invest;
        let Some((_, r)) = &st.read else { return pg.moving };
        let (back, plus, hh) = widgets::page_header(&mut cx, pg.top, "Investments", Some("plus"));
        if back {
            act = Some(Act::Back);
        }
        if plus {
            act = Some(Act::Go(Route::AccountEdit { id: None, kind: Some(relay_money::model::AccountType::Investment) }));
        }
        if mk::header_key(&mut cx, pg.top, "download") {
            act = Some(Act::Go(Route::Import));
        }
        let v = logic::view(r.portfolio.as_ref(), &r.rows, st.lens);
        bottom = content(&mut cx, &v, r, &fmt, today, pg.top + hh, &mut act);
    }
    app.page_end(&pg, bottom);
    match act {
        Some(Act::Go(r)) => app.push(r),
        Some(Act::Lens(l)) => app.st.invest.lens = l,
        Some(Act::Back) => {
            app.pop();
        }
        _ => {}
    }
    pg.moving
}

#[allow(clippy::too_many_arguments)]
fn content(cx: &mut Ctx, v: &InvestView, r: &Read, fmt: &MoneyFormatter, today: Date, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = top;
    if let Some(pr) = &r.problem {
        y += cx.c.para(pr, PAD + 4.0, y, W - 8.0, Text::new(ty::CAPTION, p.held_text).line(1.4), 3) + 12.0;
    }
    if v.accounts.is_empty() {
        let eh = widgets::empty(
            cx,
            PAD,
            y,
            W,
            "trend",
            "No investment accounts",
            "Add a TFSA, an RRSP or a brokerage account, then import its Wealthsimple files or record what it is worth.",
        );
        y += eh + 14.0;
        let kw = (W - 10.0) / 2.0;
        if kit::key(cx, PAD, y, kw, 52.0, "Add an account", Some("plus"), KeyKind::Primary) {
            *act = Some(Act::Go(Route::AccountEdit { id: None, kind: Some(relay_money::model::AccountType::Investment) }));
        }
        if kit::key(cx, PAD + kw + 10.0, y, kw, 52.0, "Import", Some("download"), KeyKind::Plain) {
            *act = Some(Act::Go(Route::Import));
        }
        return y + 52.0;
    }

    if !v.lenses.is_empty() {
        let mut labels = vec!["All"];
        labels.extend(v.lenses.iter().map(|l| l.label));
        let selected = match &v.lens {
            None => 0,
            Some(l) => v.lenses.iter().position(|x| x == l).map(|i| i + 1).unwrap_or(0),
        };
        let (picked, fh) = widgets::pills(cx, PAD, y, W, &labels, Some(selected));
        match picked {
            Some(0) => *act = Some(Act::Lens(None)),
            Some(i) => *act = Some(Act::Lens(Some(v.lenses[i - 1].registration))),
            None => {}
        }
        y += fh + 16.0;
    }

    // What it is worth, on the glass bars.
    let ids: Vec<i64> = v.accounts.iter().map(|a| a.id).collect();
    let start: i64 = ids.iter().map(|id| r.year_start.get(id).copied().unwrap_or(0)).sum();
    let mut line = logic::year_line(v.worth, start, fmt);
    if let Some(s) = v.newest.filter(|n| logic::is_stale(*n, today)).map(|n| logic::stale_line(n, today)) {
        line = format!("{line} · {}", s.to_lowercase());
    }
    let eyebrow = v.lens.as_ref().map(|l| l.label).unwrap_or("Invested");
    let figure = fmt.format(v.worth);
    let ch = 360.0;
    kit::pic_card(cx, PAD, y, W, ch, "chart", &PicCard { eyebrow: Some(eyebrow), title: &figure, title_size: 34.0, line: Some(&line), ..Default::default() });
    y += ch + 16.0;

    // The last twelve months.
    let mut series = logic::worth_series(&r.history, &ids);
    if let Some(last) = series.last_mut() {
        last.1 = v.worth;
    }
    if series.len() >= 2 {
        let change = series[series.len() - 1].1 - series[0].1;
        let h = 18.0 + 22.0 + 12.0 + 156.0 + 16.0;
        kit::card(cx, PAD, y, W, h);
        let colour = if change < 0 { p.held_text } else { p.live_text };
        let words = accounts::signed_whole(change, fmt);
        cx.c.text("Last 12 months", PAD + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
        cx.c.text(&words, PAD + W - 18.0, y + 18.0, Text::new(15.0, colour).semi().right());
        let values: Vec<i64> = series.iter().map(|(_, v)| *v).collect();
        let labels: Vec<String> = series.iter().map(|(d, _)| d.strftime("%b").to_string().chars().next().map(String::from).unwrap_or_default()).collect();
        mk::value_chart(cx, PAD + 18.0, y + 18.0 + 22.0 + 12.0, W - 36.0, 156.0, &values, &labels, |t| fmt.format_whole(t));
        y += h + 16.0;
    }

    // The room left this year.
    if !v.room.is_empty() {
        y = room_card(cx, v, fmt, today, y, act) + 16.0;
    }

    // Cash and income, side by side.
    let tw = (W - 12.0) / 2.0;
    let th = 18.0 + 20.0 + 6.0 + 30.0 + 4.0 + 17.0 + 16.0;
    let income = v.income_12m.filter(|i| *i != 0).map(|i| fmt.format_whole(i)).unwrap_or_else(|| "None yet".into());
    for (i, (cap, fig, sub)) in [("Cash", fmt.format_whole(v.cash), "not invested yet"), ("Income, 12 months", income, "dividends and interest")].iter().enumerate() {
        let x = PAD + i as f32 * (tw + 12.0);
        kit::card(cx, x, y, tw, th);
        cx.c.text(cap, x + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
        cx.c.text_fit(fig, x + 18.0, y + 18.0 + 20.0 + 6.0, tw - 36.0, Text::new(24.0, p.ink).medium());
        cx.c.text_fit(sub, x + 18.0, y + th - 16.0 - 17.0, tw - 36.0, Text::new(13.0, p.ink3));
    }
    y += th + 28.0;

    // The accounts.
    let (hh, _) = kit::section(cx, y, "Accounts", None);
    y += hh + 12.0;
    let (h, hit) = mk::row_card(cx, y, v.accounts.len(), |cx, i, ry| {
        let a = &v.accounts[i];
        let worth = fmt.format(a.worth);
        let ret = logic::return_text(a, today);
        let under = ret.as_deref().map(|r| (r, if r.starts_with('-') || r.starts_with('−') { p.held_text } else { p.live_text }));
        mk::card_row(cx, PAD, ry, W, "trend", &a.name, &logic::account_meta(a, today), Some((&worth, p.ink)), under)
    });
    if let Some(i) = hit {
        *act = Some(Act::Go(Route::Transactions { category: None, account: Some(v.accounts[i].id) }));
    }
    y += h + 28.0;

    if !v.allocation.is_empty() {
        y = share_card(cx, y, "By account kind", &v.allocation, fmt) + 16.0;
    }
    if !v.kinds.is_empty() {
        y = share_card(cx, y, "By kind of holding", &v.kinds, fmt) + 16.0;
    }

    if !v.holdings.is_empty() {
        y += 12.0;
        let shown = &v.holdings[..v.holdings.len().min(20)];
        let (hh, _) = kit::section(cx, y, "Holdings", None);
        y += hh + 4.0;
        cx.c.text("Largest first", PAD, y, Text::new(13.0, p.ink3));
        y += 26.0;
        let (h, hit) = mk::row_card(cx, y, shown.len(), |cx, i, ry| {
            let h = &shown[i];
            let value = fmt.format_whole(h.value);
            let gain = logic::gain_text(h.gain, h.gain_bps, fmt);
            let name = if h.name.trim().is_empty() { h.symbol.clone() } else { format!("{} · {}", h.symbol, h.name) };
            let gc = if h.gain >= 0 { p.live_text } else { p.held_text };
            mk::card_row(cx, PAD, ry, W, "coins", &name, &logic::holding_line(h), Some((&value, p.ink)), Some((&gain, gc)))
        });
        // The row reads as pressed under a finger: a tap opens the account that holds it, as its
        // account's row above does, rather than nothing.
        if let Some(i) = hit {
            *act = Some(Act::Go(Route::Transactions { category: None, account: Some(shown[i].account_id) }));
        }
        y += h + 16.0;
    }
    for issue in &v.issues {
        y += cx.c.para(issue, PAD + 4.0, y, W - 8.0, Text::new(ty::CAPTION, p.waiting_text).line(1.4), 3) + 8.0;
    }
    y
}

/// The room left for the year: per registration what is left of the CRA's figure, an ink meter of
/// what went in with a mark where an even pace would be, and how to use the rest.
fn room_card(cx: &mut Ctx, v: &InvestView, fmt: &MoneyFormatter, today: Date, y: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let years: Vec<i32> = v.room.iter().map(|l| l.year).collect();
    let title = if years.iter().all(|y| *y == years[0]) { format!("Room left for {}", years[0]) } else { "Room left".to_string() };
    let iw = W - 36.0;
    let cap = Text::new(12.5, p.ink3).line(1.35);
    // The reading shows what is left; its line keeps only the pace that uses it.
    let texts: Vec<String> = v
        .room
        .iter()
        .map(|l| {
            let t = logic::room_text(l, today, fmt);
            let lead = format!("{} left · ", fmt.format_whole(l.left.unwrap_or(0)));
            match t.strip_prefix(&lead) {
                Some(rest) => capitalised(rest),
                None => t,
            }
        })
        .collect();
    let heights: Vec<f32> = v.room.iter().zip(&texts).map(|(l, t)| 12.0 + 22.0 + if l.room.is_some_and(|r| r > 0) { 16.0 } else { 2.0 } + cx.c.para_h(t, iw, cap, 2) + 12.0).collect();
    let head = 18.0 + 22.0 + 6.0;
    let h = head + heights.iter().sum::<f32>() + 8.0;
    kit::card(cx, PAD, y, W, h);
    cx.c.text(&title, PAD + 18.0, y + 18.0, Text::new(16.0, p.ink).semi());
    let mut ry = y + head;
    for ((line, text), rh) in v.room.iter().zip(&texts).zip(heights) {
        let (x, w) = (PAD + 6.0, W - 12.0);
        if cx.pressed(x, ry, w, rh) {
            cx.c.rect(x, ry, w, rh, 16.0, p.wash);
        }
        let tx = PAD + 18.0;
        let over = line.over > 0;
        let ly = ry + 12.0;
        let right = tx + iw;
        // "$2,400 of $7,000": what is left, of the figure.
        let mut rx = right;
        match line.room {
            Some(room) => {
                let of = format!(" of {}", fmt.format_whole(room));
                let ot = Text::new(13.0, p.ink3).right();
                let ow = cx.c.measure(&of, ot);
                cx.c.text(&of, rx, ly + 2.0, ot);
                rx -= ow;
                let (left, c) = if over { (format!("{} over", fmt.format_whole(line.over)), p.held_text) } else { (fmt.format_whole(line.left.unwrap_or(0)), p.ink) };
                let lt = Text::new(15.0, c).semi().right();
                cx.c.text(&left, rx, ly, lt);
                rx -= cx.c.measure(&left, lt);
            }
            None => {
                let t = Text::new(13.0, p.ink3).right();
                let words = format!("{} in · not set", fmt.format_whole(line.contributed));
                rx -= cx.c.measure(&words, t);
                cx.c.text(&words, right, ly + 2.0, t);
            }
        }
        cx.c.text_fit(line.registration.label(), tx, ly, rx - tx - 10.0, Text::new(15.0, p.ink).semi());
        let mut ty_ = ly + 22.0;
        match line.room {
            Some(room) if room > 0 => {
                let pace = logic::room_pace_fraction(line.registration, line.year, today);
                kit::meter(cx, tx, ty_ + 4.0, iw, line.contributed as f32 / room as f32, Some(pace), if over { p.held } else { p.ink });
                ty_ += 16.0;
            }
            _ => ty_ += 2.0,
        }
        cx.c.para(text, tx, ty_, iw, cap.color(if over { p.held_text } else { p.ink3 }), 2);
        if cx.tap(x, ry, w, rh) {
            *act = Some(Act::Go(Route::RoomEdit { kind: line.registration.label().to_string() }));
        }
        ry += rh;
    }
    y + h
}

/// A card with a stack bar and a legend row per share.
fn share_card(cx: &mut Ctx, y: f32, title: &str, lines: &[logic::ShareLine], fmt: &MoneyFormatter) -> f32 {
    let head = 18.0 + 22.0 + 12.0;
    let h = head + 12.0 + 14.0 + lines.len() as f32 * 32.0 + 14.0;
    kit::card(cx, PAD, y, W, h);
    mk::card_title(cx, PAD, y, W, title, None);
    let parts: Vec<(Rgba, i64)> = lines.iter().map(|l| (theme::category_color(Some(l.hue)), l.value)).collect();
    let x = PAD + 18.0;
    let w = W - 36.0;
    let mut ry = y + head;
    mk::stack_bar(cx, x, ry, w, 12.0, &parts);
    ry += 12.0 + 14.0;
    for l in lines {
        ry += mk::share_row(cx, x, ry, w, l.label, theme::category_color(Some(l.hue)), &fmt.format_whole(l.value), &logic::share_percent(l.share_bps));
    }
    y + h
}

// The room editor

/// The room being typed, while its page is open, to keep across the process being stopped.
pub fn saved_draft(app: &App) -> Option<PageDraft> {
    let e = app.st.invest.room.as_ref()?;
    let route = Route::RoomEdit { kind: e.route_kind.clone() };
    let depth = app.depth_of(&route)?;
    Some(PageDraft { depth, route, draft: Draft::Room { input: e.input.text.clone() } })
}

struct RoomEditor {
    /// The kind its route names, to find its page on the stack when keeping the draft.
    route_kind: String,
    registration: Registration,
    year: i32,
    existing: Option<i64>,
    input: AmountInput,
    contributed: i64,
    limit: Option<i64>,
}

fn open_room(app: &App, kind: &str, today: Date) -> RoomEditor {
    let registration = logic::registration_named(kind).filter(|r| logic::ROOM_KINDS.contains(r)).unwrap_or(Registration::Tfsa);
    // An RRSP's year runs to its deadline: until then, room goes on last year's figure.
    let year = logic::room_year(registration, today);
    let digits = app.store.formatter().fraction_digits;
    let line = app.st.invest.read.as_ref().and_then(|(_, r)| r.portfolio.as_ref()).and_then(|p| p.room.iter().find(|l| l.registration == registration && l.year == year).cloned());
    let existing = line.as_ref().and_then(|l| l.room);
    RoomEditor {
        route_kind: kind.to_string(),
        registration,
        year,
        existing,
        input: existing.map(|v| AmountInput::of(v, digits)).unwrap_or_else(|| AmountInput::empty(digits)),
        contributed: line.as_ref().map(|l| l.contributed).unwrap_or(0),
        limit: line.as_ref().and_then(|l| l.limit).or_else(|| relay_money::invest::room_line(registration, year, None, 0, 0).limit),
    }
}

fn room_editor(app: &mut App, dt: f32, kind: &str, today: Date) -> bool {
    if app.fresh || app.st.invest.room.is_none() {
        let mut r = open_room(app, kind, today);
        // Restored after the system stopped Tally: the figure as it was being typed.
        if let Some(Draft::Room { input }) = app.take_draft() {
            r.input = AmountInput::new(&input, r.input.fraction_digits);
        }
        app.st.invest.room = Some(r);
    }
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let Some(e) = app.st.invest.room.as_mut() else { return pg.moving };
        bottom = room_form(&mut cx, e, &fmt, pg.top, &mut act);
    }
    app.page_end(&pg, bottom);
    let Some(e) = app.st.invest.room.as_ref() else { return pg.moving };
    let (reg, year) = (e.registration, e.year);
    match act {
        Some(Act::Back) => {
            app.pop();
        }
        Some(Act::SaveRoom) => {
            let amount = e.input.minor();
            if amount <= 0 {
                app.say(format!("Type your {} room from CRA My Account first", reg.label()), None);
                return pg.moving;
            }
            match app.store.ledger.room_set(reg, year, amount) {
                Ok(()) => {
                    app.invalidate();
                    app.say(format!("{} room set for {year}", reg.label()), None);
                    app.pop();
                }
                Err(err) => app.say(err.to_string(), None),
            }
        }
        Some(Act::RemoveRoom) => {
            // A figure of zero removes it. One tap does it, so Undo sets the old figure again.
            let old = e.existing;
            match app.store.ledger.room_set(reg, year, 0) {
                Ok(()) => {
                    app.invalidate();
                    let words = format!("{} room removed", reg.label());
                    match old {
                        Some(old) => app.say_undo(words, move |app: &mut App| app.store.ledger.room_set(reg, year, old).map_err(|e| e.to_string())),
                        None => app.say(words, None),
                    }
                    app.pop();
                }
                Err(err) => app.say(err.to_string(), None),
            }
        }
        _ => {}
    }
    pg.moving
}

fn room_form(cx: &mut Ctx, e: &mut RoomEditor, fmt: &MoneyFormatter, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = top;
    let title = format!("{} room {}", e.registration.label(), e.year);
    let (back, save, h) = widgets::page_header(cx, y, &title, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::SaveRoom);
    }
    y += h;
    let deadline = format!("Deadline {}", logic::long(logic::room_deadline(e.registration, e.year)));
    cx.c.text(&deadline, WIDTH / 2.0, y - 10.0, Text::new(13.0, p.ink2).center());
    y += 22.0;

    // The figure being typed, large, over what went in so far.
    let ch = 22.0 + 17.0 + 10.0 + 52.0 + 12.0 + 20.0 + 22.0;
    kit::card(cx, PAD, y, W, ch);
    let mid = WIDTH / 2.0;
    let mut ty_ = y + 22.0;
    cx.c.text(&format!("ROOM FOR {}", e.year), mid, ty_, Text::new(12.0, p.ink3).semi().center());
    ty_ += 17.0 + 10.0;
    let shown = if e.input.is_empty() {
        fmt.format_whole(0)
    } else if e.input.text.contains('.') {
        fmt.format(e.input.minor())
    } else {
        fmt.format_whole(e.input.minor())
    };
    let mt = Text::new(44.0, if e.input.is_empty() { p.ink3 } else { p.ink }).medium().center().line(1.15);
    cx.c.text_fit(&shown, mid, ty_, W - 36.0, mt);
    ty_ += 52.0 + 12.0;
    let went = format!("{} went in this year so far", fmt.format_whole(e.contributed));
    cx.c.text(&went, mid, ty_, Text::new(14.0, p.ink2).center());
    y += ch + 16.0;

    if let Some(limit) = e.limit {
        let words = format!("Use this year's limit, {}", fmt.format_whole(limit));
        let (picked, ph) = widgets::pills(cx, PAD, y, W, &[words.as_str()], None);
        if picked.is_some() {
            e.input = AmountInput::of(limit, fmt.fraction_digits);
        }
        y += ph + 16.0;
    }

    if let Some(k) = mk::keypad(cx, y, e.input.fraction_digits > 0) {
        e.input = match k {
            "." => e.input.decimal(),
            "⌫" => e.input.backspace(),
            d => e.input.digit(d.parse().unwrap_or(0)),
        };
    }
    y += mk::KEYPAD_H + 20.0;

    let source = logic::room_source(e.registration);
    y += cx.c.para(source, PAD + 4.0, y, W - 8.0, Text::new(13.0, p.ink3).line(1.45), 6) + 20.0;

    if e.existing.is_some() {
        if mk::action_row(cx, y, "trash", "Remove this figure", true) {
            *act = Some(Act::RemoveRoom);
        }
        y += mk::ACTION_H;
    }
    y
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
