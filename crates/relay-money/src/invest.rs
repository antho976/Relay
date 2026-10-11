//! Investments: holdings, their value and the registered-account room (docs/INVESTMENTS.md). Port
//! of `Invest.kt`, held to the same shared tests.
//!
//! Money is minor units of its currency; units held, prices and FX rates are `i64` at 1e-8. Every
//! product is computed in `i128` and rounded once, half-even, and `None` says the result does not
//! fit an `i64` (the Kotlin throws `ArithmeticException`). Positions, book values, gains and room
//! are read from the ledger's rows here, never stored and never synced.

use crate::csv::kt_trim;
use crate::model::{ActivityType, Registration, SecurityKind};
use crate::money::{div_half_even, fraction_digits};
use crate::period::days_between;
use jiff::civil::{Date, Weekday};
use jiff::ToSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub use crate::wealthsimple::{plan, ImportPlan, PlanInput};

/// Units held, at 1e-8 of a unit.
pub const QTY_SCALE: i64 = 100_000_000;
/// A price per unit, at 1e-8 of its currency's major unit.
pub const PRICE_SCALE: i64 = 100_000_000;
/// An FX rate: units of `quote` per one `base`, at 1e-8.
pub const RATE_SCALE: i64 = 100_000_000;

/// Where an activity came from (`activities.source`).
pub const SOURCE_MANUAL: &str = "MANUAL";
pub const SOURCE_WEALTHSIMPLE: &str = "WEALTHSIMPLE";
/// Where a price or a rate came from (`prices.source`, `fx_rates.source`), besides [`SOURCE_MANUAL`].
pub const SOURCE_IMPORT: &str = "IMPORT";
pub const SOURCE_BANK_OF_CANADA: &str = "BANK_OF_CANADA";

fn fit(v: i128) -> Option<i64> {
    i64::try_from(v).ok()
}

fn digits(currency: &str) -> u32 {
    fraction_digits(currency).unwrap_or(2)
}

/// `a·b / d`, rounded half-even. `None` for a zero `d` or a result past `i64`.
pub fn mul_div_half_even(a: i64, b: i64, d: i64) -> Option<i64> {
    let n = i128::from(a) * i128::from(b);
    match d {
        0 => None,
        d if d < 0 => fit(div_half_even(-n, -i128::from(d))),
        d => fit(div_half_even(n, i128::from(d))),
    }
}

/// What `quantity` units are worth at `price`, in minor units of a currency with `fraction_digits`.
pub fn market_value(quantity: i64, price: i64, fraction_digits: u32) -> Option<i64> {
    let d = 10i128.checked_pow(16u32.checked_sub(fraction_digits)?)?;
    fit(div_half_even(i128::from(quantity) * i128::from(price), d))
}

/// `minor` of a currency with `from_digits` decimals, in a currency with `to_digits`, at `rate`.
pub fn convert(minor: i64, from_digits: u32, to_digits: u32, rate: i64) -> Option<i64> {
    let n = i128::from(minor).checked_mul(i128::from(rate))?;
    let (n, d) = if to_digits >= from_digits {
        (n.checked_mul(10i128.checked_pow(to_digits - from_digits)?)?, i128::from(RATE_SCALE))
    } else {
        (n, i128::from(RATE_SCALE).checked_mul(10i128.checked_pow(from_digits - to_digits)?)?)
    };
    fit(div_half_even(n, d))
}

/// A decimal read to a fixed number of places, and whether reading it dropped digits that were not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scaled {
    pub value: i64,
    pub rounded: bool,
}

/// `text` as an exact decimal, rounded half-even to `digits` places: "0.123456789" at 8 is
/// 12_345_679, rounded. An optional sign, digits (commas among them group and are dropped), and
/// optionally a point and more digits; surrounding whitespace is ignored. Anything else (a
/// currency sign, ".5", "5.", "1e5") is `None`, as is a figure past `i64`.
pub fn parse_scaled(text: &str, digits: u32) -> Option<Scaled> {
    let t = kt_trim(text);
    let (negative, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if !int.starts_with(|c: char| c.is_ascii_digit()) || !int.bytes().all(|b| b.is_ascii_digit() || b == b',') {
        return None;
    }
    if body.contains('.') && (frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let keep = frac.len().min(digits as usize);
    let mut value: i128 = 0;
    for b in int.bytes().filter(|b| *b != b',').chain(frac[..keep].bytes()) {
        value = value.checked_mul(10)?.checked_add(i128::from(b - b'0'))?;
    }
    for _ in keep..digits as usize {
        value = value.checked_mul(10)?;
    }
    let rest = &frac[keep..];
    let rounded = rest.bytes().any(|b| b != b'0');
    if let Some(first) = rest.bytes().next() {
        let beyond = rest[1..].bytes().any(|b| b != b'0');
        if first > b'5' || (first == b'5' && (beyond || value % 2 == 1)) {
            value += 1;
        }
    }
    Some(Scaled { value: fit(if negative { -value } else { value })?, rounded })
}

/// The uid an imported activity gets: the same line imported on either device is the same row.
/// `parts` are the account uid, the date, the type, the security uid (or ""), the quantity, the
/// amount, the currency and the fee, numbers in decimal; `occurrence` counts the identical lines
/// before it in the same file.
pub fn import_uid(parts: &[&str], occurrence: usize) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let text = format!("{}\u{1F}{occurrence}", parts.join("\u{1F}"));
    let mut uid = String::from("imp:");
    for b in Sha256::digest(text.as_bytes()).iter().take(16) {
        let _ = write!(uid, "{b:02x}");
    }
    uid
}

// ── The ledger's rows, as the reading takes them ─────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    pub id: i64,
    pub uid: String,
    pub name: String,
    pub registration: Option<Registration>,
    pub institution: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityRow {
    pub id: i64,
    pub uid: String,
    pub symbol: String,
    pub name: String,
    pub currency: String,
    pub kind: SecurityKind,
    pub exchange: String,
}

/// One line of a holdings snapshot: `book` in the ledger currency, `book_market` in the security's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingRow {
    pub account_id: i64,
    pub security_id: i64,
    pub date: Date,
    pub quantity: i64,
    pub book: i64,
    pub book_market: i64,
}

/// One activity. `quantity`, `amount` and `fee` are never negative; the type gives the direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityRow {
    pub id: i64,
    pub uid: String,
    pub account_id: i64,
    pub security_id: Option<i64>,
    pub r#type: ActivityType,
    pub date: Date,
    pub quantity: i64,
    pub amount: i64,
    pub fee: i64,
    pub currency: String,
    pub to_amount: Option<i64>,
    pub to_currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceRow {
    pub security_id: i64,
    pub date: Date,
    pub price: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxRow {
    pub base: String,
    pub quote: String,
    pub date: Date,
    pub rate: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomRow {
    pub registration: Registration,
    pub year: i32,
    pub amount: i64,
}

/// A Tally transfer touching an investment account: `amount` is positive into the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRow {
    pub account_id: i64,
    pub date: Date,
    pub amount: i64,
}

/// Everything [`portfolio`] reads: the ledger currency, today, the investment accounts and the
/// rows that touch them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioInput {
    pub currency: String,
    pub today: Date,
    pub accounts: Vec<AccountRow>,
    pub securities: Vec<SecurityRow>,
    pub holdings: Vec<HoldingRow>,
    pub activities: Vec<ActivityRow>,
    pub prices: Vec<PriceRow>,
    pub fx_rates: Vec<FxRow>,
    pub room_facts: Vec<RoomRow>,
    pub transfers: Vec<TransferRow>,
}

impl PortfolioInput {
    /// No accounts and no rows yet.
    pub fn new(currency: impl Into<String>, today: Date) -> Self {
        PortfolioInput {
            currency: currency.into(),
            today,
            accounts: vec![],
            securities: vec![],
            holdings: vec![],
            activities: vec![],
            prices: vec![],
            fx_rates: vec![],
            room_facts: vec![],
            transfers: vec![],
        }
    }
}

// ── Rates ────────────────────────────────────────────────────────────────

/// Units of `to` per one `from` on `on`: the newest `from`→`to` rate on or before it, else the
/// inverse (1e16 / r, half-even) of the newest `to`→`from` one.
pub fn rate_on(rates: &[FxRow], from: &str, to: &str, on: Date) -> Option<i64> {
    if from == to {
        return Some(RATE_SCALE);
    }
    let newest = |base: &str, quote: &str| {
        rates.iter().filter(|r| r.base == base && r.quote == quote && r.date <= on && r.rate > 0).max_by_key(|r| r.date).map(|r| r.rate)
    };
    newest(from, to).or_else(|| newest(to, from).and_then(|r| mul_div_half_even(RATE_SCALE, RATE_SCALE, r)))
}

/// `minor` of `from` in `to` on `on`: by a rate, else by `ratio` (a position's book over its book
/// in its own currency, when both are above zero), else one to one. True when it was not a rate.
fn to_ledger(minor: i64, from: &str, to: &str, on: Date, rates: &[FxRow], ratio: Option<(i64, i64)>) -> (i64, bool) {
    if from == to {
        return (minor, false);
    }
    if let Some(r) = rate_on(rates, from, to, on) {
        return (convert(minor, digits(from), digits(to), r).unwrap_or(0), false);
    }
    if let Some((book, book_market)) = ratio.filter(|(b, m)| *b > 0 && *m > 0) {
        return (mul_div_half_even(minor, book, book_market).unwrap_or(0), true);
    }
    (convert(minor, digits(from), digits(to), RATE_SCALE).unwrap_or(0), true)
}

// ── Positions and cash ───────────────────────────────────────────────────

/// What an account holds of one security: `book` in the ledger currency, `book_market` in the
/// security's. `fx_estimated` when some of the book was converted without a rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub account_id: i64,
    pub security_id: i64,
    pub quantity: i64,
    pub book: i64,
    pub book_market: i64,
    pub fx_estimated: bool,
}

/// An account's cash in one currency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cash {
    pub account_id: i64,
    pub currency: String,
    pub amount: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Positions {
    pub positions: Vec<Position>,
    pub cash: Vec<Cash>,
    pub issues: Vec<String>,
}

#[derive(Default)]
struct Pos {
    quantity: i64,
    book: i64,
    book_market: i64,
    fx_estimated: bool,
}

/// Each account's positions and cash: its newest snapshot (its holdings on their newest date),
/// moved by its activities dated after it (all of them without a snapshot), in date, type and
/// uid order. The book is the average cost. Positions with no units and no book are dropped.
pub fn positions(input: &PortfolioInput) -> Positions {
    let securities: HashMap<i64, &SecurityRow> = input.securities.iter().map(|s| (s.id, s)).collect();
    let ledger = input.currency.as_str();
    let mut out = Positions::default();
    for account in &input.accounts {
        let snapshot = input.holdings.iter().filter(|h| h.account_id == account.id).map(|h| h.date).max();
        let mut held: BTreeMap<i64, Pos> = BTreeMap::new();
        let mut cash: BTreeMap<String, i64> = BTreeMap::new();
        for h in input.holdings.iter().filter(|h| h.account_id == account.id && Some(h.date) == snapshot) {
            match securities.get(&h.security_id) {
                Some(s) if s.kind == SecurityKind::Cash => {
                    *cash.entry(s.currency.clone()).or_default() += market_value(h.quantity, PRICE_SCALE, digits(&s.currency)).unwrap_or(0);
                }
                _ => {
                    let p = held.entry(h.security_id).or_default();
                    p.quantity += h.quantity;
                    p.book += h.book;
                    p.book_market += h.book_market;
                }
            }
        }
        let mut acts: Vec<&ActivityRow> =
            input.activities.iter().filter(|a| a.account_id == account.id && snapshot.is_none_or(|s| a.date > s)).collect();
        acts.sort_by(|a, b| (a.date, a.r#type, &a.uid).cmp(&(b.date, b.r#type, &b.uid)));
        for a in acts {
            let (amount, fee) = (a.amount, a.fee);
            let mut move_cash = |currency: &str, by: i64| *cash.entry(currency.to_string()).or_default() += by;
            match a.r#type {
                ActivityType::Deposit | ActivityType::Dividend | ActivityType::Interest | ActivityType::Credit | ActivityType::ReturnOfCapital => {
                    move_cash(&a.currency, amount)
                }
                ActivityType::Withdrawal | ActivityType::Fee | ActivityType::Tax => move_cash(&a.currency, -amount),
                ActivityType::Buy => move_cash(&a.currency, -(amount + fee)),
                ActivityType::Sell => move_cash(&a.currency, amount - fee),
                ActivityType::TransferIn if a.security_id.is_none() => move_cash(&a.currency, amount),
                ActivityType::TransferOut if a.security_id.is_none() => move_cash(&a.currency, -amount),
                ActivityType::Fx => {
                    if let (Some(to_amount), Some(to_currency)) = (a.to_amount, a.to_currency.as_deref()) {
                        move_cash(&a.currency, -amount);
                        move_cash(to_currency, to_amount);
                    }
                }
                _ => {}
            }
            let Some(security) = a.security_id else { continue };
            let p = held.entry(security).or_default();
            let ledger_of = |x: i64, p: &mut Pos| {
                let (v, estimated) = to_ledger(x, &a.currency, ledger, a.date, &input.fx_rates, Some((p.book, p.book_market)));
                p.fx_estimated |= estimated;
                v
            };
            match a.r#type {
                ActivityType::Buy | ActivityType::Reinvest => {
                    let cost = amount + fee;
                    let l = ledger_of(cost, p);
                    p.quantity += a.quantity;
                    p.book_market += cost;
                    p.book += l;
                }
                ActivityType::TransferIn => {
                    let l = ledger_of(amount, p);
                    p.quantity += a.quantity;
                    p.book_market += amount;
                    p.book += l;
                }
                ActivityType::Split => p.quantity += a.quantity,
                ActivityType::NotionalDistribution => {
                    let l = ledger_of(amount, p);
                    p.book_market += amount;
                    p.book += l;
                }
                ActivityType::ReturnOfCapital => {
                    let l = ledger_of(amount, p);
                    p.book_market = (p.book_market - amount).max(0);
                    p.book = (p.book - l).max(0);
                }
                ActivityType::Sell | ActivityType::TransferOut => {
                    if a.quantity > p.quantity {
                        let symbol = securities.get(&security).map_or("?", |s| s.symbol.as_str());
                        let issue = if a.r#type == ActivityType::Sell {
                            format!("Sold more {symbol} than the ledger holds")
                        } else {
                            format!("Moved out more {symbol} than the ledger holds")
                        };
                        if !out.issues.contains(&issue) {
                            out.issues.push(issue);
                        }
                        *p = Pos { fx_estimated: p.fx_estimated, ..Pos::default() };
                    } else if a.quantity > 0 {
                        let removed_book = mul_div_half_even(p.book, a.quantity, p.quantity).unwrap_or(p.book);
                        let removed_market = mul_div_half_even(p.book_market, a.quantity, p.quantity).unwrap_or(p.book_market);
                        p.quantity -= a.quantity;
                        p.book -= removed_book;
                        p.book_market -= removed_market;
                    }
                }
                _ => {}
            }
        }
        for (security_id, p) in held {
            if p.quantity != 0 || p.book != 0 {
                out.positions.push(Position {
                    account_id: account.id,
                    security_id,
                    quantity: p.quantity,
                    book: p.book,
                    book_market: p.book_market,
                    fx_estimated: p.fx_estimated,
                });
            }
        }
        for (currency, amount) in cash {
            out.cash.push(Cash { account_id: account.id, currency, amount });
        }
    }
    out
}

// ── Returns ──────────────────────────────────────────────────────────────

const GRID: [f64; 27] = [
    -0.99, -0.95, -0.9, -0.8, -0.7, -0.6, -0.5, -0.4, -0.3, -0.2, -0.1, -0.05, 0.0, 0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5, 2.0,
    3.0, 5.0, 10.0, 100.0, 1000.0,
];

/// The money-weighted return of `flows` (money in negative, money out positive), as a yearly
/// rate on an actual/365 day count; `None` without a flow of each sign or without a root. Newton
/// inside the grid brackets nearest zero, the same steps as the Kotlin so both land on the same rate.
pub fn xirr(flows: &[(Date, i64)]) -> Option<f64> {
    if !flows.iter().any(|f| f.1 > 0) || !flows.iter().any(|f| f.1 < 0) {
        return None;
    }
    let mut sorted = flows.to_vec();
    sorted.sort_by_key(|f| f.0);
    let start = sorted[0].0;
    let scale = sorted.iter().map(|f| (f.1 as f64).abs()).fold(0.0, f64::max);
    let points: Vec<(f64, f64)> = sorted.iter().map(|(d, c)| (days_between(start, *d) as f64 / 365.0, *c as f64 / scale)).collect();
    let f = |r: f64| {
        let l = r.ln_1p();
        points.iter().fold(0.0, |sum, (t, c)| sum + c * (-t * l).exp())
    };
    let slope = |r: f64| {
        let l = r.ln_1p();
        points.iter().fold(0.0, |sum, (t, c)| sum + -t * c * (-(t + 1.0) * l).exp())
    };
    let at_grid: Vec<f64> = GRID.iter().map(|g| f(*g)).collect();
    let mut brackets: Vec<(f64, f64)> = Vec::new();
    for k in 0..GRID.len() {
        if at_grid[k] == 0.0 {
            brackets.push((GRID[k], GRID[k]));
        } else if k + 1 < GRID.len() && at_grid[k + 1] != 0.0 && (at_grid[k] < 0.0) != (at_grid[k + 1] < 0.0) {
            brackets.push((GRID[k], GRID[k + 1]));
        }
    }
    let dist = |(lo, hi): (f64, f64)| if lo <= 0.0 && 0.0 <= hi { 0.0 } else { lo.abs().min(hi.abs()) };
    let nearest = brackets.iter().map(|b| dist(*b)).fold(f64::INFINITY, f64::min);
    let solve = |(mut lo, mut hi): (f64, f64)| {
        let negative_at_lo = f(lo) < 0.0;
        let mut r = (lo + hi) / 2.0;
        for _ in 0..100 {
            let fr = f(r);
            if fr == 0.0 {
                return r;
            }
            if (fr < 0.0) == negative_at_lo {
                lo = r;
            } else {
                hi = r;
            }
            let d = slope(r);
            let mut n = r - fr / d;
            if d == 0.0 || !(n > lo && n < hi) {
                n = (lo + hi) / 2.0;
            }
            if (n - r).abs() < 1e-12 {
                return n;
            }
            r = n;
        }
        r
    };
    brackets.into_iter().filter(|b| dist(*b) == nearest).map(solve).reduce(|a, b| if b.abs() < a.abs() || (b.abs() == a.abs() && b > a) { b } else { a })
}

/// A yearly `rate` held for `days`: the return over that stretch, not annualized.
pub fn period_return(rate: f64, days: i64) -> f64 {
    (1.0 + rate).powf(days as f64 / 365.0) - 1.0
}

/// A rate in basis points, rounded as Java's `Math.round`.
pub fn bps(rate: f64) -> i64 {
    (rate * 10_000.0 + 0.5).floor() as i64
}

// ── Room ─────────────────────────────────────────────────────────────────

/// The TFSA dollar limit for `year`, in cents; `None` before 2009 and for a year not announced.
pub fn tfsa_limit(year: i32) -> Option<i64> {
    Some(match year {
        2009..=2012 => 500_000,
        2013..=2014 => 550_000,
        2015 => 1_000_000,
        2016..=2018 => 550_000,
        2019..=2022 => 600_000,
        2023 => 650_000,
        2024..=2026 => 700_000,
        _ => return None,
    })
}

/// The RRSP dollar limit for `year`, in cents; `None` for a year this table does not hold.
pub fn rrsp_limit(year: i32) -> Option<i64> {
    Some(match year {
        2024 => 3_156_000,
        2025 => 3_249_000,
        2026 => 3_381_000,
        2027 => 3_539_000,
        _ => return None,
    })
}

/// What an RRSP may go over its room before the over-contribution tax.
pub const RRSP_BUFFER: i64 = 200_000;
/// The FHSA's yearly limit and its lifetime one.
pub const FHSA_ANNUAL: i64 = 800_000;
pub const FHSA_LIFETIME: i64 = 4_000_000;

/// The TFSA limits summed from `first_year` (2009 at the earliest) through `year`: the room of
/// someone eligible since `first_year` who never contributed. A hint, not CRA's figure.
pub fn tfsa_cumulative(first_year: i32, year: i32) -> i64 {
    (first_year.max(2009)..=year).filter_map(tfsa_limit).sum()
}

/// Next year's TFSA room: what is left, plus this year's withdrawals (they come back on 1
/// January), plus next year's limit.
pub fn tfsa_next_room(room: i64, contributed: i64, withdrawn: i64, next_limit: i64) -> i64 {
    room - contributed + withdrawn + next_limit
}

/// An FHSA's room in `year`: 8,000 in the year it opened, then 8,000 plus what went unused last
/// year (8,000 at most), never past what is left of the 40,000 for life, and never below zero.
/// `contributions` are (year, amount) pairs; a year may appear more than once.
pub fn fhsa_room(open_year: i32, contributions: &[(i32, i64)], year: i32) -> i64 {
    if year < open_year {
        return 0;
    }
    let in_year = |y: i32| contributions.iter().filter(|(cy, _)| *cy == y).map(|(_, a)| *a).sum::<i64>();
    let before = |y: i32| contributions.iter().filter(|(cy, _)| *cy < y).map(|(_, a)| *a).sum::<i64>();
    let mut room = FHSA_ANNUAL.min(FHSA_LIFETIME - before(open_year)).max(0);
    for y in open_year + 1..=year {
        let carry = (room - in_year(y - 1)).clamp(0, FHSA_ANNUAL);
        room = (FHSA_ANNUAL + carry).min(FHSA_LIFETIME - before(y)).max(0);
    }
    room
}

/// The last day to contribute to an RRSP for `year`: the 60th day of the next year, moved to the
/// Monday when it falls on a weekend.
pub fn rrsp_deadline(year: i32) -> Date {
    let day = Date::new((year + 1) as i16, 1, 1).expect("year in range").checked_add(59.days()).expect("date in range");
    let push = match day.weekday() {
        Weekday::Saturday => 2,
        Weekday::Sunday => 1,
        _ => 0,
    };
    day.checked_add(push.days()).expect("date in range")
}

/// The registrations Tally keeps room for, in [`Registration`] order.
pub const ROOM_REGISTRATIONS: [Registration; 3] = [Registration::Tfsa, Registration::Rrsp, Registration::Fhsa];

/// The first and last day of `registration`'s window for `year`: the calendar year, or for an
/// RRSP the day after last year's deadline through this year's.
pub fn room_window(registration: Registration, year: i32) -> (Date, Date) {
    if registration == Registration::Rrsp {
        (rrsp_deadline(year - 1).tomorrow().expect("date in range"), rrsp_deadline(year))
    } else {
        (Date::new(year as i16, 1, 1).expect("year in range"), Date::new(year as i16, 12, 31).expect("year in range"))
    }
}

/// The year `registration`'s room reads on `today`: the calendar year, but an RRSP's is last year
/// until last year's deadline (RRSP season).
pub fn room_year(registration: Registration, today: Date) -> i32 {
    let year = i32::from(today.year());
    if registration == Registration::Rrsp && today <= rrsp_deadline(year - 1) { year - 1 } else { year }
}

/// What is left of `room` after `contributed`, and what is over it. An RRSP's first $2,000 over is
/// not taxed.
pub fn room_line(registration: Registration, year: i32, room: Option<i64>, contributed: i64, withdrawn: i64) -> RoomView {
    let over = room.map_or(0, |r| (contributed - r).max(0));
    RoomView {
        registration,
        year,
        room,
        contributed,
        withdrawn,
        left: room.map(|r| (r - contributed).max(0)),
        over,
        over_taxed: if registration == Registration::Rrsp { (over - RRSP_BUFFER).max(0) } else { over },
        deadline: room_window(registration, year).1.to_string(),
        limit: match registration {
            Registration::Tfsa => tfsa_limit(year),
            Registration::Rrsp => rrsp_limit(year),
            Registration::Fhsa => Some(FHSA_ANNUAL),
            _ => None,
        },
    }
}

// ── The reading ──────────────────────────────────────────────────────────

/// The portfolio as both devices read it (docs/INVESTMENTS.md, "The portfolio reading"). Money is
/// in the ledger currency; `*_bps` are basis points. Nothing here is stored or synced: it is read
/// again from the rows each time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyPortfolio")]
pub struct Portfolio {
    pub currency: String,
    pub today: String,
    /// No investment account.
    pub empty: bool,
    /// The newest snapshot or price date the reading rests on.
    pub as_of: Option<String>,
    #[schemars(extend("x-unit" = "minor"))]
    pub value: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub book: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub gain: i64,
    pub gain_bps: Option<i64>,
    #[schemars(extend("x-unit" = "minor"))]
    pub cash: i64,
    /// Dividends, interest and reinvested distributions over the 365 days ending today.
    #[schemars(extend("x-unit" = "minor"))]
    pub income_12m: i64,
    /// The same by calendar month, the last twelve, oldest first.
    pub income_by_month: Vec<IncomeMonth>,
    pub accounts: Vec<PortfolioAccount>,
    /// By registration, largest first.
    pub allocation: Vec<AllocationShare>,
    /// By kind of security, cash as `CASH`, largest first.
    pub kinds: Vec<KindShare>,
    /// Largest value first.
    pub holdings: Vec<HoldingView>,
    /// The newest income, at most 10.
    pub income: Vec<IncomeView>,
    pub room: Vec<RoomView>,
    /// What the reading could not square, in plain words.
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyIncomeMonth")]
pub struct IncomeMonth {
    /// `YYYY-MM`.
    pub month: String,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyPortfolioAccount")]
pub struct PortfolioAccount {
    pub id: i64,
    pub name: String,
    pub registration: Option<Registration>,
    pub institution: String,
    #[schemars(extend("x-unit" = "minor"))]
    pub value: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub book: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub gain: i64,
    pub gain_bps: Option<i64>,
    #[schemars(extend("x-unit" = "minor"))]
    pub cash: i64,
    /// How many positions it holds.
    pub holdings: usize,
    /// The newest snapshot or price date its value rests on.
    pub valued_on: Option<String>,
    /// The money-weighted return: yearly when `return_annual`, else over the stretch since `return_since`.
    pub return_bps: Option<i64>,
    pub return_annual: bool,
    pub return_since: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyAllocation")]
pub struct AllocationShare {
    pub registration: Option<Registration>,
    #[schemars(extend("x-unit" = "minor"))]
    pub value: i64,
    pub share_bps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyKindShare")]
pub struct KindShare {
    pub kind: SecurityKind,
    #[schemars(extend("x-unit" = "minor"))]
    pub value: i64,
    pub share_bps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyHolding")]
pub struct HoldingView {
    pub account_id: i64,
    pub account: String,
    pub registration: Option<Registration>,
    pub security_id: i64,
    pub symbol: String,
    pub name: String,
    pub kind: SecurityKind,
    /// The security's currency; `price` is in it.
    pub currency: String,
    #[schemars(extend("x-unit" = "e8"))]
    pub quantity: i64,
    #[schemars(extend("x-unit" = "e8"))]
    pub price: Option<i64>,
    pub price_date: Option<String>,
    #[schemars(extend("x-unit" = "minor"))]
    pub value: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub book: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub gain: i64,
    pub gain_bps: Option<i64>,
    pub weight_bps: i64,
    /// Some of its value or book was converted without a rate.
    pub fx_estimated: bool,
    /// No price on or before today: its value is its book.
    pub no_price: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyIncome")]
pub struct IncomeView {
    pub date: String,
    pub account_id: i64,
    pub symbol: Option<String>,
    pub r#type: ActivityType,
    #[schemars(extend("x-unit" = "minor"))]
    pub amount: i64,
}

/// A registration's room in `year`: the CRA figure (`room`, `None` when none was entered) minus
/// what went in over the window that ends on `deadline`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyRoom")]
pub struct RoomView {
    pub registration: Registration,
    pub year: i32,
    #[schemars(extend("x-unit" = "minor"))]
    pub room: Option<i64>,
    #[schemars(extend("x-unit" = "minor"))]
    pub contributed: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub withdrawn: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub left: Option<i64>,
    #[schemars(extend("x-unit" = "minor"))]
    pub over: i64,
    /// What is over past the RRSP's $2,000 buffer; all of `over` for the others.
    #[schemars(extend("x-unit" = "minor"))]
    pub over_taxed: i64,
    /// The last day that counts for the year.
    pub deadline: String,
    #[schemars(extend("x-unit" = "minor"))]
    pub limit: Option<i64>,
}

fn share(part: i64, total: i64) -> i64 {
    if total == 0 { 0 } else { mul_div_half_even(part, 10_000, total).unwrap_or(0) }
}

fn gain_bps(gain: i64, book: i64) -> Option<i64> {
    if book > 0 { mul_div_half_even(gain, 10_000, book) } else { None }
}

/// The portfolio from the ledger's rows: positions valued at the newest price on or before today,
/// cash, gains, income, money-weighted returns and the registered room.
pub fn portfolio(input: &PortfolioInput) -> Portfolio {
    let ledger = input.currency.as_str();
    let today = input.today;
    let rates = &input.fx_rates;
    let securities: HashMap<i64, &SecurityRow> = input.securities.iter().map(|s| (s.id, s)).collect();
    let read = positions(input);
    let mut issues = read.issues.clone();
    let mut note = |issue: String| {
        if !issues.contains(&issue) {
            issues.push(issue);
        }
    };
    let price_on = |security: i64| {
        input.prices.iter().filter(|p| p.security_id == security && p.date <= today).max_by_key(|p| p.date).map(|p| (p.price, p.date))
    };
    let mut holdings: Vec<HoldingView> = Vec::new();
    let mut accounts: Vec<PortfolioAccount> = Vec::new();
    let mut by_kind: BTreeMap<SecurityKind, i64> = BTreeMap::new();
    let mut as_of: Option<Date> = None;
    for account in &input.accounts {
        let snapshot = input.holdings.iter().filter(|h| h.account_id == account.id).map(|h| h.date).max();
        let mut valued_on = snapshot;
        let (mut value, mut book, mut count) = (0i64, 0i64, 0usize);
        for p in read.positions.iter().filter(|p| p.account_id == account.id) {
            let sec = securities.get(&p.security_id);
            let kind = sec.map_or(SecurityKind::Other, |s| s.kind);
            let currency = sec.map_or(ledger, |s| s.currency.as_str());
            let priced = if kind == SecurityKind::Cash { Some((PRICE_SCALE, None)) } else { price_on(p.security_id).map(|(px, d)| (px, Some(d))) };
            let (v, estimated) = match priced {
                Some((px, _)) => {
                    let market = market_value(p.quantity, px, digits(currency)).unwrap_or(0);
                    to_ledger(market, currency, ledger, today, rates, Some((p.book, p.book_market)))
                }
                None => (p.book, false),
            };
            let price_date = priced.and_then(|(_, d)| d);
            valued_on = valued_on.max(price_date);
            value += v;
            book += p.book;
            count += 1;
            *by_kind.entry(kind).or_default() += v;
            holdings.push(HoldingView {
                account_id: account.id,
                account: account.name.clone(),
                registration: account.registration,
                security_id: p.security_id,
                symbol: sec.map_or_else(|| "?".to_string(), |s| s.symbol.clone()),
                name: sec.map_or_else(String::new, |s| s.name.clone()),
                kind,
                currency: currency.to_string(),
                quantity: p.quantity,
                price: priced.map(|(px, _)| px),
                price_date: price_date.map(|d| d.to_string()),
                value: v,
                book: p.book,
                gain: v - p.book,
                gain_bps: gain_bps(v - p.book, p.book),
                weight_bps: 0,
                fx_estimated: p.fx_estimated || estimated,
                no_price: priced.is_none(),
            });
        }
        let mut cash = 0;
        for c in read.cash.iter().filter(|c| c.account_id == account.id && c.amount != 0) {
            let (v, estimated) = to_ledger(c.amount, &c.currency, ledger, today, rates, None);
            if estimated {
                note(format!("No {} to {ledger} rate: {} cash is counted one to one", c.currency, c.currency));
            }
            cash += v;
        }
        *by_kind.entry(SecurityKind::Cash).or_default() += cash;
        value += cash;
        book += cash;
        as_of = as_of.max(valued_on);
        let (return_bps, return_annual, return_since) = account_return(input, account.id, value);
        accounts.push(PortfolioAccount {
            id: account.id,
            name: account.name.clone(),
            registration: account.registration,
            institution: account.institution.clone(),
            value,
            book,
            gain: value - book,
            gain_bps: gain_bps(value - book, book),
            cash,
            holdings: count,
            valued_on: valued_on.map(|d| d.to_string()),
            return_bps,
            return_annual,
            return_since: return_since.map(|d| d.to_string()),
        });
    }
    let value: i64 = accounts.iter().map(|a| a.value).sum();
    let book: i64 = accounts.iter().map(|a| a.book).sum();
    for h in &mut holdings {
        h.weight_bps = share(h.value, value);
    }
    holdings.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.symbol.cmp(&b.symbol)).then(a.account_id.cmp(&b.account_id)));
    let mut by_registration: BTreeMap<Option<Registration>, i64> = BTreeMap::new();
    for a in &accounts {
        *by_registration.entry(a.registration).or_default() += a.value;
    }
    // Largest first; on a tie, no registration first, then the registrations in their order.
    let mut allocation: Vec<AllocationShare> = by_registration
        .into_iter()
        .filter(|(_, v)| *v != 0)
        .map(|(registration, v)| AllocationShare { registration, value: v, share_bps: share(v, value) })
        .collect();
    allocation.sort_by(|a, b| b.value.cmp(&a.value).then(a.registration.cmp(&b.registration)));
    let mut kinds: Vec<KindShare> =
        by_kind.into_iter().filter(|(_, v)| *v != 0).map(|(kind, v)| KindShare { kind, value: v, share_bps: share(v, value) }).collect();
    kinds.sort_by(|a, b| b.value.cmp(&a.value).then(a.kind.cmp(&b.kind)));

    // Income: what dividends, interest and reinvested distributions brought, in the ledger currency.
    let investing: Vec<i64> = input.accounts.iter().map(|a| a.id).collect();
    let mut earned: Vec<(&ActivityRow, i64)> = input
        .activities
        .iter()
        .filter(|a| investing.contains(&a.account_id) && a.date <= today)
        .filter(|a| matches!(a.r#type, ActivityType::Dividend | ActivityType::Interest | ActivityType::Reinvest))
        .map(|a| (a, to_ledger(a.amount, &a.currency, ledger, a.date, rates, None).0))
        .collect();
    earned.sort_by(|(a, _), (b, _)| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    let year_ago = today.checked_sub(365.days()).expect("date in range");
    let income_12m = earned.iter().filter(|(a, _)| a.date > year_ago).map(|(_, v)| v).sum();
    let this_month = today.first_of_month();
    let income_by_month = (0i64..12)
        .rev()
        .map(|back| {
            let start = this_month.checked_sub(back.months()).expect("date in range");
            let month = format!("{:04}-{:02}", start.year(), start.month());
            let amount = earned.iter().filter(|(a, _)| a.date.first_of_month() == start).map(|(_, v)| v).sum();
            IncomeMonth { month, amount }
        })
        .collect();
    let income = earned
        .iter()
        .take(10)
        .map(|(a, v)| IncomeView {
            date: a.date.to_string(),
            account_id: a.account_id,
            symbol: a.security_id.and_then(|s| securities.get(&s)).map(|s| s.symbol.clone()),
            r#type: a.r#type,
            amount: *v,
        })
        .collect();

    Portfolio {
        currency: input.currency.clone(),
        today: today.to_string(),
        empty: input.accounts.is_empty(),
        as_of: as_of.map(|d| d.to_string()),
        value,
        book,
        gain: value - book,
        gain_bps: gain_bps(value - book, book),
        cash: accounts.iter().map(|a| a.cash).sum(),
        income_12m,
        income_by_month,
        accounts,
        allocation,
        kinds,
        holdings,
        income,
        room: room(input),
        issues,
    }
}

/// An account's money-weighted return in basis points, whether it is yearly, and the day its
/// flows start. Only for an account whose deposits Tally has seen from the start: one with a
/// `DEPOSIT` and either no snapshot or an activity on or before its first one.
fn account_return(input: &PortfolioInput, account: i64, value: i64) -> (Option<i64>, bool, Option<Date>) {
    let acts: Vec<&ActivityRow> = input.activities.iter().filter(|a| a.account_id == account).collect();
    let first_snapshot = input.holdings.iter().filter(|h| h.account_id == account).map(|h| h.date).min();
    let from_the_start = first_snapshot.is_none_or(|first| acts.iter().any(|a| a.date <= first));
    if !acts.iter().any(|a| a.r#type == ActivityType::Deposit) || !from_the_start {
        return (None, false, None);
    }
    let ledger = input.currency.as_str();
    let mut flows: Vec<(Date, i64)> = acts
        .iter()
        .filter_map(|a| {
            let sign = match a.r#type {
                ActivityType::Deposit | ActivityType::TransferIn => -1,
                ActivityType::Withdrawal | ActivityType::TransferOut => 1,
                _ => return None,
            };
            Some((a.date, sign * to_ledger(a.amount, &a.currency, ledger, a.date, &input.fx_rates, None).0))
        })
        .collect();
    flows.push((input.today, value));
    let since = flows.iter().map(|f| f.0).min();
    let Some(rate) = xirr(&flows) else { return (None, false, since) };
    let days = since.map_or(0, |s| days_between(s, input.today));
    if days >= 365 {
        (Some(bps(rate)), true, since)
    } else {
        (Some(bps(period_return(rate, days))), false, since)
    }
}

/// TFSA, RRSP and FHSA room in the year each reads today ([`room_year`]), for each that has an
/// account or a room figure. In RRSP season an RRSP reads last year: what goes in then counts toward it.
fn room(input: &PortfolioInput) -> Vec<RoomView> {
    let ledger = input.currency.as_str();
    let mut out = Vec::new();
    for registration in ROOM_REGISTRATIONS {
        let year = room_year(registration, input.today);
        let accounts: Vec<&AccountRow> = input.accounts.iter().filter(|a| a.registration == Some(registration)).collect();
        let fact = input.room_facts.iter().find(|r| r.registration == registration && r.year == year).map(|r| r.amount);
        if accounts.is_empty() && fact.is_none() {
            continue;
        }
        let (start, end) = room_window(registration, year);
        let within = |d: Date| d >= start && d <= end;
        let (mut contributed, mut withdrawn) = (0i64, 0i64);
        for account in accounts {
            let acts: Vec<&ActivityRow> = input.activities.iter().filter(|a| a.account_id == account.id).collect();
            if acts.is_empty() {
                for t in input.transfers.iter().filter(|t| t.account_id == account.id && within(t.date)) {
                    if t.amount > 0 {
                        contributed += t.amount;
                    } else {
                        withdrawn -= t.amount;
                    }
                }
                continue;
            }
            for a in acts.into_iter().filter(|a| within(a.date)) {
                let v = to_ledger(a.amount, &a.currency, ledger, a.date, &input.fx_rates, None).0;
                match a.r#type {
                    ActivityType::Deposit | ActivityType::TransferIn => contributed += v,
                    ActivityType::Withdrawal | ActivityType::TransferOut => withdrawn += v,
                    _ => {}
                }
            }
        }
        out.push(room_line(registration, year, fact, contributed, withdrawn));
    }
    out
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    // The shared tests of docs/INVESTMENTS.md, by id, under the names `InvestTest.kt` gives them.
    use super::*;
    use jiff::civil::date;

    const UNIT: i64 = QTY_SCALE;

    fn input(today: Date) -> PortfolioInput {
        let mut i = PortfolioInput::new("CAD", today);
        i.accounts.push(AccountRow { id: 1, uid: "a1".into(), name: "TFSA".into(), registration: Some(Registration::Tfsa), institution: String::new() });
        i.securities.push(security(1, "XEQT", "CAD", SecurityKind::Etf));
        i
    }

    fn security(id: i64, symbol: &str, currency: &str, kind: SecurityKind) -> SecurityRow {
        SecurityRow { id, uid: format!("sec:{symbol}"), symbol: symbol.into(), name: String::new(), currency: currency.into(), kind, exchange: String::new() }
    }

    /// An activity in account 1; one that moves cash only names no security.
    fn act(uid: &str, ty: ActivityType, day: Date, quantity: i64, amount: i64, fee: i64) -> ActivityRow {
        let security = (!matches!(ty, ActivityType::Deposit | ActivityType::Withdrawal | ActivityType::Fee | ActivityType::Tax | ActivityType::Fx)).then_some(1);
        ActivityRow {
            id: 0, uid: uid.into(), account_id: 1, security_id: security, r#type: ty, date: day, quantity, amount, fee,
            currency: "CAD".into(), to_amount: None, to_currency: None,
        }
    }

    fn hold(i: &mut PortfolioInput, day: Date, quantity: i64, book: i64) {
        i.holdings.push(HoldingRow { account_id: 1, security_id: 1, date: day, quantity, book, book_market: book });
    }

    /// The one position's units and book after `acts`, applied a day apart from 2026-01-01.
    fn after(acts: &[(ActivityType, i64, i64, i64)]) -> (i64, i64) {
        let mut i = input(date(2026, 12, 31));
        for (n, (ty, q, a, f)) in acts.iter().enumerate() {
            i.activities.push(act(&format!("u{n:03}"), *ty, date(2026, 1, 1).checked_add((n as i64).days()).unwrap(), *q, *a, *f));
        }
        let p = positions(&i);
        p.positions.first().map_or((0, 0), |p| (p.quantity, p.book))
    }

    fn cash(i: &PortfolioInput, currency: &str) -> i64 {
        positions(i).cash.iter().filter(|c| c.currency == currency).map(|c| c.amount).sum()
    }

    use ActivityType::{Buy, Deposit, Dividend, Fee, Reinvest, ReturnOfCapital, Sell, Split, Tax, Withdrawal};

    // ── A: fixed point ───────────────────────────────────────────────────

    #[test]
    fn a1_mul_div_rounds_half_to_even() {
        assert_eq!(Some(6), mul_div_half_even(5, 130_000_000, 100_000_000));
        assert_eq!(Some(2), mul_div_half_even(1, 150_000_000, 100_000_000));
        assert_eq!(Some(-2), mul_div_half_even(-15, 1, 10));
        assert_eq!(Some(-2), mul_div_half_even(-25, 1, 10));
    }

    #[test]
    fn a2_market_value_rounds_once_in_minor_units() {
        assert_eq!(Some(4115), market_value(33_333_300, 12_345_000_000, 2));
        assert_eq!(Some(0), market_value(50_000_000, 1_000_000, 2));
        assert_eq!(Some(2), market_value(150_000_000, 1_000_000, 2));
        assert_eq!(Some(2), market_value(250_000_000, 1_000_000, 2));
    }

    #[test]
    fn a3_convert_rounds_half_to_even() {
        assert_eq!(Some(4570), convert(3333, 2, 2, 137_125_000));
        assert_eq!(Some(6), convert(5, 2, 2, 130_000_000));
    }

    #[test]
    fn a4_parse_scaled_reads_exact_decimals() {
        assert_eq!(Some(Scaled { value: 12_345_679, rounded: true }), parse_scaled("0.123456789", 8));
        assert_eq!(Some(12_345_678), parse_scaled("0.123456785", 8).map(|s| s.value));
        assert_eq!(Some(12_345_678), parse_scaled("0.123456775", 8).map(|s| s.value));
        assert_eq!(Some(Scaled { value: 1_000_000_000, rounded: false }), parse_scaled("10", 8));
        assert_eq!(Some(123_450), parse_scaled("1,234.5", 2).map(|s| s.value));
        assert_eq!(Some(-300), parse_scaled("-3", 2).map(|s| s.value));
        for bad in ["", "abc", "1.2.3", "$5", "1e5", ".5", "5.", "--3", "\u{663}"] {
            assert_eq!(None, parse_scaled(bad, 2), "{bad}");
        }
    }

    /// The Kotlin throws `ArithmeticException`; here the answer is `None`.
    #[test]
    fn a5_market_value_says_when_it_overflows() {
        assert_eq!(None, market_value(9_000_000_000_000_000_000, 100_000_000_000_000_000, 2));
    }

    // ── B: positions ─────────────────────────────────────────────────────

    #[test]
    fn b1_book_is_the_average_cost() {
        assert_eq!((250 * UNIT, 450_000), after(&[(Buy, 100 * UNIT, 150_000, 0), (Buy, 150 * UNIT, 300_000, 0)]));
        assert_eq!((50 * UNIT, 90_000), after(&[(Buy, 100 * UNIT, 150_000, 0), (Buy, 150 * UNIT, 300_000, 0), (Sell, 200 * UNIT, 400_000, 0)]));
        let all = [(Buy, 100 * UNIT, 150_000, 0), (Buy, 150 * UNIT, 300_000, 0), (Sell, 200 * UNIT, 400_000, 0), (Buy, 350 * UNIT, 735_000, 0)];
        assert_eq!((400 * UNIT, 825_000), after(&all));
    }

    #[test]
    fn b3_a_fee_adds_to_the_book_and_a_sale_takes_its_share() {
        assert_eq!((10 * UNIT, 50_999), after(&[(Buy, 10 * UNIT, 50_000, 999)]));
        assert_eq!((6 * UNIT, 30_599), after(&[(Buy, 10 * UNIT, 50_000, 999), (Sell, 4 * UNIT, 24_000, 999)]));
    }

    #[test]
    fn b4_three_sales_of_a_third_empty_the_book_exactly() {
        let buy = (Buy, 3 * UNIT, 10_000, 0);
        let sell = (Sell, UNIT, 4_000, 0);
        assert_eq!((2 * UNIT, 10_000 - 3333), after(&[buy, sell]));
        assert_eq!((UNIT, 10_000 - 3333 - 3334), after(&[buy, sell, sell]));
        assert_eq!((0, 0), after(&[buy, sell, sell, sell]));
    }

    #[test]
    fn b5_fractional_units() {
        assert_eq!((75_000_000, 16_000), after(&[(Buy, 50_000_000, 10_000, 0), (Buy, 25_000_000, 6_000, 0)]));
        assert_eq!((45_000_000, 9_600), after(&[(Buy, 50_000_000, 10_000, 0), (Buy, 25_000_000, 6_000, 0), (Sell, 30_000_000, 9_000, 0)]));
    }

    #[test]
    fn b7_a_split_adds_units_and_leaves_the_book() {
        let mut i = input(date(2026, 12, 31));
        hold(&mut i, date(2026, 1, 1), 100 * UNIT, 500_000);
        i.activities.push(act("s", Split, date(2026, 2, 1), 100 * UNIT, 0, 0));
        let p = &positions(&i).positions[0];
        assert_eq!((200 * UNIT, 500_000), (p.quantity, p.book));
    }

    #[test]
    fn b9_return_of_capital_lowers_the_book_to_zero_and_no_further() {
        assert_eq!((100 * UNIT, 51_000), after(&[(Buy, 100 * UNIT, 101_000, 0), (ReturnOfCapital, 0, 50_000, 0)]));
        assert_eq!((100 * UNIT, 0), after(&[(Buy, 100 * UNIT, 101_000, 0), (ReturnOfCapital, 0, 50_000, 0), (ReturnOfCapital, 0, 60_000, 0)]));
    }

    #[test]
    fn b10_a_reinvested_distribution_adds_units_book_and_income_and_no_cash() {
        let mut i = input(date(2026, 3, 1));
        hold(&mut i, date(2026, 1, 1), 100 * UNIT, 200_000);
        i.activities.push(act("r", Reinvest, date(2026, 2, 1), 2 * UNIT, 5_000, 0));
        let p = &positions(&i).positions[0];
        assert_eq!((102 * UNIT, 205_000), (p.quantity, p.book));
        assert_eq!(0, cash(&i, "CAD"));
        assert_eq!(5_000, portfolio(&i).income_12m);
    }

    #[test]
    fn b16_selling_more_than_is_held_is_an_issue_and_empties_the_position() {
        let mut i = input(date(2026, 12, 31));
        i.activities.push(act("b", Buy, date(2026, 1, 1), 5 * UNIT, 5_000, 0));
        i.activities.push(act("s", Sell, date(2026, 1, 2), 6 * UNIT, 7_200, 0));
        let p = positions(&i);
        assert!(p.positions.is_empty(), "no units and no book: dropped");
        assert_eq!(vec!["Sold more XEQT than the ledger holds".to_string()], p.issues);
    }

    #[test]
    fn b21_a_buy_and_a_sell_on_one_day_apply_buy_first() {
        let mut i = input(date(2026, 12, 31));
        i.activities.push(act("b", Sell, date(2026, 1, 1), 10 * UNIT, 12_000, 0));
        i.activities.push(act("a", Buy, date(2026, 1, 1), 10 * UNIT, 10_000, 0));
        let p = positions(&i);
        assert!(p.issues.is_empty(), "{:?}", p.issues);
        assert!(p.positions.is_empty());
    }

    #[test]
    fn s1_a_snapshot_replaces_what_came_before_it_and_later_entries_add_to_it() {
        let mut i = input(date(2026, 5, 31));
        hold(&mut i, date(2026, 5, 8), 10 * UNIT, 30_000);
        i.activities.push(act("early", Buy, date(2026, 5, 1), 10 * UNIT, 30_000, 0));
        i.activities.push(act("late", Buy, date(2026, 5, 10), 2 * UNIT, 7_000, 0));
        let p = &positions(&i).positions[0];
        assert_eq!((12 * UNIT, 37_000), (p.quantity, p.book));
    }

    // ── C: cash ──────────────────────────────────────────────────────────

    #[test]
    fn c1_cash_follows_every_kind_of_activity() {
        let mut i = input(date(2026, 12, 31));
        let d = date(2026, 1, 1);
        i.activities.push(act("1", Deposit, d, 0, 100_000, 0));
        i.activities.push(act("2", Buy, d, UNIT, 50_000, 999));
        i.activities.push(act("3", Dividend, d, 0, 1_234, 0));
        i.activities.push(act("4", Tax, d, 0, 185, 0));
        i.activities.push(act("5", Fee, d, 0, 500, 0));
        i.activities.push(act("6", Withdrawal, d, 0, 20_000, 0));
        assert_eq!(29_550, cash(&i, "CAD"));
    }

    #[test]
    fn c2_an_exchange_moves_cash_between_currencies() {
        let mut i = input(date(2026, 12, 31));
        i.activities.push(act("1", Deposit, date(2026, 1, 1), 0, 100_000, 0));
        let mut fx = act("2", ActivityType::Fx, date(2026, 1, 2), 0, 50_000, 0);
        fx.to_amount = Some(36_000);
        fx.to_currency = Some("USD".into());
        i.activities.push(fx);
        assert_eq!((50_000, 36_000), (cash(&i, "CAD"), cash(&i, "USD")));
    }

    // ── V: value ─────────────────────────────────────────────────────────

    #[test]
    fn v1_a_holding_is_worth_its_newest_price_and_a_foreign_one_its_own_ratio_without_a_rate() {
        let mut i = input(date(2026, 6, 1));
        hold(&mut i, date(2026, 5, 8), 10 * UNIT, 20_000);
        i.prices.push(PriceRow { security_id: 1, date: date(2026, 5, 8), price: 25 * PRICE_SCALE });
        let p = portfolio(&i);
        let h = &p.holdings[0];
        assert_eq!((h.value, h.gain, h.gain_bps), (25_000, 5_000, Some(2_500)));
        assert!(!h.fx_estimated && !h.no_price);

        let mut i = input(date(2026, 6, 1));
        i.securities = vec![security(1, "AAPL", "USD", SecurityKind::Stock)];
        i.holdings.push(HoldingRow { account_id: 1, security_id: 1, date: date(2026, 5, 8), quantity: 10 * UNIT, book: 1_000, book_market: 750 });
        i.prices.push(PriceRow { security_id: 1, date: date(2026, 5, 8), price: PRICE_SCALE });
        let h = portfolio(&i).holdings[0].clone();
        assert_eq!(1_333, h.value);
        assert!(h.fx_estimated);
        // With a rate on the books, the rate wins.
        i.fx_rates.push(FxRow { base: "USD".into(), quote: "CAD".into(), date: date(2026, 5, 1), rate: 137_125_000 });
        assert_eq!(1_371, portfolio(&i).holdings[0].value);
    }

    // ── R: room ──────────────────────────────────────────────────────────

    #[test]
    fn r1_tfsa_cumulative_sums_the_limits_from_2009() {
        assert_eq!(10_900_000, tfsa_cumulative(2009, 2026));
        assert_eq!(5_700_000, tfsa_cumulative(2018, 2026));
        assert_eq!(700_000, tfsa_cumulative(2026, 2026));
        assert_eq!(0, tfsa_cumulative(2027, 2026));
    }

    #[test]
    fn r4_tfsa_withdrawals_come_back_next_year() {
        assert_eq!(1_100_000, tfsa_next_room(1_000_000, 1_000_000, 400_000, 700_000));
    }

    #[test]
    fn r7_fhsa_room_carries_what_went_unused() {
        assert_eq!(1_400_000, fhsa_room(2024, &[(2024, 800_000), (2025, 200_000)], 2026));
        assert_eq!(1_600_000, fhsa_room(2025, &[], 2026));
        assert_eq!(400_000, fhsa_room(2025, &[(2025, 3_600_000)], 2026));
    }

    #[test]
    fn r8_the_rrsp_deadline_is_the_sixtieth_day_moved_off_a_weekend() {
        assert_eq!(date(2026, 3, 2), rrsp_deadline(2025));
        assert_eq!(date(2027, 3, 1), rrsp_deadline(2026));
        assert_eq!(date(2028, 2, 29), rrsp_deadline(2027));
    }

    #[test]
    fn r9_an_rrsp_has_a_two_thousand_dollar_buffer() {
        let rrsp = |contributed: i64| {
            let mut i = PortfolioInput::new("CAD", date(2026, 10, 9));
            i.accounts.push(AccountRow { id: 1, uid: "a1".into(), name: "RRSP".into(), registration: Some(Registration::Rrsp), institution: String::new() });
            i.room_facts.push(RoomRow { registration: Registration::Rrsp, year: 2026, amount: 2_000_000 });
            i.activities.push(act("d", Deposit, date(2026, 4, 1), 0, contributed, 0));
            // Before last year's deadline: it counts for last year.
            i.activities.push(act("e", Deposit, date(2026, 3, 2), 0, 999_999, 0));
            portfolio(&i).room.into_iter().find(|r| r.registration == Registration::Rrsp).unwrap()
        };
        let r = rrsp(2_150_000);
        assert_eq!((r.over, r.over_taxed, r.left), (150_000, 0, Some(0)));
        assert_eq!("2027-03-01", r.deadline);
        assert_eq!(100_000, rrsp(2_300_000).over_taxed);
    }

    // ── X: money-weighted return ─────────────────────────────────────────

    fn close(want: f64, got: Option<f64>) {
        let got = got.expect("a rate");
        assert!((want - got).abs() < 1e-9, "want {want}, got {got}");
    }

    #[test]
    fn x1_a_year_at_ten_percent() {
        close(0.1, xirr(&[(date(2025, 1, 1), -1000), (date(2026, 1, 1), 1100)]));
    }

    #[test]
    fn x2_a_leap_year_counts_366_days() {
        close(0.0997135859341, xirr(&[(date(2024, 1, 1), -1000), (date(2025, 1, 1), 1100)]));
    }

    #[test]
    fn x3_two_deposits() {
        close(0.1343767484042, xirr(&[(date(2025, 1, 1), -1000), (date(2025, 7, 1), -1000), (date(2026, 1, 1), 2200)]));
    }

    #[test]
    fn x4_under_a_year_shows_the_period_return() {
        let r = xirr(&[(date(2026, 1, 1), -1000), (date(2026, 3, 1), 1020)]);
        close(0.1303279129010, r);
        assert!((period_return(r.unwrap(), 59) - 0.02).abs() < 1e-9);
    }

    #[test]
    fn x5_no_money_out_has_no_rate() {
        assert_eq!(None, xirr(&[(date(2025, 1, 1), -1000), (date(2026, 1, 1), -1000)]));
    }

    #[test]
    fn x6_two_roots_take_the_one_nearest_zero() {
        close(0.1127016653793, xirr(&[(date(2025, 1, 1), -1000), (date(2026, 1, 1), 3000), (date(2027, 1, 1), -2100)]));
    }

    #[test]
    fn x7_a_loss_of_half() {
        close(-0.5, xirr(&[(date(2025, 1, 1), -1000), (date(2026, 1, 1), 500)]));
    }

    #[test]
    fn x8_money_in_and_out_along_the_way() {
        let flows = [(date(2025, 1, 1), -1000), (date(2025, 6, 1), -2000), (date(2025, 9, 1), 500), (date(2026, 1, 1), 2700)];
        close(0.1006305548521, xirr(&flows));
    }

    // ── I: ids ───────────────────────────────────────────────────────────

    #[test]
    fn i1_an_imported_line_has_the_same_uid_on_both_devices() {
        let parts = ["3f2a9c1e-0000-4000-8000-000000000001", "2026-03-02", "BUY", "sec:XTSE:XEQT", "1000000000", "35000", "CAD", "0"];
        assert_eq!("imp:86fe65d31110bce166befa06e9747e9a", import_uid(&parts, 0));
        assert_eq!("imp:2523ed6741037c001850cee952d9149f", import_uid(&parts, 1));
    }

    // ── The reading ──────────────────────────────────────────────────────

    #[test]
    fn an_account_with_deposits_from_the_start_reads_its_money_weighted_return() {
        let mut i = input(date(2026, 1, 1));
        i.activities.push(act("d", Deposit, date(2025, 1, 1), 0, 100_000, 0));
        i.activities.push(act("b", Buy, date(2025, 1, 2), 10 * UNIT, 100_000, 0));
        i.prices.push(PriceRow { security_id: 1, date: date(2025, 12, 31), price: 110 * PRICE_SCALE });
        let p = portfolio(&i);
        let a = &p.accounts[0];
        assert_eq!((a.value, a.book, a.gain_bps), (110_000, 100_000, Some(1_000)));
        assert_eq!((a.return_bps, a.return_annual, a.return_since.as_deref()), (Some(1_000), true, Some("2025-01-01")));
        assert_eq!(p.as_of.as_deref(), Some("2025-12-31"));
        assert_eq!((p.allocation[0].registration, p.allocation[0].share_bps), (Some(Registration::Tfsa), 10_000));
        assert_eq!((p.kinds[0].kind, p.kinds[0].share_bps), (SecurityKind::Etf, 10_000));
        // A snapshot from before any activity hides where the money started.
        hold(&mut i, date(2024, 12, 1), 10 * UNIT, 90_000);
        assert_eq!(None, portfolio(&i).accounts[0].return_bps);
    }

    #[test]
    fn income_counts_twelve_months_and_lists_the_newest_first() {
        let mut i = input(date(2026, 10, 9));
        i.activities.push(act("old", Dividend, date(2025, 10, 9), 0, 1_000, 0));
        i.activities.push(act("in", Dividend, date(2025, 10, 10), 0, 200, 0));
        i.activities.push(act("new", ActivityType::Interest, date(2026, 10, 1), 0, 30, 0));
        let p = portfolio(&i);
        assert_eq!(230, p.income_12m, "the 365 days ending today");
        assert_eq!(12, p.income_by_month.len());
        assert_eq!(("2025-11", 0), (p.income_by_month[0].month.as_str(), p.income_by_month[0].amount));
        assert_eq!(("2026-10", 30), (p.income_by_month[11].month.as_str(), p.income_by_month[11].amount));
        assert_eq!(vec!["2026-10-01", "2025-10-10", "2025-10-09"], p.income.iter().map(|e| e.date.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn tfsa_room_is_the_cra_figure_minus_what_went_in_this_year() {
        let mut i = input(date(2026, 10, 9));
        i.room_facts.push(RoomRow { registration: Registration::Tfsa, year: 2026, amount: 1_000_000 });
        i.activities.push(act("last year", Deposit, date(2025, 12, 31), 0, 500_000, 0));
        i.activities.push(act("in", Deposit, date(2026, 2, 1), 0, 300_000, 0));
        i.activities.push(act("out", Withdrawal, date(2026, 3, 1), 0, 50_000, 0));
        let r = &portfolio(&i).room[0];
        assert_eq!((r.registration, r.room, r.contributed, r.withdrawn, r.left, r.over), (Registration::Tfsa, Some(1_000_000), 300_000, 50_000, Some(700_000), 0));
        assert_eq!((r.deadline.as_str(), r.limit), ("2026-12-31", Some(700_000)));
        // An account without activities counts the transfers Tally made into it.
        i.activities.clear();
        i.transfers.push(TransferRow { account_id: 1, date: date(2026, 4, 1), amount: 200_000 });
        i.transfers.push(TransferRow { account_id: 1, date: date(2026, 5, 1), amount: -20_000 });
        let r = &portfolio(&i).room[0];
        assert_eq!((r.contributed, r.withdrawn), (200_000, 20_000));
    }

    #[test]
    fn in_rrsp_season_the_rrsp_row_reads_last_year() {
        let mut i = PortfolioInput::new("CAD", date(2027, 2, 10));
        i.accounts.push(AccountRow { id: 1, uid: "a1".into(), name: "RRSP".into(), registration: Some(Registration::Rrsp), institution: String::new() });
        i.activities.push(act("before", Deposit, date(2026, 2, 20), 0, 30000, 0));
        i.activities.push(act("summer", Deposit, date(2026, 6, 1), 0, 50000, 0));
        i.activities.push(act("season", Deposit, date(2027, 1, 15), 0, 100000, 0));
        let r = &portfolio(&i).room[0];
        assert_eq!((r.registration, r.year, r.contributed, r.deadline.as_str()), (Registration::Rrsp, 2026, 150000, "2027-03-01"));
        i.today = date(2027, 3, 2);
        let r = &portfolio(&i).room[0];
        assert_eq!((r.year, r.contributed), (2027, 0));
    }

    #[test]
    fn an_empty_portfolio_says_so() {
        let p = portfolio(&PortfolioInput::new("CAD", date(2026, 10, 9)));
        assert!(p.empty);
        assert_eq!((p.value, p.gain_bps, p.as_of), (0, None, None));
        assert!(p.room.is_empty() && p.holdings.is_empty());
    }

    #[test]
    fn foreign_cash_without_a_rate_is_counted_one_to_one_and_said() {
        let mut i = input(date(2026, 10, 9));
        let mut fx = act("x", ActivityType::Fx, date(2026, 1, 2), 0, 0, 0);
        fx.to_amount = Some(1_000);
        fx.to_currency = Some("USD".into());
        i.activities.push(fx);
        let p = portfolio(&i);
        assert_eq!(1_000, p.cash);
        assert_eq!(vec!["No USD to CAD rate: USD cash is counted one to one".to_string()], p.issues);
        i.fx_rates.push(FxRow { base: "CAD".into(), quote: "USD".into(), date: date(2026, 1, 1), rate: 72_926_162 });
        assert_eq!(1_371, portfolio(&i).cash, "the inverse of a CAD to USD rate");
    }

    #[test]
    fn the_reading_encodes_and_decodes() {
        let mut i = input(date(2026, 10, 9));
        hold(&mut i, date(2026, 5, 8), 10 * UNIT, 20_000);
        i.prices.push(PriceRow { security_id: 1, date: date(2026, 5, 8), price: 25 * PRICE_SCALE });
        i.activities.push(act("d", Dividend, date(2026, 9, 15), 0, 1_234, 0));
        let p = portfolio(&i);
        assert_eq!(26_234, p.value, "25,000 held and 1,234 cash");
        assert_eq!(9_530, p.holdings[0].weight_bps);
        assert_eq!(vec![SecurityKind::Etf, SecurityKind::Cash], p.kinds.iter().map(|k| k.kind).collect::<Vec<_>>());
        assert!(!p.empty);
        assert_eq!(p, serde_json::from_str::<Portfolio>(&serde_json::to_string(&p).unwrap()).unwrap());
    }
}
