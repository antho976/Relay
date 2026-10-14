//! Chart series for Threads (docs/THREADS.md): spending, income or net, by category, week, day or
//! budget period, over one or more budget periods.
//!
//! A reading of the ledger for the PC's charts, not a money rule: the phone has no counterpart, so
//! nothing here needs mirroring in Tally's `core/` (antho976/Tally). Amounts stay minor units;
//! transfers count as neither income nor spending, as everywhere else.

use crate::model::TxType;
use crate::period::{days_between, BudgetPeriod};
use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a chart counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MoneySeriesMeasure")]
pub enum Measure {
    #[default]
    Spending,
    Income,
    /// Income less spending.
    Net,
}

/// What a chart's labels are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MoneySeriesBy")]
pub enum By {
    /// One label per category, the largest first; each period is a series.
    #[default]
    Category,
    /// Week 1 is the period's first seven days; each period is a series.
    Week,
    /// Day 1 is the period's first day; each period is a series.
    Day,
    /// One label per budget period, oldest first; one series.
    Period,
}

/// The question a chart asks. A chart keeps this, not its numbers, so it redraws when the ledger
/// moves.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MoneySeriesIn")]
pub struct SeriesQuery {
    pub measure: Option<Measure>,
    pub by: Option<By>,
    /// How many budget periods, ending with `period_offset`'s: 1 to 6 (24 by period). Default 1.
    pub periods: Option<u32>,
    /// The last period: 0 is the current one, -1 the one before.
    pub period_offset: Option<i64>,
    /// Only this category, by name (any case).
    pub category: Option<String>,
    /// Running totals across weeks or days, for pace.
    pub cumulative: Option<bool>,
    /// The day to read the current period from; today in the engine's time zone when absent.
    pub today: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneySeriesLine")]
pub struct SeriesLine {
    pub name: String,
    /// One value per label, minor units.
    #[schemars(extend("x-unit" = "minor"))]
    pub values: Vec<i64>,
    /// By week or day, in the period still running: how many labels it has reached. Later ones
    /// have not happened yet, so a running total stops here instead of going flat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneySeries")]
pub struct Series {
    pub currency: String,
    #[schemars(extend("x-unit" = "omit"))]
    pub fraction_digits: u32,
    pub measure: Measure,
    pub by: By,
    pub labels: Vec<String>,
    /// Each label's category colour (Tally's palette index), when the labels are categories.
    #[schemars(extend("x-unit" = "omit"))]
    pub label_colors: Vec<Option<i64>>,
    pub series: Vec<SeriesLine>,
}

/// One entry, as a series reads it.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub r#type: TxType,
    pub amount: i64,
    pub date: Date,
    pub category_id: Option<i64>,
}

/// A category, as a series names it.
#[derive(Debug, Clone)]
pub struct Named {
    pub id: i64,
    pub name: String,
    pub color: i64,
}

/// The most categories a chart names; the rest are summed as Other.
pub const MAX_CATEGORIES: usize = 8;

/// How many periods a query may span.
pub fn period_count(q: &SeriesQuery) -> usize {
    let most = if q.by.unwrap_or_default() == By::Period { 24 } else { 6 };
    q.periods.unwrap_or(1).clamp(1, most) as usize
}

/// A period's name: its month ("September") for a calendar month, else its first day ("Sep 15").
pub fn period_name(p: &BudgetPeriod, with_year: bool) -> String {
    let base = if p.start.day() == 1 { p.start.strftime("%B").to_string() } else { p.start.strftime("%b %-d").to_string() };
    if with_year { format!("{base} {}", p.start.year()) } else { base }
}

fn signed(measure: Measure, row: &Row) -> i64 {
    match (measure, row.r#type) {
        (Measure::Spending, TxType::Expense) | (Measure::Income | Measure::Net, TxType::Income) => row.amount,
        (Measure::Net, TxType::Expense) => -row.amount,
        _ => 0,
    }
}

/// Sort `rows` into a series over `periods` (oldest first), as read on `today`.
pub fn bucket(rows: &[Row], periods: &[BudgetPeriod], by: By, measure: Measure, cumulative: bool, categories: &[Named], today: Date) -> (Vec<String>, Vec<Option<i64>>, Vec<SeriesLine>) {
    let years: std::collections::BTreeSet<i16> = periods.iter().map(|p| p.start.year()).collect();
    let with_year = years.len() > 1;
    let which = |date: Date| periods.iter().position(|p| p.contains(date));
    match by {
        By::Period => {
            let mut values = vec![0; periods.len()];
            for row in rows {
                if let Some(i) = which(row.date) {
                    values[i] += signed(measure, row);
                }
            }
            let name = match measure {
                Measure::Spending => "Spending",
                Measure::Income => "Income",
                Measure::Net => "Net",
            };
            let labels = periods.iter().map(|p| period_name(p, with_year)).collect::<Vec<_>>();
            let colors = vec![None; labels.len()];
            (labels, colors, vec![SeriesLine { name: name.into(), values, known: None }])
        }
        By::Week | By::Day => {
            let step = if by == By::Week { 7 } else { 1 };
            let longest = periods.iter().map(BudgetPeriod::days).max().unwrap_or(1);
            let slots = ((longest + step - 1) / step) as usize;
            let mut lines: Vec<SeriesLine> = periods
                .iter()
                .map(|p| SeriesLine {
                    name: period_name(p, with_year),
                    values: vec![0; slots],
                    known: p.contains(today).then(|| (days_between(p.start, today) / step + 1) as u32),
                })
                .collect();
            for row in rows {
                if let Some(i) = which(row.date) {
                    let slot = (days_between(periods[i].start, row.date) / step) as usize;
                    if let Some(v) = lines[i].values.get_mut(slot) {
                        *v += signed(measure, row);
                    }
                }
            }
            if cumulative {
                for line in &mut lines {
                    let mut total = 0;
                    for v in &mut line.values {
                        total += *v;
                        *v = total;
                    }
                }
            }
            let labels = (1..=slots).map(|n| if by == By::Week { format!("W{n}") } else { n.to_string() }).collect::<Vec<_>>();
            let colors = vec![None; labels.len()];
            (labels, colors, lines)
        }
        By::Category => {
            // Totals per category per period; the key None is entries with no category.
            let mut totals: Vec<(Option<i64>, Vec<i64>)> = Vec::new();
            for row in rows {
                let Some(i) = which(row.date) else { continue };
                let value = signed(measure, row);
                if value == 0 {
                    continue;
                }
                let slot = match totals.iter().position(|(c, _)| *c == row.category_id) {
                    Some(s) => s,
                    None => {
                        totals.push((row.category_id, vec![0; periods.len()]));
                        totals.len() - 1
                    }
                };
                totals[slot].1[i] += value;
            }
            totals.sort_by_key(|(c, v)| (std::cmp::Reverse(v.iter().map(|x| x.abs()).sum::<i64>()), *c));
            let mut labels = Vec::new();
            let mut colors = Vec::new();
            let mut columns: Vec<Vec<i64>> = Vec::new();
            for (n, (category, values)) in totals.into_iter().enumerate() {
                if n >= MAX_CATEGORIES {
                    let other = columns.last_mut().expect("MAX_CATEGORIES > 0");
                    for (o, v) in other.iter_mut().zip(values) {
                        *o += v;
                    }
                    if n == MAX_CATEGORIES {
                        *labels.last_mut().expect("named") = String::from("Other");
                        *colors.last_mut().expect("named") = None;
                    }
                    continue;
                }
                let named = category.and_then(|id| categories.iter().find(|c| c.id == id));
                labels.push(named.map_or_else(|| String::from("No category"), |c| c.name.clone()));
                colors.push(named.map(|c| c.color));
                columns.push(values);
            }
            let lines = periods
                .iter()
                .enumerate()
                .map(|(i, p)| SeriesLine { name: period_name(p, with_year), values: columns.iter().map(|c| c[i]).collect(), known: None })
                .collect();
            (labels, colors, lines)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn row(ty: TxType, amount: i64, date: &str, category: Option<i64>) -> Row {
        Row { r#type: ty, amount, date: d(date), category_id: category }
    }

    fn months() -> Vec<BudgetPeriod> {
        let october = BudgetPeriod::containing(d("2026-10-07"), 1);
        vec![october.shift(-1), october]
    }

    #[test]
    fn weeks_split_each_period_from_its_first_day() {
        let rows = [
            row(TxType::Expense, 8_800, "2026-09-03", Some(1)),
            row(TxType::Expense, 14_100, "2026-09-16", Some(1)),
            row(TxType::Expense, 4_218, "2026-10-06", Some(1)),
            row(TxType::Income, 100_000, "2026-10-02", None),
        ];
        let today = d("2026-10-07");
        let (labels, _, lines) = bucket(&rows, &months(), By::Week, Measure::Spending, false, &[], today);
        assert_eq!(labels, ["W1", "W2", "W3", "W4", "W5"]);
        assert_eq!(lines[0].name, "September");
        assert_eq!(lines[0].values, [8_800, 0, 14_100, 0, 0]);
        assert_eq!(lines[0].known, None, "September is over");
        assert_eq!(lines[1].values, [4_218, 0, 0, 0, 0]);
        assert_eq!(lines[1].known, Some(1), "October has reached its first week");
        let (_, _, running) = bucket(&rows, &months(), By::Week, Measure::Spending, true, &[], today);
        assert_eq!(running[0].values, [8_800, 8_800, 22_900, 22_900, 22_900]);
        let (_, _, days) = bucket(&rows, &months(), By::Day, Measure::Spending, true, &[], today);
        assert_eq!(days[1].known, Some(7));
        let (_, _, net) = bucket(&rows, &months(), By::Period, Measure::Net, false, &[], today);
        assert_eq!(net[0].values, [-22_900, 100_000 - 4_218]);
    }

    #[test]
    fn categories_rank_by_total_and_the_tail_is_other() {
        let categories: Vec<Named> = (1..=10).map(|id| Named { id, name: format!("C{id}"), color: id }).collect();
        let mut rows: Vec<Row> = (1..=10).map(|id| row(TxType::Expense, id * 100, "2026-10-03", Some(id))).collect();
        rows.push(row(TxType::Expense, 50, "2026-10-04", None));
        let (labels, colors, lines) = bucket(&rows, &months()[1..], By::Category, Measure::Spending, false, &categories, d("2026-10-07"));
        assert_eq!(labels.len(), MAX_CATEGORIES);
        assert_eq!(labels[0], "C10");
        assert_eq!(colors[0], Some(10));
        assert_eq!(labels[7], "Other");
        assert_eq!(colors[7], None);
        let total: i64 = lines[0].values.iter().sum();
        assert_eq!(total, (1..=10).map(|id| id * 100).sum::<i64>() + 50, "nothing is dropped");
    }

    #[test]
    fn periods_are_named_and_counted_within_bounds() {
        let p = BudgetPeriod::containing(d("2026-10-20"), 15);
        assert_eq!(period_name(&p, false), "Oct 15");
        assert_eq!(period_name(&months()[0], true), "September 2026");
        assert_eq!(period_count(&SeriesQuery { periods: Some(40), ..Default::default() }), 6);
        assert_eq!(period_count(&SeriesQuery { periods: Some(40), by: Some(By::Period), ..Default::default() }), 24);
        assert_eq!(period_count(&SeriesQuery::default()), 1);
    }
}
