//! What Insights reads, worked out once per ledger change (Kotlin: InsightsViewModel.kt): the
//! month as a running line against its pace, six months side by side, the days as a heat grid,
//! and where the money went and came from. The charts' series come from relay-money's
//! [`Ledger::series`](relay_money::ledger::Ledger::series); nothing here is summed while drawing.

use std::collections::{BTreeMap, HashSet};

use jiff::ToSpan;
use relay_money::Date;
use relay_money::model::TxType;
use relay_money::period::BudgetPeriod;
use relay_money::series::{By, Measure, SeriesQuery};
use relay_money::views::{CategoryView, Tx, TxQuery};

use crate::activity_data;
use crate::model::{Result, Store};

/// How many periods the Trend lens reads, the selected one included.
pub const TREND_PERIODS: i64 = 6;
/// How many rows a breakdown shows before the rest fold into one.
pub const TOP_CATEGORIES: usize = 8;
pub const LARGEST_COUNT: usize = 3;
pub const TOP_PAYEES: usize = 5;

/// One row of a breakdown: a category, the uncategorised pool, or everything past the top rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub category_id: Option<i64>,
    pub name: String,
    pub icon: String,
    pub color: Option<i64>,
    pub total: i64,
    pub count: usize,
    /// How many categories this row folds together; 0 for one category.
    pub folded: usize,
}

/// One period of the Trend lens; `offset` is periods from the current one.
#[derive(Clone, Debug, PartialEq)]
pub struct TrendPoint {
    pub period: BudgetPeriod,
    pub offset: i64,
    pub spent: i64,
    pub income: i64,
}

impl TrendPoint {
    pub fn net(&self) -> i64 {
        self.income - self.spent
    }
    pub fn running(&self) -> bool {
        self.offset == 0
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrendSummary {
    pub average_spent: i64,
    pub average_income: i64,
    pub months: usize,
    pub over_budget: usize,
}

impl TrendSummary {
    pub fn average_net(&self) -> i64 {
        self.average_income - self.average_spent
    }
}

/// What one weekday took over the days so far.
#[derive(Clone, Debug, PartialEq)]
pub struct WeekdaySpend {
    pub day: jiff::civil::Weekday,
    pub total: i64,
    pub days: i64,
}

impl WeekdaySpend {
    pub fn average(&self) -> i64 {
        if self.days > 0 { self.total / self.days } else { 0 }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalendarSummary {
    pub days_so_far: i64,
    pub active_days: i64,
    pub longest_quiet_run: i64,
    pub busiest: Option<(Date, i64)>,
    pub top_weekday: Option<jiff::civil::Weekday>,
    pub weekend_total: i64,
    pub weekdays: Vec<WeekdaySpend>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Payee {
    pub note: String,
    pub total: i64,
    pub count: usize,
}

/// Everything Insights draws for one period.
#[derive(Clone, Debug)]
pub struct Insights {
    pub today: Date,
    pub period: BudgetPeriod,
    pub offset: i64,
    pub week_starts_monday: bool,
    pub elapsed: i64,
    /// Spent on each day of the period, first day first.
    pub daily: Vec<i64>,
    /// Running spend at the end of each day read so far; days still ahead are not in it.
    pub cumulative: Vec<i64>,
    /// The period before's running spend, every one of its days: the pace card's other line.
    pub last_cumulative: Vec<i64>,
    /// Each day's heat step, 0 to 4.
    pub heat: Vec<u8>,
    pub spent: i64,
    pub income: i64,
    pub budget: Option<i64>,
    pub last_same_day: i64,
    pub categories: Vec<Slice>,
    pub category_count: usize,
    pub income_sources: Vec<Slice>,
    pub largest: Vec<Tx>,
    pub by_day: BTreeMap<Date, Vec<Tx>>,
    pub calendar: CalendarSummary,
    pub trend: Vec<TrendPoint>,
    pub trend_summary: TrendSummary,
    pub payees: Vec<Payee>,
    pub repeats: HashSet<i64>,
}

impl Insights {
    pub fn current(&self) -> bool {
        self.offset >= 0
    }
    pub fn per_day(&self) -> i64 {
        if self.elapsed > 0 { self.spent / self.elapsed } else { 0 }
    }
    /// The share of what came in that was not spent; none with no income.
    pub fn kept(&self) -> Option<i64> {
        kept_share(self.income, self.spent)
    }
    /// Where an even spend of today's rate would end the period; none once it is over or before
    /// any spend.
    pub fn projected(&self) -> Option<i64> {
        let days = self.period.days();
        (self.current() && self.elapsed >= 1 && self.elapsed < days && self.spent > 0).then(|| self.spent * days / self.elapsed)
    }
    pub fn largest_total(&self) -> i64 {
        self.largest.iter().map(|t| t.amount).sum()
    }
}

/// A running total of `values`.
pub fn running(values: &[i64]) -> Vec<i64> {
    let mut total = 0;
    values
        .iter()
        .map(|v| {
            total += v;
            total
        })
        .collect()
}

/// The pace card's comparison on day `day` (1 is the first): how much less was spent by then than
/// the period before had (negative when more). None when the period before has no day that far.
pub fn ahead_by(cumulative: &[i64], last: &[i64], day: usize) -> Option<i64> {
    let i = day.checked_sub(1)?;
    let now = *cumulative.get(i)?;
    let then = *last.get(i.min(last.len().checked_sub(1)?))?;
    Some(then - now)
}

/// Round steps for a chart's scale up to `max` (minor units, at most five lines): each step is
/// 1, 2 or 5 times a power of ten of the currency's whole unit. Always starts at zero.
pub fn ticks(max: i64, unit: i64) -> Vec<i64> {
    let unit = unit.max(1);
    if max <= 0 {
        return vec![0, unit];
    }
    let raw = (max as f64 / unit as f64 / 4.0).max(1.0);
    let pow = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * pow).find(|s| *s >= raw).unwrap_or(10.0 * pow);
    let step = (step * unit as f64).round() as i64;
    let mut out = vec![0];
    while *out.last().unwrap_or(&0) < max {
        out.push(out.last().unwrap_or(&0) + step);
    }
    out
}

pub fn kept_share(income: i64, spent: i64) -> Option<i64> {
    (income > 0).then(|| ((income - spent) as f64 * 100.0 / income as f64).round() as i64)
}

/// What an even spend of `budget` would have used by the end of day `index` (0 is the first).
pub fn pace_at(budget: i64, index: usize, days: i64) -> i64 {
    if budget <= 0 || days <= 0 {
        return 0;
    }
    let elapsed = (index as i64 + 1).clamp(0, days);
    (budget as f64 * elapsed as f64 / days as f64).round() as i64
}

/// A day's heat against the busiest day: 0 unlit, then four steps by quarter. Any spend at all
/// lights the first, so a coffee never reads as an empty day.
pub fn heat_step(value: i64, max: i64) -> u8 {
    if value <= 0 || max <= 0 {
        return 0;
    }
    let f = value as f64 / max as f64;
    if f <= 0.25 {
        1
    } else if f <= 0.5 {
        2
    } else if f <= 0.75 {
        3
    } else {
        4
    }
}

/// Totals per category as ranked rows, largest first; past `limit` the tail folds into one
/// "Everything else" row.
pub fn rank_slices(rows: &[Tx], ty: TxType, categories: &[CategoryView], limit: usize) -> Vec<Slice> {
    let mut by: Vec<Slice> = Vec::new();
    for t in rows.iter().filter(|t| t.r#type == ty && t.amount > 0) {
        let c = t.category_id.and_then(|id| categories.iter().find(|c| c.id == id));
        let id = c.map(|c| c.id);
        match by.iter_mut().find(|s| s.category_id == id) {
            Some(s) => {
                s.total += t.amount;
                s.count += 1;
            }
            None => by.push(Slice {
                category_id: id,
                name: c.map(|c| c.name.clone()).unwrap_or_else(|| "Uncategorized".into()),
                icon: c.map(|c| c.icon.clone()).unwrap_or_else(|| "dots".into()),
                color: c.map(|c| c.color),
                total: t.amount,
                count: 1,
                folded: 0,
            }),
        }
    }
    by.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));
    if by.len() <= limit || limit < 2 {
        return by;
    }
    let rest = by.split_off(limit - 1);
    by.push(Slice {
        category_id: None,
        name: "Everything else".into(),
        icon: "dots".into(),
        color: None,
        total: rest.iter().map(|s| s.total).sum(),
        count: rest.iter().map(|s| s.count).sum(),
        folded: rest.len(),
    });
    by
}

/// Averages over the periods with any entry, the running one left out unless it is the only one.
pub fn summarize_trend(points: &[TrendPoint], budget: Option<i64>) -> TrendSummary {
    let with_data: Vec<&TrendPoint> = points.iter().filter(|p| p.spent > 0 || p.income > 0).collect();
    let full: Vec<&TrendPoint> = with_data.iter().copied().filter(|p| !p.running()).collect();
    let basis = if full.is_empty() { with_data } else { full };
    if basis.is_empty() {
        return TrendSummary::default();
    }
    let n = basis.len() as i64;
    TrendSummary {
        average_spent: basis.iter().map(|p| p.spent).sum::<i64>() / n,
        average_income: basis.iter().map(|p| p.income).sum::<i64>() / n,
        months: basis.len(),
        over_budget: budget.filter(|b| *b > 0).map_or(0, |b| points.iter().filter(|p| p.spent > b).count()),
    }
}

/// The calendar's readings over the first `elapsed` days of `daily`.
pub fn summarize_calendar(daily: &[i64], period: &BudgetPeriod, elapsed: i64, week_starts_monday: bool) -> CalendarSummary {
    use jiff::civil::Weekday;
    let days = &daily[..(elapsed.max(0) as usize).min(daily.len())];
    if days.is_empty() {
        return CalendarSummary::default();
    }
    let (mut longest, mut run) = (0, 0);
    for v in days {
        if *v > 0 {
            run = 0;
        } else {
            run += 1;
            longest = longest.max(run);
        }
    }
    let date = |i: usize| period.start.checked_add((i as i64).days()).unwrap_or(period.start);
    let busiest = (0..days.len()).max_by_key(|i| (days[*i], std::cmp::Reverse(*i))).filter(|i| days[*i] > 0).map(|i| (date(i), days[i]));
    let first = if week_starts_monday { Weekday::Monday } else { Weekday::Sunday };
    let weekdays: Vec<WeekdaySpend> = (0..7)
        .map(|k| {
            let day = first.wrapping_add(k);
            let idx: Vec<usize> = (0..days.len()).filter(|i| date(*i).weekday() == day).collect();
            WeekdaySpend { day, total: idx.iter().map(|i| days[*i]).sum(), days: idx.len() as i64 }
        })
        .collect();
    let top = weekdays.iter().filter(|w| w.total > 0).max_by_key(|w| w.average()).map(|w| w.day);
    CalendarSummary {
        days_so_far: days.len() as i64,
        active_days: days.iter().filter(|v| **v > 0).count() as i64,
        longest_quiet_run: longest,
        busiest,
        top_weekday: top,
        weekend_total: (0..days.len()).filter(|i| matches!(date(*i).weekday(), Weekday::Saturday | Weekday::Sunday)).map(|i| days[i]).sum(),
        weekdays,
    }
}

/// The period's biggest payees by what they took, from the entries' notes.
pub fn payees(rows: &[Tx], top: usize) -> Vec<Payee> {
    let mut by: Vec<(String, Payee)> = Vec::new();
    for t in rows.iter().filter(|t| t.r#type == TxType::Expense && !t.note.trim().is_empty()) {
        let key = t.note.trim().to_lowercase();
        match by.iter_mut().find(|(k, _)| *k == key) {
            Some((_, p)) => {
                p.total += t.amount;
                p.count += 1;
            }
            None => by.push((key, Payee { note: t.note.trim().to_string(), total: t.amount, count: 1 })),
        }
    }
    let mut out: Vec<Payee> = by.into_iter().map(|(_, p)| p).collect();
    out.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.note.cmp(&b.note)));
    out.truncate(top);
    out
}

fn series(store: &Store, today: Date, measure: Measure, by: By, periods: u32, offset: i64, cumulative: bool) -> Result<relay_money::series::Series> {
    let q = SeriesQuery {
        measure: Some(measure),
        by: Some(by),
        periods: Some(periods),
        period_offset: Some(offset),
        cumulative: Some(cumulative),
        today: Some(today.to_string()),
        ..Default::default()
    };
    store.ledger.series(&q, today).map_err(|e| e.to_string())
}

/// Read one period's Insights.
pub fn read(store: &Store, today: Date, offset: i64) -> Result<Insights> {
    let err = |e: relay_money::ledger::LedgerError| e.to_string();
    let offset = offset.min(0);
    let period = activity_data::period_at(store, today, offset);
    let days = period.days();
    let elapsed = period.elapsed_days(today).clamp(0, days);
    let settings = store.ledger.settings().map_err(err)?;

    // The month's days, from the ledger's own chart series.
    let by_day = series(store, today, Measure::Spending, By::Day, 1, offset, false)?;
    let mut daily = by_day.series.first().map(|l| l.values.clone()).unwrap_or_default();
    daily.resize(days as usize, 0);
    let mut total = 0;
    let cumulative: Vec<i64> = daily
        .iter()
        .take(elapsed as usize)
        .map(|v| {
            total += v;
            total
        })
        .collect();
    let last_daily = series(store, today, Measure::Spending, By::Day, 1, offset - 1, false)?.series.first().map(|l| l.values.clone()).unwrap_or_default();
    let last_cumulative = running(&last_daily);
    let busiest = daily.iter().take(elapsed as usize).copied().max().unwrap_or(0);
    let heat = daily.iter().enumerate().map(|(i, v)| if (i as i64) < elapsed { heat_step(*v, busiest) } else { 0 }).collect();

    let q = TxQuery { from: Some(period.start), to: Some(period.last_day()), limit: Some(100_000), ..Default::default() };
    let page = store.ledger.tx_list(&q, today).map_err(err)?;
    let rows = page.transactions;
    let categories = store.ledger.categories().map_err(err)?;

    // What the period before had spent by the same day (all of it, for a past period).
    let prev = period.shift(-1);
    let prev_last = if offset < 0 {
        prev.last_day()
    } else {
        prev.start.checked_add((elapsed - 1).max(0).days()).unwrap_or(prev.start).min(prev.last_day())
    };
    let last_same_day = store.ledger.tx_list(&TxQuery { from: Some(prev.start), to: Some(prev_last), limit: Some(1), ..Default::default() }, today).map_err(err)?.spent;

    // Six months, from the series by period.
    let spent6 = series(store, today, Measure::Spending, By::Period, TREND_PERIODS as u32, offset, false)?;
    let income6 = series(store, today, Measure::Income, By::Period, TREND_PERIODS as u32, offset, false)?;
    let sv = spent6.series.first().map(|l| l.values.clone()).unwrap_or_default();
    let iv = income6.series.first().map(|l| l.values.clone()).unwrap_or_default();
    let trend: Vec<TrendPoint> = (0..TREND_PERIODS)
        .map(|i| {
            let o = offset - (TREND_PERIODS - 1) + i;
            TrendPoint { period: activity_data::period_at(store, today, o), offset: o, spent: sv.get(i as usize).copied().unwrap_or(0), income: iv.get(i as usize).copied().unwrap_or(0) }
        })
        .collect();

    let budget = store.ledger.budget(None).map_err(err)?.filter(|b| *b > 0);
    let spent: i64 = daily.iter().sum();
    let mut largest: Vec<Tx> = rows.iter().filter(|t| t.r#type == TxType::Expense).cloned().collect();
    largest.sort_by_key(|t| std::cmp::Reverse(t.amount));
    largest.truncate(LARGEST_COUNT);
    let mut grouped: BTreeMap<Date, Vec<Tx>> = BTreeMap::new();
    for t in &rows {
        if let Ok(d) = t.date.parse::<Date>() {
            grouped.entry(d).or_default().push(t.clone());
        }
    }
    let expense_slices = rank_slices(&rows, TxType::Expense, &categories, TOP_CATEGORIES);
    let category_count = {
        let ids: HashSet<Option<i64>> = rows
            .iter()
            .filter(|t| t.r#type == TxType::Expense && t.amount > 0)
            .map(|t| t.category_id.filter(|id| categories.iter().any(|c| c.id == *id)))
            .collect();
        ids.len()
    };
    Ok(Insights {
        today,
        period,
        offset,
        week_starts_monday: settings.week_starts_monday,
        elapsed,
        calendar: summarize_calendar(&daily, &period, elapsed, settings.week_starts_monday),
        cumulative,
        last_cumulative,
        heat,
        spent,
        income: page.income,
        budget,
        last_same_day,
        categories: expense_slices,
        category_count,
        income_sources: rank_slices(&rows, TxType::Income, &categories, TOP_CATEGORIES),
        largest,
        by_day: grouped,
        trend_summary: summarize_trend(&trend, budget),
        trend,
        payees: payees(&rows, TOP_PAYEES),
        repeats: activity_data::bill_posted(store),
        daily,
    })
}
