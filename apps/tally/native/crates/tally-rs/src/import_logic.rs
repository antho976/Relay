//! The bank import's pure half (Kotlin `ImportLogic.kt`, and the investment import's words from
//! `InvestLogic.kt`): which bank, what each statement line becomes, which lines are already
//! logged, and the totals the preview reads. No drawing and no ledger, so tests hold all of it.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::bank_statements::{Statement, StatementRow, normalize};
use relay_money::copy;
use relay_money::model::{AccountType, CategoryKind, Registration, TxType};
use relay_money::payees;
use relay_money::views::{AccountView, CategoryView, ImportPreview, ImportPreviewAccount};
use relay_money::wealthsimple::WsKind;

/// The banks the import page names, with where their CSV is and what a new account from them is
/// called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Desjardins,
    Wealthsimple,
    Other,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Desjardins, Source::Wealthsimple, Source::Other];

    pub fn key(self) -> &'static str {
        match self {
            Source::Desjardins => "desjardins",
            Source::Wealthsimple => "wealthsimple",
            Source::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Source::Desjardins => "Desjardins",
            Source::Wealthsimple => "Wealthsimple",
            Source::Other => "Another bank",
        }
    }
    pub fn steps(self) -> &'static str {
        match self {
            Source::Desjardins => "On desjardins.com, open the account in AccèsD, pick the period, then export it as CSV",
            Source::Wealthsimple => "On wealthsimple.com, open Documents, then Statements, and download a month as CSV",
            Source::Other => "Most banks export an account's transactions as CSV from their website",
        }
    }
    pub fn account_name(self) -> &'static str {
        match self {
            Source::Desjardins => "Desjardins chequing",
            Source::Wealthsimple => "Wealthsimple Chequing",
            Source::Other => "Imported account",
        }
    }
    /// What a new card from this bank's statement is called.
    pub fn card_name(self) -> &'static str {
        match self {
            Source::Desjardins => "Desjardins card",
            Source::Wealthsimple => "Wealthsimple card",
            Source::Other => "Imported card",
        }
    }
}

/// What the import names a new account for a statement from `source` (none picked reads as
/// another bank's), a card's when the statement reads like one.
pub fn default_name(source: Option<Source>, card: bool) -> &'static str {
    let s = source.unwrap_or(Source::Other);
    if card { s.card_name() } else { s.account_name() }
}

/// What happens to the lines that move money between the owner's own accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransferMode {
    #[default]
    Skip,
    Transfers,
    Entries,
}

impl TransferMode {
    pub const ALL: [TransferMode; 3] = [TransferMode::Skip, TransferMode::Transfers, TransferMode::Entries];
    pub fn label(self) -> &'static str {
        match self {
            TransferMode::Skip => "Skip",
            TransferMode::Transfers => "Transfers",
            TransferMode::Entries => "Entries",
        }
    }
}

/// The choices the preview offers, as the owner set them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choices {
    /// The account in the file to take, when it holds several; none takes them all.
    pub file_account: Option<String>,
    /// True when the statement writes money out as positive (most card statements).
    pub flip: bool,
    pub transfers: TransferMode,
    /// The account at the other end of the moves, for [`TransferMode::Transfers`].
    pub counterpart: Option<i64>,
}

/// One statement line as the import reads it, before the owner says import.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedLine {
    pub date: Date,
    pub note: String,
    /// Signed from the account's side: below zero leaves it.
    pub amount: i64,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub category_icon: Option<String>,
    pub category_color: Option<i64>,
    pub transfer: bool,
    /// Its twin (same day, same amount) is already in the account.
    pub duplicate: bool,
    /// The category came from the owner's own entries, not from the merchant list.
    pub learned: bool,
    /// The statement's balance after this line, from the account's side, when it gives one.
    pub balance: Option<i64>,
}

/// One line as the import writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportLine {
    pub date: Date,
    pub note: String,
    pub amount: i64,
    pub category_id: Option<i64>,
    /// Set for a move between the owner's own accounts: the account at the other end.
    pub counterpart: Option<i64>,
}

/// Everything the preview shows and the import writes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub lines: Vec<PlannedLine>,
    pub to_write: Vec<ImportLine>,
    pub duplicates: usize,
    pub transfer_like: usize,
    pub skipped: usize,
    pub categorized: usize,
    pub learned: usize,
    pub uncategorized: usize,
    pub out: i64,
    pub inn: i64,
    pub first: Option<Date>,
    pub last: Option<Date>,
    /// What the account held before the statement's first line, by the statement's own balances.
    pub opening_from_statement: Option<i64>,
    pub end_balance: Option<i64>,
}

impl Plan {
    pub fn count(&self) -> usize {
        self.to_write.len()
    }
}

/// Past this share of positive lines a card statement reads flipped.
pub const CARD_POSITIVE_SHARE: f32 = 0.6;

/// Whether a statement into an account of `ty` most likely writes money out as positive.
pub fn guess_flip(statement: &Statement, ty: AccountType) -> bool {
    ty == AccountType::Credit && statement.positive_share() > CARD_POSITIVE_SHARE
}

/// The lines in the order they happened: a file listing the newest first is turned round.
pub fn chronological(rows: Vec<StatementRow>) -> Vec<StatementRow> {
    if rows.len() < 2 {
        return rows;
    }
    let (first, last) = (rows[0].date, rows[rows.len() - 1].date);
    let newest_first = if first != last {
        first > last
    } else {
        // One day: the dates can't say, so read it from how the balances chain. Oldest first, each
        // line's balance is the one before plus its amount; newest first, the other way round.
        let (mut oldest, mut newest) = (0, 0);
        for w in rows.windows(2) {
            if let (Some(a), Some(b)) = (w[0].balance, w[1].balance) {
                if b == a + w[1].amount {
                    oldest += 1;
                }
                if a == b + w[0].amount {
                    newest += 1;
                }
            }
        }
        newest > oldest
    };
    if newest_first { rows.into_iter().rev().collect() } else { rows }
}

/// A note the owner filed, from the ledger: what the import learns payees from.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteUse {
    pub note: String,
    pub category_id: i64,
    pub r#type: TxType,
}

fn kind_name(k: CategoryKind) -> &'static str {
    match k {
        CategoryKind::Expense => "EXPENSE",
        CategoryKind::Income => "INCOME",
    }
}

/// The category each payee was filed under most often, by kind and payee key.
pub fn learned_categories(uses: &[NoteUse], categories: &[CategoryView]) -> HashMap<String, i64> {
    let live: HashMap<i64, &CategoryView> = categories.iter().filter(|c| !c.archived).map(|c| (c.id, c)).collect();
    // Per key, each category's count, in the order first seen (a tie goes to the first).
    let mut counts: HashMap<String, Vec<(i64, usize)>> = HashMap::new();
    for u in uses {
        let Some(c) = live.get(&u.category_id) else { continue };
        let kind = if u.r#type == TxType::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if c.kind != kind {
            continue;
        }
        let key = payees::key(&u.note);
        if key.is_empty() {
            continue;
        }
        let e = counts.entry(format!("{}:{key}", kind_name(kind))).or_default();
        match e.iter_mut().find(|(id, _)| *id == c.id) {
            Some(slot) => slot.1 += 1,
            None => e.push((c.id, 1)),
        }
    }
    counts
        .into_iter()
        .filter_map(|(k, ids)| {
            let mut best: Option<(i64, usize)> = None;
            for (id, n) in ids {
                if best.is_none_or(|(_, b)| n > b) {
                    best = Some((id, n));
                }
            }
            best.map(|(id, _)| (k, id))
        })
        .collect()
}

/// What the ledger already holds, counted by a line's key: each logged entry stands twin to one
/// line of a file only, so a file holding the same coffee twice, once logged, adds the second.
#[derive(Clone, Debug)]
pub struct Twins<K>(HashMap<K, usize>);

impl<K: std::hash::Hash + Eq> Twins<K> {
    pub fn new(logged: impl IntoIterator<Item = K>) -> Twins<K> {
        let mut counts = HashMap::new();
        for k in logged {
            *counts.entry(k).or_default() += 1;
        }
        Twins(counts)
    }

    /// True when a logged twin of `key` is left, which it then uses up.
    pub fn take(&mut self, key: &K) -> bool {
        match self.0.get_mut(key) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }
}

/// Every line of `statement` as the import would write it, with the choices applied. `existing`
/// is what the target account already holds over those days, as (day, signed effect).
pub fn plan_import(statement: &Statement, choices: &Choices, categories: &[CategoryView], learned: &HashMap<String, i64>, existing: &[(Date, i64)]) -> Plan {
    let rows = chronological(statement.rows.iter().filter(|r| choices.file_account.is_none() || r.account == choices.file_account).cloned().collect());
    let sign = if choices.flip { -1 } else { 1 };
    let by_id: HashMap<i64, &CategoryView> = categories.iter().map(|c| (c.id, c)).collect();
    let mut logged = Twins::new(existing.iter().copied());
    // Whether a line is written at all, before it is matched against the ledger. A zero-amount
    // line never is (the ledger takes none), nor a transfer left out.
    let wanted = |amount: i64, transfer: bool| {
        amount != 0
            && (!transfer
                || match choices.transfers {
                    TransferMode::Entries => true,
                    TransferMode::Transfers => choices.counterpart.is_some(),
                    TransferMode::Skip => false,
                })
    };
    let lines: Vec<PlannedLine> = rows
        .iter()
        .map(|r| {
            let amount = r.amount * sign;
            let cleaned = payees::clean(&r.description);
            let note = if cleaned.trim().is_empty() { "Bank entry".to_string() } else { cleaned };
            let transfer = payees::looks_like_transfer(&r.description, r.code.as_deref());
            // Only a line that would be written uses up a logged twin: a skipped transfer of the
            // same day and amount must not take the match a real purchase needs.
            let duplicate = wanted(amount, transfer) && logged.take(&(r.date, amount));
            let kind = if amount < 0 { CategoryKind::Expense } else { CategoryKind::Income };
            let from_ledger = learned.get(&format!("{}:{}", kind_name(kind), payees::key(&note))).copied().filter(|id| by_id.get(id).is_some_and(|c| c.kind == kind));
            let guessed = if from_ledger.is_none() {
                payees::guess_category(&r.description, amount > 0, r.code.as_deref())
                    .and_then(|name| categories.iter().find(|c| !c.archived && c.kind == kind && c.name.to_lowercase() == name.to_lowercase()).map(|c| c.id))
            } else {
                None
            };
            let category_id = if transfer && choices.transfers != TransferMode::Entries { None } else { from_ledger.or(guessed) };
            let c = category_id.and_then(|id| by_id.get(&id));
            PlannedLine {
                date: r.date,
                note,
                amount,
                category_id,
                category_name: c.map(|c| c.name.clone()),
                category_icon: c.map(|c| c.icon.clone()),
                category_color: c.map(|c| c.color),
                transfer,
                duplicate,
                learned: from_ledger.is_some() && category_id == from_ledger,
                balance: r.balance.map(|b| b * sign),
            }
        })
        .collect();
    let writes = |l: &PlannedLine| !l.duplicate && wanted(l.amount, l.transfer);
    let writing: Vec<&PlannedLine> = lines.iter().filter(|l| writes(l)).collect();
    let as_transfer = |l: &PlannedLine| l.transfer && choices.transfers == TransferMode::Transfers;
    let to_write = writing
        .iter()
        .map(|l| ImportLine { date: l.date, note: l.note.clone(), amount: l.amount, category_id: l.category_id, counterpart: if as_transfer(l) { choices.counterpart } else { None } })
        .collect();
    let entries: Vec<&&PlannedLine> = writing.iter().filter(|l| !as_transfer(l)).collect();
    // The balance before the file's first line: the first balance given, less every amount up to
    // and including its line (lines before it with no balance of their own count too).
    let opening = rows.iter().position(|r| r.balance.is_some()).and_then(|k| {
        let before: i64 = rows[..=k].iter().map(|r| r.amount).sum();
        rows[k].balance.map(|b| sign * (b - before))
    });
    let last_with_balance = rows.iter().rev().find(|r| r.balance.is_some());
    Plan {
        duplicates: lines.iter().filter(|l| l.duplicate).count(),
        transfer_like: lines.iter().filter(|l| l.transfer).count(),
        skipped: lines.iter().filter(|l| l.transfer && !l.duplicate).count() - writing.iter().filter(|l| l.transfer).count(),
        categorized: entries.iter().filter(|l| l.category_id.is_some()).count(),
        learned: entries.iter().filter(|l| l.learned).count(),
        uncategorized: entries.iter().filter(|l| l.category_id.is_none()).count(),
        out: entries.iter().filter(|l| l.amount < 0).map(|l| -l.amount).sum(),
        inn: entries.iter().filter(|l| l.amount > 0).map(|l| l.amount).sum(),
        first: rows.first().map(|r| r.date),
        last: rows.last().map(|r| r.date),
        opening_from_statement: opening,
        end_balance: last_with_balance.and_then(|l| l.balance).map(|b| sign * b),
        to_write,
        lines,
    }
}

/// Where a new account made by the import would read, and where the statement says it ends:
/// (reads, end). Only when the statement gives balances, and the two differ, as they do when
/// lines are left out (transfers skipped, or written as transfers with no other account picked).
pub fn new_account_gap(plan: &Plan) -> Option<(i64, i64)> {
    let (opening, end) = (plan.opening_from_statement?, plan.end_balance?);
    let reads = opening + plan.to_write.iter().map(|l| l.amount).sum::<i64>();
    (reads != end).then_some((reads, end))
}

/// The preview's word on [`new_account_gap`]: "The new account would read $1,240.00, but the
/// statement ends at $1,740.00. The 2 transfers left out make the difference: bring them in as
/// Transfers or Entries."
pub fn new_account_gap_line(plan: &Plan, money: impl Fn(i64) -> String) -> Option<String> {
    let (reads, end) = new_account_gap(plan)?;
    let head = format!("The new account would read {}, but the statement ends at {}.", money(reads), money(end));
    Some(match plan.skipped {
        0 => head,
        1 => format!("{head} The transfer left out makes the difference: bring it in as Transfers or Entries."),
        n => format!("{head} The {n} transfers left out make the difference: bring them in as Transfers or Entries."),
    })
}

/// An existing account as an older statement's lines meet it: its opening balance, the day of
/// its first entry (none when it has none), and whether a recorded value sets its balance instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub opening: i64,
    pub first_entry: Option<Date>,
    pub valued: bool,
}

/// What the statement says the account held at the end of the last line dated before `day` (with
/// no day, after its last line): the last balance it gives up to there, plus the lines after it.
/// None when the statement gives no balances.
pub fn statement_balance_before(plan: &Plan, day: Option<Date>) -> Option<i64> {
    let before: Vec<&PlannedLine> = plan.lines.iter().filter(|l| day.is_none_or(|d| l.date < d)).collect();
    match before.iter().rposition(|l| l.balance.is_some()) {
        Some(k) => Some(before[k].balance? + before[k + 1..].iter().map(|l| l.amount).sum::<i64>()),
        None => Some(plan.opening_from_statement? + before.iter().map(|l| l.amount).sum::<i64>()),
    }
}

/// Lines an import writes into an existing account before its first entry, and what that does
/// to its balance. Its opening balance stands before its first entry, so it may already hold them
/// (it was made from a later statement, or typed in): then every one would count twice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Earlier {
    /// The statement stood at the opening balance on the eve of the first entry, so the opening
    /// holds these lines: it moves back to before `back_to`, from `from` to `to`.
    Move { from: i64, to: i64, back_to: Date },
    /// The statement says otherwise: the account would read `reads` on `day`, the statement `says`.
    Off { day: Date, reads: i64, says: i64 },
    /// The statement gives no balances to tell by, and the opening is not zero.
    Unsure { lines: usize, first_entry: Option<Date>, opening: i64 },
}

/// What [`Earlier`] applies to `plan` going into the account `at`, if any. None when no line goes
/// before the first entry (unless an import that stopped half way left its move undone, see
/// [`logged_but_held`]), or when a recorded value sets the account's balance (lines before it do
/// not move it, so its opening holds nothing to take back).
pub fn earlier_lines(plan: &Plan, at: &Opening) -> Option<Earlier> {
    if at.valued {
        return None;
    }
    let earlier: Vec<&ImportLine> = plan.to_write.iter().filter(|l| at.first_entry.is_none_or(|d| l.date < d)).collect();
    let Some(back_to) = earlier.iter().map(|l| l.date).min() else { return logged_but_held(plan, at) };
    let day = earlier.iter().map(|l| l.date).max()?;
    let sum: i64 = earlier.iter().map(|l| l.amount).sum();
    let reads = at.opening + sum;
    match statement_balance_before(plan, at.first_entry) {
        Some(says) if says == at.opening => (sum != 0).then_some(Earlier::Move { from: at.opening, to: at.opening - sum, back_to }),
        Some(says) => (reads != says).then_some(Earlier::Off { day, reads, says }),
        None => (at.opening != 0).then_some(Earlier::Unsure { lines: earlier.len(), first_entry: at.first_entry, opening: at.opening }),
    }
}

/// An older statement's import that stopped between its lines going in and its opening moving
/// (the app killed between the two, or the move refused): the lines are logged now, so the
/// account's first entry is the statement's, and its opening still holds them. Told by the
/// statement as [`earlier_lines`] tells it: on the eve of the first entry it stood below (or
/// above) the opening, and at the end of a later day it stood at the opening, every line written
/// in between already logged. The opening then moves back by those logged lines. A day with a
/// line still to write ends the search: the opening never held that one.
fn logged_but_held(plan: &Plan, at: &Opening) -> Option<Earlier> {
    let first = at.first_entry?;
    // The statement must reach back to the eve of the first entry to say what stood there.
    if plan.first? > first {
        return None;
    }
    let says = statement_balance_before(plan, Some(first))?;
    if says == at.opening {
        return None;
    }
    let stop = plan.to_write.iter().map(|l| l.date).min();
    let from_first: Vec<&PlannedLine> = plan.lines.iter().filter(|l| l.date >= first && stop.is_none_or(|s| l.date < s)).collect();
    let (mut balance, mut logged) = (says, 0);
    for (i, l) in from_first.iter().enumerate() {
        balance = l.balance.unwrap_or(balance + l.amount);
        if l.duplicate {
            logged += l.amount;
        }
        let day_ends = from_first.get(i + 1).is_none_or(|n| n.date != l.date);
        if day_ends && logged != 0 && balance == at.opening {
            return Some(Earlier::Move { from: at.opening, to: at.opening - logged, back_to: first });
        }
    }
    None
}

/// The preview's word on [`Earlier`], for the account `name`.
pub fn earlier_line(e: &Earlier, name: &str, skipped: usize, short: impl Fn(Date) -> String, money: impl Fn(i64) -> String) -> String {
    match *e {
        Earlier::Move { to, back_to, .. } => format!(
            "{name}'s starting balance already holds these older lines, so it moves back to {}, at {}, and nothing is counted twice.",
            short(back_to),
            money(to)
        ),
        Earlier::Off { day, reads, says } => {
            let head = format!(
                "{name} would read {} on {}, but the statement says {}. Its starting balance is left as it is: change it in Accounts if it is off.",
                money(reads),
                short(day),
                money(says)
            );
            match skipped {
                0 => head,
                1 => format!("{head} The transfer left out counts toward it: bring it in as Transfers or Entries."),
                n => format!("{head} The {n} transfers left out count toward it: bring them in as Transfers or Entries."),
            }
        }
        Earlier::Unsure { lines, first_entry, opening } => {
            let (what, comes, they) = if lines == 1 { ("this line".to_string(), "comes", "it") } else { (format!("these {lines} lines"), "come", "they") };
            let when = match first_entry {
                Some(d) => format!("{} {comes} before {name}'s first entry, on {}", capital(&what), short(d)),
                None => format!("{name} has no entries yet"),
            };
            format!("{when}. If its starting balance of {} already holds {what}, {they} would count twice: change it in Accounts after.", money(opening))
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The done line when the opening moved too: "… · its starting balance moved back to 1 Oct".
pub fn opening_moved_line(done: &str, back_to: &str) -> String {
    format!("{done} · its starting balance moved back to {back_to}")
}

/// The done line when only the opening moved, its lines logged by an import that stopped before
/// moving it: "Desjardins chequing's starting balance moved back to 1 Oct · 12 already there".
pub fn only_opening_moved_line(into: &str, back_to: &str, duplicates: usize) -> String {
    let dup = if duplicates > 0 { format!(" · {duplicates} already there") } else { String::new() };
    format!("{into}'s starting balance moved back to {back_to}{dup}")
}

/// The done line when the lines went in but the opening could not move: read again, the file
/// offers the move once more.
pub fn opening_not_moved_line(done: &str) -> String {
    format!("{done} · its starting balance could not be moved: open the file again to move it")
}

/// Whether a numeric date like 03/04 reads day first: French and most of the world do, English
/// Canada and the US do not.
pub fn day_first_for(language: &str, country: &str) -> bool {
    !(country == "US" || (language == "en" && country == "CA"))
}

/// The words over the preview: "1 Sep to 30 Sep · $2,140 out · $3,200 in".
pub fn plan_span(plan: &Plan, short: impl Fn(Date) -> String, money: impl Fn(i64) -> String) -> String {
    let mut parts = Vec::new();
    if let (Some(a), Some(b)) = (plan.first, plan.last) {
        parts.push(if a == b { short(a) } else { format!("{} to {}", short(a), short(b)) });
    }
    parts.push(format!("{} out", money(plan.out)));
    parts.push(format!("{} in", money(plan.inn)));
    parts.join(" · ")
}

/// The import's outcome: "84 entries imported into Desjardins chequing · 12 already there".
pub fn imported_line(count: usize, into: &str, duplicates: usize) -> String {
    let n = if count == 1 { "1 entry".to_string() } else { format!("{count} entries") };
    let dup = if duplicates > 0 { format!(" · {duplicates} already there") } else { String::new() };
    format!("{n} imported into {into}{dup}")
}

/// Tally's own CSV read back: "Imported 40 entries, skipped 2 · 12 already there".
pub fn csv_imported_line(added: usize, skipped: usize, already: usize) -> String {
    let mut s = if added == 0 { "Nothing new to add".to_string() } else { format!("Imported {}", copy::plural_as(added as i64, "entry", "entries")) };
    if skipped > 0 {
        s.push_str(&format!(", skipped {skipped}"));
    }
    if already > 0 {
        s.push_str(&format!(" · {already} already there"));
    }
    s
}

/// True when `name` reads like an account of `source`'s ("Desjardins chequing" for Desjardins).
pub fn belongs_to(name: &str, source: Option<Source>) -> bool {
    matches!(source, Some(s) if s != Source::Other && normalize(name).contains(s.key()))
}

/// What the account a statement goes into most likely is: the one open, non-investment account
/// named for the bank of the kind the statement reads as. A statement that writes most lines
/// positive (`positive_share`) reads like a card's and only goes to a credit account; guessing
/// chequing for it would import every purchase as money in. Others go to a chequing-like account,
/// else the one card. With no account named for the bank (another bank's statement, or none
/// picked), the one an earlier import of the same kind made, by the name it gave it ("Imported
/// account"), so a second statement is checked for lines already in rather than opening a second
/// account that counts the money twice. None when no account fits, or when more than one does.
pub fn guess_target(accounts: &[AccountView], source: Option<Source>, positive_share: f32) -> Option<i64> {
    let open: Vec<&AccountView> = accounts.iter().filter(|a| !a.archived && a.r#type != AccountType::Investment).collect();
    let named: Vec<&AccountView> = open.iter().copied().filter(|a| belongs_to(&a.name, source)).collect();
    let one = |v: &[&AccountView]| if v.len() == 1 { Some(v[0].id) } else { None };
    let card = positive_share > CARD_POSITIVE_SHARE;
    if named.is_empty() {
        let name = default_name(source, card).to_lowercase();
        // A card's statement still only goes to a card.
        let made: Vec<&AccountView> = open.into_iter().filter(|a| a.name.trim().to_lowercase() == name && (!card || a.r#type == AccountType::Credit)).collect();
        return one(&made);
    }
    let (cards, others): (Vec<&AccountView>, Vec<&AccountView>) = named.into_iter().partition(|a| a.r#type == AccountType::Credit);
    if card || others.is_empty() { one(&cards) } else { one(&others) }
}

/// The preference holding the uid of the account the last statement from `source` went into,
/// one for a file that reads as a card's (`card`: purchases positive) and one for any other (none
/// picked reads as another bank's). Kept and looked up by how the file reads, never by the kind
/// of account it went into: the next file can only be told apart by how it reads.
pub fn last_target_pref(source: Option<Source>, card: bool) -> String {
    format!("import_last_target_{}_{}", source.unwrap_or(Source::Other).key(), if card { "card" } else { "account" })
}

/// The preference holding the uid of the account the last statement from `source` went into when
/// it named `file_account` (the account's number or name in the file, as Desjardins writes it).
/// A bank's chequing and its card may read alike (a card that writes purchases below zero), but
/// the file still names which of its accounts each line is from.
pub fn last_target_pref_for(source: Option<Source>, file_account: &str) -> String {
    format!("import_last_target_{}_file_{}", source.unwrap_or(Source::Other).key(), file_account.trim().to_lowercase())
}

/// The account a statement goes into first, before the owner picks: where the last file naming
/// the same account of the file went (`by_file`); else where the last one from the bank that read
/// the same way went (`by_source`); else the bank-name `guess`. The bank's one memory is shared by
/// every account of it that reads alike (chequing and savings, or chequing and a card writing
/// purchases below zero), so it gives way to a guess of another kind of account: the Visa a card's
/// file went into last must not take the next chequing file, which would land pay and spending in
/// the card.
pub fn pick_target(accounts: &[AccountView], by_file: Option<i64>, by_source: Option<i64>, card: bool, guess: Option<i64>) -> Option<i64> {
    if let Some(id) = remembered_target(accounts, by_file, card) {
        return Some(id);
    }
    let kind = |id: i64| accounts.iter().find(|a| a.id == id).map(|a| a.r#type);
    match (remembered_target(accounts, by_source, card), guess) {
        (Some(r), Some(g)) if kind(r) != kind(g) => Some(g),
        (Some(r), _) => Some(r),
        (None, g) => g,
    }
}

/// The account the last statement from the same bank went into, when it is still open and of a
/// kind the statement fits: a file that reads as a card's only into a card. Any other may go
/// into a card too: a card that writes purchases below zero reads like chequing, and where the
/// person put the last such file is the best guess for the next.
pub fn remembered_target(accounts: &[AccountView], last: Option<i64>, card: bool) -> Option<i64> {
    let id = last?;
    accounts
        .iter()
        .find(|a| a.id == id && !a.archived && a.r#type != AccountType::Investment && (!card || a.r#type == AccountType::Credit))
        .map(|a| a.id)
}

// The investment import

/// What the investment import adds: a report's holdings, or the new lines of the others.
pub fn invest_count(p: &ImportPreview) -> usize {
    if p.kind == WsKind::Holdings { p.holdings } else { p.new }
}

fn registration_words(r: Registration) -> &'static [&'static str] {
    match r {
        Registration::Tfsa => &["tfsa", "celi"],
        Registration::Rrsp => &["rrsp", "reer"],
        Registration::Fhsa => &["fhsa", "celiapp"],
        Registration::Resp => &["resp", "reee"],
        Registration::Lira => &["lira", "cri"],
        Registration::Rrif => &["rrif", "ferr"],
        _ => &[],
    }
}

/// Where each account in the file goes, before the owner says: the account already holding its
/// number; else the one open investment account whose name says its kind that nothing else took;
/// else a new one (none).
pub fn guess_mapping(file: &[ImportPreviewAccount], held: &[AccountView]) -> Vec<(String, Option<i64>)> {
    let open: Vec<&AccountView> = held.iter().filter(|a| a.r#type == AccountType::Investment && !a.archived).collect();
    let mut taken: Vec<i64> = file.iter().filter_map(|f| f.account_id).collect();
    file.iter()
        .map(|f| {
            let id = f.account_id.or_else(|| {
                let words = registration_words(f.registration);
                if words.is_empty() {
                    return None;
                }
                let named: Vec<i64> = open.iter().filter(|a| !taken.contains(&a.id) && normalize(&a.name).split(' ').any(|w| words.contains(&w))).map(|a| a.id).collect();
                let one = (named.len() == 1).then(|| named[0]);
                if let Some(id) = one {
                    taken.push(id);
                }
                one
            });
            (f.number.clone(), id)
        })
        .collect()
}

/// What an account the import creates is called: the file's name for it, or "Wealthsimple TFSA".
pub fn new_account_name(f: &ImportPreviewAccount) -> String {
    let n = f.name.trim();
    if n.is_empty() { format!("Wealthsimple {}", f.registration.label()) } else { n.to_string() }
}

/// The preview's head: "Wealthsimple holdings report · as of 8 May".
pub fn invest_file_line(p: &ImportPreview, short: impl Fn(Date) -> String) -> String {
    match p.kind {
        WsKind::Holdings => {
            let as_of = p.as_of.as_deref().and_then(|d| d.parse::<Date>().ok()).map(|d| format!(" · as of {}", short(d))).unwrap_or_default();
            format!("Wealthsimple holdings report{as_of}")
        }
        WsKind::Activities => format!("Wealthsimple activities export · {}", copy::plural(p.activities as i64, "line")),
        WsKind::Statement => format!("Wealthsimple monthly statement · {}", copy::plural(p.activities as i64, "line")),
    }
}

/// The line under the preview's figure: "holdings in 3 accounts".
pub fn invest_plan_line(p: &ImportPreview, into: Option<&str>) -> String {
    match p.kind {
        WsKind::Holdings => format!("{} in {}", if p.holdings == 1 { "holding" } else { "holdings" }, copy::plural(p.accounts.len() as i64, "account")),
        WsKind::Activities => format!("{} across {}", if p.new == 1 { "new activity" } else { "new activities" }, copy::plural(p.accounts.len() as i64, "account")),
        WsKind::Statement => format!("{} into {}", if p.new == 1 { "new line" } else { "new lines" }, into.unwrap_or("the account you pick")),
    }
}

/// The import's act: "Import 12 holdings", or that nothing is new.
pub fn invest_action_label(p: &ImportPreview) -> String {
    let n = invest_count(p) as i64;
    match p.kind {
        _ if n == 0 => "Nothing new to import".into(),
        WsKind::Holdings => format!("Import {}", copy::plural(n, "holding")),
        WsKind::Activities => format!("Import {}", copy::plural_as(n, "activity", "activities")),
        WsKind::Statement => format!("Import {}", copy::plural(n, "line")),
    }
}

/// What the investment import did: "Imported 12 holdings · 2 accounts added".
pub fn invest_imported_line(holdings: usize, activities: usize, accounts_created: usize, duplicates: usize) -> String {
    let mut added = Vec::new();
    if holdings > 0 {
        added.push(copy::plural(holdings as i64, "holding"));
    }
    if activities > 0 {
        added.push(copy::plural_as(activities as i64, "activity", "activities"));
    }
    let mut parts = vec![if added.is_empty() { "Nothing new to add".to_string() } else { format!("Imported {}", added.join(" and ")) }];
    if accounts_created > 0 {
        parts.push(format!("{} added", copy::plural(accounts_created as i64, "account")));
    }
    if duplicates > 0 {
        parts.push(format!("{duplicates} already in Tally"));
    }
    parts.join(" · ")
}
