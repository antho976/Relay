//! Import a statement (Kotlin: ui/settings/ImportScreen.kt, ImportViewModel.kt, ImportLogic.kt):
//! which bank, the file, then a preview of every line (filed the way the owner filed the payee
//! before, lines already logged left out) and one import. A Wealthsimple investment file goes to
//! the investment import instead, and Tally's own CSV is read back as it was written.
//!
//! The file is read and parsed on a thread; the plan is redone on the UI thread only when a choice
//! changes (it is a pass over the lines). Nothing of the file is kept once the import is done.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::bank_statements::{self, Statement, StatementRead};
use relay_money::model::AccountType;
use relay_money::views::{AccountView, CategoryView, ImportAccount, ImportPreview};
use relay_money::wealthsimple::WsKind;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text};
use crate::import_logic::{self as logic, Choices, Plan, Source, TransferMode};
use crate::import_store::{self as store, Target};
use crate::kit::{self, Ctx, PAD, PicCard};
use crate::nav::Route;
use crate::settings_files::{self, Job, Poll};
use crate::settings_kit::{self as sk, GAP, GROUP_GAP, W};
use crate::theme;
use crate::ui::Scroll;
use crate::widgets::{self, SheetOut};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Stage {
    #[default]
    Choose,
    Reading,
    Preview,
    Done,
}

/// A sheet of choices open over the preview.
#[derive(Clone, Debug, PartialEq)]
enum Pick {
    Target,
    Counterpart,
    Statement,
    /// Where a file account (its number) goes.
    Map(String),
}

/// A Wealthsimple investment file under way.
struct Invest {
    text: String,
    preview: ImportPreview,
    mapping: Vec<(String, Option<i64>)>,
    statement_account: Option<i64>,
}

impl Invest {
    fn ready(&self) -> bool {
        self.preview.kind != WsKind::Statement || self.statement_account.is_some()
    }
    fn mapped(&self, number: &str) -> Option<i64> {
        self.mapping.iter().find(|(n, _)| n == number).and_then(|(_, id)| *id)
    }
}

#[derive(Default)]
pub struct State {
    stage: Stage,
    source: Option<Source>,
    problem: Option<String>,
    done: Option<String>,
    /// The pick is out with the platform.
    asked: bool,
    reading: Option<Job<(String, StatementRead)>>,
    statement: Option<Statement>,
    choices: Choices,
    target: Option<i64>,
    new_name: String,
    new_type: Option<AccountType>,
    accounts: Vec<AccountView>,
    categories: Vec<CategoryView>,
    learned: HashMap<String, i64>,
    /// What the target holds over the statement's days, and for which (target, first, last).
    existing: Vec<(Date, i64)>,
    existing_for: Option<(i64, Date, Date)>,
    plan: Plan,
    stale: bool,
    invest: Option<Invest>,
    /// The last import was investments: Done opens Investments.
    invested: bool,
    picking: Option<Pick>,
    sheet: Scroll,
}

impl State {
    fn new_type(&self) -> AccountType {
        self.new_type.unwrap_or(AccountType::Chequing)
    }
    fn target_name(&self) -> String {
        match self.target.and_then(|id| self.accounts.iter().find(|a| a.id == id)) {
            Some(a) => a.name.clone(),
            None if self.new_name.trim().is_empty() => "a new account".into(),
            None => self.new_name.trim().to_string(),
        }
    }
    fn account_name(&self, id: Option<i64>) -> Option<String> {
        id.and_then(|id| self.accounts.iter().find(|a| a.id == id)).map(|a| a.name.clone())
    }
    /// Back on a preview left open: entries may have come or gone since, and an account it names
    /// may have been archived or deleted. Duplicates are read again from the ledger, and a target,
    /// counterpart or mapping that is no longer open is let go (a new account, or the owner picks).
    fn revalidate(&mut self) {
        let open = |id: &i64| self.accounts.iter().any(|a| a.id == *id);
        self.target = self.target.filter(open);
        self.choices.counterpart = self.choices.counterpart.filter(open);
        if let Some(inv) = &mut self.invest {
            inv.statement_account = inv.statement_account.filter(open);
            for (_, id) in &mut inv.mapping {
                *id = id.filter(open);
            }
        }
        if self.picking == Some(Pick::Target) || self.picking == Some(Pick::Counterpart) {
            self.picking = None;
        }
        self.existing_for = None;
        self.stale = true;
    }
    fn reset(&mut self) {
        self.stage = Stage::Choose;
        self.problem = None;
        self.done = None;
        self.statement = None;
        self.invest = None;
        self.plan = Plan::default();
        self.picking = None;
        self.reading = None;
    }
}

enum Act {
    Back,
    Banks,
    Choose(Source),
    PickFile,
    Again,
    Confirm,
    Done,
    Open(Pick),
    Close,
    Picked(Pick, usize),
    NewType(AccountType),
    Flip,
    FileAccount(Option<String>),
    Transfers(TransferMode),
}

/// A sheet's rows: a title and an optional line under it.
type SheetItems = Vec<(String, Option<String>)>;

const NEW_NAME: &str = "import.new_name";

/// Back closes the account picker when it is open.
pub fn back(app: &mut App) -> bool {
    app.st.import.picking.take().is_some()
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.fresh && app.st.import.stage != Stage::Reading {
        let st = &mut app.st.import;
        st.accounts = app.store.ledger.accounts().unwrap_or_default().into_iter().filter(|a| !a.archived).collect();
        st.categories = app.store.ledger.categories().unwrap_or_default();
        if st.stage == Stage::Done {
            st.reset();
        }
        if st.stage == Stage::Preview {
            st.revalidate();
        }
    }
    let waiting = poll(app);
    if app.st.import.stale {
        replan(app);
    }
    let pg_raw = app.page_begin(dt);
    let mut pg = pg_raw;
    let fmt = app.store.formatter();
    let stage = app.st.import.stage;
    let has_footer = matches!(stage, Stage::Preview | Stage::Done) && !app.keyboard_up;
    let mut act = None;
    let bottom;
    let footer_at;
    let footer_tap;
    let sheet_tap;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.import;
        footer_at = sk::footer_top(&cx, sk::FOOTER_KEYS);
        sheet_tap = if st.picking.is_some() { widgets::hold(&mut cx) } else { None };
        footer_tap = if has_footer { sk::footer_hold(&mut cx, footer_at) } else { None };
        let mut y = pg.top;
        let title = if st.invest.is_some() { "Import investments" } else { "Import a statement" };
        let (back, _, h) = widgets::page_header(&mut cx, y, title, None);
        if back {
            act = Some(Act::Back);
        }
        y += h;
        y += 4.0;
        if let Some(problem) = &st.problem {
            y += sk::status(&mut cx, y, sk::Tone::Problem, problem).0 + GAP;
        }
        y = match stage {
            Stage::Choose => choose(&mut cx, y, st, &mut act),
            Stage::Reading => y + widgets::empty(&mut cx, PAD, y, W, "file", "Reading the file", "Bank statements are read on this phone."),
            Stage::Done => {
                let words = st.done.clone().unwrap_or_else(|| "Imported".into());
                y + widgets::empty(&mut cx, PAD, y, W, "check", &words, "The entries are in, filed and ready.")
            }
            Stage::Preview => match &st.invest {
                Some(_) => invest_preview(&mut cx, y, st, &mut act),
                None => statement_preview(&mut cx, y, st, &fmt, &mut act),
            },
        };
        bottom = y;
    }
    if has_footer {
        pg.bottom = footer_at;
    }
    app.page_end(&pg, bottom);
    {
        let mut cx = cx!(app);
        let st = &mut app.st.import;
        if has_footer {
            cx.ui.tap = footer_tap;
            sk::footer_ground(&mut cx, footer_at);
            let ky = footer_at + 14.0;
            if stage == Stage::Done {
                if kit::key(&mut cx, PAD, ky, W, sk::FOOTER_KEYS, "Done", Some("check"), kit::KeyKind::Primary) {
                    act = Some(Act::Done);
                }
            } else {
                let (label, can) = match &st.invest {
                    Some(i) => (logic::invest_action_label(&i.preview), i.ready() && logic::invest_count(&i.preview) > 0),
                    None => {
                        let n = st.plan.count();
                        (if n == 1 { "Import 1 entry".to_string() } else { format!("Import {n} entries") }, n > 0)
                    }
                };
                let w1 = (W - 8.0) / 2.4;
                if kit::key(&mut cx, PAD, ky, w1, sk::FOOTER_KEYS, "Another file", None, kit::KeyKind::Plain) {
                    act = Some(Act::Again);
                }
                if sk::wide_key(&mut cx, PAD + w1 + 8.0, ky, W - w1 - 8.0, &label, Some("download"), true, can) {
                    act = Some(Act::Confirm);
                }
            }
        }
        if let Some(pick) = st.picking.clone() {
            widgets::release(&mut cx, sheet_tap);
            let (title, items, selected) = sheet_items(st, &pick, &fmt);
            match widgets::sheet_list(&mut cx, &title, &items, selected, &mut st.sheet, dt) {
                SheetOut::Pick(i) => act = Some(Act::Picked(pick, i)),
                SheetOut::Close => act = Some(Act::Close),
                SheetOut::Nothing => {}
            }
        }
    }
    if let Some(a) = act {
        perform(app, a);
    }
    pg_raw.moving || waiting || app.st.import.picking.is_some()
}

/// Moves the file along: a pick that came back, a read that finished. True while waiting.
fn poll(app: &mut App) -> bool {
    if app.st.import.asked
        && let Some((name, bytes)) = settings_files::take_picked(app)
    {
        let st = &mut app.st.import;
        st.asked = false;
        st.reset();
        if bytes.is_empty() {
            st.problem = Some("That file could not be opened.".into());
            return false;
        }
        let digits = app.store.formatter().fraction_digits;
        let locale = &app.store.locale;
        let day_first = logic::day_first_for(&locale.language, &locale.country);
        st.stage = Stage::Reading;
        let _ = name;
        st.reading = Some(Job::spawn(move || {
            let text = bank_statements::decode_text(&bytes);
            let read = bank_statements::read(&text, digits, day_first);
            (text, read)
        }));
    }
    let Some(job) = &app.st.import.reading else { return false };
    match job.poll() {
        Poll::Running => true,
        Poll::Lost => {
            let st = &mut app.st.import;
            st.reading = None;
            st.stage = Stage::Choose;
            st.problem = Some("That file could not be read.".into());
            false
        }
        Poll::Done((text, read)) => {
            app.st.import.reading = None;
            opened(app, text, read);
            false
        }
    }
}

/// A file read: what it turned out to be.
fn opened(app: &mut App, text: String, read: StatementRead) {
    match read {
        StatementRead::Invalid(why) => {
            let st = &mut app.st.import;
            st.stage = Stage::Choose;
            st.problem = Some(why);
        }
        StatementRead::TallyCsv => match store::import_tally_csv(&mut app.store, &text) {
            Ok(words) => {
                let st = &mut app.st.import;
                st.stage = Stage::Done;
                st.invested = false;
                st.done = Some(words.clone());
                app.invalidate();
                app.say(words, None);
            }
            Err(why) => {
                let st = &mut app.st.import;
                st.stage = Stage::Choose;
                st.problem = Some(why);
            }
        },
        StatementRead::Investments => match app.store.ledger.invest_preview(&text, None) {
            Ok(preview) => {
                let st = &mut app.st.import;
                let held: Vec<AccountView> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).cloned().collect();
                let mapping = logic::guess_mapping(&preview.accounts, &held);
                let statement_account = if preview.kind == WsKind::Statement && held.len() == 1 { Some(held[0].id) } else { None };
                st.invest = Some(Invest { text, preview, mapping, statement_account });
                st.stage = Stage::Preview;
            }
            Err(e) => {
                let st = &mut app.st.import;
                st.stage = Stage::Choose;
                st.problem = Some(e.to_string());
            }
        },
        StatementRead::Ok(statement) => {
            let learned = logic::learned_categories(&store::learned_notes(&app.store, crate::model::today()), &app.st.import.categories);
            let st = &mut app.st.import;
            st.learned = learned;
            if st.new_name.trim().is_empty() {
                st.new_name = st.source.unwrap_or(Source::Other).account_name().into();
            }
            st.target = logic::guess_target(&st.accounts, st.source);
            let ty = st.account_type(st.target);
            let accounts = statement.accounts();
            st.choices = Choices { file_account: if accounts.len() > 1 { accounts.first().cloned() } else { None }, flip: logic::guess_flip(&statement, ty), ..Choices::default() };
            st.statement = Some(statement);
            st.existing_for = None;
            st.stage = Stage::Preview;
            st.stale = true;
        }
    }
}

impl State {
    fn account_type(&self, id: Option<i64>) -> AccountType {
        id.and_then(|id| self.accounts.iter().find(|a| a.id == id)).map(|a| a.r#type).unwrap_or(self.new_type())
    }
}

/// The plan again, after a choice changed.
fn replan(app: &mut App) {
    let st = &mut app.st.import;
    st.stale = false;
    let Some(statement) = &st.statement else { return };
    let want = match (st.target, statement.first(), statement.last()) {
        (Some(t), Some(a), Some(b)) => Some((t, a, b)),
        _ => None,
    };
    if want != st.existing_for {
        st.existing = match want {
            Some((t, a, b)) => store::existing_effects(&app.store, t, a, b),
            None => Vec::new(),
        };
        st.existing_for = want;
    }
    st.plan = logic::plan_import(statement, &st.choices, &st.categories, &st.learned, &st.existing);
}

fn perform(app: &mut App, act: Act) {
    match act {
        Act::Back => {
            app.pop();
        }
        Act::Banks => app.push(Route::Banks),
        Act::Choose(s) => {
            let st = &mut app.st.import;
            st.source = if st.source == Some(s) { None } else { Some(s) };
            st.problem = None;
            if let Some(s) = st.source {
                st.new_name = s.account_name().into();
                st.new_type = Some(AccountType::Chequing);
            }
        }
        Act::PickFile => {
            let st = &mut app.st.import;
            st.asked = true;
            st.problem = None;
            if st.new_name.is_empty() {
                st.new_name = Source::Other.account_name().into();
            }
            app.host.pick_file("text/*");
            poll(app);
        }
        Act::Again => {
            app.finish_typing();
            app.st.import.reset();
        }
        Act::Confirm => confirm(app),
        Act::Done => {
            let invested = app.st.import.invested;
            app.st.import.reset();
            if invested {
                app.push(Route::Investments);
            } else {
                app.pop();
            }
        }
        Act::Open(p) => {
            app.finish_typing();
            let st = &mut app.st.import;
            st.picking = Some(p);
            st.sheet = Scroll::default();
        }
        Act::Close => app.st.import.picking = None,
        Act::Picked(pick, i) => {
            let st = &mut app.st.import;
            st.picking = None;
            match pick {
                Pick::Target => {
                    let id = if i == 0 { None } else { st.accounts.get(i - 1).map(|a| a.id) };
                    st.target = id;
                    let ty = st.account_type(id);
                    if let Some(s) = &st.statement {
                        st.choices.flip = logic::guess_flip(s, ty);
                    }
                    if st.choices.counterpart == id {
                        st.choices.counterpart = None;
                    }
                    st.stale = true;
                }
                Pick::Counterpart => {
                    let others: Vec<i64> = st.accounts.iter().filter(|a| Some(a.id) != st.target).map(|a| a.id).collect();
                    st.choices.counterpart = others.get(i).copied();
                    st.stale = true;
                }
                Pick::Statement => {
                    let ids: Vec<i64> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).map(|a| a.id).collect();
                    if let Some(inv) = &mut st.invest {
                        inv.statement_account = ids.get(i).copied();
                    }
                }
                Pick::Map(number) => {
                    let ids: Vec<i64> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).map(|a| a.id).collect();
                    let id = if i == 0 { None } else { ids.get(i - 1).copied() };
                    if let Some(inv) = &mut st.invest {
                        match inv.mapping.iter_mut().find(|(n, _)| *n == number) {
                            Some(slot) => slot.1 = id,
                            None => inv.mapping.push((number, id)),
                        }
                    }
                }
            }
        }
        Act::NewType(t) => {
            let st = &mut app.st.import;
            st.new_type = Some(t);
            if st.target.is_none()
                && let Some(s) = &st.statement
            {
                st.choices.flip = logic::guess_flip(s, t);
            }
            st.stale = true;
        }
        Act::Flip => {
            let st = &mut app.st.import;
            st.choices.flip = !st.choices.flip;
            st.stale = true;
        }
        Act::FileAccount(a) => {
            let st = &mut app.st.import;
            st.choices.file_account = a;
            st.stale = true;
        }
        Act::Transfers(mode) => {
            let st = &mut app.st.import;
            if mode == TransferMode::Transfers && st.choices.counterpart.is_none() {
                st.choices.counterpart = st.accounts.iter().find(|a| Some(a.id) != st.target).map(|a| a.id);
            }
            st.choices.transfers = mode;
            st.stale = true;
        }
    }
}

/// Writes what the preview shows: the statement's lines, or the investment file.
fn confirm(app: &mut App) {
    app.finish_typing();
    if let Some(inv) = app.st.import.invest.take() {
        if !inv.ready() {
            app.st.import.invest = Some(inv);
            return;
        }
        let accounts: Vec<ImportAccount> = inv.mapping.iter().map(|(n, id)| ImportAccount { number: n.clone(), account_id: *id }).collect();
        let account = if inv.preview.kind == WsKind::Statement { inv.statement_account } else { None };
        match app.store.ledger.invest_import(&inv.text, &accounts, account) {
            Ok(r) => {
                let words = logic::invest_imported_line(r.holdings, r.activities, r.accounts_created, r.duplicates);
                let st = &mut app.st.import;
                st.stage = Stage::Done;
                st.done = Some(words.clone());
                st.invested = true;
                app.invalidate();
                app.say(words, None);
            }
            Err(e) => {
                let st = &mut app.st.import;
                st.problem = Some(e.to_string());
                st.invest = Some(inv);
            }
        }
        return;
    }
    let st = &mut app.st.import;
    if st.stage != Stage::Preview || st.plan.count() == 0 {
        return;
    }
    let target = match st.target {
        Some(id) => Target::Existing(id),
        None => Target::New {
            name: if st.new_name.trim().is_empty() { Source::Other.account_name().into() } else { st.new_name.trim().to_string() },
            ty: st.new_type(),
            // A new account opens where the statement says it stood before its first line.
            opening: st.plan.opening_from_statement.unwrap_or(0),
        },
    };
    let into = st.target_name();
    let (n, duplicates) = (st.plan.count(), st.plan.duplicates);
    match store::write(&mut app.store.ledger, &target, &st.plan.to_write) {
        Ok(id) => {
            let words = logic::imported_line(n, &into, duplicates);
            st.statement = None;
            st.target = Some(id);
            st.stage = Stage::Done;
            st.invested = false;
            st.done = Some(words.clone());
            app.invalidate();
            app.say(words, None);
        }
        Err(e) => st.problem = Some(format!("The import did not finish. Nothing was added. {e}")),
    }
}

fn sheet_items(st: &State, pick: &Pick, fmt: &relay_money::money::MoneyFormatter) -> (String, SheetItems, Option<usize>) {
    let investment: Vec<&AccountView> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).collect();
    match pick {
        Pick::Target => {
            let mut items = vec![("A new account".to_string(), None)];
            items.extend(st.accounts.iter().map(|a| (a.name.clone(), Some(fmt.format(a.balance)))));
            let sel = match st.target {
                None => Some(0),
                Some(id) => st.accounts.iter().position(|a| a.id == id).map(|i| i + 1),
            };
            ("Into".into(), items, sel)
        }
        Pick::Counterpart => {
            let others: Vec<&AccountView> = st.accounts.iter().filter(|a| Some(a.id) != st.target).collect();
            let sel = others.iter().position(|a| Some(a.id) == st.choices.counterpart);
            ("The other account".into(), others.iter().map(|a| (a.name.clone(), None)).collect(), sel)
        }
        Pick::Statement => {
            let chosen = st.invest.as_ref().and_then(|i| i.statement_account);
            let sel = investment.iter().position(|a| Some(a.id) == chosen);
            ("Its account".into(), investment.iter().map(|a| (a.name.clone(), None)).collect(), sel)
        }
        Pick::Map(number) => {
            let chosen = st.invest.as_ref().and_then(|i| i.mapped(number));
            let mut items = vec![("A new account".to_string(), None)];
            items.extend(investment.iter().map(|a| (a.name.clone(), None)));
            let sel = match chosen {
                None => Some(0),
                Some(id) => investment.iter().position(|a| a.id == id).map(|i| i + 1),
            };
            ("Goes into".into(), items, sel)
        }
    }
}

// The stages

fn choose(cx: &mut Ctx, mut y: f32, st: &State, act: &mut Option<Act>) -> f32 {
    // Skip the CSV: the bank picture, with the way to connect.
    let ch = 400.0;
    let (_, tapped) = kit::pic_card(
        cx,
        PAD,
        y,
        W,
        ch,
        "bank",
        &PicCard { title: "Skip the CSV", line: Some("Desjardins and Wealthsimple send new entries on their own, read only."), button: Some("Connect a bank"), ..Default::default() },
    );
    if tapped {
        *act = Some(Act::Banks);
    }
    y += ch + GROUP_GAP;

    y += sk::eyebrow(cx, y, "Or bring a file from");
    let rows: Vec<sk::Row> = Source::ALL.iter().map(|s| sk::Row::new(s.label()).icon(if *s == Source::Other { "file" } else { "bank" }).line(s.steps()).checked(st.source == Some(*s))).collect();
    let (h, tapped) = sk::rows_card(cx, y, &rows);
    if let Some(i) = tapped {
        *act = Some(Act::Choose(Source::ALL[i]));
    }
    y += h + 16.0;
    if sk::wide_key(cx, PAD, y, W, "Choose the file", Some("file"), true, true) {
        *act = Some(Act::PickFile);
    }
    y += 52.0 + 14.0;
    y += sk::note(cx, y, None, "The file is read on this phone. Only the entries you import are kept, and they sync like any other.");
    y
}

/// What the statement holds, two by two on one card: to import, filed, out and in.
fn summary(cx: &mut Ctx, y: f32, cells: &[(&str, String, String, Rgba); 4]) -> f32 {
    let p = cx.p;
    let cw = W / 2.0;
    let lt = Text::new(13.0, p.ink2).medium();
    let ft = Text::new(24.0, p.ink).medium().line(1.2);
    let st = Text::new(12.5, p.ink3).line(1.35);
    let cell_h = |cx: &mut Ctx, row: &[(&str, String, String, Rgba)]| {
        row.iter().map(|c| 18.0 + lt.lh() + 4.0 + ft.lh() + if c.2.is_empty() { 0.0 } else { 2.0 + cx.c.para_h(&c.2, cw - 36.0, st, 2) } + 18.0).fold(0.0, f32::max)
    };
    let h1 = cell_h(cx, &cells[..2]);
    let h2 = cell_h(cx, &cells[2..]);
    kit::card(cx, PAD, y, W, h1 + h2);
    let line = p.line_subtle;
    cx.c.rect(PAD + 18.0, y + h1, W - 36.0, 1.0, 0.0, line);
    cx.c.rect(PAD + cw, y + 18.0, 1.0, h1 + h2 - 36.0, 0.0, line);
    for (i, (label, figure, sub, colour)) in cells.iter().enumerate() {
        let x = PAD + (i % 2) as f32 * cw + 18.0;
        let mut at = y + if i < 2 { 0.0 } else { h1 } + 18.0;
        cx.c.text(label, x, at, lt);
        at += lt.lh() + 4.0;
        kit::money(cx, figure, x, at, 24.0, *colour, p.ink2);
        at += ft.lh();
        if !sub.is_empty() {
            cx.c.para(sub, x, at + 2.0, cw - 36.0, st, 2);
        }
    }
    h1 + h2
}

fn statement_preview(cx: &mut Ctx, mut y: f32, st: &mut State, fmt: &relay_money::money::MoneyFormatter, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let plan = &st.plan;
    let short = |d: Date| crate::model::short_date(&d.to_string());
    y += sk::note(cx, y - 4.0, None, &logic::plan_span(plan, short, |m| fmt.format_whole(m))) + 14.0;

    let cells = [
        ("To import", plan.count().to_string(), if plan.duplicates > 0 { format!("{} already there", plan.duplicates) } else { "None already there".into() }, p.ink),
        ("Filed", plan.categorized.to_string(), if plan.learned > 0 { format!("{} the way you filed them", plan.learned) } else { format!("{} left unfiled", plan.uncategorized) }, p.ink),
        ("Out", fmt.format(plan.out), String::new(), p.ink),
        ("In", fmt.format(plan.inn), String::new(), p.live_text),
    ];
    y += summary(cx, y, &cells) + GROUP_GAP;

    y += sk::eyebrow(cx, y, "Into");
    let name = st.target_name();
    if select_row(cx, y, "bank", &name) {
        *act = Some(Act::Open(Pick::Target));
    }
    y += SELECT_H + GAP;
    if st.target.is_none() {
        y += widgets::field(cx, NEW_NAME, PAD, y, W, "The new account's name", "Imported account", &mut st.new_name, false) + GAP;
        let types = [AccountType::Chequing, AccountType::Credit, AccountType::Savings];
        let chosen = types.iter().position(|t| *t == st.new_type()).unwrap_or(0);
        if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Chequing", "Card", "Savings"], chosen) {
            *act = Some(Act::NewType(types[i]));
        }
        y += 44.0 + GAP;
    }
    y += GROUP_GAP - GAP;
    let plan = &st.plan;
    if let Some(statement) = &st.statement {
        let accounts = statement.accounts();
        if accounts.len() > 1 {
            y += sk::eyebrow(cx, y, "Which account in the file");
            let mut labels = vec!["All"];
            labels.extend(accounts.iter().map(String::as_str));
            let sel = match &st.choices.file_account {
                None => Some(0),
                Some(a) => accounts.iter().position(|x| x == a).map(|i| i + 1),
            };
            let (picked, h) = widgets::pills(cx, PAD, y, W, &labels, sel);
            if let Some(i) = picked {
                *act = Some(Act::FileAccount(if i == 0 { None } else { accounts.get(i - 1).cloned() }));
            }
            y += h + GROUP_GAP;
        }
    }
    let (h, tapped) = sk::group(cx, y, Some("How it reads"), &[sk::Row::new("Purchases are positive").icon("card").line("Most card statements write money out as positive").toggle(st.choices.flip)]);
    if tapped.is_some() {
        *act = Some(Act::Flip);
    }
    y += h + GROUP_GAP;
    if plan.transfer_like > 0 {
        y += sk::eyebrow(cx, y, &format!("Card payments and transfers ({})", plan.transfer_like));
        let labels: Vec<&str> = TransferMode::ALL.iter().map(|m| m.label()).collect();
        let chosen = TransferMode::ALL.iter().position(|m| *m == st.choices.transfers).unwrap_or(0);
        if let Some(i) = widgets::segmented(cx, PAD, y, W, &labels, chosen) {
            *act = Some(Act::Transfers(TransferMode::ALL[i]));
        }
        y += 44.0 + GAP;
        if st.choices.transfers == TransferMode::Transfers {
            let other = st.account_name(st.choices.counterpart).unwrap_or_else(|| "Pick the other account".into());
            if select_row(cx, y, "transfer", &other) {
                *act = Some(Act::Open(Pick::Counterpart));
            }
            y += SELECT_H + GAP;
        }
        y += GROUP_GAP - GAP;
    }
    y + lines_panel(cx, y, st, fmt)
}

const SELECT_H: f32 = 64.0;

/// A card that opens a sheet: a glyph tile, words, and a chevron down.
fn select_row(cx: &mut Ctx, y: f32, glyph: &'static str, words: &str) -> bool {
    let p = cx.p;
    let h = SELECT_H;
    let tapped = kit::card_button(cx, PAD, y, W, h);
    sk::glyph_tile(cx, PAD + 14.0, y + (h - 36.0) / 2.0, 36.0, glyph, None);
    let x = PAD + 14.0 + 36.0 + 14.0;
    let t = Text::new(16.0, p.ink).medium();
    cx.c.text_fit(words, x, y + (h - t.lh()) / 2.0, PAD + W - 40.0 - x, t);
    cx.c.icon("chevron-down", PAD + W - 18.0 - 7.0, y + h / 2.0, 14.0, p.ink3);
    tapped
}

/// The lines, dimmed where they are already logged or left out (the first 200).
fn lines_panel(cx: &mut Ctx, y: f32, st: &State, fmt: &relay_money::money::MoneyFormatter) -> f32 {
    let p = cx.p;
    let plan = &st.plan;
    let top = y;
    let mut y = y + sk::eyebrow(cx, y, "Lines");
    y += sk::note(cx, y - 4.0, None, "Dimmed lines are already logged, or left out.") + 8.0;
    let shown = plan.lines.len().min(200);
    let line_h = kit::ENTRY_H;
    let more = plan.lines.len() > 200;
    let h = 8.0 + shown as f32 * line_h + if more { 36.0 } else { 0.0 };
    kit::card(cx, PAD, y, W, h);
    let mut ly = y + 4.0;
    let (x, w) = (PAD + 16.0, W - 32.0);
    for (i, l) in plan.lines.iter().take(200).enumerate() {
        if i > 0 {
            cx.c.rect(x + 54.0, ly, w - 54.0, 1.0, 0.0, p.line_subtle);
        }
        let out = l.duplicate || (l.transfer && st.choices.transfers == TransferMode::Skip);
        let colour = if l.transfer {
            theme::category_color(Some(2))
        } else if l.category_id.is_some() {
            theme::category_color(l.category_color)
        } else {
            p.ink3
        };
        let icon = if l.transfer { "transfer" } else { l.category_icon.as_deref().unwrap_or("dots") };
        kit::badge(cx, x, ly + 12.0, 40.0, icon, if out { colour.a(0.5) } else { colour });
        let amount = format!("{}{}", if l.amount > 0 { "+" } else { "" }, fmt.format(l.amount));
        let at = Text::new(16.0, if out { p.ink3 } else if l.amount > 0 { p.live_text } else { p.ink }).medium().right();
        let aw = cx.c.measure(&amount, at);
        cx.c.text(&amount, x + w, ly + 12.0, at);
        let tx = x + 54.0;
        let room = x + w - aw - 12.0 - tx;
        let note = if l.note.trim().is_empty() { "No description" } else { &l.note };
        cx.c.text_fit(note, tx, ly + 12.0, room, Text::new(16.0, if out { p.ink3 } else { p.ink }).medium());
        let mut parts = vec![crate::model::short_date(&l.date.to_string())];
        parts.push(l.category_name.clone().unwrap_or_else(|| if l.transfer { "Transfer".into() } else { "Unfiled".into() }));
        if l.duplicate {
            parts.push("already there".into());
        }
        if l.learned {
            parts.push("as you filed it".into());
        }
        cx.c.text_fit(&parts.join(" · "), tx, ly + 35.0, x + w - tx, Text::new(13.0, if out { p.ink3 } else { p.ink2 }));
        ly += line_h;
    }
    if more {
        cx.c.text(&format!("And {} more.", plan.lines.len() - 200), x, ly + 8.0, Text::new(13.0, p.ink3));
    }
    y + h - top
}

fn invest_preview(cx: &mut Ctx, mut y: f32, st: &State, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let Some(inv) = &st.invest else { return y };
    let pv = &inv.preview;
    let short = |d: Date| crate::model::short_date(&d.to_string());
    y += sk::note(cx, y - 4.0, None, &logic::invest_file_line(pv, short)) + 14.0;
    let into = st.account_name(inv.statement_account);
    let head = format!("{} {}", logic::invest_count(pv), logic::invest_plan_line(pv, into.as_deref()));
    let tt = Text::new(20.0, p.ink).semi().line(1.25);
    let ct = Text::new(13.0, p.ink2);
    let head_h = cx.c.para_h(&head, W - 40.0, tt, 3);
    let card_h = 20.0 + head_h + if pv.duplicates > 0 { 6.0 + ct.lh() } else { 0.0 } + 20.0;
    kit::card(cx, PAD, y, W, card_h);
    cx.c.para(&head, PAD + 20.0, y + 20.0, W - 40.0, tt, 3);
    if pv.duplicates > 0 {
        cx.c.text(&format!("{} already in Tally", pv.duplicates), PAD + 20.0, y + 20.0 + head_h + 6.0, ct);
    }
    y += card_h + GROUP_GAP;
    if pv.kind == WsKind::Statement {
        y += sk::eyebrow(cx, y, "Its account");
        let name = into.unwrap_or_else(|| "Pick the account".into());
        if select_row(cx, y, "trend", &name) {
            *act = Some(Act::Open(Pick::Statement));
        }
        y += SELECT_H + GROUP_GAP;
    } else {
        y += sk::eyebrow(cx, y, "Accounts in the file");
        let labels: Vec<(String, String)> = pv
            .accounts
            .iter()
            .map(|f| {
                let label = format!("{} · {}", if f.name.trim().is_empty() { f.registration.label().to_string() } else { f.name.clone() }, f.number);
                let into = match inv.mapped(&f.number).and_then(|id| st.account_name(Some(id))) {
                    Some(n) => n,
                    None => format!("New: {}", logic::new_account_name(f)),
                };
                (label, format!("Into {into}"))
            })
            .collect();
        let rows: Vec<sk::Row> = labels.iter().map(|(l, i)| sk::Row::new(l).icon("trend").line(i).opens()).collect();
        let (h, tapped) = sk::rows_card(cx, y, &rows);
        if let Some(i) = tapped
            && let Some(f) = pv.accounts.get(i)
        {
            *act = Some(Act::Open(Pick::Map(f.number.clone())));
        }
        y += h + GROUP_GAP;
    }
    for s in pv.skipped.iter().take(5) {
        y += cx.c.para(&format!("Line {}: {}", s.line, s.reason), PAD + 2.0, y, W - 4.0, Text::new(13.0, p.waiting_text), 3) + 4.0;
    }
    y
}

/// Opens the page's preview on a file already in hand (a statement shared to Tally, or a test):
/// it is read as a picked file is.
pub fn open_file(app: &mut App, name: String, bytes: Vec<u8>) {
    app.st.import.asked = true;
    app.file_picked(name, bytes);
}

/// The stage the page is at, in words: "choose", "reading", "preview" or "done" (for tests).
pub fn stage(app: &App) -> &'static str {
    match app.st.import.stage {
        Stage::Choose => "choose",
        Stage::Reading => "reading",
        Stage::Preview => "preview",
        Stage::Done => "done",
    }
}
