//! The Plan pages' readings, as the Kotlin app's PlanLogic.kt folds them: budget envelopes and
//! what is spent without one, suggestions from the months before, the bills' monthly cost and what
//! falls due, every kind of goal's reading and pace, and the editors' words. Pure: no drawing, no
//! ledger; [`load`] reads what they need in one go.

use jiff::ToSpan;
use relay_money::Date;
use relay_money::copy;
use relay_money::model::{AccountType, GoalKind, TxType};
use relay_money::money::MoneyFormatter;
use relay_money::pace::{PaceReading, PaceStatus};
use relay_money::period::BudgetPeriod;
use relay_money::recurrence::{Frequency, Recurrence};
use relay_money::views::{AccountView, CategoryView};

use crate::model::Store;
use crate::plan_store::{self, BillRow, Goal};

pub type Result<T> = std::result::Result<T, String>;

/// The longest gap the bill editor's stepper offers between two dates.
pub const MAX_INTERVAL: i32 = 12;
/// The periods a budget suggestion averages: the three before this one.
pub const SUGGEST_PERIODS: i64 = 3;
/// A suggestion needs spending in at least this many of them, so a one-off buys nothing.
pub const SUGGEST_MIN_MONTHS: usize = 2;
pub const SUGGEST_MAX: usize = 6;
/// Days into the period before a projection means anything.
pub const PROJECT_AFTER_DAYS: i64 = 3;
/// A monthly goal's record looks back this many months before the current one.
pub const GOAL_MONTHS: i64 = 5;

// Dates, in the Kotlin app's words (ui/common/Dates.kt)

/// "14 Mar", with the year when it is not this one.
pub fn short(d: Date, today: Date) -> String {
    if d.year() == today.year() { d.strftime("%-d %b").to_string() } else { d.strftime("%-d %b %Y").to_string() }
}

/// "Today", "Yesterday", "Tomorrow", else "Mon 17 Nov".
pub fn day(d: Date, today: Date) -> String {
    match (d - today).get_days() {
        0 => "Today".into(),
        -1 => "Yesterday".into(),
        1 => "Tomorrow".into(),
        _ if d.year() == today.year() => d.strftime("%a %-d %b").to_string(),
        _ => d.strftime("%-d %b %Y").to_string(),
    }
}

/// "March 14, 2027".
pub fn long(d: Date) -> String {
    d.strftime("%B %-d, %Y").to_string()
}

/// "October", or "15 Sep to 14 Oct" for a period that starts on another day.
pub fn period_name(p: &BudgetPeriod, today: Date) -> String {
    if p.start.day() == 1 {
        let m = p.start.strftime("%B").to_string();
        if p.start.year() == today.year() { m } else { format!("{m} {}", p.start.year()) }
    } else {
        format!("{} to {}", p.start.strftime("%-d %b"), p.last_day().strftime("%-d %b"))
    }
}

/// Whole months from `today` to `date`, rounded up: at least 1 until the date passes, then 0.
pub fn months_until(today: Date, date: Date) -> i64 {
    if date < today {
        return 0;
    }
    let packed = |d: Date| (i64::from(d.year()) * 12 + i64::from(d.month()) - 1) * 32 + i64::from(d.day());
    let mut months = (packed(date) - packed(today)) / 32;
    if today.checked_add(months.months()).is_ok_and(|d| d < date) {
        months += 1;
    }
    months.max(1)
}

fn days(a: Date, b: Date) -> i64 {
    (b - a).get_days() as i64
}

fn round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

// Budgets

/// The limit is reached once nothing is left of it.
pub fn limit_reached(r: &PaceReading) -> bool {
    r.budget > 0 && r.remaining <= 0
}

/// The pace in days, or that the limit is reached.
pub fn pace_days(r: &PaceReading) -> String {
    if limit_reached(r) { "Limit reached".into() } else { copy::days_line(r) }
}

/// A budget's verdict: what is left and the pace, or that its limit is reached and by how much.
pub fn budget_words(r: &PaceReading, fmt: &MoneyFormatter) -> String {
    if !limit_reached(r) {
        format!("{} left · {}", fmt.format_whole(r.remaining), copy::pace_line(r, fmt).to_lowercase())
    } else if r.remaining == 0 {
        "Limit reached".into()
    } else {
        format!("Limit reached · {} over", fmt.format_whole(-r.remaining))
    }
}

/// How a reading's words are coloured.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Quiet,
    Warn,
    Held,
    Good,
}

pub fn pace_tone(r: &PaceReading) -> Tone {
    if limit_reached(r) {
        Tone::Held
    } else if r.status == PaceStatus::OverPace {
        Tone::Warn
    } else {
        Tone::Quiet
    }
}

/// What is left of a limit, or how far over it went: ("$188 left", false), ("$20 over", true).
pub fn left_words(r: &PaceReading, fmt: &MoneyFormatter) -> (String, bool) {
    if r.remaining >= 0 { (format!("{} left", fmt.format_whole(r.remaining)), false) } else { (format!("{} over", fmt.format_whole(-r.remaining)), true) }
}

/// The monthly budget's line on its card: "$1,915 of $3,200 spent. On pace with 11 days to go."
pub fn budgets_hero_line(r: &PaceReading, fmt: &MoneyFormatter, days_left: i64) -> String {
    let spent = format!("{} of {} spent", fmt.format_whole(r.spent), fmt.format_whole(r.budget));
    let days = copy::plural(days_left, "day");
    if limit_reached(r) {
        return format!("{spent}, with {days} to go.");
    }
    match r.status {
        PaceStatus::OnPace => format!("{spent}. On pace with {days} to go."),
        _ => format!("{spent}. {}, with {days} to go.", copy::days_line(r)),
    }
}

#[derive(Clone, Debug)]
pub struct BudgetLine {
    pub category_id: i64,
    pub name: String,
    pub icon: String,
    pub color: i64,
    pub reading: PaceReading,
}

#[derive(Clone, Debug)]
pub struct Unbudgeted {
    pub category_id: i64,
    pub name: String,
    pub icon: String,
    pub color: i64,
    pub spent: i64,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub category_id: i64,
    pub name: String,
    pub icon: String,
    pub color: i64,
    pub amount: i64,
    pub months: usize,
}

#[derive(Clone, Debug, Default)]
pub struct BudgetsReading {
    pub spent: i64,
    pub last_spent: i64,
    pub overall: Option<PaceReading>,
    /// Furthest over pace first, against each budget's size.
    pub envelopes: Vec<BudgetLine>,
    pub budgeted: i64,
    pub unbudgeted: Vec<Unbudgeted>,
    pub unbudgeted_spent: i64,
    pub projected: Option<i64>,
    pub suggestions: Vec<Suggestion>,
}

impl BudgetsReading {
    pub fn suggested_total(&self) -> i64 {
        self.suggestions.iter().map(|s| s.amount).sum()
    }
}

/// Where `spent` lands by the period's end at the pace so far; none in the first days.
pub fn projected_spend(spent: i64, days: i64, elapsed: i64) -> Option<i64> {
    if spent <= 0 || elapsed < PROJECT_AFTER_DAYS || days <= 0 {
        return None;
    }
    if elapsed >= days {
        return Some(spent);
    }
    Some(round(spent as f64 * days as f64 / elapsed as f64))
}

/// The mean of the months that had any spending.
pub fn average_on_record(values: &[i64]) -> Option<i64> {
    let rec: Vec<i64> = values.iter().copied().filter(|v| *v > 0).collect();
    (!rec.is_empty()).then(|| rec.iter().sum::<i64>() / rec.len() as i64)
}

pub fn round_up_to(amount: i64, step: i64) -> i64 {
    if amount <= 0 || step <= 0 { amount.max(0) } else { (amount + step - 1) / step * step }
}

pub fn round_to(amount: i64, step: i64) -> i64 {
    if amount <= 0 || step <= 0 { amount.max(0) } else { (amount + step / 2) / step * step }
}

/// Budgets for the spending categories with none, each the average of the periods before this
/// one (`history`: each period's (category, total)), rounded up to `step`, biggest first.
pub fn suggestions(history: &[Vec<(i64, i64, i64)>], budgeted: &[i64], categories: &[CategoryView], step: i64) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = categories
        .iter()
        .filter(|c| c.kind == relay_money::model::CategoryKind::Expense && !c.archived && !budgeted.contains(&c.id))
        .filter_map(|c| {
            let values: Vec<i64> = history.iter().map(|p| p.iter().find(|t| t.0 == c.id).map_or(0, |t| t.1)).collect();
            let months = values.iter().filter(|v| **v > 0).count();
            if months < SUGGEST_MIN_MONTHS {
                return None;
            }
            let amount = round_up_to(average_on_record(&values)?, step);
            (amount > 0).then(|| Suggestion { category_id: c.id, name: c.name.clone(), icon: c.icon.clone(), color: c.color, amount, months })
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.amount));
    out.truncate(SUGGEST_MAX);
    out
}

/// Everything the Budgets page reads, folded.
#[allow(clippy::too_many_arguments)]
pub fn budgets_reading(
    period: &BudgetPeriod,
    today: Date,
    overall: Option<i64>,
    budgets: &[(i64, i64)],
    spending: &[(i64, i64, i64)],
    spent: i64,
    last_spent: i64,
    categories: &[CategoryView],
    history: &[Vec<(i64, i64, i64)>],
    step: i64,
) -> BudgetsReading {
    let (d, elapsed) = (period.days(), period.elapsed_days(today));
    let mut envelopes: Vec<BudgetLine> = budgets
        .iter()
        .filter_map(|(cid, amount)| {
            let c = categories.iter().find(|c| c.id == *cid)?;
            let used = spending.iter().find(|t| t.0 == *cid).map_or(0, |t| t.1);
            Some(BudgetLine { category_id: c.id, name: c.name.clone(), icon: c.icon.clone(), color: c.color, reading: PaceReading::new(*amount, used, d, elapsed) })
        })
        .collect();
    let key = |l: &BudgetLine| l.reading.pace_delta as f64 / l.reading.budget.max(1) as f64;
    envelopes.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal));
    let ids: Vec<i64> = envelopes.iter().map(|e| e.category_id).collect();
    let mut unbudgeted: Vec<Unbudgeted> = spending
        .iter()
        .filter(|t| t.1 > 0 && !ids.contains(&t.0))
        .filter_map(|t| {
            let c = categories.iter().find(|c| c.id == t.0 && c.kind == relay_money::model::CategoryKind::Expense)?;
            Some(Unbudgeted { category_id: c.id, name: c.name.clone(), icon: c.icon.clone(), color: c.color, spent: t.1, count: t.2 })
        })
        .collect();
    unbudgeted.sort_by_key(|u| std::cmp::Reverse(u.spent));
    BudgetsReading {
        spent,
        last_spent,
        overall: overall.filter(|a| *a > 0).map(|a| PaceReading::new(a, spent, d, elapsed)),
        budgeted: envelopes.iter().map(|e| e.reading.budget).sum(),
        unbudgeted_spent: unbudgeted.iter().map(|u| u.spent).sum(),
        unbudgeted,
        projected: projected_spend(spent, d, elapsed),
        suggestions: suggestions(history, &ids, categories, step),
        envelopes,
    }
}

// Bills

/// "Every month", "Every 3 weeks".
pub fn every_label(f: Frequency, interval: i32) -> String {
    let n = interval.max(1);
    let unit = match f {
        Frequency::Weekly => "week",
        Frequency::Monthly => "month",
        Frequency::Yearly => "year",
    };
    if n == 1 { format!("Every {unit}") } else { format!("Every {n} {unit}s") }
}

/// "Weekly", "Monthly", "Yearly": the frequency choice.
pub fn frequency_word(f: Frequency) -> &'static str {
    match f {
        Frequency::Weekly => "Weekly",
        Frequency::Monthly => "Monthly",
        Frequency::Yearly => "Yearly",
    }
}

/// A bill's amount in an average month.
pub fn per_month(amount: i64, f: Frequency, interval: i32) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let anchor = jiff::civil::date(2000, 1, 1);
    round(amount as f64 * Recurrence::new(anchor, f, interval.clamp(1, 52)).per_month_factor())
}

/// A bill's line: when it is due, and how often.
pub fn bill_line(next: Date, days_until: i64, every: &str, today: Date) -> String {
    let when = if (-30..=14).contains(&days_until) { copy::due_line(days_until) } else { short(next, today) };
    format!("{when} · {every}")
}

#[derive(Clone, Debug)]
pub struct BillLine {
    pub row: BillRow,
    pub days_until: i64,
    pub per_month: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DueSum {
    pub total: i64,
    pub payments: i64,
}

#[derive(Clone, Debug, Default)]
pub struct BillsReading {
    pub active: Vec<BillLine>,
    pub paused: Vec<BillLine>,
    pub monthly_cost: i64,
    pub monthly_income: i64,
    /// Active expense bills: (name, a month, colour), biggest first.
    pub cost_shares: Vec<(String, i64, Option<i64>)>,
    pub week: DueSum,
    pub month: DueSum,
    pub reminders: usize,
}

/// Every payment of `bills` from each one's next date through `through`.
pub fn due_through(bills: &[&BillLine], through: Date) -> DueSum {
    let mut s = DueSum::default();
    for b in bills {
        let bill = &b.row.bill;
        if bill.next > through {
            continue;
        }
        let n = bill.rule().between_limited(bill.next, through, 60).len().max(1) as i64;
        s.total += bill.amount * n;
        s.payments += n;
    }
    s
}

pub fn bills_reading(rows: Vec<BillRow>, today: Date) -> BillsReading {
    let lines: Vec<BillLine> = rows
        .into_iter()
        .map(|row| {
            let b = &row.bill;
            BillLine { days_until: days(today, b.next), per_month: per_month(b.amount, b.frequency, b.interval), row }
        })
        .collect();
    let (mut active, mut paused): (Vec<BillLine>, Vec<BillLine>) = lines.into_iter().partition(|l| l.row.bill.active);
    active.sort_by_key(|l| l.row.bill.next);
    paused.sort_by_key(|l| l.row.bill.name.to_lowercase());
    let expense: Vec<&BillLine> = active.iter().filter(|l| l.row.bill.kind == TxType::Expense).collect();
    let mut shares: Vec<(String, i64, Option<i64>)> = expense.iter().filter(|l| l.per_month > 0).map(|l| (l.row.bill.name.clone(), l.per_month, l.row.color)).collect();
    shares.sort_by_key(|s| std::cmp::Reverse(s.1));
    let week_end = today.checked_add(6.days()).unwrap_or(today);
    let month_end = today.checked_add(29.days()).unwrap_or(today);
    BillsReading {
        monthly_cost: expense.iter().map(|l| l.per_month).sum(),
        monthly_income: active.iter().filter(|l| l.row.bill.kind == TxType::Income).map(|l| l.per_month).sum(),
        cost_shares: shares,
        week: due_through(&expense, week_end),
        month: due_through(&expense, month_end),
        reminders: active.iter().filter(|l| !l.row.bill.auto_post).count(),
        active,
        paused,
    }
}

/// The last day of the 30 the Bills card counts.
pub fn bills_horizon(today: Date) -> Date {
    today.checked_add(29.days()).unwrap_or(today)
}

/// The Bills card: the money due in the next 30 days, and a line on how many and the big one.
pub fn bills_hero(r: &BillsReading, today: Date, fmt: &MoneyFormatter) -> (String, String) {
    let through = bills_horizon(today);
    let due: Vec<&BillLine> = r.active.iter().filter(|b| b.row.bill.kind == TxType::Expense && b.row.bill.next <= through).collect();
    let Some(big) = due.iter().max_by_key(|b| (b.row.bill.amount, std::cmp::Reverse(b.row.bill.next))) else {
        return ("Nothing due".into(), "No bills fall due in the next 30 days.".into());
    };
    let on = big.row.bill.next.strftime("%B %-d").to_string();
    let line = if due.len() == 1 {
        format!("1 bill in the next 30 days: {}, on {on}.", big.row.bill.name)
    } else {
        format!("{} in the next 30 days. {} is the big one, on {on}.", copy::plural(due.len() as i64, "bill"), big.row.bill.name)
    };
    (fmt.format(r.month.total), line)
}

/// The running bills in groups by when they fall due: this week (and anything overdue), the rest
/// of this month, next month, and later. Each group is (title, indexes into `active`).
pub fn bill_groups(active: &[BillLine], today: Date) -> Vec<(String, Vec<usize>)> {
    let month_of = |d: Date| i64::from(d.year()) * 12 + i64::from(d.month());
    let name = |d: Date| if d.year() == today.year() { d.strftime("%B").to_string() } else { d.strftime("%B %Y").to_string() };
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, b) in active.iter().enumerate() {
        let next = b.row.bill.next;
        let title = if b.days_until <= 6 {
            "This week".to_string()
        } else if month_of(next) == month_of(today) {
            format!("Later in {}", name(today))
        } else if month_of(next) == month_of(today) + 1 {
            name(next)
        } else {
            "Later".to_string()
        };
        match groups.iter_mut().find(|g| g.0 == title) {
            Some(g) => g.1.push(i),
            None => groups.push((title, vec![i])),
        }
    }
    groups
}

/// A bill's line on Bills: when it is close, what kind it is, how often when not monthly, and the
/// account. "In 3 days · Visa", "Autopay · Every 2 weeks · Chequing".
pub fn bill_note(b: &BillLine) -> String {
    let bill = &b.row.bill;
    let kind = match (bill.kind, &b.row.to_account) {
        (TxType::Transfer, Some(to)) => format!("Transfer to {to}"),
        (TxType::Transfer, None) => "Transfer".into(),
        (TxType::Income, _) => "Comes in".into(),
        (TxType::Expense, _) if bill.auto_post => "Autopay".into(),
        (TxType::Expense, _) => "Reminder".into(),
    };
    let mut parts: Vec<String> = Vec::new();
    match b.days_until {
        d if d < 0 => parts.push(copy::due_line(d)),
        0 => parts.push("Today".into()),
        1 => parts.push("Tomorrow".into()),
        d @ 2..=3 => parts.push(format!("In {d} days")),
        _ => {}
    }
    // Close, the date says enough for an autopay; anything else still says what it is.
    let transfer = bill.kind == TxType::Transfer;
    if parts.is_empty() || kind != "Autopay" {
        parts.push(kind);
    }
    if !(bill.frequency == Frequency::Monthly && bill.interval <= 1) {
        parts.push(every_label(bill.frequency, bill.interval));
    }
    // A transfer names where it goes; anything else the account it is paid from or into.
    if !transfer || b.row.to_account.is_none() {
        parts.push(b.row.account.clone());
    }
    parts.join(" · ")
}

/// Running bills that posted in the last week: (index into `active`, the day it posted), oldest
/// first. `posted` is each bill's latest posting.
pub fn paid_lately(active: &[BillLine], posted: &[(i64, Date)], today: Date) -> Vec<(usize, Date)> {
    let mut out: Vec<(usize, Date)> = active
        .iter()
        .enumerate()
        .filter_map(|(i, b)| posted.iter().find(|p| p.0 == b.row.bill.id).map(|p| (i, p.1)))
        .filter(|(_, d)| (0..=6).contains(&days(*d, today)))
        .collect();
    out.sort_by_key(|p| p.1);
    out
}

/// What stops a bill from saving, field by field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BillProblems {
    pub name: Option<&'static str>,
    pub amount: Option<&'static str>,
    pub account: Option<&'static str>,
}

impl BillProblems {
    pub fn any(&self) -> bool {
        self.name.is_some() || self.amount.is_some() || self.account.is_some()
    }
    /// The first problem, as the footer's line.
    pub fn line(&self) -> Option<&'static str> {
        self.name.or(self.amount).or(self.account)
    }
}

pub fn bill_problems(name: &str, amount: Option<i64>, kind: TxType, account: Option<i64>, to: Option<i64>) -> BillProblems {
    BillProblems {
        name: name.trim().is_empty().then_some("Name the bill so you can find it later"),
        amount: (amount.unwrap_or(0) <= 0).then_some("Enter an amount above zero"),
        account: match (account, kind) {
            (None, TxType::Income) => Some("Pick the account it is paid into"),
            (None, TxType::Transfer) => Some("Pick the account it leaves"),
            (None, TxType::Expense) => Some("Pick the account it comes out of"),
            (Some(_), TxType::Transfer) if to.is_none() => Some("Pick the account it goes to"),
            (Some(a), TxType::Transfer) if to == Some(a) => Some("From and to are the same account. A transfer needs two"),
            _ => None,
        },
    }
}

// Goals

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GoalStatus {
    Reached,
    OnTrack,
    Behind,
    Open,
}

/// One month of a monthly goal: what it asked, what happened, what came in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GoalMonth {
    pub start: Date,
    pub achieved: i64,
    pub asked: i64,
    pub income: i64,
}

impl GoalMonth {
    pub fn met(&self) -> bool {
        self.asked > 0 && self.achieved >= self.asked
    }
}

/// A goal read for the pages, whatever its kind (PlanLogic.kt `GoalLine`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoalLine {
    pub id: i64,
    pub name: String,
    pub color: i64,
    pub saved: i64,
    pub target: i64,
    pub target_date: Option<Date>,
    pub months_left: Option<i64>,
    pub pace_fraction: Option<f32>,
    pub kind: GoalKind,
    pub start: i64,
    pub percent: i64,
    pub income: i64,
    pub account_name: Option<String>,
    /// A monthly goal's earlier months, oldest first.
    pub history: Vec<GoalMonth>,
}

impl GoalLine {
    pub fn monthly(&self) -> bool {
        matches!(self.kind, GoalKind::Invest | GoalKind::Save)
    }
    fn falling(&self) -> bool {
        self.kind == GoalKind::Balance && self.target < self.start
    }
    pub fn fraction(&self) -> f32 {
        if self.kind == GoalKind::Balance {
            let span = self.target - self.start;
            if span == 0 {
                return if self.saved == self.target { 1.0 } else { 0.0 };
            }
            return ((self.saved - self.start) as f64 / span as f64).max(0.0) as f32;
        }
        if self.target <= 0 { 0.0 } else { (self.saved as f64 / self.target as f64).max(0.0) as f32 }
    }
    pub fn reached(&self) -> bool {
        if self.kind == GoalKind::Balance {
            if self.falling() { self.saved <= self.target } else { self.saved >= self.target && self.target != self.start }
        } else {
            self.target > 0 && self.saved >= self.target
        }
    }
    pub fn left(&self) -> i64 {
        (if self.falling() { self.saved - self.target } else { self.target - self.saved }).max(0)
    }
    pub fn achieved_percent(&self) -> Option<i64> {
        (self.income > 0).then(|| self.saved * 100 / self.income)
    }
    pub fn streak(&self) -> usize {
        self.history.iter().rev().take_while(|m| m.met()).count()
    }
    pub fn status(&self) -> GoalStatus {
        if self.reached() {
            GoalStatus::Reached
        } else if self.monthly() && self.target <= 0 {
            GoalStatus::Open
        } else {
            match self.pace_fraction {
                None => GoalStatus::Open,
                Some(p) if self.fraction() + 0.005 >= p => GoalStatus::OnTrack,
                Some(_) => GoalStatus::Behind,
            }
        }
    }
}

/// Where the pace tick stands: the share of the days from `first` to `target` gone by today.
pub fn goal_pace_fraction(first: Option<Date>, target: Option<Date>, today: Date) -> Option<f32> {
    let (first, target) = (first?, target?);
    if first >= target {
        return None;
    }
    Some((days(first, today) as f64 / days(first, target) as f64).clamp(0.0, 1.0) as f32)
}

/// What every goal reading needs besides the goals.
#[derive(Clone, Debug, Default)]
pub struct GoalInputs {
    /// The month running now, then the ones before it, newest first.
    pub periods: Vec<BudgetPeriod>,
    /// Each period's (money in, money out).
    pub flows: Vec<(i64, i64)>,
    pub accounts: Vec<AccountView>,
    /// Every transfer over the periods: (date, from, to, amount).
    pub transfers: Vec<(Date, i64, i64, i64)>,
}

pub fn net_worth(accounts: &[AccountView]) -> i64 {
    accounts.iter().filter(|a| !a.archived).map(|a| a.balance).sum()
}

fn moved_into(into: &[i64], transfers: &[(Date, i64, i64, i64)], p: &BudgetPeriod) -> i64 {
    transfers
        .iter()
        .filter(|t| p.contains(t.0))
        .map(|&(_, from, to, amount)| match (into.contains(&from), into.contains(&to)) {
            (false, true) => amount,
            (true, false) => -amount,
            _ => 0,
        })
        .sum()
}

pub fn monthly_ask(percent: i64, amount: i64, income: i64) -> i64 {
    if percent > 0 { income * percent / 100 } else { amount }
}

/// Any goal, read against `inputs`.
pub fn goal_line(g: &Goal, i: &GoalInputs, today: Date) -> GoalLine {
    let months_left = g.target_date.map(|d| months_until(today, d));
    let base = GoalLine {
        id: g.id,
        name: g.name.clone(),
        color: g.color,
        saved: 0,
        target: g.target,
        target_date: g.target_date,
        months_left,
        pace_fraction: None,
        kind: g.kind,
        start: 0,
        percent: 0,
        income: 0,
        account_name: None,
        history: Vec::new(),
    };
    match g.kind {
        GoalKind::Savings => {
            let line = GoalLine { saved: g.contributed, ..base };
            let pace = if line.reached() { None } else { goal_pace_fraction(g.first, g.target_date, today) };
            GoalLine { pace_fraction: pace, ..line }
        }
        GoalKind::Balance => {
            let account = g.account_id.and_then(|id| i.accounts.iter().find(|a| a.id == id));
            let line = GoalLine { saved: account.map_or_else(|| net_worth(&i.accounts), |a| a.balance), start: g.start_amount, account_name: account.map(|a| a.name.clone()), ..base };
            let pace = if line.reached() { None } else { goal_pace_fraction(g.start_date, g.target_date, today) };
            GoalLine { pace_fraction: pace, ..line }
        }
        GoalKind::Invest | GoalKind::Save => {
            let into: Vec<i64> = match g.account_id {
                Some(a) => vec![a],
                None => i.accounts.iter().filter(|a| a.r#type == AccountType::Investment).map(|a| a.id).collect(),
            };
            let month = |n: usize| {
                let p = i.periods[n];
                let (income, spent) = i.flows.get(n).copied().unwrap_or((0, 0));
                let achieved = if g.kind == GoalKind::Invest { moved_into(&into, &i.transfers, &p) } else { income - spent };
                GoalMonth { start: p.start, achieved, asked: monthly_ask(g.percent, g.target, income), income }
            };
            let current = if i.periods.is_empty() { GoalMonth { start: today, achieved: 0, asked: monthly_ask(g.percent, g.target, 0), income: 0 } } else { month(0) };
            let pace = i.periods.first().map(|p| (p.elapsed_days(today) as f32 / p.days() as f32).clamp(0.0, 1.0));
            let line = GoalLine {
                saved: current.achieved,
                target: current.asked,
                target_date: None,
                months_left: None,
                pace_fraction: pace,
                percent: g.percent,
                income: current.income,
                account_name: g.account_id.and_then(|id| i.accounts.iter().find(|a| a.id == id)).map(|a| a.name.clone()),
                history: (1..i.periods.len()).rev().map(month).collect(),
                ..base
            };
            if line.reached() { GoalLine { pace_fraction: None, ..line } } else { line }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GoalsReading {
    pub goals: Vec<GoalLine>,
    /// What the savings pots hold, and their targets.
    pub saved: i64,
    pub target: i64,
    pub reached: usize,
    pub next_date: Option<Date>,
    pub next_date_name: Option<String>,
    pub on_track: usize,
    pub behind: usize,
    pub pots: usize,
}

pub fn goals_reading(goals: Vec<GoalLine>, today: Date) -> GoalsReading {
    let pots: Vec<&GoalLine> = goals.iter().filter(|g| g.kind == GoalKind::Savings).collect();
    let next = goals.iter().filter(|g| !g.reached() && g.target_date.is_some_and(|d| d >= today)).min_by_key(|g| g.target_date);
    GoalsReading {
        saved: pots.iter().map(|g| g.saved.max(0)).sum(),
        target: pots.iter().map(|g| g.target.max(0)).sum(),
        pots: pots.len(),
        reached: goals.iter().filter(|g| g.reached()).count(),
        next_date: next.and_then(|g| g.target_date),
        next_date_name: next.map(|g| g.name.clone()),
        on_track: goals.iter().filter(|g| g.status() == GoalStatus::OnTrack).count(),
        behind: goals.iter().filter(|g| g.status() == GoalStatus::Behind).count(),
        goals,
    }
}

/// How a goal is filling: money a month on average, over how many months, and when that lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GoalPace {
    pub per_month: i64,
    pub months: i64,
    pub reach_by: Option<Date>,
}

pub fn goal_pace(first: Option<Date>, saved: i64, target: i64, today: Date) -> Option<GoalPace> {
    let first = first?;
    if saved <= 0 {
        return None;
    }
    let months = months_until(first, today).max(1);
    let per_month = saved / months;
    let left = target - saved;
    let reach_by = (left > 0 && per_month > 0).then(|| months_ahead(today, left, per_month)).flatten();
    Some(GoalPace { per_month, months, reach_by })
}

/// The day `left` is reached at `per_month` (both above zero), counted in whole months from
/// `today`; none past the calendar's end. A pace of $5 against $50,000 lands ten thousand months
/// out, and jiff's spans stop short of that, so the span is built without a panic.
fn months_ahead(today: Date, left: i64, per_month: i64) -> Option<Date> {
    let n = (left - 1) / per_month + 1;
    let span = jiff::Span::new().try_months(n).ok()?;
    today.checked_add(span).ok()
}

pub fn balance_pace(line: &GoalLine, start: Option<Date>, today: Date) -> Option<GoalPace> {
    let start = start?;
    if line.kind != GoalKind::Balance {
        return None;
    }
    let months = months_until(start, today).max(1);
    let toward = if line.target >= line.start { line.saved - line.start } else { line.start - line.saved };
    if toward <= 0 {
        return Some(GoalPace { per_month: 0, months, reach_by: None });
    }
    let per_month = toward / months;
    let reach_by = (!line.reached() && per_month > 0 && line.left() > 0).then(|| months_ahead(today, line.left(), per_month)).flatten();
    Some(GoalPace { per_month, months, reach_by })
}

/// The words under a goal's name.
pub fn goal_row_line(g: &GoalLine, fmt: &MoneyFormatter, today: Date) -> String {
    let by = g.target_date.map(|d| format!(" · by {}", short(d, today))).unwrap_or_default();
    match g.kind {
        GoalKind::Savings => copy::goal_line(g.saved, g.target, g.months_left, fmt) + &by,
        GoalKind::Balance => {
            let head = match g.months_left {
                _ if g.reached() => "Reached".to_string(),
                None => format!("{} to go", fmt.format_whole(g.left())),
                Some(m) if m <= 0 => format!("{} to go, date passed", fmt.format_whole(g.left())),
                Some(m) => format!("{} a month for {}", fmt.format_whole((g.left() + m - 1) / m), copy::plural(m, "month")),
            };
            format!("{head} · {}{by}", g.account_name.as_deref().unwrap_or("everything you own"))
        }
        GoalKind::Invest | GoalKind::Save => {
            let verb = if g.kind == GoalKind::Invest { "Invested" } else { "Kept" };
            let head = if g.percent > 0 && g.income <= 0 {
                "Nothing came in yet this month".to_string()
            } else if g.reached() {
                format!("{verb} {} this month{}", fmt.format_whole(g.saved), g.achieved_percent().map(|p| format!(", {p}% of what came in")).unwrap_or_default())
            } else {
                format!("{} to go this month{}", fmt.format_whole(g.left()), if g.percent > 0 { format!(" for {}% of what came in", g.percent) } else { String::new() })
            };
            let streak = g.streak();
            if streak > 1 { format!("{head} · {streak} months in a row") } else { head }
        }
    }
}

/// A goal's reading at its row's end: "$1,860 of $3,200", "12% of 10%".
pub fn goal_reading(g: &GoalLine, fmt: &MoneyFormatter) -> String {
    if g.monthly() && g.percent > 0 {
        format!("{}% of {}%", g.achieved_percent().unwrap_or(0).max(0), g.percent)
    } else {
        format!("{} of {}", fmt.format_whole(g.saved), fmt.format_whole(g.target))
    }
}

/// The goal the Goals page features: the one nearest its target that is not reached yet. A pot or
/// a balance goal before a monthly one, whose fraction is only this month's.
pub fn closest_goal(goals: &[GoalLine]) -> Option<usize> {
    let open = |monthly: bool| {
        goals
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.reached() && g.monthly() == monthly && g.target > 0)
            .max_by(|a, b| a.1.fraction().partial_cmp(&b.1.fraction()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
    };
    open(false).or_else(|| open(true)).or(if goals.is_empty() { None } else { Some(0) })
}

/// A goal's line on its card: "$5,200 of $6,000 · 2 months to go".
pub fn goal_card_line(g: &GoalLine, fmt: &MoneyFormatter) -> String {
    let reading = goal_reading(g, fmt);
    match g.months_left {
        _ if g.reached() => format!("{reading} · reached"),
        Some(m) if m > 0 => format!("{reading} · {} to go", copy::plural(m, "month")),
        _ => reading,
    }
}

/// "May 2027", or "May" in this year.
fn month_words(d: Date, today: Date) -> String {
    if d.year() == today.year() { d.strftime("%B").to_string() } else { d.strftime("%B %Y").to_string() }
}

/// What a goal needs, as a sentence: "$310 a month reaches it by May 2027."
pub fn goal_need_line(g: &GoalLine, fmt: &MoneyFormatter, today: Date) -> String {
    if g.monthly() {
        return format!("{}.", goal_row_line(g, fmt, today));
    }
    if g.reached() {
        return if g.kind == GoalKind::Balance { format!("Reached: it reads {}.", fmt.format_whole(g.saved)) } else { format!("Reached, with {} put in.", fmt.format_whole(g.saved)) };
    }
    let left = g.left();
    match (g.months_left, g.target_date) {
        (Some(m), Some(d)) if m > 0 => format!("{} a month reaches it by {}.", fmt.format_whole((left + m - 1) / m), d.strftime("%B %Y")),
        (Some(_), Some(_)) => format!("{} to go, and its date has passed.", fmt.format_whole(left)),
        _ => format!("{} to go, with no date set.", fmt.format_whole(left)),
    }
}

/// How it has filled so far, as a sentence, when that says something: "You have put in $280 a
/// month since June. At that pace it lands in July 2027." `since` is the first contribution (a
/// pot) or the day it was set (a balance goal).
pub fn goal_pace_words(g: &GoalLine, pace: Option<GoalPace>, since: Option<Date>, fmt: &MoneyFormatter, today: Date) -> Option<String> {
    let lands = |p: &GoalPace| p.reach_by.map(|d| format!(" At that pace it lands in {}.", month_words(d, today))).unwrap_or_default();
    match g.kind {
        GoalKind::Savings => match (pace, since) {
            _ if g.reached() => None,
            (Some(p), Some(s)) => Some(format!("You have put in {} a month since {}.{}", fmt.format_whole(p.per_month), month_words(s, today), lands(&p))),
            _ => Some("Nothing put in yet.".into()),
        },
        GoalKind::Balance => match (pace, since) {
            _ if g.reached() => None,
            (Some(p), Some(s)) if p.per_month > 0 => Some(format!("It has moved {} a month since {}.{}", fmt.format_whole(p.per_month), month_words(s, today), lands(&p))),
            (Some(_), Some(s)) => Some(format!("It has not moved toward the target since {}.", month_words(s, today))),
            _ => None,
        },
        GoalKind::Invest | GoalKind::Save => None,
    }
}

pub fn goal_kind_label(k: GoalKind) -> &'static str {
    match k {
        GoalKind::Savings => "Savings pot",
        GoalKind::Balance => "Reach an amount by a date",
        GoalKind::Invest => "Invest every month",
        GoalKind::Save => "Keep part of every month",
    }
}

pub fn goal_kind_hint(k: GoalKind) -> &'static str {
    match k {
        GoalKind::Savings => "Set money aside for something, by hand",
        GoalKind::Balance => "An account, or everything you own, at an amount by a date",
        GoalKind::Invest => "A share of what comes in moved to your investments",
        GoalKind::Save => "A share of what comes in left over after spending",
    }
}

pub fn goal_glyph(k: GoalKind) -> &'static str {
    match k {
        GoalKind::Savings => "target",
        GoalKind::Balance => "flag",
        GoalKind::Invest => "trend",
        GoalKind::Save => "wallet",
    }
}

/// What stops a goal from saving.
pub fn goal_problems(name: &str, kind: GoalKind, target: Option<i64>, by_share: bool, percent: i64) -> (Option<&'static str>, Option<&'static str>) {
    let monthly = matches!(kind, GoalKind::Invest | GoalKind::Save);
    let name_p = name.trim().is_empty().then_some(if kind == GoalKind::Savings { "Name what you are saving for" } else { "Name the goal so you can find it later" });
    let target_p = if kind == GoalKind::Balance && target.is_none() {
        Some("Enter the amount to reach")
    } else if monthly && by_share && !(1..=100).contains(&percent) {
        Some("Pick a share from 1 to 100 percent")
    } else if monthly && by_share {
        None
    } else if kind != GoalKind::Balance && target.unwrap_or(0) <= 0 {
        Some("Enter a target above zero")
    } else {
        None
    };
    (name_p, target_p)
}

/// "Resets on the 1st of every month".
pub fn reset_line(start_day: i64) -> String {
    let n = start_day.max(1);
    let suffix = if (11..=13).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("Resets on the {n}{suffix} of every month")
}

// Reading the ledger

/// Everything the three Plan pages draw, read at once.
pub struct PlanData {
    pub today: Date,
    pub period: BudgetPeriod,
    pub fmt: MoneyFormatter,
    pub budgets: BudgetsReading,
    pub bills: BillsReading,
    pub goals: GoalsReading,
    pub categories: Vec<CategoryView>,
    pub accounts: Vec<AccountView>,
    pub start_day: i64,
    /// The goals as stored, for their pages and editors.
    pub goal_rows: Vec<Goal>,
    pub inputs: GoalInputs,
    /// Each bill's latest posting: (bill id, day).
    pub bill_posts: Vec<(i64, Date)>,
    /// Put into goals this period, less what was taken out.
    pub goals_month: i64,
}

/// The period holding `today`, by the ledger's month start day.
pub fn period_of(store: &Store, today: Date) -> (BudgetPeriod, i64) {
    let day = store.ledger.settings().map(|s| s.month_start_day).unwrap_or(1).clamp(1, 28);
    (BudgetPeriod::containing(today, day as i8), day)
}

pub fn load(store: &Store, today: Date) -> Result<PlanData> {
    let e = |e: relay_money::ledger::LedgerError| e.to_string();
    let conn = store.sql()?;
    let fmt = store.formatter();
    let (period, start_day) = period_of(store, today);
    let categories = store.ledger.categories().map_err(e)?;
    let accounts = store.ledger.accounts().map_err(e)?;

    // Budgets.
    let overall = store.ledger.budget(None).map_err(e)?;
    let mut budgets = Vec::new();
    for c in categories.iter().filter(|c| c.kind == relay_money::model::CategoryKind::Expense) {
        if let Some(a) = store.ledger.budget(Some(c.id)).map_err(e)? {
            budgets.push((c.id, a));
        }
    }
    let spending = plan_store::spent_by_category(conn, period.start, period.end_exclusive)?;
    let (_, spent) = plan_store::flows(conn, period.start, period.end_exclusive)?;
    let last = period.shift(-1);
    let (_, last_spent) = plan_store::flows(conn, last.start, last.end_exclusive)?;
    let mut history = Vec::new();
    for back in (1..=SUGGEST_PERIODS).rev() {
        let p = period.shift(-back);
        history.push(plan_store::spent_by_category(conn, p.start, p.end_exclusive)?);
    }
    let step = 10 * relay_money::money::pow10(fmt.fraction_digits);
    let budgets = budgets_reading(&period, today, overall, &budgets, &spending, spent, last_spent, &categories, &history, step);

    // Bills.
    let bills = bills_reading(plan_store::bills(conn)?, today);
    let bill_posts = plan_store::last_posted_all(conn)?;

    // Goals.
    let goal_rows = plan_store::goals(conn)?;
    let inputs = goal_inputs(conn, &period, accounts.clone())?;
    let lines = goal_rows.iter().filter(|g| !g.archived).map(|g| goal_line(g, &inputs, today)).collect();
    let goals = goals_reading(lines, today);
    let goals_month = plan_store::contributed(conn, period.start, period.end_exclusive)?;

    Ok(PlanData { today, period, fmt, budgets, bills, goals, categories, accounts, start_day, goal_rows, inputs, bill_posts, goals_month })
}

pub fn goal_inputs(conn: &rusqlite::Connection, period: &BudgetPeriod, accounts: Vec<AccountView>) -> Result<GoalInputs> {
    let periods: Vec<BudgetPeriod> = (0..=GOAL_MONTHS).map(|n| period.shift(-n)).collect();
    let mut flows = Vec::new();
    for p in &periods {
        flows.push(plan_store::flows(conn, p.start, p.end_exclusive)?);
    }
    let first = periods.last().map_or(period.start, |p| p.start);
    let transfers = plan_store::transfers(conn, first, period.end_exclusive)?;
    Ok(GoalInputs { periods, flows, accounts, transfers })
}

/// One budget's editor reading: spent in this period and the three before (oldest first), with
/// each period's first day.
pub fn budget_history(conn: &rusqlite::Connection, period: &BudgetPeriod, category: Option<i64>) -> Result<Vec<(Date, i64)>> {
    let mut out = Vec::new();
    for back in (0..4).rev() {
        let p = period.shift(-back);
        let v = match category {
            None => plan_store::flows(conn, p.start, p.end_exclusive)?.1,
            Some(c) => plan_store::spent_by_category(conn, p.start, p.end_exclusive)?.iter().find(|t| t.0 == c).map_or(0, |t| t.1),
        };
        out.push((p.start, v));
    }
    Ok(out)
}
