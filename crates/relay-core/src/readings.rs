//! Readings for small models (docs/THREADS.md, "Small models"): what a read op answered, as short
//! labelled lines a 4B model can quote.
//!
//! A small model misreads JSON in ways a large one does not: it takes the wrong one of two close
//! fields, adds figures up wrongly, counts weekdays wrongly and reads "last month" as this one.
//! So each reading says what it is and what today is, gives money as "$1,234.56" and days as
//! "Thu Oct 15", and works out what a question is likely to need (left, pace, totals by category,
//! the biggest entry, this week so far) and names it in words. Ids appear only where a write
//! needs one.
//!
//! The readings are read from the bus's own results (minor units, 1e-8, basis points), never
//! from the MCP door's decimals, so the engine and the door read the same way. A summary folds
//! in the reads it is most often followed by ([`extras`]), so one call answers most questions.

use jiff::civil::Date;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// What every reading needs besides the result: the day it is read on and the ledger's currency,
/// for a result that does not name its own.
pub struct Ctx {
    pub today: Date,
    pub currency: String,
}

/// The read ops that have a reading.
pub const OPS: &[&str] = &[
    "money.summary", "money.tx.list", "money.lists", "money.series", "money.invest.summary", "money.invest.list",
    "gym.summary", "gym.sessions", "gym.session.get", "gym.lifts", "gym.lift.get", "gym.cardio", "gym.series",
];

/// The reads a reading of `op` (asked with `payload`) folds in, given its `result`, in the order
/// [`render`] takes them.
pub fn extras(op: &str, payload: &Value, result: &Value, today: Date) -> Vec<(&'static str, Value)> {
    use serde_json::json;
    let by_month = |measure: &str| ("gym.series", json!({"measure": measure, "by": "month", "periods": 6}));
    match op {
        "money.summary" => vec![
            ("money.tx.list", json!({})),
            ("money.tx.list", json!({"period_offset": -1})),
            ("money.tx.list", json!({"period_offset": -2})),
            ("money.series", json!({"by": "period", "periods": 6})),
            ("money.series", json!({"by": "period", "periods": 6, "measure": "income"})),
            ("money.invest.summary", json!({})),
            ("money.lists", json!({})),
            // The whole ledger: every pay ("which months had 3 paycheques", "when did my pay go
            // up"), this calendar year, and what made a past budget month cost what it did.
            ("money.tx.list", json!({"from": "1970-01-01"})),
        ],
        "gym.summary" => {
            let mut reads = vec![
                ("gym.lifts", json!({})), ("gym.cardio", json!({"limit": SUMMARY_CARDIO})), by_month("sessions"), by_month("volume"), by_month("cardio_minutes"), by_month("distance"),
                ("gym.sessions", json!({"limit": 14})),
                // "This year": the months above are the last six (bench v3, 2026-10-30).
                ("gym.sessions", json!({"from": jiff::civil::date(today.year(), 1, 1).to_string(), "to": today.to_string(), "limit": 1})),
            ];
            if let Some(id) = result["recent"].get(0).and_then(|x| x["id"].as_i64()) {
                reads.push(("gym.session.get", json!({"id": id})));
            }
            reads
        }
        // An account valued by hand is worth its balance, which the portfolio does not carry.
        "money.invest.summary" => vec![("money.lists", json!({}))],
        // Which accounts Wealthsimple's files fill, which an activity does not say.
        "money.invest.list" => vec![("money.lists", json!({}))],
        // Which categories are for spending, so money back filed under one is told apart; and
        // for a search for money back, the same days unsearched: a return noted in French or by
        // the shop's name is not found by "refund".
        "money.tx.list" => {
            let mut reads = vec![("money.lists", json!({}))];
            let period = &result["period"];
            let last = date(s(period, "end_exclusive")).and_then(|d| d.yesterday().ok());
            if let (true, Some(last)) = (money_back_search(payload, result), last) {
                reads.push(("money.tx.list", json!({"from": s(period, "start"), "to": last.to_string()})));
            }
            // A search with no days reads this budget month alone: "Costco this year" and an entry
            // from months ago were not found there (bench v3, 2026-10-30).
            if ledger_search(payload) {
                let mut all = json!({"from": "1970-01-01", "query": payload["query"]});
                if payload["category_id"].is_i64() {
                    all["category_id"] = payload["category_id"].clone();
                }
                reads.push(("money.tx.list", all));
            }
            reads
        }
        // What went in each category lately: a second invoice from a client goes where the first did.
        "money.lists" => vec![("money.tx.list", json!({})), ("money.tx.list", json!({"period_offset": -1})), ("money.tx.list", json!({"period_offset": -2}))],
        // A workout list does not say its unit; a week's volume does.
        "gym.sessions" => vec![("gym.series", json!({"periods": 1}))],
        // The other lifts: one they moved to in its place (a dumbbell bench for the bench).
        "gym.lift.get" => vec![("gym.lifts", json!({}))],
        _ => Vec::new(),
    }
}

/// `result` of `op` (asked with `payload`) as a reading; `None` for an op without one. `extras`
/// are the results of [`extras`]`(op)`, each `None` when it could not be read.
pub fn render(op: &str, payload: &Value, result: &Value, extras: &[Option<Value>], ctx: &Ctx) -> Option<String> {
    let extra = |i: usize| extras.get(i).and_then(Option::as_ref);
    let text = match op {
        "money.summary" => money_summary(result, [extra(0), extra(1), extra(2)], [extra(3), extra(4)], extra(5), extra(6), extra(7), ctx),
        "money.tx.list" => {
            // In the order `extras` read them: money back only when the period has an end.
            let back = money_back_search(payload, result) && date(s(&result["period"], "end_exclusive")).is_some();
            let ledger = if ledger_search(payload) { extra(if back { 2 } else { 1 }) } else { None };
            tx_list(payload, result, extra(0), extra(1).filter(|_| back), ledger, ctx)
        }
        "money.lists" => money_lists(result, [extra(0), extra(1), extra(2)], ctx),
        "money.series" => money_series(payload, result, ctx),
        "money.invest.summary" => invest_summary(result, extra(0), ctx),
        "money.invest.list" => invest_list(result, extra(0), ctx),
        "gym.summary" => gym_summary(result, extra(0), extra(1), [extra(2), extra(3), extra(4), extra(5)], [extra(6), extra(7)], extra(8), ctx),
        "gym.sessions" => gym_sessions(payload, result, extra(0).map_or("", |series| s(series, "unit")), ctx),
        "gym.session.get" => gym_session(result, ctx),
        "gym.lifts" => gym_lifts(payload, result, ctx),
        "gym.lift.get" => gym_lift(result, extra(0), ctx),
        "gym.cardio" => gym_cardio(payload, result, ctx),
        "gym.series" => gym_series(payload, result),
        _ => return None,
    };
    Some(text.trim_end().to_string())
}

/// The newest pay (Salary income) among `pages`' entries.
pub fn last_pay<'a>(pages: impl IntoIterator<Item = &'a Value>) -> Option<&'a Value> {
    pages.into_iter().flat_map(|p| list(p, "transactions"))
        .filter(|t| s(t, "type") == "INCOME" && s(t, "category").eq_ignore_ascii_case("Salary"))
        .max_by(|a, b| s(a, "date").cmp(s(b, "date")).then(int(a, "id").cmp(&int(b, "id"))))
}

/// How many pays (Salary income) came in each budget month of the whole ledger, and which calendar
/// months had three: a pay every two weeks lands three times in some months, which no one
/// month's reading shows. `start` is this budget month's first day, whose day of the month the
/// others share.
fn pays_by_month(page: &Value, start: Option<Date>, today: Date) -> Option<String> {
    // A page cut short would leave the oldest months out.
    if page["more"] == true {
        return None;
    }
    let mut days: Vec<Date> = list(page, "transactions").iter()
        .filter(|t| s(t, "type") == "INCOME" && s(t, "category").eq_ignore_ascii_case("Salary")).filter_map(|t| date(s(t, "date"))).collect();
    days.sort();
    let count = |month_of: &dyn Fn(Date) -> Option<Date>| {
        let mut months: Vec<(Date, i64)> = Vec::new();
        for d in &days {
            let Some(m) = month_of(*d) else { continue };
            match months.last_mut() {
                Some((at, n)) if *at == m => *n += 1,
                _ => months.push((m, 1)),
            }
        }
        months
    };
    let calendar = count(&|d| d.first_of_month().into());
    if calendar.len() < 3 {
        return None;
    }
    let three = |months: &[(Date, i64)], name: &dyn Fn(Date) -> String| {
        let these: Vec<String> = months.iter().filter(|(_, n)| *n >= 3).map(|(m, _)| name(*m)).collect();
        if these.is_empty() { "none".to_string() } else { these.join(", ") }
    };
    let in_calendar = three(&calendar, &|m| m.strftime("%B %Y").to_string());
    // A budget month from the 1st is the calendar month; past the 28th it has no fixed day.
    let Some(k) = start.map(|d| d.day()).filter(|k| (2..=28).contains(k)) else {
        let cells: Vec<String> = calendar.iter().map(|(m, n)| format!("{} {n}{}", m.strftime("%B %Y"), if m.last_of_month() >= today { " so far" } else { "" })).collect();
        return Some(format!("- Pays (Salary) in each month, the whole ledger: {}; months with 3 or more: {in_calendar}.", cells.join(", ")));
    };
    let budget = count(&|d| {
        let m = if d.day() >= k { d } else { d.first_of_month().yesterday().ok()? };
        m.with().day(k).build().ok()
    });
    let last = |m: Date| m.checked_add(jiff::Span::new().months(1)).ok().and_then(|n| n.yesterday().ok()).unwrap_or(m);
    let name = |m: Date| format!("{} to {}", m.strftime("%b %-d"), last(m).strftime("%b %-d %Y"));
    // Most months hold the usual count: name it once and the months that differ, not every month.
    let mut counts: Vec<i64> = budget.iter().map(|(_, n)| *n).collect();
    counts.sort_unstable();
    let usual = counts[counts.len() / 2];
    let other: Vec<String> = budget.iter().filter(|(m, n)| *n != usual && *n < 3 && last(*m) < today).map(|(m, n)| format!("{} ({n})", name(*m))).collect();
    let other = if other.is_empty() { String::new() } else { format!(", fewer in {}", other.join(", ")) };
    Some(format!("- Pays (Salary), the whole ledger: {usual} in most budget months{other}. Budget months with 3 or more: {}. Calendar months with 3 or more: {in_calendar}.",
        three(&budget, &name)))
}

/// When the pay (Salary income) changed, by how much a pay and over a year of pays: "when did my
/// pay go up" was read off the budget months holding three pays (bench v3, 2026-10-30). A lone
/// pay of another amount (a bonus, a short week) is not a change.
fn pay_changes(page: &Value, m: &Money, today: Date) -> Option<String> {
    let mut pays: Vec<(Date, i64)> = list(page, "transactions").iter()
        .filter(|t| s(t, "type") == "INCOME" && s(t, "category").eq_ignore_ascii_case("Salary"))
        .filter_map(|t| Some((date(s(t, "date"))?, int(t, "amount")))).collect();
    if pays.len() < 4 {
        return None;
    }
    pays.sort_unstable();
    let mut gaps: Vec<i64> = pays.windows(2).map(|w| i64::from((w[1].0 - w[0].0).get_days())).filter(|g| *g > 0).collect();
    gaps.sort_unstable();
    let gap = *gaps.get(gaps.len() / 2)?;
    let per_year = (365.25 / gap as f64).round() as i64;
    let cadence = match gap {
        7 => "every week".to_string(),
        14 => "every 2 weeks".to_string(),
        13..=17 => "twice a month".to_string(),
        28..=31 => "every month".to_string(),
        g => format!("every {g} days"),
    };
    // Runs of one amount, oldest first: (amount, first day, last day, pays).
    let mut runs: Vec<(i64, Date, Date, usize)> = Vec::new();
    for (d, a) in &pays {
        match runs.last_mut() {
            Some(r) if r.0 == *a => {
                r.2 = *d;
                r.3 += 1;
            }
            _ => runs.push((*a, *d, *d, 1)),
        }
    }
    let steady: Vec<&(i64, Date, Date, usize)> = runs.iter().filter(|r| r.3 >= 2).collect();
    let changes: Vec<String> = steady.windows(2).filter(|w| w[0].0 != w[1].0).map(|w| {
        let (was, now) = (w[0], w[1]);
        let diff = now.0 - was.0;
        let sign = if diff > 0 { "+" } else { "−" };
        format!("from {} to {} a pay ({sign}{} a pay, {sign}{} over a year of {per_year} pays) with the pay of {}; the last at {} was {}",
            m.fmt(was.0), m.fmt(now.0), m.fmt(diff.abs()), m.fmt(diff.abs() * per_year), day(now.1, today), m.fmt(was.0), day(was.2, today))
    }).collect();
    let (first, last) = (pays[0].0, pays[pays.len() - 1]);
    Some(if changes.is_empty() {
        format!("- Pay (Salary) per pay, the whole ledger: {} since {} ({cadence}, {per_year} a year): no raise or cut logged.", m.fmt(last.1), day(first, today))
    } else {
        format!("- Pay (Salary) per pay, the whole ledger ({cadence}, {per_year} pays a year): it went {}. Now {} a pay.", changes.join("; then "), m.fmt(last.1))
    })
}

/// The calendar year so far from the whole ledger, and the ledger itself: "this year" and
/// "ever" were answered from the three budget months above (bench v3, 2026-10-30).
fn this_year(o: &mut String, ledger: &Value, cash_ids: &[i64], m: &Money, today: Date) {
    let year = jiff::civil::date(today.year(), 1, 1);
    let rows: Vec<Value> = list(ledger, "transactions").iter().filter(|t| date(s(t, "date")).is_some_and(|d| d >= year && d <= today)).cloned().collect();
    let page = serde_json::json!({ "transactions": rows });
    let e = Entries::of(&page);
    if !e.rows.is_empty() {
        let _ = writeln!(o, "\nTHIS YEAR (calendar year {}, Jan 1 to today; the budget months above are other days):", today.year());
        let _ = writeln!(o, "- Spent: {} on {}. Income: {} ({}).", m.fmt(e.spent), plural(e.count("EXPENSE"), "expense", "expenses"), m.fmt(e.income), e.count("INCOME"));
        let mut months: BTreeMap<i8, i64> = BTreeMap::new();
        for t in &e.expenses {
            if let Some(d) = date(s(t, "date")) {
                *months.entry(d.month()).or_default() += int(t, "amount");
            }
        }
        let cells: Vec<String> = months.iter().map(|(mo, sum)| {
            let now = if *mo == today.month() { " so far" } else { "" };
            format!("{}{now} {}", jiff::civil::date(today.year(), *mo, 1).strftime("%B"), m.fmt(*sum))
        }).collect();
        if !cells.is_empty() {
            let _ = writeln!(o, "- Spent by calendar month: {}.", cells.join("; "));
        }
        if !e.by_category.is_empty() {
            let _ = writeln!(o, "- Spent by category, largest first: {}.", e.categories(m));
        }
        if let Some(line) = by_account(&e.expenses, m) {
            let _ = writeln!(o, "- {line}");
        }
        let cash: Vec<&Value> = e.rows.iter().filter(|t| s(t, "type") == "TRANSFER" && t["to_account_id"].as_i64().is_some_and(|id| cash_ids.contains(&id))).collect();
        if !cash.is_empty() {
            let _ = writeln!(o, "- Taken out as cash (transfers into Cash: an ATM, a \"retrait\"): {} ({}).", m.fmt(cash.iter().map(|t| int(t, "amount")).sum()), cash.len());
        }
        let _ = writeln!(o, "- One shop or one kind of thing this year: money.tx.list with its query and from {year}.");
    }
    let all = Entries::of(ledger);
    let Some(first) = all.rows.iter().filter_map(|t| date(s(t, "date"))).min() else { return };
    let loose: Vec<&&Value> = all.expenses.iter().filter(|t| s(t, "category").is_empty()).collect();
    let _ = writeln!(o, "THE WHOLE LEDGER (every entry, since {}): {}; spent {}; spent with no category: {} ({}).", day(first, today), all.counts(), m.fmt(all.spent),
        m.fmt(loose.iter().map(|t| int(t, "amount")).sum()), plural(loose.len() as i64, "expense", "expenses"));
}

/// Canada's and Québec's holidays this year, for "since Labour Day": a model put Labour Day in
/// October (bench v3, 2026-10-30).
fn holidays_line(today: Date) -> String {
    let y = today.year();
    let d = |mo: i8, day: i8| jiff::civil::date(y, mo, day);
    // The `n`th Monday of a month, from 1.
    let monday = |mo: i8, n: i16| {
        let first = d(mo, 1);
        let to_monday = (7 - i16::from(first.weekday().to_monday_zero_offset())) % 7;
        first.checked_add(jiff::Span::new().days(to_monday + 7 * (n - 1))).unwrap_or(first)
    };
    let easter = easter(y);
    let shift = |n: i64| easter.checked_add(jiff::Span::new().days(n)).unwrap_or(easter);
    // The Monday before May 25.
    let victoria = d(5, 24).checked_sub(jiff::Span::new().days(i64::from(d(5, 24).weekday().to_monday_zero_offset()))).unwrap_or(d(5, 24));
    let days = [
        ("New Year's Day", d(1, 1)), ("Good Friday", shift(-2)), ("Easter Monday", shift(1)), ("Victoria Day (Journée des patriotes)", victoria),
        ("Fête nationale (Saint-Jean)", d(6, 24)), ("Canada Day", d(7, 1)), ("Labour Day", monday(9, 1)), ("Thanksgiving", monday(10, 2)),
        ("Remembrance Day", d(11, 11)), ("Christmas", d(12, 25)), ("Boxing Day", d(12, 26)),
    ];
    let cells: Vec<String> = days.iter().map(|(name, at)| format!("{name} {}", at.strftime("%a %b %-d"))).collect();
    format!("HOLIDAYS {y} (Canada, Québec): {}.", cells.join("; "))
}

/// Easter Sunday of `year` (the Gregorian computus).
fn easter(year: i16) -> Date {
    let y = i32::from(year);
    let (a, b, c) = (y % 19, y / 100, y % 100);
    let (d, e) = (b / 4, b % 4);
    let g = (8 * b + 13) / 25;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    jiff::civil::date(year, month as i8, day as i8)
}

/// The prefetch's line on both apps: each budget month's workouts, counted over the same days
/// as Tally's month, for a question that asks for both ("last month, how much on health and how
/// many workouts"). Each of `months` is its name, its `period` and `gym.sessions` over those days.
pub fn workouts_by_budget_month(months: &[(&str, &Value, &Value)], today: Date) -> Option<String> {
    // No workouts in either: nothing imported, or a line that only says zero.
    if months.iter().all(|(_, _, sessions)| int(sessions, "total") == 0) {
        return None;
    }
    let cells: Vec<String> = months.iter().filter_map(|(name, period, sessions)| {
        let (start, last) = (date(s(period, "start"))?, date(s(period, "end_exclusive"))?.yesterday().ok()?);
        let untracked = int(sessions, "untracked");
        let excluded = if untracked > 0 { format!(", and {untracked} the person excluded in Avex (untracked, not counted)") } else { String::new() };
        Some(format!("{name} ({} to {}{}) {}{excluded}", start.strftime("%b %-d"), last.strftime("%b %-d"), if last >= today { ", so far" } else { "" },
            plural(int(sessions, "total") - untracked, "workout", "workouts")))
    }).collect();
    // Avex's calendar months sit beside it with other counts, and a small model took August's 16
    // for "last month (aug 15 to sep 14)", whose days hold 17 (bench, 2026-10-30).
    (!cells.is_empty()).then(|| format!("BOTH APPS, EACH BUDGET MONTH (Avex's workouts counted over the days of Tally's month): {}. \
        Asked how many workouts in a budget month or its days, give the count here, not a calendar month's from Avex (other days).", cells.join("; ")))
}

/// Month names in English and French, short and long. Not "may" (an English verb too, so it
/// needs "in" or "since" before it), "mar" or "sept" (French for seven).
const MONTH_WORDS: &[(&str, i8)] = &[
    ("january", 1), ("jan", 1), ("janvier", 1), ("janv", 1), ("february", 2), ("feb", 2), ("février", 2), ("fevrier", 2), ("févr", 2),
    ("march", 3), ("mars", 3), ("april", 4), ("apr", 4), ("avril", 4), ("avr", 4), ("mai", 5), ("june", 6), ("jun", 6), ("juin", 6),
    ("july", 7), ("jul", 7), ("juillet", 7), ("juil", 7), ("august", 8), ("aug", 8), ("août", 8), ("aout", 8),
    ("september", 9), ("sep", 9), ("septembre", 9), ("october", 10), ("oct", 10), ("octobre", 10),
    ("november", 11), ("nov", 11), ("novembre", 11), ("december", 12), ("dec", 12), ("décembre", 12), ("decembre", 12), ("déc", 12),
];

/// The calendar months `text` names with no day beside them ("in June", "since January"), as
/// (name, first day, last day) to read: the most recent such month, through today after "since".
/// Months that start on or after `held_from`, which the prefetch already holds, are left out.
pub fn months_named(text: &str, today: Date, held_from: Date) -> Vec<(String, Date, Date)> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let is_day = |w: Option<&&str>| w.is_some_and(|w| w.len() <= 4 && w.starts_with(|c: char| c.is_ascii_digit()) && !(w.len() == 4 && w.chars().all(|c| c.is_ascii_digit())));
    let mut out: Vec<(String, Date, Date)> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| words.get(j));
        let month = match MONTH_WORDS.iter().find(|(name, _)| name == w) {
            Some((_, n)) => *n,
            None if *w == "may" && before.is_some_and(|b| ["in", "since", "during", "of", "early", "late", "mid", "until"].contains(b)) => 5,
            None => continue,
        };
        if is_day(before) || is_day(words.get(i + 1)) {
            continue;
        }
        let year = words.get(i + 1).and_then(|y| y.parse::<i16>().ok()).filter(|y| (1990..=2100).contains(y))
            .unwrap_or(if month > today.month() { today.year() - 1 } else { today.year() });
        let Ok(first) = Date::new(year, month, 1) else { continue };
        let since = before.is_some_and(|b| ["since", "depuis"].contains(b));
        let last = if since { today } else { first.last_of_month().min(today) };
        if first >= held_from || out.iter().any(|(_, a, _)| *a == first) {
            continue;
        }
        out.push((first.strftime("%B").to_string(), first, last));
    }
    out
}

/// A tool's description for a small model, with an example call, in place of the bus's summary;
/// `None` for an op without one. `today` dates the examples.
pub fn description(op: &str, today: Date) -> Option<String> {
    Some(match op {
        "money.summary" => "Start here for any money question. This month's budget worked out in words: spent, income, left, pace, \
today, yesterday, this week and last week, every category this month and month by month, the biggest expenses, last month, \
accounts and net worth, bills to pay, goals and investments. Example: {}".into(),
        "money.tx.list" => format!("The entries of one budget month, each with its id, added up by category, by week and by day. \
period_offset: 0 this month, -1 last month, -2 the month before: \"last month\" is always period_offset -1, never from and to. \
from and to (YYYY-MM-DD, both days counted) in place of it only for other days: a calendar month they name (\"in August\"), \
since a payday, one day; from alone runs to today. query: a word to find in the note, \
category or account. Example: {{\"period_offset\": -1, \"query\": \"Provigo\"}}, or {{\"from\": \"{}-01-01\", \"to\": \"{today}\"}}", today.year()),
        "money.lists" => "The accounts and categories (each name with its id) that money.tx.add and money.tx.update need, what \
goes in each category, and the dates of the last two weeks. Example: {}".into(),
        "money.tx.add" => format!("Add an expense, income or transfer. amount is a decimal string; date is the day it happened as \
YYYY-MM-DD; account and category are names, as the person said them or as money.lists has them; a TRANSFER has \
to_account and no category. Example: \
{{\"type\": \"EXPENSE\", \"amount\": \"18.50\", \"date\": \"{today}\", \"account\": \"cash\", \"category\": \"Dining\", \"note\": \"Lunch\"}}"),
        "money.tx.update" => "Change an entry, by its id from money.tx.list; give only the fields that change, an account or \
category by name. Not to take back an entry added by mistake: that is a delete, through thread.propose. \
Example: {\"id\": 91, \"amount\": \"75.49\"}, or {\"id\": 91, \"category\": \"Groceries\"}".into(),
        "money.tx.restore" => "Bring back a deleted entry, by its id. Example: {\"id\": 91}".into(),
        "money.series" => format!("The numbers behind a money chart, with their total. Example: {{\"by\": \"period\", \"periods\": 3, \
\"category\": \"Dining\"}} gives Dining in each of the last 3 months. {SERIES_PERIODS}"),
        "money.invest.summary" => "Investments worked out: value, cost, gain, as of when, each account and holding, and \
contribution room. Example: {}".into(),
        "money.invest.list" => "Investment activities (buys, sells, dividends, deposits), newest first. Example: {}".into(),
        "money.invest.add" => format!("Record an investment activity in an investment account that Wealthsimple's files do not \
fill. Example: {{\"account_id\": 5, \"type\": \"BUY\", \"date\": \"{today}\", \"symbol\": \"XEQT\", \"quantity\": \"2\", \"amount\": \"70.00\"}}"),
        "gym.summary" => "Start here for any training question. Avex's history worked out in words: how old the copy is, totals, \
this week and last week, week by week and month by month, recent workouts and the last one in detail, every lift's best \
set (in lb and kg) and how it improved, cardio and goals. Example: {}".into(),
        "gym.sessions" => "Workouts, newest first, one line each with its id, and how many in all. lift: only workouts with \
that lift. from and to (YYYY-MM-DD, both days counted): only those days' workouts. Example: {\"lift\": \"Deadlift\", \"limit\": 5}, \
or {\"from\": \"2026-08-15\", \"to\": \"2026-09-14\"}".into(),
        "gym.session.get" => "Every set of one workout, by its id from gym.sessions or gym.summary. Example: {\"id\": 149}".into(),
        "gym.lifts" => "Every lift's best set, how often it was done, and how it improved. query: part of a lift's name. \
Example: {\"query\": \"squat\"}".into(),
        "gym.lift.get" => "One lift's history, workout by workout. Example: {\"name\": \"Bench Press\"}".into(),
        "gym.cardio" => "Every run and other cardio, with totals. Example: {}".into(),
        "gym.series" => "The numbers behind a training chart. Example: {\"measure\": \"e1rm\", \"lift\": \"Bench Press\", \
\"by\": \"week\", \"periods\": 12}".into(),
        _ => return None,
    })
}

/// What money.series' `periods` counts. Asked for "the last 2 months by week", both models in the
/// bench asked for 8 periods, counting weeks.
const SERIES_PERIODS: &str = "periods counts months (budget periods), even when by is week or day: \"the last 2 months by \
week\" is {\"by\": \"week\", \"periods\": 2}, not 8: it does not count weeks.";

/// Fields a small model is better without: a `today` it could set wrongly, and a `limit` that
/// would cut the entries a reading adds up.
pub const HIDDEN_FIELDS: &[(&str, &str)] = &[
    ("money.summary", "today"), ("money.tx.list", "today"), ("money.tx.list", "limit"), ("money.series", "today"),
    ("money.invest.summary", "today"),
];

// ── reading values ──────────────────────────────────────────────────────────────────────────

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn int(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn date(text: &str) -> Option<Date> {
    text.get(..10)?.parse().ok()
}

// ── writing values ──────────────────────────────────────────────────────────────────────────

/// `1234567` as "1,234,567".
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A currency's amounts as a person writes them.
struct Money {
    code: String,
    digits: u32,
}

impl Money {
    fn of(code: &str) -> Money {
        let code = code.trim().to_ascii_uppercase();
        let digits = relay_money::money::fraction_digits(&code).unwrap_or(2);
        Money { code, digits }
    }

    /// Minor units as "$1,234.56", "-$16.61" or "1,500 JPY".
    fn fmt(&self, minor: i64) -> String {
        self.decimal(i128::from(minor), self.digits, self.digits)
    }

    /// An integer at `places` decimals, shown with at least `min` of them.
    fn decimal(&self, value: i128, places: u32, min: u32) -> String {
        let unit = 10u128.pow(places);
        let abs = value.unsigned_abs();
        let mut frac = format!("{:0width$}", abs % unit, width = places as usize);
        while frac.len() > min as usize && frac.ends_with('0') {
            frac.pop();
        }
        let whole = u64::try_from(abs / unit).unwrap_or(u64::MAX);
        let body = if frac.is_empty() { grouped(whole) } else { format!("{}.{frac}", grouped(whole)) };
        let sign = if value < 0 { "-" } else { "" };
        match self.code.as_str() {
            "CAD" | "USD" | "AUD" | "NZD" => format!("{sign}${body}"),
            "EUR" => format!("{sign}€{body}"),
            "GBP" => format!("{sign}£{body}"),
            code => format!("{sign}{body} {code}"),
        }
    }

    /// A price at 1e-8 in this currency: "$38.12", "$0.0042".
    fn price(&self, e8: i64) -> String {
        self.decimal(i128::from(e8), 8, 2)
    }
}

/// A quantity at 1e-8: "10", "1.5".
fn quantity(e8: i64) -> String {
    let m = Money { code: "-".into(), digits: 0 };
    m.decimal(i128::from(e8), 8, 0).trim_end_matches(" -").to_string()
}

/// Basis points as "12.5%".
fn pct_bps(bps: i64) -> String {
    let m = Money { code: "-".into(), digits: 0 };
    format!("{}%", m.decimal(i128::from(bps), 2, 0).trim_end_matches(" -"))
}

/// A weight or a volume: "215", "142.5", "26,185".
fn weight(w: f64) -> String {
    let tenths = (w * 10.0).round() as i64;
    let whole = grouped(tenths.unsigned_abs() / 10);
    let sign = if tenths < 0 { "-" } else { "" };
    match tenths.unsigned_abs() % 10 {
        0 => format!("{sign}{whole}"),
        d => format!("{sign}{whole}.{d}"),
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "Thu Oct 15", with the year when it is not today's.
fn day(d: Date, today: Date) -> String {
    if d.year() == today.year() { d.strftime("%a %b %-d").to_string() } else { d.strftime("%a %b %-d %Y").to_string() }
}

fn day_of(text: &str, today: Date) -> String {
    date(text).map_or_else(|| text.to_string(), |d| day(d, today))
}

/// Days from `d` to `today`: positive in the past.
fn days_since(d: Date, today: Date) -> i64 {
    today.since(d).map_or(0, |span| i64::from(span.get_days()))
}

/// How far `d` is from today: "today", "yesterday", "3 days ago", "in 4 days".
fn ago(d: Date, today: Date) -> String {
    match days_since(d, today) {
        0 => "today".into(),
        1 => "yesterday".into(),
        -1 => "tomorrow".into(),
        n if n > 1 => format!("{n} days ago"),
        n => format!("in {} days", -n),
    }
}

fn ago_of(text: &str, today: Date) -> String {
    date(text).map_or_else(String::new, |d| ago(d, today))
}

/// "Today is Thursday, October 15, 2026 (2026-10-15)."
fn today_line(today: Date) -> String {
    format!("Today is {} ({today}).", today.strftime("%A, %B %-d, %Y"))
}

/// The last two weeks' dates, for a model writing "Wednesday" or "last Saturday" as a date.
/// A small model counts weekdays wrongly, so each name of the last week is matched to its date.
fn calendar(today: Date) -> String {
    let monday = monday(today);
    let week: Vec<String> = (0..7).filter_map(|back| {
        let d = today.checked_sub(jiff::Span::new().days(back)).ok()?;
        let name = match back {
            0 => "today ",
            1 => "yesterday ",
            _ if d < monday => "last ",
            _ => "",
        };
        Some(format!("{name}{} = {d}", d.strftime("%A %b %-d")))
    }).collect();
    format!("DATES (\"Wednesday\" or \"last Saturday\" is the most recent one): {}.", week.join("; "))
}

/// The Monday a week starts on.
fn monday(d: Date) -> Date {
    let back = i64::from(d.weekday().to_monday_zero_offset());
    d.checked_sub(jiff::Span::new().days(back)).unwrap_or(d)
}

/// A weight in the other unit: "(97.5 kg)" beside pounds, "(214.9 lb)" beside kilograms.
fn other_unit(w: f64, unit: &str) -> String {
    match unit {
        "lb" => format!(" ({} kg)", weight(w * 0.453_592_37)),
        "kg" => format!(" ({} lb)", weight(w / 0.453_592_37)),
        _ => String::new(),
    }
}

/// A budget period by name: "October 2026 (Oct 1 to Oct 31)", or its days when it is not a
/// calendar month.
fn period_name(period: &Value, today: Date) -> String {
    let (Some(start), Some(end)) = (date(s(period, "start")), date(s(period, "end_exclusive"))) else { return "this period".into() };
    let last = end.yesterday().unwrap_or(end);
    let span = format!("{} to {}", start.strftime("%b %-d"), last.strftime("%b %-d"));
    let span = if start.year() == today.year() && last.year() == today.year() { span } else { format!("{span} {}", last.year()) };
    if start.day() == 1 && last.month() == start.month() {
        format!("{} ({span})", start.strftime("%B %Y"))
    } else {
        span
    }
}

/// `TRANSFER` as "transfer", `CREDIT` as "credit card".
fn word(kind: &str) -> String {
    match kind {
        "CREDIT" => "credit card".into(),
        "NON_REGISTERED" => "non-registered".into(),
        "TFSA" | "RRSP" | "FHSA" | "RESP" | "LIRA" | "RRIF" | "ETF" => kind.into(),
        k => k.to_ascii_lowercase().replace('_', " "),
    }
}

// ── Tally ───────────────────────────────────────────────────────────────────────────────────

/// One period's entries worked out: totals, by category, the biggest and smallest expense.
struct Entries<'a> {
    rows: &'a [Value],
    spent: i64,
    income: i64,
    moved: i64,
    expenses: Vec<&'a Value>,
    /// Category name → (spent, entries), largest first.
    by_category: Vec<(String, i64, i64)>,
}

impl<'a> Entries<'a> {
    fn of(page: &'a Value) -> Entries<'a> {
        let rows = list(page, "transactions");
        let of = |kind: &str| rows.iter().filter(|t| s(t, "type") == kind).map(|t| int(t, "amount")).sum::<i64>();
        let expenses: Vec<&Value> = rows.iter().filter(|t| s(t, "type") == "EXPENSE").collect();
        let mut sums: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        for t in &expenses {
            let name = if s(t, "category").is_empty() { "No category" } else { s(t, "category") };
            let e = sums.entry(name.to_string()).or_default();
            e.0 += int(t, "amount");
            e.1 += 1;
        }
        let mut by_category: Vec<(String, i64, i64)> = sums.into_iter().map(|(k, (a, n))| (k, a, n)).collect();
        by_category.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Entries { rows, spent: of("EXPENSE"), income: of("INCOME"), moved: of("TRANSFER"), expenses, by_category }
    }

    fn count(&self, kind: &str) -> i64 {
        self.rows.iter().filter(|t| s(t, "type") == kind).count() as i64
    }

    fn counts(&self) -> String {
        format!(
            "{}: {}, {}, {}",
            plural(self.rows.len() as i64, "entry", "entries"),
            plural(self.count("EXPENSE"), "expense", "expenses"),
            plural(self.count("INCOME"), "income", "incomes"),
            plural(self.count("TRANSFER"), "transfer", "transfers")
        )
    }

    fn categories(&self, m: &Money) -> String {
        self.by_category.iter().map(|(name, amount, n)| format!("{name} {} ({n})", m.fmt(*amount))).collect::<Vec<_>>().join("; ")
    }

    /// The largest (or smallest) expense, as a phrase.
    fn extreme(&self, largest: bool, m: &Money, today: Date) -> Option<String> {
        let pick = |a: &&&Value, b: &&&Value| int(a, "amount").cmp(&int(b, "amount"));
        let t = if largest { self.expenses.iter().max_by(pick) } else { self.expenses.iter().min_by(pick) }?;
        Some(expense(t, m, today))
    }

    /// The `n` largest expenses, numbered.
    fn largest(&self, n: usize, m: &Money, today: Date) -> String {
        let mut sorted = self.expenses.clone();
        sorted.sort_by_key(|t| -int(t, "amount"));
        sorted.iter().take(n).enumerate().map(|(i, t)| format!("{}. {}", i + 1, expense(t, m, today))).collect::<Vec<_>>().join("; ")
    }
}

/// An expense as a phrase: "$1,350.00, Rent (Housing), Thu Oct 1, on Chequing". "Which card was
/// that on?" follows a biggest expense.
fn expense(t: &Value, m: &Money, today: Date) -> String {
    let mut phrase = format!("{}, {} ({}), {}", m.fmt(int(t, "amount")), entry_name(t), category_of(t), day_of(s(t, "date"), today));
    if !s(t, "account").is_empty() {
        let _ = write!(phrase, ", on {}", s(t, "account"));
    }
    phrase
}

/// What the expenses among `rows` from `from` to `to` (both included) add up to, and how many.
fn spent_between<'a>(rows: impl Iterator<Item = &'a Value>, from: Date, to: Date) -> (i64, i64) {
    rows.filter(|t| s(t, "type") == "EXPENSE" && date(s(t, "date")).is_some_and(|d| d >= from && d <= to))
        .fold((0, 0), |(sum, n), t| (sum + int(t, "amount"), n + 1))
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// What a Tally reading is, said on its first line: a model took no income logged for no income.
const TALLY_IS: &str = "Only what the person has logged in their Tally app: not their bank, not everything they did.";
/// What an Avex reading is, likewise.
const AVEX_IS: &str = "Only what their Avex export holds: not everything they did.";
/// A figure with nothing behind it, said so: "none logged in Tally", never a bare "$0.00".
const NONE_LOGGED: &str = "none logged in Tally";

/// An amount, or "none logged in Tally" for nothing.
fn logged(minor: i64, m: &Money) -> String {
    if minor == 0 { NONE_LOGGED.into() } else { m.fmt(minor) }
}

/// "$42.10 on 3 expenses", or "none logged in Tally".
fn spent_phrase((sum, n): (i64, i64), m: &Money) -> String {
    if n == 0 { NONE_LOGGED.into() } else { format!("{} on {}", m.fmt(sum), plural(n, "expense", "expenses")) }
}

fn entry_name(t: &Value) -> &str {
    let note = s(t, "note").trim();
    if note.is_empty() { "no note" } else { note }
}

fn category_of(t: &Value) -> &str {
    if s(t, "category").is_empty() { "no category" } else { s(t, "category") }
}

fn money_summary(r: &Value, pages: [Option<&Value>; 3], series: [Option<&Value>; 2], invest: Option<&Value>, lists: Option<&Value>, ledger: Option<&Value>, ctx: &Ctx) -> String {
    let today = ctx.today;
    let m = Money::of(if s(r, "currency").is_empty() { &ctx.currency } else { s(r, "currency") });
    let [this_month, last_month, _] = pages;
    // A ledger cut short would leave its oldest entries out of every whole-ledger figure.
    let ledger = ledger.filter(|l| l["more"] != true);
    let mut o = String::new();
    let _ = writeln!(o, "TALLY (budget app). {} Amounts are in {}. {TALLY_IS}", today_line(today), m.code);
    let _ = writeln!(o, "{}", calendar(today));
    let _ = writeln!(o, "{}", holidays_line(today));
    if r["empty"] == true {
        let _ = writeln!(o, "The ledger is empty: nothing has been logged in Tally yet.");
        return o;
    }
    let period = &r["period"];
    let (days, left) = (int(period, "days"), int(period, "days_left"));
    let _ = writeln!(o, "\nTHIS MONTH: {}, day {} of {days}, {} left counting today.", period_name(period, today), days - left + 1, plural(left, "day", "days"));
    let _ = writeln!(o, "- Spent so far (all expenses): {}", logged(int(r, "spent"), &m));
    let _ = writeln!(o, "- Income so far: {}", logged(int(r, "income"), &m));
    let pace = &r["pace"];
    if pace.is_object() && s(pace, "status") != "NO_BUDGET" {
        let budget = int(pace, "budget");
        let remaining = int(pace, "remaining");
        let used = num(pace, "spent_fraction") * 100.0;
        if remaining >= 0 {
            let _ = writeln!(o, "- Monthly budget: {}. Left to spend: {} ({used:.1}% of the budget used).", m.fmt(budget), m.fmt(remaining));
        } else {
            let _ = writeln!(o, "- Monthly budget: {}. OVER BUDGET by {} ({used:.1}% of the budget used).", m.fmt(budget), m.fmt(-remaining));
        }
        let delta = int(pace, "pace_delta");
        let even = format!("an even pace would be {} spent by today", m.fmt(int(pace, "expected")));
        let _ = match delta.cmp(&0) {
            std::cmp::Ordering::Greater => writeln!(o, "- Pace: {} OVER pace ({even}).", m.fmt(delta)),
            std::cmp::Ordering::Less => writeln!(o, "- Pace: {} UNDER pace ({even}).", m.fmt(-delta)),
            std::cmp::Ordering::Equal => writeln!(o, "- Pace: on pace ({even})."),
        };
        if remaining > 0 {
            let _ = writeln!(o, "- To stay within the budget: {} a day for the {} left.", m.fmt(int(pace, "daily_allowance")), plural(int(pace, "days_left"), "day", "days"));
        }
    } else {
        let _ = writeln!(o, "- No monthly budget is set.");
    }
    let elapsed = int(pace, "elapsed_days");
    let last_start = last_month.and_then(|p| date(s(&p["period"], "start")));
    match (last_month, last_start) {
        (Some(page), Some(from)) if elapsed > 0 => {
            let to = from.checked_add(jiff::Span::new().days(elapsed - 1)).unwrap_or(from);
            let (then, _) = spent_between(list(page, "transactions").iter(), from, to);
            let now = int(r, "spent");
            let diff = match now.cmp(&then) {
                std::cmp::Ordering::Less => format!("{} LESS than last month by this day", m.fmt(then - now)),
                std::cmp::Ordering::Greater => format!("{} MORE than last month by this day", m.fmt(now - then)),
                std::cmp::Ordering::Equal => "the same as last month by this day".into(),
            };
            let _ = writeln!(o, "- Compared with last month at the same day: last month had spent {} by {} (its day {elapsed}); this month {}: {diff}.", m.fmt(then), day(to, today), m.fmt(now));
        }
        _ => {
            if let Some(versus) = r["lines"]["versus_last"].as_str().filter(|l| !l.is_empty()) {
                let _ = writeln!(o, "- Compared with last month at the same day: {versus}.");
            }
        }
    }
    let offsets: Vec<String> = pages.iter().enumerate()
        .filter_map(|(i, p)| Some(format!("{} = {}", -(i as i64), date(s(&p.as_ref()?["period"], "start"))?.strftime("%B %Y"))))
        .collect();
    if !offsets.is_empty() {
        let _ = writeln!(o, "- money.tx.list's period_offset: {}.", offsets.join(", "));
    }
    // "Since my last paycheque" needs the day it came in, which the bills (the next one) do not give.
    if let Some(t) = last_pay(pages.iter().flatten().copied()) {
        let since: Vec<&Value> = pages.iter().flatten().flat_map(|p| list(p, "transactions"))
            .filter(|x| s(x, "type") == "EXPENSE" && s(x, "date") >= s(t, "date")).collect();
        let _ = writeln!(o, "- Last pay (Salary) came in on {}: {}, {}. Spent since then (that day to today): {} on {}. Its entries: money.tx.list with from {}.",
            day_of(s(t, "date"), today), entry_name(t), m.fmt(int(t, "amount")), m.fmt(since.iter().map(|x| int(x, "amount")).sum()),
            plural(since.len() as i64, "expense", "expenses"), s(t, "date"));
    }
    if let Some(line) = ledger.and_then(|p| pays_by_month(p, date(s(period, "start")), today)) {
        let _ = writeln!(o, "{line}");
    }
    if let Some(line) = ledger.and_then(|p| pay_changes(p, &m, today)) {
        let _ = writeln!(o, "{line}");
    }
    if this_month.is_some() {
        // Days and weeks cross into last month: both months' entries count.
        let rows = || pages.iter().take(2).flatten().flat_map(|p| list(p, "transactions"));
        let yesterday = today.yesterday().unwrap_or(today);
        let this_monday = monday(today);
        let last_monday = this_monday.checked_sub(jiff::Span::new().days(7)).unwrap_or(this_monday);
        let last_sunday = this_monday.yesterday().unwrap_or(this_monday);
        let _ = writeln!(o, "\nRECENT DAYS:");
        for (name, d) in [("yesterday", yesterday), ("today", today)] {
            let _ = writeln!(o, "- Spent {name}, {}: {}", d.strftime("%A %b %-d"), spent_phrase(spent_between(rows(), d, d), &m));
        }
        let _ = writeln!(o, "- This week so far ({} to today): {}", day(this_monday, today), spent_phrase(spent_between(rows(), this_monday, today), &m));
        let _ = writeln!(o, "- Last week ({} to {}): {}", day(last_monday, today), day(last_sunday, today), spent_phrase(spent_between(rows(), last_monday, last_sunday), &m));
        let weeks: Vec<String> = (2..6).filter_map(|back| {
            let start = this_monday.checked_sub(jiff::Span::new().days(7 * back)).ok()?;
            let end = start.checked_add(jiff::Span::new().days(6)).ok()?;
            Some(format!("{} to {}: {}", day(start, today), day(end, today), logged(spent_between(pages.iter().flatten().flat_map(|p| list(p, "transactions")), start, end).0, &m)))
        }).collect();
        let _ = writeln!(o, "- Weeks before (Monday to Sunday): {}.", weeks.join("; "));
    }

    let budgets = list(r, "budgets");
    if !budgets.is_empty() {
        // Over budget and over pace are different questions a small model mixes up: each gets
        // its own yes or no, and the over ones are named together first.
        let over: Vec<&str> = budgets.iter().filter(|b| int(b, "left") < 0).map(|b| s(b, "name")).collect();
        let _ = writeln!(o, "\nBUDGETS BY CATEGORY. Over budget means more spent than the budget; ahead of pace only means spending faster than the days gone, and is NOT over budget.");
        let _ = writeln!(o, "Over budget now: {}.", if over.is_empty() { "none".to_string() } else { over.join(", ") });
        for b in budgets {
            let (spent, budget, left, delta) = (int(b, "spent"), int(b, "budget"), int(b, "left"), int(b, "pace_delta"));
            let budget_q = if left < 0 { format!("Over budget? YES, by {}", m.fmt(-left)) } else { format!("Over budget? NO, {} left", m.fmt(left)) };
            let pace_q = match delta.cmp(&0) {
                std::cmp::Ordering::Greater => format!("Ahead of pace? YES, by {}", m.fmt(delta)),
                std::cmp::Ordering::Less => format!("Ahead of pace? NO, under pace by {}", m.fmt(-delta)),
                std::cmp::Ordering::Equal => "Ahead of pace? NO, on pace".into(),
            };
            let spent = if spent == 0 { format!("{NONE_LOGGED} this month, budget {}", m.fmt(budget)) } else { format!("{} of {}", m.fmt(spent), m.fmt(budget)) };
            let _ = writeln!(o, "- {}: {spent}. {budget_q}. {pace_q}.", s(b, "name"));
        }
    }

    if let Some(page) = this_month {
        let e = Entries::of(page);
        let _ = writeln!(o, "\nSPENDING BY CATEGORY THIS MONTH (every category, largest first; entries in brackets):");
        for (name, amount, n) in &e.by_category {
            let _ = write!(o, "- {name}: {} ({n})", m.fmt(*amount));
            // A total with no names leaves nothing to say about which entries are not filed.
            if name == "No category" && *n <= 5 {
                let named: Vec<String> = e.expenses.iter().filter(|t| s(t, "category").is_empty())
                    .map(|t| format!("#{} {}, {}, {}", int(t, "id"), m.fmt(int(t, "amount")), entry_name(t), day_of(s(t, "date"), today))).collect();
                let _ = write!(o, ": {}", named.join("; "));
            }
            o.push('\n');
        }
        if e.by_category.is_empty() {
            let _ = writeln!(o, "- {NONE_LOGGED} this month");
        }
        if !e.expenses.is_empty() {
            let _ = writeln!(o, "Biggest expenses this month: {}.", e.largest(3, &m, today));
        }
        if let Some(small) = e.extreme(false, &m, today) {
            let _ = writeln!(o, "Smallest expense this month: {small}.");
        }
        let _ = writeln!(o, "Entries this month: {}.", if e.rows.is_empty() { format!("{NONE_LOGGED} yet") } else { e.counts() });
        if let Some(line) = put_aside(e.rows, lists, &m) {
            let _ = writeln!(o, "{line}");
        }
    }
    if let Some(groups) = r["duplicates"].as_array() {
        // Which budget month each is in: "the ones from this month" is a budget month too.
        let starts = [(s(&r["period"], "start"), "this month"), (last_month.map_or("", |p| s(&p["period"], "start")), "last month")];
        let month_of = |day: &str| starts.iter().find(|(start, _)| !start.is_empty() && day >= *start).map_or("an earlier month", |(_, name)| *name);
        let named: Vec<String> = groups.iter().filter_map(|g| {
            let g = g.as_array().filter(|g| !g.is_empty())?;
            let ids: Vec<String> = g.iter().map(|t| format!("#{}", int(t, "id"))).collect();
            let t = &g[0];
            Some(format!("{} {}, {} {}, {}, {} ({})", ids.join(" and "), entry_name(t), m.fmt(int(t, "amount")), s(t, "type").to_ascii_lowercase(), s(t, "account"),
                day_of(s(t, "date"), today), month_of(s(t, "date"))))
        }).collect();
        if named.is_empty() {
            let _ = writeln!(o, "Possible duplicates in the whole ledger (same day, amount, account and note: maybe logged twice): none.");
        } else {
            // Every entry is already compared: a model that looks them up again day by day drops
            // the pair from a month it did not look at.
            let numbered: Vec<String> = named.iter().enumerate().map(|(i, pair)| format!("{}) {pair}", i + 1)).collect();
            let _ = writeln!(o, "Possible duplicates in the whole ledger, every entry compared (same day, amount, account and note: maybe logged twice): {}: {}.",
                plural(named.len() as i64, "pair", "pairs"), numbered.join("; "));
            let _ = writeln!(o, "Asked about duplicates, name every pair here. When they ask to remove the extra one of a pair: thread.propose deleting its newer id (money.tx.delete), never adding or restoring.");
        }
    }
    if let Some(page) = last_month {
        let e = Entries::of(page);
        let _ = writeln!(o, "\nLAST MONTH: {}.", period_name(&page["period"], today));
        let _ = writeln!(o, "- Spent: {}. Income: {}.", logged(e.spent, &m), logged(e.income, &m));
        if !e.expenses.is_empty() {
            let _ = writeln!(o, "- Biggest expenses: {}.", e.largest(3, &m, today));
        }
        if let Some(line) = put_aside(e.rows, lists, &m) {
            let _ = writeln!(o, "- {line}");
        }
        // Tally keeps one amount per budget, so a past month is read against it as it is now;
        // no tool said where last month stood, and a model did the subtraction itself, wrongly.
        let standing = |name: &str, spent: i64, budget: i64| {
            let gap = budget - spent;
            let verdict = if gap < 0 { format!("{} over budget", m.fmt(-gap)) } else { format!("{} left", m.fmt(gap)) };
            format!("{name} {} of {}, {verdict}", m.fmt(spent), m.fmt(budget))
        };
        let mut cells: Vec<String> = Vec::new();
        let overall = int(&r["pace"], "budget");
        if overall > 0 {
            cells.push(standing("all spending", e.spent, overall));
        }
        for b in list(r, "budgets").iter().filter(|b| int(b, "category_id") != 0 && int(b, "budget") > 0) {
            let spent = e.by_category.iter().find(|c| c.0 == s(b, "name")).map_or(0, |c| c.1);
            cells.push(standing(s(b, "name"), spent, int(b, "budget")));
        }
        if !cells.is_empty() {
            let _ = writeln!(o, "- Budgets last month, against each budget as it is set now: {}.", cells.join("; "));
        }
    }
    month_by_month(&mut o, pages, series, date(s(period, "start")), ledger, &m, today);
    if let Some(all) = ledger {
        let cash: Vec<i64> = list(r, "accounts").iter().filter(|a| is_cash(a)).filter_map(|a| a["id"].as_i64()).collect();
        this_year(&mut o, all, &cash, &m, today);
    }

    accounts(&mut o, list(r, "accounts"), r.get("net_worth"), &m);
    // No ids here: a model that has them adds an entry without money.lists, whose reading says
    // which account "cash" or "the Visa" is, and picks the wrong one (bench, 2026-10-15).
    if let Some(p) = invest.filter(|p| p["empty"] != true) {
        let _ = writeln!(o, "\nINVESTMENTS (money.invest.summary has the holdings, returns and room):");
        let _ = writeln!(o, "- {}", invest_headline(p, &m, today));
        by_hand(&mut o, p, lists, &m);
        holdings(&mut o, p, &m, 5);
    }

    let bills = list(r, "bills");
    let to_pay: Vec<&Value> = bills.iter().filter(|b| s(b, "type") == "EXPENSE").collect();
    if to_pay.is_empty() {
        let _ = writeln!(o, "\nBILLS TO PAY: none set up in Tally.");
    } else {
        let _ = writeln!(o, "\nBILLS TO PAY (expenses only, next first):");
        for b in &to_pay {
            let _ = writeln!(o, "- {}: {} on {} ({})", s(b, "name"), m.fmt(int(b, "amount")), day_of(s(b, "next_date"), today), ago_of(s(b, "next_date"), today));
        }
        let _ = writeln!(o, "Total of these bills: {}.", m.fmt(to_pay.iter().map(|b| int(b, "amount")).sum()));
        // "If I pay the bills still coming this month": the budget month ends on its own day, not
        // the calendar month's (a rent due on the 1st was left out).
        let ends = date(s(&r["period"], "end_exclusive"));
        let due: Vec<&&Value> = to_pay.iter().filter(|b| ends.zip(date(s(b, "next_date"))).is_some_and(|(end, d)| d < end)).collect();
        if let (Some(last), false) = (ends.and_then(|e| e.yesterday().ok()), due.is_empty()) {
            let total: i64 = due.iter().map(|b| int(b, "amount")).sum();
            let pace = &r["pace"];
            let after = if pace.is_object() && s(pace, "status") != "NO_BUDGET" {
                let left = int(pace, "remaining");
                format!(" Left to spend after them: {} − {} = {}.", m.fmt(left), m.fmt(total), m.fmt(left - total))
            } else {
                String::new()
            };
            let _ = writeln!(o, "Still to pay before this budget month ends ({}), each at its next date: {} ({}).{after}", day(last, today), m.fmt(total), plural(due.len() as i64, "bill", "bills"));
        }
    }
    let others: Vec<String> = bills.iter().filter(|b| s(b, "type") != "EXPENSE")
        .map(|b| format!("{} {} {} on {} ({})", s(b, "name"), m.fmt(int(b, "amount")), word(s(b, "type")), day_of(s(b, "next_date"), today), ago_of(s(b, "next_date"), today)))
        .collect();
    if !others.is_empty() {
        let _ = writeln!(o, "Scheduled but not bills (income and transfers): {}.", others.join("; "));
    }

    let goals = list(r, "goals");
    if goals.is_empty() {
        let _ = writeln!(o, "\nGOALS: none set up in Tally.");
    } else {
        let _ = writeln!(o, "\nGOALS:");
        for g in goals {
            let _ = writeln!(o, "- {} ({} goal): {} of {}. {}", s(g, "name"), word(s(g, "kind")), m.fmt(int(g, "saved")), m.fmt(int(g, "target")), s(g, "line"));
        }
    }
    let _ = writeln!(o, "\nSingle entries are in money.tx.list.");
    o
}

/// The last months' spending and income, and each category over the last three months. `start`
/// is this budget month's first day, `ledger` every entry.
fn month_by_month(o: &mut String, pages: [Option<&Value>; 3], series: [Option<&Value>; 2], start: Option<Date>, ledger: Option<&Value>, m: &Money, today: Date) {
    let [spending, income] = series;
    if let Some(spending) = spending {
        let labels = list(spending, "labels");
        let values = |s: Option<&Value>| s.and_then(|s| list(s, "series").first()).map(|x| list(x, "values").to_vec()).unwrap_or_default();
        let (out, inn) = (values(Some(spending)), values(income));
        // Each budget month's days, year and period_offset: "Apr 15" alone was read as 2025, and
        // a month past the three below had no offset to read it by (bench v3, 2026-10-30).
        let back = |i: usize| (labels.len() - 1 - i) as i64;
        let days = |i: usize| -> Option<(Date, Date)> {
            let first = start?.checked_sub(jiff::Span::new().months(back(i))).ok()?;
            let next = start?.checked_sub(jiff::Span::new().months(back(i) - 1)).ok()?;
            // A month from the 1st is labelled by its name ("August"), any other by its first day.
            let label = labels[i].as_str()?;
            (label == first.strftime("%b %-d").to_string() || label == first.strftime("%B").to_string()).then_some((first, next.yesterday().ok()?))
        };
        let months: Vec<String> = labels.iter().enumerate().map(|(i, label)| {
            let label = match days(i) {
                Some((a, b)) => format!("{} to {} (period_offset {})", a.strftime("%b %-d"), b.strftime("%b %-d %Y"), -back(i)),
                None => label.as_str().unwrap_or("").to_string(),
            };
            let (spent, earned) = (out.get(i).and_then(Value::as_i64).unwrap_or(0), inn.get(i).and_then(Value::as_i64).unwrap_or(0));
            let now = if i + 1 == labels.len() { " so far" } else { "" };
            if spent == 0 && earned == 0 { format!("{label}{now}: {NONE_LOGGED}") } else { format!("{label}{now}: spent {}, income {}", logged(spent, m), logged(earned, m)) }
        }).collect();
        let _ = writeln!(o, "\nMONTH BY MONTH (budget periods, oldest first): {}.", months.join("; "));
        // "What made it so expensive": the costliest month past, by its biggest expenses.
        let costliest = (0..labels.len().saturating_sub(1)).filter(|i| out.get(*i).and_then(Value::as_i64).unwrap_or(0) > 0)
            .max_by_key(|i| out.get(*i).and_then(Value::as_i64).unwrap_or(0));
        if let (Some(i), Some(all)) = (costliest, ledger) {
            if let Some((a, b)) = days(i) {
                let rows: Vec<Value> = list(all, "transactions").iter().filter(|t| date(s(t, "date")).is_some_and(|d| d >= a && d <= b)).cloned().collect();
                let page = serde_json::json!({ "transactions": rows });
                let e = Entries::of(&page);
                if !e.expenses.is_empty() {
                    let _ = writeln!(o, "The costliest of these: {} to {}, {}; its biggest expenses: {}.", a.strftime("%b %-d"), b.strftime("%b %-d %Y"), m.fmt(e.spent), e.largest(3, m, today));
                }
            }
        }
    }
    // Oldest first, as a person reads months; each a budget month, named as one ("July" was read
    // as July 1 to 31).
    let months: Vec<(String, Entries)> = pages.iter().enumerate().rev().filter_map(|(i, p)| p.map(|p| (i, p)))
        .map(|(i, p)| {
            let name = ["this month", "last month", "the month before last"][i];
            let span = match (date(s(&p["period"], "start")), date(s(&p["period"], "end_exclusive")).and_then(|d| d.yesterday().ok())) {
                (Some(a), Some(b)) => format!(" ({} to {})", a.strftime("%b %-d"), b.strftime("%b %-d")),
                _ => String::new(),
            };
            (format!("{name}{span}"), Entries::of(p))
        })
        .collect();
    let mut names: Vec<&str> = months.iter().flat_map(|(_, e)| e.by_category.iter().map(|(n, _, _)| n.as_str())).collect();
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        return;
    }
    let _ = writeln!(o, "SPENT BY CATEGORY, BUDGET MONTH BY BUDGET MONTH (oldest first):");
    let n = months.len() as i64;
    for name in names {
        let cells: Vec<String> = months.iter().enumerate().map(|(i, (month, e))| {
            let now = if i + 1 == months.len() { " so far" } else { "" };
            match e.by_category.iter().find(|(n, _, _)| n == name) {
                Some((_, amount, _)) => format!("{month}{now} {}", logged(*amount, m)),
                None => format!("{month}{now} {NONE_LOGGED}"),
            }
        }).collect();
        let total: i64 = months.iter().filter_map(|(_, e)| e.by_category.iter().find(|(n, _, _)| n == name).map(|(_, a, _)| *a)).sum();
        let average = if n > 1 { format!(", {} a month on average", m.fmt((2 * total + n) / (2 * n))) } else { String::new() };
        let _ = writeln!(o, "- {name}: {}; these {n} months together {}{average}", cells.join("; "), m.fmt(total));
    }
}

/// The accounts' lines and the net worth.
fn accounts(o: &mut String, rows: &[Value], net_worth: Option<&Value>, m: &Money) {
    let open: Vec<&Value> = rows.iter().filter(|a| a["archived"] != true).collect();
    if open.is_empty() {
        return;
    }
    let _ = writeln!(o, "\nACCOUNTS (balance now):");
    for a in &open {
        let mut kind = word(s(a, "type"));
        if let Some(reg) = a["registration"].as_str() {
            kind = format!("{kind}, {}", word(reg));
        }
        if !s(a, "institution").is_empty() {
            kind = format!("{kind}, at {}", s(a, "institution"));
        }
        let owed = if s(a, "type") == "CREDIT" && int(a, "balance") < 0 { " owed" } else { "" };
        let filled = if import_filled(a) { format!("; {FILLED}") } else { String::new() };
        let _ = writeln!(o, "- {} ({kind}): {}{owed}{filled}", s(a, "name"), m.fmt(int(a, "balance")));
    }
    if let Some(n) = net_worth.and_then(Value::as_i64) {
        let _ = writeln!(o, "- NET WORTH, all accounts together: {}", m.fmt(n));
    }
    // The summary is what a first message carries: "mets 300$ dans mon épargne" was recorded into
    // one of two savings accounts without money.lists, which says so (bench, 2026-10-30).
    same_kind(o, &open);
}

/// "My card" with two cards, or "my savings" with two savings accounts, names none of them; and
/// cash taken out is money moved, not spent.
fn same_kind(o: &mut String, open: &[&Value]) {
    let line_of_credit = |a: &&&Value| { let n = s(a, "name").to_lowercase(); n.contains("line of credit") || n.contains("marge") };
    for (kind, said) in [("CREDIT", "\"my card\", \"the card\" or \"ma carte\""), ("SAVINGS", "\"my savings\" or \"mon épargne\"")] {
        let these: Vec<&str> = open.iter().filter(|a| s(a, "type") == kind && !line_of_credit(a)).map(|a| s(a, "name")).collect();
        if these.len() > 1 {
            let _ = writeln!(o, "{} accounts of this kind: {}. When they say only {said}, it could be any of them: ask which before adding or changing anything.", these.len(), these.join(", "));
        }
    }
    // "I took 100 out at the ATM" was recorded as an expense, under Other (bench v3, 2026-10-30).
    if let Some(cash) = open.iter().find(|a| is_cash(a)) {
        let _ = writeln!(o, "Cash taken out (an ATM, a \"retrait\") is a TRANSFER from the account it came out of to {}, not an expense: what they buy with it later is.", s(cash, "name"));
    }
}

/// An account that holds cash: Tally's cash type, or one named for it that is no card.
fn is_cash(account: &Value) -> bool {
    let n = s(account, "name").to_lowercase();
    s(account, "type") == "CASH" || (s(account, "type") != "CREDIT" && (n == "cash" || n.contains("comptant")))
}

/// A range of days longer than this is added up by month, not by week and day.
const LONG_RANGE_DAYS: i64 = 62;
/// The most entries a list of them writes out one by one: a range of a year would fill a small
/// model's context with lines its figures already add up.
const LISTED_ENTRIES: usize = 80;

/// Transfers by the account they went into, and what went into savings and investment accounts
/// from the others: "how much did I put aside" is a sum a small model gets wrong from a list.
fn transfers_into(o: &mut String, rows: &[Value], lists: Option<&Value>, m: &Money) {
    let mut into: Vec<(&str, Option<i64>, i64, i64)> = Vec::new();
    for t in rows.iter().filter(|t| s(t, "type") == "TRANSFER") {
        let to = t["to_account_id"].as_i64();
        match into.iter_mut().find(|c| c.1 == to && c.0 == s(t, "to_account")) {
            Some(c) => {
                c.2 += int(t, "amount");
                c.3 += 1;
            }
            None => into.push((s(t, "to_account"), to, int(t, "amount"), 1)),
        }
    }
    into.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
    let cells: Vec<String> = into.iter().map(|(name, _, amount, n)| format!("{name} {} ({n})", m.fmt(*amount))).collect();
    let _ = writeln!(o, "- Transfers by the account they went into, largest first: {}.", cells.join("; "));
    if let Some(line) = put_aside(rows, lists, m) {
        let _ = writeln!(o, "- {line}");
    }
}

/// What `rows` moved into savings and investment accounts from the other accounts, in words.
fn put_aside(rows: &[Value], lists: Option<&Value>, m: &Money) -> Option<String> {
    let lists = lists?;
    let kind = |id: Option<i64>| list(lists, "accounts").iter().find(|a| id.is_some() && a["id"].as_i64() == id).map(|a| s(a, "type"));
    let aside = |id: Option<i64>| matches!(kind(id), Some("SAVINGS" | "INVESTMENT"));
    let saved: Vec<&Value> = rows.iter().filter(|t| s(t, "type") == "TRANSFER" && aside(t["to_account_id"].as_i64()) && !aside(t["account_id"].as_i64())).collect();
    if saved.is_empty() {
        return None;
    }
    let mut by: Vec<(&str, i64, i64)> = Vec::new();
    for t in &saved {
        match by.iter_mut().find(|b| b.0 == s(t, "to_account")) {
            Some(b) => {
                b.1 += int(t, "amount");
                b.2 += 1;
            }
            None => by.push((s(t, "to_account"), int(t, "amount"), 1)),
        }
    }
    by.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let named: Vec<String> = by.iter().map(|(name, amount, n)| format!("{name} {} ({n})", m.fmt(*amount))).collect();
    Some(format!("Put into savings and investment accounts (from the other accounts): {} in all ({}): {}.",
        m.fmt(saved.iter().map(|t| int(t, "amount")).sum()), saved.len(), named.join("; ")))
}

/// Words a search for money back is written with, in English and French.
const MONEY_BACK_WORDS: &[&str] = &["refund", "return", "rembours", "retour", "money back", "reimburs", "cashback", "remise"];

/// A list searched for money back: by one of [`MONEY_BACK_WORDS`], or the Refunds category.
fn money_back_search(payload: &Value, result: &Value) -> bool {
    let query = payload["query"].as_str().unwrap_or("").to_lowercase();
    MONEY_BACK_WORDS.iter().any(|w| query.contains(w))
        || (payload["category_id"].is_i64() && list(result, "transactions").iter().any(|t| s(t, "category").eq_ignore_ascii_case("Refunds")))
}

/// Money back among `rows`: income in Tally's Refunds category, or filed under a spending one
/// (a return filed where it was bought does not lower that category's spending).
fn money_back<'a>(rows: &'a [Value], lists: Option<&Value>) -> Vec<&'a Value> {
    let spending: Vec<&str> = lists.map_or_else(Vec::new, |l| list(l, "categories").iter().filter(|c| s(c, "kind") == "EXPENSE").map(|c| s(c, "name")).collect());
    rows.iter().filter(|t| s(t, "type") == "INCOME" && (spending.contains(&s(t, "category")) || s(t, "category").eq_ignore_ascii_case("Refunds"))).collect()
}

/// "$190.40 in all, 3 entries: #674 Remboursement Simons, $59.00 (Shopping), Wed Sep 16; …".
fn money_back_entries(back: &[&Value], m: &Money, today: Date) -> String {
    let named: Vec<String> = back.iter().map(|t| format!("#{} {}, {} ({}), {}", int(t, "id"), entry_name(t), m.fmt(int(t, "amount")), category_of(t), day_of(s(t, "date"), today))).collect();
    format!("{} in all, {}: {}", m.fmt(back.iter().map(|t| int(t, "amount")).sum()), plural(back.len() as i64, "entry", "entries"), named.join("; "))
}

/// "What did I pay cash for", "how much went on the Visa": added up, not left to the model.
fn by_account(expenses: &[&Value], m: &Money) -> Option<String> {
    let mut sums: Vec<(&str, i64, i64)> = Vec::new();
    for t in expenses {
        match sums.iter_mut().find(|a| a.0 == s(t, "account")) {
            Some(a) => {
                a.1 += int(t, "amount");
                a.2 += 1;
            }
            None => sums.push((s(t, "account"), int(t, "amount"), 1)),
        }
    }
    if sums.len() < 2 {
        return None;
    }
    sums.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let cells: Vec<String> = sums.iter().map(|(name, amount, n)| format!("{name} {} ({n})", m.fmt(*amount))).collect();
    Some(format!("Spent by the account it was paid from, largest first: {}.", cells.join("; ")))
}

/// A search asked with no days, which Tally reads over this budget month alone.
fn ledger_search(payload: &Value) -> bool {
    payload["query"].as_str().is_some_and(|q| !q.trim().is_empty()) && ["from", "to", "period_offset"].iter().all(|k| payload[*k].is_null())
}

/// What a search matches over the whole ledger, added up by type for all of it and for this
/// calendar year, with the newest entries: the budget month searched may hold none of them.
fn ledger_matches(o: &mut String, query: &str, page: &Value, m: &Money, today: Date) {
    let rows = list(page, "transactions");
    if rows.is_empty() {
        let _ = writeln!(o, "\nIN THE WHOLE LEDGER (every entry ever, not only this budget month): no entry matches \"{query}\" either. Notes are as the person typed them, often in French \
            (\"intérêts\", \"retrait\", \"loyer\", \"épicerie\") or a shop's name: search again with another word, or read the entries with no query.");
        return;
    }
    let year = jiff::civil::date(today.year(), 1, 1);
    let e = Entries::of(page);
    let _ = writeln!(o, "\nIN THE WHOLE LEDGER (every entry ever, not only this budget month), matching \"{query}\": {}.", e.counts());
    for (kind, name) in [("EXPENSE", "Spent"), ("INCOME", "Income"), ("TRANSFER", "Moved between accounts (transfers, not spending)")] {
        let these: Vec<&Value> = rows.iter().filter(|t| s(t, "type") == kind).collect();
        if these.is_empty() {
            continue;
        }
        let now: Vec<&&Value> = these.iter().filter(|t| date(s(t, "date")).is_some_and(|d| d >= year)).collect();
        let _ = writeln!(o, "- {name}: {} ({}); this calendar year ({}, Jan 1 to today): {} ({}).", m.fmt(these.iter().map(|t| int(t, "amount")).sum()), these.len(),
            today.year(), m.fmt(now.iter().map(|t| int(t, "amount")).sum()), now.len());
    }
    let mut years: BTreeMap<i16, (i64, i64)> = BTreeMap::new();
    for t in &e.expenses {
        if let Some(d) = date(s(t, "date")) {
            let y = years.entry(d.year()).or_default();
            y.0 += int(t, "amount");
            y.1 += 1;
        }
    }
    if years.len() > 1 {
        let cells: Vec<String> = years.iter().map(|(y, (sum, n))| format!("{y} {} ({n})", m.fmt(*sum))).collect();
        let _ = writeln!(o, "- Spent by calendar year: {}.", cells.join("; "));
    }
    let shown = rows.len().min(LEDGER_MATCHES);
    let _ = writeln!(o, "{} (#id, day, note, amount, category, account):", if shown < rows.len() { format!("The newest {shown} of them") } else { "Each of them, newest first".into() });
    for t in &rows[..shown] {
        let _ = writeln!(o, "{}", entry_line(t, m, today));
    }
}

/// The most workouts a list of them writes out one by one.
const LISTED_WORKOUTS: usize = 50;

/// The most entries of a whole-ledger search listed one by one.
const LEDGER_MATCHES: usize = 12;

/// "- #685 Fri Sep 18: Café Olimpico, $5.75 expense, Dining, Cash".
fn entry_line(t: &Value, m: &Money, today: Date) -> String {
    let amount = m.fmt(int(t, "amount"));
    let what = match s(t, "type") {
        "TRANSFER" => format!("{amount} transfer, {} to {}", s(t, "account"), s(t, "to_account")),
        "INCOME" => format!("{amount} income, {}, {}", category_of(t), s(t, "account")),
        _ => format!("{amount} expense, {}, {}", category_of(t), s(t, "account")),
    };
    format!("- #{} {}: {}, {what}", int(t, "id"), day_of(s(t, "date"), today), entry_name(t))
}

fn tx_list(payload: &Value, r: &Value, lists: Option<&Value>, unsearched: Option<&Value>, ledger: Option<&Value>, ctx: &Ctx) -> String {
    let today = ctx.today;
    let m = Money::of(&ctx.currency);
    let offset = payload["period_offset"].as_i64().unwrap_or(0);
    let ranged = ["from", "to"].iter().any(|k| payload[*k].is_string());
    let which = match offset {
        _ if ranged => "the days asked for, not a budget month".to_string(),
        0 => "this month".to_string(),
        -1 => "last month".to_string(),
        n if n < 0 => format!("{} months ago", -n),
        n => format!("{n} months ahead"),
    };
    let mut o = String::new();
    let _ = writeln!(o, "TALLY ENTRIES for {}, {which}. {} Amounts are in {}. {TALLY_IS}", period_name(&r["period"], today), today_line(today), m.code);
    let mut filters = Vec::new();
    if let Some(q) = payload["query"].as_str().filter(|q| !q.trim().is_empty()) {
        filters.push(format!("matching \"{}\"", q.trim()));
    }
    if let Some(id) = payload["category_id"].as_i64() {
        let name = list(r, "transactions").iter().find(|t| t["category_id"].as_i64() == Some(id)).map(category_of);
        filters.push(format!("category {}", name.map_or_else(|| format!("#{id}"), str::to_string)));
    }
    if let Some(id) = payload["account_id"].as_i64() {
        let name = list(r, "transactions").iter().find_map(|t| match (t["account_id"].as_i64(), t["to_account_id"].as_i64()) {
            (Some(a), _) if a == id => Some(s(t, "account")),
            (_, Some(a)) if a == id => Some(s(t, "to_account")),
            _ => None,
        });
        filters.push(format!("account {}", name.map_or_else(|| format!("#{id}"), str::to_string)));
    }
    if !filters.is_empty() {
        let _ = writeln!(o, "Only entries {}.", filters.join(", "));
    }
    if let Some(all) = unsearched.filter(|_| money_back_search(payload, r)) {
        // What a search for "refund" misses: "Retour IKEA", a return filed under Shopping.
        let back = money_back(list(all, "transactions"), lists);
        let _ = writeln!(o, "- Money back over these days, whatever the search matched (refunds and returns: income in Refunds, or filed under a spending category, noted in any language): {}.",
            if back.is_empty() { format!("none: {NONE_LOGGED}") } else { money_back_entries(&back, &m, today) });
    }
    let e = Entries::of(r);
    let query = payload["query"].as_str().unwrap_or("").trim();
    if e.rows.is_empty() {
        let _ = writeln!(o, "No entries: {NONE_LOGGED} in {}{}.", period_name(&r["period"], today), if filters.is_empty() { "" } else { " that matches" });
        if let Some(all) = ledger {
            ledger_matches(&mut o, query, all, &m, today);
        }
        return o;
    }
    let _ = writeln!(o, "{}.", e.counts());
    let _ = writeln!(o, "- Spent (all expenses added up): {}", logged(e.spent, &m));
    let _ = writeln!(o, "- Income: {}", logged(e.income, &m));
    if e.moved != 0 {
        let _ = writeln!(o, "- Moved between accounts (transfers, not spending): {}", m.fmt(e.moved));
        transfers_into(&mut o, e.rows, lists, &m);
    }
    if !e.by_category.is_empty() {
        let _ = writeln!(o, "- Spent by category, largest first: {}.", e.categories(&m));
    }
    if let Some(line) = by_account(&e.expenses, &m) {
        let _ = writeln!(o, "- {line}");
    }
    let incomes: Vec<&Value> = e.rows.iter().filter(|t| s(t, "type") == "INCOME").collect();
    if !incomes.is_empty() {
        let mut sums: Vec<(&str, i64, i64)> = Vec::new();
        for t in &incomes {
            match sums.iter_mut().find(|c| c.0 == category_of(t)) {
                Some(c) => {
                    c.1 += int(t, "amount");
                    c.2 += 1;
                }
                None => sums.push((category_of(t), int(t, "amount"), 1)),
            }
        }
        sums.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let cells: Vec<String> = sums.iter().map(|(name, amount, n)| format!("{name} {} ({n})", m.fmt(*amount))).collect();
        let _ = writeln!(o, "- Income by category, largest first: {}.", cells.join("; "));
        // A return filed under the category it was bought in is income there: it does not lower
        // that category's spending, and a model asked for refunds never looked for it. Tally's own
        // Refunds category is money back too: together, what came back.
        let back = money_back(e.rows, lists);
        if back.iter().any(|t| !s(t, "category").eq_ignore_ascii_case("Refunds")) {
            let _ = writeln!(o, "- Money back (refunds and returns: income in Refunds, or filed under a spending category, which does not lower its spending): {}.", money_back_entries(&back, &m, today));
        }
    }
    if !e.expenses.is_empty() {
        let _ = writeln!(o, "- Biggest expenses: {}.", e.largest(3, &m, today));
    }
    if e.expenses.len() > 1 {
        if let Some(small) = e.extreme(false, &m, today) {
            let _ = writeln!(o, "- Smallest expense: {small}.");
        }
    }
    // Weeks run Monday to Sunday, cut to the period.
    let mut weeks: Vec<(Date, i64, i64)> = Vec::new();
    let mut days: Vec<(Date, i64, i64)> = Vec::new();
    for t in &e.expenses {
        let Some(d) = date(s(t, "date")) else { continue };
        for (key, into) in [(monday(d), &mut weeks), (d, &mut days)] {
            match into.iter_mut().find(|(k, _, _)| *k == key) {
                Some(row) => {
                    row.1 += int(t, "amount");
                    row.2 += 1;
                }
                None => into.push((key, int(t, "amount"), 1)),
            }
        }
    }
    weeks.sort_by_key(|w| w.0);
    days.sort_by_key(|d| std::cmp::Reverse(d.0));
    // Over a long range of days, weeks and days run to hundreds of figures: months instead.
    let span = match (date(s(&r["period"], "start")), date(s(&r["period"], "end_exclusive"))) {
        (Some(a), Some(b)) => i64::from((b - a).get_days()),
        _ => 0,
    };
    if span > LONG_RANGE_DAYS {
        let mut months: BTreeMap<(i16, i8), (i64, i64)> = BTreeMap::new();
        for (d, sum, n) in &days {
            let row = months.entry((d.year(), d.month())).or_default();
            row.0 += sum;
            row.1 += n;
        }
        let shown: Vec<String> = months.iter().map(|((y, mo), (sum, n))| {
            let first = jiff::civil::date(*y, *mo, 1);
            format!("{} {} ({n})", if *y == today.year() { first.strftime("%B").to_string() } else { first.strftime("%B %Y").to_string() }, m.fmt(*sum))
        }).collect();
        if !shown.is_empty() {
            let _ = writeln!(o, "- Spent by calendar month, oldest first (cut to the days asked for): {}.", shown.join("; "));
        }
        // Each month's income by category: "which months had 3 paycheques" counts Salary alone.
        // Month → (income, entries, each category's (name, income, entries)).
        type Month<'a> = (i64, i64, Vec<(&'a str, i64, i64)>);
        let mut income: BTreeMap<(i16, i8), Month> = BTreeMap::new();
        for t in e.rows.iter().filter(|t| s(t, "type") == "INCOME") {
            let Some(d) = date(s(t, "date")) else { continue };
            let row = income.entry((d.year(), d.month())).or_default();
            row.0 += int(t, "amount");
            row.1 += 1;
            match row.2.iter_mut().find(|c| c.0 == category_of(t)) {
                Some(c) => {
                    c.1 += int(t, "amount");
                    c.2 += 1;
                }
                None => row.2.push((category_of(t), int(t, "amount"), 1)),
            }
        }
        let shown: Vec<String> = income.iter_mut().map(|((y, mo), (sum, n, cats))| {
            let first = jiff::civil::date(*y, *mo, 1);
            cats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let cats: Vec<String> = cats.iter().map(|(name, amount, k)| format!("{name} {} ({k})", m.fmt(*amount))).collect();
            format!("{} {} ({n}: {})", if *y == today.year() { first.strftime("%B").to_string() } else { first.strftime("%B %Y").to_string() }, m.fmt(*sum), cats.join(", "))
        }).collect();
        if !shown.is_empty() {
            let _ = writeln!(o, "- Income by calendar month, oldest first (entries in brackets, then each category's): {}.", shown.join("; "));
        }
        weeks.clear();
        days.clear();
    }
    if !weeks.is_empty() {
        let shown: Vec<String> = weeks.iter().map(|(start, sum, n)| {
            let end = start.checked_add(jiff::Span::new().days(6)).unwrap_or(*start);
            format!("{} to {}: {} ({n})", day(*start, today), day(end, today), m.fmt(*sum))
        }).collect();
        let _ = writeln!(o, "- Spent by week (Monday to Sunday): {}.", shown.join("; "));
    }
    if !days.is_empty() {
        let shown: Vec<String> = days.iter().map(|(d, sum, n)| format!("{} {} ({n})", day(*d, today), m.fmt(*sum))).collect();
        let _ = writeln!(o, "- Spent by day, newest first (days with no expense left out): {}.", shown.join("; "));
    }
    let shown = e.rows.len().min(LISTED_ENTRIES);
    if shown < e.rows.len() {
        let _ = writeln!(o, "\nENTRIES, the newest {shown} of {} (every figure above counts them all; a query, a category_id or fewer days lists the others) (#id, day, note, amount, category, account):", e.rows.len());
    } else {
        let _ = writeln!(o, "\nENTRIES, newest first (#id, day, note, amount, category, account):");
    }
    for t in &e.rows[..shown] {
        let _ = writeln!(o, "{}", entry_line(t, &m, today));
    }
    if let Some(all) = ledger {
        ledger_matches(&mut o, query, all, &m, today);
    }
    o
}

/// What goes in each of Tally's default categories (`relay_money::model::DEFAULT_CATEGORIES`), for
/// a model filing an entry: it put coffee under Subscriptions and a Bixi ride under Travel. A
/// category the person named themselves gets no examples.
const CATEGORY_EXAMPLES: &[(&str, &str)] = &[
    ("Groceries", "supermarket, grocery store, food to cook at home"),
    ("Dining", "restaurants, cafés, coffee, lunch, takeout, bars"),
    ("Transport", "transit, bus, metro, Bixi, taxi, Uber, gas, parking"),
    ("Housing", "rent, mortgage, condo fees, home insurance"),
    ("Utilities", "hydro, electricity, heating, water"),
    ("Phone & internet", "phone plan, internet"),
    ("Shopping", "clothes, electronics, household things, online orders"),
    ("Health", "pharmacy, doctor, dentist, physio, glasses"),
    ("Entertainment", "movies, concerts, shows, games, outings"),
    ("Subscriptions", "streaming and app plans, memberships billed every month"),
    ("Travel", "flights, hotels, trips away from home"),
    ("Gifts", "presents, donations"),
    ("Other", "only what fits no other category"),
    ("Salary", "pay from an employer"),
    ("Side income", "freelance work, selling things"),
    ("Refunds", "money back for a return or a reimbursement"),
    ("Other income", "only income that fits no other category"),
];

/// What goes in default category `name`, without an example another of `names` is named for:
/// beside the person's own Freelance, Side income no longer claims "freelance work" (a model
/// filed every invoice there).
fn category_examples(name: &str, names: &[&str]) -> Option<String> {
    let examples = CATEGORY_EXAMPLES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name.trim())).map(|(_, e)| *e)?;
    let others: Vec<String> = names.iter().filter(|n| !n.trim().eq_ignore_ascii_case(name.trim())).map(|n| n.trim().to_lowercase()).collect();
    let kept: Vec<&str> = examples.split(", ")
        .filter(|e| !e.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|word| others.iter().any(|o| o == word)))
        .collect();
    (!kept.is_empty()).then(|| kept.join(", "))
}

/// An account Wealthsimple's files fill: its activity comes with the next export, and the engine
/// refuses a thread's money.invest.add into it (`money.import_filled`).
fn import_filled(a: &Value) -> bool {
    s(a, "institution").trim().eq_ignore_ascii_case("wealthsimple")
}

const FILLED: &str = "filled from Wealthsimple's files: don't record activity here, the next export brings it";

/// The newest notes filed in a category, the same words once, in `pages` (newest first).
const CATEGORY_NOTES: usize = 3;

/// `text` cut to `n` characters, with an ellipsis when it was longer.
fn clip(text: &str, n: usize) -> String {
    match text.char_indices().nth(n) {
        Some((at, _)) => format!("{}…", text[..at].trim_end()),
        None => text.to_string(),
    }
}

fn category_notes<'a>(pages: &[Option<&'a Value>], id: i64) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for t in pages.iter().flatten().flat_map(|p| list(p, "transactions")) {
        let note = s(t, "note").trim();
        if t["category_id"].as_i64() == Some(id) && !note.is_empty() && !out.iter().any(|n| n.eq_ignore_ascii_case(note)) {
            out.push(note);
            if out.len() == CATEGORY_NOTES {
                break;
            }
        }
    }
    out
}

fn money_lists(r: &Value, pages: [Option<&Value>; 3], ctx: &Ctx) -> String {
    let m = Money::of(if s(r, "currency").is_empty() { &ctx.currency } else { s(r, "currency") });
    let mut o = String::new();
    let _ = writeln!(o, "TALLY ACCOUNTS AND CATEGORIES (as set up in their Tally app) for money.tx.add and money.tx.update: each name, with its id beside it. Amounts are in {}.", m.code);
    let _ = writeln!(o, "{} {}", today_line(ctx.today), calendar(ctx.today));
    let _ = writeln!(o, "\nACCOUNTS (name (account_id): type, balance now). Use the one they name: cash is the cash account; a card or Visa is a credit card; chequing or debit is chequing:");
    let open: Vec<&Value> = list(r, "accounts").iter().filter(|a| a["archived"] != true).collect();
    for a in &open {
        let filled = if import_filled(a) { format!("; {FILLED}") } else { String::new() };
        let _ = writeln!(o, "- {} (account_id {}): {}, balance {}{filled}", s(a, "name"), int(a, "id"), word(s(a, "type")), m.fmt(int(a, "balance")));
    }
    same_kind(&mut o, &open);
    if let Some(n) = r["net_worth"].as_i64() {
        let _ = writeln!(o, "NET WORTH, all accounts together: {}", m.fmt(n));
    }
    let names: Vec<&str> = list(r, "categories").iter().filter(|c| c["archived"] != true).map(|c| s(c, "name")).collect();
    for (kind, title) in [("EXPENSE", "EXPENSE CATEGORIES"), ("INCOME", "INCOME CATEGORIES")] {
        let rows: Vec<String> = list(r, "categories").iter()
            .filter(|c| s(c, "kind") == kind && c["archived"] != true)
            .map(|c| match category_examples(s(c, "name"), &names) {
                Some(e) => format!("- {} (category_id {}): {e}", s(c, "name"), int(c, "id")),
                // Not one of Tally's own: the person made it, for what its name says.
                None if CATEGORY_EXAMPLES.iter().all(|(n, _)| !n.eq_ignore_ascii_case(s(c, "name").trim())) =>
                    format!("- {} (category_id {}): a category the person made, for what its name says", s(c, "name"), int(c, "id")),
                None => format!("- {} (category_id {})", s(c, "name"), int(c, "id")),
            })
            .zip(list(r, "categories").iter().filter(|c| s(c, "kind") == kind && c["archived"] != true))
            .map(|(row, c)| {
                let notes: Vec<String> = category_notes(&pages, int(c, "id")).iter().map(|n| format!("\"{}\"", clip(n, 48))).collect();
                if notes.is_empty() { row } else { format!("{row}; lately: {}", notes.join(", ")) }
            })
            .collect();
        if !rows.is_empty() {
            let _ = writeln!(o, "\n{title} (name (category_id): what goes there):\n{}", rows.join("\n"));
        }
    }
    o
}

fn money_series(payload: &Value, r: &Value, ctx: &Ctx) -> String {
    let m = Money::of(if s(r, "currency").is_empty() { &ctx.currency } else { s(r, "currency") });
    let labels: Vec<&str> = list(r, "labels").iter().map(|l| l.as_str().unwrap_or("")).collect();
    let mut o = String::new();
    let _ = writeln!(o, "TALLY {} by {} (chart numbers). Amounts are in {}. {TALLY_IS}", s(r, "measure").to_ascii_uppercase(), s(r, "by"), m.code);
    for series in list(r, "series") {
        let values: Vec<String> = list(series, "values").iter().zip(&labels)
            .map(|(v, l)| format!("{l} {}", logged(v.as_i64().unwrap_or(0), &m)))
            .collect();
        let total: i64 = list(series, "values").iter().filter_map(Value::as_i64).sum();
        let _ = writeln!(o, "- {}: {}. Total {}.", s(series, "name"), values.join("; "), logged(total, &m));
    }
    let mut query = payload.clone();
    if let Some(q) = query.as_object_mut() {
        q.remove("today");
    }
    let kind = if payload["cumulative"] == true { "line" } else if s(r, "by") == "category" && list(r, "series").len() == 1 { "donut" } else { "bar" };
    let what = match payload["category"].as_str() { Some(c) => format!("{c} {}", s(r, "measure")), None => capitalised(s(r, "measure")) };
    let _ = writeln!(o, "\n{SERIES_PERIODS}");
    chart_block(&mut o, serde_json::json!({"type": kind, "title": format!("{what} by {}", s(r, "by")), "query": query}));
    o
}

/// The chart block that draws a series, for the model to copy as it is.
fn chart_block(o: &mut String, chart: Value) {
    let _ = writeln!(o, "\nTo show this as a chart, write this block as it is:\n```chart\n{chart}\n```");
}

/// The investment accounts valued by hand (no holdings, cash or cost): each is worth its balance,
/// which the portfolio's value leaves out; and the two together.
fn by_hand(o: &mut String, portfolio: &Value, lists: Option<&Value>, m: &Money) {
    let Some(lists) = lists else { return };
    let balance = |id: i64| list(lists, "accounts").iter().find(|a| a["id"].as_i64() == Some(id)).map(|a| int(a, "balance"));
    let hand: Vec<(&str, i64)> = list(portfolio, "accounts").iter()
        .filter(|a| int(a, "holdings") == 0 && int(a, "book") == 0 && int(a, "cash") == 0)
        .filter_map(|a| Some((s(a, "name"), balance(a["id"].as_i64()?)?)))
        .collect();
    if hand.is_empty() {
        return;
    }
    let names: Vec<String> = hand.iter().map(|(name, b)| format!("{name} {}", m.fmt(*b))).collect();
    let total = int(portfolio, "value") + hand.iter().map(|(_, b)| b).sum::<i64>();
    let _ = writeln!(o, "- Valued by hand, at their balance (not in the figures above): {}.", names.join("; "));
    let _ = writeln!(o, "- All investment accounts together, those included: {}.", m.fmt(total));
}

/// The portfolio's holdings, largest first, at most `n`.
fn holdings(o: &mut String, r: &Value, m: &Money, n: usize) {
    let mut sorted: Vec<&Value> = list(r, "holdings").iter().collect();
    sorted.sort_by_key(|h| -int(h, "value"));
    for h in sorted.into_iter().take(n) {
        let own = Money::of(s(h, "currency"));
        let gain = h["gain_bps"].as_i64().map(|b| format!(" ({})", pct_bps(b))).unwrap_or_default();
        let share = h["weight_bps"].as_i64().map(|b| format!(", {} of the portfolio", pct_bps(b))).unwrap_or_default();
        let mut line = format!(
            "- {} ({}, {}) in {}: {} {} at {} = {}{share}; cost {}, gain {}{gain}",
            s(h, "symbol"), s(h, "name"), word(s(h, "kind")), s(h, "account"), quantity(int(h, "quantity")),
            if int(h, "quantity") == 100_000_000 { "unit" } else { "units" }, own.price(int(h, "price")),
            m.fmt(int(h, "value")), m.fmt(int(h, "book")), m.fmt(int(h, "gain"))
        );
        if h["no_price"] == true {
            line.push_str("; no price, counted at its cost");
        }
        if h["fx_estimated"] == true {
            line.push_str("; converted at an estimated exchange rate, so this value is an ESTIMATE");
        }
        let _ = writeln!(o, "{line}");
    }
}

/// The portfolio in one line: its value, as of when, and its gain.
fn invest_headline(r: &Value, m: &Money, today: Date) -> String {
    let gain = r["gain_bps"].as_i64().map(|b| format!(" ({})", pct_bps(b))).unwrap_or_default();
    let mut line = format!("Worth {} in all, cost {}, gain {}{gain}", m.fmt(int(r, "value")), m.fmt(int(r, "book")), m.fmt(int(r, "gain")));
    if let Some(as_of) = date(s(r, "as_of")) {
        let _ = write!(line, "; values as of {} ({})", day(as_of, today), ago(as_of, today));
        if days_since(as_of, today) > 31 {
            line.push_str(", more than a month old: say so, and offer a fresh holdings report");
        }
    }
    line
}

fn invest_summary(r: &Value, lists: Option<&Value>, ctx: &Ctx) -> String {
    let today = ctx.today;
    let m = Money::of(if s(r, "currency").is_empty() { &ctx.currency } else { s(r, "currency") });
    let mut o = String::new();
    let _ = writeln!(o, "TALLY INVESTMENTS. {} Amounts are in {}. {TALLY_IS}", today_line(today), m.code);
    if r["empty"] == true {
        let _ = writeln!(o, "No investment account has holdings or activity logged in Tally yet.");
        return o;
    }
    if let Some(as_of) = date(s(r, "as_of")) {
        let old = days_since(as_of, today) > 31;
        let note = if old { " These values are more than a month old: offer a fresh holdings report." } else { "" };
        let _ = writeln!(o, "Values are as of {} ({}).{note}", day(as_of, today), ago(as_of, today));
    }
    let gain_pct = r["gain_bps"].as_i64().map(|b| format!(" ({})", pct_bps(b))).unwrap_or_default();
    let _ = writeln!(o, "- Total value: {}", m.fmt(int(r, "value")));
    let _ = writeln!(o, "- Cost (book value): {}", m.fmt(int(r, "book")));
    let _ = writeln!(o, "- Gain: {}{gain_pct}", m.fmt(int(r, "gain")));
    let _ = writeln!(o, "- Cash: {}", m.fmt(int(r, "cash")));
    let _ = writeln!(o, "- Income (dividends, interest) in the last 12 months: {}", logged(int(r, "income_12m"), &m));
    for issue in list(r, "issues").iter().filter_map(Value::as_str) {
        let _ = writeln!(o, "- Note: {issue}");
    }
    by_hand(&mut o, r, lists, &m);

    let _ = writeln!(o, "\nACCOUNTS:");
    for a in list(r, "accounts") {
        let mut line = format!("- {}", s(a, "name"));
        let mut about = Vec::new();
        if let Some(reg) = a["registration"].as_str() {
            about.push(word(reg));
        }
        if !s(a, "institution").is_empty() {
            about.push(s(a, "institution").to_string());
        }
        if !about.is_empty() {
            line = format!("{line} ({})", about.join(", "));
        }
        let gain = a["gain_bps"].as_i64().map(|b| format!(" ({})", pct_bps(b))).unwrap_or_default();
        let _ = write!(line, ": value {}, cost {}, gain {}{gain}, cash {}, {}", m.fmt(int(a, "value")), m.fmt(int(a, "book")), m.fmt(int(a, "gain")), m.fmt(int(a, "cash")), plural(int(a, "holdings"), "holding", "holdings"));
        if let Some(on) = a["valued_on"].as_str() {
            let _ = write!(line, ", valued on {}", day_of(on, today));
        }
        match a["return_bps"].as_i64() {
            Some(b) if a["return_annual"] == true => {
                let _ = write!(line, "; money-weighted return {} a year", pct_bps(b));
            }
            Some(b) => {
                let since = a["return_since"].as_str().map(|d| format!(" since {}", day_of(d, today))).unwrap_or_default();
                let _ = write!(line, "; money-weighted return {}{since}", pct_bps(b));
            }
            None => {}
        }
        if int(a, "holdings") == 0 && int(a, "book") == 0 && int(a, "cash") == 0 {
            line.push_str("; valued by hand: its balance in money.lists is what it is worth");
        }
        if import_filled(a) {
            let _ = write!(line, "; {FILLED}");
        }
        let _ = writeln!(o, "{line}");
    }

    if !list(r, "holdings").is_empty() {
        let _ = writeln!(o, "\nHOLDINGS (largest first):");
        holdings(&mut o, r, &m, usize::MAX);
    }
    let shares = |key: &str, name: &str| -> Vec<String> {
        list(r, key).iter().map(|x| format!("{} {} ({})", word(s(x, name)), m.fmt(int(x, "value")), pct_bps(int(x, "share_bps")))).collect()
    };
    let by_account = shares("allocation", "registration");
    if !by_account.is_empty() {
        let _ = writeln!(o, "\nBy account type: {}.", by_account.join("; "));
    }
    let by_kind = shares("kinds", "kind");
    if !by_kind.is_empty() {
        let _ = writeln!(o, "By kind: {}.", by_kind.join("; "));
    }
    let room = list(r, "room");
    if !room.is_empty() {
        let _ = writeln!(o, "\nCONTRIBUTION ROOM (from their CRA figure; withdrawals give no room back this year):");
        for x in room {
            let reg = s(x, "registration");
            let mut line = match x["room"].as_i64() {
                Some(room) => format!("- {reg} {}: room {}", int(x, "year"), m.fmt(room)),
                None => format!("- {reg} {}: room UNKNOWN: Tally does not have their CRA room figure yet (CRA My Account shows it; they type it on Tally's Investments page)", int(x, "year")),
            };
            let _ = write!(line, "; put in {}, taken out {}", m.fmt(int(x, "contributed")), m.fmt(int(x, "withdrawn")));
            if let Some(left) = x["left"].as_i64() {
                let _ = write!(line, "; left {}", m.fmt(left));
            }
            if int(x, "over") > 0 {
                let _ = write!(line, "; OVER by {} (check CRA My Account)", m.fmt(int(x, "over")));
            }
            if let Some(deadline) = x["deadline"].as_str() {
                let _ = write!(line, "; deadline {}", day_of(deadline, today));
            }
            let _ = writeln!(o, "{line}");
        }
    }
    o
}

fn invest_list(r: &Value, lists: Option<&Value>, ctx: &Ctx) -> String {
    let today = ctx.today;
    let rows = list(r, "activities");
    let mut o = String::new();
    let _ = writeln!(o, "TALLY INVESTMENT ACTIVITY, newest first. {} {TALLY_IS}", today_line(today));
    for a in lists.map_or(&[][..], |l| list(l, "accounts")).iter().filter(|a| a["archived"] != true && import_filled(a)) {
        let _ = writeln!(o, "- {}: {FILLED}.", s(a, "name"));
    }
    if rows.is_empty() {
        let _ = writeln!(o, "No activities: {NONE_LOGGED}. Money moved into an investment or savings account from the bank is a transfer: money.tx.list adds those up.");
        return o;
    }
    let _ = writeln!(o, "{}.", plural(rows.len() as i64, "activity", "activities"));
    for a in rows {
        let m = Money::of(s(a, "currency"));
        let mut line = format!("- #{} {}: {}", int(a, "id"), day_of(s(a, "date"), today), word(s(a, "type")));
        if int(a, "quantity") != 0 {
            let _ = write!(line, " {} units", quantity(int(a, "quantity")));
        }
        if let Some(sym) = a["symbol"].as_str() {
            let _ = write!(line, " of {sym}");
        }
        let _ = write!(line, ", amount {}", m.fmt(int(a, "amount")));
        if int(a, "fee") != 0 {
            let _ = write!(line, ", fee {}", m.fmt(int(a, "fee")));
        }
        if let (Some(to), Some(cur)) = (a["to_amount"].as_i64(), a["to_currency"].as_str()) {
            let _ = write!(line, ", became {}", Money::of(cur).fmt(to));
        }
        let _ = write!(line, ", in {}", s(a, "account"));
        if !s(a, "note").is_empty() {
            let _ = write!(line, " ({})", s(a, "note"));
        }
        let _ = writeln!(o, "{line}");
    }
    o
}

// ── Avex ────────────────────────────────────────────────────────────────────────────────────

/// A set: "215 lb x 5", "bodyweight x 14".
fn set(weight_v: &Value, reps: &Value, unit: &str) -> String {
    let reps = reps.as_i64().unwrap_or(0);
    match weight_v.as_f64() {
        Some(w) => format!("{} {unit} x {reps}", weight(w)),
        None => format!("bodyweight x {reps}"),
    }
}

/// What a lift with a weighted best set but no estimate says in place of one: Epley's estimate is
/// made only from sets of 1 to 12 reps, so a best set without one means every set was over 12.
const NO_E1RM: &str = "no estimated max: every set was over 12 reps (an estimate needs a set of 1 to 12 reps)";

/// An exercise's working sets (warm-ups left out, as Avex counts them), their volume (the
/// exercise's own figure when it has one, else weight x reps added up) and its warm-ups.
fn working_sets(e: &Value) -> (Vec<&Value>, f64, usize) {
    let sets = list(e, "sets");
    let work: Vec<&Value> = sets.iter().filter(|x| s(x, "kind") != "warmup" && int(x, "reps") > 0).collect();
    let volume = e["volume"].as_f64().unwrap_or_else(|| work.iter().map(|x| num(x, "weight") * int(x, "reps") as f64).sum());
    (work, volume, sets.iter().filter(|x| s(x, "kind") == "warmup").count())
}

/// "3 working sets, volume 3,990 lb": volume only when a working set had a weight.
fn work_phrase(work: &[&Value], volume: f64, unit: &str) -> String {
    let n = plural(work.len() as i64, "working set", "working sets");
    if volume > 0.0 { format!("{n}, volume {} {unit}", weight(volume)) } else { n }
}

/// One exercise of a workout as one line: its working sets, their volume, and each set.
fn exercise_line(e: &Value, unit: &str) -> String {
    if e["skipped"] == true {
        return format!("- {}: skipped", s(e, "name"));
    }
    let (work, volume, warm) = working_sets(e);
    let sets: Vec<String> = work.iter().map(|x| set(&x["weight"], &x["reps"], unit)).collect();
    let mut line = format!("- {}: {}", s(e, "name"), work_phrase(&work, volume, unit));
    if !sets.is_empty() {
        let _ = write!(line, "; {}", sets.join(", "));
    }
    if warm > 0 {
        let _ = write!(line, "; plus {} not counted", plural(warm as i64, "warm-up set", "warm-up sets"));
    }
    line
}

/// One workout as one line.
fn session_line(x: &Value, unit: &str, today: Date) -> String {
    let mut line = format!(
        "- {} {} ({}): {}, {} min, {}, {} {unit} lifted",
        day_of(s(x, "day"), today), s(x, "time"), ago_of(s(x, "day"), today), s(x, "title"), int(x, "minutes"),
        plural(int(x, "sets"), "set", "sets"), weight(num(x, "volume"))
    );
    if int(x, "prs") > 0 {
        let _ = write!(line, ", {}", plural(int(x, "prs"), "record", "records"));
    }
    let lifts: Vec<&str> = list(x, "lifts").iter().filter_map(Value::as_str).collect();
    if !lifts.is_empty() {
        let _ = write!(line, "; lifts: {}", lifts.join(", "));
    }
    if int(x, "skipped") > 0 {
        let _ = write!(line, "; {} skipped", plural(int(x, "skipped"), "lift", "lifts"));
    }
    if !s(x, "mood").is_empty() {
        let _ = write!(line, "; felt {}", s(x, "mood"));
    }
    if x["untracked"] == true {
        line.push_str("; UNTRACKED (counts toward nothing)");
    }
    let _ = write!(line, " [id {}]", int(x, "id"));
    line
}

/// A lift's line: its best set, how often, and its trend.
fn lift_line(l: &Value, unit: &str, today: Date) -> String {
    let mut line = format!("- {}:", s(l, "name"));
    match l.get("best").filter(|b| b.is_object()) {
        Some(b) => {
            let _ = write!(line, " best set {}", set(&b["weight"], &b["reps"], unit));
            if let Some(w) = b["weight"].as_f64() {
                line.push_str(&other_unit(w, unit));
            }
            let _ = write!(line, " on {}", day_of(s(b, "day"), today));
            match b["e1rm"].as_f64() {
                Some(e) => {
                    let _ = write!(line, ", estimated 1-rep max {} {unit}{}", weight(e), other_unit(e, unit));
                }
                None => {
                    let _ = write!(line, ", {NO_E1RM}");
                }
            }
            // The best set is ranked by estimate: a heavier set with fewer reps is not it.
            if let Some(h) = l.get("heaviest").filter(|h| h["weight"].as_f64() > b["weight"].as_f64()) {
                let w = h["weight"].as_f64().unwrap_or(0.0);
                let _ = write!(line, "; heaviest weight ever lifted {}{} on {}", set(&h["weight"], &h["reps"], unit), other_unit(w, unit), day_of(s(h, "day"), today));
                let again = int(l, "heaviest_sessions");
                if let Some(last) = l["heaviest_last_day"].as_str().filter(|d| again > 1 && *d != s(h, "day")) {
                    let _ = write!(line, " ({} {unit} reached in {}, the last on {})", weight(w), plural(again, "workout", "workouts"), day_of(last, today));
                }
                if let (Some(he), Some(be)) = (h["e1rm"].as_f64().filter(|he| Some(*he) <= b["e1rm"].as_f64()), b["e1rm"].as_f64()) {
                    let _ = write!(line, ", heavier but not a better set: its estimated 1-rep max is {} {unit}, the best set's {} {unit}", weight(he), weight(be));
                }
            }
        }
        None => match l["best_reps"].as_i64() {
            Some(reps) => {
                let _ = write!(line, " bodyweight, no weighted best set, so no estimated max; most reps in one set: {reps}");
            }
            None => line.push_str(" bodyweight or timed only, no weighted best set, so no estimated max"),
        },
    }
    let _ = write!(line, "; done in {} ({}), last on {}", plural(int(l, "sessions"), "workout", "workouts"), plural(int(l, "sets"), "set", "sets"), day_of(s(l, "last_day"), today));
    if let Some(c) = l["change_pct"].as_f64() {
        let _ = write!(line, "; estimated max over the last 4 weeks vs the 4 before: {}", signed_pct(c));
    }
    line
}

fn signed_pct(c: f64) -> String {
    format!("{}{}%", if c > 0.0 { "+" } else { "" }, weight(c))
}

/// When the copy was exported: "on Thu Sep 17 at 21:14 (yesterday)".
fn exported_phrase(imported: &Value, today: Date) -> String {
    let exported = s(imported, "exported_at");
    date(exported).map_or_else(String::new, |d| format!("on {} at {} ({})", day(d, today), exported.get(11..16).unwrap_or(""), ago(d, today)))
}

/// Where the copy comes from and how old it is; `None` when nothing was imported.
fn gym_header(o: &mut String, imported: &Value, unit: &str, today: Date) -> bool {
    let _ = writeln!(o, "AVEX (gym app; weights in {unit}). {} {AVEX_IS}", today_line(today));
    if !imported.is_object() {
        let _ = writeln!(o, "Nothing has been imported yet: export it in Avex (Settings, Export, Training history, JSON), copy avex_export.json to this computer, and import it on the Avex page's Data tab.");
        return false;
    }
    let exported = s(imported, "exported_at");
    if let Some(d) = date(exported) {
        let _ = writeln!(o, "This is a copy exported from the phone on {} at {} ({}): anything logged after that is not here.", day(d, today), exported.get(11..16).unwrap_or(""), ago(d, today));
    }
    true
}

fn gym_summary(r: &Value, lifts: Option<&Value>, cardio: Option<&Value>, months: [Option<&Value>; 4], pages: [Option<&Value>; 2], last_workout: Option<&Value>, ctx: &Ctx) -> String {
    // The newest workouts, and this year's count.
    let [sessions, year] = pages;
    let today = ctx.today;
    let unit = s(r, "unit");
    let mut o = String::new();
    if !gym_header(&mut o, &r["imported"], unit, today) {
        return o;
    }
    let t = &r["totals"];
    let untracked = match int(t, "untracked") {
        0 => String::new(),
        n => {
            let days: Vec<String> = list(t, "untracked_days").iter().filter_map(|d| d.as_str()).map(|d| day_of(d, today)).collect();
            let named = match days.len() as i64 {
                0 => String::new(),
                k if k == n => format!(": {}", days.join(", ")),
                k => format!("; the newest {k}: {}", days.join(", ")),
            };
            format!(" {} the person excluded in Avex (untracked) {} not counted{named}.", plural(n, "workout", "workouts"), if n == 1 { "is" } else { "are" })
        }
    };
    let _ = writeln!(
        o, "\nALL TIME ({} to {}): {}, {}, {} {unit} lifted, {}.{untracked}",
        day_of(s(t, "first_day"), today), day_of(s(t, "last_day"), today), plural(int(t, "sessions"), "workout", "workouts"),
        plural(int(t, "sets"), "set", "sets"), weight(num(t, "volume")),
        match int(t, "cardio") { 0 => "no cardio in this Avex copy".to_string(), n => format!("{} ({} minutes)", plural(n, "cardio session", "cardio sessions"), int(t, "cardio_minutes")) }
    );
    if let Some(y) = year.filter(|y| y["total"].is_i64()) {
        let excluded = match int(y, "untracked") { 0 => String::new(), n => format!(", and {n} the person excluded (untracked, not counted)") };
        let _ = writeln!(o, "THIS YEAR ({}, Jan 1 to today): {}{excluded}.", today.year(), plural(int(y, "total") - int(y, "untracked"), "counted workout", "counted workouts"));
    }
    let week = |w: &Value| {
        let cardio = match int(w, "cardio_minutes") { 0 => "no cardio".to_string(), c => format!("{c} cardio minutes") };
        match int(w, "sessions") {
            0 => format!("no workouts in this Avex copy, {cardio}"),
            n => format!(
                "{}, {}, {} {unit} lifted, {} active minutes, {cardio}",
                plural(n, "workout", "workouts"), plural(int(w, "sets"), "set", "sets"), weight(num(w, "volume")), int(w, "active_minutes")
            ),
        }
    };
    // Which workouts each week held, from the newest ones the summary lists.
    let named = |from: Option<Date>, to: Option<Date>| -> String {
        let (Some(from), Some(to)) = (from, to) else { return String::new() };
        let rows: Vec<&Value> = list(r, "recent").iter().chain(sessions.map(|x| list(x, "sessions")).unwrap_or(&[]))
            .filter(|x| date(s(x, "day")).is_some_and(|d| d >= from && d <= to))
            .collect();
        let mut seen: Vec<String> = Vec::new();
        for x in rows {
            let line = format!("{} {}", day_of(s(x, "day"), today), s(x, "title"));
            if !seen.contains(&line) {
                seen.push(line);
            }
        }
        seen.reverse();
        if seen.is_empty() { String::new() } else { format!(" Workouts: {}.", seen.join("; ")) }
    };
    let this = &r["this_week"];
    let _ = writeln!(o, "THIS WEEK ({} to today), so far: {}.{}", day_of(s(this, "start"), today), week(this), named(date(s(this, "start")), Some(today)));
    let last = &r["last_week"];
    let last_end = date(s(last, "start")).and_then(|d| d.checked_add(jiff::Span::new().days(6)).ok());
    let _ = writeln!(o, "LAST WEEK ({} to {}): {}.{}", day_of(s(last, "start"), today), last_end.map(|d| day(d, today)).unwrap_or_default(), week(last), named(date(s(last, "start")), last_end));
    let trained_today = list(r, "recent").iter().any(|x| date(s(x, "day")) == Some(today));
    if !trained_today {
        let _ = writeln!(o, "TODAY: no workout in this copy. It was exported {}, so a workout logged after that would not be here.", exported_phrase(&r["imported"], today));
    }
    if let Some(since) = r["days_since_last"].as_i64() {
        let last_day = list(r, "recent").first().map(|x| day_of(s(x, "day"), today)).unwrap_or_default();
        let _ = writeln!(o, "Last workout: {last_day} ({}).", plural(since, "day ago", "days ago"));
    }
    let _ = writeln!(o, "Streak: {} in a row with at least one workout.", plural(int(r, "streak_weeks"), "week", "weeks"));
    let breaks: Vec<String> = list(r, "breaks").iter().map(|b| format!(
        "{} to {}, {} days, after {} in a row with a workout",
        day_of(s(b, "last_before"), today), day_of(s(b, "first_after"), today), int(b, "days"), plural(int(b, "streak_before_weeks"), "week", "weeks"),
    )).collect();
    if !breaks.is_empty() {
        let _ = writeln!(o, "Longest breaks (last workout before, first one back): {}.", breaks.join("; "));
    }
    let marked: Vec<String> = list(r, "marked").iter().map(|m| {
        let days: Vec<String> = list(m, "days").iter().filter_map(|d| d.as_str()).map(|d| day_of(d, today)).collect();
        let shown = if days.len() as i64 == int(m, "sessions") { days.join(", ") } else { format!("the newest {}", days.join(", ")) };
        let weeks: Vec<String> = list(m, "weeks").iter().map(|w| {
            let (now, before) = (num(w, "volume"), num(w, "week_before_volume"));
            let less = if before > 0.0 && now < before {
                format!(", so {} {unit} less ({}% less)", weight(before - now), weight(100.0 * (before - now) / before))
            } else {
                String::new()
            };
            format!(
                "the week of {} had {}, {} {unit} lifted; the week before {}, {} {unit}{less}",
                day_of(s(w, "start"), today), plural(int(w, "sessions"), "workout", "workouts"), weight(now),
                plural(int(w, "week_before_sessions"), "workout", "workouts"), weight(before),
            )
        }).collect();
        let weeks = if weeks.is_empty() { String::new() } else { format!("; {}", weeks.join("; ")) };
        format!("{} {} ({shown}{weeks})", s(m, "kind").replace('_', " "), int(m, "sessions"))
    }).collect();
    if !marked.is_empty() {
        let _ = writeln!(o, "Workouts Avex marks with a type (a deload, a test day…), how many and when: {}.", marked.join("; "));
    }
    let notes: Vec<String> = list(r, "notes").iter().map(|n| {
        let mut where_ = day_of(s(n, "day"), today);
        if let Some(lift) = n["lift"].as_str() {
            let _ = write!(where_, ", on {lift}");
        }
        if int(n, "times") > 1 {
            let _ = write!(where_, ", the newest of {} workouts with it", int(n, "times"));
        }
        if n["untracked"].as_bool() == Some(true) {
            where_.push_str(", an untracked workout");
        }
        format!("{where_}: \"{}\"", s(n, "text"))
    }).collect();
    if !notes.is_empty() {
        let _ = writeln!(o, "What the person wrote in Avex (a workout's journal or a lift's note), newest first, the why behind a change: {}.", notes.join("; "));
    }
    let per_week = int(&r["imported"], "days_per_week");
    if per_week > 0 {
        let _ = writeln!(o, "Plan in Avex: {per_week} training days a week.");
    }
    if let Some(c) = cardio {
        let moving: Vec<&Value> = list(c, "entries").iter().filter(|x| s(x, "kind") != "rest").collect();
        let km: f64 = moving.iter().filter_map(|x| x["distance_km"].as_f64()).sum();
        if moving.is_empty() {
            let _ = writeln!(o, "Cardio in all: no cardio in this Avex copy.");
        } else {
            let _ = writeln!(o, "Cardio in all: {}, {} minutes, {} km.", plural(moving.len() as i64, "session", "sessions"), moving.iter().map(|x| int(x, "minutes")).sum::<i64>(), (km * 100.0).round() / 100.0);
            cardio_kinds(&mut o, list(c, "entries"), today);
        }
    }
    gym_months(&mut o, months, unit);
    if let Some(c) = cardio {
        cardio_months(&mut o, list(c, "entries"), list(c, "entries").len() as u64 >= SUMMARY_CARDIO, today);
    }

    let _ = writeln!(o, "\nWEEK BY WEEK (week starting Monday):");
    for w in list(r, "weeks") {
        let _ = writeln!(o, "- {}: {}", day_of(s(w, "start"), today), week(w));
    }
    let _ = writeln!(o, "\nRECENT WORKOUTS (newest first):");
    for x in list(r, "recent") {
        let _ = writeln!(o, "{}", session_line(x, unit, today));
    }
    if let Some(detail) = last_workout {
        let row = &detail["row"];
        let _ = writeln!(
            o, "\nLAST WORKOUT IN DETAIL ({}, {}: {} in all, {} {unit} lifted in all). A lift's volume is weight x reps added up over its working sets; warm-ups are not counted:",
            day_of(s(row, "day"), today), s(row, "title"), plural(int(row, "sets"), "set", "sets"), weight(num(row, "volume"))
        );
        for e in list(detail, "exercises") {
            let _ = writeln!(o, "{}", exercise_line(e, unit));
        }
    }
    let all = lifts.map(|l| list(l, "lifts")).filter(|l| !l.is_empty()).unwrap_or_else(|| list(r, "lifts"));
    if !all.is_empty() {
        let _ = writeln!(o, "\nEVERY LIFT, done most first (best set ever, by estimated 1-rep max):");
        lift_lines(&mut o, all, unit, today);
        if lifts.is_some() {
            let names: Vec<&str> = all.iter().map(|l| s(l, "name")).collect();
            let _ = writeln!(o, "These are the only lifts ever logged: {}. Any other lift has never been logged.", names.join(", "));
        }
    }
    let records = list(r, "records");
    if !records.is_empty() {
        let recs: Vec<String> = records.iter().map(|x| format!("{} {} on {}", s(x, "lift"), set(&x["weight"], &x["reps"], unit), day_of(s(x, "day"), today))).collect();
        let _ = writeln!(o, "\nNEWEST RECORDS: {}.", recs.join("; "));
    }
    if let Some(c) = cardio {
        let rows = list(c, "entries");
        if !rows.is_empty() {
            let _ = writeln!(o, "\nRECENT CARDIO (newest first; gym.cardio has all of it):");
            for x in rows.iter().take(5) {
                let _ = writeln!(o, "{}", cardio_line(x, today));
            }
        }
    }
    let goals = list(r, "goals");
    if goals.is_empty() {
        let _ = writeln!(o, "\nOPEN GOALS: none in this Avex copy.");
    } else {
        let _ = writeln!(o, "\nOPEN GOALS:");
        for g in goals {
            let target = g["target_value"].as_f64().map(|v| if s(g, "kind") == "lift_target" { format!(" {} {unit}", weight(v)) } else { format!(" {}", weight(v)) }).unwrap_or_default();
            let note = if s(g, "note").is_empty() { String::new() } else { format!(" \"{}\"", s(g, "note")) };
            let best = g.get("best").filter(|b| b.is_object()).map(|b| {
                let e1rm = b["e1rm"].as_f64().map(|e| format!(", estimated 1-rep max {} {unit}", weight(e))).unwrap_or_default();
                format!("; best so far {} on {}{e1rm}", set(&b["weight"], &b["reps"], unit), day_of(s(b, "day"), today))
            }).unwrap_or_default();
            let _ = writeln!(o, "- {} {}{target}, set {}{note}{best}", s(g, "kind").replace('_', " "), s(g, "target_key"), day_of(s(g, "created_day"), today));
        }
    }
    o
}

/// Workouts, volume, cardio minutes and distance month by month, from `gym.series`.
fn gym_months(o: &mut String, months: [Option<&Value>; 4], unit: &str) {
    let [sessions, volume, minutes, distance] = months;
    let Some(sessions) = sessions else { return };
    let values = |s: Option<&Value>| s.and_then(|s| list(s, "series").first()).map(|x| list(x, "values").to_vec()).unwrap_or_default();
    let (n, v, c, d) = (values(Some(sessions)), values(volume), values(minutes), values(distance));
    let labels = list(sessions, "labels");
    let shown: Vec<String> = labels.iter().enumerate().map(|(i, label)| {
        let at = |x: &Vec<Value>| x.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        let now = if i + 1 == labels.len() { " so far" } else { "" };
        let label = label.as_str().unwrap_or("");
        let cardio = if at(&c) > 0.0 { format!("{} cardio minutes, {} km", weight(at(&c)), (at(&d) * 100.0).round() / 100.0) } else { "no cardio".into() };
        match at(&n) as i64 {
            0 if at(&c) == 0.0 => format!("{label}{now}: nothing in this Avex copy"),
            0 => format!("{label}{now}: no workouts, {cardio}"),
            n => format!("{label}{now}: {}, {} {unit} lifted, {cardio}", plural(n, "workout", "workouts"), weight(at(&v))),
        }
    }).collect();
    let _ = writeln!(o, "\nMONTH BY MONTH (calendar months, oldest first; for a budget month or any other days, gym.sessions with from and to counts them): {}.", shown.join("; "));
}

fn gym_sessions(payload: &Value, r: &Value, unit: &str, ctx: &Ctx) -> String {
    let today = ctx.today;
    let rows = list(r, "sessions");
    let mut o = String::new();
    let _ = writeln!(o, "AVEX WORKOUTS, newest first (weights in {}). {} {AVEX_IS}", if unit.is_empty() { "their unit" } else { unit }, today_line(today));
    if let Some(lift) = payload["lift"].as_str().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(o, "Only workouts with {}.", lift.trim());
    }
    let (first, last) = (payload["from"].as_str().and_then(date), payload["to"].as_str().and_then(date));
    if first.is_some() || last.is_some() {
        let _ = writeln!(o, "Only workouts {}.", match (first, last) {
            (Some(a), Some(b)) => format!("from {} to {}", day(a, today), day(b, today)),
            (Some(a), None) => format!("from {} to today", day(a, today)),
            (None, Some(b)) => format!("up to {}", day(b, today)),
            (None, None) => unreachable!(),
        });
    }
    let total = int(r, "total");
    let untracked = int(r, "untracked");
    let from = payload["offset"].as_i64().unwrap_or(0);
    if total == 0 {
        let _ = writeln!(o, "No workouts in this Avex copy{}.", if first.is_some() || last.is_some() { " on those days" } else { "" });
    } else {
        let counted = if untracked > 0 {
            format!(" ({} counted, {} the person excluded in Avex: untracked, marked below, not counted)", total - untracked, untracked)
        } else {
            String::new()
        };
        let _ = writeln!(o, "{} in all{counted}; showing {} to {}.", plural(total, "workout", "workouts"), if rows.is_empty() { 0 } else { from + 1 }, from + rows.len() as i64);
    }
    let counted: Vec<&Value> = rows.iter().filter(|x| x["untracked"] != true).collect();
    if counted.len() >= 6 {
        // "How long does Upper A usually take": each kind added up over every one read, not left
        // to a model averaging the ones it could see (bench v3, 2026-10-30).
        let mut titles: Vec<(&str, i64, i64)> = Vec::new();
        for x in &counted {
            match titles.iter_mut().find(|t| t.0 == s(x, "title")) {
                Some(t) => {
                    t.1 += 1;
                    t.2 += int(x, "minutes");
                }
                None => titles.push((s(x, "title"), 1, int(x, "minutes"))),
            }
        }
        titles.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let cells: Vec<String> = titles.iter().map(|(title, n, minutes)| format!("{title} {n}, {:.1} min on average", *minutes as f64 / *n as f64)).collect();
        let _ = writeln!(o, "Counted workouts by kind, every one of these {} read: {}.", rows.len(), cells.join("; "));
        // "I planned 4 a week since I started": the weeks since the first one read.
        if let (true, Some(first)) = (from == 0 && rows.len() as i64 == total, counted.iter().filter_map(|x| date(s(x, "day"))).min()) {
            let days = days_since(first, today) + 1;
            let _ = writeln!(o, "From the first of these, {}, to today: {} days ({:.1} weeks), {}, {:.1} a week.", day(first, today), days, days as f64 / 7.0,
                plural(counted.len() as i64, "counted workout", "counted workouts"), counted.len() as f64 * 7.0 / days as f64);
        }
    }
    // A year of workouts one per line filled a small model's context before it could answer
    // (bench v3, 2026-10-30): the counts by month, and the newest ones.
    let shown = rows.len().min(LISTED_WORKOUTS);
    if shown < rows.len() {
        let mut months: BTreeMap<(i16, i8), i64> = BTreeMap::new();
        for x in rows.iter().filter(|x| x["untracked"] != true) {
            if let Some(d) = date(s(x, "day").get(..10).unwrap_or("")) {
                *months.entry((d.year(), d.month())).or_default() += 1;
            }
        }
        let cells: Vec<String> = months.iter().map(|((y, mo), n)| {
            let first = jiff::civil::date(*y, *mo, 1);
            format!("{} {n}", if *y == today.year() { first.strftime("%B").to_string() } else { first.strftime("%B %Y").to_string() })
        }).collect();
        let _ = writeln!(o, "Counted workouts by calendar month, these {} read: {}.", rows.len(), cells.join("; "));
        let _ = writeln!(o, "The newest {shown} of them (fewer days, from and to, list the others):");
    }
    for x in &rows[..shown] {
        let _ = writeln!(o, "{}", session_line(x, unit, today));
    }
    let _ = writeln!(o, "\ngym.session.get with an id gives every set of a workout.");
    o
}

fn gym_session(r: &Value, ctx: &Ctx) -> String {
    let today = ctx.today;
    let unit = s(r, "unit");
    let row = &r["row"];
    let mut o = String::new();
    let _ = writeln!(o, "AVEX WORKOUT (weights in {unit}), from their Avex export. {}", today_line(today));
    let _ = writeln!(o, "{}", session_line(row, unit, today));
    if !s(r, "journal").is_empty() {
        let _ = writeln!(o, "Journal: {}", s(r, "journal"));
    }
    for e in list(r, "exercises") {
        if e["skipped"] == true {
            let _ = writeln!(o, "\n{}: skipped", s(e, "name"));
            continue;
        }
        let (work, volume, warm) = working_sets(e);
        let mut head = format!("\n{}: {}", s(e, "name"), work_phrase(&work, volume, unit));
        if warm > 0 {
            let _ = write!(head, " (plus {} not counted)", plural(warm as i64, "warm-up set", "warm-up sets"));
        }
        match e["best_e1rm"].as_f64() {
            Some(b) => {
                let _ = write!(head, ", best estimated 1-rep max {} {unit}", weight(b));
            }
            None if work.iter().any(|x| x["weight"].is_number()) && work.iter().filter(|x| x["weight"].is_number()).all(|x| int(x, "reps") > 12) => {
                let _ = write!(head, ", {NO_E1RM}");
            }
            None => {}
        }
        if e["was_pr"] == true {
            head.push_str(", a RECORD");
        }
        if !s(e, "difficulty").is_empty() {
            let _ = write!(head, ", felt {}", s(e, "difficulty").to_ascii_lowercase());
        }
        if let Some(p) = e.get("previous").filter(|p| p.is_object()) {
            let _ = write!(head, "; the time before, {}: {}", day_of(s(p, "day"), today), set(&p["top_weight"], &p["top_reps"], unit));
        }
        let _ = writeln!(o, "{head}");
        for (i, x) in list(e, "sets").iter().enumerate() {
            let mut line = format!("  set {}: {} ({})", i + 1, set(&x["weight"], &x["reps"], unit), s(x, "kind"));
            if let Some(rpe) = x["rpe"].as_f64() {
                let _ = write!(line, ", RPE {}", weight(rpe));
            }
            if x["best"] == true {
                line.push_str(", best set");
            }
            let _ = writeln!(o, "{line}");
        }
    }
    o
}

fn gym_lifts(payload: &Value, r: &Value, ctx: &Ctx) -> String {
    let unit = s(r, "unit");
    let mut o = String::new();
    let _ = writeln!(o, "AVEX LIFTS, done most first (weights in {unit}; best set ever, by estimated 1-rep max). {} {AVEX_IS}", today_line(ctx.today));
    if list(r, "lifts").is_empty() {
        let _ = writeln!(o, "No lifts in this Avex copy{}.", payload["query"].as_str().map(|q| format!(" matching \"{q}\"")).unwrap_or_default());
    }
    lift_lines(&mut o, list(r, "lifts"), unit, ctx.today);
    o
}

/// Each lift's line, then the heaviest set of all and the lifts by how much they improved.
/// Lifts they stopped while one with the same name in a longer one goes on ("Bench Press" and
/// "Dumbbell Bench Press"): asked how the bench goes, a small model told of June's bench and
/// never of the dumbbell bench done every week since (bench, 2026-10-30).
fn moved_to(rows: &[Value]) -> Vec<(&Value, &Value)> {
    let newest = rows.iter().filter_map(|l| date(s(l, "last_day"))).max();
    let Some(stale) = newest.and_then(|d| d.checked_sub(jiff::Span::new().days(28)).ok()) else { return Vec::new() };
    let mut pairs = Vec::new();
    for from in rows.iter().filter(|l| date(s(l, "last_day")).is_some_and(|d| d < stale)) {
        let name = s(from, "name").to_lowercase();
        let words = |n: &str| format!(" {} ", n.to_lowercase());
        let to = rows.iter().filter(|l| s(l, "name").len() > name.len() && words(s(l, "name")).contains(&words(&name)) && s(l, "last_day") > s(from, "last_day"))
            .max_by(|a, b| s(a, "last_day").cmp(s(b, "last_day")));
        if let Some(to) = to {
            pairs.push((from, to));
        }
    }
    pairs
}

fn moved_line(o: &mut String, pairs: &[(&Value, &Value)], today: Date) {
    if pairs.is_empty() {
        return;
    }
    let cells: Vec<String> = pairs.iter().map(|(from, to)| format!("{} not done since {}, while {} goes on ({}, last on {})", s(from, "name"), day_of(s(from, "last_day"), today),
        s(to, "name"), plural(int(to, "sessions"), "workout", "workouts"), day_of(s(to, "last_day"), today))).collect();
    let _ = writeln!(o, "Lifts they stopped, with a longer one of the same name still done (asked about the one, say how both go): {}.", cells.join("; "));
}

fn lift_lines(o: &mut String, rows: &[Value], unit: &str, today: Date) {
    for l in rows {
        let _ = writeln!(o, "{}", lift_line(l, unit, today));
    }
    moved_line(o, &moved_to(rows), today);
    let heaviest = rows.iter().filter_map(|l| l.get("heaviest").or(l.get("best")).filter(|b| b.is_object()).map(|b| (l, b))).max_by(|a, b| num(a.1, "weight").total_cmp(&num(b.1, "weight")));
    if let Some((l, b)) = heaviest {
        let _ = writeln!(o, "Heaviest set of any lift: {} {} on {}.", s(l, "name"), set(&b["weight"], &b["reps"], unit), day_of(s(b, "day"), today));
    }
    let high_rep: Vec<&str> = rows.iter().filter(|l| l.get("best").is_some_and(|b| b.is_object() && b["e1rm"].is_null())).map(|l| s(l, "name")).collect();
    if !high_rep.is_empty() {
        let _ = writeln!(o, "No estimated max, because every set was over 12 reps (never give these another lift's estimate): {}.", high_rep.join(", "));
    }
    let mut changes: Vec<(&str, f64)> = rows.iter().filter_map(|l| Some((s(l, "name"), l["change_pct"].as_f64()?))).collect();
    changes.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !changes.is_empty() {
        let shown: Vec<String> = changes.iter().map(|(name, c)| format!("{name} {}", signed_pct(*c))).collect();
        let _ = writeln!(o, "Most improved first (estimated max, last 4 weeks vs the 4 before): {}.", shown.join("; "));
    }
}

fn gym_lift(r: &Value, lifts: Option<&Value>, ctx: &Ctx) -> String {
    let today = ctx.today;
    let unit = s(r, "unit");
    let mut o = String::new();
    let _ = writeln!(o, "AVEX LIFT HISTORY (weights in {unit}), from their Avex export. {}", today_line(today));
    let _ = writeln!(o, "{}", lift_line(&r["row"], unit, today));
    let name = s(&r["row"], "name");
    let moved: Vec<_> = lifts.map(|l| moved_to(list(l, "lifts"))).unwrap_or_default().into_iter().filter(|(from, _)| s(from, "name") == name).collect();
    moved_line(&mut o, &moved, today);
    let _ = writeln!(o, "\nEACH WORKOUT, newest first (heaviest working set, estimated 1-rep max, sets, volume):");
    for p in list(r, "history") {
        let e1rm = match p["e1rm"].as_f64() {
            Some(e) => format!(", estimated 1-rep max {} {unit}", weight(e)),
            None if p["top_weight"].is_number() => ", no estimated max (over 12 reps)".into(),
            None => String::new(),
        };
        let record = if p["record"] == true { ", a RECORD" } else { "" };
        let _ = writeln!(
            o, "- {} ({}): {}{e1rm}, {}, {} {unit} volume{record}",
            day_of(s(p, "day"), today), ago_of(s(p, "day"), today), set(&p["top_weight"], &p["top_reps"], unit),
            plural(int(p, "sets"), "set", "sets"), weight(num(p, "volume"))
        );
    }
    o
}

fn cardio_line(x: &Value, today: Date) -> String {
    let mut line = format!("- {} {}: {}", day_of(s(x, "day"), today), s(x, "time"), s(x, "kind"));
    if int(x, "minutes") > 0 {
        let _ = write!(line, ", {} min", int(x, "minutes"));
    }
    if let Some(km) = x["distance_km"].as_f64() {
        let _ = write!(line, ", {} km", (km * 100.0).round() / 100.0);
    }
    if !s(x, "effort").is_empty() {
        let _ = write!(line, ", {}", s(x, "effort"));
    }
    if !s(x, "note").is_empty() {
        let _ = write!(line, " ({})", s(x, "note"));
    }
    line
}

/// How many entries gym.cardio gives when its payload names no limit.
const CARDIO_LIMIT: u64 = 50;
/// How many the gym summary reads, so its months are whole.
const SUMMARY_CARDIO: u64 = 1000;

/// "5 runs", "1 bike ride", "2 rowing sessions": Avex's cardio kind counted.
fn cardio_count(kind: &str, n: i64) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        k @ ("run" | "walk" | "swim" | "hike") => plural(n, k, &format!("{k}s")),
        "bike" | "ride" | "cycling" => plural(n, "bike ride", "bike rides"),
        "" => plural(n, "cardio session", "cardio sessions"),
        k => plural(n, &format!("{k} session"), &format!("{k} sessions")),
    }
}

/// Minutes as "41 min" or "2 h 41 min".
fn duration(minutes: i64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Cardio added up by kind over every entry read, most first: a run is not a treadmill session,
/// and a model adding a year of months mixes them. A kind done three times or fewer lists each,
/// with its note (where a hike went).
fn cardio_kinds(o: &mut String, rows: &[Value], today: Date) {
    let mut kinds: Vec<(String, Vec<&Value>)> = Vec::new();
    for x in rows.iter().filter(|x| s(x, "kind") != "rest") {
        let kind = s(x, "kind").trim().to_ascii_lowercase();
        match kinds.iter_mut().find(|k| k.0 == kind) {
            Some(k) => k.1.push(x),
            None => kinds.push((kind, vec![x])),
        }
    }
    if kinds.is_empty() {
        return;
    }
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1.len()));
    let cells: Vec<String> = kinds.iter().map(|(kind, xs)| {
        let km: f64 = xs.iter().map(|x| num(x, "distance_km")).sum();
        let mut cell = format!("{}, {}", cardio_count(kind, xs.len() as i64), duration(xs.iter().map(|x| int(x, "minutes")).sum()));
        if km > 0.0 {
            let _ = write!(cell, ", {} km", (km * 100.0).round() / 100.0);
        }
        let one = |x: &Value| {
            let mut one = day_of(s(x, "day"), today);
            if let Some(d) = x["distance_km"].as_f64() {
                let _ = write!(one, " {d} km");
            }
            if int(x, "minutes") > 0 {
                let _ = write!(one, " in {}", duration(int(x, "minutes")));
            }
            if !s(x, "note").trim().is_empty() {
                let _ = write!(one, " \"{}\"", s(x, "note").trim());
            }
            one
        };
        if xs.len() <= 3 {
            let each: Vec<String> = xs.iter().map(|x| one(x)).collect();
            let _ = write!(cell, " ({})", each.join("; "));
        } else {
            // Too many to list: the ones a question names (a race, a trail) and the longest. "Since
            // my race" is what came after it, added up: a model summed runs from either side.
            let after = |x: &Value| {
                let later: Vec<&&Value> = xs.iter().filter(|y| s(y, "day") > s(x, "day")).collect();
                if later.is_empty() {
                    return String::new();
                }
                let km: f64 = later.iter().map(|y| num(y, "distance_km")).sum();
                let km = if km > 0.0 { format!(", {} km", (km * 100.0).round() / 100.0) } else { String::new() };
                format!(", then after it to today {}{km}", cardio_count(kind, later.len() as i64))
            };
            let noted: Vec<String> = xs.iter().filter(|x| !s(x, "note").trim().is_empty()).take(3).map(|x| format!("{}{}", one(x), after(x))).collect();
            let longest = xs.iter().filter(|x| num(x, "distance_km") > 0.0).max_by(|a, b| num(a, "distance_km").total_cmp(&num(b, "distance_km")));
            let mut extra = Vec::new();
            if !noted.is_empty() {
                extra.push(format!("with a note: {}", noted.join("; ")));
            }
            if let Some(l) = longest.filter(|l| s(l, "note").trim().is_empty()) {
                extra.push(format!("longest {}", one(l)));
            }
            if !extra.is_empty() {
                let _ = write!(cell, " ({})", extra.join("; "));
            }
        }
        cell
    }).collect();
    let _ = writeln!(o, "Cardio by kind, every entry read (each kind its own; a treadmill run is not a run): {}.", cells.join("; "));
}

/// Cardio counted month by month, oldest first, each kind on its own: "- August: 5 runs, 2 h 41
/// min, 26.1 km". A small model miscounts a month's runs from a list. `partial` when the entries
/// were cut at a limit, so the oldest month may be missing some.
fn cardio_months(o: &mut String, rows: &[Value], partial: bool, today: Date) {
    // (year, month) → each kind's (name, count, minutes, km), in the order first met.
    type Kinds = Vec<(String, i64, i64, f64)>;
    let mut months: BTreeMap<(i16, i8), Kinds> = BTreeMap::new();
    for x in rows.iter().filter(|x| s(x, "kind") != "rest") {
        let Some(d) = date(s(x, "day")) else { continue };
        let kinds = months.entry((d.year(), d.month())).or_default();
        let kind = s(x, "kind").trim().to_ascii_lowercase();
        let at = match kinds.iter().position(|k| k.0 == kind) {
            Some(i) => i,
            None => {
                kinds.push((kind, 0, 0, 0.0));
                kinds.len() - 1
            }
        };
        kinds[at].1 += 1;
        kinds[at].2 += int(x, "minutes");
        kinds[at].3 += num(x, "distance_km");
    }
    if months.is_empty() {
        return;
    }
    let _ = writeln!(o, "\nCARDIO BY MONTH (calendar months, oldest first; each run counted; a month not listed has no cardio in this Avex copy):");
    let oldest = months.keys().next().copied();
    for ((year, month), mut kinds) in months {
        kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
        let cells: Vec<String> = kinds.iter().map(|(kind, n, minutes, km)| {
            let mut cell = format!("{}, {}", cardio_count(kind, *n), duration(*minutes));
            if *km > 0.0 {
                let _ = write!(cell, ", {} km", (km * 100.0).round() / 100.0);
            }
            cell
        }).collect();
        let first = jiff::civil::date(year, month, 1);
        let name = if year == today.year() { first.strftime("%B").to_string() } else { first.strftime("%B %Y").to_string() };
        let now = if year == today.year() && month == today.month() { " so far" } else { "" };
        let cut = if partial && oldest == Some((year, month)) { " (older entries were not read, so this month may have more)" } else { "" };
        let _ = writeln!(o, "- {name}{now}: {}{cut}", cells.join("; "));
    }
}

fn gym_cardio(payload: &Value, r: &Value, ctx: &Ctx) -> String {
    let rows = list(r, "entries");
    let mut o = String::new();
    let _ = writeln!(o, "AVEX CARDIO AND REST DAYS, newest first. {} {AVEX_IS}", today_line(ctx.today));
    if rows.is_empty() {
        let _ = writeln!(o, "No cardio and no rest days in this Avex copy.");
        return o;
    }
    let moving: Vec<&Value> = rows.iter().filter(|x| s(x, "kind") != "rest").collect();
    let km: f64 = moving.iter().filter_map(|x| x["distance_km"].as_f64()).sum();
    let limit = payload["limit"].as_u64().unwrap_or(CARDIO_LIMIT) as usize;
    let partial = rows.len() >= limit;
    let _ = writeln!(
        o, "{}, {} minutes, {} km in all{}{}.",
        plural(moving.len() as i64, "cardio session", "cardio sessions"), moving.iter().map(|x| int(x, "minutes")).sum::<i64>(),
        (km * 100.0).round() / 100.0,
        match rows.len() - moving.len() { 0 => String::new(), n => format!("; {} rest days", n) },
        if partial { format!(" (the newest {} entries only: a larger limit reads older ones)", rows.len()) } else { String::new() }
    );
    cardio_kinds(&mut o, rows, ctx.today);
    cardio_months(&mut o, rows, partial, ctx.today);
    let _ = writeln!(o, "\nEACH ENTRY:");
    for x in rows {
        let _ = writeln!(o, "{}", cardio_line(x, ctx.today));
    }
    o
}

fn gym_series(payload: &Value, r: &Value) -> String {
    let labels: Vec<&str> = list(r, "labels").iter().map(|l| l.as_str().unwrap_or("")).collect();
    let mut o = String::new();
    let lift = r["lift"].as_str().map(|l| format!(" of {l}")).unwrap_or_default();
    let _ = writeln!(o, "AVEX {}{lift} by {} (chart numbers, in {}). {AVEX_IS}", s(r, "measure").replace('_', " ").to_ascii_uppercase(), s(r, "by"), s(r, "unit"));
    for series in list(r, "series") {
        let values: Vec<String> = list(series, "values").iter().zip(&labels)
            .map(|(v, l)| format!("{l} {}", v.as_f64().map_or_else(|| "none".into(), weight)))
            .collect();
        let _ = writeln!(o, "- {}: {}", s(series, "name"), values.join("; "));
    }
    let mut query = payload.clone();
    if let Some(q) = query.as_object_mut() {
        q.remove("today");
    }
    let measure = s(r, "measure");
    let kind = if matches!(measure, "e1rm" | "top_weight") { "line" } else { "bar" };
    let what = match measure { "e1rm" => "estimated max".to_string(), "top_weight" => "top weight".to_string(), m => m.replace('_', " ") };
    let title = match r["lift"].as_str() { Some(l) => format!("{l}, {what} by {}", s(r, "by")), None => format!("{} by {}", capitalised(&what), s(r, "by")) };
    chart_block(&mut o, serde_json::json!({"type": kind, "title": title, "gym": query}));
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Ctx {
        Ctx { today: "2026-10-15".parse().unwrap(), currency: "CAD".into() }
    }

    #[test]
    fn money_reads_as_a_person_writes_it() {
        let m = Money::of("cad");
        assert_eq!((m.fmt(15799), m.fmt(-1661), m.fmt(123456789), m.fmt(0), m.fmt(5)), ("$157.99".into(), "-$16.61".into(), "$1,234,567.89".into(), "$0.00".into(), "$0.05".into()));
        assert_eq!(Money::of("JPY").fmt(150000), "150,000 JPY");
        assert_eq!(Money::of("EUR").fmt(100), "€1.00");
        assert_eq!((m.price(3812000000), m.price(42_0000), m.price(10_000_000_000)), ("$38.12".into(), "$0.0042".into(), "$100.00".into()));
        assert_eq!((quantity(150000000), quantity(1_000_000_000)), ("1.5".into(), "10".into()));
        assert_eq!((pct_bps(1250), pct_bps(-75), pct_bps(10000)), ("12.5%".into(), "-0.75%".into(), "100%".into()));
        assert_eq!((weight(215.0), weight(142.5), weight(26185.0), weight(250.83)), ("215".into(), "142.5".into(), "26,185".into(), "250.8".into()));
    }

    #[test]
    fn days_carry_their_weekday_and_their_distance() {
        let today: Date = "2026-10-15".parse().unwrap();
        assert_eq!(day("2026-10-01".parse().unwrap(), today), "Thu Oct 1");
        assert_eq!(day("2025-12-31".parse().unwrap(), today), "Wed Dec 31 2025");
        assert_eq!(ago("2026-10-14".parse().unwrap(), today), "yesterday");
        assert_eq!(ago("2026-09-15".parse().unwrap(), today), "30 days ago");
        assert_eq!(ago("2025-10-15".parse().unwrap(), today), "365 days ago");
        assert_eq!(ago("2026-10-18".parse().unwrap(), today), "in 3 days");
        assert_eq!(today_line(today), "Today is Thursday, October 15, 2026 (2026-10-15).");
        let period = json!({"start": "2026-10-01", "end_exclusive": "2026-11-01"});
        assert_eq!(period_name(&period, today), "October 2026 (Oct 1 to Oct 31)");
        let period = json!({"start": "2026-09-15", "end_exclusive": "2026-10-15"});
        assert_eq!(period_name(&period, today), "Sep 15 to Oct 14");
    }

    fn tx(id: i64, kind: &str, amount: i64, date: &str, category: Option<&str>, note: &str) -> Value {
        json!({"id": id, "type": kind, "amount": amount, "date": date, "account_id": 2, "account": "Visa", "to_account_id": if kind == "TRANSFER" { json!(3) } else { Value::Null },
               "to_account": if kind == "TRANSFER" { json!("Savings") } else { Value::Null }, "category_id": category.map(|_| 1), "category": category, "note": note, "icon": "x", "color": 1})
    }

    fn page() -> Value {
        json!({"period": {"start": "2026-10-01", "end_exclusive": "2026-11-01", "days": 31, "days_left": 17}, "income": 215000, "spent": 143563,
               "transactions": [tx(100, "EXPENSE", 2614, "2026-10-15", Some("Dining"), "Pho Lien"), tx(122, "INCOME", 215000, "2026-10-14", Some("Salary"), "Salary"),
                                tx(130, "TRANSFER", 30000, "2026-10-07", None, "To savings"), tx(91, "EXPENSE", 15799, "2026-10-08", Some("Shopping"), "Phone case"),
                                tx(80, "EXPENSE", 1350 * 100, "2026-10-01", Some("Housing"), "Rent"), tx(81, "EXPENSE", 1498, "2026-10-02", Some("Dining"), "")]})
    }

    #[test]
    fn a_month_of_entries_names_its_totals_and_extremes() {
        let text = render("money.tx.list", &json!({}), &page(), &[], &ctx()).unwrap();
        for want in [
            "TALLY ENTRIES for October 2026 (Oct 1 to Oct 31), this month. Today is Thursday, October 15, 2026 (2026-10-15). Amounts are in CAD. Only what the person has logged in their Tally app: not their bank, not everything they did.\n",
            "6 entries: 4 expenses, 1 income, 1 transfer.",
            "- Spent (all expenses added up): $1,549.11",
            "- Income: $2,150.00",
            "- Moved between accounts (transfers, not spending): $300.00",
            "- Spent by category, largest first: Housing $1,350.00 (1); Shopping $157.99 (1); Dining $41.12 (2).",
            "- Biggest expenses: 1. $1,350.00, Rent (Housing), Thu Oct 1, on Visa; 2. $157.99, Phone case (Shopping), Thu Oct 8, on Visa; 3. $26.14, Pho Lien (Dining), Thu Oct 15, on Visa.",
            "- Smallest expense: $14.98, no note (Dining), Fri Oct 2, on Visa.",
            "- Spent by week (Monday to Sunday): Mon Sep 28 to Sun Oct 4: $1,364.98 (2); Mon Oct 5 to Sun Oct 11: $157.99 (1); Mon Oct 12 to Sun Oct 18: $26.14 (1).",
            "- Spent by day, newest first (days with no expense left out): Thu Oct 15 $26.14 (1); Thu Oct 8 $157.99 (1); Fri Oct 2 $14.98 (1); Thu Oct 1 $1,350.00 (1).",
            "- #100 Thu Oct 15: Pho Lien, $26.14 expense, Dining, Visa",
            "- #122 Wed Oct 14: Salary, $2,150.00 income, Salary, Visa",
            "- #130 Wed Oct 7: To savings, $300.00 transfer, Visa to Savings",
            "- Transfers by the account they went into, largest first: Savings $300.00 (1).",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        assert!(!text.contains("Put into savings"), "without the accounts' kinds: {text}");
        // With them, what went into a savings or investment account from the others, added up;
        // not a card payment, and not one moved between two savings accounts.
        let mut moves = page();
        let rows = moves["transactions"].as_array_mut().unwrap();
        rows.push(json!({"id": 131, "type": "TRANSFER", "amount": 50000, "date": "2026-10-09", "account_id": 1, "account": "Chequing", "to_account_id": 2, "to_account": "Visa", "note": "Visa payment"}));
        rows.push(json!({"id": 132, "type": "TRANSFER", "amount": 25000, "date": "2026-10-10", "account_id": 1, "account": "Chequing", "to_account_id": 4, "to_account": "RRSP", "note": ""}));
        rows.push(json!({"id": 133, "type": "TRANSFER", "amount": 10000, "date": "2026-10-11", "account_id": 3, "account": "Savings", "to_account_id": 4, "to_account": "RRSP", "note": ""}));
        let lists = json!({"accounts": [{"id": 1, "name": "Chequing", "type": "CHECKING"}, {"id": 2, "name": "Visa", "type": "CREDIT"},
                                        {"id": 3, "name": "Savings", "type": "SAVINGS"}, {"id": 4, "name": "RRSP", "type": "INVESTMENT"}], "categories": []});
        let text = render("money.tx.list", &json!({}), &moves, &[Some(lists)], &ctx()).unwrap();
        for want in [
            "- Transfers by the account they went into, largest first: Visa $500.00 (1); RRSP $350.00 (2); Savings $300.00 (1).",
            "- Put into savings and investment accounts (from the other accounts): $550.00 in all (2): Savings $300.00 (1); RRSP $250.00 (1).",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let last = render("money.tx.list", &json!({"period_offset": -1, "category_id": 1}), &page(), &[], &ctx()).unwrap();
        assert!(last.contains(", last month.") && last.contains("Only entries category Dining"), "{last}");
        assert!(!text.contains("\"icon\"") && !text.contains("color"), "{text}");
        let mut days = page();
        days["period"] = json!({"start": "2026-10-02", "end_exclusive": "2026-10-09", "days": 7, "days_left": 0});
        let mut back = page();
        back["transactions"].as_array_mut().unwrap().push(tx(131, "INCOME", 5900, "2026-10-09", Some("Shopping"), "Remboursement Simons"));
        let lists = json!({"categories": [{"id": 7, "name": "Shopping", "kind": "EXPENSE"}, {"id": 14, "name": "Salary", "kind": "INCOME"}]});
        let text = render("money.tx.list", &json!({}), &back, &[Some(lists)], &ctx()).unwrap();
        for want in [
            "- Income by category, largest first: Salary $2,150.00 (1); Shopping $59.00 (1).",
            "- Money back (refunds and returns: income in Refunds, or filed under a spending category, which does not lower its spending): $59.00 in all, 1 entry: #131 Remboursement Simons, $59.00 (Shopping), Fri Oct 9.",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let ranged = render("money.tx.list", &json!({"from": "2026-10-02", "to": "2026-10-08"}), &days, &[], &ctx()).unwrap();
        assert!(ranged.starts_with("TALLY ENTRIES for Oct 2 to Oct 8, the days asked for, not a budget month."), "{ranged}");
    }

    /// A search for "refund" misses "Retour IKEA" filed under Shopping: the reading reads the same
    /// days unsearched and adds up all the money back in them.
    #[test]
    fn a_search_for_money_back_adds_up_all_of_it_over_its_days() {
        let mut all = page();
        all["transactions"].as_array_mut().unwrap().extend([tx(131, "INCOME", 5900, "2026-10-09", Some("Shopping"), "Retour IKEA"),
                                                             tx(132, "INCOME", 8640, "2026-10-03", Some("Refunds"), "Air Transat")]);
        let mut found = all.clone();
        found["transactions"] = json!([tx(132, "INCOME", 8640, "2026-10-03", Some("Refunds"), "Air Transat")]);
        let asked = json!({"from": "2026-10-01", "query": "refund"});
        assert_eq!(extras("money.tx.list", &asked, &found, ctx().today)[1], ("money.tx.list", json!({"from": "2026-10-01", "to": "2026-10-31"})));
        assert_eq!(extras("money.tx.list", &json!({"query": "metro", "from": "2026-10-01"}), &found, ctx().today).len(), 1, "only a search for money back");
        let lists = json!({"categories": [{"id": 7, "name": "Shopping", "kind": "EXPENSE"}, {"id": 9, "name": "Refunds", "kind": "INCOME"}]});
        let text = render("money.tx.list", &asked, &found, &[Some(lists), Some(all)], &ctx()).unwrap();
        let line = "- Money back over these days, whatever the search matched (refunds and returns: income in Refunds, or filed under a spending category, noted in any language): \
                    $145.40 in all, 2 entries: #131 Retour IKEA, $59.00 (Shopping), Fri Oct 9; #132 Air Transat, $86.40 (Refunds), Sat Oct 3.";
        assert!(text.contains(line), "{text}");
    }

    #[test]
    fn spending_is_added_up_by_the_account_it_was_paid_from() {
        let mut cash = page();
        let rows = cash["transactions"].as_array_mut().unwrap();
        for (id, amount) in [(140, 1500), (141, 7265)] {
            let mut t = tx(id, "EXPENSE", amount, "2026-10-03", Some("Groceries"), "Marché");
            t["account"] = json!("Cash");
            rows.push(t);
        }
        let text = render("money.tx.list", &json!({}), &cash, &[], &ctx()).unwrap();
        assert!(text.contains("- Spent by the account it was paid from, largest first: Visa $1,549.11 (4); Cash $87.65 (2).\n"), "{text}");
        assert!(!render("money.tx.list", &json!({}), &page(), &[], &ctx()).unwrap().contains("paid from"), "one account: its total is the month's");
    }

    #[test]
    fn workouts_are_counted_over_each_budget_month() {
        let (this, last) = (json!({"start": "2026-10-15", "end_exclusive": "2026-11-15"}), json!({"start": "2026-09-15", "end_exclusive": "2026-10-15"}));
        let line = workouts_by_budget_month(&[("this month", &this, &json!({"total": 1})), ("last month", &last, &json!({"total": 17, "untracked": 1}))], ctx().today).unwrap();
        assert_eq!(line, "BOTH APPS, EACH BUDGET MONTH (Avex's workouts counted over the days of Tally's month): this month (Oct 15 to Nov 14, so far) 1 workout; \
                          last month (Sep 15 to Oct 14) 16 workouts, and 1 the person excluded in Avex (untracked, not counted). \
                          Asked how many workouts in a budget month or its days, give the count here, not a calendar month's from Avex (other days).");
        assert_eq!(workouts_by_budget_month(&[], ctx().today), None);
        assert_eq!(workouts_by_budget_month(&[("this month", &this, &json!({"total": 0}))], ctx().today), None, "nothing imported");
    }

    #[test]
    fn a_lift_they_stopped_names_the_one_that_goes_on_in_its_place() {
        let lift = |name: &str, sessions: i64, last: &str| json!({"name": name, "sessions": sessions, "sets": sessions * 3, "last_day": last});
        let rows = vec![lift("Bench Press", 27, "2026-06-15"), lift("Dumbbell Bench Press", 11, "2026-10-12"), lift("Incline Dumbbell Press", 33, "2026-10-08"),
                        lift("Deadlift", 30, "2026-07-01"), lift("Romanian Deadlift", 38, "2026-06-20"), lift("Back Squat", 40, "2026-10-13")];
        let line = "Lifts they stopped, with a longer one of the same name still done (asked about the one, say how both go): \
                    Bench Press not done since Mon Jun 15, while Dumbbell Bench Press goes on (11 workouts, last on Mon Oct 12).";
        let text = render("gym.lifts", &json!({}), &json!({"unit": "kg", "lifts": rows.clone()}), &[], &ctx()).unwrap();
        assert!(text.contains(line), "a deadlift stopped before the Romanian one is not moved to it: {text}");
        let one = json!({"unit": "kg", "row": rows[0], "history": []});
        assert!(render("gym.lift.get", &json!({"name": "bench"}), &one, &[Some(json!({"lifts": rows}))], &ctx()).unwrap().contains(line));
        let other = json!({"unit": "kg", "row": lift("Deadlift", 30, "2026-07-01"), "history": []});
        assert!(!render("gym.lift.get", &json!({"name": "deadlift"}), &other, &[Some(json!({"lifts": [lift("Bench Press", 27, "2026-06-15"), lift("Dumbbell Bench Press", 11, "2026-10-12")]}))], &ctx()).unwrap().contains("Lifts they stopped"));
    }

    #[test]
    fn a_month_named_with_no_day_is_read_by_its_days() {
        let (today, held) = (ctx().today, "2026-09-15".parse::<Date>().unwrap());
        let d = |s: &str| s.parse::<Date>().unwrap();
        assert_eq!(months_named("while i was in portugal in june, did i work out?", today, held), vec![("June".into(), d("2026-06-01"), d("2026-06-30"))]);
        assert_eq!(months_named("refunds since january?", today, held), vec![("January".into(), d("2026-01-01"), today)]);
        assert_eq!(months_named("en décembre", today, held), vec![("December".into(), d("2025-12-01"), d("2025-12-31"))]);
        assert_eq!(months_named("what about in may 2025", today, held), vec![("May".into(), d("2025-05-01"), d("2025-05-31"))]);
        for none in ["from aug 15 to sep 14, how much?", "may I add 20 for lunch", "le 3 juin", "in october", "sept jours", "last month"] {
            assert_eq!(months_named(none, today, held), vec![], "{none}");
        }
    }

    /// A long range is added up by month, and lists only its newest entries, its figures whole.
    #[test]
    fn a_long_range_of_entries_reads_by_month_and_lists_the_newest() {
        let rows: Vec<Value> = (0..100).map(|i| {
            let day = jiff::civil::date(2026, 1, 1).checked_add(jiff::Span::new().days(i * 2)).unwrap();
            tx(1000 + i, "EXPENSE", 1000, &day.to_string(), Some("Dining"), "Lunch")
        }).rev().collect();
        let mut rows = rows;
        rows.push(tx(2000, "INCOME", 241250, "2026-01-29", Some("Salary"), "Salaire"));
        rows.push(tx(2001, "INCOME", 241250, "2026-01-15", Some("Salary"), "Salaire"));
        let r = json!({"period": {"start": "2026-01-01", "end_exclusive": "2026-07-20", "days": 200, "days_left": 0}, "income": 0, "spent": 100000, "transactions": rows});
        let text = render("money.tx.list", &json!({"from": "2026-01-01"}), &r, &[], &ctx()).unwrap();
        assert!(text.contains("- Spent (all expenses added up): $1,000.00"), "{text}");
        assert!(text.contains("- Spent by calendar month, oldest first (cut to the days asked for): January $160.00 (16); February $140.00 (14);"), "{text}");
        assert!(!text.contains("Spent by week") && !text.contains("Spent by day"), "{text}");
        assert!(text.contains("- Income by calendar month, oldest first (entries in brackets, then each category's): January $4,825.00 (2: Salary $4,825.00 (2))."), "{text}");
        assert!(text.contains("ENTRIES, the newest 80 of 102"), "{text}");
        assert!(text.contains("- #1099 ") && !text.contains("- #1019 "), "the newest are listed: {text}");
    }

    fn summary() -> Value {
        json!({
            "currency": "CAD", "fraction_digits": 2, "empty": false,
            "period": {"start": "2026-10-01", "end_exclusive": "2026-11-01", "days": 31, "days_left": 17},
            "pace": {"budget": 290000, "spent": 242526, "total_days": 31, "elapsed_days": 15, "expected": 140323, "remaining": 47474, "pace_delta": 102203,
                     "days_left": 17, "daily_allowance": 2792, "spent_fraction": 0.8363, "pace_fraction": 0.4839, "status": "OVER_PACE"},
            "income": 215000, "spent": 242526,
            "lines": {"margin": "$28 a day for 17 days", "pace": "$1,022 over pace", "versus_last": "$176 less than last month by this day"},
            "budgets": [
                {"category_id": 2, "name": "Dining", "budget": 32000, "spent": 33661, "left": -1661, "pace_delta": 18177, "status": "OVER_BUDGET", "line": "$16.61 over budget"},
                {"category_id": 1, "name": "Groceries", "budget": 50000, "spent": 38217, "left": 11783, "pace_delta": 14023, "status": "OVER_PACE", "line": "x"},
                {"category_id": 9, "name": "Entertainment", "budget": 8000, "spent": 0, "left": 8000, "pace_delta": -3871, "status": "UNDER_PACE", "line": "x"},
            ],
            "net_worth": 2226430,
            "accounts": [{"id": 1, "name": "Chequing", "type": "CHEQUING", "balance": 764320, "archived": false, "registration": null, "institution": ""},
                         {"id": 2, "name": "Visa", "type": "CREDIT", "balance": -307867, "archived": false, "registration": null, "institution": ""},
                         {"id": 6, "name": "Old", "type": "CASH", "balance": 0, "archived": true, "registration": null, "institution": ""}],
            "bills": [{"id": 2, "name": "Hydro", "amount": 7840, "type": "EXPENSE", "next_date": "2026-10-18", "days_until": 3},
                      {"id": 7, "name": "Salary", "amount": 215000, "type": "INCOME", "next_date": "2026-10-28", "days_until": 13},
                      {"id": 1, "name": "Rent", "amount": 135000, "type": "EXPENSE", "next_date": "2026-11-01", "days_until": 17}],
            "goals": [{"id": 2, "name": "Lisbon trip", "kind": "SAVINGS", "target": 240000, "saved": 70000, "line": "$283 a month for 6 months"}],
        })
    }

    /// "How much did I put aside last month" is answered by the summary itself.
    #[test]
    fn the_money_summary_adds_up_what_went_into_savings() {
        let lists = json!({"accounts": [{"id": 2, "name": "Visa", "type": "CREDIT"}, {"id": 3, "name": "Savings", "type": "SAVINGS"}], "categories": []});
        let text = render("money.summary", &json!({}), &summary(), &[Some(page()), Some(page()), None, None, None, None, Some(lists)], &ctx()).unwrap();
        let line = "Put into savings and investment accounts (from the other accounts): $300.00 in all (1): Savings $300.00 (1).";
        assert_eq!(text.matches(line).count(), 2, "this month and last: {text}");
    }

    #[test]
    fn the_money_summary_counts_the_pays_in_each_calendar_month() {
        let mut pays: Vec<Value> = ["2026-07-02", "2026-07-16", "2026-07-30", "2026-08-13", "2026-08-27", "2026-09-10", "2026-09-24", "2026-10-08"]
            .iter().enumerate().map(|(i, d)| tx(200 + i as i64, "INCOME", 215000, d, Some("Salary"), "Pay")).collect();
        // The search matched a note, not a pay.
        pays.push(tx(300, "EXPENSE", 1200, "2026-08-14", Some("Dining"), "Lunch, salary day"));
        let extras = [None, None, None, None, None, None, None, Some(json!({"transactions": pays, "more": false}))];
        let text = render("money.summary", &json!({}), &summary(), &extras, &ctx()).unwrap();
        assert!(text.contains("- Pays (Salary) in each month, the whole ledger: July 2026 3, August 2026 2, September 2026 2, October 2026 1 so far; months with 3 or more: July 2026.\n"), "{text}");
        let mut mid = summary();
        mid["period"]["start"] = json!("2026-10-15");
        mid["period"]["end_exclusive"] = json!("2026-11-15");
        let text = render("money.summary", &json!({}), &mid, &extras, &ctx()).unwrap();
        assert!(text.contains("- Pays (Salary), the whole ledger: 2 in most budget months, fewer in Jun 15 to Jul 14 2026 (1). Budget months with 3 or more: \
            Jul 15 to Aug 14 2026. Calendar months with 3 or more: July 2026.\n"), "{text}");
        let cut = [None, None, None, None, None, None, None, Some(json!({"transactions": [], "more": true}))];
        assert!(!render("money.summary", &json!({}), &summary(), &cut, &ctx()).unwrap().contains("Pays (Salary)"));
    }

    /// "This year", "ever", "when did my pay go up" and "what made that month so expensive" were
    /// answered from the three budget months a summary held: the whole ledger answers them.
    #[test]
    fn the_money_summary_reads_the_year_the_whole_ledger_and_the_pay() {
        let mut rows: Vec<Value> = (0..8).map(|i| {
            let d = jiff::civil::date(2026, 1, 8).checked_add(jiff::Span::new().days(14 * i)).unwrap();
            tx(200 + i, "INCOME", if i < 4 { 234000 } else { 241250 }, &d.to_string(), Some("Salary"), "Salaire")
        }).collect();
        rows.extend([tx(300, "EXPENSE", 52000, "2026-08-20", Some("Health"), "Couronne dentaire"), tx(301, "EXPENSE", 4500, "2026-03-02", None, "Virement"),
                     tx(302, "EXPENSE", 9000, "2025-11-02", None, "Dépanneur")]);
        let mut cash = tx(303, "TRANSFER", 14000, "2026-07-17", None, "Retrait guichet");
        (cash["to_account_id"], cash["to_account"]) = (json!(4), json!("Cash"));
        rows.push(cash);
        let spending = json!({"labels": ["August", "September", "October"], "series": [{"name": "Spending", "values": [52000, 36435, 242526]}]});
        let mut r = summary();
        r["accounts"].as_array_mut().unwrap().push(json!({"id": 4, "name": "Cash", "type": "CASH", "balance": 1000}));
        let extras = [None, None, None, Some(spending), None, None, None, Some(json!({"transactions": rows}))];
        let text = render("money.summary", &json!({}), &r, &extras, &ctx()).unwrap();
        for want in [
            "- Pay (Salary) per pay, the whole ledger (every 2 weeks, 26 pays a year): it went from $2,340.00 to $2,412.50 a pay (+$72.50 a pay, +$1,885.00 over a year of 26 pays) \
             with the pay of Thu Mar 5; the last at $2,340.00 was Thu Feb 19. Now $2,412.50 a pay.\n",
            "The costliest of these: Aug 1 to Aug 31 2026, $520.00; its biggest expenses: 1. $520.00, Couronne dentaire (Health), Thu Aug 20, on Visa.\n",
            "THIS YEAR (calendar year 2026, Jan 1 to today; the budget months above are other days):\n- Spent: $565.00 on 2 expenses. Income: $19,010.00 (8).\n\
             - Spent by calendar month: March $45.00; August $520.00.\n- Spent by category, largest first: Health $520.00 (1); No category $45.00 (1).\n",
            "- Taken out as cash (transfers into Cash: an ATM, a \"retrait\"): $140.00 (1).\n",
            "THE WHOLE LEDGER (every entry, since Sun Nov 2 2025): 12 entries: 3 expenses, 8 incomes, 1 transfer; spent $655.00; spent with no category: $135.00 (2 expenses).\n",
            "Cash taken out (an ATM, a \"retrait\") is a TRANSFER from the account it came out of to Cash, not an expense: what they buy with it later is.\n",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let same: Vec<Value> = (0..6).map(|i| tx(200 + i, "INCOME", 241250, &format!("2026-0{}-10", i + 3), Some("Salary"), "Salaire")).collect();
        let text = render("money.summary", &json!({}), &summary(), &[None, None, None, None, None, None, None, Some(json!({"transactions": same}))], &ctx()).unwrap();
        assert!(text.contains("- Pay (Salary) per pay, the whole ledger: $2,412.50 since Tue Mar 10 (every month, 12 a year): no raise or cut logged.\n"), "{text}");
    }

    /// "Costco this year" and an entry from months ago: a search with no days reads this budget
    /// month, and the same search over the whole ledger beside it.
    #[test]
    fn a_search_with_no_days_also_reads_the_whole_ledger() {
        let asked = json!({"query": "costco"});
        assert_eq!(extras("money.tx.list", &asked, &page(), ctx().today)[1], ("money.tx.list", json!({"from": "1970-01-01", "query": "costco"})));
        let none = json!({"period": page()["period"], "transactions": []});
        let all = json!({"transactions": [tx(150, "EXPENSE", 17168, "2026-09-06", Some("Groceries"), "Costco"), tx(120, "EXPENSE", 23019, "2026-01-04", Some("Groceries"), "Costco"),
                                          tx(40, "EXPENSE", 19000, "2025-12-07", Some("Groceries"), "Costco")]});
        let text = render("money.tx.list", &asked, &none, &[None, Some(all)], &ctx()).unwrap();
        for want in [
            "No entries: none logged in Tally in October 2026 (Oct 1 to Oct 31) that matches.\n",
            "IN THE WHOLE LEDGER (every entry ever, not only this budget month), matching \"costco\": 3 entries: 3 expenses, 0 incomes, 0 transfers.\n",
            "- Spent: $591.87 (3); this calendar year (2026, Jan 1 to today): $401.87 (2).\n",
            "- Spent by calendar year: 2025 $190.00 (1); 2026 $401.87 (2).\n",
            "Each of them, newest first (#id, day, note, amount, category, account):\n- #150 Sun Sep 6: Costco, $171.68 expense, Groceries, Visa\n",
            "- #40 Sun Dec 7 2025: Costco, $190.00 expense, Groceries, Visa",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let nothing = render("money.tx.list", &asked, &none, &[None, Some(json!({"transactions": []}))], &ctx()).unwrap();
        assert!(nothing.contains("no entry matches \"costco\" either. Notes are as the person typed them, often in French"), "{nothing}");
    }

    #[test]
    fn holidays_fall_on_their_days() {
        assert_eq!([easter(2024), easter(2025), easter(2026)], [jiff::civil::date(2024, 3, 31), jiff::civil::date(2025, 4, 20), jiff::civil::date(2026, 4, 5)]);
        let line = holidays_line(jiff::civil::date(2025, 6, 1));
        assert!(line.contains("Victoria Day (Journée des patriotes) Mon May 19; ") && line.contains("Labour Day Mon Sep 1; Thanksgiving Mon Oct 13;"), "{line}");
    }

    #[test]
    fn the_money_summary_says_which_kinds_of_account_need_asking() {
        let mut r = summary();
        r["accounts"] = json!([{"id": 3, "name": "Desjardins savings", "type": "SAVINGS", "balance": 155000}, {"id": 5, "name": "Tangerine savings", "type": "SAVINGS", "balance": 1100000},
                               {"id": 4, "name": "Visa", "type": "CREDIT", "balance": -2000}]);
        let text = render("money.summary", &json!({}), &r, &[], &ctx()).unwrap();
        assert!(text.contains("2 accounts of this kind: Desjardins savings, Tangerine savings. When they say only \"my savings\" or \"mon épargne\", it could be any of them: ask which before adding or changing anything."), "{text}");
        assert!(!text.contains("\"ma carte\""), "one card is no question: {text}");
    }

    #[test]
    fn the_money_summary_names_the_entries_with_no_category() {
        let mut month = page();
        month["transactions"].as_array_mut().unwrap().push(tx(140, "EXPENSE", 985, "2026-10-13", None, "Dépanneur Chez Ben"));
        let mut r = summary();
        r["duplicates"] = json!([[tx(141, "EXPENSE", 3240, "2026-10-12", Some("Dining"), "Uber Eats"), tx(142, "EXPENSE", 3240, "2026-10-12", Some("Dining"), "Uber Eats")]]);
        let text = render("money.summary", &json!({}), &r, &[Some(month)], &ctx()).unwrap();
        assert!(text.contains("Possible duplicates in the whole ledger, every entry compared (same day, amount, account and note: maybe logged twice): 1 pair: 1) #141 and #142 Uber Eats, $32.40 expense, Visa, Mon Oct 12 (this month).\nAsked about duplicates, name every pair here. When they ask to remove the extra one of a pair: thread.propose deleting its newer id (money.tx.delete), never adding or restoring."), "{text}");
        assert!(text.contains("- No category: $9.85 (1): #140 $9.85, Dépanneur Chez Ben, Tue Oct 13\n"), "{text}");
    }

    #[test]
    fn the_money_summary_is_a_brief_in_words() {
        let last = json!({"period": {"start": "2026-09-01", "end_exclusive": "2026-10-01"}, "transactions": [tx(50, "EXPENSE", 9000, "2026-09-03", Some("Dining"), "Bistro")]});
        let spending = json!({"labels": ["August", "September", "October"], "series": [{"name": "Spending", "values": [0, 364352, 242526]}]});
        let income = json!({"labels": ["August", "September", "October"], "series": [{"name": "Income", "values": [0, 645000, 215000]}]});
        let lists = json!({"accounts": [{"id": 1, "name": "Chequing", "type": "CHEQUING", "archived": false}, {"id": 4, "name": "Cash", "type": "CASH", "archived": false}],
                           "categories": [{"id": 2, "name": "Dining", "kind": "EXPENSE"}, {"id": 14, "name": "Salary", "kind": "INCOME"}]});
        let extras = [Some(page()), Some(last), None, Some(spending), Some(income), None, Some(lists)];
        let text = render("money.summary", &json!({}), &summary(), &extras, &ctx()).unwrap();
        for want in [
            "TALLY (budget app). Today is Thursday, October 15, 2026 (2026-10-15). Amounts are in CAD. Only what the person has logged in their Tally app: not their bank, not everything they did.\n",
            "THIS MONTH: October 2026 (Oct 1 to Oct 31), day 15 of 31, 17 days left counting today.",
            "- Last pay (Salary) came in on Wed Oct 14: Salary, $2,150.00. Spent since then (that day to today): $26.14 on 1 expense. Its entries: money.tx.list with from 2026-10-14.",
            "- Budgets last month, against each budget as it is set now: all spending $90.00 of $2,900.00, $2,810.00 left; Dining $90.00 of $320.00, $230.00 left; Groceries $0.00 of $500.00, $500.00 left; Entertainment $0.00 of $80.00, $80.00 left.",
            "- Spent so far (all expenses): $2,425.26",
            "- Monthly budget: $2,900.00. Left to spend: $474.74 (83.6% of the budget used).",
            "- Pace: $1,022.03 OVER pace (an even pace would be $1,403.23 spent by today).",
            "- To stay within the budget: $27.92 a day for the 17 days left.",
            "- Compared with last month at the same day: last month had spent $90.00 by Tue Sep 15 (its day 15); this month $2,425.26: $2,335.26 MORE than last month by this day.",
            "- Dining: $336.61 of $320.00. Over budget? YES, by $16.61.",
            "Over budget now: Dining.",
            "- Groceries: $382.17 of $500.00. Over budget? NO, $117.83 left. Ahead of pace? YES, by $140.23.",
            "- Entertainment: none logged in Tally this month, budget $80.00. Over budget? NO, $80.00 left. Ahead of pace? NO, under pace by $38.71.",
            "- money.tx.list's period_offset: 0 = October 2026, -1 = September 2026.",
            "- Weeks before (Monday to Sunday): Mon Sep 28 to Sun Oct 4: $1,364.98; Mon Sep 21 to Sun Sep 27: none logged in Tally;",
            "- Housing: $1,350.00 (1)",
            "Biggest expenses this month: 1. $1,350.00, Rent (Housing), Thu Oct 1, on Visa; 2. $157.99",
            "Smallest expense this month: $14.98, no note (Dining), Fri Oct 2, on Visa.",
            "DATES (\"Wednesday\" or \"last Saturday\" is the most recent one): today Thursday Oct 15 = 2026-10-15; yesterday Wednesday Oct 14 = 2026-10-14; Tuesday Oct 13 = 2026-10-13; Monday Oct 12 = 2026-10-12; last Sunday Oct 11 = 2026-10-11; last Saturday Oct 10 = 2026-10-10; last Friday Oct 9 = 2026-10-09.",
            "- Spent today, Thursday Oct 15: $26.14 on 1 expense",
            "- Spent yesterday, Wednesday Oct 14: none logged in Tally",
            "- This week so far (Mon Oct 12 to today): $26.14 on 1 expense",
            "- Last week (Mon Oct 5 to Sun Oct 11): $157.99 on 1 expense",
            "LAST MONTH: September 2026 (Sep 1 to Sep 30).",
            "- Spent: $90.00. Income: none logged in Tally.",
            "MONTH BY MONTH (budget periods, oldest first): Aug 1 to Aug 31 2026 (period_offset -2): none logged in Tally; Sep 1 to Sep 30 2026 (period_offset -1): spent $3,643.52, income $6,450.00; \
             Oct 1 to Oct 31 2026 (period_offset 0) so far: spent $2,425.26, income $2,150.00.",
            "HOLIDAYS 2026 (Canada, Québec): New Year's Day Thu Jan 1; Good Friday Fri Apr 3; Easter Monday Mon Apr 6; Victoria Day (Journée des patriotes) Mon May 18; \
             Fête nationale (Saint-Jean) Wed Jun 24; Canada Day Wed Jul 1; Labour Day Mon Sep 7; Thanksgiving Mon Oct 12; Remembrance Day Wed Nov 11; Christmas Fri Dec 25; Boxing Day Sat Dec 26.",
            "- Dining: last month (Sep 1 to Sep 30) $90.00; this month (Oct 1 to Oct 31) so far $41.12; these 2 months together $131.12, $65.56 a month on average",
            "- Housing: last month (Sep 1 to Sep 30) none logged in Tally; this month (Oct 1 to Oct 31) so far $1,350.00",
            "- Visa (credit card): -$3,078.67 owed",
            "- NET WORTH, all accounts together: $22,264.30",
            "- Hydro: $78.40 on Sun Oct 18 (in 3 days)",
            "Total of these bills: $1,428.40.",
            "Still to pay before this budget month ends (Sat Oct 31), each at its next date: $78.40 (1 bill). Left to spend after them: $474.74 − $78.40 = $396.34.",
            "Scheduled but not bills (income and transfers): Salary $2,150.00 income on Wed Oct 28 (in 13 days).",
            "- Lisbon trip (savings goal): $700.00 of $2,400.00. $283 a month for 6 months",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        assert!(!text.contains("Old"), "archived accounts are left out");
        // Without its extras it still reads, without the parts they give.
        let bare = render("money.summary", &json!({}), &summary(), &[None, None], &ctx()).unwrap();
        assert!(!bare.contains("LAST MONTH") && bare.contains("NET WORTH"));
        assert!(!text.lines().any(|l| l.ends_with(": $0.00")), "nothing logged reads as such, not as $0.00:\n{text}");
        assert!(!text.contains("account_id"), "ids are money.lists' to give, with which account is which");
    }

    #[test]
    fn lists_give_the_names_and_ids_writes_take() {
        let lists = json!({"currency": "CAD", "net_worth": 100,
                           "accounts": [{"id": 1, "name": "Chequing", "type": "CHEQUING", "balance": 100, "archived": false, "institution": ""},
                                        {"id": 4, "name": "Visa", "type": "CREDIT", "balance": 0, "archived": false, "institution": ""},
                                        {"id": 5, "name": "Mastercard", "type": "CREDIT", "balance": 0, "archived": false, "institution": ""},
                                        {"id": 7, "name": "Line of credit", "type": "CREDIT", "balance": 0, "archived": false, "institution": ""},
                                        {"id": 6, "name": "WS TFSA", "type": "INVESTMENT", "balance": 0, "archived": false, "registration": "TFSA", "institution": "wealthsimple "}],
                           "categories": [{"id": 2, "name": "Dining", "kind": "EXPENSE", "archived": false}, {"id": 3, "name": "Transport", "kind": "EXPENSE", "archived": false},
                                          {"id": 10, "name": "Subscriptions", "kind": "EXPENSE", "archived": false}, {"id": 11, "name": "Travel", "kind": "EXPENSE", "archived": false},
                                          {"id": 20, "name": "Climbing", "kind": "EXPENSE", "archived": false}, {"id": 14, "name": "Salary", "kind": "INCOME", "archived": false},
                                          {"id": 15, "name": "Side income", "kind": "INCOME", "archived": false}, {"id": 19, "name": "Freelance", "kind": "INCOME", "archived": false}]});
        let text = render("money.lists", &json!({}), &lists, &[], &ctx()).unwrap();
        for want in [
            "TALLY ACCOUNTS AND CATEGORIES (as set up in their Tally app) for money.tx.add and money.tx.update: each name, with its id beside it.",
            "- Chequing (account_id 1): chequing, balance $1.00\n",
            "- WS TFSA (account_id 6): investment, balance $0.00; filled from Wealthsimple's files: don't record activity here, the next export brings it",
            "2 accounts of this kind: Visa, Mastercard. When they say only \"my card\", \"the card\" or \"ma carte\", it could be any of them: ask which before adding or changing anything.",
            "EXPENSE CATEGORIES (name (category_id): what goes there):\n- Dining (category_id 2): restaurants, cafés, coffee, lunch, takeout, bars\n",
            "- Transport (category_id 3): transit, bus, metro, Bixi, taxi, Uber, gas, parking",
            "- Subscriptions (category_id 10): streaming and app plans, memberships billed every month",
            "- Travel (category_id 11): flights, hotels, trips away from home",
            "- Climbing (category_id 20): a category the person made, for what its name says\n",
            "INCOME CATEGORIES (name (category_id): what goes there):\n- Salary (category_id 14): pay from an employer",
            "- Side income (category_id 15): selling things\n",
            "- Freelance (category_id 19): a category the person made, for what its name says",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        // Every default category has its examples, and nothing else does.
        for c in &relay_money::model::DEFAULT_CATEGORIES {
            assert!(category_examples(c.name, &[]).is_some(), "{}", c.name);
        }
        assert_eq!(CATEGORY_EXAMPLES.len(), relay_money::model::DEFAULT_CATEGORIES.len());
        // What went in each category lately, newest first, the same words once.
        let invoice = |id: i64, note: &str| json!({"id": id, "type": "INCOME", "category_id": 19, "note": note});
        let this = json!({"transactions": [invoice(9, "Facture 14 — Café Lune"), invoice(8, "facture 14 — café lune")]});
        let last = json!({"transactions": [invoice(5, "Facture 13 — a logo and a menu for the bistro on the corner, two rounds"), invoice(4, "Facture 12"), invoice(3, "Facture 11")]});
        let text = render("money.lists", &json!({}), &lists, &[Some(this), Some(last), None], &ctx()).unwrap();
        assert!(text.contains("- Freelance (category_id 19): a category the person made, for what its name says; lately: \"Facture 14 — Café Lune\", \
                               \"Facture 13 — a logo and a menu for the bistro on…\", \"Facture 12\""), "{text}");
        assert!(text.contains("- Side income (category_id 15): selling things\n"), "{text}");
    }

    #[test]
    fn wealthsimple_accounts_say_not_to_record_activity_wherever_they_appear() {
        let mut s = summary();
        s["accounts"].as_array_mut().unwrap().push(json!({"id": 7, "name": "WS TFSA", "type": "INVESTMENT", "balance": 500000, "archived": false, "registration": "TFSA", "institution": "Wealthsimple"}));
        let text = render("money.summary", &json!({}), &s, &[], &ctx()).unwrap();
        assert!(text.contains("- WS TFSA (investment, TFSA, at Wealthsimple): $5,000.00; filled from Wealthsimple's files: don't record activity here, the next export brings it"), "{text}");
        assert_eq!(text.matches("filled from Wealthsimple").count(), 1, "one clause per account: {text}");
        let portfolio = json!({"currency": "CAD", "empty": false, "value": 500000, "book": 400000, "gain": 100000,
                               "accounts": [{"id": 7, "name": "WS TFSA", "registration": "TFSA", "institution": "Wealthsimple", "value": 500000, "book": 400000, "gain": 100000, "cash": 0, "holdings": 2},
                                            {"id": 8, "name": "Desjardins RRSP", "registration": "RRSP", "institution": "Desjardins", "value": 0, "book": 0, "gain": 0, "cash": 0, "holdings": 0}]});
        let text = render("money.invest.summary", &json!({}), &portfolio, &[None], &ctx()).unwrap();
        assert!(text.contains("- WS TFSA (TFSA, Wealthsimple): value $5,000.00, cost $4,000.00, gain $1,000.00, cash $0.00, 2 holdings; filled from Wealthsimple's files"), "{text}");
        assert_eq!(text.matches("filled from Wealthsimple").count(), 1, "{text}");
        let lists = json!({"accounts": [{"id": 7, "name": "WS TFSA", "type": "INVESTMENT", "archived": false, "institution": "Wealthsimple"},
                                        {"id": 8, "name": "Desjardins RRSP", "type": "INVESTMENT", "archived": false, "institution": "Desjardins"}]});
        let activity = json!({"activities": [{"id": 3, "account_id": 7, "account": "WS TFSA", "type": "BUY", "date": "2026-10-01", "symbol": "XEQT", "quantity": 200_000_000, "amount": 7624, "fee": 0, "currency": "CAD", "note": ""}]});
        assert_eq!(extras("money.invest.list", &json!({}), &activity, ctx().today).len(), 1);
        let text = render("money.invest.list", &json!({}), &activity, &[Some(lists)], &ctx()).unwrap();
        assert!(text.contains("- WS TFSA: filled from Wealthsimple's files: don't record activity here, the next export brings it."), "{text}");
        assert!(!text.contains("Desjardins"), "{text}");
    }

    #[test]
    fn nothing_logged_reads_as_nothing_logged_not_as_nothing_done() {
        let mut r = summary();
        r["income"] = json!(0);
        r["bills"] = json!([]);
        r["goals"] = json!([]);
        let empty = json!({"period": {"start": "2026-10-01", "end_exclusive": "2026-11-01"}, "transactions": []});
        let text = render("money.summary", &json!({}), &r, &[Some(empty.clone())], &ctx()).unwrap();
        for want in [
            "- Income so far: none logged in Tally\n", "SPENDING BY CATEGORY THIS MONTH (every category, largest first; entries in brackets):\n- none logged in Tally this month\n",
            "Entries this month: none logged in Tally yet.", "BILLS TO PAY: none set up in Tally.", "GOALS: none set up in Tally.",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let text = render("money.tx.list", &json!({"period_offset": -1}), &empty, &[], &ctx()).unwrap();
        assert!(text.contains("No entries: none logged in Tally in October 2026 (Oct 1 to Oct 31)."), "{text}");
        let text = render("money.tx.list", &json!({"query": "Provigo"}), &empty, &[], &ctx()).unwrap();
        assert!(text.contains("No entries: none logged in Tally in October 2026 (Oct 1 to Oct 31) that matches."), "{text}");
        let text = render("money.invest.list", &json!({}), &json!({"activities": []}), &[], &ctx()).unwrap();
        assert!(text.contains("No activities: none logged in Tally. Money moved into an investment or savings account from the bank is a transfer"), "{text}");
        let series = json!({"measure": "income", "by": "period", "labels": ["September", "October"], "series": [{"name": "Income", "values": [0, 0]}]});
        let text = render("money.series", &json!({"measure": "income"}), &series, &[], &ctx()).unwrap();
        assert!(text.contains("- Income: September none logged in Tally; October none logged in Tally. Total none logged in Tally."), "{text}");
        let text = render("gym.sessions", &json!({}), &json!({"sessions": [], "total": 0}), &[], &ctx()).unwrap();
        assert!(text.contains("Only what their Avex export holds: not everything they did.\nNo workouts in this Avex copy."), "{text}");
        let text = render("gym.cardio", &json!({}), &json!({"entries": []}), &[], &ctx()).unwrap();
        assert!(text.contains("No cardio and no rest days in this Avex copy."), "{text}");
        let text = render("gym.lifts", &json!({"query": "curl"}), &json!({"unit": "lb", "lifts": []}), &[], &ctx()).unwrap();
        assert!(text.contains("No lifts in this Avex copy matching \"curl\"."), "{text}");
        let r = json!({"imported": {"exported_at": "2026-10-08 21:14"}, "unit": "lb", "totals": {"sessions": 2, "cardio": 0}, "goals": []});
        let text = render("gym.summary", &json!({}), &r, &[None, Some(json!({"entries": []}))], &ctx()).unwrap();
        for want in ["lb lifted, no cardio in this Avex copy.", "Cardio in all: no cardio in this Avex copy.", "OPEN GOALS: none in this Avex copy."] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
    }

    #[test]
    fn investments_read_in_dollars_units_and_percents() {
        let r = json!({"currency": "CAD", "today": "2026-10-15", "empty": false, "as_of": "2026-08-08", "value": 163333, "book": 130000, "gain": 33333, "gain_bps": 2564,
                       "cash": 0, "income_12m": 0, "issues": [],
                       "accounts": [{"id": 6, "name": "Demo TFSA", "registration": "TFSA", "institution": "Wealthsimple", "value": 163333, "book": 130000, "gain": 33333, "gain_bps": 2564,
                                     "return_bps": 1210, "return_annual": true, "return_since": "2025-01-01", "cash": 0, "holdings": 1, "valued_on": "2026-08-08"}],
                       "allocation": [{"registration": "TFSA", "value": 163333, "share_bps": 10000}], "kinds": [],
                       "holdings": [{"account": "Demo TFSA", "symbol": "AAPL", "name": "Apple Inc", "kind": "STOCK", "currency": "USD", "quantity": 1_000_000_000_i64, "price": 10_000_000_000_i64,
                                     "value": 133333, "book": 100000, "gain": 33333, "gain_bps": 3333, "weight_bps": 8163, "no_price": false, "fx_estimated": true}],
                       "room": [{"registration": "TFSA", "year": 2026, "room": null, "contributed": 0, "withdrawn": 0, "left": null, "over": 0, "deadline": "2026-12-31"}]});
        let lists = json!({"accounts": [{"id": 5, "balance": 935000}]});
        let mut r = r;
        r["accounts"].as_array_mut().unwrap().push(json!({"id": 5, "name": "Sample TFSA", "registration": null, "institution": "", "value": 0, "book": 0, "gain": 0, "cash": 0, "holdings": 0}));
        let text = render("money.invest.summary", &json!({}), &r, &[Some(lists)], &ctx()).unwrap();
        for want in [
            "Values are as of Sat Aug 8 (68 days ago). These values are more than a month old",
            "- Gain: $333.33 (25.64%)",
            "money-weighted return 12.1% a year",
            "- AAPL (Apple Inc, stock) in Demo TFSA: 10 units at $100.00 = $1,333.33, 81.63% of the portfolio; cost $1,000.00, gain $333.33 (33.33%); converted at an estimated exchange rate",
            "By account type: TFSA $1,633.33 (100%).",
            "- TFSA 2026: room UNKNOWN: Tally does not have their CRA room figure yet",
            "- Valued by hand, at their balance (not in the figures above): Sample TFSA $9,350.00.",
            "- All investment accounts together, those included: $10,983.33.",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
    }

    fn session(id: i64, day: &str) -> Value {
        json!({"id": id, "day": day, "time": "18:32", "title": "Legs A", "session_type": "normal", "minutes": 44, "lifts": ["Back Squat", "Leg Press"], "skipped": 0,
               "sets": 12, "volume": 26185.0, "prs": 4, "mood": "good", "untracked": false})
    }

    fn lift(name: &str, weight: Option<f64>) -> Value {
        json!({"name": name, "sessions": 23, "sets": 69, "last_day": "2026-10-05",
               "best": weight.map(|w| json!({"lift": name, "day": "2026-10-05", "session_id": 147, "weight": w, "reps": 5, "e1rm": 250.8})),
               "recent_e1rm": 250.8, "change_pct": 4.9})
    }

    /// A kind with too many entries to list still names the ones with a note and the longest:
    /// "my 10k race" is a note, not a kind.
    #[test]
    fn a_long_cardio_kind_names_its_noted_entries_and_its_longest() {
        let run = |day: &str, km: f64, minutes: i64, note: &str| json!({"kind": "run", "day": day, "distance_km": km, "minutes": minutes, "note": note});
        let rows = [run("2026-09-13", 6.0, 33, ""), run("2026-09-06", 7.1, 40, ""), run("2026-05-24", 10.0, 52, "Course 10 km"), run("2026-05-17", 12.0, 70, "")];
        let mut o = String::new();
        cardio_kinds(&mut o, &rows, ctx().today);
        assert_eq!(o, "Cardio by kind, every entry read (each kind its own; a treadmill run is not a run): 4 runs, 3 h 15 min, 35.1 km \
                       (with a note: Sun May 24 10 km in 52 min \"Course 10 km\", then after it to today 2 runs, 13.1 km; longest Sun May 17 12 km in 1 h 10 min).\n");
    }

    #[test]
    fn the_gym_summary_says_how_old_the_copy_is_and_what_each_week_held() {
        let r = json!({"imported": {"exported_at": "2026-10-08 21:14", "days_per_week": 4}, "unit": "lb",
                       "totals": {"sessions": 48, "sets": 561, "volume": 622650.0, "cardio": 12, "cardio_minutes": 366, "first_day": "2026-07-13", "last_day": "2026-10-08"},
                       "this_week": {"start": "2026-10-12", "sessions": 0, "sets": 0, "volume": 0.0, "active_minutes": 0, "cardio_minutes": 0},
                       "last_week": {"start": "2026-10-05", "sessions": 3, "sets": 36, "volume": 46405.0, "active_minutes": 129, "cardio_minutes": 0},
                       "weeks": [{"start": "2026-10-05", "sessions": 3, "sets": 36, "volume": 46405.0, "active_minutes": 129, "cardio_minutes": 0}],
                       "streak_weeks": 13, "days_since_last": 7, "recent": [session(149, "2026-10-08")], "records": [], "lifts": [lift("Bench Press", Some(215.0))],
                       "goals": [{"kind": "lift_target", "target_key": "bench", "target_value": 225.0, "created_day": "2026-07-13", "note": "Two plates"}],
                       "breaks": [{"last_before": "2026-08-20", "first_after": "2026-09-05", "days": 16, "streak_before_weeks": 6}],
                       "marked": [{"kind": "deload", "sessions": 2, "days": ["2026-09-28", "2026-09-29"],
                                    "weeks": [{"start": "2026-09-28", "sessions": 2, "volume": 9000.0, "week_before_sessions": 3, "week_before_volume": 25344.0}]},
                                   {"kind": "first_back", "sessions": 1, "days": ["2026-09-05"]}],
                       "notes": [{"text": "Switched to dumbbells (shoulder).", "lift": "Dumbbell Bench Press", "day": "2026-09-05", "times": 1, "untracked": false},
                                 {"text": "Hotel gym", "day": "2026-08-20", "times": 2, "untracked": true}]});
        let mut pull_up = lift("Pull-up", None);
        pull_up["best_reps"] = json!(12);
        let mut deadlift = lift("Deadlift", Some(335.0));
        deadlift["heaviest"] = json!({"lift": "Deadlift", "day": "2026-09-26", "session_id": 140, "weight": 365.0, "reps": 1, "e1rm": 240.0});
        deadlift["heaviest_sessions"] = json!(3);
        deadlift["heaviest_last_day"] = json!("2026-10-03");
        let lifts = json!({"unit": "lb", "lifts": [lift("Bench Press", Some(215.0)), deadlift, lift("Dips", None), pull_up]});
        let cardio = json!({"entries": [{"day": "2026-10-03", "time": "08:00", "kind": "run", "minutes": 36, "distance_km": 5.65, "effort": "easy", "note": ""},
                                        {"day": "2026-08-30", "time": "08:00", "kind": "rest", "minutes": 0, "distance_km": null, "effort": "", "note": ""},
                                        {"day": "2026-08-20", "time": "08:00", "kind": "run", "minutes": 35, "distance_km": 5.5, "effort": "", "note": ""},
                                        {"day": "2026-08-12", "time": "08:00", "kind": "Bike", "minutes": 40, "distance_km": 15.0, "effort": "", "note": ""},
                                        {"day": "2026-08-05", "time": "08:00", "kind": "run", "minutes": 30, "distance_km": 5.0, "effort": "", "note": ""}]});
        let month = |values: [f64; 2]| json!({"labels": ["Aug", "Sep", "Oct"], "series": [{"values": [0.0, values[0], values[1]]}]});
        let detail = json!({"row": session(149, "2026-10-08"), "exercises": [
            {"name": "Back Squat", "sets": [{"weight": 142.0, "reps": 8, "kind": "warmup"}, {"weight": 285.0, "reps": 5, "kind": "work"}, {"weight": 285.0, "reps": 4, "kind": "work"}]},
            {"name": "Leg Press", "volume": 3990.0, "sets": [{"weight": 285.0, "reps": 5, "kind": "work"}, {"weight": 285.0, "reps": 5, "kind": "work"}, {"weight": 285.0, "reps": 4, "kind": "work"}]},
            {"name": "Dips", "volume": 0.0, "sets": [{"weight": null, "reps": 14, "kind": "work"}]}]});
        let sessions = json!({"sessions": [session(149, "2026-10-08"), session(147, "2026-10-05"), session(146, "2026-10-02")], "total": 3});
        let extras = [Some(lifts), Some(cardio), Some(month([15.0, 5.0])), Some(month([180000.0, 60000.0])), Some(month([0.0, 36.0])), Some(month([0.0, 5.65])), Some(sessions), None, Some(detail)];
        let text = render("gym.summary", &json!({}), &r, &extras, &ctx()).unwrap();
        for want in [
            "AVEX (gym app; weights in lb). Today is Thursday, October 15, 2026 (2026-10-15). Only what their Avex export holds: not everything they did.\n",
            "exported from the phone on Thu Oct 8 at 21:14 (7 days ago): anything logged after that is not here.",
            "ALL TIME (Mon Jul 13 to Thu Oct 8): 48 workouts, 561 sets, 622,650 lb lifted, 12 cardio sessions (366 minutes).",
            "THIS WEEK (Mon Oct 12 to today), so far: no workouts in this Avex copy, no cardio.",
            "LAST WEEK (Mon Oct 5 to Sun Oct 11): 3 workouts, 36 sets, 46,405 lb lifted, 129 active minutes, no cardio. Workouts: Mon Oct 5 Legs A; Thu Oct 8 Legs A.",
            "TODAY: no workout in this copy. It was exported on Thu Oct 8 at 21:14 (7 days ago), so a workout logged after that would not be here.",
            "These are the only lifts ever logged: Bench Press, Deadlift, Dips, Pull-up. Any other lift has never been logged.",
            "Last workout: Thu Oct 8 (7 days ago).",
            "Streak: 13 weeks in a row",
            "Longest breaks (last workout before, first one back): Thu Aug 20 to Sat Sep 5, 16 days, after 6 weeks in a row with a workout.",
            "Workouts Avex marks with a type (a deload, a test day…), how many and when: deload 2 (Mon Sep 28, Tue Sep 29; the week of Mon Sep 28 had 2 workouts, 9,000 lb lifted; the week before 3 workouts, 25,344 lb, so 16,344 lb less (64.5% less)); first back 1 (Sat Sep 5).",
            "What the person wrote in Avex (a workout's journal or a lift's note), newest first, the why behind a change: Sat Sep 5, on Dumbbell Bench Press: \"Switched to dumbbells (shoulder).\"; Thu Aug 20, the newest of 2 workouts with it, an untracked workout: \"Hotel gym\".",
            "- Thu Oct 8 18:32 (7 days ago): Legs A, 44 min, 12 sets, 26,185 lb lifted, 4 records; lifts: Back Squat, Leg Press; felt good [id 149]",
            "- Deadlift: best set 335 lb x 5 (152 kg) on Mon Oct 5, estimated 1-rep max 250.8 lb (113.8 kg); heaviest weight ever lifted 365 lb x 1 (165.6 kg) on Sat Sep 26 (365 lb reached in 3 workouts, the last on Sat Oct 3), heavier but not a better set: its estimated 1-rep max is 240 lb, the best set's 250.8 lb;",
            "- Dips: bodyweight or timed only",
            "- Pull-up: bodyweight, no weighted best set, so no estimated max; most reps in one set: 12;",
            "Most improved first (estimated max, last 4 weeks vs the 4 before): Bench Press +4.9%",
            "Cardio in all: 4 sessions, 141 minutes, 31.15 km.",
            "Cardio by kind, every entry read (each kind its own; a treadmill run is not a run): 3 runs, 1 h 41 min, 16.15 km (Sat Oct 3 5.65 km in 36 min; Thu Aug 20 5.5 km in 35 min; ",
            "1 bike ride, 40 min, 15 km (Wed Aug 12 15 km in 40 min).",
            "MONTH BY MONTH (calendar months, oldest first; for a budget month or any other days, gym.sessions with from and to counts them): Aug: nothing in this Avex copy; Sep: 15 workouts, 180,000 lb lifted, no cardio; Oct so far: 5 workouts, 60,000 lb lifted, 36 cardio minutes, 5.65 km.",
            "OPEN GOALS:\n- lift target bench",
            "LAST WORKOUT IN DETAIL (Thu Oct 8, Legs A: 12 sets in all, 26,185 lb lifted in all). A lift's volume is weight x reps added up over its working sets; warm-ups are not counted:",
            "- Back Squat: 2 working sets, volume 2,565 lb; 285 lb x 5, 285 lb x 4; plus 1 warm-up set not counted\n",
            "- Leg Press: 3 working sets, volume 3,990 lb; 285 lb x 5, 285 lb x 5, 285 lb x 4\n",
            "- Dips: 1 working set; bodyweight x 14\n",
            "CARDIO BY MONTH (calendar months, oldest first; each run counted; a month not listed has no cardio in this Avex copy):\n- August: 2 runs, 1 h 5 min, 10.5 km; 1 bike ride, 40 min, 15 km\n- October so far: 1 run, 36 min, 5.65 km\n",
            "Heaviest set of any lift: Deadlift 365 lb x 1 on Sat Sep 26.",
            "- lift target bench 225 lb, set Mon Jul 13 \"Two plates\"",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let none = render("gym.summary", &json!({}), &json!({"unit": "kg", "imported": null}), &[], &ctx()).unwrap();
        assert!(none.contains("Nothing has been imported yet"), "{none}");
    }

    #[test]
    fn fields_a_newer_avex_reading_carries_are_read_when_there() {
        let r = json!({"imported": {"exported_at": "2026-10-08 21:14"}, "unit": "lb",
                       "totals": {"sessions": 2, "untracked": 1, "sets": 24, "volume": 1000.0, "first_day": "2026-10-01", "last_day": "2026-10-08"},
                       "goals": [{"kind": "lift_target", "target_key": "bench", "target_value": 225.0, "created_day": "2026-07-13", "note": "",
                                  "best": {"lift": "Bench Press", "day": "2026-10-05", "weight": 215.0, "reps": 5, "e1rm": 250.8}}]});
        let text = render("gym.summary", &json!({}), &r, &[], &ctx()).unwrap();
        assert!(text.contains("1 workout the person excluded in Avex (untracked) is not counted."), "{text}");
        let year = [None, None, None, None, None, None, None, Some(json!({"sessions": [], "total": 120, "untracked": 2}))];
        let text = render("gym.summary", &json!({}), &r, &year, &ctx()).unwrap();
        assert!(text.contains("\nTHIS YEAR (2026, Jan 1 to today): 118 counted workouts, and 2 the person excluded (untracked, not counted).\n"), "{text}");
        let mut named = r.clone();
        named["totals"]["untracked"] = json!(3);
        named["totals"]["untracked_days"] = json!(["2026-08-08", "2026-10-03"]);
        let text = render("gym.summary", &json!({}), &named, &[], &ctx()).unwrap();
        assert!(text.contains("3 workouts the person excluded in Avex (untracked) are not counted; the newest 2: Sat Aug 8, Sat Oct 3."), "{text}");
        assert!(text.contains("- lift target bench 225 lb, set Mon Jul 13; best so far 215 lb x 5 on Mon Oct 5, estimated 1-rep max 250.8 lb"), "{text}");
        let detail = json!({"row": session(149, "2026-10-08"), "unit": "lb", "older_id": 148, "newer_id": null,
                            "exercises": [{"name": "Bench Press", "volume": 3010.0, "sets": [{"weight": 215.0, "reps": 7, "kind": "work"}, {"weight": 215.0, "reps": 7, "kind": "work"}],
                                           "previous": {"day": "2026-10-02", "top_weight": 190.0, "top_reps": 8}}]});
        let text = render("gym.session.get", &json!({"id": 149}), &detail, &[], &ctx()).unwrap();
        assert!(text.contains("Bench Press: 2 working sets, volume 3,010 lb; the time before, Fri Oct 2: 190 lb x 8"), "{text}");
    }

    #[test]
    fn workouts_and_sets_read_one_line_each() {
        let r = json!({"sessions": [session(149, "2026-10-08")], "total": 48});
        let text = render("gym.sessions", &json!({"limit": 1}), &r, &[], &ctx()).unwrap();
        assert!(text.contains("48 workouts in all; showing 1 to 1."), "{text}");
        let r = json!({"sessions": [session(149, "2026-10-08")], "total": 5, "untracked": 1});
        let text = render("gym.sessions", &json!({"from": "2026-10-01", "to": "2026-10-14"}), &r, &[], &ctx()).unwrap();
        assert!(text.contains("Only workouts from Thu Oct 1 to Wed Oct 14.\n5 workouts in all (4 counted, 1 the person excluded in Avex: untracked, marked below, not counted)"), "{text}");
        // A year of them is counted by month and the newest listed.
        let mut year: Vec<Value> = (0..60).map(|i| session(100 + i, &jiff::civil::date(2026, 10, 14).checked_sub(jiff::Span::new().days(3 * i)).unwrap().to_string())).collect();
        year[0]["untracked"] = json!(true);
        let text = render("gym.sessions", &json!({"from": "2026-01-01", "limit": 500}), &json!({"sessions": year, "total": 60, "untracked": 1}), &[], &ctx()).unwrap();
        assert!(text.contains("Counted workouts by calendar month, these 60 read: April 4; May 10; June 10; July 11; August 10; September 10; October 4.\nThe newest 50 of them"), "{text}");
        assert_eq!(text.matches("[id ").count(), 50, "{text}");
        assert!(text.contains("Counted workouts by kind, every one of these 60 read: Legs A 59, 44.0 min on average.\n"), "{text}");
        assert!(text.contains("From the first of these, Mon Apr 20, to today: 179 days (25.6 weeks), 59 counted workouts, 2.3 a week.\n"), "{text}");
        let detail = json!({"row": session(149, "2026-10-08"), "unit": "lb", "journal": "Gym was packed",
                            "exercises": [{"name": "Back Squat", "skipped": false, "was_pr": true, "difficulty": "EASY", "volume": 3990.0, "best_e1rm": 332.5,
                                           "sets": [{"weight": 142.0, "reps": 8, "rpe": null, "kind": "warmup", "best": false}, {"weight": 285.0, "reps": 5, "rpe": 8.0, "kind": "work", "best": true}]},
                                          {"name": "Dips", "skipped": false, "was_pr": false, "difficulty": "", "volume": 0.0, "best_e1rm": null, "sets": [{"weight": null, "reps": 14, "kind": "work"}]}]});
        let text = render("gym.session.get", &json!({"id": 149}), &detail, &[], &ctx()).unwrap();
        for want in [
            "Journal: Gym was packed", "Back Squat: 1 working set, volume 3,990 lb (plus 1 warm-up set not counted), best estimated 1-rep max 332.5 lb, a RECORD, felt easy",
            "  set 2: 285 lb x 5 (work), RPE 8, best set", "\nDips: 1 working set\n", "  set 1: bodyweight x 14 (work)",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let cardio = json!({"entries": [{"day": "2026-10-03", "time": "08:00", "kind": "run", "minutes": 36, "distance_km": 5.65, "effort": "easy", "note": ""},
                                        {"day": "2026-10-04", "time": "", "kind": "rest", "minutes": 0, "distance_km": null, "effort": "", "note": ""}]});
        let text = render("gym.cardio", &json!({}), &cardio, &[], &ctx()).unwrap();
        assert!(text.contains("1 cardio session, 36 minutes, 5.65 km in all; 1 rest days.") && text.contains("- Sat Oct 3 08:00: run, 36 min, 5.65 km, easy"), "{text}");
        assert!(text.contains("CARDIO BY MONTH (calendar months, oldest first; each run counted; a month not listed has no cardio in this Avex copy):\n- October so far: 1 run, 36 min, 5.65 km\n"), "{text}");
    }

    #[test]
    fn cardio_is_counted_month_by_month_and_says_when_the_list_was_cut() {
        let run = |day: &str, minutes: i64, km: f64| json!({"day": day, "time": "07:00", "kind": "run", "minutes": minutes, "distance_km": km, "effort": "", "note": ""});
        let mut entries = vec![run("2026-09-02", 30, 5.0), json!({"day": "2026-08-31", "time": "", "kind": "swim", "minutes": 25, "distance_km": null, "effort": "", "note": ""})];
        entries.extend([("2026-08-29", 32, 5.2), ("2026-08-22", 35, 5.4), ("2026-08-15", 31, 5.1), ("2026-08-08", 33, 5.3), ("2026-08-01", 30, 5.1)].map(|(d, m, k)| run(d, m, k)));
        entries.push(json!({"day": "2025-12-30", "time": "", "kind": "elliptical", "minutes": 20, "distance_km": null, "effort": "", "note": ""}));
        let r = json!({"entries": entries});
        let text = render("gym.cardio", &json!({}), &r, &[], &ctx()).unwrap();
        for want in [
            "- December 2025: 1 elliptical session, 20 min\n",
            "- August: 5 runs, 2 h 41 min, 26.1 km; 1 swim, 25 min\n",
            "- September: 1 run, 30 min, 5 km\n",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        assert!(!text.contains("may have more"), "{text}");
        // Cut at the limit asked: the oldest month may be missing entries, and says so.
        let cut = render("gym.cardio", &json!({"limit": 8}), &r, &[], &ctx()).unwrap();
        assert!(cut.contains("(the newest 8 entries only: a larger limit reads older ones)"), "{cut}");
        assert!(cut.contains("- December 2025: 1 elliptical session, 20 min (older entries were not read, so this month may have more)"), "{cut}");
        assert_eq!(duration(60), "1 h");
    }

    #[test]
    fn a_lift_done_only_over_12_reps_says_it_has_no_estimated_max() {
        let mut lateral = lift("Lateral Raise", Some(45.0));
        lateral["best"]["reps"] = json!(15);
        lateral["best"]["e1rm"] = Value::Null;
        lateral["change_pct"] = Value::Null;
        let lifts = json!({"unit": "lb", "lifts": [lift("Bench Press", Some(215.0)), lateral.clone()]});
        let text = render("gym.lifts", &json!({}), &lifts, &[], &ctx()).unwrap();
        for want in [
            "- Lateral Raise: best set 45 lb x 15 (20.4 kg) on Mon Oct 5, no estimated max: every set was over 12 reps (an estimate needs a set of 1 to 12 reps); done in",
            "No estimated max, because every set was over 12 reps (never give these another lift's estimate): Lateral Raise.",
        ] {
            assert!(text.contains(want), "missing {want:?} in\n{text}");
        }
        let r = json!({"imported": {"exported_at": "2026-10-08 21:14"}, "unit": "lb", "totals": {}});
        let summary = render("gym.summary", &json!({}), &r, &[Some(lifts)], &ctx()).unwrap();
        assert!(summary.contains("- Lateral Raise: best set 45 lb x 15 (20.4 kg) on Mon Oct 5, no estimated max: every set was over 12 reps"), "{summary}");
        let detail = json!({"row": lateral, "unit": "lb", "history": [{"day": "2026-10-05", "session_id": 147, "top_weight": 45.0, "top_reps": 15, "e1rm": null, "sets": 3, "volume": 2025.0, "record": false}]});
        let text = render("gym.lift.get", &json!({"name": "lateral"}), &detail, &[], &ctx()).unwrap();
        assert!(text.contains("- Lateral Raise: best set 45 lb x 15 (20.4 kg) on Mon Oct 5, no estimated max: every set was over 12 reps"), "{text}");
        assert!(text.contains("- Mon Oct 5 (10 days ago): 45 lb x 15, no estimated max (over 12 reps), 3 sets, 2,025 lb volume"), "{text}");
        let workout = json!({"row": session(147, "2026-10-05"), "unit": "lb",
                             "exercises": [{"name": "Lateral Raise", "volume": 2025.0, "best_e1rm": null, "sets": [{"weight": 45.0, "reps": 15, "kind": "work"}, {"weight": 45.0, "reps": 15, "kind": "work"}, {"weight": 45.0, "reps": 15, "kind": "work"}]}]});
        let text = render("gym.session.get", &json!({"id": 147}), &workout, &[], &ctx()).unwrap();
        assert!(text.contains("Lateral Raise: 3 working sets, volume 2,025 lb, no estimated max: every set was over 12 reps"), "{text}");
    }

    #[test]
    fn a_series_comes_with_the_chart_block_that_draws_it() {
        let series = json!({"currency": "CAD", "measure": "spending", "by": "week", "labels": ["Week 1", "Week 2"], "series": [{"name": "September", "values": [15799, 1000]}]});
        let text = render("money.series", &json!({"by": "week", "periods": 1, "category": "Groceries", "today": "2026-10-15"}), &series, &[], &ctx()).unwrap();
        assert!(text.contains("- September: Week 1 $157.99; Week 2 $10.00. Total $167.99."), "{text}");
        let periods = "periods counts months (budget periods), even when by is week or day: \"the last 2 months by week\" is {\"by\": \"week\", \"periods\": 2}";
        assert!(text.contains(periods), "{text}");
        assert!(description("money.series", ctx().today).unwrap().contains(periods));
        let block = text.split("```chart\n").nth(1).and_then(|b| b.split("\n```").next()).expect("a chart block");
        let chart: Value = serde_json::from_str(block).unwrap();
        assert_eq!(chart, json!({"type": "bar", "title": "Groceries spending by week", "query": {"by": "week", "periods": 1, "category": "Groceries"}}));
        let gym = json!({"measure": "e1rm", "by": "week", "lift": "Bench Press", "unit": "lb", "labels": ["Oct 5"], "series": [{"name": "Bench Press", "values": [250.8]}]});
        let text = render("gym.series", &json!({"measure": "e1rm", "lift": "Bench", "by": "week", "periods": 12}), &gym, &[], &ctx()).unwrap();
        let block = text.split("```chart\n").nth(1).and_then(|b| b.split("\n```").next()).expect("a chart block");
        let chart: Value = serde_json::from_str(block).unwrap();
        assert_eq!(chart, json!({"type": "line", "title": "Bench Press, estimated max by week", "gym": {"measure": "e1rm", "lift": "Bench", "by": "week", "periods": 12}}));
        assert!(relay_client::thread_view::chart_spec(block).is_some(), "a chart the thread draws");
    }

    #[test]
    fn every_op_with_a_reading_has_a_renderer_and_others_have_none() {
        for op in OPS {
            assert!(render(op, &json!({}), &json!({}), &[], &ctx()).is_some(), "{op}");
        }
        assert!(render("money.tx.add", &json!({}), &json!({}), &[], &ctx()).is_none());
    }
}
