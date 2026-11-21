//! The entry editor's pure half, as the Kotlin app's EntryLogic.kt and CategoryGrid.kt: what stops
//! a save, the readings beside the amount, the grid's cells, and the words a save or a delete
//! leaves. No drawing here, so tests hold all of it.

use relay_money::Date;
use relay_money::copy;
use relay_money::model::TxType;
use relay_money::money::MoneyFormatter;
use relay_money::period::BudgetPeriod;
use relay_money::views::Tx;

use crate::model::LineTone;

/// Four to a row, two rows when folded.
pub const GRID_COLUMNS: usize = 4;
pub const GRID_SLOTS: usize = GRID_COLUMNS * 2;

/// The draft as the rules read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub kind: TxType,
    pub amount: i64,
    pub date: Date,
    pub account: Option<i64>,
    pub to_account: Option<i64>,
    pub category: Option<i64>,
}

/// What stops the save, in the words the screen shows; none when the draft can be written.
pub fn validate(d: &Draft) -> Option<&'static str> {
    if d.account.is_none() {
        Some("Pick an account")
    } else if d.kind == TxType::Transfer && d.to_account.is_some() && d.to_account == d.account {
        Some("Pick two different accounts")
    } else if d.amount <= 0 {
        Some("Type an amount")
    } else if d.kind == TxType::Transfer && d.to_account.is_none() {
        Some("Pick the account it goes to")
    } else if d.kind != TxType::Transfer && d.category.is_none() {
        Some("Pick a category")
    } else {
        None
    }
}

/// The folded grid: the first `slots` less one (that cell is More), the picked category always
/// among them, swapped in for the last one when it sits further down. Returns the indices shown
/// and how many wait behind More.
pub fn grid_cells(ids: &[i64], selected: Option<i64>, slots: usize) -> (Vec<usize>, usize) {
    if ids.len() <= slots {
        return ((0..ids.len()).collect(), 0);
    }
    let mut shown: Vec<usize> = (0..slots - 1).collect();
    if let Some(at) = selected.and_then(|s| ids.iter().position(|id| *id == s))
        && !shown.contains(&at)
    {
        shown.pop();
        shown.push(at);
    }
    let hidden = ids.len() - shown.len();
    (shown, hidden)
}

/// The grid's order: the categories used most first, the rest in their own order after them.
pub fn by_use(ids: &[i64], uses: &[(i64, usize)]) -> Vec<usize> {
    let count = |id: i64| uses.iter().find(|(c, _)| *c == id).map_or(0, |(_, n)| *n);
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by(|a, b| count(ids[*b]).cmp(&count(ids[*a])).then(a.cmp(b)));
    order
}

/// The period's spending once this draft is saved. `spent` is what is stored, which for an edit
/// already holds the stored entry, so that comes out first.
pub fn spent_after(spent: i64, original: Option<&Tx>, d: &Draft, period: &BudgetPeriod) -> i64 {
    let mut total = spent;
    if let Some(o) = original
        && o.r#type == TxType::Expense
        && o.date.parse::<Date>().is_ok_and(|day| period.contains(day))
    {
        total -= o.amount;
    }
    if d.kind == TxType::Expense && period.contains(d.date) {
        total += d.amount;
    }
    total.max(0)
}

/// A category's period with this draft in it: (total, count).
pub fn category_month(total: i64, count: usize, original: Option<&Tx>, d: &Draft, period: &BudgetPeriod) -> (i64, usize) {
    let (mut sum, mut n) = (total, count as i64);
    if let Some(o) = original
        && o.r#type == d.kind
        && o.category_id == d.category
        && o.date.parse::<Date>().is_ok_and(|day| period.contains(day))
    {
        sum -= o.amount;
        n -= 1;
    }
    if d.amount > 0 && period.contains(d.date) {
        sum += d.amount;
        n += 1;
    }
    (sum.max(0), n.max(0) as usize)
}

/// The line under the amount: where this month's spending stands once this is in. Never a
/// negative figure: past the limit it says by how much.
pub fn after_line(kind: TxType, left_after: Option<i64>, spent_after: Option<i64>, fmt: &MoneyFormatter) -> Option<(String, LineTone)> {
    match kind {
        TxType::Transfer => Some(("A transfer moves money between your accounts. Budgets do not count it.".into(), LineTone::Quiet)),
        TxType::Income => None,
        TxType::Expense => match left_after {
            None => spent_after.map(|s| (format!("{} spent this month with this", fmt.format_whole(s)), LineTone::Quiet)),
            Some(l) if l > 0 => Some((format!("{} left this month after this", fmt.format_whole(l)), LineTone::Quiet)),
            Some(0) => Some(("This reaches this month's limit".into(), LineTone::Warn)),
            Some(l) => Some((format!("Over this month's limit by {}", fmt.format_whole(-l)), LineTone::Held)),
        },
    }
}

/// The picked category's month with this entry in it: against its own limit when it has one,
/// else what it holds and how many entries.
pub fn category_line(kind: TxType, name: &str, (total, count): (i64, usize), budget: Option<i64>, fmt: &MoneyFormatter) -> (String, LineTone) {
    if kind == TxType::Expense
        && let Some(b) = budget.filter(|b| *b > 0)
    {
        let head = format!("{} of {} in {name} this month", fmt.format_whole(total), fmt.format_whole(b));
        return if total > b {
            (format!("{head}, {} over its limit", fmt.format_whole(total - b)), LineTone::Held)
        } else if total == b {
            (format!("{head}, its limit reached"), LineTone::Warn)
        } else {
            (format!("{head}, {} left", fmt.format_whole(b - total)), LineTone::Quiet)
        };
    }
    if count == 0 {
        return (format!("Nothing in {name} yet this month"), LineTone::Quiet);
    }
    (format!("{} in {name} this month · {}", fmt.format_whole(total), copy::plural_as(count as i64, "entry", "entries")), LineTone::Quiet)
}

pub fn type_label(kind: TxType) -> &'static str {
    match kind {
        TxType::Expense => "Expense",
        TxType::Income => "Income",
        TxType::Transfer => "Transfer",
    }
}

/// "New expense", "Edit transfer".
pub fn title(kind: TxType, editing: bool) -> String {
    format!("{} {}", if editing { "Edit" } else { "New" }, type_label(kind).to_lowercase())
}

/// The notice after a save: what was saved, then where the month stands.
pub fn saved_notice(kind: TxType, category: Option<&str>, left_after: Option<i64>, fmt: &MoneyFormatter) -> String {
    let head = match kind {
        TxType::Transfer => "Transfer saved".to_string(),
        _ => {
            let words = match category {
                Some(c) => format!("{c} {} saved", type_label(kind).to_lowercase()),
                None => format!("{} saved", type_label(kind)),
            };
            let mut ch = words.chars();
            ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
        }
    };
    match (kind, left_after) {
        (TxType::Expense, Some(l)) if l > 0 => format!("{head} · {} left this month", fmt.format_whole(l)),
        (TxType::Expense, Some(0)) => format!("{head} · this month's limit reached"),
        (TxType::Expense, Some(l)) => format!("{head} · {} over this month's limit", fmt.format_whole(-l)),
        _ => head,
    }
}

/// The notice after a delete, which Undo takes back.
pub fn deleted_notice(kind: TxType, category: Option<&str>) -> String {
    match (kind, category) {
        (TxType::Transfer, _) => "Transfer deleted".into(),
        (_, Some(c)) => format!("{c} entry deleted"),
        _ => "Entry deleted".into(),
    }
}

/// How often a bill repeats: "Every month", "Every 2 weeks".
pub fn every_label(frequency: &str, interval: i64) -> String {
    let unit = match frequency.to_ascii_uppercase().as_str() {
        "WEEKLY" => "week",
        "YEARLY" => "year",
        _ => "month",
    };
    if interval <= 1 { format!("Every {unit}") } else { format!("Every {interval} {unit}s") }
}

/// The bill an entry already belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkedBill {
    pub id: i64,
    pub frequency: String,
    pub interval: i64,
    pub next: Date,
    pub active: bool,
    pub auto_post: bool,
}

/// The line under "Repeats": when it posts next and how, or that it is paused.
pub fn linked_bill_line(b: &LinkedBill, today: Date) -> String {
    let day = crate::model::day_word(&b.next.to_string(), today);
    if !b.active {
        "Paused: nothing posts until you resume it".into()
    } else if b.auto_post {
        format!("Posts itself next on {day}")
    } else {
        format!("Reminds you next on {day}")
    }
}

/// How an entry repeats, for its overview: "Repeats monthly · next on December 17", "Repeats
/// every 2 weeks · paused", or "Does not repeat" when no bill posted it. The date carries its year
/// when it is not this one.
pub fn repeats_line(b: Option<&LinkedBill>, today: Date) -> String {
    let Some(b) = b else { return "Does not repeat".into() };
    let unit = match b.frequency.to_ascii_uppercase().as_str() {
        "WEEKLY" => ("weekly", "week"),
        "YEARLY" => ("yearly", "year"),
        _ => ("monthly", "month"),
    };
    let every = if b.interval <= 1 { unit.0.to_string() } else { format!("every {} {}s", b.interval, unit.1) };
    if !b.active {
        return format!("Repeats {every} · paused");
    }
    let next = if b.next.year() == today.year() { b.next.strftime("%B %-d").to_string() } else { b.next.strftime("%B %-d, %Y").to_string() };
    format!("Repeats {every} · next on {next}")
}

/// One of the usual entries: a note logged at least twice with the same category, and its last
/// amount, filled in one tap.
#[derive(Clone, Debug, PartialEq)]
pub struct Usual {
    pub kind: TxType,
    pub note: String,
    pub category: i64,
    pub last_amount: i64,
    pub uses: usize,
}

/// The usual entries of each type, most used first (then most recent), from rows newest first.
pub fn usual(rows: &[Tx], limit: usize) -> Vec<Usual> {
    let mut out: Vec<Usual> = Vec::new();
    for t in rows.iter().filter(|t| t.r#type != TxType::Transfer && !t.note.trim().is_empty()) {
        let Some(c) = t.category_id else { continue };
        let note = t.note.trim();
        match out.iter_mut().find(|u| u.kind == t.r#type && u.category == c && u.note == note) {
            Some(u) => u.uses += 1,
            None => out.push(Usual { kind: t.r#type, note: note.to_string(), category: c, last_amount: t.amount, uses: 1 }),
        }
    }
    out.retain(|u| u.uses >= 2);
    // A stable sort keeps the most recent first among equals.
    out.sort_by_key(|u| std::cmp::Reverse(u.uses));
    let mut per_kind = [0usize; 2];
    out.retain(|u| {
        let k = usize::from(u.kind == TxType::Income);
        per_kind[k] += 1;
        per_kind[k] <= limit
    });
    out
}

/// Notes used before that start with what is typed (any case), the most recent first; never the
/// note exactly as typed.
pub fn suggestions(notes: &[(String, Option<i64>)], typed: &str, limit: usize) -> Vec<String> {
    let prefix = typed.trim().to_lowercase();
    if prefix.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    for (n, _) in notes {
        let low = n.to_lowercase();
        if low.starts_with(&prefix) && low != prefix && !out.iter().any(|o| o.to_lowercase() == low) {
            out.push(n.clone());
            if out.len() == limit {
                break;
            }
        }
    }
    out
}
