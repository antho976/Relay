//! Plan: budgets, bills and goals, each with its editor, as the Kotlin app's ui/plan
//! (PlanScreens.kt, PlanEditors.kt), drawn in the premium look (PORTING.md, "The premium look"):
//! each page is pushed, opens on a picture card, and lists its rows plainly under it. Budgets are
//! read and set through relay-money; bills, goals and contributions through `plan_store`. Every
//! page reads [`PlanData`] once per change to the ledger (`app.ledger_gen`), and an editor reads
//! its row once, when it opens.
//!
//! Deleting a bill, a goal, a contribution or a budget happens at once, with Undo in the Plan's
//! own toast (the app's toast can only undo an entry).

use relay_money::Date;
use relay_money::amount_input::AmountInput;
use relay_money::copy;
use relay_money::model::{AccountType, CategoryKind, GoalKind, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::pace::PaceReading;
use relay_money::period::BudgetPeriod;
use relay_money::recurrence::Frequency;
use relay_money::views::{AccountView, CategoryView};

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, CW, Ctx, KeyKind, PAD, PicCard, ty};
use crate::model;
use crate::nav::Route;
use crate::plan_kit::{self as pk, W};
use crate::plan_logic::{self as logic, GoalLine, GoalPace, PlanData};
use crate::plan_store::{self as store, Bill, BillInput, Contribution, DeletedGoal, Goal, GoalInput};
use crate::theme;
use crate::ui::Scroll;
use crate::widgets::{self, SheetOut};

#[derive(Default)]
pub struct State {
    data: Option<PlanData>,
    /// The ledger generation `data` was read at.
    read_gen: u64,
    problem: Option<String>,
    sheet: Option<Sheet>,
    sheet_scroll: Scroll,
    sheet_month: Option<Date>,
    budget: Option<BudgetEditor>,
    bill: Option<BillEditor>,
    goal: Option<GoalPage>,
    goal_edit: Option<GoalEditor>,
    undo: Option<Undo>,
    /// A goal opened from its "Add money" button: its page opens with the amount field typing.
    add_money: Option<i64>,
}

/// The sheet open over a Plan page.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Sheet {
    AddBudget,
    Account,
    ToAccount,
    Date,
    GoalAccount,
    /// A goal's more key: edit it or delete it.
    GoalMore,
}

struct Undo {
    words: String,
    act: UndoAct,
    until_ns: u64,
}

enum UndoAct {
    /// Budgets to set back: (category, amount; 0 removes).
    Budgets(Vec<(Option<i64>, i64)>),
    Bill(i64),
    Goal(DeletedGoal),
    Contribution(i64),
}

/// What a tap asked for, done once drawing is over.
enum Act {
    Go(Route),
    Back,
    Sheet(Option<Sheet>),
    ApplySuggestions,
    SaveBudget,
    RemoveBudget,
    SaveBill,
    DeleteBill,
    SaveGoal,
    DeleteGoal(i64),
    AddMoney(i64),
    Contribute { out: bool },
    RemoveContribution(i64),
}

const NAME: &str = "plan.bill.name";
const AMOUNT: &str = "plan.bill.amount";
const GOAL_NAME: &str = "plan.goal.name";
const GOAL_TARGET: &str = "plan.goal.target";
const PUT_AMOUNT: &str = "plan.goal.amount";
const PUT_NOTE: &str = "plan.goal.note";

pub fn draw(app: &mut App, dt: f32) -> bool {
    refresh(app);
    let route = app.route().clone();
    let toast_bottom = app.canvas.h - app.canvas.inset_bottom - 8.0;
    let live = app.st.plan.undo.as_ref().is_some_and(|u| app.now_ns <= u.until_ns);
    if !live {
        app.st.plan.undo = None;
    }
    // The Undo pill takes its own taps before the page under it.
    let toast_tap = match app.ui.tap {
        Some((x, y)) if live && (16.0..WIDTH - 16.0).contains(&x) && (toast_bottom - 60.0..toast_bottom - 12.0).contains(&y) => app.ui.tap.take(),
        _ => None,
    };
    let moving = match route {
        Route::Budgets => budgets_page(app, dt),
        Route::BudgetEdit { category } => budget_edit_page(app, dt, category),
        Route::Bills => bills_page(app, dt),
        Route::BillEdit { id } => bill_edit_page(app, dt, id),
        Route::Goals => goals_page(app, dt),
        Route::Goal { id } => goal_page(app, dt, id),
        Route::GoalEdit { id } => goal_edit_page(app, dt, id),
        _ => false,
    };
    if live && app.route() == &route && !app.keyboard_up {
        let words = app.st.plan.undo.as_ref().map(|u| u.words.clone()).unwrap_or_default();
        let undone = {
            let mut cx = cx!(app);
            cx.ui.tap = toast_tap;
            pk::undo_toast(&mut cx, toast_bottom, &words)
        };
        if undone && let Some(u) = app.st.plan.undo.take() {
            undo(app, u.act);
        }
    }
    moving || live
}

fn is_plan(r: &Route) -> bool {
    matches!(r, Route::Budgets | Route::BudgetEdit { .. } | Route::Bills | Route::BillEdit { .. } | Route::Goals | Route::Goal { .. } | Route::GoalEdit { .. })
}

/// Reads the Plan's figures again when the page opens, the ledger changed or the day turned.
fn refresh(app: &mut App) {
    let today = model::today();
    let st = &mut app.st.plan;
    let stale = st.data.as_ref().is_none_or(|d| d.today != today);
    if app.fresh || stale || st.read_gen != app.ledger_gen {
        match logic::load(&app.store, today) {
            Ok(d) => {
                st.data = Some(d);
                st.problem = None;
            }
            Err(e) => st.problem = Some(e),
        }
        st.read_gen = app.ledger_gen;
    }
    if app.fresh {
        st.sheet = None;
    }
}

fn say_undo(app: &mut App, words: String, act: UndoAct) {
    if is_plan(app.route()) {
        app.st.plan.undo = Some(Undo { words, act, until_ns: app.now_ns + 5_000_000_000 });
    } else {
        // Off the Plan its toast is not drawn: say it plainly.
        app.say(words, None);
    }
}

fn undo(app: &mut App, act: UndoAct) {
    let result = match act {
        UndoAct::Budgets(list) => list.into_iter().try_for_each(|(c, a)| app.store.ledger.budget_set(c, a).map_err(|e| e.to_string())),
        UndoAct::Bill(id) => app.store.sql().and_then(|c| store::bill_restore(c, id)),
        UndoAct::Goal(d) => app.store.sql().and_then(|c| store::goal_restore(c, &d)),
        UndoAct::Contribution(id) => app.store.sql().and_then(|c| store::contribution_restore(c, id)),
    };
    match result {
        Ok(()) => app.invalidate(),
        Err(e) => app.say(format!("Could not undo: {e}"), None),
    }
}

/// While the Plan cannot be read: the header, and what is wrong.
fn problem(cx: &mut Ctx, top: f32, title: &str, words: Option<&str>, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let (back, _, hh) = widgets::page_header(cx, top, title, None);
    if back {
        *act = Some(Act::Back);
    }
    cx.c.para(words.unwrap_or("Opening the ledger"), PAD, top + hh + 8.0, W, Text::new(ty::UI, p.ink2), 4);
    top + hh + 48.0
}

/// The empty state with its key under the words. Returns (height, key tapped).
fn empty_key(cx: &mut Ctx, y: f32, glyph: &'static str, title: &str, body: &str, key: &str) -> (f32, bool) {
    let p = cx.p;
    let bt = Text::new(ty::UI, p.ink2).center().line(1.4);
    let bh = cx.c.para_h(body, W - 48.0, bt, 4);
    let h = 24.0 + 48.0 + 12.0 + 22.0 + 6.0 + bh + 18.0 + 48.0 + 24.0;
    kit::card(cx, PAD, y, W, h);
    cx.c.rect(WIDTH / 2.0 - 24.0, y + 24.0, 48.0, 48.0, 15.0, p.card2);
    cx.c.icon(glyph, WIDTH / 2.0, y + 48.0, 22.0, p.ink2);
    cx.c.text(title, WIDTH / 2.0, y + 84.0, Text::new(ty::TITLE, p.ink).semi().center());
    cx.c.para(body, PAD + 24.0, y + 112.0, W - 48.0, bt, 4);
    let t = Text::new(ty::BODY, p.ink).semi();
    let kw = cx.c.measure(key, t) + 26.0 + 44.0;
    let tapped = kit::key(cx, WIDTH / 2.0 - kw / 2.0, y + h - 24.0 - 48.0, kw, 48.0, key, Some("plus"), KeyKind::Plain);
    (h, tapped)
}

/// Gives a field the keyboard, empty, as a tap on it would.
fn focus_field(cx: &mut Ctx, id: &'static str, numeric: bool) {
    cx.focus.id = Some(id);
    cx.focus.text.clear();
    cx.focus.numeric = numeric;
    cx.focus.at = None;
    cx.host.keyboard(true, "", numeric);
}

// Budgets

fn budgets_page(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let State { data, problem: prob, sheet, sheet_scroll, .. } = &mut app.st.plan;
        let held = if sheet.is_some() { widgets::hold(&mut cx) } else { None };
        bottom = match data.as_ref() {
            None => problem(&mut cx, pg.top, "Budgets", prob.as_deref(), &mut act),
            Some(d) => budgets_body(&mut cx, d, pg.top, &mut act),
        };
        if *sheet == Some(Sheet::AddBudget)
            && let Some(d) = data.as_ref()
        {
            widgets::release(&mut cx, held);
            let budgeted: Vec<i64> = d.budgets.envelopes.iter().map(|e| e.category_id).collect();
            let cats: Vec<&CategoryView> = d.categories.iter().filter(|c| c.kind == CategoryKind::Expense && !c.archived && !budgeted.contains(&c.id)).collect();
            let items: Vec<(String, Option<String>)> = if cats.is_empty() {
                vec![("Every spending category has a budget.".into(), None)]
            } else {
                cats.iter().map(|c| (c.name.clone(), None)).collect()
            };
            match widgets::sheet_list(&mut cx, "Budget a category", &items, None, sheet_scroll, dt) {
                SheetOut::Pick(i) => act = Some(cats.get(i).map_or(Act::Sheet(None), |c| Act::Go(Route::BudgetEdit { category: Some(c.id) }))),
                SheetOut::Close => act = Some(Act::Sheet(None)),
                SheetOut::Nothing => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    let open = app.st.plan.sheet.is_some();
    perform(app, act);
    pg.moving || open
}

fn budgets_body(cx: &mut Ctx, d: &PlanData, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &d.fmt;
    let b = &d.budgets;
    let (back, add, hh) = widgets::page_header(cx, top, "Budgets", Some("plus"));
    if back {
        *act = Some(Act::Back);
    }
    if add {
        *act = Some(Act::Sheet(Some(Sheet::AddBudget)));
    }
    let mut y = top + hh + 4.0;

    // The month's limit on the piggy bank: what is left, how it is going.
    let days_left = d.period.days_left(d.today);
    let eyebrow = format!("{} budgets", logic::period_name(&d.period, d.today));
    let ch = if b.overall.is_some() { 300.0 } else { 340.0 };
    let tapped = match &b.overall {
        Some(r) => {
            let (title, _) = logic::left_words(r, fmt);
            let line = logic::budgets_hero_line(r, fmt, days_left);
            let spec = PicCard { eyebrow: Some(&eyebrow), title: &title, title_size: 34.0, line: Some(&line), meter: Some(r.spent_fraction), button: None };
            let (card, _) = kit::pic_card(cx, PAD, y, CW, ch, "piggy", &spec);
            card
        }
        None => {
            let title = format!("{} spent", fmt.format_whole(b.spent));
            let spec = PicCard {
                eyebrow: Some(&eyebrow),
                title: &title,
                title_size: 34.0,
                line: Some("Set a monthly limit and Tally reads every day of the month against it."),
                meter: None,
                button: Some("Set a limit"),
            };
            let (card, button) = kit::pic_card(cx, PAD, y, CW, ch, "piggy", &spec);
            card || button
        }
    };
    if tapped {
        *act = Some(Act::Go(Route::BudgetEdit { category: None }));
    }
    y += ch + 12.0;

    // The month in figures, right under its card.
    let projected = match b.projected {
        Some(v) => (fmt.format_whole(v), "by the month's end at this pace".to_string()),
        None => ("Not yet".to_string(), format!("It reads after the first {} days", logic::PROJECT_AFTER_DAYS)),
    };
    let items = [
        ("Spent", fmt.format_whole(b.spent), copy::versus_last_line(b.spent, b.last_spent, fmt)),
        ("Projected", projected.0, projected.1),
        ("In budgets", fmt.format_whole(b.budgeted), copy::plural_as(b.envelopes.len() as i64, "category", "categories")),
        ("Without a budget", fmt.format_whole(b.unbudgeted_spent), copy::plural_as(b.unbudgeted.len() as i64, "category", "categories")),
    ];
    y += pk::figures(cx, y, &items) + 22.0;

    // Each category's budget, furthest over pace first.
    if !b.envelopes.is_empty() {
        let (h, hit) = pk::row_card(cx, y, b.envelopes.len(), pk::BUDGET_ROW_H, pk::BUDGET_TEXT_X, |cx, i, x, ry, w| pk::budget_line(cx, x, ry, w, &b.envelopes[i], fmt));
        if let Some(i) = hit {
            *act = Some(Act::Go(Route::BudgetEdit { category: Some(b.envelopes[i].category_id) }));
        }
        y += h + 30.0;
    } else if b.suggestions.is_empty() {
        let (h, tapped) = empty_key(cx, y, "pie", "No category budgets yet", "Give a category its own monthly limit, and its pace shows here and on Overview.", "Budget a category");
        if tapped {
            *act = Some(Act::Sheet(Some(Sheet::AddBudget)));
        }
        y += h + 30.0;
    }

    if !b.suggestions.is_empty() {
        let all = format!("Set all {}", fmt.format_whole(b.suggested_total()));
        let (sh, tapped) = kit::section(cx, y, "Suggested", Some(&all));
        if tapped {
            *act = Some(Act::ApplySuggestions);
        }
        y += sh + 4.0;
        cx.c.text("From what each category took in the months before", PAD, y, Text::new(13.0, p.ink3));
        y += 28.0;
        let (h, hit) = pk::row_card(cx, y, b.suggestions.len(), pk::ROW_H, pk::ROW_TEXT_X, |cx, i, x, ry, w| {
            let s = &b.suggestions[i];
            let sub = format!("Average of {}", copy::plural(s.months as i64, "month"));
            pk::badge_row(cx, x, ry, w, &s.icon, theme::category_color(Some(s.color)), &s.name, &sub, &fmt.format_whole(s.amount), p.ink)
        });
        if let Some(i) = hit {
            *act = Some(Act::Go(Route::BudgetEdit { category: Some(b.suggestions[i].category_id) }));
        }
        y += h + 30.0;
    }

    if !b.unbudgeted.is_empty() {
        let (sh, _) = kit::section(cx, y, "Without a budget", None);
        y += sh + 12.0;
        let (h, hit) = pk::row_card(cx, y, b.unbudgeted.len(), pk::ROW_H, pk::ROW_TEXT_X, |cx, i, x, ry, w| {
            let u = &b.unbudgeted[i];
            let sub = format!("{} · tap to give it a budget", copy::plural_as(u.count, "entry", "entries"));
            pk::badge_row(cx, x, ry, w, &u.icon, theme::category_color(Some(u.color)), &u.name, &sub, &fmt.format_whole(u.spent), p.ink)
        });
        if let Some(i) = hit {
            *act = Some(Act::Go(Route::BudgetEdit { category: Some(b.unbudgeted[i].category_id) }));
        }
        y += h + 30.0;
    }

    y
}

/// One monthly limit, typed on the keypad, against this month and the three before.
struct BudgetEditor {
    category: Option<i64>,
    name: String,
    icon: String,
    color: i64,
    existing: Option<i64>,
    input: AmountInput,
    /// This period and the three before, oldest first: (first day, spent).
    history: Vec<(Date, i64)>,
    period: BudgetPeriod,
    start_day: i64,
    today: Date,
}

impl BudgetEditor {
    fn open(app: &App, category: Option<i64>) -> Result<BudgetEditor, String> {
        let today = model::today();
        let (period, start_day) = logic::period_of(&app.store, today);
        let cat = match category {
            Some(id) => app.store.ledger.categories().map_err(|e| e.to_string())?.into_iter().find(|c| c.id == id),
            None => None,
        };
        let existing = app.store.ledger.budget(category).map_err(|e| e.to_string())?;
        let digits = app.store.formatter().fraction_digits;
        let input = existing.map_or_else(|| AmountInput::empty(digits), |a| AmountInput::of(a, digits));
        let history = logic::budget_history(app.store.sql()?, &period, category)?;
        Ok(BudgetEditor {
            category,
            name: match (&cat, category) {
                (Some(c), _) => c.name.clone(),
                (None, None) => "Monthly budget".into(),
                (None, Some(_)) => "Category".into(),
            },
            icon: cat.as_ref().map_or_else(|| "dots".into(), |c| c.icon.clone()),
            color: cat.as_ref().map_or(10, |c| c.color),
            existing,
            input,
            history,
            period,
            start_day,
            today,
        })
    }

    fn this_month(&self) -> i64 {
        self.history.last().map_or(0, |h| h.1)
    }

    fn completed(&self) -> Vec<i64> {
        self.history.iter().take(self.history.len().saturating_sub(1)).map(|h| h.1).collect()
    }

    fn reading(&self) -> PaceReading {
        PaceReading::new(self.input.minor(), self.this_month(), self.period.days().max(1), self.period.elapsed_days(self.today))
    }
}

/// The scroll of an editor whose footer sits over the bottom of the page.
struct Frame {
    moving: bool,
    off: f32,
    top: f32,
    foot_top: f32,
}

fn editor_begin(app: &mut App, dt: f32, foot_h: f32) -> Frame {
    let foot_top = app.canvas.h - foot_h;
    let viewport = [0.0, 0.0, WIDTH, foot_top];
    let ui = &app.ui;
    let sc = &mut app.nav.last_mut().expect("a page").1;
    let moving = sc.update(ui, viewport, dt);
    Frame { moving, off: sc.off, top: app.canvas.inset_top + 10.0, foot_top }
}

fn editor_end(app: &mut App, fr: &Frame, bottom: f32) {
    let keyboard = app.keyboard_up;
    let at = app.focus.at;
    let sc = app.scroll();
    sc.max = bottom + 24.0 - fr.foot_top;
    if keyboard && let Some((y, h)) = at {
        let need = y + h + 16.0 - fr.foot_top;
        if need > sc.off {
            sc.off = need;
        }
    }
}

/// Splits this frame's tap between what scrolls and the footer under `foot_top`.
fn split_tap(cx: &mut Ctx, foot_top: f32) -> Option<(f32, f32)> {
    let tap = cx.ui.tap.filter(|&(_, y)| y >= foot_top);
    if tap.is_some() {
        cx.ui.tap = None;
    }
    tap
}

fn budget_edit_page(app: &mut App, dt: f32, category: Option<i64>) -> bool {
    if app.fresh || app.st.plan.budget.as_ref().is_none_or(|b| b.category != category) {
        match BudgetEditor::open(app, category) {
            Ok(b) => app.st.plan.budget = Some(b),
            Err(e) => {
                app.say(e, None);
                app.pop();
                return false;
            }
        }
    }
    let fr = editor_begin(app, dt, pk::KEYPAD_H + app.canvas.inset_bottom);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let Some(e) = app.st.plan.budget.as_mut() else { return false };
        let foot = split_tap(&mut cx, fr.foot_top);
        cx.c.dy = -fr.off;
        cx.c.push_clip(0.0, 0.0, WIDTH, fr.foot_top);
        bottom = budget_form(&mut cx, e, fr.top, &fmt, &mut act);
        cx.c.pop_clip();
        cx.c.dy = 0.0;
        if foot.is_some() {
            cx.ui.tap = foot;
        }
        let inset = cx.c.inset_bottom;
        if let Some(k) = pk::keypad(&mut cx, fr.foot_top, pk::KEYPAD_H + inset, e.input.fraction_digits > 0) {
            e.input = match k {
                '.' => e.input.decimal(),
                '<' => e.input.backspace(),
                d => e.input.digit(d.to_digit(10).unwrap_or(0)),
            };
        }
    }
    editor_end(app, &fr, bottom);
    perform(app, act);
    fr.moving
}

fn budget_form(cx: &mut Ctx, e: &mut BudgetEditor, top: f32, fmt: &MoneyFormatter, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let (back, save, hh) = widgets::page_header(cx, top, &e.name, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::SaveBudget);
    }
    let mut y = top + hh + 4.0;
    let pad = 20.0;
    let inner = W - pad * 2.0;
    let r = e.reading();
    let completed = e.completed();
    let last = completed.last().copied().unwrap_or(0);
    let average = logic::average_on_record(&completed);
    let step = 10 * relay_money::money::pow10(e.input.fraction_digits);
    let suggest_avg = average.map(|a| logic::round_to(a, step)).filter(|a| *a > 0);
    let suggest_last = Some(logic::round_up_to(last, step)).filter(|a| *a > 0);
    let typed = e.input.minor() > 0;
    let line = format!("{} so far · {}", copy::of_budget(r.spent, r.budget, fmt), logic::pace_days(&r));
    let lt = Text::new(13.0, pk::tone_colour(cx, logic::pace_tone(&r))).line(1.4);
    let lh = if typed { cx.c.para_h(&line, inner, lt, 2) } else { 0.0 };
    let chips = suggest_avg.is_some() || suggest_last.is_some();
    let big = Text::new(40.0, if e.input.is_empty() { p.ink3 } else { p.ink }).medium().line(1.15);
    let head = 18.0 + big.lh() + 4.0 + 18.0;
    let h = pad + head + if typed { 16.0 + 6.0 + 12.0 + lh } else { 0.0 } + if chips { 18.0 + 36.0 } else { 0.0 } + pad;
    kit::card(cx, PAD, y, W, h);
    let mut tx = PAD + pad;
    if e.category.is_some() {
        kit::badge(cx, tx, y + pad + 2.0, 44.0, &e.icon, theme::category_color(Some(e.color)));
        tx += 44.0 + 14.0;
    }
    cx.c.text("A MONTH", tx, y + pad, Text::new(12.0, p.ink3).semi());
    let amount = dress(&e.input, fmt);
    cx.c.text_fit(&amount, tx, y + pad + 18.0, PAD + W - pad - tx, big);
    cx.c.text_fit(&logic::reset_line(e.start_day), tx, y + pad + 18.0 + big.lh() + 4.0, PAD + W - pad - tx, Text::new(13.0, p.ink3));
    let mut cy = y + pad + head;
    if typed {
        cy += 16.0;
        let over = r.spent > r.budget;
        let fill = if over {
            p.held
        } else if r.status == relay_money::pace::PaceStatus::OverPace {
            p.waiting
        } else if e.category.is_some() {
            theme::category_color(Some(e.color))
        } else {
            p.signal
        };
        kit::meter(cx, PAD + pad, cy, inner, r.spent_fraction, Some(r.pace_fraction), fill);
        cy += 6.0 + 12.0;
        cy += cx.c.para(&line, PAD + pad, cy, inner, lt, 2);
    }
    if chips {
        cy += 18.0;
        let mut px = PAD + pad;
        if let Some(a) = suggest_avg {
            let (w, t) = pk::chip(cx, px, cy, &format!("Average {}", fmt.format_whole(a)), Some("sparkle"));
            if t {
                e.input = AmountInput::of(a, e.input.fraction_digits);
            }
            px += w + 8.0;
        }
        if let Some(a) = suggest_last {
            let (_, t) = pk::chip(cx, px, cy, &format!("Last month {}", fmt.format_whole(a)), None);
            if t {
                e.input = AmountInput::of(a, e.input.fraction_digits);
            }
        }
    }
    y += h + 28.0;

    // What it spent, a meter a month.
    let (sh, _) = kit::section(cx, y, "What it spent", None);
    y += sh + 14.0;
    let rows = e.history.len();
    let avg_line = average.map(|a| format!("Average {} over {}", fmt.format_whole(a), copy::plural(completed.iter().filter(|v| **v > 0).count() as i64, "month")));
    let ph = 14.0 + rows as f32 * 34.0 + if avg_line.is_some() { 4.0 + 20.0 } else { 0.0 } + 12.0;
    kit::card(cx, PAD, y, W, ph);
    let mut ry = y + 14.0;
    let top_v = e.history.iter().map(|h| h.1).max().unwrap_or(0).max(1);
    for (i, (start, v)) in e.history.iter().enumerate() {
        let now = i + 1 == rows;
        let mt = Text::new(14.0, if now { p.ink } else { p.ink2 });
        let mt = if now { mt.semi() } else { mt.medium() };
        cx.c.text(&start.strftime("%b").to_string(), PAD + pad, ry + 6.0, mt);
        let vt = Text::new(14.0, if now { p.ink } else { p.ink2 }).medium().right();
        cx.c.text(&fmt.format_whole(*v), PAD + W - pad, ry + 6.0, vt);
        let (mx, mw) = (PAD + pad + 44.0, W - pad * 2.0 - 44.0 - 76.0);
        kit::meter(cx, mx, ry + 12.0, mw, *v as f32 / top_v as f32, None, if now { p.signal } else { p.signal.a(0.4) });
        ry += 34.0;
    }
    if let Some(l) = avg_line {
        cx.c.text(&l, PAD + pad, ry + 4.0, Text::new(13.0, p.ink3));
    }
    y += ph;
    if e.existing.is_some() {
        y += 24.0;
        if pk::remove_key(cx, y, "Remove budget") {
            *act = Some(Act::RemoveBudget);
        }
        y += 52.0;
    }
    y
}

/// The typed amount in the currency's dress: "$1,250.5" while typing "1250.5".
fn dress(input: &AmountInput, fmt: &MoneyFormatter) -> String {
    let text = if input.text.is_empty() { "0" } else { input.text.as_str() };
    let (int, frac) = text.split_once('.').map_or((text, None), |(a, b)| (a, Some(b)));
    let n: i64 = int.parse().unwrap_or(0);
    let whole = fmt.format_whole(n.saturating_mul(relay_money::money::pow10(fmt.fraction_digits)));
    match frac {
        None => whole,
        Some(f) => {
            // The separator goes after the last digit, wherever the symbol sits.
            let at = whole.rfind(|c: char| c.is_ascii_digit()).map_or(whole.len(), |i| i + 1);
            format!("{}{}{f}{}", &whole[..at], fmt.locale.decimal_separator(), &whole[at..])
        }
    }
}

// Bills

fn bills_page(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &app.st.plan;
        bottom = match st.data.as_ref() {
            None => problem(&mut cx, pg.top, "Bills", st.problem.as_deref(), &mut act),
            Some(d) => bills_body(&mut cx, d, pg.top, &mut act),
        };
    }
    app.page_end(&pg, bottom);
    perform(app, act);
    pg.moving
}

/// A bill's amount and its colour: money in is green with a plus, a transfer quiet.
fn bill_amount(cx: &Ctx, bill: &Bill, fmt: &MoneyFormatter) -> (String, Rgba) {
    match bill.kind {
        TxType::Income => (format!("+{}", fmt.format(bill.amount)), cx.p.live_text),
        TxType::Transfer => (fmt.format(bill.amount), cx.p.ink2),
        TxType::Expense => (fmt.format(bill.amount), cx.p.ink),
    }
}

fn bills_body(cx: &mut Ctx, d: &PlanData, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &d.fmt;
    let r = &d.bills;
    let (back, add, hh) = widgets::page_header(cx, top, "Bills", Some("plus"));
    if back {
        *act = Some(Act::Back);
    }
    if add {
        *act = Some(Act::Go(Route::BillEdit { id: None }));
    }
    let mut y = top + hh + 4.0;
    let ch = 380.0;
    if r.active.is_empty() && r.paused.is_empty() {
        let spec = PicCard {
            eyebrow: Some("Bills"),
            title: "Nothing set to repeat",
            line: Some("Rent, pay, subscriptions: each one posts itself on its day."),
            button: Some("Add a bill"),
            ..Default::default()
        };
        let (_, button) = kit::pic_card(cx, PAD, y, CW, ch, "receipts", &spec);
        if button {
            *act = Some(Act::Go(Route::BillEdit { id: None }));
        }
        return y + ch;
    }

    // The next 30 days on the receipts.
    let (title, line) = logic::bills_hero(r, d.today, fmt);
    let spec = PicCard { eyebrow: Some("Coming up"), title: &title, title_size: 34.0, line: Some(&line), ..Default::default() };
    kit::pic_card(cx, PAD, y, CW, ch, "receipts", &spec);
    y += ch + 12.0;

    // What the bills take in an average month, and where it goes, right under the card.
    if !r.cost_shares.is_empty() {
        let shown = r.cost_shares.len().min(5);
        let pad = 20.0;
        let lt = Text::new(12.0, p.ink3).semi();
        let h = pad + lt.lh() + 8.0 + 30.0 + 10.0 + 14.0 + shown as f32 * 30.0 + 14.0;
        kit::card(cx, PAD, y, W, h);
        let mut cy = y + pad;
        cx.c.text("A MONTH OF BILLS", PAD + pad, cy, lt);
        cy += lt.lh() + 8.0;
        let out = fmt.format_whole(r.monthly_cost);
        let ow = kit::money(cx, &out, PAD + pad, cy - 2.0, 26.0, p.ink, p.ink2);
        cx.c.text("out", PAD + pad + ow + 8.0, cy + 9.0, Text::new(14.0, p.ink2));
        if r.monthly_income > 0 {
            let words = format!("{} in", fmt.format_whole(r.monthly_income));
            cx.c.text(&words, PAD + W - pad, cy + 9.0, Text::new(14.0, p.live_text).medium().right());
        }
        cy += 30.0 + 10.0;
        let parts: Vec<(Rgba, i64)> = r.cost_shares.iter().map(|s| (theme::category_color(s.2), s.1)).collect();
        pk::stack_bar(cx, PAD + pad, cy, W - pad * 2.0, 8.0, &parts);
        cy += 8.0 + 14.0;
        for (name, per, color) in r.cost_shares.iter().take(5) {
            cx.c.circle(PAD + pad + 5.0, cy + 10.0, 5.0, theme::category_color(*color));
            let amount = format!("{} a month", fmt.format_whole(*per));
            let at = Text::new(14.0, p.ink2).right();
            let aw = cx.c.measure(&amount, at);
            cx.c.text(&amount, PAD + W - pad, cy + 1.0, at);
            cx.c.text_fit(name, PAD + pad + 18.0, cy + 1.0, W - pad * 2.0 - 18.0 - aw - 10.0, Text::new(14.0, p.ink).medium());
            cy += 30.0;
        }
        y += h;
    }
    y += 26.0;

    // The running bills by when they fall due, a card a group, what was paid this week first.
    let mut groups = logic::bill_groups(&r.active, d.today);
    let paid = logic::paid_lately(&r.active, &d.bill_posts, d.today);
    if !paid.is_empty() && groups.first().is_none_or(|g| g.0 != "This week") {
        groups.insert(0, ("This week".to_string(), Vec::new()));
    }
    for (gi, (title, rows)) in groups.iter().enumerate() {
        y += pk::group_label(cx, y, title);
        // Each row and the bill it opens.
        let mut shows: Vec<(pk::BillShow, i64)> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let lately: &[(usize, Date)] = if gi == 0 && title == "This week" { &paid } else { &[] };
        for (i, _) in lately {
            let b = &r.active[*i];
            notes.push(match (b.row.bill.kind, &b.row.to_account) {
                (TxType::Income, _) => format!("Paid into {}", b.row.account),
                (TxType::Transfer, Some(to)) => format!("Moved to {to}"),
                _ => format!("Paid from {}", b.row.account),
            });
        }
        for i in rows {
            notes.push(logic::bill_note(&r.active[*i]));
        }
        for (k, (i, day)) in lately.iter().enumerate() {
            let bill = &r.active[*i].row.bill;
            let (amount, _) = bill_amount(cx, bill, fmt);
            let show = pk::BillShow {
                month: day.strftime("%b").to_string(),
                day: day.day(),
                hue: theme::date_hue(0),
                name: &bill.name,
                note: &notes[k],
                note_colour: p.live_text,
                paid: true,
                amount,
                amount_colour: p.ink2,
            };
            shows.push((show, bill.id));
        }
        for (k, i) in rows.iter().enumerate() {
            let b = &r.active[*i];
            let bill = &b.row.bill;
            let (amount, amount_colour) = bill_amount(cx, bill, fmt);
            let show = pk::BillShow {
                month: bill.next.strftime("%b").to_string(),
                day: bill.next.day(),
                hue: if b.days_until <= 3 { p.signal } else { theme::date_hue(0) },
                name: &bill.name,
                note: &notes[lately.len() + k],
                note_colour: if b.days_until < 0 { p.held_text } else { p.ink2 },
                paid: false,
                amount,
                amount_colour,
            };
            shows.push((show, bill.id));
        }
        let (rows_only, ids): (Vec<pk::BillShow>, Vec<i64>) = shows.into_iter().unzip();
        let (h, hit) = pk::bill_card(cx, y, &rows_only);
        if let Some(k) = hit {
            *act = Some(Act::Go(Route::BillEdit { id: Some(ids[k]) }));
        }
        y += h + 26.0;
    }
    if r.active.is_empty() {
        cx.c.text("Every bill is paused. Open one to run it again.", PAD, y, Text::new(14.0, p.ink3));
        y += 40.0;
    }

    // Paused bills, at the date they would next fall due.
    if !r.paused.is_empty() {
        y += pk::group_label(cx, y, "Paused");
        let notes: Vec<String> = r.paused.iter().map(|b| format!("Paused · {} · {}", logic::every_label(b.row.bill.frequency, b.row.bill.interval), b.row.account)).collect();
        let mut rows = Vec::new();
        for (b, note) in r.paused.iter().zip(&notes) {
            let bill = &b.row.bill;
            let (amount, _) = bill_amount(cx, bill, fmt);
            rows.push(pk::BillShow {
                month: bill.next.strftime("%b").to_string(),
                day: bill.next.day(),
                hue: theme::date_hue(0),
                name: &bill.name,
                note,
                note_colour: p.ink3,
                paid: false,
                amount,
                amount_colour: p.ink3,
            });
        }
        let (h, hit) = pk::bill_card(cx, y, &rows);
        if let Some(k) = hit {
            *act = Some(Act::Go(Route::BillEdit { id: Some(r.paused[k].row.bill.id) }));
        }
        y += h + 26.0;
    }
    y - 26.0
}

fn bill_edit_page(app: &mut App, dt: f32, id: Option<i64>) -> bool {
    if app.fresh || app.st.plan.bill.as_ref().is_none_or(|b| b.route_id != id) {
        match BillEditor::open(app, id) {
            Ok(b) => app.st.plan.bill = Some(b),
            Err(e) => {
                app.say(e, None);
                app.pop();
                return false;
            }
        }
    }
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let State { bill, sheet, sheet_scroll, sheet_month, .. } = &mut app.st.plan;
        let Some(e) = bill.as_mut() else { return false };
        let held = if sheet.is_some() { widgets::hold(&mut cx) } else { None };
        bottom = bill_form(&mut cx, e, pg.top, &fmt, &mut act);
        e.resolve();
        if let Some(which) = *sheet {
            widgets::release(&mut cx, held);
            match which {
                Sheet::Account | Sheet::ToAccount => {
                    let chosen = if which == Sheet::ToAccount { e.to } else { e.account };
                    let list: Vec<&AccountView> = e.accounts.iter().filter(|a| !a.archived || Some(a.id) == chosen).collect();
                    let items: Vec<(String, Option<String>)> = list.iter().map(|a| (a.name.clone(), Some(fmt.format(a.balance)))).collect();
                    let selected = list.iter().position(|a| Some(a.id) == chosen);
                    match widgets::sheet_list(&mut cx, "Account", &items, selected, sheet_scroll, dt) {
                        SheetOut::Pick(i) => {
                            if which == Sheet::ToAccount {
                                e.to = Some(list[i].id);
                            } else {
                                e.account = Some(list[i].id);
                            }
                            act = Some(Act::Sheet(None));
                        }
                        SheetOut::Close => act = Some(Act::Sheet(None)),
                        SheetOut::Nothing => {}
                    }
                }
                Sheet::Date => {
                    let mut month = sheet_month.unwrap_or(e.anchor);
                    let (out, picked) = widgets::sheet_date(&mut cx, if e.stored.is_some() { "Counted from" } else { "First due" }, e.anchor, &mut month);
                    *sheet_month = Some(month);
                    if let Some(d) = picked {
                        e.anchor = d;
                    }
                    if out != SheetOut::Nothing {
                        act = Some(Act::Sheet(None));
                    }
                }
                _ => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    let open = app.st.plan.sheet.is_some();
    perform(app, act);
    pg.moving || open
}

fn bill_form(cx: &mut Ctx, e: &mut BillEditor, top: f32, fmt: &MoneyFormatter, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let title = if e.stored.is_none() {
        "New bill".to_string()
    } else if e.name.trim().is_empty() {
        "Bill".to_string()
    } else {
        e.name.clone()
    };
    let (back, save, hh) = widgets::page_header(cx, top, &title, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::SaveBill);
    }
    let mut y = top + hh;
    let problems = e.problems();
    let errors = e.show_errors;

    let kinds = [TxType::Expense, TxType::Income, TxType::Transfer];
    let at = kinds.iter().position(|k| *k == e.kind).unwrap_or(0);
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Expense", "Income", "Transfer"], at) {
        e.kind = kinds[i];
        e.resolve();
    }
    y += 44.0 + 18.0;

    y += pk::label(cx, y, "Name");
    y += pk::field(cx, NAME, PAD, y, W, None, "Rent, Netflix, Salary", &mut e.name, false, errors && problems.name.is_some());
    if errors && let Some(w) = problems.name {
        y += pk::hint(cx, y, w, true);
    }
    y += 18.0;

    y += pk::label(cx, y, "Amount");
    let placeholder = fmt.format(0);
    y += pk::field(cx, AMOUNT, PAD, y, W, Some("coins"), &placeholder, &mut e.amount_text, true, errors && problems.amount.is_some());
    let per = logic::per_month(e.amount().unwrap_or(0), e.frequency, e.interval);
    if errors && let Some(w) = problems.amount {
        y += pk::hint(cx, y, w, true);
    } else if per > 0 && e.frequency != Frequency::Monthly {
        y += pk::hint(cx, y, &format!("{} a month", fmt.format_whole(per)), false);
    }
    y += 18.0;

    y += pk::label(cx, y, if e.kind == TxType::Transfer { "From" } else { "Account" });
    let name = e.account_name(e.account);
    if pk::pick_field(cx, PAD, y, W, name.as_deref().unwrap_or("Pick an account"), name.is_some(), None) {
        *act = Some(Act::Sheet(Some(Sheet::Account)));
    }
    y += 52.0;
    if errors && let Some(w) = problems.account {
        y += pk::hint(cx, y, w, true);
    }
    y += 18.0;

    if e.kind == TxType::Transfer {
        y += pk::label(cx, y, "To");
        let name = e.account_name(e.to);
        if pk::pick_field(cx, PAD, y, W, name.as_deref().unwrap_or("Pick an account"), name.is_some(), None) {
            *act = Some(Act::Sheet(Some(Sheet::ToAccount)));
        }
        y += 52.0 + 18.0;
    } else {
        y += pk::label(cx, y, "Category");
        y += category_grid(cx, e, y) + 18.0;
    }

    y += pk::label(cx, y, "How often");
    let freqs = [Frequency::Weekly, Frequency::Monthly, Frequency::Yearly];
    let at = freqs.iter().position(|f| *f == e.frequency).unwrap_or(1);
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Weekly", "Monthly", "Yearly"], at) {
        e.frequency = freqs[i];
    }
    y += 44.0 + 10.0;
    // The gap between dates, stepped.
    kit::card(cx, PAD, y, W, 56.0);
    let every = logic::every_label(e.frequency, e.interval);
    cx.c.text(&every, PAD + 18.0, y + (56.0 - Text::new(16.0, p.ink).lh()) / 2.0, Text::new(16.0, p.ink).medium());
    let (kx, ky) = (PAD + W - 8.0 - 44.0, y + 6.0);
    cx.c.rect(kx - 52.0, ky + 4.0, 36.0, 36.0, 18.0, p.card2);
    cx.c.rect(kx + 4.0, ky + 4.0, 36.0, 36.0, 18.0, p.card2);
    if kit::round_key(cx, kx - 56.0, ky, "minus") && e.interval > 1 {
        e.interval -= 1;
    }
    if kit::round_key(cx, kx, ky, "plus") {
        e.interval = (e.interval + 1).min(logic::MAX_INTERVAL);
    }
    y += 56.0 + 18.0;

    y += pk::label(cx, y, if e.stored.is_some() { "Counted from" } else { "First due" });
    if pk::pick_field(cx, PAD, y, W, &logic::day(e.anchor, e.today), true, Some("calendar")) {
        *act = Some(Act::Sheet(Some(Sheet::Date)));
    }
    y += 52.0;
    let preview = e.preview();
    if !preview.is_empty() {
        let words = preview.iter().map(|d| logic::short(*d, e.today)).collect::<Vec<_>>().join(", ");
        y += pk::hint(cx, y, &format!("Next: {words}"), false);
    }
    y += 18.0;

    let rows_h = widgets::row_h(true) * 2.0 + 12.0;
    kit::card(cx, PAD, y, W, rows_h);
    let (auto_sub, active_sub) = (
        if e.auto_post { "Logged on its day, with nothing to do" } else { "A reminder on Overview instead; you log it" },
        if e.active { "Running" } else { "Paused: nothing posts until it is resumed" },
    );
    widgets::toggle_row(cx, PAD + 4.0, y + 6.0, W - 8.0, Some(("bolt", p.ink)), "Posts itself", Some(auto_sub), &mut e.auto_post);
    widgets::divider(cx, PAD + 4.0, y + 6.0 + widgets::row_h(true), W - 8.0);
    widgets::toggle_row(cx, PAD + 4.0, y + 6.0 + widgets::row_h(true), W - 8.0, Some(("play", p.ink)), "Active", Some(active_sub), &mut e.active);
    y += rows_h;
    if e.stored.is_some() {
        y += 28.0;
        if pk::remove_key(cx, y, "Delete bill") {
            *act = Some(Act::DeleteBill);
        }
        y += 52.0;
    }
    y
}

/// The categories of the bill's kind as tiles, four to a row, folded after seven.
fn category_grid(cx: &mut Ctx, e: &mut BillEditor, y: f32) -> f32 {
    let p = cx.p;
    let kind = if e.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
    let cats: Vec<&CategoryView> = e.categories.iter().filter(|c| c.kind == kind && (!c.archived || Some(c.id) == e.category)).collect();
    let (cols, gap, chh) = (4usize, 8.0, 80.0);
    let cw = (W - gap * (cols as f32 - 1.0)) / cols as f32;
    let picked_late = e.category.and_then(|id| cats.iter().position(|c| c.id == id)).is_some_and(|i| i >= 7);
    let limit = if e.all_categories || picked_late || cats.len() <= 8 { cats.len() } else { 7 };
    let mut chosen = None;
    for (i, c) in cats.iter().take(limit).enumerate() {
        let (x, cy) = (PAD + (i % cols) as f32 * (cw + gap), y + (i / cols) as f32 * (chh + gap));
        let on = e.category == Some(c.id);
        cx.c.rect(x, cy, cw, chh, 18.0, p.slab);
        if on {
            cx.c.outline(x, cy, cw, chh, 18.0, 2.0, p.ink);
        } else {
            cx.c.outline(x, cy, cw, chh, 18.0, 1.0, p.line_subtle);
        }
        if cx.pressed(x, cy, cw, chh) {
            cx.c.rect(x, cy, cw, chh, 18.0, p.wash);
        }
        kit::badge(cx, x + cw / 2.0 - 18.0, cy + 10.0, 36.0, &c.icon, theme::category_color(Some(c.color)));
        let t = Text::new(12.5, p.ink).medium().center();
        cx.c.text_fit(&c.name, x + cw / 2.0, cy + 54.0, cw - 10.0, if on { t.semi() } else { t });
        if cx.tap(x, cy, cw, chh) {
            chosen = Some(c.id);
        }
    }
    let more = limit < cats.len();
    if more {
        let i = limit;
        let (x, cy) = (PAD + (i % cols) as f32 * (cw + gap), y + (i / cols) as f32 * (chh + gap));
        cx.c.rect(x, cy, cw, chh, 18.0, p.card2);
        cx.c.icon("more", x + cw / 2.0, cy + 28.0, 20.0, p.ink2);
        cx.c.text(&format!("{} more", cats.len() - limit), x + cw / 2.0, cy + 54.0, Text::new(12.5, p.ink).medium().center());
        if cx.tap(x, cy, cw, chh) {
            e.all_categories = true;
        }
    }
    if let Some(id) = chosen {
        // A second tap on the picked category clears it: a bill can post uncategorised.
        e.category = if e.category == Some(id) { None } else { Some(id) };
    }
    if cats.is_empty() {
        cx.c.text("No categories of this kind yet.", PAD + 2.0, y + 4.0, Text::new(ty::CAPTION, p.ink3));
        return 24.0;
    }
    let shown = limit + usize::from(more);
    shown.div_ceil(cols) as f32 * (chh + gap) - gap
}

/// The bill editor's working copy.
struct BillEditor {
    route_id: Option<i64>,
    /// The bill as stored when the editor opened, and the latest date posted against it.
    stored: Option<Bill>,
    last_posted: Option<Date>,
    name: String,
    kind: TxType,
    amount_text: String,
    account: Option<i64>,
    to: Option<i64>,
    category: Option<i64>,
    frequency: Frequency,
    interval: i32,
    anchor: Date,
    auto_post: bool,
    active: bool,
    show_errors: bool,
    all_categories: bool,
    accounts: Vec<AccountView>,
    categories: Vec<CategoryView>,
    today: Date,
    digits: u32,
}

impl BillEditor {
    fn open(app: &App, id: Option<i64>) -> Result<BillEditor, String> {
        let today = model::today();
        let conn = app.store.sql()?;
        let stored = match id {
            Some(id) => store::bill(conn, id)?,
            None => None,
        };
        let last_posted = match &stored {
            Some(b) => store::last_posted(conn, b.id)?,
            None => None,
        };
        let fmt = app.store.formatter();
        let accounts = app.store.ledger.accounts().map_err(|e| e.to_string())?;
        let categories = app.store.ledger.categories().map_err(|e| e.to_string())?;
        let mut e = BillEditor {
            route_id: id,
            stored: None,
            last_posted,
            name: String::new(),
            kind: TxType::Expense,
            amount_text: String::new(),
            // A new bill starts on the account new entries start in; resolve() checks it is open.
            account: crate::accounts_store::default_account(&app.store),
            to: None,
            category: None,
            frequency: Frequency::Monthly,
            interval: 1,
            anchor: today,
            auto_post: true,
            active: true,
            show_errors: false,
            all_categories: false,
            accounts,
            categories,
            today,
            digits: fmt.fraction_digits,
        };
        if let Some(b) = stored {
            e.name = b.name.clone();
            e.kind = b.kind;
            e.amount_text = widgets::minor_text(b.amount, fmt.fraction_digits);
            e.account = Some(b.account_id);
            e.to = b.to_account_id;
            e.category = b.category_id;
            e.frequency = b.frequency;
            e.interval = b.interval;
            e.anchor = b.anchor;
            e.auto_post = b.auto_post;
            e.active = b.active;
            e.stored = Some(b);
        }
        e.resolve();
        Ok(e)
    }

    /// Fills what is not picked yet and drops picks that no longer fit (PlanLogic.kt `resolveBill`).
    fn resolve(&mut self) {
        let live: Vec<&AccountView> = self.accounts.iter().filter(|a| !a.archived).collect();
        if !self.account.is_some_and(|id| self.accounts.iter().any(|a| a.id == id)) {
            self.account = live.first().map(|a| a.id);
        }
        if self.kind == TxType::Transfer {
            if !self.to.is_some_and(|id| self.accounts.iter().any(|a| a.id == id)) {
                self.to = live.iter().find(|a| Some(a.id) != self.account).map(|a| a.id);
            }
        } else {
            self.to = None;
        }
        let kind = if self.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if self.kind == TxType::Transfer || !self.category.is_some_and(|id| self.categories.iter().any(|c| c.id == id && c.kind == kind)) {
            self.category = None;
        }
    }

    fn amount(&self) -> Option<i64> {
        widgets::parse_minor(&self.amount_text, self.digits)
    }

    fn input(&self) -> BillInput {
        BillInput {
            name: self.name.trim().to_string(),
            kind: self.kind,
            amount: self.amount().unwrap_or(0),
            account_id: self.account.unwrap_or(0),
            to_account_id: self.to,
            category_id: self.category,
            frequency: self.frequency,
            interval: self.interval,
            anchor: self.anchor,
            auto_post: self.auto_post,
            active: self.active,
        }
    }

    fn problems(&self) -> logic::BillProblems {
        logic::bill_problems(&self.name, self.amount(), self.kind, self.account, self.to)
    }

    /// The next three dates it would post on, as saving would set them; none while paused.
    fn preview(&self) -> Vec<Date> {
        if !self.active {
            return Vec::new();
        }
        let input = self.input();
        let rule = input.rule();
        let mut next = store::next_due(&input, self.stored.as_ref(), self.today, self.last_posted);
        let mut out = Vec::new();
        for _ in 0..3 {
            out.push(next);
            next = rule.after(next);
        }
        out
    }

    fn account_name(&self, id: Option<i64>) -> Option<String> {
        id.and_then(|id| self.accounts.iter().find(|a| a.id == id)).map(|a| a.name.clone())
    }
}

// Goals

fn goals_page(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &app.st.plan;
        bottom = match st.data.as_ref() {
            None => problem(&mut cx, pg.top, "Goals", st.problem.as_deref(), &mut act),
            Some(d) => goals_body(&mut cx, d, pg.top, &mut act),
        };
    }
    app.page_end(&pg, bottom);
    perform(app, act);
    pg.moving
}

fn goal_picture(g: &GoalLine) -> &'static str {
    crate::pictures::for_goal(&g.name, g.kind == GoalKind::Invest)
}

/// Goals, as the owner asked: the total saved, one featured card for the goal closest to its
/// target, then the rest two to a row as smaller picture cards, and a dashed tile that adds one.
fn goals_body(cx: &mut Ctx, d: &PlanData, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &d.fmt;
    let g = &d.goals;
    let (back, add, hh) = widgets::page_header(cx, top, "Goals", Some("plus"));
    if back {
        *act = Some(Act::Back);
    }
    if add {
        *act = Some(Act::Go(Route::GoalEdit { id: None }));
    }
    let mut y = top + hh + 2.0;
    if g.goals.is_empty() {
        let ch = 400.0;
        let spec = PicCard {
            eyebrow: Some("Goals"),
            title: "Save toward something",
            line: Some("A trip, a cushion, a card paid off: Tally keeps the pace each one needs."),
            button: Some("Add a goal"),
            ..Default::default()
        };
        let (_, button) = kit::pic_card(cx, PAD, y, CW, ch, "piggy", &spec);
        if button {
            *act = Some(Act::Go(Route::GoalEdit { id: None }));
        }
        return y + ch;
    }

    // What the pots hold, and what went in this month.
    if g.pots > 0 {
        cx.c.text("Saved toward goals", PAD, y, Text::new(14.0, p.ink2).medium());
        y += 22.0;
        kit::money(cx, &fmt.format(g.saved), PAD, y, 40.0, p.ink, p.ink2);
        y += 48.0;
        if d.goals_month != 0 {
            let (words, colour) = if d.goals_month > 0 {
                (format!("+{} this month", fmt.format_whole(d.goals_month)), p.live_text)
            } else {
                (format!("−{} this month", fmt.format_whole(-d.goals_month)), p.ink2)
            };
            cx.c.text(&words, PAD, y, Text::new(14.0, colour).medium());
            y += 22.0;
        }
        y += 16.0;
    }

    // The closest goal, full width.
    let closest = logic::closest_goal(&g.goals);
    if let Some(ci) = closest {
        let goal = &g.goals[ci];
        let ch = 450.0;
        let line = logic::goal_card_line(goal, fmt);
        let pot = goal.kind == GoalKind::Savings && !goal.reached();
        let spec = PicCard {
            eyebrow: Some(if goal.reached() { "Reached" } else { "Closest" }),
            title: &goal.name,
            line: Some(&line),
            meter: Some(goal.fraction()),
            button: pot.then_some("Add money"),
            ..Default::default()
        };
        let (card, button) = kit::pic_card(cx, PAD, y, CW, ch, goal_picture(goal), &spec);
        if button {
            *act = Some(Act::AddMoney(goal.id));
        } else if card {
            *act = Some(Act::Go(Route::Goal { id: goal.id }));
        }
        y += ch + 12.0;
    }

    // The rest, two to a row, then the tile that adds one.
    let (gap, ch) = (12.0, 236.0);
    let cw = (CW - gap) / 2.0;
    let rest: Vec<&GoalLine> = g.goals.iter().enumerate().filter(|(i, _)| Some(*i) != closest).map(|(_, g)| g).collect();
    for (k, goal) in rest.iter().enumerate() {
        let (x, gy) = (PAD + (k % 2) as f32 * (cw + gap), y + (k / 2) as f32 * (ch + gap));
        let line = logic::goal_reading(goal, fmt);
        let spec = PicCard { title: &goal.name, title_size: 17.0, line: Some(&line), meter: Some(goal.fraction()), ..Default::default() };
        let (card, _) = kit::pic_card(cx, x, gy, cw, ch, goal_picture(goal), &spec);
        if card {
            *act = Some(Act::Go(Route::Goal { id: goal.id }));
        }
    }
    let k = rest.len();
    if pk::new_tile(cx, PAD + (k % 2) as f32 * (cw + gap), y + (k / 2) as f32 * (ch + gap), cw, ch, "New goal") {
        *act = Some(Act::Go(Route::GoalEdit { id: None }));
    }
    y + (k / 2 + 1) as f32 * (ch + gap) - gap
}

/// One goal's page: its reading, its pace and, for a pot, money in and out.
struct GoalPage {
    id: i64,
    read_gen: u64,
    goal: Option<Goal>,
    line: Option<GoalLine>,
    contributions: Vec<Contribution>,
    pace: Option<GoalPace>,
    amount: String,
    note: String,
    /// The history row tapped, showing its Remove key.
    selected: Option<i64>,
    /// The money in or out card is open.
    adding: bool,
}

impl GoalPage {
    fn read(&mut self, app: &App) {
        self.read_gen = app.ledger_gen;
        let Some(d) = app.st.plan.data.as_ref() else { return };
        self.goal = d.goal_rows.iter().find(|g| g.id == self.id).cloned();
        self.line = self.goal.as_ref().map(|g| logic::goal_line(g, &d.inputs, d.today));
        self.contributions = app.store.sql().and_then(|c| store::contributions(c, self.id)).unwrap_or_default();
        self.pace = match (&self.goal, &self.line) {
            (Some(g), Some(l)) if l.kind == GoalKind::Balance => logic::balance_pace(l, g.start_date, d.today),
            (Some(_), Some(l)) => logic::goal_pace(self.first(), l.saved, l.target, d.today),
            _ => None,
        };
    }

    /// The day of its first contribution.
    fn first(&self) -> Option<Date> {
        self.contributions.iter().map(|c| c.date).min()
    }
}

fn goal_page(app: &mut App, dt: f32, id: i64) -> bool {
    let renew = app.fresh || app.st.plan.goal.as_ref().is_none_or(|g| g.id != id);
    if renew {
        let adding = app.st.plan.add_money.take() == Some(id);
        app.st.plan.goal = Some(GoalPage { id, read_gen: 0, goal: None, line: None, contributions: Vec::new(), pace: None, amount: String::new(), note: String::new(), selected: None, adding });
        if adding {
            let mut cx = cx!(app);
            focus_field(&mut cx, PUT_AMOUNT, true);
        }
    }
    if renew || app.st.plan.goal.as_ref().is_some_and(|g| g.read_gen != app.ledger_gen) {
        let mut page = app.st.plan.goal.take().expect("set above");
        page.read(app);
        app.st.plan.goal = Some(page);
    }
    let pg = app.page_begin(dt);
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let State { goal, data, sheet, sheet_scroll, .. } = &mut app.st.plan;
        let (Some(page), Some(d)) = (goal.as_mut(), data.as_ref()) else { return false };
        let held = if sheet.is_some() { widgets::hold(&mut cx) } else { None };
        bottom = goal_body(&mut cx, page, d, pg.top, &mut act);
        if *sheet == Some(Sheet::GoalMore) {
            widgets::release(&mut cx, held);
            let items = vec![("Edit goal".to_string(), None), ("Delete goal".to_string(), Some("Its contributions go with it. Undo brings both back.".to_string()))];
            match widgets::sheet_list(&mut cx, page.line.as_ref().map_or("Goal", |l| l.name.as_str()), &items, None, sheet_scroll, dt) {
                SheetOut::Pick(0) => act = Some(Act::Go(Route::GoalEdit { id: Some(id) })),
                SheetOut::Pick(_) => act = Some(Act::DeleteGoal(id)),
                SheetOut::Close => act = Some(Act::Sheet(None)),
                SheetOut::Nothing => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    let open = app.st.plan.sheet.is_some();
    perform(app, act);
    pg.moving || open
}

fn goal_body(cx: &mut Ctx, s: &mut GoalPage, d: &PlanData, top: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &d.fmt;
    let Some(g) = s.line.clone() else {
        let (back, _, hh) = widgets::page_header(cx, top, "Goal", None);
        if back {
            *act = Some(Act::Back);
        }
        let y = top + hh;
        return y + widgets::empty(cx, PAD, y, W, "target", "This goal is gone", "It was deleted. Its contributions went with it.");
    };

    // The goal's picture across the top, the keys over it.
    let ph = 430.0;
    kit::picture(cx, goal_picture(&g), 0.0, 0.0, WIDTH, ph, 0.0, (0.5, 0.4));
    if pk::over_key(cx, PAD - 4.0, top + 2.0, "arrow-left") {
        *act = Some(Act::Back);
    }
    if pk::over_key(cx, WIDTH - PAD + 4.0 - 40.0, top + 2.0, "more") {
        *act = Some(Act::Sheet(Some(Sheet::GoalMore)));
    }

    // The body, a sheet on the ground with rounded top corners.
    let sheet_top = ph - 28.0;
    cx.c.rect(0.0, sheet_top, WIDTH, 6000.0, 28.0, p.ground[0]);
    let mut y = sheet_top + 30.0;
    let nt = Text::new(30.0, p.ink).semi().line(1.2);
    let lines = cx.c.wrap(&g.name, CW, nt, 2).len().max(1) as f32;
    cx.c.para(&g.name, PAD, y, CW, nt, 2);
    y += nt.lh() * lines + 6.0;
    let (big, of) = if g.monthly() && g.percent > 0 {
        (format!("{}%", g.achieved_percent().unwrap_or(0).max(0)), format!("of {}% this month", g.percent))
    } else if g.monthly() {
        (fmt.format(g.saved), format!("of {} this month", fmt.format_whole(g.target)))
    } else {
        (fmt.format(g.saved), format!("of {}", fmt.format_whole(g.target)))
    };
    let mw = kit::money(cx, &big, PAD, y, 40.0, p.ink, p.ink2);
    let ot = Text::new(15.0, p.ink3);
    cx.c.text(&of, PAD + mw + 10.0, y + 46.0 - ot.lh() - 3.0, ot);
    y += 46.0 + 16.0;
    let fill = if g.reached() { p.live } else { p.signal };
    kit::meter(cx, PAD, y, CW, g.fraction(), g.pace_fraction, fill);
    y += 6.0 + 22.0;

    // What it needs, and how it has gone.
    let since = if g.kind == GoalKind::Balance { s.goal.as_ref().and_then(|r| r.start_date) } else { s.first() };
    let need = logic::goal_need_line(&g, fmt, d.today);
    let pace = logic::goal_pace_words(&g, s.pace, since, fmt, d.today);
    let pad = 18.0;
    let inner = CW - pad * 2.0;
    let st = Text::new(15.0, p.ink).medium().line(1.35);
    let lt = Text::new(13.0, p.ink2).line(1.45);
    let nh = cx.c.para_h(&need, inner, st, 3);
    let lh = pace.as_ref().map_or(0.0, |l| 6.0 + cx.c.para_h(l, inner, lt, 3));
    let h = pad + nh + lh + pad;
    kit::card(cx, PAD, y, CW, h);
    cx.c.para(&need, PAD + pad, y + pad, inner, st, 3);
    if let Some(l) = &pace {
        cx.c.para(l, PAD + pad, y + pad + nh + 6.0, inner, lt, 3);
    }
    y += h + 16.0;

    let pot = g.kind == GoalKind::Savings;
    if pot {
        let ew = 92.0;
        if pk::action_key(cx, PAD, y, CW - ew - 10.0, "Add money", "plus", true, true) {
            s.adding = !s.adding;
            if s.adding {
                focus_field(cx, PUT_AMOUNT, true);
            }
        }
        if kit::key(cx, PAD + CW - ew, y, ew, 52.0, "Edit", None, KeyKind::Plain) {
            *act = Some(Act::Go(Route::GoalEdit { id: Some(g.id) }));
        }
    } else if kit::key(cx, PAD, y, CW, 52.0, "Edit goal", Some("edit"), KeyKind::Plain) {
        *act = Some(Act::Go(Route::GoalEdit { id: Some(g.id) }));
    }
    y += 52.0 + 16.0;

    // Money in or out of a pot.
    if pot && s.adding {
        let pad = 16.0;
        let h = pad + 52.0 + 10.0 + 52.0 + 12.0 + 52.0 + pad;
        kit::card(cx, PAD, y, CW, h);
        let (fx, fw) = (PAD + pad, CW - pad * 2.0);
        let mut fy = y + pad;
        let placeholder = fmt.format(0);
        fy += pk::field(cx, PUT_AMOUNT, fx, fy, fw, Some("coins"), &placeholder, &mut s.amount, true, false) + 10.0;
        fy += pk::field(cx, PUT_NOTE, fx, fy, fw, Some("notes"), "Note (optional)", &mut s.note, false, false) + 12.0;
        let ok = widgets::parse_minor(&s.amount, fmt.fraction_digits).is_some_and(|a| a > 0);
        let kw = (fw - 10.0) / 2.0;
        if pk::action_key(cx, fx, fy, kw, "Take out", "upload", false, ok) {
            *act = Some(Act::Contribute { out: true });
        }
        if pk::action_key(cx, fx + kw + 10.0, fy, kw, "Put in", "download", true, ok) {
            *act = Some(Act::Contribute { out: false });
        }
        y += h + 16.0;
    }

    if pot {
        y += 12.0;
        let sub = if s.contributions.is_empty() {
            None
        } else {
            let added: i64 = s.contributions.iter().filter(|c| c.amount > 0).map(|c| c.amount).sum();
            let out: i64 = s.contributions.iter().filter(|c| c.amount < 0).map(|c| -c.amount).sum();
            Some(format!("{} in · {} out", fmt.format_whole(added), fmt.format_whole(out)))
        };
        let (sh, _) = kit::section(cx, y, "Contributions", sub.as_deref());
        y += sh + 8.0;
        if s.contributions.is_empty() {
            cx.c.text("Nothing put in yet. Add money puts in the first.", PAD, y + 4.0, Text::new(14.0, p.ink3));
            y += 30.0;
        }
        let rh = 62.0;
        for c in &s.contributions {
            let outward = c.amount < 0;
            let on = s.selected == Some(c.id);
            if cx.pressed(PAD - 8.0, y, CW + 16.0, rh) || on {
                cx.c.rect(PAD - 8.0, y + 2.0, CW + 16.0, rh - 4.0, 14.0, p.wash);
            }
            let right = PAD + CW;
            let mut room = CW - 10.0;
            if on {
                let t = Text::new(ty::UI, p.on_ink).semi();
                let kw = cx.c.measure("Remove", t) + 28.0;
                cx.c.rect(right - kw, y + 13.0, kw, 36.0, 18.0, p.held);
                cx.c.text("Remove", right - kw + 14.0, y + 13.0 + (36.0 - t.lh()) / 2.0, t.color(p.ground[0]));
                if cx.tap(right - kw - 6.0, y, kw + 12.0, rh) {
                    *act = Some(Act::RemoveContribution(c.id));
                }
                room -= kw;
            } else {
                let amount = format!("{}{}", if outward { "−" } else { "+" }, fmt.format(c.amount.abs()));
                let at = Text::new(16.0, if outward { p.ink2 } else { p.live_text }).medium().right();
                room -= cx.c.measure(&amount, at);
                cx.c.text(&amount, right, y + (rh - at.lh()) / 2.0, at);
            }
            let sub = if c.note.trim().is_empty() { if outward { "Taken out" } else { "Put in" } } else { c.note.as_str() };
            cx.c.text_fit(&logic::short(c.date, d.today), PAD, y + 12.0, room, Text::new(16.0, p.ink).medium());
            cx.c.text_fit(sub, PAD, y + 35.0, room, Text::new(13.0, p.ink2));
            if cx.tap(PAD - 8.0, y, CW + 16.0, rh) {
                s.selected = if on { None } else { Some(c.id) };
            }
            y += rh;
        }
        if !s.contributions.is_empty() {
            cx.c.text("Tap a row to remove it.", PAD, y + 8.0, Text::new(13.0, p.ink3));
            y += 30.0;
        }
    }

    if g.monthly() && !g.history.is_empty() {
        y += 12.0;
        let (sh, _) = kit::section(cx, y, "Earlier months", None);
        y += sh + 10.0;
        for m in g.history.iter().rev() {
            cx.c.circle(PAD + 5.0, y + 11.0, 5.0, if m.met() { p.live } else { p.track });
            let words = format!("{} of {}", fmt.format_whole(m.achieved), fmt.format_whole(m.asked));
            cx.c.text(&words, PAD + CW, y + 1.0, Text::new(15.0, p.ink2).right());
            cx.c.text(&m.start.strftime("%B").to_string(), PAD + 22.0, y + 1.0, Text::new(15.0, p.ink).medium());
            y += 36.0;
        }
    }
    y
}

/// The goal editor's working copy.
struct GoalEditor {
    route_id: Option<i64>,
    stored: Option<Goal>,
    name: String,
    target_text: String,
    target_date: Option<Date>,
    color: i64,
    kind: GoalKind,
    account: Option<i64>,
    percent: i64,
    by_share: bool,
    show_errors: bool,
    accounts: Vec<AccountView>,
    today: Date,
    digits: u32,
}

impl GoalEditor {
    fn open(app: &App, id: Option<i64>) -> Result<GoalEditor, String> {
        let today = model::today();
        let stored = match id {
            Some(id) => store::goal(app.store.sql()?, id)?,
            None => None,
        };
        let digits = app.store.formatter().fraction_digits;
        let accounts = app.store.ledger.accounts().map_err(|e| e.to_string())?;
        let mut e = GoalEditor {
            route_id: id,
            stored: None,
            name: String::new(),
            target_text: String::new(),
            target_date: None,
            color: 0,
            kind: GoalKind::Savings,
            account: None,
            percent: 10,
            by_share: true,
            show_errors: false,
            accounts,
            today,
            digits,
        };
        if let Some(g) = stored {
            e.name = g.name.clone();
            e.target_text = if g.target > 0 || g.kind == GoalKind::Balance { widgets::minor_text(g.target, digits) } else { String::new() };
            e.target_date = g.target_date;
            e.color = g.color;
            e.kind = g.kind;
            e.account = g.account_id;
            e.percent = if g.percent > 0 { g.percent } else { 10 };
            e.by_share = g.percent > 0 || g.target <= 0;
            e.stored = Some(g);
        }
        Ok(e)
    }

    fn monthly(&self) -> bool {
        matches!(self.kind, GoalKind::Invest | GoalKind::Save)
    }

    fn target(&self) -> Option<i64> {
        widgets::parse_minor(&self.target_text, self.digits)
    }

    fn problems(&self) -> (Option<&'static str>, Option<&'static str>) {
        logic::goal_problems(&self.name, self.kind, self.target(), self.by_share, self.percent)
    }

    /// What a balance goal's pick reads now: the account's balance, or the net of every open one.
    fn balance_now(&self) -> i64 {
        self.account.and_then(|id| self.accounts.iter().find(|a| a.id == id)).map_or_else(|| logic::net_worth(&self.accounts), |a| a.balance)
    }

    fn input(&self) -> GoalInput {
        let monthly = self.monthly();
        // A balance goal's pace starts the day it is set, from what its pick reads then; an edit
        // keeps that start unless the pick changed under it.
        let keep = self.stored.as_ref().filter(|o| o.kind == GoalKind::Balance && o.account_id == self.account);
        GoalInput {
            name: self.name.trim().to_string(),
            target: if monthly && self.by_share { 0 } else { self.target().unwrap_or(0) },
            target_date: if monthly { None } else { self.target_date },
            color: self.color,
            kind: self.kind,
            account_id: if matches!(self.kind, GoalKind::Savings | GoalKind::Save) { None } else { self.account },
            percent: if monthly && self.by_share { self.percent } else { 0 },
            start_date: match (self.kind, keep) {
                (GoalKind::Balance, Some(o)) => o.start_date.or(Some(self.today)),
                (GoalKind::Balance, None) => Some(self.today),
                _ => None,
            },
            start_amount: match (self.kind, keep) {
                (GoalKind::Balance, Some(o)) => o.start_amount,
                (GoalKind::Balance, None) => self.balance_now(),
                _ => 0,
            },
        }
    }

    fn account_choices(&self) -> Vec<&AccountView> {
        self.accounts.iter().filter(|a| !a.archived && (self.kind != GoalKind::Invest || a.r#type == AccountType::Investment)).collect()
    }
}

fn goal_edit_page(app: &mut App, dt: f32, id: Option<i64>) -> bool {
    if app.fresh || app.st.plan.goal_edit.as_ref().is_none_or(|g| g.route_id != id) {
        match GoalEditor::open(app, id) {
            Ok(g) => app.st.plan.goal_edit = Some(g),
            Err(e) => {
                app.say(e, None);
                app.pop();
                return false;
            }
        }
    }
    let pg = app.page_begin(dt);
    let fmt = app.store.formatter();
    let mut act = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let State { goal_edit, sheet, sheet_scroll, sheet_month, .. } = &mut app.st.plan;
        let Some(e) = goal_edit.as_mut() else { return false };
        let held = if sheet.is_some() { widgets::hold(&mut cx) } else { None };
        bottom = goal_form(&mut cx, e, pg.top, &fmt, &mut act);
        if let Some(which) = *sheet {
            widgets::release(&mut cx, held);
            match which {
                Sheet::Date => {
                    let chosen = e.target_date.unwrap_or_else(|| e.today.checked_add(jiff::Span::new().months(6)).unwrap_or(e.today));
                    let mut month = sheet_month.unwrap_or(chosen);
                    let (out, picked) = widgets::sheet_date(&mut cx, "By", chosen, &mut month);
                    *sheet_month = Some(month);
                    if let Some(d) = picked {
                        e.target_date = Some(d);
                    }
                    if out != SheetOut::Nothing {
                        act = Some(Act::Sheet(None));
                    }
                }
                Sheet::GoalAccount => {
                    let list: Vec<(i64, String, i64)> = e.account_choices().iter().map(|a| (a.id, a.name.clone(), a.balance)).collect();
                    let every = if e.kind == GoalKind::Invest { "Every investment account" } else { "Everything you own" };
                    let mut items: Vec<(String, Option<String>)> = vec![(every.to_string(), None)];
                    items.extend(list.iter().map(|(_, n, b)| (n.clone(), Some(fmt.format(*b)))));
                    let selected = match e.account {
                        None => Some(0),
                        Some(id) => list.iter().position(|a| a.0 == id).map(|i| i + 1),
                    };
                    match widgets::sheet_list(&mut cx, "Account", &items, selected, sheet_scroll, dt) {
                        SheetOut::Pick(i) => {
                            e.account = if i == 0 { None } else { list.get(i - 1).map(|a| a.0) };
                            act = Some(Act::Sheet(None));
                        }
                        SheetOut::Close => act = Some(Act::Sheet(None)),
                        SheetOut::Nothing => {}
                    }
                }
                _ => {}
            }
        }
    }
    app.page_end(&pg, bottom);
    let open = app.st.plan.sheet.is_some();
    perform(app, act);
    pg.moving || open
}

fn goal_form(cx: &mut Ctx, e: &mut GoalEditor, top: f32, fmt: &MoneyFormatter, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let title = if e.stored.is_none() {
        "New goal".to_string()
    } else if e.name.trim().is_empty() {
        "Goal".to_string()
    } else {
        e.name.clone()
    };
    let (back, save, hh) = widgets::page_header(cx, top, &title, Some("Save"));
    if back {
        *act = Some(Act::Back);
    }
    if save {
        *act = Some(Act::SaveGoal);
    }
    let mut y = top + hh;
    let (name_p, target_p) = e.problems();
    let errors = e.show_errors;

    // The kind is picked once, on a new goal: a pot's contributions mean nothing to another kind.
    if e.stored.is_none() {
        y += pk::label(cx, y, "Kind");
        let kinds = [GoalKind::Savings, GoalKind::Balance, GoalKind::Invest, GoalKind::Save];
        let tw = (W - 10.0) / 2.0;
        for row in kinds.chunks(2) {
            let th = row.iter().map(|k| pk::choice_tile_h(cx, tw, logic::goal_kind_label(*k), logic::goal_kind_hint(*k))).fold(0.0, f32::max);
            for (i, k) in row.iter().enumerate() {
                if pk::choice_tile(cx, PAD + i as f32 * (tw + 10.0), y, tw, th, logic::goal_kind_label(*k), logic::goal_kind_hint(*k), logic::goal_glyph(*k), e.kind == *k) && e.kind != *k {
                    e.kind = *k;
                    e.account = None;
                }
            }
            y += th + 10.0;
        }
        y += 8.0;
    }

    y += pk::label(cx, y, "Name");
    y += pk::field(cx, GOAL_NAME, PAD, y, W, None, "Lisbon, an emergency fund, the Visa", &mut e.name, false, errors && name_p.is_some());
    if errors && let Some(w) = name_p {
        y += pk::hint(cx, y, w, true);
    }
    y += 18.0;

    let monthly = e.monthly();
    if monthly {
        y += pk::label(cx, y, "Each month");
        if let Some(i) = widgets::segmented(cx, PAD, y, W, &["A share of income", "A set amount"], if e.by_share { 0 } else { 1 }) {
            e.by_share = i == 0;
        }
        y += 44.0 + 18.0;
    }
    if monthly && e.by_share {
        y += pk::label(cx, y, "Share");
        kit::card(cx, PAD, y, W, 64.0);
        let ky = y + 10.0;
        cx.c.rect(PAD + 14.0, ky + 4.0, 36.0, 36.0, 18.0, p.card2);
        cx.c.rect(PAD + W - 14.0 - 36.0, ky + 4.0, 36.0, 36.0, 18.0, p.card2);
        if kit::round_key(cx, PAD + 10.0, ky, "minus") {
            e.percent = (e.percent - 1).clamp(1, 100);
        }
        if kit::round_key(cx, PAD + W - 10.0 - 44.0, ky, "plus") {
            e.percent = (e.percent + 1).clamp(1, 100);
        }
        let t = Text::new(28.0, p.ink).medium().center();
        cx.c.text(&format!("{}%", e.percent), WIDTH / 2.0, y + (64.0 - t.lh()) / 2.0, t);
        y += 64.0 + 12.0;
        let choices = [5i64, 10, 15, 20, 25];
        let labels: Vec<String> = choices.iter().map(|v| format!("{v}%")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let (picked, ph) = widgets::pills(cx, PAD, y, W, &refs, choices.iter().position(|v| *v == e.percent));
        if let Some(i) = picked {
            e.percent = choices[i];
        }
        y += ph;
        if errors && let Some(w) = target_p {
            y += pk::hint(cx, y, w, true);
        }
        y += 18.0;
    } else {
        let label = if e.kind == GoalKind::Balance {
            "Amount to reach"
        } else if monthly {
            "A month"
        } else {
            "Target"
        };
        y += pk::label(cx, y, label);
        let placeholder = fmt.format(0);
        y += pk::field(cx, GOAL_TARGET, PAD, y, W, Some("coins"), &placeholder, &mut e.target_text, true, errors && target_p.is_some());
        if errors && let Some(w) = target_p {
            y += pk::hint(cx, y, w, true);
        } else if e.kind == GoalKind::Balance {
            y += pk::hint(cx, y, &format!("It reads {} now", fmt.format(e.balance_now())), false);
        }
        y += 18.0;
    }

    if !monthly {
        y += pk::label(cx, y, "By");
        let text = e.target_date.map_or_else(|| "No date".to_string(), logic::long);
        let fw = if e.target_date.is_some() { W - 56.0 } else { W };
        if pk::pick_field(cx, PAD, y, fw, &text, e.target_date.is_some(), Some("calendar")) {
            *act = Some(Act::Sheet(Some(Sheet::Date)));
        }
        if e.target_date.is_some() {
            cx.c.rect(PAD + W - 48.0, y + 2.0, 48.0, 48.0, 24.0, p.card2);
            if kit::round_key(cx, PAD + W - 46.0, y + 4.0, "close") {
                e.target_date = None;
            }
        }
        y += 52.0;
        if let Some(dt) = e.target_date {
            y += pk::hint(cx, y, &format!("{} from now", copy::plural(logic::months_until(e.today, dt), "month")), false);
        }
        y += 18.0;
    }

    if matches!(e.kind, GoalKind::Balance | GoalKind::Invest) {
        y += pk::label(cx, y, if e.kind == GoalKind::Invest { "Into" } else { "Account" });
        let every = if e.kind == GoalKind::Invest { "Every investment account" } else { "Everything you own" };
        let name = e.account.and_then(|id| e.accounts.iter().find(|a| a.id == id)).map_or(every.to_string(), |a| a.name.clone());
        if pk::pick_field(cx, PAD, y, W, &name, true, None) {
            *act = Some(Act::Sheet(Some(Sheet::GoalAccount)));
        }
        y += 52.0 + 18.0;
    }

    y += pk::label(cx, y, "Colour");
    let (h, picked) = pk::swatches(cx, y, e.color);
    if let Some(c) = picked {
        e.color = c;
    }
    y += h;
    if let Some(g) = &e.stored {
        y += 28.0;
        if pk::remove_key(cx, y, "Delete goal") {
            *act = Some(Act::DeleteGoal(g.id));
        }
        y += 52.0;
    }
    y
}

// Acting on what was tapped

fn perform(app: &mut App, act: Option<Act>) {
    let Some(act) = act else { return };
    let today = model::today();
    match act {
        Act::Go(r) => {
            app.st.plan.sheet = None;
            app.push(r);
        }
        Act::Back => {
            app.pop();
        }
        Act::Sheet(s) => {
            let st = &mut app.st.plan;
            if s.is_some() {
                st.sheet_scroll = Scroll::default();
                st.sheet_month = None;
            }
            st.sheet = s;
            if s.is_some() {
                app.finish_typing();
            }
        }
        Act::ApplySuggestions => {
            let Some(d) = app.st.plan.data.as_ref() else { return };
            let list: Vec<(i64, i64)> = d.budgets.suggestions.iter().map(|s| (s.category_id, s.amount)).collect();
            let result = app.store.ledger.atomically(|l| list.iter().try_for_each(|(c, a)| l.budget_set(Some(*c), *a)));
            match result {
                Ok(()) => {
                    app.invalidate();
                    let words = format!("{} set from your averages", copy::plural(list.len() as i64, "budget"));
                    say_undo(app, words, UndoAct::Budgets(list.iter().map(|(c, _)| (Some(*c), 0)).collect()));
                }
                Err(e) => app.say(e.to_string(), None),
            }
        }
        Act::SaveBudget => {
            let Some(e) = app.st.plan.budget.as_ref() else { return };
            let (category, amount) = (e.category, e.input.minor());
            if amount <= 0 {
                app.say("Type the monthly limit on the keypad first".to_string(), None);
                return;
            }
            match app.store.ledger.budget_set(category, amount) {
                Ok(()) => {
                    app.invalidate();
                    app.pop();
                }
                Err(e) => app.say(e.to_string(), None),
            }
        }
        Act::RemoveBudget => {
            let Some(e) = app.st.plan.budget.as_ref() else { return };
            let (category, name) = (e.category, e.name.clone());
            let Ok(Some(old)) = app.store.ledger.budget(category) else { return };
            match app.store.ledger.budget_set(category, 0) {
                Ok(()) => {
                    app.invalidate();
                    app.pop();
                    let words = if category.is_none() { "Monthly budget removed".to_string() } else { format!("{name} budget removed") };
                    say_undo(app, words, UndoAct::Budgets(vec![(category, old)]));
                }
                Err(e) => app.say(e.to_string(), None),
            }
        }
        Act::SaveBill => {
            app.finish_typing();
            let Some(e) = app.st.plan.bill.as_mut() else { return };
            if e.problems().any() {
                e.show_errors = true;
                return;
            }
            let (id, input) = (e.stored.as_ref().map(|b| b.id), e.input());
            match app.store.sql().and_then(|c| store::bill_save(c, id, &input, today)) {
                Ok(_) => {
                    app.invalidate();
                    app.pop();
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::DeleteBill => {
            let Some(id) = app.st.plan.bill.as_ref().and_then(|e| e.stored.as_ref().map(|b| b.id)) else { return };
            match app.store.sql().and_then(|c| store::bill_delete(c, id)) {
                Ok(name) => {
                    app.invalidate();
                    app.pop();
                    say_undo(app, format!("{name} deleted"), UndoAct::Bill(id));
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::SaveGoal => {
            app.finish_typing();
            let Some(e) = app.st.plan.goal_edit.as_mut() else { return };
            let (a, b) = e.problems();
            if a.is_some() || b.is_some() {
                e.show_errors = true;
                return;
            }
            let (id, input) = (e.stored.as_ref().map(|g| g.id), e.input());
            match app.store.sql().and_then(|c| store::goal_save(c, id, &input)) {
                Ok(_) => {
                    app.invalidate();
                    app.pop();
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::DeleteGoal(id) => {
            app.st.plan.sheet = None;
            match app.store.sql().and_then(|c| store::goal_delete(c, id)) {
                Ok(d) => {
                    app.invalidate();
                    // Its editor and its own page are gone with it.
                    while matches!(app.route(), Route::Goal { id: g } | Route::GoalEdit { id: Some(g) } if *g == id) {
                        if !app.pop() {
                            break;
                        }
                    }
                    say_undo(app, format!("{} deleted", d.name), UndoAct::Goal(d));
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::AddMoney(id) => {
            app.st.plan.add_money = Some(id);
            app.push(Route::Goal { id });
        }
        Act::Contribute { out } => {
            let fmt = app.store.formatter();
            let Some(page) = app.st.plan.goal.as_mut() else { return };
            let Some(amount) = widgets::parse_minor(&page.amount, fmt.fraction_digits).filter(|a| *a > 0) else { return };
            let (id, note) = (page.id, page.note.clone());
            match app.store.sql().and_then(|c| store::contribute(c, id, if out { -amount } else { amount }, today, &note)) {
                Ok(_) => {
                    if let Some(page) = app.st.plan.goal.as_mut() {
                        page.amount.clear();
                        page.note.clear();
                        page.adding = false;
                    }
                    app.finish_typing();
                    app.invalidate();
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::RemoveContribution(cid) => match app.store.sql().and_then(|c| store::contribution_delete(c, cid)) {
            Ok(amount) => {
                if let Some(page) = app.st.plan.goal.as_mut() {
                    page.selected = None;
                }
                app.invalidate();
                say_undo(app, if amount < 0 { "Withdrawal removed".into() } else { "Contribution removed".into() }, UndoAct::Contribution(cid));
            }
            Err(e) => app.say(e, None),
        },
    }
}
