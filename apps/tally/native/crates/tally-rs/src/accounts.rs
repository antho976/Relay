//! Accounts and the account editor (Kotlin: ui/accounts/AccountsScreens.kt), in the premium look.
//!
//! Accounts leads with net worth on the cards picture, over the accounts that can say what they
//! hold, and how far it moved since last month ended; then the accounts in groups (everyday,
//! credit, investing, archived), each a card of rows with its balance, a card's next payment and
//! the bank it follows. An account given nowhere to start shows no balance, since one built from
//! spending alone would read as a debt that is not there. This month's spending, the way to bank
//! connections and a transfer key stay. The editor names an account, picks its kind (and an
//! investment account's registration), sets what it opened at, whether new entries start in it
//! and whether it is archived, records an investment account's value, and deletes it, saying what
//! goes with it.

use relay_money::copy;
use relay_money::model::{AccountType, TxType};
use relay_money::money::MoneyFormatter;

use crate::accounts_kit::{self as mk, Answer, W};
use crate::accounts_store::{self as store, AccountLine, AccountUse, AccountsState, Draft, Group, Stored, Template, Value};
use crate::app::App;
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Ctx, KeyKind, PAD, PicCard, ty};
use crate::model;
use crate::nav::Route;
use crate::widgets;

#[derive(Default)]
pub struct State {
    /// The list as read at a ledger generation.
    list: Option<(u64, Result<AccountsState, String>)>,
    edit: Option<Editor>,
}

/// The account editor's draft and what it was read from.
struct Editor {
    id: Option<i64>,
    read_at: u64,
    stored: Option<Stored>,
    draft: Draft,
    show_errors: bool,
    /// What goes with the account, while the delete dialog is open.
    confirm: Option<AccountUse>,
    values: Vec<Value>,
    value_text: String,
    templates: Vec<Template>,
}

enum Act {
    Go(Route),
    Back,
    Save,
    AskDelete,
    Delete,
    Archive,
    CloseDialog,
    Record,
    RemoveValue(i64),
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    match app.route().clone() {
        Route::AccountEdit { id } => editor(app, dt, id),
        _ => list(app, dt),
    }
}

// The list

fn list(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let at = app.ledger_gen;
    if app.fresh || app.st.accounts.list.as_ref().is_none_or(|(g, _)| *g != at) {
        app.st.accounts.list = Some((at, store::read_state(&app.store, model::today())));
    }
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let Some((_, read)) = &app.st.accounts.list else { return pg.moving };
        let can_transfer = read.as_ref().is_ok_and(|s| s.can_transfer());
        let (back, plus, hh) = widgets::page_header(&mut cx, pg.top, "Accounts", Some("plus"));
        if back {
            act = Some(Act::Back);
        }
        if plus {
            act = Some(Act::Go(Route::AccountEdit { id: None }));
        }
        if can_transfer && mk::header_key(&mut cx, pg.top, "transfer") {
            act = Some(Act::Go(Route::Entry { id: None, kind: Some(TxType::Transfer), account: None, category: None }));
        }
        let y = pg.top + hh;
        bottom = match read {
            Ok(s) => list_content(&mut cx, s, &fmt, y, &mut act),
            Err(e) => {
                let p = cx.p;
                cx.c.para(e, PAD, y, W, Text::new(ty::UI, p.held_text), 4);
                y + 60.0
            }
        };
    }
    app.page_end(&pg, bottom);
    match act {
        Some(Act::Go(r)) => app.push(r),
        Some(Act::Back) => {
            app.pop();
        }
        _ => {}
    }
    pg.moving
}

fn list_content(cx: &mut Ctx, s: &AccountsState, fmt: &MoneyFormatter, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = top;

    if s.active.is_empty() && s.archived.is_empty() {
        let eh = widgets::empty(
            cx,
            PAD,
            y,
            W,
            "bank",
            "No accounts yet",
            "An account is what you pay with: a chequing account, a card, cash. Each entry says which one.",
        );
        y += eh + 14.0;
        if kit::key(cx, PAD, y, W, 52.0, "Add an account", Some("plus"), KeyKind::Primary) {
            *act = Some(Act::Go(Route::AccountEdit { id: None }));
        }
        y += 52.0 + 24.0;
        return banks_link(cx, s, y, act);
    }

    // Net worth, on the cards picture; with nothing that says what it holds, this month's spending.
    let counted = !s.worth.now.is_empty();
    let figure = if counted { fmt.format(s.worth.total()) } else { fmt.format_whole(s.month_spent) };
    let line = if counted { store::change_line(&s.worth, fmt).unwrap_or_else(|| store::top_line(s)) } else { store::top_line(s) };
    let ch = 360.0;
    kit::pic_card(
        cx,
        PAD,
        y,
        W,
        ch,
        "cards",
        &PicCard { eyebrow: Some(if counted { "Net worth" } else { "Spent this month" }), title: &figure, title_size: 34.0, line: Some(&line), ..Default::default() },
    );
    y += ch + 28.0;

    for g in Group::ALL {
        let lines: Vec<&AccountLine> = s.active.iter().filter(|l| Group::of(l.row.view.r#type) == g).collect();
        if lines.is_empty() {
            continue;
        }
        let total: Option<i64> = lines.iter().filter_map(|l| s.worth.of(l.row.view.id)).reduce(|a, b| a + b);
        let figure = total.map(|t| signed_money(t, fmt));
        y += mk::group_head(cx, y, g.label(), figure.as_deref());
        y = account_card(cx, y, &lines, s, fmt, act) + 26.0;
    }
    if !s.archived.is_empty() {
        let lines: Vec<&AccountLine> = s.archived.iter().collect();
        y += mk::group_head(cx, y, "Archived", None);
        y = account_card(cx, y, &lines, s, fmt, act) + 26.0;
    }

    // What went out of them this month, when the lead is net worth.
    if counted {
        y += mk::group_head(cx, y, "This month", None);
        let h = 18.0 + 20.0 + 6.0 + 30.0 + 6.0 + 18.0 + 18.0;
        kit::card(cx, PAD, y, W, h);
        cx.c.text("Spent from your accounts", PAD + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
        kit::money(cx, &fmt.format(s.month_spent), PAD + 18.0, y + 18.0 + 20.0 + 6.0, 26.0, p.ink, p.ink2);
        cx.c.text_fit(&store::top_line(s), PAD + 18.0, y + h - 18.0 - 18.0, W - 36.0, Text::new(13.0, p.ink3));
        y += h + 26.0;
    }

    y = banks_link(cx, s, y, act);
    if s.worth.now.len() < s.active.len() {
        y += 14.0;
        y += cx.c.para(
            "Tally counts what you spend from each account. Give one its balance in its settings and Tally keeps what it holds as well.",
            PAD + 4.0,
            y,
            W - 8.0,
            Text::new(ty::CAPTION, p.ink3).line(1.4),
            3,
        );
    }
    y
}

/// A balance as the list shows it: a debt with its minus.
fn signed_money(v: i64, fmt: &MoneyFormatter) -> String {
    if v < 0 { format!("−{}", fmt.format(-v)) } else { fmt.format(v) }
}

/// One group's card: a row per account, hairlines between.
fn account_card(cx: &mut Ctx, y: f32, lines: &[&AccountLine], s: &AccountsState, fmt: &MoneyFormatter, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let (h, hit) = mk::row_card(cx, y, lines.len(), |cx, i, ry| {
        let l = lines[i];
        let a = &l.row.view;
        let figure = match s.worth.of(a.id) {
            Some(v) => (signed_money(v, fmt), if v < 0 { p.held_text } else { p.ink }),
            None if a.archived => (String::new(), p.ink3),
            None => ("No balance".to_string(), p.ink3),
        };
        let line = store::row_line(l, s, fmt);
        let fig = (!figure.0.is_empty()).then_some((figure.0.as_str(), figure.1));
        mk::card_row(cx, PAD, ry, W, store::account_glyph(a.r#type), &a.name, &line, fig, None)
    });
    if let Some(i) = hit {
        let a = &lines[i].row.view;
        *act = Some(Act::Go(if a.archived { Route::AccountEdit { id: Some(a.id) } } else { Route::Transactions { category: None, account: Some(a.id) } }));
    }
    y + h
}

/// The way to bank connections: which bank the accounts already follow, or the offer to connect.
fn banks_link(cx: &mut Ctx, s: &AccountsState, y: f32, act: &mut Option<Act>) -> f32 {
    let mut banks: Vec<String> = Vec::new();
    for l in s.active.iter().filter(|l| l.row.connected()) {
        let b = store::bank_name(&l.row);
        if !banks.contains(&b) {
            banks.push(b);
        }
    }
    let (title, line) = if banks.is_empty() {
        ("Connect a bank".to_string(), "Desjardins and Wealthsimple, read only".to_string())
    } else {
        let n = s.active.iter().filter(|l| l.row.connected()).count() as i64;
        (format!("Connected to {}", banks.join(" and ")), format!("{} follow it, read only", copy::plural(n, "account")))
    };
    let h = widgets::row_h(true) + 8.0;
    kit::card(cx, PAD, y, W, h);
    let ink = cx.p.ink;
    if widgets::row(cx, PAD + 4.0, y + 4.0, W - 8.0, Some(("link", ink)), &title, Some(&line), None, true) {
        *act = Some(Act::Go(Route::Banks));
    }
    y + h
}

// The editor

fn open_editor(app: &mut App, id: Option<i64>) -> Editor {
    let digits = app.store.formatter().fraction_digits;
    let default_id = store::default_account(&app.store);
    let stored = id.and_then(|id| store::stored(&app.store, id).ok().flatten());
    let draft = match &stored {
        Some(s) => store::draft_of(s, default_id, digits),
        // The first account becomes the default, so the entry screen has somewhere to start.
        None => Draft { is_default: default_id.is_none(), ..Draft::default() },
    };
    let templates = if stored.is_none() {
        let names: Vec<String> = store::rows(&app.store).unwrap_or_default().into_iter().map(|r| r.view.name).collect();
        store::free_templates(&names)
    } else {
        Vec::new()
    };
    let values = id.map(|id| store::values(&app.store, id).unwrap_or_default()).unwrap_or_default();
    Editor { id: stored.as_ref().map(|s| s.row.view.id), read_at: app.ledger_gen, stored, draft, show_errors: false, confirm: None, values, value_text: String::new(), templates }
}

fn editor(app: &mut App, dt: f32, id: Option<i64>) -> bool {
    if app.fresh || app.st.accounts.edit.is_none() {
        let e = open_editor(app, id);
        app.st.accounts.edit = Some(e);
    }
    // Another write moved the ledger (a value recorded): read the stored row again, keep the draft.
    if let Some(e) = &mut app.st.accounts.edit
        && e.read_at != app.ledger_gen
    {
        e.read_at = app.ledger_gen;
        if let Some(id) = e.id {
            e.stored = store::stored(&app.store, id).ok().flatten();
            e.values = store::values(&app.store, id).unwrap_or_default();
        }
    }
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let today = model::today();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let Some(e) = app.st.accounts.edit.as_mut() else { return pg.moving };
        let dialog = e.confirm.is_some();
        let held = if dialog { widgets::hold(&mut cx) } else { None };
        bottom = edit_form(&mut cx, e, &fmt, today, pg.top, &mut act);
        if let Some(u) = &e.confirm {
            widgets::release(&mut cx, held);
            let name = e.stored.as_ref().map(|s| s.row.view.name.clone()).unwrap_or_default();
            let can_archive = e.stored.as_ref().is_some_and(|s| !s.row.view.archived) && !u.is_empty();
            let body = store::delete_line(&name, u, &fmt, can_archive);
            match mk::confirm(&mut cx, &format!("Delete {name}?"), &body, "Delete", can_archive.then_some("Archive")) {
                Some(Answer::Act) => act = Some(Act::Delete),
                Some(Answer::Alternate) => act = Some(Act::Archive),
                Some(Answer::Cancel) => act = Some(Act::CloseDialog),
                None => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    if let Some(a) = act {
        edit_act(app, a);
    }
    pg.moving
}

fn edit_form(cx: &mut Ctx, e: &mut Editor, fmt: &MoneyFormatter, today: relay_money::Date, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let digits = fmt.fraction_digits;
    let mut y = top;
    let (title, subtitle) = match &e.stored {
        None => ("New account".to_string(), None),
        Some(s) => {
            let mut parts = Vec::new();
            let opening = e.draft.signed_opening(digits).unwrap_or(s.row.opening);
            if opening != 0 || !e.values.is_empty() {
                let words = store::balance_words(e.draft.kind, store::preview_balance(Some(s), opening), fmt);
                parts.push(capitalise(&words));
            }
            parts.push(copy::plural_as(s.row.entries, "entry", "entries"));
            (if s.row.view.name.trim().is_empty() { "Account".into() } else { s.row.view.name.clone() }, Some(parts.join(" · ")))
        }
    };
    let (back, save, h) = widgets::page_header(cx, y, &title, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::Save);
    }
    y += h;
    if let Some(sub) = &subtitle {
        cx.c.text_fit(sub, WIDTH / 2.0, y - 10.0, W, Text::new(13.0, p.ink2).center());
        y += 28.0;
    }
    let (name_problem, opening_problem) = store::problems(&e.draft, digits);
    let errors = e.show_errors;

    if e.stored.is_none() && !e.templates.is_empty() {
        y += mk::label(cx, PAD, y, "Quick start");
        let names: Vec<&str> = e.templates.iter().map(|t| t.name).collect();
        let (picked, fh) = widgets::pills(cx, PAD, y, W, &names, None);
        if let Some(i) = picked {
            let t = e.templates[i];
            e.draft.apply(&t);
        }
        y += fh + 20.0;
    }

    y += widgets::field(cx, "account.name", PAD, y, W, "Name", "Chequing, Visa, Cash", &mut e.draft.name, false);
    if let Some(pr) = name_problem.filter(|_| errors) {
        y += mk::hint(cx, PAD, y, W, pr, true);
    }
    y += 20.0;

    y += mk::label(cx, PAD, y, "Kind");
    let kinds: Vec<&str> = store::TYPES.iter().map(|t| store::type_label(*t)).collect();
    let selected = store::TYPES.iter().position(|t| *t == e.draft.kind);
    let (picked, kh) = widgets::pills(cx, PAD, y, W, &kinds, selected);
    if let Some(i) = picked {
        e.draft.kind = store::TYPES[i];
    }
    y += kh + 4.0;
    y += mk::hint(cx, PAD, y, W, store::type_hint(e.draft.kind), false) + 20.0;

    if e.draft.kind == AccountType::Investment {
        y += mk::label(cx, PAD, y, "Registration");
        let mut regs = vec!["None"];
        regs.extend(store::REGISTRATIONS.iter().map(|r| r.label()));
        let selected = match e.draft.registration {
            None => 0,
            Some(r) => store::REGISTRATIONS.iter().position(|x| *x == r).map(|i| i + 1).unwrap_or(0),
        };
        let (picked, rh) = widgets::pills(cx, PAD, y, W, &regs, Some(selected));
        match picked {
            Some(0) => e.draft.registration = None,
            Some(i) => e.draft.registration = Some(store::REGISTRATIONS[i - 1]),
            None => {}
        }
        y += rh + 20.0;
    }

    let opening_label = if e.draft.owe { "Owed when it opened" } else { "Balance when it opened" };
    y += widgets::field(cx, "account.opening", PAD, y, W, opening_label, &fmt.format(0), &mut e.draft.opening_text, true);
    y += match opening_problem.filter(|_| errors) {
        Some(pr) => mk::hint(cx, PAD, y, W, pr, true),
        None => mk::hint(cx, PAD, y, W, "What it held before the first entry in Tally", false),
    };
    y += 20.0;

    // The switches, in one card.
    let mut rows: Vec<(&'static str, &str, Option<&str>)> = Vec::new();
    if e.draft.shows_owe() {
        rows.push(("card", "Money owed", Some("Counts against what you hold")));
    }
    rows.push(("flag", "New entries start here", None));
    if e.stored.is_some() {
        rows.push(("save", "Archived", Some("Entries kept, out of the pickers")));
    }
    let ch: f32 = rows.iter().map(|r| widgets::row_h(r.2.is_some())).sum::<f32>() + 8.0;
    kit::card(cx, PAD, y, W, ch);
    let mut ry = y + 4.0;
    let archived = e.draft.archived;
    for (i, (glyph, words, sub)) in rows.into_iter().enumerate() {
        if i > 0 {
            widgets::divider(cx, PAD + 4.0, ry, W - 8.0);
        }
        let on = match words {
            "Money owed" => &mut e.draft.owe,
            "Archived" => &mut e.draft.archived,
            _ => &mut e.draft.is_default,
        };
        widgets::toggle_row(cx, PAD + 4.0, ry, W - 8.0, Some((glyph, p.ink)), words, sub, on);
        ry += widgets::row_h(sub.is_some());
    }
    // An archived account is never the one new entries start in.
    if archived || e.draft.archived {
        e.draft.is_default = false;
    }
    y += ch + 20.0;

    if e.stored.is_some() && e.draft.kind == AccountType::Investment {
        y = values_panel(cx, e, fmt, today, y, act);
    }
    if let Some(id) = e.id {
        if mk::action_row(cx, y, "list", "See its entries", false) {
            *act = Some(Act::Go(Route::Transactions { category: None, account: Some(id) }));
        }
        y += mk::ACTION_H + 10.0;
        if mk::action_row(cx, y, "trash", "Delete account", true) {
            *act = Some(Act::AskDelete);
        }
        y += mk::ACTION_H;
    }
    y
}

fn values_panel(cx: &mut Ctx, e: &mut Editor, fmt: &MoneyFormatter, today: relay_money::Date, y: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = y;
    y += mk::group_head(cx, y, "Values", None);
    let rows = e.values.len() as f32;
    let label_h = Text::new(ty::CAPTION, p.ink).lh() + 4.0;
    let field_h = label_h + 48.0;
    let h = 18.0 + field_h + 10.0 + 17.0 + if rows > 0.0 { 10.0 + rows * 44.0 } else { 0.0 } + 14.0;
    kit::card(cx, PAD, y, W, h);
    let mut ry = y + 18.0;
    let kw = 96.0;
    let fw = W - 36.0 - kw - 10.0;
    widgets::field(cx, "account.value", PAD + 18.0, ry, fw, "Worth today", &fmt.format(0), &mut e.value_text, true);
    let ok = widgets::parse_minor(&e.value_text, fmt.fraction_digits).is_some();
    let kx = PAD + 18.0 + fw + 10.0;
    let ky = ry + label_h;
    if ok {
        if kit::key(cx, kx, ky, kw, 48.0, "Record", None, KeyKind::Primary) {
            *act = Some(Act::Record);
        }
    } else {
        cx.c.rect(kx, ky, kw, 48.0, kit::KEY_R, p.card2);
        let t = Text::new(ty::BODY, p.ink3).semi().center();
        cx.c.text("Record", kx + kw / 2.0, ky + (48.0 - t.lh()) / 2.0, t);
    }
    ry += field_h + 10.0;
    cx.c.text_fit("The balance follows the newest value", PAD + 20.0, ry, W - 40.0, Text::new(ty::CAPTION, p.ink3));
    ry += 17.0 + 10.0;
    for (i, v) in e.values.iter().enumerate() {
        if i > 0 {
            cx.c.rect(PAD + 18.0, ry, W - 36.0, 1.0, 0.0, p.line_subtle);
        }
        let t = Text::new(14.0, p.ink2);
        cx.c.text(&store::value_day(&v.date, today), PAD + 20.0, ry + (44.0 - t.lh()) / 2.0, t);
        let at = Text::new(15.0, p.ink).medium().right();
        cx.c.text(&fmt.format(v.value), PAD + W - 18.0 - 34.0, ry + (44.0 - at.lh()) / 2.0, at);
        cx.c.icon("close", PAD + W - 18.0 - 10.0, ry + 22.0, 14.0, p.ink3);
        if cx.tap(PAD + W - 18.0 - 30.0, ry, 40.0, 44.0) {
            *act = Some(Act::RemoveValue(v.id));
        }
        ry += 44.0;
    }
    y + h + 20.0
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn edit_act(app: &mut App, a: Act) {
    let digits = app.store.formatter().fraction_digits;
    let Some(e) = app.st.accounts.edit.as_mut() else { return };
    match a {
        Act::Back => {
            app.pop();
        }
        Act::Go(r) => app.push(r),
        Act::Save => {
            let (n, o) = store::problems(&e.draft, digits);
            if n.is_some() || o.is_some() {
                e.show_errors = true;
                return;
            }
            let (id, draft) = (e.id, e.draft.clone());
            app.finish_typing();
            match store::save(&mut app.store, id, &draft, digits) {
                Ok(_) => {
                    app.invalidate();
                    app.pop();
                }
                Err(err) => app.say(err, None),
            }
        }
        Act::AskDelete => {
            if let Some(id) = e.id {
                match store::account_use(&app.store, id) {
                    Ok(u) => e.confirm = Some(u),
                    Err(err) => app.say(err, None),
                }
            }
        }
        Act::CloseDialog => e.confirm = None,
        Act::Delete => {
            let (Some(id), Some(u)) = (e.id, e.confirm.take()) else { return };
            let name = e.stored.as_ref().map(|s| s.row.view.name.clone()).unwrap_or_default();
            match store::delete(&mut app.store, id) {
                Ok(()) => {
                    app.invalidate();
                    app.say(store::deleted_line(&name, &u), None);
                    app.pop();
                }
                Err(err) => app.say(err, None),
            }
        }
        Act::Archive => {
            e.confirm = None;
            let Some(id) = e.id else { return };
            let name = e.stored.as_ref().map(|s| s.row.view.name.clone()).unwrap_or_default();
            match store::set_archived(&mut app.store, id, true) {
                Ok(()) => {
                    app.invalidate();
                    app.say(format!("{name} archived · its entries and bills stay"), None);
                    app.pop();
                }
                Err(err) => app.say(err, None),
            }
        }
        Act::Record => {
            let Some(id) = e.id else { return };
            let Some(v) = widgets::parse_minor(&e.value_text, digits) else { return };
            e.value_text.clear();
            app.finish_typing();
            match app.store.ledger.value_set(id, &model::today().to_string(), v) {
                Ok(_) => {
                    app.invalidate();
                    app.say("Value recorded", None);
                }
                Err(err) => app.say(err.to_string(), None),
            }
        }
        Act::RemoveValue(vid) => match store::delete_value(&app.store, vid) {
            Ok(()) => {
                app.invalidate();
                app.say("Value removed", None);
            }
            Err(err) => app.say(err, None),
        },
    }
}
