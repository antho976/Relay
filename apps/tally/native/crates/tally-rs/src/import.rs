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
    /// A statement's preview read again for the account it goes into: its lines' uids are made
    /// from that account's, so which are already in is only known once one is picked (read with
    /// none, every line counts as new and the preview offers lines Import then finds there).
    fn reread(&mut self, ledger: &relay_money::ledger::Ledger) -> Result<(), String> {
        if self.preview.kind != WsKind::Statement {
            return Ok(());
        }
        self.preview = ledger.invest_preview(&self.text, self.statement_account).map_err(|e| e.to_string())?;
        Ok(())
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
    /// `app.ledger_gen` when `existing` was read: a sync or an edit since may have logged some of
    /// the statement's lines, so they are read again before Import can write them twice.
    existing_gen: u64,
    plan: Plan,
    /// Lines the plan writes before the target's first entry, and what that does to its opening.
    earlier: Option<logic::Earlier>,
    stale: bool,
    invest: Option<Invest>,
    /// The last import was investments: Done opens Investments.
    invested: bool,
    picking: Option<Pick>,
    sheet: Scroll,
    /// Each account's figure as its row on the Accounts page shows it, read when the Into sheet
    /// opens: one with no figure there ("No balance") is missing.
    worth: HashMap<i64, i64>,
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
    /// Nothing left to write, but the target's opening still holds lines already logged (an
    /// import stopped between the two): Import moves just the opening.
    fn only_moves(&self) -> bool {
        self.plan.count() == 0 && self.target.is_some() && matches!(self.earlier, Some(logic::Earlier::Move { .. }))
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
        self.earlier = None;
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
    NewAccount,
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
        // Opened anew (not uncovered by Back): an error from an earlier visit is not this one's.
        if !app.returned && st.stage == Stage::Choose {
            st.problem = None;
        }
        if st.stage == Stage::Preview {
            st.revalidate();
            // Back from making the investment account a statement needs: with just the one, it
            // is the one, as when the file was opened.
            let held: Vec<i64> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).map(|a| a.id).collect();
            if let Some(inv) = &mut st.invest
                && inv.preview.kind == WsKind::Statement
                && inv.statement_account.is_none()
                && held.len() == 1
            {
                inv.statement_account = Some(held[0]);
            }
            // Its account may have changed (picked, made, or let go), and the ledger with it.
            if let Some(inv) = &mut st.invest
                && let Err(why) = inv.reread(&app.store.ledger)
            {
                st.problem = Some(why);
            }
        }
    }
    {
        let st = &mut app.st.import;
        if st.existing_for.is_some() && st.existing_gen != app.ledger_gen {
            st.existing_for = None;
            st.stale = true;
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
        // Done after investments: confirm took `invest`, so `invested` says what was imported.
        let title = if st.invest.is_some() || (stage == Stage::Done && st.invested) { "Import investments" } else { "Import a statement" };
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
                        let label = match n {
                            // Every line is in, but an import that stopped half way left the
                            // opening holding them: the move is all there is to do.
                            0 if st.only_moves() => "Move its starting balance".to_string(),
                            1 => "Import 1 entry".to_string(),
                            n => format!("Import {n} entries"),
                        };
                        (label, n > 0 || st.only_moves())
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
    // The picker closed with nothing picked: stop waiting for it.
    if app.st.import.asked && settings_files::take_cancelled() {
        app.st.import.asked = false;
    }
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
                let mut inv = Invest { text, preview, mapping, statement_account };
                // The one account picked for it: what of the statement is already in is read for it.
                if inv.statement_account.is_some()
                    && let Err(why) = inv.reread(&app.store.ledger)
                {
                    st.problem = Some(why);
                }
                st.invest = Some(inv);
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
            // Each file starts from its bank's defaults: what the last import made (a card, a
            // savings account, its name) must not carry over to this one. A choice made on the
            // preview from here on stays.
            st.new_type = None;
            st.new_name = logic::default_name(st.source, false).into();
            // The account the last statement from this bank went into comes first: renamed since
            // ("RBC chequing"), it no longer reads as the bank's or by the name the import gave it,
            // and a new account would count the overlap's money twice.
            // Where the last file naming the same account of the file went comes before that: a
            // bank's chequing and its card can read alike, but not name the same account.
            let card = statement.positive_share() > logic::CARD_POSITIVE_SHARE;
            let by_file = statement.accounts().first().and_then(|f| last_target(&app.store, &logic::last_target_pref_for(st.source, f)));
            let by_source = last_target(&app.store, &logic::last_target_pref(st.source, card));
            let guess = logic::guess_target(&st.accounts, st.source, statement.positive_share());
            st.target = logic::pick_target(&st.accounts, by_file, by_source, card, guess);
            // A card's file (purchases positive): a new account for it is a card, so its purchases
            // come in as spending, not income. Set even when it found a card, for "A new account"
            // picked in its place.
            if card && st.new_type() == AccountType::Chequing {
                st.new_type = Some(AccountType::Credit);
                if st.new_name == logic::default_name(st.source, false) {
                    st.new_name = logic::default_name(st.source, true).into();
                }
            }
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
        st.existing_gen = app.ledger_gen;
    }
    st.plan = logic::plan_import(statement, &st.choices, &st.categories, &st.learned, &st.existing);
    // An older statement into an account a later one made: its opening already holds the lines
    // before its first entry. Read again with the plan, so a sync since is seen before Import.
    st.earlier = st.target.and_then(|t| store::opening_of(&app.store, t)).and_then(|at| logic::earlier_lines(&st.plan, &at));
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
            // The new account's name and type are set from the bank (or none picked) when the
            // file is read, so a bank picked and then let go leaves nothing of its name behind.
        }
        Act::PickFile => {
            let st = &mut app.st.import;
            st.asked = true;
            st.problem = None;
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
            // Import opened from Investments goes back to it; it isn't opened a second time.
            let under = app.nav.len().checked_sub(2).map(|i| &app.nav[i].0);
            if invested && under != Some(&Route::Investments) {
                // In Import's place, so Back from Investments doesn't land on the finished import.
                app.replace(Route::Investments);
            } else {
                app.pop();
            }
        }
        Act::Open(p) => {
            app.finish_typing();
            if p == Pick::Target {
                app.st.import.worth = worths(&app.store);
            }
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
                        if let Err(why) = inv.reread(&app.store.ledger) {
                            st.problem = Some(why);
                        }
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
            // That account of the file goes where its last file went, when one did.
            let card = st.statement.as_ref().is_some_and(|s| s.positive_share() > logic::CARD_POSITIVE_SHARE);
            let last = a.as_deref().and_then(|f| last_target(&app.store, &logic::last_target_pref_for(st.source, f)));
            if let Some(id) = logic::remembered_target(&st.accounts, last, card)
                && st.target != Some(id)
            {
                st.target = Some(id);
                let ty = st.account_type(Some(id));
                if let Some(s) = &st.statement {
                    st.choices.flip = logic::guess_flip(s, ty);
                }
                if st.choices.counterpart == Some(id) {
                    st.choices.counterpart = None;
                }
            }
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
        Act::NewAccount => {
            app.finish_typing();
            app.push(Route::AccountEdit { id: None, kind: Some(AccountType::Investment) });
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
                // A failed try before this one is over: its words don't belong on Done.
                st.problem = None;
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
    if st.stage != Stage::Preview || (st.plan.count() == 0 && !st.only_moves()) {
        return;
    }
    // A name left blank saves as the field's placeholder, and the done line says that name.
    let new_name = if st.new_name.trim().is_empty() { Source::Other.account_name().to_string() } else { st.new_name.trim().to_string() };
    let into = if st.target.is_some() { st.target_name() } else { new_name.clone() };
    let target = match st.target {
        Some(id) => Target::Existing(id),
        None => Target::New {
            name: new_name,
            ty: st.new_type(),
            // A new account opens where the statement says it stood before its first line.
            opening: st.plan.opening_from_statement.unwrap_or(0),
        },
    };
    // Remembered by how the file reads, as it is looked up next time, so a bank's card and
    // chequing statements each keep their own. A card's file that writes purchases below zero
    // reads like chequing: kept with those, it still finds the card it went into.
    let card = st.statement.as_ref().is_some_and(|s| s.positive_share() > logic::CARD_POSITIVE_SHARE);
    // And by the account of the file it took, when it names one: the one picked, or its only one.
    let file_account = st.choices.file_account.clone().or_else(|| st.statement.as_ref().map(|s| s.accounts()).filter(|a| a.len() == 1).and_then(|mut a| a.pop()));
    let source = st.source;
    let (n, duplicates) = (st.plan.count(), st.plan.duplicates);
    // Lines before the account's first entry that its opening already holds: the opening moves
    // back by them, or the account would count them twice and read off from its bank.
    let moved = match (&target, st.earlier) {
        (Target::Existing(_), Some(logic::Earlier::Move { from, to, back_to })) => Some((from, to, back_to)),
        _ => None,
    };
    let written = match (&target, moved) {
        (Target::Existing(id), Some((from, to, _))) => store::write_moving_opening(&mut app.store, *id, from, to, &st.plan.to_write),
        _ => store::write(&mut app.store.ledger, &target, &st.plan.to_write).map(|account| store::Written { account, moved: false }),
    };
    match written {
        Ok(store::Written { account: id, moved: did_move }) => {
            remember_target(&mut app.store, source, card, file_account.as_deref(), id);
            let st = &mut app.st.import;
            let mut words = logic::imported_line(n, &into, duplicates);
            if let Some((_, _, back_to)) = moved {
                let back_to = crate::model::short_date(&back_to.to_string());
                words = match (did_move, n) {
                    (true, 0) => logic::only_opening_moved_line(&into, &back_to, duplicates),
                    (true, _) => logic::opening_moved_line(&words, &back_to),
                    (false, _) => logic::opening_not_moved_line(&words),
                };
            }
            // A failed try before this one is over: its words don't belong on Done.
            st.problem = None;
            st.statement = None;
            st.target = Some(id);
            st.stage = Stage::Done;
            st.invested = false;
            st.done = Some(words.clone());
            app.invalidate();
            app.say(words, None);
        }
        Err(e) => app.st.import.problem = Some(format!("The import did not finish. Nothing was added. {e}")),
    }
}

/// The account the last statement kept under the preference `key` went into (its bank's, a card's
/// or another's, or its account of the file's), if it is still in the ledger. Kept by uid, so a
/// rename or a restart keeps it.
fn last_target(store: &crate::model::Store, key: &str) -> Option<i64> {
    use rusqlite::OptionalExtension;
    let uid = store.pref(key)?;
    let db = store.sql().ok()?;
    db.prepare_cached("SELECT id FROM accounts WHERE uid = ?1 AND deleted = 0").ok()?.query_row([uid], |r| r.get(0)).optional().ok().flatten()
}

/// Keeps where this statement went, for the next one from the same bank, and for the next naming
/// the same account of the file.
fn remember_target(store: &mut crate::model::Store, source: Option<Source>, card: bool, file_account: Option<&str>, id: i64) {
    use rusqlite::OptionalExtension;
    let uid: Option<String> = store
        .sql()
        .ok()
        .and_then(|db| db.prepare_cached("SELECT uid FROM accounts WHERE id = ?1 AND deleted = 0").ok()?.query_row([id], |r| r.get(0)).optional().ok().flatten());
    if let Some(uid) = uid {
        let _ = store.set_pref(&logic::last_target_pref(source, card), &uid);
        if let Some(f) = file_account {
            let _ = store.set_pref(&logic::last_target_pref_for(source, f), &uid);
        }
    }
}

/// Each open account's figure as the Accounts page shows it, those it shows one for.
fn worths(store: &crate::model::Store) -> HashMap<i64, i64> {
    let Ok((rows, now)) = crate::accounts_store::worth_now(store, crate::model::today()) else { return HashMap::new() };
    rows.iter()
        .filter_map(|r| {
            let &(v, priced) = now.get(&r.view.id)?;
            crate::accounts_store::counted(r, v, priced).then_some((r.view.id, v))
        })
        .collect()
}

/// A balance as the Accounts list shows it: a debt with its minus.
fn signed_money(v: i64, fmt: &relay_money::money::MoneyFormatter) -> String {
    if v < 0 { format!("−{}", fmt.format(-v)) } else { fmt.format(v) }
}

fn sheet_items(st: &State, pick: &Pick, fmt: &relay_money::money::MoneyFormatter) -> (String, SheetItems, Option<usize>) {
    let investment: Vec<&AccountView> = st.accounts.iter().filter(|a| a.r#type == AccountType::Investment).collect();
    match pick {
        Pick::Target => {
            let mut items = vec![("A new account".to_string(), None)];
            // What the Accounts page says each is worth, not its raw balance (spending alone on an
            // account given nowhere to start, or an investment's cash without its holdings).
            items.extend(st.accounts.iter().map(|a| (a.name.clone(), st.worth.get(&a.id).map(|&v| signed_money(v, fmt)))));
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
    y += summary(cx, y, &cells);
    // Lines whose date could not be read are not in the plan: say so rather than drop them.
    let unread = st.statement.as_ref().map(|s| s.errors.as_slice()).unwrap_or_default();
    if !unread.is_empty() {
        y += 12.0;
        let head = if unread.len() == 1 { "1 line could not be read and is left out.".to_string() } else { format!("{} lines could not be read and are left out.", unread.len()) };
        let wt = Text::new(13.0, p.waiting_text);
        y += cx.c.para(&head, PAD + 2.0, y, W - 4.0, wt, 3) + 4.0;
        for why in unread.iter().take(2) {
            y += cx.c.para(why, PAD + 2.0, y, W - 4.0, wt, 3) + 4.0;
        }
    }
    y += GROUP_GAP;

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
            if st.accounts.iter().any(|a| Some(a.id) != st.target) {
                let other = st.account_name(st.choices.counterpart).unwrap_or_else(|| "Pick the other account".into());
                if select_row(cx, y, "transfer", &other) {
                    *act = Some(Act::Open(Pick::Counterpart));
                }
                y += SELECT_H + GAP;
            } else {
                // The picker would be empty and the transfers left out with nothing saying why.
                let words = "A transfer needs another account at its other end, and there is none yet, so these lines are left out. Bring them in as Entries, or add the other account first.";
                y += sk::note(cx, y - 4.0, None, words) + GAP;
            }
        }
        y += GROUP_GAP - GAP;
    }
    // A new account opens where the statement starts; what is left out would leave it off from
    // where the statement ends, with nothing else to say so.
    if st.target.is_none()
        && let Some(words) = logic::new_account_gap_line(plan, |m| fmt.format(m))
    {
        y += sk::note(cx, y - 4.0, None, &words) + GROUP_GAP;
    }
    // An account that exists: lines before its first entry may already be in its opening.
    if st.target.is_some()
        && let Some(e) = &st.earlier
    {
        let words = logic::earlier_line(e, &st.target_name(), plan.skipped, short, |m| fmt.format(m));
        y += sk::note(cx, y - 4.0, None, &words) + GROUP_GAP;
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
        // Dimmed as the plan leaves it out: a duplicate, a skipped transfer, or a transfer with no
        // account picked for its other end yet.
        let left_out = match st.choices.transfers {
            TransferMode::Skip => true,
            TransferMode::Transfers => st.choices.counterpart.is_none(),
            TransferMode::Entries => false,
        };
        // A zero-amount line is never written either.
        let out = l.duplicate || (l.transfer && left_out) || l.amount == 0;
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
        if st.accounts.iter().any(|a| a.r#type == AccountType::Investment) {
            let name = into.unwrap_or_else(|| "Pick the account".into());
            if select_row(cx, y, "trend", &name) {
                *act = Some(Act::Open(Pick::Statement));
            }
            y += SELECT_H + GROUP_GAP;
        } else {
            // The picker would be empty and Import would stay off with nothing saying why.
            y += sk::note(cx, y - 4.0, None, "A statement goes into one investment account, and you have none yet. Add one with the type Investment, then come back.") + 10.0;
            if sk::wide_key(cx, PAD, y, W, "Add an investment account", Some("plus"), true, true) {
                *act = Some(Act::NewAccount);
            }
            y += 52.0 + GROUP_GAP;
        }
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

/// Picks `id` (None: a new account) in the Into sheet, as a tap on its row does (for tests).
#[doc(hidden)]
pub fn pick_into(app: &mut App, id: Option<i64>) {
    let st = &app.st.import;
    let i = match id {
        None => 0,
        Some(id) => st.accounts.iter().position(|a| a.id == id).map_or(0, |i| i + 1),
    };
    perform(app, Act::Picked(Pick::Target, i));
}

/// Picks the bank on the page's first step, as a tap on its row does (for tests).
#[doc(hidden)]
pub fn choose_source(app: &mut App, source: Source) {
    if app.st.import.source != Some(source) {
        perform(app, Act::Choose(source));
    }
}

/// The failure the page shows, if any (for tests).
#[doc(hidden)]
pub fn problem(app: &App) -> Option<&str> {
    app.st.import.problem.as_deref()
}

/// The account the preview goes into, or the last import went into (None: a new one; for tests).
#[doc(hidden)]
pub fn into(app: &App) -> Option<i64> {
    app.st.import.target
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
