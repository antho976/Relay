//! An entry, new or being edited, as the Kotlin app's EntryScreen.kt in the premium look
//! (DESIGN.md): the type, the big amount on Tally's own keypad with the note under it, what it does
//! to the month, the usual entries, the categories as a sideways row of chips, the account (or the
//! two of a transfer) and the day as chips, then one Save key. The keypad shows while the amount is
//! typed and folds once it is in and a category is picked; an edit opens folded, with Copy to today
//! and Delete (with Undo) in its header. Accounts, the rest of the categories and another day are
//! picked from sheets.

use relay_money::Date;
use relay_money::amount_input::AmountInput;
use relay_money::model::{CategoryKind, TxType};
use relay_money::period::BudgetPeriod;
use relay_money::views::{AccountView, CategoryView, Tx, TxInput, TxPatch, TxQuery};

use crate::activity_data;
use crate::activity_kit as ak;
use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::entry_logic::{self as logic, Draft, GRID_SLOTS, LinkedBill, Usual};
use crate::kit::{self, Ctx, KeyKind, PAD, ty};
use crate::model::{self, LineTone, Store};
use crate::nav::Route;
use crate::theme;
use crate::ui::{HScroll, Scroll};
use crate::widgets::{self, SheetOut};

/// The editor's draft as kept while the system has stopped Tally: what was typed and picked, and
/// which entry it edits. Ids are checked again as it is put back.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedDraft {
    pub editing: Option<i64>,
    pub kind: TxType,
    /// The keypad's text, as typed ("12." stays "12.").
    pub amount: String,
    pub note: String,
    pub account: Option<i64>,
    pub to_account: Option<i64>,
    pub category: Option<i64>,
    #[serde(default)]
    pub category_chosen: bool,
    /// The day, as YYYY-MM-DD.
    pub date: String,
    /// The account sheet's "New account" was picked from this side, so the account made under
    /// it comes back to the same side after the system stopped Tally.
    #[serde(default)]
    pub made_for: Option<Pick>,
    /// "New category" was picked, so the category made under it comes back picked.
    #[serde(default)]
    pub made_category: bool,
}

/// A sheet open over the editor.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Pick {
    From,
    To,
    Day,
    Category,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: TxType,
    pub amount: AmountInput,
    pub category: Option<i64>,
    pub account: Option<i64>,
    pub to_account: Option<i64>,
    pub date: Date,
    pub note: String,
    /// The entry being edited, as stored, or none for a new one.
    pub editing: Option<i64>,
    pub original: Option<Tx>,
    /// Every category to choose from, the most used first.
    pub categories: Vec<CategoryView>,
    pub accounts: Vec<AccountView>,
    /// The keypad shows; it folds once the amount and a category are in.
    pub pad_open: bool,
    pub picking: Option<Pick>,
    pub sheet_scroll: Scroll,
    /// The month the day sheet shows.
    pub month: Date,
    /// This period, what it has spent, its overall limit, and each category's total and count.
    pub period: BudgetPeriod,
    pub period_spent: i64,
    pub budget: Option<i64>,
    pub by_category: Vec<(i64, TxType, i64, usize)>,
    pub category_budgets: Vec<(i64, i64)>,
    pub bill: Option<LinkedBill>,
    /// The [`App::ledger_gen`] `bill` was read at: a change since (a bill deleted, then put back
    /// by its Undo) reads it again, so "Open bill" comes back with it.
    pub bill_gen: u64,
    /// The usual entries, offered on a new one, and notes used before, for suggestions.
    pub usual: Vec<Usual>,
    pub notes: Vec<(String, Option<i64>)>,
    /// The person picked the category; a suggested note's category never overrides it.
    pub category_chosen: bool,
    pub today: Date,
    /// The sideways rows: categories, the usual entries, notes used before.
    pub cat_scroll: HScroll,
    pub usual_scroll: HScroll,
    pub sugg_scroll: HScroll,
    /// Where the keys at the bottom start on screen (the app's notice sits above them).
    pub footer_top: f32,
    /// The account sheet's "New account" was picked from this side: the account made comes back
    /// to it.
    pub made_for: Option<Pick>,
    /// "New category" was picked: the category made comes back picked.
    pub made_category: bool,
    /// The entry this editor was opened to edit is gone (deleted meanwhile, on another page or by
    /// a sync): the editor leaves on its first frame rather than open blank as a new entry.
    pub gone: bool,
}

impl Default for Entry {
    fn default() -> Self {
        let today = model::today();
        Entry {
            kind: TxType::Expense,
            amount: AmountInput::empty(2),
            category: None,
            account: None,
            to_account: None,
            date: today,
            note: String::new(),
            editing: None,
            original: None,
            categories: Vec::new(),
            accounts: Vec::new(),
            pad_open: true,
            picking: None,
            sheet_scroll: Scroll::default(),
            month: today,
            period: BudgetPeriod::containing(today, 1),
            period_spent: 0,
            budget: None,
            by_category: Vec::new(),
            category_budgets: Vec::new(),
            bill: None,
            bill_gen: 0,
            usual: Vec::new(),
            notes: Vec::new(),
            category_chosen: false,
            today,
            cat_scroll: HScroll::default(),
            usual_scroll: HScroll::default(),
            sugg_scroll: HScroll::default(),
            footer_top: 0.0,
            made_for: None,
            made_category: false,
            gone: false,
        }
    }
}

impl Entry {
    /// An empty entry, with the ledger's categories (the most used first) and accounts.
    pub fn fresh(store: &Store) -> Entry {
        let today = model::today();
        let mut categories: Vec<CategoryView> = store.ledger.categories().unwrap_or_default().into_iter().filter(|c| !c.archived).collect();
        let accounts: Vec<AccountView> = store.ledger.accounts().unwrap_or_default().into_iter().filter(|a| !a.archived).collect();
        let digits = store.formatter().fraction_digits;

        // The grid starts where the thumb usually goes: uses over the last three months.
        let year = TxQuery { from: today.checked_sub(jiff::Span::new().days(365)).ok(), to: Some(today), limit: Some(100_000), ..Default::default() };
        let past = store.ledger.tx_list(&year, today).map(|p| p.transactions).unwrap_or_default();
        let since = today.checked_sub(jiff::Span::new().days(92)).unwrap_or(today).to_string();
        let recent: Vec<Tx> = past.iter().filter(|t| t.date >= since).cloned().collect();
        let mut notes: Vec<(String, Option<i64>)> = Vec::new();
        for t in &past {
            let n = t.note.trim();
            if !n.is_empty() && !notes.iter().any(|(x, _)| x.eq_ignore_ascii_case(n)) {
                notes.push((n.to_string(), t.category_id));
            }
        }
        let mut uses: Vec<(i64, usize)> = Vec::new();
        for t in &recent {
            if let Some(c) = t.category_id {
                match uses.iter_mut().find(|(id, _)| *id == c) {
                    Some(u) => u.1 += 1,
                    None => uses.push((c, 1)),
                }
            }
        }
        let ids: Vec<i64> = categories.iter().map(|c| c.id).collect();
        let order = logic::by_use(&ids, &uses);
        categories = order.into_iter().map(|i| categories[i].clone()).collect();

        // This period's readings, for the lines beside the amount and under the grid.
        let period = activity_data::period_at(store, today, 0);
        let q = TxQuery { from: Some(period.start), to: Some(period.last_day()), limit: Some(100_000), ..Default::default() };
        let rows = store.ledger.tx_list(&q, today).map(|p| p.transactions).unwrap_or_default();
        let mut by_category: Vec<(i64, TxType, i64, usize)> = Vec::new();
        for t in &rows {
            if let Some(c) = t.category_id {
                match by_category.iter_mut().find(|(id, k, _, _)| *id == c && *k == t.r#type) {
                    Some(e) => {
                        e.2 += t.amount;
                        e.3 += 1;
                    }
                    None => by_category.push((c, t.r#type, t.amount, 1)),
                }
            }
        }
        let category_budgets =
            categories.iter().filter(|c| c.kind == CategoryKind::Expense).filter_map(|c| store.ledger.budget(Some(c.id)).ok().flatten().map(|b| (c.id, b))).collect();
        let usual = logic::usual(&recent, 8).into_iter().filter(|u| categories.iter().any(|c| c.id == u.category)).collect();
        // The account new entries start in (Accounts sets it), else the first open one.
        let account = crate::accounts_store::default_account(store).filter(|id| accounts.iter().any(|a| a.id == *id)).or_else(|| accounts.first().map(|a| a.id));
        // A transfer goes somewhere else: the first account that is not the one it leaves.
        let to_account = accounts.iter().map(|a| a.id).find(|id| Some(*id) != account);
        Entry {
            amount: AmountInput::empty(digits),
            account,
            to_account,
            categories,
            accounts,
            period,
            period_spent: rows.iter().filter(|t| t.r#type == TxType::Expense).map(|t| t.amount).sum(),
            budget: store.ledger.budget(None).ok().flatten().filter(|b| *b > 0),
            by_category,
            category_budgets,
            today,
            date: today,
            month: today,
            usual,
            notes,
            ..Entry::default()
        }
    }

    /// The editor for a route: a new entry, optionally with its type, account or category chosen,
    /// or an existing one (`id`) loaded to edit.
    pub fn open(store: &Store, id: Option<i64>, kind: Option<TxType>, account: Option<i64>, category: Option<i64>) -> Entry {
        let mut e = Entry::fresh(store);
        if let Some(k) = kind {
            e.kind = k;
        }
        // The route's account or category may be archived (an archived account's "See its
        // entries", then +): it is put on offer, as an edit's are, so the chip shows it and a
        // trip to another page and back keeps it. One gone altogether is not used.
        if let Some(a) = account
            && e.offer_account(store, a)
        {
            e.account = account;
            if e.to_account == account {
                e.to_account = e.accounts.iter().map(|a| a.id).find(|a| Some(*a) != account);
            }
        }
        if let Some(c) = category
            && e.offer_category(store, c)
        {
            e.category = category;
            e.category_chosen = true;
        }
        if let Some(id) = id {
            let Ok(t) = store.ledger.tx(id) else {
                e.gone = true;
                return e;
            };
            let digits = store.formatter().fraction_digits;
            e.editing = Some(id);
            e.kind = t.r#type;
            e.amount = AmountInput::of(t.amount, digits);
            e.account = Some(t.account_id);
            e.to_account = t.to_account_id.or(e.to_account);
            // Not a transfer, its To is the fresh one's pick, which may be the account this entry
            // is in: switched to Transfer it would go from an account to itself.
            if e.to_account == e.account {
                e.to_account = e.accounts.iter().map(|a| a.id).find(|a| Some(*a) != e.account);
            }
            e.category = t.category_id;
            e.category_chosen = true;
            e.note = t.note.clone();
            if let Ok(d) = t.date.parse() {
                e.date = d;
                e.month = d;
            }
            e.pad_open = false;
            // An entry that wears an archived account or category keeps it on offer.
            for wanted in [Some(t.account_id), t.to_account_id].into_iter().flatten() {
                e.offer_account(store, wanted);
            }
            if let Some(c) = t.category_id {
                e.offer_category(store, c);
            }
            e.bill = linked_bill(store, id);
            e.original = Some(t);
        }
        e
    }

    /// Puts account `id` on offer if it is not (an archived one). False when there is no such
    /// account.
    pub fn offer_account(&mut self, store: &Store, id: i64) -> bool {
        if self.accounts.iter().any(|a| a.id == id) {
            return true;
        }
        match store.ledger.accounts().unwrap_or_default().into_iter().find(|a| a.id == id) {
            Some(a) => {
                self.accounts.push(a);
                true
            }
            None => false,
        }
    }

    /// Puts category `id` on offer if it is not (an archived one). False when there is no such
    /// category.
    fn offer_category(&mut self, store: &Store, id: i64) -> bool {
        if self.categories.iter().any(|c| c.id == id) {
            return true;
        }
        match store.ledger.categories().unwrap_or_default().into_iter().find(|c| c.id == id) {
            Some(c) => {
                self.categories.push(c);
                true
            }
            None => false,
        }
    }

    /// One entry as its overview reads it: the entry, its accounts' names, its bill, and how its
    /// own category stands this period. Much less than [`Entry::open`] reads (no year of usual
    /// entries, no budget per category), so it is cheap to read again when the ledger moves.
    pub fn view(store: &Store, id: i64) -> Entry {
        let today = model::today();
        let mut e = Entry { period: activity_data::period_at(store, today, 0), ..Entry::default() };
        let Ok(t) = store.ledger.tx(id) else { return e };
        let digits = store.formatter().fraction_digits;
        e.accounts = store.ledger.accounts().unwrap_or_default();
        e.editing = Some(id);
        e.kind = t.r#type;
        e.amount = AmountInput::of(t.amount, digits);
        e.account = Some(t.account_id);
        e.to_account = t.to_account_id;
        e.category = t.category_id;
        e.category_chosen = true;
        e.note = t.note.clone();
        e.pad_open = false;
        if let Ok(d) = t.date.parse() {
            e.date = d;
            e.month = d;
        }
        if let Some(c) = t.category_id {
            e.categories = store.ledger.categories().unwrap_or_default().into_iter().filter(|x| x.id == c).collect();
            let q = TxQuery {
                from: Some(e.period.start),
                to: Some(e.period.last_day()),
                category_id: Some(c),
                limit: Some(100_000),
                ..Default::default()
            };
            for r in store.ledger.tx_list(&q, today).map(|p| p.transactions).unwrap_or_default() {
                if r.category_id != Some(c) {
                    continue;
                }
                match e.by_category.iter_mut().find(|(_, k, _, _)| *k == r.r#type) {
                    Some(x) => {
                        x.2 += r.amount;
                        x.3 += 1;
                    }
                    None => e.by_category.push((c, r.r#type, r.amount, 1)),
                }
            }
            if e.categories.iter().any(|x| x.kind == CategoryKind::Expense)
                && let Some(b) = store.ledger.budget(Some(c)).ok().flatten()
            {
                e.category_budgets.push((c, b));
            }
        }
        e.bill = linked_bill(store, id);
        e.original = Some(t);
        e
    }

    /// Back on the editor from a page it opened (a new account or category, the bill): the draft
    /// stays as it was; the accounts and categories are read again, and one just made is picked.
    pub fn came_back(&mut self, store: &Store) {
        self.picking = None;
        // The entry being edited went meanwhile (a sync deleted it): the editor leaves on its
        // next frame, as when it is opened for one that is gone.
        if let Some(id) = self.editing
            && store.ledger.tx(id).is_err()
        {
            self.gone = true;
            return;
        }
        let made_for = self.made_for.take();
        let had: Vec<i64> = self.accounts.iter().map(|a| a.id).collect();
        self.accounts = store.ledger.accounts().unwrap_or_default().into_iter().filter(|a| !a.archived || had.contains(&a.id)).collect();
        let present = |id: Option<i64>, list: &[AccountView]| id.filter(|id| list.iter().any(|a| a.id == *id));
        self.account = present(self.account, &self.accounts);
        self.to_account = present(self.to_account, &self.accounts);
        // Only an account this editor asked for is picked: one that arrived otherwise (a sync
        // brought it in) would move the entry on Save.
        if let Some(side) = made_for
            && let Some(new) = self.accounts.iter().filter(|a| !had.contains(&a.id)).map(|a| a.id).max()
        {
            if side == Pick::To {
                self.to_account = Some(new);
            } else {
                self.account = Some(new);
            }
        }
        if self.account.is_none() {
            self.account = self.accounts.first().map(|a| a.id);
        }

        let all = store.ledger.categories().unwrap_or_default();
        let had: Vec<i64> = self.categories.iter().map(|c| c.id).collect();
        // The order stays (the most used first); renamed ones read anew; new ones go last.
        let mut categories: Vec<CategoryView> = self.categories.iter().filter_map(|c| all.iter().find(|x| x.id == c.id).cloned()).collect();
        let made: Vec<CategoryView> = all.into_iter().filter(|c| !c.archived && !had.contains(&c.id)).collect();
        let want = if self.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        // As for accounts: only a category this editor asked to make is picked.
        let asked = std::mem::take(&mut self.made_category);
        let pick = made.iter().filter(|c| c.kind == want).map(|c| c.id).max().filter(|_| asked);
        categories.extend(made);
        self.categories = categories;
        if self.category.is_some_and(|id| !self.categories.iter().any(|c| c.id == id)) {
            self.category = None;
            self.category_chosen = false;
        }
        if let Some(id) = pick.filter(|_| self.kind != TxType::Transfer) {
            self.category = Some(id);
            self.category_chosen = true;
            self.fold_if_typed();
        }
        if let Some(id) = self.editing {
            self.bill = linked_bill(store, id);
        }
    }

    /// What Copy to today adds: the draft as it stands (kind, amount, accounts, category, note),
    /// dated `today`. Err says what stops it, as Save would.
    pub fn copy_input(&self, today: Date) -> Result<TxInput, &'static str> {
        let draft = Draft { date: today, ..self.draft() };
        if let Some(why) = logic::validate(&draft) {
            return Err(why);
        }
        let Some(account) = self.account else { return Err("Pick an account") };
        let transfer = self.kind == TxType::Transfer;
        let note = self.note.trim().to_string();
        Ok(TxInput {
            r#type: self.kind,
            amount: draft.amount,
            date: today.to_string(),
            account_id: account,
            to_account_id: if transfer { self.to_account } else { None },
            category_id: if transfer { None } else { self.category },
            note: (!note.is_empty()).then_some(note),
        })
    }

    fn draft(&self) -> Draft {
        Draft { kind: self.kind, amount: self.amount.minor(), date: self.date, account: self.account, to_account: self.to_account, category: self.category }
    }

    /// What was typed, to keep across the system stopping Tally (Android's saved state).
    pub fn saved_draft(&self) -> SavedDraft {
        SavedDraft {
            editing: self.editing,
            kind: self.kind,
            amount: self.amount.text.clone(),
            note: self.note.clone(),
            account: self.account,
            to_account: self.to_account,
            category: self.category,
            category_chosen: self.category_chosen,
            date: self.date.to_string(),
            made_for: self.made_for,
            made_category: self.made_category,
        }
    }

    /// Puts back what [`Entry::saved_draft`] kept, over the editor its route just opened. A draft
    /// for another entry than the one open is not used. An account or category gone since (or
    /// not on offer) is dropped: the editor keeps the one it opened with.
    pub fn restore_draft(&mut self, d: &SavedDraft) {
        if d.editing != self.editing {
            return;
        }
        self.kind = d.kind;
        self.amount = AmountInput::new(&d.amount, self.amount.fraction_digits);
        self.note = d.note.clone();
        self.made_for = d.made_for;
        self.made_category = d.made_category;
        let account = |id: Option<i64>| id.filter(|id| self.accounts.iter().any(|a| a.id == *id));
        if let Some(a) = account(d.account) {
            self.account = Some(a);
        }
        if let Some(a) = account(d.to_account) {
            self.to_account = Some(a);
        }
        match d.category.filter(|id| self.categories.iter().any(|c| c.id == *id)) {
            Some(c) => {
                self.category = Some(c);
                self.category_chosen = d.category_chosen;
            }
            // Dropped on purpose (none was picked): none, as it was.
            None if d.category.is_none() => {
                self.category = None;
                self.category_chosen = false;
            }
            None => {}
        }
        if let Ok(date) = d.date.parse::<Date>() {
            self.date = date;
            self.month = date;
        }
        // Typed in and categorised: the keypad folds, as it would have.
        if self.category.is_some() {
            self.fold_if_typed();
        }
    }

    fn shown_categories(&self) -> Vec<&CategoryView> {
        let want = if self.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        self.categories.iter().filter(|c| c.kind == want).collect()
    }

    fn category_name(&self) -> Option<String> {
        self.category.and_then(|id| self.categories.iter().find(|c| c.id == id)).map(|c| c.name.clone())
    }

    /// What the month's limit has left once this is saved; none without a limit.
    fn left_after(&self) -> Option<i64> {
        let spent = logic::spent_after(self.period_spent, self.original.as_ref(), &self.draft(), &self.period);
        self.budget.map(|b| b - spent)
    }

    /// The category's line under the grid (and on the entry's overview).
    pub fn category_line(&self, fmt: &relay_money::money::MoneyFormatter) -> Option<(String, LineTone)> {
        let id = self.category?;
        let name = self.categories.iter().find(|c| c.id == id)?.name.clone();
        let (total, count) = self.by_category.iter().find(|(c, k, _, _)| *c == id && *k == self.kind).map_or((0, 0), |e| (e.2, e.3));
        let month = logic::category_month(total, count, self.original.as_ref(), &self.draft(), &self.period);
        let budget = self.category_budgets.iter().find(|(c, _)| *c == id).map(|(_, b)| *b);
        Some(logic::category_line(self.kind, &name, month, budget, fmt))
    }

    /// The keypad folds once there is an amount to keep.
    fn fold_if_typed(&mut self) {
        if self.amount.minor() > 0 {
            self.pad_open = false;
        }
    }
}

/// The bill an entry was posted by, if it still exists.
fn linked_bill(store: &Store, id: i64) -> Option<LinkedBill> {
    let db = store.sql().ok()?;
    db.query_row(
        "SELECT r.id, r.frequency, r.interval, r.next_date, r.active, r.auto_post FROM transactions t
           JOIN recurring r ON r.id = t.recurring_id AND r.deleted = 0 WHERE t.id = ?1",
        [id],
        |r| {
            Ok(LinkedBill {
                id: r.get(0)?,
                frequency: r.get(1)?,
                interval: r.get(2)?,
                next: r.get::<_, String>(3)?.parse().unwrap_or_else(|_| model::today()),
                active: r.get(4)?,
                auto_post: r.get(5)?,
            })
        },
    )
    .ok()
}

enum Act {
    Back,
    Save { next: bool },
    Delete,
    Duplicate,
    Go(Route),
}

const W: f32 = WIDTH - PAD * 2.0;
const NOTE: &str = "entry.note";
const KEY_H: f32 = 52.0;
const GAP: f32 = 8.0;
/// A chip of what an entry is made of: a category, an account, a day.
const TAG_H: f32 = 44.0;

pub fn draw(app: &mut App, dt: f32) -> bool {
    // Opened to edit an entry that is gone: say so and leave, rather than offer a blank editor
    // whose Save would add a new entry.
    if app.entry.gone {
        app.entry.gone = false;
        leave_gone(app);
        return true;
    }
    if app.entry.bill_gen != app.ledger_gen {
        app.entry.bill_gen = app.ledger_gen;
        if let Some(id) = app.entry.editing {
            app.entry.bill = linked_bill(&app.store, id);
        }
    }
    // While the keyboard is up the keypad steps aside: the bottom inset is then the keyboard.
    let typing = app.keyboard_up && app.focus.is(NOTE);
    let fmt = app.store.formatter();
    let e = &app.entry;
    let can_save = logic::validate(&e.draft()).is_none();
    let problem_line = !can_save && !e.amount.is_empty() && !e.pad_open;
    let footer_h = if typing {
        app.canvas.inset_bottom
    } else if e.pad_open {
        12.0 + 4.0 * (KEY_H + GAP) + 4.0 + KEY_H + 12.0 + app.canvas.inset_bottom
    } else {
        12.0 + if problem_line { 24.0 } else { 0.0 } + KEY_H + if e.editing.is_none() { 44.0 } else { 12.0 } + app.canvas.inset_bottom
    };
    let footer_top = app.canvas.h - footer_h;
    app.entry.footer_top = footer_top;
    let e = &app.entry;
    let viewport = [0.0, 0.0, WIDTH, footer_top];
    let picking = e.picking;
    let ui = &app.ui;
    let sc = &mut app.nav.last_mut().expect("a page").1;
    let mut moving = if picking.is_none() { sc.update(ui, viewport, dt) } else { false };
    let off = sc.off;
    let top = app.canvas.inset_top + 10.0;
    let mut act = None;
    let bottom;
    {
        let e = &mut app.entry;
        let mut cx = cx!(app);
        // A sheet takes the frame's tap; otherwise the footer takes its own first, as it sits over
        // what scrolls.
        let held = if picking.is_some() { widgets::hold(&mut cx) } else { None };
        let tap = cx.ui.tap.filter(|&(_, y)| y >= footer_top);
        if tap.is_some() {
            cx.ui.tap = None;
        }
        cx.c.dy = -off;
        cx.c.push_clip(0.0, 0.0, WIDTH, footer_top + 30.0);
        bottom = form(&mut cx, e, top, &fmt, dt, &mut act, &mut moving);
        cx.c.pop_clip();
        cx.c.dy = 0.0;
        cx.ui.tap = tap;
        if !typing {
            footer(&mut cx, e, footer_top, footer_h, problem_line, &mut act);
        }
        if picking.is_some() {
            widgets::release(&mut cx, held);
            sheet(&mut cx, e, dt, &mut act);
        }
    }
    let at = app.focus.at;
    let sc = app.scroll();
    sc.max = bottom + 24.0 - footer_top;
    // Typing: keep the note above the keyboard.
    let mut raised = false;
    if typing && let Some((y, h)) = at {
        let need = y + h + 16.0 - footer_top;
        if need > sc.off {
            sc.off = need;
            raised = true;
        }
    }
    // This frame drew at the old offset: draw again so the note shows above the keyboard now.
    if raised {
        app.wake_at(app.now_ns);
    }
    match act {
        Some(Act::Back) => {
            app.pop();
        }
        Some(Act::Save { next }) => save(app, next),
        Some(Act::Delete) => delete(app),
        Some(Act::Duplicate) => duplicate(app),
        Some(Act::Go(r)) => app.push(r),
        None => {}
    }
    moving || app.entry.picking.is_some()
}

/// The words for a line in its tone.
fn tone_colour(cx: &Ctx, tone: LineTone) -> Rgba {
    match tone {
        LineTone::Quiet => cx.p.ink3,
        LineTone::Warn => cx.p.waiting_text,
        LineTone::Held => cx.p.held_text,
    }
}

#[allow(clippy::too_many_arguments)]
fn form(cx: &mut Ctx, e: &mut Entry, top: f32, fmt: &relay_money::money::MoneyFormatter, dt: f32, act: &mut Option<Act>, moving: &mut bool) -> f32 {
    let p = cx.p;
    let mut y = top;

    // Close, the title, and for an edit Copy to today and Delete.
    if kit::round_key(cx, 10.0, y, "close") {
        *act = Some(Act::Back);
    }
    let mut side = 56.0;
    if e.editing.is_some() {
        if kit::round_key(cx, WIDTH - 10.0 - 44.0, y, "trash") {
            *act = Some(Act::Delete);
        }
        if kit::round_key(cx, WIDTH - 10.0 - 44.0 - 46.0, y, "copy") {
            *act = Some(Act::Duplicate);
        }
        side = 104.0;
    }
    let title = if e.editing.is_some() { logic::title(e.kind, true) } else { "New entry".to_string() };
    let tt = Text::new(17.0, p.ink).semi().center();
    cx.c.text_fit(&title, WIDTH / 2.0, y + (44.0 - tt.lh()) / 2.0, WIDTH - side * 2.0, tt);
    y += 44.0 + 12.0;

    // Expense, Income, Transfer.
    let kinds = [TxType::Expense, TxType::Income, TxType::Transfer];
    let at = kinds.iter().position(|k| *k == e.kind).unwrap_or(0);
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Expense", "Income", "Transfer"], at) {
        e.kind = kinds[i];
        // A category of the other kind does not carry over.
        let want = if e.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if e.category.and_then(|id| e.categories.iter().find(|c| c.id == id)).is_some_and(|c| c.kind != want) {
            e.category = None;
            // Nothing is picked now, so a suggested note may bring its category again.
            e.category_chosen = false;
        }
    }
    y += 44.0 + 22.0;

    // The amount, cents raised, with a brass caret while the keypad shows; tapped, it brings the
    // keypad back.
    let big = 56.0;
    let empty = e.amount.text.is_empty();
    // Folded, it reads as money ("$6.50"); while typed, as typed ("$6.5").
    let amount = if empty {
        fmt.format_whole(0)
    } else if e.pad_open {
        dress(&e.amount.text, fmt)
    } else {
        fmt.format(e.amount.minor())
    };
    let (ink, cents) = if empty {
        (p.ink3, p.ink3)
    } else if e.kind == TxType::Income {
        (p.live_text, p.live_text)
    } else {
        (p.ink, p.ink2)
    };
    // A very large amount shrinks to fit the screen, caret or pencil included; it keeps the full
    // size's row, centred in it, so nothing below jumps as digits go in.
    let mut size = big;
    let mut aw = kit::money_w(cx.c, &amount, size);
    while aw > W - 40.0 && size > 20.0 {
        size -= 2.0;
        aw = kit::money_w(cx.c, &amount, size);
    }
    let ty = y + (big - size) * 0.575;
    let ax = WIDTH / 2.0 - aw / 2.0;
    kit::money(cx, &amount, ax, ty, size, ink, cents);
    if e.pad_open {
        if cx.focus.id.is_none() {
            cx.c.rect(ax + aw + 5.0, ty + size * 0.14, 2.5, size * 0.92, 1.25, p.signal);
        }
    } else {
        cx.c.icon("edit", ax + aw + 18.0, ty + size * 0.6, 16.0, p.ink3);
    }
    let amount_h = big * 1.15;
    if cx.tap(PAD, y - 6.0, W, amount_h + 6.0) {
        e.pad_open = true;
        if cx.focus.id.take().is_some() {
            cx.host.keyboard(false, "", false);
        }
    }
    y += amount_h + 2.0;

    // The note, under the amount.
    let placeholder = match e.kind {
        TxType::Expense => "Where, or what for",
        TxType::Income => "Who paid",
        TxType::Transfer => "Add a note",
    };
    if ak::centred_field(cx, NOTE, WIDTH / 2.0, y, W - 40.0, 30.0, placeholder, &mut e.note, Text::new(15.0, p.ink2)) {
        // The keyboard and the keypad never share the screen.
        e.pad_open = false;
    }
    y += 30.0 + 4.0;

    // Notes used before that start with what is typed: one tap fills it.
    let typed = if cx.focus.is(NOTE) { cx.focus.text.clone() } else { e.note.clone() };
    let suggested = logic::suggestions(&e.notes, &typed, 6);
    if !suggested.is_empty() {
        y += 6.0;
        let labels: Vec<&str> = suggested.iter().map(String::as_str).collect();
        let (picked, m) = ak::chip_row(cx, &mut e.sugg_scroll, y, &labels, None, dt);
        *moving |= m;
        if let Some(i) = picked {
            let n = suggested[i].clone();
            if cx.focus.is(NOTE) {
                cx.focus.text = n.clone();
                cx.host.keyboard(true, &n, false);
            }
            // The category last used with this note comes with it, unless one was picked.
            if !e.category_chosen && e.kind != TxType::Transfer {
                let want = if e.kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
                let last = e.notes.iter().find(|(x, _)| x.eq_ignore_ascii_case(&n)).and_then(|(_, c)| *c);
                if let Some(c) = last.filter(|c| e.categories.iter().any(|x| x.id == *c && x.kind == want)) {
                    e.category = Some(c);
                }
            }
            e.note = n;
        }
        y += ak::CHIP_H + 4.0;
    }

    // What it does to the month.
    let spent = logic::spent_after(e.period_spent, e.original.as_ref(), &e.draft(), &e.period);
    if let Some((words, tone)) = logic::after_line(e.kind, e.left_after(), Some(spent), fmt) {
        let t = Text::new(13.0, tone_colour(cx, tone)).medium().line(1.35);
        y += 4.0;
        for l in cx.c.wrap(&words, W - 20.0, t, 2) {
            cx.c.text(&l, WIDTH / 2.0, y, t.center());
            y += t.lh();
        }
    }
    y += 18.0;

    // The usual entries, one tap each, on a new one.
    if e.editing.is_none() && e.usual.iter().any(|u| u.kind == e.kind) {
        y = usual_row(cx, e, y, fmt, dt, moving) + 10.0;
    }

    if e.kind != TxType::Transfer {
        y = categories(cx, e, y, dt, act, moving) + 8.0;
        if let Some((words, tone)) = e.category_line(fmt) {
            let t = Text::new(12.5, if tone == LineTone::Quiet { p.ink3 } else { tone_colour(cx, tone) }).medium().line(1.35);
            y += cx.c.para(&words, PAD + 2.0, y, W - 4.0, t, 2) + 10.0;
        } else {
            y += 2.0;
        }
    }

    // The account (or the two of a transfer) and the day.
    let day = if e.date == e.today { "Today".to_string() } else { model::day_word(&e.date.to_string(), e.today) };
    let dt_ = Text::new(ty::UI, p.ink).medium();
    let day_w = 12.0 + 24.0 + cx.c.measure(&day, dt_) + 14.0;
    let name = |id: Option<i64>| match id.and_then(|id| e.accounts.iter().find(|a| a.id == id)) {
        Some(a) => (a.name.clone(), activity_data::account_glyph(a.r#type)),
        None if e.accounts.is_empty() => ("Add an account first".to_string(), "bank"),
        None => ("Pick an account".to_string(), "bank"),
    };
    let mut open = None;
    if e.kind == TxType::Transfer {
        let cw = (W - 30.0) / 2.0;
        let (from, fg) = name(e.account);
        let (to, tg) = name(e.to_account);
        if ak::tag_chip(cx, PAD, y, Some(cw), TAG_H, ak::Lead::Glyph(fg), &from, false).1 {
            open = Some(Pick::From);
        }
        cx.c.icon("chevron-right", WIDTH / 2.0, y + TAG_H / 2.0, 14.0, p.ink3);
        if ak::tag_chip(cx, PAD + W - cw, y, Some(cw), TAG_H, ak::Lead::Glyph(tg), &to, false).1 {
            open = Some(Pick::To);
        }
        y += TAG_H + 10.0;
        if ak::tag_chip(cx, PAD, y, Some(day_w), TAG_H, ak::Lead::Glyph("calendar"), &day, false).1 {
            open = Some(Pick::Day);
        }
        y += TAG_H;
        if e.to_account.is_some() && e.to_account == e.account {
            y += 10.0;
            cx.c.text("Pick two different accounts", PAD + 2.0, y, Text::new(ty::CAPTION, p.held_text).medium());
            y += 18.0;
        }
    } else {
        let (acct, glyph) = name(e.account);
        if ak::tag_chip(cx, PAD, y, Some(W - day_w - 10.0), TAG_H, ak::Lead::Glyph(glyph), &acct, false).1 {
            open = Some(Pick::From);
        }
        if ak::tag_chip(cx, PAD + W - day_w, y, Some(day_w), TAG_H, ak::Lead::Glyph("calendar"), &day, false).1 {
            open = Some(Pick::Day);
        }
        y += TAG_H;
    }
    if let Some(pick) = open {
        close_note(cx);
        e.fold_if_typed();
        if pick == Pick::Day {
            e.month = e.date;
        }
        e.picking = Some(pick);
        e.sheet_scroll = Scroll::default();
    }

    // The bill that posted it, on an edit.
    if let Some(b) = &e.bill {
        y += 14.0;
        let bh = 68.0;
        kit::card(cx, PAD, y, W, bh);
        ak::glyph_badge(cx, PAD + 16.0, y + (bh - 36.0) / 2.0, 36.0, "repeat", p.ink2);
        let kt = Text::new(ty::UI, p.ink).semi();
        let kw = cx.c.measure("Open bill", kt) + 28.0;
        let kx = PAD + W - 14.0 - kw;
        let tx = PAD + 16.0 + 36.0 + 12.0;
        let room = kx - tx - 8.0;
        cx.c.text_fit(&format!("Repeats {}", logic::every_label(&b.frequency, b.interval).to_lowercase()), tx, y + 14.0, room, Text::new(15.0, p.ink).medium());
        cx.c.text_fit(&logic::linked_bill_line(b, e.today), tx, y + 36.0, room, Text::new(13.0, p.ink2));
        if kit::key(cx, kx, y + (bh - 38.0) / 2.0, kw, 38.0, "Open bill", None, KeyKind::Plain) {
            *act = Some(Act::Go(Route::BillEdit { id: Some(b.id) }));
        }
        y += bh;
    }
    y
}

/// The categories as a sideways row of chips, the picked one tinted and outlined in its colour.
/// Folded as the grid was: the most used first, the picked one always among them, then "n more"
/// (a sheet of all of them), or New when everything fits. Returns the bottom.
fn categories(cx: &mut Ctx, e: &mut Entry, y: f32, dt: f32, act: &mut Option<Act>, moving: &mut bool) -> f32 {
    let cats: Vec<(i64, String, String, i64)> = e.shown_categories().iter().map(|c| (c.id, c.name.clone(), c.icon.clone(), c.color)).collect();
    let ids: Vec<i64> = cats.iter().map(|c| c.0).collect();
    let (shown, hidden) = logic::grid_cells(&ids, e.category, GRID_SLOTS);
    let gap = 8.0;
    let t = Text::new(ty::UI, cx.p.ink).medium();
    let chip_w = |cx: &mut Ctx, words: &str, category: bool| 12.0 + if category { 34.0 } else { 24.0 } + cx.c.measure(words, t) + 14.0;
    let tail = if hidden > 0 { format!("{hidden} more") } else { "New".to_string() };
    let mut widths: Vec<f32> = shown.iter().map(|&i| chip_w(cx, &cats[i].1, true)).collect();
    widths.push(chip_w(cx, &tail, false));
    let content = widths.iter().sum::<f32>() + gap * (widths.len() - 1) as f32;
    let (x0, m) = kit::carousel(cx, &mut e.cat_scroll, y, TAG_H, content, dt);
    *moving |= m;
    let mut x = x0;
    let mut chosen = None;
    for (slot, &i) in shown.iter().enumerate() {
        let (id, name, icon, color) = &cats[i];
        let on = e.category == Some(*id);
        if x < WIDTH && x + widths[slot] > 0.0 {
            let colour = theme::category_color(Some(*color));
            if ak::tag_chip(cx, x, y, Some(widths[slot]), TAG_H, ak::Lead::Category(icon, colour), name, on).1 {
                chosen = Some(*id);
            }
        }
        x += widths[slot] + gap;
    }
    let glyph = if hidden > 0 { "more" } else { "plus" };
    let tail_tapped = ak::tag_chip(cx, x, y, widths.last().copied(), TAG_H, ak::Lead::Glyph(glyph), &tail, false).1;
    kit::carousel_end(cx);
    if let Some(id) = chosen {
        e.category = Some(id);
        e.category_chosen = true;
        e.fold_if_typed();
    }
    if tail_tapped {
        if hidden > 0 {
            close_note(cx);
            e.picking = Some(Pick::Category);
            e.sheet_scroll = Scroll::default();
        } else {
            e.made_category = true;
            *act = Some(Act::Go(new_category(e.kind)));
        }
    }
    y + TAG_H
}

/// The usual entries as a sideways row of chips: the category's badge, the note and the last
/// amount. Returns the bottom.
fn usual_row(cx: &mut Ctx, e: &mut Entry, y: f32, fmt: &relay_money::money::MoneyFormatter, dt: f32, moving: &mut bool) -> f32 {
    let p = cx.p;
    let h = 40.0;
    let gap = 8.0;
    let nt = Text::new(ty::CAPTION, p.ink).medium();
    let at = Text::new(ty::CAPTION, p.ink2).medium();
    let chips: Vec<(Usual, &CategoryView, f32, f32, String)> = e
        .usual
        .iter()
        .filter(|u| u.kind == e.kind)
        .filter_map(|u| {
            let c = e.categories.iter().find(|c| c.id == u.category)?;
            // The exact amount it fills, not a rounded one ($4.75, not $5).
            Some((u.clone(), c, 0.0, 0.0, fmt.format(u.last_amount)))
        })
        .collect();
    let chips: Vec<(Usual, &CategoryView, f32, f32, String)> = chips
        .into_iter()
        .map(|(u, c, _, _, amount)| {
            let nw = cx.c.measure(&u.note, nt).min(140.0);
            let aw = cx.c.measure(&amount, at);
            (u, c, nw, aw, amount)
        })
        .collect();
    let width = |nw: f32, aw: f32| 6.0 + 28.0 + 8.0 + nw + 8.0 + aw + 14.0;
    let content = chips.iter().map(|(_, _, nw, aw, _)| width(*nw, *aw)).sum::<f32>() + gap * chips.len().saturating_sub(1) as f32;
    let (x0, m) = kit::carousel(cx, &mut e.usual_scroll, y, h, content, dt);
    *moving |= m;
    let mut px = x0;
    let mut picked = None;
    for (u, c, nw, aw, amount) in &chips {
        let w = width(*nw, *aw);
        cx.c.rect(px, y, w, h, h / 2.0, p.card2);
        if cx.pressed(px, y, w, h) {
            cx.c.rect(px, y, w, h, h / 2.0, p.wash);
        }
        kit::badge(cx, px + 6.0, y + 6.0, 28.0, &c.icon, theme::category_color(Some(c.color)));
        cx.c.text_fit(&u.note, px + 42.0, y + (h - nt.lh()) / 2.0, *nw, nt);
        cx.c.text(amount, px + 42.0 + nw + 8.0, y + (h - at.lh()) / 2.0, at);
        if cx.tap(px, y, w, h) {
            picked = Some(u.clone());
        }
        px += w + gap;
    }
    kit::carousel_end(cx);
    if let Some(u) = picked {
        // A focused note field copies its own text back each frame: it takes the usual note too,
        // as a suggested note does, or the tap's note is lost.
        if cx.focus.is(NOTE) {
            cx.focus.text = u.note.clone();
            cx.host.keyboard(true, &u.note, false);
        }
        e.note = u.note;
        e.category = Some(u.category);
        e.category_chosen = true;
        e.amount = AmountInput::of(u.last_amount, e.amount.fraction_digits);
        e.pad_open = false;
    }
    y + h
}

/// A sheet opens over the editor: typing in the note stops, as a tap on the amount stops it, so
/// the keyboard does not stay up over the sheet with the hidden note focused.
fn close_note(cx: &mut Ctx) {
    if cx.focus.is(NOTE) {
        cx.focus.id = None;
        cx.focus.at = None;
        cx.host.keyboard(false, "", false);
    }
}

fn new_category(kind: TxType) -> Route {
    Route::CategoryEdit { id: None, kind: if kind == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense } }
}

/// The keypad (while it shows) and the save key, on the ground at the bottom.
fn footer(cx: &mut Ctx, e: &mut Entry, top: f32, h: f32, problem_line: bool, act: &mut Option<Act>) {
    let p = cx.p;
    let ground = p.ground[0];
    cx.c.rect_grad(-1000.0, top - 18.0, WIDTH + 2000.0, 18.0, 0.0, ground.a(0.0), ground);
    cx.c.rect(-1000.0, top, WIDTH + 2000.0, h + 40.0, 0.0, ground);
    let mut by = top + 12.0;
    if e.pad_open {
        let kw = (W - 2.0 * GAP) / 3.0;
        let keys = ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "⌫"];
        for (i, k) in keys.iter().enumerate() {
            if *k == "." && e.amount.fraction_digits == 0 {
                continue;
            }
            let (x, y) = (PAD + (i % 3) as f32 * (kw + GAP), by + (i / 3) as f32 * (KEY_H + GAP));
            cx.c.rect(x, y, kw, KEY_H, 14.0, p.card2);
            if cx.pressed(x, y, kw, KEY_H) {
                cx.c.rect(x, y, kw, KEY_H, 14.0, p.wash);
            }
            if *k == "⌫" {
                cx.c.icon("arrow-left", x + kw / 2.0, y + KEY_H / 2.0, 20.0, p.ink);
            } else {
                let t = Text::new(22.0, p.ink).medium().center();
                cx.c.text(k, x + kw / 2.0, y + (KEY_H - t.lh()) / 2.0, t);
            }
            if cx.tap(x, y, kw, KEY_H) {
                e.amount = match *k {
                    "." => e.amount.decimal(),
                    "⌫" => e.amount.backspace(),
                    d => e.amount.digit(d.parse().unwrap_or(0)),
                };
                if cx.focus.id.is_some() {
                    cx.focus.id = None;
                    cx.host.keyboard(false, "", false);
                }
            }
        }
        by += 4.0 * (KEY_H + GAP) + 4.0;
    } else if problem_line && let Some(why) = logic::validate(&e.draft()) {
        cx.c.text(why, WIDTH / 2.0, by, Text::new(ty::CAPTION, p.waiting_text).medium().center());
        by += 24.0;
    }
    let label = if e.editing.is_some() {
        "Save changes"
    } else {
        match e.kind {
            TxType::Expense => "Save expense",
            TxType::Income => "Save income",
            TxType::Transfer => "Save transfer",
        }
    };
    if kit::key(cx, PAD, by, W, KEY_H, label, None, KeyKind::Primary) {
        *act = Some(Act::Save { next: false });
    }
    // With the keypad folded there is room for the second way to save: and start the next one.
    if e.editing.is_none() && !e.pad_open {
        let t = Text::new(ty::UI, p.ink2).medium().center();
        let words = "Save and add another";
        let tw = cx.c.measure(words, t);
        let ky = by + KEY_H + 4.0;
        if cx.pressed(WIDTH / 2.0 - tw / 2.0 - 12.0, ky, tw + 24.0, 36.0) {
            cx.c.rect(WIDTH / 2.0 - tw / 2.0 - 12.0, ky, tw + 24.0, 36.0, 18.0, p.wash);
        }
        cx.c.text(words, WIDTH / 2.0, ky + (36.0 - t.lh()) / 2.0, t);
        if cx.tap(WIDTH / 2.0 - tw / 2.0 - 12.0, ky, tw + 24.0, 36.0) {
            *act = Some(Act::Save { next: true });
        }
    }
}

/// The sheet open over the editor: an account, every category, or another day.
fn sheet(cx: &mut Ctx, e: &mut Entry, dt: f32, act: &mut Option<Act>) {
    match e.picking {
        Some(pick @ (Pick::From | Pick::To)) => {
            let chosen = if pick == Pick::To { e.to_account } else { e.account };
            let mut items: Vec<(String, Option<String>)> =
                e.accounts.iter().map(|a| (a.name.clone(), Some(activity_data::account_type_label(a.r#type).to_string()))).collect();
            items.push(("New account".into(), Some("Add one to log against".into())));
            let selected = chosen.and_then(|id| e.accounts.iter().position(|a| a.id == id));
            let title = if pick == Pick::To { "To account" } else { "Account" };
            match widgets::sheet_list(cx, title, &items, selected, &mut e.sheet_scroll, dt) {
                SheetOut::Pick(i) if i < e.accounts.len() => {
                    let id = e.accounts[i].id;
                    if pick == Pick::To {
                        e.to_account = Some(id);
                    } else {
                        e.account = Some(id);
                    }
                    e.picking = None;
                }
                SheetOut::Pick(_) => {
                    e.picking = None;
                    e.made_for = Some(pick);
                    *act = Some(Act::Go(Route::AccountEdit { id: None, kind: None }));
                }
                SheetOut::Close => e.picking = None,
                SheetOut::Nothing => {}
            }
        }
        Some(Pick::Category) => {
            let cats: Vec<(i64, String)> = e.shown_categories().iter().map(|c| (c.id, c.name.clone())).collect();
            let mut items: Vec<(String, Option<String>)> = cats.iter().map(|(_, n)| (n.clone(), None)).collect();
            items.push(("New category".into(), None));
            let selected = e.category.and_then(|id| cats.iter().position(|(c, _)| *c == id));
            match widgets::sheet_list(cx, "Category", &items, selected, &mut e.sheet_scroll, dt) {
                SheetOut::Pick(i) if i < cats.len() => {
                    e.category = Some(cats[i].0);
                    e.category_chosen = true;
                    e.picking = None;
                    e.fold_if_typed();
                }
                SheetOut::Pick(_) => {
                    e.picking = None;
                    e.made_category = true;
                    *act = Some(Act::Go(new_category(e.kind)));
                }
                SheetOut::Close => e.picking = None,
                SheetOut::Nothing => {}
            }
        }
        Some(Pick::Day) => {
            let (out, day) = widgets::sheet_date(cx, "Day", e.date, &mut e.month);
            if let Some(d) = day {
                e.date = d;
                e.picking = None;
            } else if out == SheetOut::Close {
                e.picking = None;
            }
        }
        None => {}
    }
}

/// The typed amount in the currency's dress: "$12.5" while typing "12.5" ("12,5 $" in French).
pub fn dress(text: &str, fmt: &relay_money::money::MoneyFormatter) -> String {
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

/// The entry being edited is gone: say so and leave the editor.
fn leave_gone(app: &mut App) {
    if !app.pop() {
        app.push(Route::Overview);
    }
    app.say("This entry is gone", None);
}

/// A write to the entry being edited failed because the entry itself went meanwhile (a sync
/// deleted it while the editor was open), not for a reason the person can fix.
fn edited_gone(app: &App) -> bool {
    app.entry.editing.is_some_and(|id| app.store.ledger.tx(id).is_err())
}

/// Back on the editor closes the sheet open over it, if one is.
pub fn back(app: &mut App) -> bool {
    app.entry.picking.take().is_some()
}

fn save(app: &mut App, next: bool) {
    let e = &app.entry;
    if let Some(why) = logic::validate(&e.draft()) {
        let why = if why == "Pick an account" && e.accounts.is_empty() { "Add an account first" } else { why };
        app.say(why, None);
        return;
    }
    let Some(account) = e.account else { return };
    let minor = e.amount.minor();
    let note = e.note.trim().to_string();
    let to = if e.kind == TxType::Transfer { e.to_account } else { None };
    let category = if e.kind == TxType::Transfer { None } else { e.category };
    let left_after = e.left_after();
    let result = match e.editing {
        Some(id) => app
            .store
            .ledger
            .tx_update(
                id,
                &TxPatch {
                    r#type: Some(e.kind),
                    amount: Some(minor),
                    date: Some(e.date.to_string()),
                    account_id: Some(account),
                    to_account_id: to,
                    category_id: category,
                    note: Some(note),
                    clear_category: Some(category.is_none()),
                },
            )
            .map_err(|e| e.to_string()),
        None => app.store.add(&TxInput {
            r#type: e.kind,
            amount: minor,
            date: e.date.to_string(),
            account_id: account,
            to_account_id: to,
            category_id: category,
            note: (!note.is_empty()).then_some(note),
        }),
    };
    let editing = e.editing.is_some();
    match result {
        Ok(tx) => {
            let fmt = app.store.formatter();
            let words = logic::saved_notice(tx.r#type, tx.category.as_deref(), left_after, &fmt);
            app.say(words, if editing { None } else { Some(tx.id) });
            app.invalidate();
            if next && !editing {
                // Let go of the note field first, or next frame it writes what was typed into the
                // fresh entry's note.
                app.finish_typing();
                let keep = (app.entry.kind, app.entry.account, app.entry.to_account);
                app.entry = Entry::fresh(&app.store);
                // An archived account kept from the last entry stays on offer.
                for a in [keep.1, keep.2].into_iter().flatten() {
                    app.entry.offer_account(&app.store, a);
                }
                (app.entry.kind, app.entry.account, app.entry.to_account) = keep;
                app.ui.reset();
                *app.scroll() = Scroll::default();
            } else {
                app.pop();
            }
        }
        Err(_) if edited_gone(app) => leave_gone(app),
        Err(e) => app.say(e, None),
    }
}

/// Delete the entry being edited and leave; the notice's Undo puts it back.
fn delete(app: &mut App) {
    let Some(id) = app.entry.editing else { return };
    // The words name what is deleted (the stored entry), not what the editor was changed to.
    let words = match &app.entry.original {
        Some(t) => logic::deleted_notice(t.r#type, t.category.as_deref()),
        None => logic::deleted_notice(app.entry.kind, app.entry.category_name().as_deref()),
    };
    match app.store.delete(id) {
        Ok(()) => {
            app.invalidate();
            app.pop();
            // The overview it was opened from would read as gone: leave it too.
            if matches!(app.route(), Route::EntryView { id: v } if *v == id) {
                app.pop();
            }
            app.say_restore(words, id);
        }
        Err(_) if edited_gone(app) => leave_gone(app),
        Err(e) => app.say(e, None),
    }
}

/// The entry as the editor shows it, again, dated today. Changes made before Copy go into the
/// copy rather than being thrown away; the stored entry stays as it was.
fn duplicate(app: &mut App) {
    if app.entry.editing.is_none() {
        return;
    }
    let input = match app.entry.copy_input(model::today()) {
        Ok(i) => i,
        Err(why) => {
            app.say(why, None);
            return;
        }
    };
    match app.store.add(&input) {
        Ok(tx) => {
            app.invalidate();
            app.pop();
            // A copy is a new entry, as one added: Undo takes it out again.
            app.say("Copied to today", Some(tx.id));
        }
        Err(e) => app.say(e, None),
    }
}
