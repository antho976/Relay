//! Categories and the category editor (Kotlin: ui/categories/CategoryScreens.kt), in the
//! premium look.
//!
//! Categories shows one kind at a time (spending or income) as a three-column grid of tiles with
//! this period's money in each, a dashed New tile last. A tap opens its entries; holding it opens
//! the editor. The editor names a category, picks its colour (the palette's own shades) and icon,
//! archives it, and deletes it, asking first where its entries and bills go when it has any.

use std::collections::HashMap;

use relay_money::copy;
use relay_money::icon_hints;
use relay_money::model::CategoryKind;
use relay_money::money::MoneyFormatter;
use relay_money::views::CategoryView;

use crate::accounts_kit::{self as mk, W};
use crate::app::App;
use crate::categories_kit::{self as ck, MoveRow};
use crate::categories_store::{self as store, CategoriesState, CategoryUse};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Ctx, PAD, ty};
use crate::model;
use crate::nav::Route;
use crate::theme;
use crate::ui::Scroll;
use crate::widgets;

/// How long a finger rests on a tile before it opens the editor.
const HOLD_NS: u64 = 450_000_000;

#[derive(Default)]
pub struct State {
    list: Option<(u64, Result<CategoriesState, String>)>,
    /// The lens: income rather than spending.
    income: bool,
    /// Where a finger came to rest, and when, for the hold that edits.
    press: Option<((f32, f32), u64)>,
    edit: Option<Editor>,
}

struct Editor {
    id: Option<i64>,
    kind: CategoryKind,
    stored_name: String,
    entries: i64,
    period_total: i64,
    name: String,
    icon: String,
    color: i64,
    archived: bool,
    /// The owner picked the icon; until then a new category's name suggests one.
    icon_chosen: bool,
    show_errors: bool,
    all: Vec<CategoryView>,
    /// How many entries each category holds, for the move sheet.
    counts: HashMap<i64, i64>,
    /// What the delete touches, while the move sheet is open.
    moving: Option<CategoryUse>,
    sheet: Scroll,
}

enum Act {
    Go(Route),
    Back,
    Lens(bool),
    Save,
    AskDelete,
    Move(Option<i64>),
    CloseSheet,
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let at = app.ledger_gen;
    if app.fresh || app.st.categories.list.as_ref().is_none_or(|(g, _)| *g != at) {
        app.st.categories.list = Some((at, store::read_state(&app.store, model::today())));
    }
    match app.route().clone() {
        Route::CategoryEdit { id, kind } => editor(app, dt, id, kind),
        _ => list(app, dt),
    }
}

// The list

fn list(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let now = app.now_ns;
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.categories;
        // A finger resting long enough on one spot is a hold.
        let held = match (cx.ui.down, st.press) {
            (Some(d), Some((at, since))) if at == d => (now.saturating_sub(since) >= HOLD_NS).then_some(d),
            (Some(d), _) => {
                st.press = Some((d, now));
                None
            }
            (None, _) => {
                st.press = None;
                None
            }
        };
        let kind = if st.income { CategoryKind::Income } else { CategoryKind::Expense };
        let Some((_, read)) = &st.list else { return pg.moving };
        bottom = list_content(&mut cx, read, kind, &fmt, held, pg.top, &mut act);
    }
    app.page_end(&pg, bottom);
    match act {
        Some(Act::Go(r)) => app.push(r),
        Some(Act::Back) => {
            app.pop();
        }
        Some(Act::Lens(income)) => app.st.categories.income = income,
        _ => {}
    }
    // Keep drawing while a finger rests, so a hold is seen without it moving.
    pg.moving || app.st.categories.press.is_some()
}

#[allow(clippy::too_many_arguments)]
fn list_content(cx: &mut Ctx, read: &Result<CategoriesState, String>, kind: CategoryKind, fmt: &MoneyFormatter, held: Option<(f32, f32)>, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = top;
    let (back, plus, h) = widgets::page_header(cx, y, "Categories", Some("plus"));
    if back {
        *act = Some(Act::Back);
    }
    if plus {
        *act = Some(Act::Go(Route::CategoryEdit { id: None, kind }));
    }
    y += h;
    let selected = if kind == CategoryKind::Income { 1 } else { 0 };
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Spending", "Income"], selected) {
        *act = Some(Act::Lens(i == 1));
    }
    y += 44.0 + 18.0;
    let (s, period) = match read {
        Ok(st) => (st.of(kind), st.period.clone()),
        Err(e) => {
            y += cx.c.para(e, PAD, y, W, Text::new(ty::UI, p.held_text), 4);
            return y;
        }
    };

    // "November so far", with the kind's total for it on the right.
    let ct = Text::new(13.0, p.ink2);
    cx.c.text(&format!("{period} so far"), PAD + 2.0, y, ct);
    if s.period_total > 0 {
        let words = format!("{} across {}", fmt.format_whole(s.period_total), copy::plural_as(s.used_this_period, "category", "categories"));
        cx.c.text(&words, PAD + W - 2.0, y, Text::new(13.0, p.ink3).right());
    }
    y += ct.lh() + 12.0;

    if s.active.is_empty() {
        let body = if kind == CategoryKind::Income { "Add one for what comes in: a salary, a refund, a side job." } else { "Add one for what you spend on: groceries, rent, coffee." };
        y += widgets::empty(cx, PAD, y, W, "tag", "No categories yet", body) + 14.0;
    }
    // Three to a row, the dashed New tile last.
    let gap = 10.0;
    let tw = (W - gap * 2.0) / 3.0;
    let th = ck::TILE_H;
    let n = s.active.len() + 1;
    for i in 0..n {
        let x = PAD + (i % 3) as f32 * (tw + gap);
        let ty_ = y + (i / 3) as f32 * (th + gap);
        match s.active.get(i) {
            Some(c) => {
                let screen_y = ty_ + cx.c.dy;
                if held.is_some_and(|(hx, hy)| hx >= x && hx < x + tw && hy >= screen_y && hy < screen_y + th) {
                    *act = Some(Act::Go(Route::CategoryEdit { id: Some(c.id), kind }));
                }
                if ck::category_tile(cx, x, ty_, tw, th, &c.name, &c.icon, c.color, &fmt.format_whole(c.period_total)) {
                    *act = Some(Act::Go(Route::Transactions { category: Some(c.id), account: None }));
                }
            }
            None => {
                if ck::new_tile(cx, x, ty_, tw, th, "New") {
                    *act = Some(Act::Go(Route::CategoryEdit { id: None, kind }));
                }
            }
        }
    }
    y += n.div_ceil(3) as f32 * (th + gap) - gap + 26.0;

    if !s.archived.is_empty() {
        y += mk::group_head(cx, y, "Archived", None);
        let lines = &s.archived;
        let (h, hit) = mk::row_card(cx, y, lines.len(), |cx, i, ry| {
            let c = &lines[i];
            let rx = PAD + 16.0;
            if cx.pressed(PAD + 4.0, ry + 4.0, W - 8.0, mk::ROW_H - 8.0) {
                let wash = cx.p.wash;
                cx.c.rect(PAD + 4.0, ry + 4.0, W - 8.0, mk::ROW_H - 8.0, 18.0, wash);
            }
            kit::badge(cx, rx, ry + (mk::ROW_H - 36.0) / 2.0, 36.0, &c.icon, theme::category_color(Some(c.color)));
            let tx = rx + 36.0 + 14.0;
            let right = PAD + W - 16.0;
            let mut room = right - tx;
            if c.period_total > 0 {
                let a = fmt.format_whole(c.period_total);
                let at = Text::new(16.0, p.ink2).medium().right();
                room -= cx.c.measure(&a, at) + 12.0;
                cx.c.text(&a, right, ry + 13.0, at);
            }
            cx.c.text_fit(&c.name, tx, ry + 13.0, room, Text::new(16.0, p.ink).medium());
            cx.c.text_fit(&store::tile_line(c), tx, ry + 36.0, right - tx, Text::new(13.0, p.ink2));
            cx.tap(PAD, ry, W, mk::ROW_H)
        });
        if let Some(i) = hit {
            *act = Some(Act::Go(Route::CategoryEdit { id: Some(lines[i].id), kind }));
        }
        y += h + 22.0;
    }
    cx.c.text("Tap a category for its entries; hold it to edit.", WIDTH / 2.0, y, Text::new(ty::CAPTION, p.ink3).center());
    y + 20.0
}

// The editor

fn open_editor(app: &App, id: Option<i64>, kind: CategoryKind) -> Editor {
    let all = app.store.ledger.categories().unwrap_or_default();
    let counts = store::entry_counts(&app.store).unwrap_or_default();
    let stored = id.and_then(|id| all.iter().find(|c| c.id == id).cloned());
    let line = stored.as_ref().and_then(|c| {
        app.st.categories.list.as_ref().and_then(|(_, r)| r.as_ref().ok()).and_then(|s| {
            let k = s.of(c.kind);
            k.active.iter().chain(&k.archived).find(|l| l.id == c.id).cloned()
        })
    });
    match stored {
        Some(c) => Editor {
            id: Some(c.id),
            kind: c.kind,
            stored_name: c.name.clone(),
            entries: line.as_ref().map(|l| l.entries).unwrap_or(0),
            period_total: line.as_ref().map(|l| l.period_total).unwrap_or(0),
            name: c.name,
            icon: c.icon,
            color: c.color,
            archived: c.archived,
            icon_chosen: true,
            show_errors: false,
            all,
            counts: counts.clone(),
            moving: None,
            sheet: Scroll::default(),
        },
        None => {
            let used: Vec<i64> = all.iter().filter(|c| c.kind == kind).map(|c| c.color).collect();
            Editor {
                id: None,
                kind,
                stored_name: String::new(),
                entries: 0,
                period_total: 0,
                name: String::new(),
                icon: "dots".into(),
                color: store::suggest_color(&used),
                archived: false,
                icon_chosen: false,
                show_errors: false,
                all,
                counts,
                moving: None,
                sheet: Scroll::default(),
            }
        }
    }
}

fn editor(app: &mut App, dt: f32, id: Option<i64>, kind: CategoryKind) -> bool {
    if app.fresh || app.st.categories.edit.is_none() {
        let e = open_editor(app, id, kind);
        app.st.categories.edit = Some(e);
    }
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    let mut sheet_moving = false;
    {
        let mut cx = cx!(app);
        let Some(e) = app.st.categories.edit.as_mut() else { return pg.moving };
        let problem = store::name_problem(&e.name, e.kind, e.id, &e.all);
        // A clash shows as you type; a missing name only once a save was tried.
        let shown = problem.clone().filter(|p| p != store::MISSING_NAME || e.show_errors);
        let sheet_open = e.moving.is_some();
        let held = if sheet_open { widgets::hold(&mut cx) } else { None };
        bottom = edit_form(&mut cx, e, &fmt, shown.as_deref(), pg.top, &mut act);
        if let Some(u) = &e.moving {
            widgets::release(&mut cx, held);
            let budget = u.budget.map(|b| fmt.format(b));
            let targets: Vec<&CategoryView> = e.all.iter().filter(|c| c.kind == e.kind && Some(c.id) != e.id && !c.archived).collect();
            let counts: Vec<String> = targets.iter().map(|t| copy::plural_as(e.counts.get(&t.id).copied().unwrap_or(0), "entry", "entries")).collect();
            let mut rows: Vec<MoveRow> = targets.iter().zip(&counts).map(|(t, n)| MoveRow { title: &t.name, sub: n, badge: Some((&t.icon, t.color)) }).collect();
            rows.push(MoveRow { title: store::leave_title(u), sub: store::leave_subtitle(u), badge: None });
            let note = store::move_note(&e.stored_name, budget.as_deref());
            sheet_moving = true;
            match ck::move_sheet(&mut cx, &store::move_heading(u), &note, &rows, &mut e.sheet, dt) {
                Some(Some(i)) if i < targets.len() => act = Some(Act::Move(Some(targets[i].id))),
                Some(Some(_)) => act = Some(Act::Move(None)),
                Some(None) => act = Some(Act::CloseSheet),
                None => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    if let Some(a) = act {
        edit_act(app, a, &fmt);
    }
    pg.moving || sheet_moving
}

fn edit_form(cx: &mut Ctx, e: &mut Editor, fmt: &MoneyFormatter, problem: Option<&str>, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let mut y = top;
    let (title, sub) = match e.id {
        None => (format!("New {} category", store::kind_label(e.kind).to_lowercase()), None),
        Some(_) => (e.stored_name.clone(), Some(format!("{} · {} this month", copy::plural_as(e.entries, "entry", "entries"), fmt.format_whole(e.period_total)))),
    };
    let (back, save, h) = widgets::page_header(cx, y, &title, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::Save);
    }
    y += h;
    if let Some(s) = &sub {
        cx.c.text_fit(s, WIDTH / 2.0, y - 10.0, W, Text::new(13.0, p.ink2).center());
        y += 28.0;
    }

    // The badge as it will look, beside the name.
    let label_h = Text::new(ty::CAPTION, p.ink).lh() + 4.0;
    kit::badge(cx, PAD, y + label_h - 2.0, 52.0, &e.icon, theme::category_color(Some(e.color)));
    let before = e.name.clone();
    y += widgets::field(cx, "category.name", PAD + 52.0 + 12.0, y, W - 52.0 - 12.0, "Name", "Groceries, Rent, Salary", &mut e.name, false);
    if e.name != before && e.id.is_none() && !e.icon_chosen {
        e.icon = icon_hints::suggest(&e.name).unwrap_or("dots").to_string();
    }
    if let Some(pr) = problem {
        y += mk::hint(cx, PAD + 52.0 + 12.0, y, W - 64.0, pr, true);
    }
    y += 24.0;

    y += mk::group_head(cx, y, "Colour", None);
    let (picked, sh) = ck::swatches(cx, PAD, y, W, e.color);
    if let Some(c) = picked {
        e.color = c;
    }
    y += sh + 24.0;

    for (group, keys) in store::ICON_GROUPS {
        y += mk::group_head(cx, y, group, None);
        let (picked, gh) = ck::icon_choices(cx, PAD, y, W, keys, e.color, &e.icon);
        if let Some(k) = picked {
            e.icon = k.to_string();
            e.icon_chosen = true;
        }
        y += gh + 24.0;
    }

    if let Some(id) = e.id {
        let sub = Some("Entries kept, out of the grid");
        let rh = widgets::row_h(true) + 8.0;
        kit::card(cx, PAD, y, W, rh);
        widgets::toggle_row(cx, PAD + 4.0, y + 4.0, W - 8.0, Some(("save", p.ink)), "Archived", sub, &mut e.archived);
        y += rh + 10.0;
        if mk::action_row(cx, y, "list", "See its entries", false) {
            *act = Some(Act::Go(Route::Transactions { category: Some(id), account: None }));
        }
        y += mk::ACTION_H + 10.0;
        if mk::action_row(cx, y, "trash", "Delete category", true) {
            *act = Some(Act::AskDelete);
        }
        y += mk::ACTION_H;
    }
    y
}

fn edit_act(app: &mut App, a: Act, fmt: &MoneyFormatter) {
    let Some(e) = app.st.categories.edit.as_mut() else { return };
    match a {
        Act::Back => {
            app.pop();
        }
        Act::Go(r) => app.push(r),
        Act::Lens(_) => {}
        Act::Save => {
            if store::name_problem(&e.name, e.kind, e.id, &e.all).is_some() {
                e.show_errors = true;
                return;
            }
            let r = match e.id {
                None => store::add(&app.store, &e.name, e.kind, &e.icon, e.color).map(|_| ()),
                Some(id) => store::update(&app.store, id, &e.name, &e.icon, e.color, e.archived),
            };
            app.finish_typing();
            match r {
                Ok(()) => {
                    app.invalidate();
                    app.pop();
                }
                Err(err) => app.say(err, None),
            }
        }
        Act::AskDelete => {
            let Some(id) = e.id else { return };
            match store::category_use(&app.store, id) {
                Ok(u) if u.has_movable() => {
                    e.moving = Some(u);
                    e.sheet = Scroll::default();
                }
                // Nothing filed under it: it goes at once.
                Ok(u) => remove(app, id, None, u, fmt),
                Err(err) => app.say(err, None),
            }
        }
        Act::CloseSheet => e.moving = None,
        Act::Move(to) => {
            let (Some(id), Some(u)) = (e.id, e.moving.take()) else { return };
            remove(app, id, to, u, fmt);
        }
    }
}

fn remove(app: &mut App, id: i64, to: Option<i64>, u: CategoryUse, fmt: &MoneyFormatter) {
    let Some(e) = app.st.categories.edit.as_ref() else { return };
    let to_name = to.and_then(|t| e.all.iter().find(|c| c.id == t)).map(|c| c.name.clone());
    let budget = u.budget.map(|b| fmt.format(b));
    let line = store::deleted_line(&e.stored_name, &u, budget.as_deref(), to_name.as_deref());
    match store::delete(&app.store, id, to) {
        Ok(()) => {
            app.invalidate();
            app.say(line, None);
            app.pop();
        }
        Err(err) => app.say(err, None),
    }
}
