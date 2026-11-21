//! The Investments screen's pure half (Kotlin: ui/invest/InvestLogic.kt): registrations and kinds
//! of security as words and hues, the room meter's pace, the sentences the page prints, and the
//! reading a kind lens narrows the page to. The numbers themselves are relay-money's
//! `Ledger::portfolio`, the same reading the PC makes.

use std::collections::HashMap;

use relay_money::Date;
use relay_money::copy;
use relay_money::invest::{self, ActivityRow, HoldingView, Portfolio, PortfolioAccount, PortfolioInput, RoomView};
use relay_money::model::{ActivityType, AccountType, Registration, SecurityKind};
use relay_money::money::MoneyFormatter;

use crate::accounts_store::AccountRow;

/// A figure older than this many days reads as stale.
pub const STALE_DAYS: i64 = 31;

/// The registrations with a yearly room Tally reads against a CRA figure.
pub const ROOM_KINDS: [Registration; 3] = [Registration::Tfsa, Registration::Rrsp, Registration::Fhsa];

/// A registration as words; none is an investment account whose kind was never set.
pub fn registration_label(r: Option<Registration>) -> &'static str {
    r.map(Registration::label).unwrap_or("Kind not set")
}

/// A registration by its stored or spoken name ("TFSA", "NON_REGISTERED").
pub fn registration_named(name: &str) -> Option<Registration> {
    let n = name.trim().to_uppercase().replace(['-', ' '], "_");
    [
        Registration::NonRegistered,
        Registration::Tfsa,
        Registration::Rrsp,
        Registration::Fhsa,
        Registration::Resp,
        Registration::Lira,
        Registration::Rrif,
        Registration::Other,
    ]
    .into_iter()
    .find(|r| r.label().to_uppercase().replace(['-', ' '], "_") == n || crate::accounts_store::registration_name(*r) == n)
}

pub fn kind_label(k: SecurityKind) -> &'static str {
    match k {
        SecurityKind::Stock => "Stocks",
        SecurityKind::Etf => "ETFs",
        SecurityKind::MutualFund => "Funds",
        SecurityKind::Bond => "Bonds",
        SecurityKind::Crypto => "Crypto",
        SecurityKind::Cash => "Cash",
        SecurityKind::Other => "Other",
    }
}

/// Account kinds and kinds of security each take hues of their own from the category palette.
pub fn registration_hue(r: Option<Registration>) -> i64 {
    match r {
        Some(Registration::Tfsa) => 0,
        Some(Registration::Resp) => 1,
        Some(Registration::Rrsp) => 2,
        Some(Registration::Fhsa) => 3,
        Some(Registration::Other) => 5,
        Some(Registration::Lira) => 8,
        Some(Registration::NonRegistered) => 9,
        Some(Registration::Rrif) => 11,
        None => 10,
    }
}

pub fn kind_hue(k: SecurityKind) -> i64 {
    match k {
        SecurityKind::Etf => 4,
        SecurityKind::Other => 5,
        SecurityKind::Stock => 6,
        SecurityKind::Crypto => 7,
        SecurityKind::MutualFund => 8,
        SecurityKind::Cash => 10,
        SecurityKind::Bond => 11,
    }
}

/// Units held, as people write them: "12.5", "0.0042", "1200".
pub fn quantity_text(quantity: i64) -> String {
    if quantity == 0 {
        return "0".into();
    }
    let sign = if quantity < 0 { "-" } else { "" };
    let q = quantity.unsigned_abs();
    let (whole, frac) = (q / 100_000_000, q % 100_000_000);
    let frac = format!("{frac:08}");
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() { format!("{sign}{whole}") } else { format!("{sign}{whole}.{frac}") }
}

/// "12.5 shares" for a stock or a fund on an exchange, "0.0042 units" for anything else.
pub fn units_line(quantity: i64, kind: SecurityKind) -> String {
    let text = quantity_text(quantity);
    let one = text == "1";
    match kind {
        SecurityKind::Stock | SecurityKind::Etf => format!("{text} {}", if one { "share" } else { "shares" }),
        _ => format!("{text} {}", if one { "unit" } else { "units" }),
    }
}

/// Basis points as a percent to one place: "8.1%", signed when asked; a loss always carries its
/// minus.
pub fn percent_text(bps: i64, signed: bool) -> String {
    let tenths = (bps.abs() + 5) / 10;
    let body = format!("{}.{}%", tenths / 10, tenths % 10);
    if tenths == 0 {
        body
    } else if bps < 0 {
        format!("−{body}")
    } else if signed {
        format!("+{body}")
    } else {
        body
    }
}

/// A share in whole percent; a sliver reads "under 1%" rather than a false zero.
pub fn share_percent(bps: i64) -> String {
    if bps <= 0 {
        "0%".into()
    } else if bps < 50 {
        "under 1%".into()
    } else {
        format!("{}%", (bps + 50) / 100)
    }
}

pub fn share_bps(part: i64, total: i64) -> i64 {
    if total <= 0 { 0 } else { invest::mul_div_half_even(part, 10_000, total).unwrap_or(0) }
}

/// A gain with its sign written out, money and percent: "+$412 · +8.1%".
pub fn gain_text(gain: i64, gain_bps: Option<i64>, fmt: &MoneyFormatter) -> String {
    let amount = if gain > 0 { format!("+{}", fmt.format_whole(gain)) } else { fmt.format_whole(gain) };
    match gain_bps {
        Some(b) => format!("{amount} · {}", percent_text(b, true)),
        None => amount,
    }
}

/// "11 Oct": a short day, as the Kotlin app's `Dates.short`.
pub fn short(d: Date, today: Date) -> String {
    if d.year() == today.year() { d.strftime("%-d %b").to_string() } else { d.strftime("%-d %b %Y").to_string() }
}

/// "31 December 2026".
pub fn long(d: Date) -> String {
    d.strftime("%-d %B %Y").to_string()
}

pub fn is_stale(newest: Date, today: Date) -> bool {
    (today - newest).get_days() as i64 > STALE_DAYS
}

/// The page's context line: "3 accounts · valued 9 Oct", "· last valued 12 Aug" once stale.
pub fn context(accounts: usize, newest: Option<Date>, today: Date) -> String {
    let count = copy::plural(accounts as i64, "account");
    match newest {
        None => format!("{count} · no values yet"),
        Some(n) if is_stale(n, today) => format!("{count} · last valued {}", short(n, today)),
        Some(n) => format!("{count} · valued {}", short(n, today)),
    }
}

/// How old the newest figure is, said plainly.
pub fn stale_line(newest: Date, today: Date) -> String {
    let days = (today - newest).get_days() as i64;
    let months = days / 30;
    if months >= 2 { format!("{months} months since the last report") } else { format!("{} since the last report", copy::plural(days, "day")) }
}

// Room

pub fn room_deadline(r: Registration, year: i32) -> Date {
    invest::room_window(r, year).1
}

pub fn room_start(r: Registration, year: i32) -> Date {
    invest::room_window(r, year).0
}

/// The room meter's tick: where an even pace to use the year's room by its deadline stands today.
pub fn room_pace_fraction(r: Registration, year: i32, today: Date) -> f32 {
    let start = room_start(r, year);
    let days = (room_deadline(r, year) - start).get_days() as i64 + 1;
    let elapsed = ((today - start).get_days() as i64 + 1).clamp(0, days);
    elapsed as f32 / days.max(1) as f32
}

/// What each month needs from here, this one included, to use `left` by `deadline`, rounded up to
/// a whole `unit`; none once nothing is left or the deadline passed.
pub fn monthly_to_fill(left: i64, today: Date, deadline: Date, unit: i64) -> Option<i64> {
    if left <= 0 || deadline < today {
        return None;
    }
    let months = (deadline.year() as i64 * 12 + deadline.month() as i64) - (today.year() as i64 * 12 + today.month() as i64) + 1;
    let each = (left + months - 1) / months;
    let unit = unit.max(1);
    Some((each + unit - 1) / unit * unit)
}

/// The room row's line: what is left and the monthly pace that uses it, or how far over it went.
pub fn room_text(line: &RoomView, today: Date, fmt: &MoneyFormatter) -> String {
    let Some(_) = line.room else { return format!("Add your {} room from CRA My Account", line.year) };
    let left = line.left.unwrap_or(0);
    if line.over > 0 && line.over_taxed == 0 {
        format!("Over by {}, inside the {} an RRSP may go over", fmt.format_whole(line.over), fmt.format_whole(invest::RRSP_BUFFER))
    } else if line.over > 0 {
        format!("Over by {}. The CRA charges 1% a month on {}", fmt.format_whole(line.over), fmt.format_whole(line.over_taxed))
    } else if left == 0 {
        format!("The {} room is used", line.year)
    } else {
        let deadline = room_deadline(line.registration, line.year);
        let unit = relay_money::money::pow10(fmt.fraction_digits);
        let monthly = monthly_to_fill(left, today, deadline, unit).map(|m| format!(" · {} a month uses it by {}", fmt.format_whole(m), short(deadline, today))).unwrap_or_default();
        format!("{} left{monthly}", fmt.format_whole(left))
    }
}

/// The room row's reading at its end: "$4,200 of $7,000", or "$4,200 in" with no room set.
pub fn room_reading(line: &RoomView, fmt: &MoneyFormatter) -> String {
    match line.room {
        None => format!("{} in", fmt.format_whole(line.contributed)),
        Some(r) => copy::of_budget(line.contributed, r, fmt),
    }
}

/// Where the owner finds a registration's room, and what Tally does with it.
pub fn room_source(r: Registration) -> &'static str {
    match r {
        Registration::Rrsp => {
            "Your RRSP deduction limit is on your latest Notice of Assessment and in CRA My Account. Tally takes off what goes in between last year's deadline and this one."
        }
        Registration::Fhsa => "Your FHSA participation room is in CRA My Account. Tally takes off what goes in this year.",
        _ => {
            "Your room on 1 January is in CRA My Account. Tally takes off what goes in this year. The yearly limit is your room only if last year's was used up; what comes out returns next year."
        }
    }
}

// The reading

/// One investment account as the page reads it: from its holdings when it has any, else from the
/// value recorded by hand.
#[derive(Clone, Debug, PartialEq)]
pub struct InvestLine {
    pub id: i64,
    pub name: String,
    pub registration: Option<Registration>,
    pub worth: i64,
    pub cost: i64,
    pub cash: i64,
    pub holdings: usize,
    pub by_hand: bool,
    pub valued_on: Option<Date>,
    pub return_bps: Option<i64>,
    pub return_annual: bool,
    pub return_since: Option<Date>,
}

fn day(s: Option<&str>) -> Option<Date> {
    s.and_then(|s| s.parse().ok())
}

/// The open investment accounts, each read from the portfolio when it holds something there,
/// else from its balance. A report older than a value recorded by hand is set aside first
/// ([`set_aside_older_reports`]), so such an account reads from its balance here too.
pub fn invest_lines(portfolio: Option<&Portfolio>, rows: &[AccountRow]) -> Vec<InvestLine> {
    rows.iter()
        .filter(|r| r.view.r#type == AccountType::Investment && !r.view.archived)
        .map(|b| {
            let p = portfolio.and_then(|p| p.accounts.iter().find(|a| a.id == b.view.id));
            match p {
                Some(p) if priced(p) => InvestLine {
                    id: b.view.id,
                    name: b.view.name.clone(),
                    registration: p.registration,
                    worth: p.value,
                    cost: p.book,
                    cash: p.cash,
                    holdings: p.holdings,
                    by_hand: false,
                    valued_on: day(p.valued_on.as_deref()),
                    return_bps: p.return_bps,
                    return_annual: p.return_annual,
                    return_since: day(p.return_since.as_deref()),
                },
                _ => InvestLine {
                    id: b.view.id,
                    name: b.view.name.clone(),
                    registration: p.map(|p| p.registration).unwrap_or(b.view.registration),
                    // As net worth reads it: one given nowhere to start by its entries
                    // (accounts_store::worth_by_entries).
                    worth: b.worth,
                    cost: 0,
                    cash: 0,
                    holdings: 0,
                    by_hand: true,
                    valued_on: b.valued_on,
                    return_bps: p.and_then(|p| p.return_bps),
                    return_annual: p.is_some_and(|p| p.return_annual),
                    return_since: day(p.and_then(|p| p.return_since.as_deref())),
                },
            }
        })
        .collect()
}

/// One kind the lens can narrow the page to.
#[derive(Clone, Debug, PartialEq)]
pub struct Lens {
    pub label: &'static str,
    pub registration: Option<Registration>,
}

/// The kinds held, in the ledger's order with the unset ones last; empty below two kinds.
pub fn lenses(lines: &[InvestLine]) -> Vec<Lens> {
    let mut held: Vec<Option<Registration>> = Vec::new();
    for l in lines {
        if !held.contains(&l.registration) {
            held.push(l.registration);
        }
    }
    if held.len() < 2 {
        return Vec::new();
    }
    held.sort_by_key(|r| (r.is_none(), *r));
    held.into_iter().map(|r| Lens { label: registration_label(r), registration: r }).collect()
}

/// One segment of an allocation bar and its legend row; `hue` indexes the category palette.
#[derive(Clone, Debug, PartialEq)]
pub struct ShareLine {
    pub label: &'static str,
    pub hue: i64,
    pub value: i64,
    pub share_bps: i64,
}

/// Everything the page draws for the lens picked.
#[derive(Clone, Debug, Default)]
pub struct InvestView {
    pub lenses: Vec<Lens>,
    pub lens: Option<Lens>,
    pub accounts: Vec<InvestLine>,
    pub worth: i64,
    pub priced: i64,
    pub cost: i64,
    pub cash: i64,
    pub newest: Option<Date>,
    pub holdings: Vec<HoldingView>,
    pub allocation: Vec<ShareLine>,
    pub kinds: Vec<ShareLine>,
    pub income_12m: Option<i64>,
    pub room: Vec<RoomView>,
    pub issues: Vec<String>,
}

impl InvestView {
    pub fn gain(&self) -> i64 {
        self.priced - self.cost
    }
    pub fn gain_bps(&self) -> Option<i64> {
        (self.cost > 0).then(|| invest::mul_div_half_even(self.gain(), 10_000, self.cost)).flatten()
    }
    pub fn has_holdings(&self) -> bool {
        self.accounts.iter().any(|a| !a.by_hand)
    }
}

/// The page for a lens (`lens` none: every account; a lens no longer held reads every account).
pub fn view(portfolio: Option<&Portfolio>, rows: &[AccountRow], lens: Option<Option<Registration>>) -> InvestView {
    let all = invest_lines(portfolio, rows);
    let lenses = lenses(&all);
    let lens = lens.and_then(|r| lenses.iter().find(|l| l.registration == r).cloned());
    let lines: Vec<InvestLine> = match &lens {
        None => all,
        Some(l) => all.into_iter().filter(|a| a.registration == l.registration).collect(),
    };
    let ids: Vec<i64> = lines.iter().map(|l| l.id).collect();
    let priced: Vec<&InvestLine> = lines.iter().filter(|l| !l.by_hand).collect();
    let holdings: Vec<HoldingView> = portfolio.map(|p| p.holdings.iter().filter(|h| ids.contains(&h.account_id)).cloned().collect()).unwrap_or_default();
    let cash = priced.iter().map(|l| l.cash).sum();
    let newest = lines.iter().filter_map(|l| l.valued_on).max().or(if lens.is_none() { day(portfolio.and_then(|p| p.as_of.as_deref())) } else { None });
    InvestView {
        worth: lines.iter().map(|l| l.worth).sum(),
        priced: priced.iter().map(|l| l.worth).sum(),
        cost: priced.iter().map(|l| l.cost).sum(),
        cash,
        newest,
        allocation: if lens.is_none() { allocation_lines(&lines) } else { Vec::new() },
        kinds: kind_lines(&holdings, cash),
        holdings,
        income_12m: if lens.is_none() { portfolio.map(|p| p.income_12m) } else { None },
        room: portfolio.map(|p| p.room.iter().filter(|r| lens.as_ref().is_none_or(|l| l.registration == Some(r.registration))).cloned().collect()).unwrap_or_default(),
        issues: portfolio.map(|p| p.issues.clone()).unwrap_or_default(),
        accounts: lines,
        lenses,
        lens,
    }
}

/// What each account kind is worth, largest first, by-hand values included.
pub fn allocation_lines(lines: &[InvestLine]) -> Vec<ShareLine> {
    let total: i64 = lines.iter().filter(|l| l.worth > 0).map(|l| l.worth).sum();
    let mut by: Vec<(Option<Registration>, i64)> = Vec::new();
    for l in lines.iter().filter(|l| l.worth > 0) {
        match by.iter_mut().find(|(r, _)| *r == l.registration) {
            Some((_, v)) => *v += l.worth,
            None => by.push((l.registration, l.worth)),
        }
    }
    let mut out: Vec<ShareLine> = by.into_iter().map(|(r, v)| ShareLine { label: registration_label(r), hue: registration_hue(r), value: v, share_bps: share_bps(v, total) }).collect();
    out.sort_by_key(|l| std::cmp::Reverse(l.value));
    out
}

/// What each kind of security is worth, the accounts' cash counted as cash, largest first.
pub fn kind_lines(holdings: &[HoldingView], cash: i64) -> Vec<ShareLine> {
    let mut by: Vec<(SecurityKind, i64)> = Vec::new();
    let mut add = |k: SecurityKind, v: i64| match by.iter_mut().find(|(kk, _)| *kk == k) {
        Some((_, t)) => *t += v,
        None => by.push((k, v)),
    };
    for h in holdings {
        add(h.kind, h.value);
    }
    if cash != 0 {
        add(SecurityKind::Cash, cash);
    }
    let positive: Vec<(SecurityKind, i64)> = by.into_iter().filter(|(_, v)| *v > 0).collect();
    let total = positive.iter().map(|(_, v)| v).sum();
    let mut out: Vec<ShareLine> = positive.into_iter().map(|(k, v)| ShareLine { label: kind_label(k), hue: kind_hue(k), value: v, share_bps: share_bps(v, total) }).collect();
    out.sort_by_key(|l| std::cmp::Reverse(l.value));
    out
}

/// A holding's line under its name: "12.5 shares · TFSA · 18%".
pub fn holding_line(h: &HoldingView) -> String {
    let at = if h.registration.is_some() { registration_label(h.registration).to_string() } else { h.account.clone() };
    format!("{} · {at} · {}", units_line(h.quantity, h.kind), share_percent(h.weight_bps))
}

/// An account's money-weighted return in words: "+6.2% a year since 4 Mar 2024".
pub fn return_text(a: &InvestLine, today: Date) -> Option<String> {
    let bps = a.return_bps?;
    let since = a.return_since.map(|d| format!(" since {}", short(d, today))).unwrap_or_default();
    Some(format!("{}{}{since}", percent_text(bps, true), if a.return_annual { " a year" } else { "" }))
}

/// An account's line under its name: "TFSA · valued 9 Oct · 5 holdings".
pub fn account_meta(a: &InvestLine, today: Date) -> String {
    let mut parts = vec![registration_label(a.registration).to_string()];
    match a.valued_on {
        Some(on) => parts.push(format!("{}{}", if a.by_hand { "valued by hand " } else { "valued " }, short(on, today))),
        None if a.by_hand => parts.push("no value recorded".into()),
        None => {}
    }
    if !a.by_hand {
        parts.push(copy::plural(a.holdings as i64, "holding"));
    }
    parts.join(" · ")
}

/// The year a registration's room reads on `today` (an RRSP's runs to its deadline).
pub fn room_year(r: Registration, today: Date) -> i32 {
    invest::room_year(r, today)
}

// Money moved after a report

/// The portfolio prices the account: it holds something there (holdings, cash or a book cost).
pub fn priced(a: &PortfolioAccount) -> bool {
    a.holdings > 0 || a.cash != 0 || a.book != 0
}

/// An activity that records money moving into or out of the account itself: a deposit, a
/// withdrawal, or cash moved in or out. Tally's own transfer for the same money is in it.
fn moves_cash(a: &ActivityRow) -> bool {
    match a.r#type {
        ActivityType::Deposit | ActivityType::Withdrawal => true,
        ActivityType::TransferIn | ActivityType::TransferOut => a.security_id.is_none(),
        _ => false,
    }
}

/// The last day the portfolio's own rows say what an investment account holds, as of `day`: its
/// newest holdings report on or before it, or a later activity that moves cash, which already
/// records a deposit or a withdrawal Tally also has as a transfer. None: neither.
pub fn report_day(input: &PortfolioInput, account: i64, day: Date) -> Option<Date> {
    let report = input.holdings.iter().filter(|h| h.account_id == account && h.date <= day).map(|h| h.date).max();
    let cash = input.activities.iter().filter(|a| a.account_id == account && a.date <= day && moves_cash(a)).map(|a| a.date).max();
    report.max(cash)
}

/// What Tally's own transfers moved into each investment account (less what they moved out)
/// after its [`report_day`] as of `day`, up to `through` (none: every later date, as a balance
/// counts them). The portfolio reads only its own rows, so without this a transfer after the
/// newest report would leave the account it came from and arrive nowhere: a holdings value is
/// read like a recorded value, plus what moved after it.
pub fn moved_since_report(input: &PortfolioInput, day: Date, through: Option<Date>) -> HashMap<i64, i64> {
    let mut out = HashMap::new();
    for a in &input.accounts {
        let from = report_day(input, a.id, day);
        let moved: i64 = input
            .transfers
            .iter()
            .filter(|t| t.account_id == a.id && from.is_none_or(|f| t.date > f) && through.is_none_or(|u| t.date <= u))
            .map(|t| t.amount)
            .sum();
        if moved != 0 {
            out.insert(a.id, moved);
        }
    }
    out
}

/// Each open investment account the portfolio prices at the end of `day`, at its value: the
/// portfolio read again with only the rows on or before that day, plus what moved in or out
/// after its report up to `through` ([`moved_since_report`]).
pub fn priced_on(input: &PortfolioInput, day: Date, through: Option<Date>) -> HashMap<i64, i64> {
    let mut i = input.clone();
    i.today = day;
    i.holdings.retain(|r| r.date <= day);
    i.activities.retain(|r| r.date <= day);
    i.prices.retain(|r| r.date <= day);
    i.fx_rates.retain(|r| r.date <= day);
    i.transfers.retain(|r| r.date <= day);
    let p = invest::portfolio(&i);
    let moved = moved_since_report(input, day, through);
    p.accounts.iter().filter(|a| priced(a)).map(|a| (a.id, a.value + moved.get(&a.id).copied().unwrap_or(0))).collect()
}

/// The portfolio as the Investments page shows it: what moved into or out of an account it
/// prices after that account's report is counted as cash there ([`moved_since_report`]), so the
/// page reads each account as the Accounts page and net worth do.
pub fn count_moves(p: &mut Portfolio, input: &PortfolioInput) {
    let moved = moved_since_report(input, input.today, None);
    if moved.is_empty() {
        return;
    }
    let bps = |part: i64, of: i64| if of > 0 { invest::mul_div_half_even(part, 10_000, of) } else { None };
    for a in p.accounts.iter_mut().filter(|a| priced(a)) {
        let m = moved.get(&a.id).copied().unwrap_or(0);
        a.value += m;
        a.cash += m;
        a.book += m;
        a.gain = a.value - a.book;
        a.gain_bps = bps(a.gain, a.book);
    }
    retotal(p);
}

/// The portfolio's totals and each holding's weight, summed again after its accounts changed.
fn retotal(p: &mut Portfolio) {
    let bps = |part: i64, of: i64| if of > 0 { invest::mul_div_half_even(part, 10_000, of) } else { None };
    p.value = p.accounts.iter().map(|a| a.value).sum();
    p.book = p.accounts.iter().map(|a| a.book).sum();
    p.cash = p.accounts.iter().map(|a| a.cash).sum();
    p.gain = p.value - p.book;
    p.gain_bps = bps(p.gain, p.book);
    for h in &mut p.holdings {
        h.weight_bps = if p.value == 0 { 0 } else { invest::mul_div_half_even(h.value, 10_000, p.value).unwrap_or(0) };
    }
}

/// Holdings say what an account holds on `day` only when they were said last: its
/// [`report_day`] is on or after `valued`, the newest value recorded by hand by then (none: no
/// value, so holdings always win). A value recorded after the report is newer word, and the
/// account reads as its balance does then: that value plus what moved after it. Net worth, the
/// Accounts page and the Investments page all ask this, now and for an earlier day alike.
pub fn holdings_win(input: &PortfolioInput, account: i64, day: Date, valued: Option<Date>) -> bool {
    valued.is_none_or(|v| report_day(input, account, day).is_some_and(|r| r >= v))
}

/// The portfolio as the Investments page shows it, with each account whose holdings are older
/// than a value recorded by hand ([`holdings_win`]) set aside: no value, cash or holdings from
/// that report, so [`invest_lines`] reads it by hand, from its balance, as net worth does.
pub fn set_aside_older_reports(p: &mut Portfolio, input: &PortfolioInput, rows: &[AccountRow]) {
    let older: Vec<i64> = p
        .accounts
        .iter()
        .filter(|a| priced(a))
        .filter(|a| !holdings_win(input, a.id, input.today, rows.iter().find(|r| r.view.id == a.id).and_then(|r| r.valued_on)))
        .map(|a| a.id)
        .collect();
    if older.is_empty() {
        return;
    }
    for a in p.accounts.iter_mut().filter(|a| older.contains(&a.id)) {
        (a.value, a.book, a.gain, a.gain_bps, a.cash, a.holdings) = (0, 0, 0, None, 0, 0);
    }
    p.holdings.retain(|h| !older.contains(&h.account_id));
    retotal(p);
}

// History

/// What each open investment account was worth at the end of `day`, and whether that came from
/// its holdings ([`priced_on`], what moved after its report that day included), else `balances`
/// (the account's balance that day, values recorded by hand included).
pub fn values_on(input: &PortfolioInput, balances: &HashMap<i64, i64>, day: Date) -> HashMap<i64, (i64, bool)> {
    let priced = priced_on(input, day, Some(day));
    input
        .accounts
        .iter()
        .map(|a| {
            let v = match priced.get(&a.id) {
                Some(&v) => (v, true),
                None => (balances.get(&a.id).copied().unwrap_or(0), false),
            };
            (a.id, v)
        })
        .collect()
}

/// What an account was worth on an earlier day, given `then` as `values_on` read it for that day
/// (the value, and whether holdings priced it), whether it had somewhere to start by then (an
/// opening balance, or a value recorded on or before the day), and `now`: what it is worth now,
/// less what was moved into it since ([`worth_before`]). Not priced from holdings then and with
/// nowhere to start: nothing says what it was worth, so it reads as `now`, and its first import
/// or first value is not read as a gain.
pub fn worth_then(then: Option<(i64, bool)>, started: bool, now: i64) -> i64 {
    match then {
        Some((v, true)) => v,
        Some((v, false)) if started => v,
        _ => now,
    }
}

/// What an account with nowhere to start by an earlier day reads as then: what it is worth `now`
/// less what was `moved` into it by transfer since (what moved out added back). The account the
/// money came from reads it as gone, so without this a transfer into a new account would read as
/// money lost. Never below zero when it is worth something now: nothing says it was a debt.
pub fn worth_before(now: i64, moved: i64) -> i64 {
    let v = now - moved;
    if now >= 0 { v.max(0) } else { v }
}

/// The chart's days: the last day of each of the eleven months before this one, then today.
pub fn month_ends(today: Date) -> Vec<Date> {
    let first = today.first_of_month();
    let mut out: Vec<Date> = (1..=11)
        .rev()
        .filter_map(|k| first.checked_sub(jiff::Span::new().months(k - 1)).ok().and_then(|d| d.yesterday().ok()))
        .collect();
    out.push(today);
    out
}

/// What the accounts in `ids` were worth on each day of `history`.
pub fn worth_series(history: &[(Date, HashMap<i64, i64>)], ids: &[i64]) -> Vec<(Date, i64)> {
    history.iter().map(|(d, by)| (*d, ids.iter().map(|id| by.get(id).copied().unwrap_or(0)).sum())).collect()
}

/// The line under the total: "+$1,830 this year, 9.4%".
pub fn year_line(now: i64, year_start: i64, fmt: &MoneyFormatter) -> String {
    let change = now - year_start;
    let amount = crate::accounts_store::signed_whole(change, fmt);
    if year_start > 0 { format!("{amount} this year, {}", percent_text(share_bps(change, year_start), false)) } else { format!("{amount} this year") }
}
