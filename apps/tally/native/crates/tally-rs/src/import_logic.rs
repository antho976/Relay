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
    let newest_first = rows[0].date > rows[rows.len() - 1].date;
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

/// Every line of `statement` as the import would write it, with the choices applied. `existing`
/// is what the target account already holds over those days, as (day, signed effect).
pub fn plan_import(statement: &Statement, choices: &Choices, categories: &[CategoryView], learned: &HashMap<String, i64>, existing: &[(Date, i64)]) -> Plan {
    let rows = chronological(statement.rows.iter().filter(|r| choices.file_account.is_none() || r.account == choices.file_account).cloned().collect());
    let sign = if choices.flip { -1 } else { 1 };
    let by_id: HashMap<i64, &CategoryView> = categories.iter().map(|c| (c.id, c)).collect();
    // Each logged entry can stand twin to one statement line only.
    let mut logged: HashMap<(Date, i64), usize> = HashMap::new();
    for e in existing {
        *logged.entry(*e).or_default() += 1;
    }
    let lines: Vec<PlannedLine> = rows
        .iter()
        .map(|r| {
            let amount = r.amount * sign;
            let cleaned = payees::clean(&r.description);
            let note = if cleaned.trim().is_empty() { "Bank entry".to_string() } else { cleaned };
            let transfer = payees::looks_like_transfer(&r.description, r.code.as_deref());
            let twin = (r.date, amount);
            let duplicate = logged.get(&twin).copied().unwrap_or(0) > 0;
            if duplicate && let Some(n) = logged.get_mut(&twin) {
                *n -= 1;
            }
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
            }
        })
        .collect();
    let writes = |l: &PlannedLine| {
        !l.duplicate
            && (!l.transfer
                || match choices.transfers {
                    TransferMode::Entries => true,
                    TransferMode::Transfers => choices.counterpart.is_some(),
                    TransferMode::Skip => false,
                })
    };
    let writing: Vec<&PlannedLine> = lines.iter().filter(|l| writes(l)).collect();
    let as_transfer = |l: &PlannedLine| l.transfer && choices.transfers == TransferMode::Transfers;
    let to_write = writing
        .iter()
        .map(|l| ImportLine { date: l.date, note: l.note.clone(), amount: l.amount, category_id: l.category_id, counterpart: if as_transfer(l) { choices.counterpart } else { None } })
        .collect();
    let entries: Vec<&&PlannedLine> = writing.iter().filter(|l| !as_transfer(l)).collect();
    let first_with_balance = rows.iter().find(|r| r.balance.is_some());
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
        opening_from_statement: first_with_balance.and_then(|f| f.balance.map(|b| sign * (b - f.amount))),
        end_balance: last_with_balance.and_then(|l| l.balance).map(|b| sign * b),
        to_write,
        lines,
    }
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

/// True when `name` reads like an account of `source`'s ("Desjardins chequing" for Desjardins).
pub fn belongs_to(name: &str, source: Option<Source>) -> bool {
    matches!(source, Some(s) if s != Source::Other && normalize(name).contains(s.key()))
}

/// What the account a statement goes into most likely is: one named for the bank, else none.
pub fn guess_target(accounts: &[AccountView], source: Option<Source>) -> Option<i64> {
    accounts.iter().find(|a| belongs_to(&a.name, source)).map(|a| a.id)
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
