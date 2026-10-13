//! Wealthsimple's own files: the holdings report, the activities export and the monthly
//! statements (docs/INVESTMENTS.md, "Wealthsimple files"). Port of `Wealthsimple.kt`.
//!
//! Wealthsimple has no public API for a person, so the files its web app exports are the bridge,
//! and nothing leaves the person's devices. [`read`] reads any of the three; [`plan`] turns one
//! into the rows the ledger writes, each with a uid both devices derive the same way, so a file
//! imported on the phone and on the PC adds its lines once.
//!
//! Columns are found by their header name, in any case and order; unknown columns are ignored.

use crate::bank_statements::statement_dates;
use crate::csv::{kt_is_blank, kt_trim, parse_iso_date, parse_records};
use crate::invest::{convert, import_uid, market_value, mul_div_half_even, parse_scaled, rate_on, FxRow, RATE_SCALE};
use crate::model::{ActivityType, Registration, SecurityKind};
use crate::money::fraction_digits;
use jiff::civil::Date;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Which of the three files it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "MoneyImportKind")]
pub enum WsKind {
    Holdings,
    Activities,
    Statement,
}

/// A line the reader left out, and why. Lines count from 1 at the header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MoneyImportSkipped")]
pub struct Skipped {
    pub line: usize,
    pub reason: String,
}

/// An account the file names, by Wealthsimple's account number (`HQ7XFMC41CAD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsAccount {
    pub number: String,
    pub name: String,
    pub registration: Registration,
}

/// One line of a holdings report. `quantity` is at 1e-8 of a unit and `price` at 1e-8 of a
/// dollar; `book` is in Canadian cents, `book_market` and `market_value` minor units of
/// `currency`, the security's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsHolding {
    pub line: usize,
    pub account: String,
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub kind: SecurityKind,
    pub currency: String,
    pub quantity: i64,
    pub price: Option<i64>,
    pub book: i64,
    pub book_market: i64,
    pub market_value: Option<i64>,
}

/// One activity as the file gives it, already in Tally's types: `quantity`, `amount` and `fee`
/// are never negative, `type` gives the direction. `account` is the account's number, `None` on a
/// statement, which names none. `price` is the unit price the file wrote, at 1e-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsActivity {
    pub line: usize,
    pub account: Option<String>,
    pub date: Date,
    pub r#type: ActivityType,
    pub symbol: Option<String>,
    pub name: String,
    pub currency: String,
    pub quantity: i64,
    pub price: Option<i64>,
    pub amount: i64,
    pub fee: i64,
    pub to_amount: Option<i64>,
    pub to_currency: Option<String>,
    pub note: String,
}

/// A file read whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsFile {
    pub kind: WsKind,
    /// The holdings report's "As of" day; `None` for the other two.
    pub as_of: Option<Date>,
    /// In the order they first appear.
    pub accounts: Vec<WsAccount>,
    pub holdings: Vec<WsHolding>,
    pub activities: Vec<WsActivity>,
    pub skipped: Vec<Skipped>,
}

/// What a cash statement is told: it belongs to the bank import.
pub const NOT_INVESTMENT: &str = "not an investment file";

/// The institution an account an import creates names.
pub const INSTITUTION: &str = "Wealthsimple";

const OPTIONS: &str = "Options aren't tracked yet";

/// How many leading lines may come before the header.
const HEADER_SCAN: usize = 12;

/// The codes that make a monthly statement an investment account's.
const INVESTMENT_CODES: &[&str] = &["BUY", "SELL", "DIV", "CONT", "NRT", "FPLINT", "LOAN", "RECALL"];

/// Reads a holdings report, an activities export or an investment account's monthly statement.
/// A byte order mark and CRLF line ends are fine.
pub fn read(text: &str) -> Result<WsFile, String> {
    let records: Vec<Vec<String>> = parse_records(text, ',').into_iter().map(|r| r.iter().map(|f| kt_trim(f).to_string()).collect()).collect();
    if records.is_empty() {
        return Err("The file is empty.".into());
    }
    for (at, record) in records.iter().enumerate().take(HEADER_SCAN) {
        let header: Vec<String> = record.iter().map(|h| h.to_lowercase()).collect();
        let has = |name: &str| header.iter().any(|h| h == name);
        let body = &records[at + 1..];
        if has("symbol") && has("quantity") && (has("book value (cad)") || has("market value")) {
            return Ok(holdings(&header, body));
        }
        if has("transaction_date") && has("activity_type") {
            return activities(&header, body);
        }
        if has("date") && has("transaction") && has("amount") {
            return statement(&header, body);
        }
    }
    Err("This is not a Wealthsimple holdings report, activities export or monthly statement.".into())
}

/// The cell under `name` on `row`; "" when the column or the cell is missing.
fn cell<'a>(header: &[String], row: &'a [String], name: &str) -> &'a str {
    header.iter().position(|h| h == name).and_then(|i| row.get(i)).map_or("", String::as_str)
}

fn digits(currency: &str) -> u32 {
    fraction_digits(currency).unwrap_or(2)
}

fn currency_or_cad(text: &str) -> String {
    let c = text.to_uppercase();
    if fraction_digits(&c).is_some() { c } else { "CAD".into() }
}

/// An account type as Wealthsimple writes it, in English or French, to a registration: the first
/// name it contains, lower-cased.
pub fn registration_of(account_type: &str) -> Registration {
    const NAMES: &[(&[&str], Registration)] = &[
        (&["fhsa", "celiapp"], Registration::Fhsa),
        (&["tfsa", "celi"], Registration::Tfsa),
        (&["rrsp", "reer"], Registration::Rrsp),
        (&["resp", "reee"], Registration::Resp),
        (&["lira", "cri"], Registration::Lira),
        (&["rrif", "ferr"], Registration::Rrif),
        (
            &["non-registered", "non registered", "non_registered", "non enregistré", "personal", "individual", "joint", "margin", "cash", "crypto"],
            Registration::NonRegistered,
        ),
    ];
    let t = account_type.to_lowercase();
    NAMES.iter().find(|(names, _)| names.iter().any(|n| t.contains(n))).map_or(Registration::Other, |(_, r)| *r)
}

/// A security type as the holdings report writes it; `None` for an option, which is not tracked.
pub fn kind_of(security_type: &str) -> Option<SecurityKind> {
    Some(match security_type.to_uppercase().as_str() {
        "OPTION" => return None,
        "EQUITY" => SecurityKind::Stock,
        "EXCHANGE_TRADED_FUND" => SecurityKind::Etf,
        "MUTUAL_FUND" => SecurityKind::MutualFund,
        "BOND" | "FIXED_INCOME" => SecurityKind::Bond,
        "CRYPTOCURRENCY" | "CRYPTO" => SecurityKind::Crypto,
        "CASH" => SecurityKind::Cash,
        _ => SecurityKind::Other,
    })
}

fn note_account(accounts: &mut Vec<WsAccount>, number: &str, name: &str, registration: Registration) {
    if !accounts.iter().any(|a| a.number == number) {
        let name = if name.is_empty() { format!("Wealthsimple {}", registration.label()) } else { name.to_string() };
        accounts.push(WsAccount { number: number.to_string(), name, registration });
    }
}

fn skip(skipped: &mut Vec<Skipped>, line: usize, reason: impl Into<String>) {
    skipped.push(Skipped { line, reason: reason.into() });
}

// ── The holdings report ──────────────────────────────────────────────────

fn holdings(header: &[String], body: &[Vec<String>]) -> WsFile {
    let mut file = WsFile { kind: WsKind::Holdings, as_of: None, accounts: vec![], holdings: vec![], activities: vec![], skipped: vec![] };
    for (i, row) in body.iter().enumerate() {
        let line = i + 2;
        let at = |name: &str| cell(header, row, name);
        // The footer: "As of 2026-05-08 12:00 GMT-04:00".
        let first = row.iter().find(|f| !kt_is_blank(f)).map_or("", String::as_str);
        if first.to_lowercase().starts_with("as of ") {
            file.as_of = first.get(6..).map(kt_trim).and_then(|t| t.get(..10)).and_then(parse_iso_date);
            continue;
        }
        let (symbol, number) = (at("symbol").to_uppercase(), at("account number"));
        if symbol.is_empty() || number.is_empty() {
            skip(&mut file.skipped, line, "A line without an account number or a symbol");
            continue;
        }
        let Some(kind) = kind_of(at("security type")) else {
            skip(&mut file.skipped, line, OPTIONS);
            continue;
        };
        let Some(quantity) = parse_scaled(at("quantity"), 8) else {
            skip(&mut file.skipped, line, format!("The quantity \"{}\" could not be read", at("quantity")));
            continue;
        };
        let currency = [at("market price currency"), at("market value currency"), at("book value currency (market)")]
            .into_iter()
            .find(|c| !c.is_empty())
            .map_or_else(|| "CAD".to_string(), currency_or_cad);
        let money = |name: &str, currency: &str| parse_scaled(at(name), digits(currency)).map(|s| s.value);
        let in_cad = money("book value (cad)", "CAD").or_else(|| if currency == "CAD" { money("book value (market)", "CAD") } else { None });
        let Some(book) = in_cad else {
            skip(&mut file.skipped, line, format!("The book value \"{}\" could not be read", at("book value (cad)")));
            continue;
        };
        note_account(&mut file.accounts, number, at("account name"), registration_of(at("account type")));
        file.holdings.push(WsHolding {
            line,
            account: number.to_string(),
            symbol,
            name: at("name").to_string(),
            exchange: at("exchange").to_string(),
            kind,
            quantity: quantity.value,
            price: parse_scaled(at("market price"), 8).map(|s| s.value),
            book,
            book_market: money("book value (market)", &currency).unwrap_or(book),
            market_value: money("market value", &currency),
            currency,
        });
    }
    file
}

// ── The activities export ────────────────────────────────────────────────

/// One side of a currency exchange, waiting for the other.
struct Leg {
    line: usize,
    account: String,
    date: Date,
    currency: String,
    net: i64,
}

fn activities(header: &[String], body: &[Vec<String>]) -> Result<WsFile, String> {
    if !header.iter().any(|h| h == "net_cash_amount") {
        return Err("The activities export has no net_cash_amount column.".into());
    }
    let mut file = WsFile { kind: WsKind::Activities, as_of: None, accounts: vec![], holdings: vec![], activities: vec![], skipped: vec![] };
    let mut legs: Vec<Leg> = Vec::new();
    for (i, row) in body.iter().enumerate() {
        let line = i + 2;
        let at = |name: &str| cell(header, row, name);
        let Some(date) = statement_dates::parse(at("transaction_date"), false) else {
            // A footer line closes the file.
            if i + 1 < body.len() {
                skip(&mut file.skipped, line, format!("The date \"{}\" could not be read", at("transaction_date")));
            }
            continue;
        };
        let number = at("account_id");
        if number.is_empty() {
            skip(&mut file.skipped, line, "A line without an account");
            continue;
        }
        let kind = at("activity_type");
        let currency = currency_or_cad(at("currency"));
        let Some(net) = parse_scaled(at("net_cash_amount"), digits(&currency)).map(|s| s.value) else {
            skip(&mut file.skipped, line, format!("The amount \"{}\" could not be read", at("net_cash_amount")));
            continue;
        };
        let units = parse_scaled(at("quantity"), 8).map_or(0, |s| s.value);
        let commission = parse_scaled(at("commission"), digits(&currency)).map_or(0, |s| s.value.abs());
        let sub = at("activity_sub_type").to_uppercase();
        let symbol = Some(at("symbol").to_uppercase()).filter(|s| !s.is_empty());
        let registration = registration_of(at("account_type"));
        let options = || matches!(sub.as_str(), "STO" | "BTO" | "STC" | "BTC");
        let ty = match kind.to_lowercase().as_str() {
            "trade" if options() => Err(OPTIONS.to_string()),
            "trade" if sub == "BUY" || sub == "DRIP" || (sub != "SELL" && units > 0) => Ok(ActivityType::Buy),
            "trade" if sub == "SELL" || units < 0 => Ok(ActivityType::Sell),
            "trade" => Err("A trade without units".to_string()),
            "optionexercise" => Err(OPTIONS.to_string()),
            "dividend" if net < 0 => Err("A reversed dividend".to_string()),
            "dividend" => Ok(ActivityType::Dividend),
            // A negative interest line is a charge, not income.
            "interest" if net < 0 => Ok(ActivityType::Fee),
            "interest" => Ok(ActivityType::Interest),
            "moneymovement" if net >= 0 => Ok(ActivityType::Deposit),
            "moneymovement" => Ok(ActivityType::Withdrawal),
            "fxexchange" => {
                legs.push(Leg { line, account: number.to_string(), date, currency, net });
                note_account(&mut file.accounts, number, "", registration);
                continue;
            }
            // A positive one is withholding given back: money in.
            "nonresidenttax" if net > 0 => Ok(ActivityType::Credit),
            "nonresidenttax" => Ok(ActivityType::Tax),
            "fee" if net > 0 => Ok(ActivityType::Credit),
            "fee" => Ok(ActivityType::Fee),
            "refund" | "bonuspayment" | "administrativepayment" if net < 0 => Ok(ActivityType::Fee),
            "refund" | "bonuspayment" | "administrativepayment" => Ok(ActivityType::Credit),
            "returnofcapital" if net < 0 => Err("A reversed return of capital".to_string()),
            "returnofcapital" => Ok(ActivityType::ReturnOfCapital),
            "noncashdistribution" => Ok(ActivityType::NotionalDistribution),
            "securitytransfer" | "internalsecuritytransfer" if units > 0 => Ok(ActivityType::TransferIn),
            "securitytransfer" | "internalsecuritytransfer" => Ok(ActivityType::TransferOut),
            "corporateaction" if units > 0 => Ok(ActivityType::Split),
            _ => Err(format!("Tally doesn't read {kind} lines yet")),
        };
        let ty = match ty {
            Ok(ty) => ty,
            Err(reason) => {
                skip(&mut file.skipped, line, reason);
                continue;
            }
        };
        if symbol.is_none() && matches!(ty, ActivityType::Buy | ActivityType::Sell | ActivityType::Split) {
            skip(&mut file.skipped, line, "A trade without a symbol");
            continue;
        }
        let (amount, fee) = match ty {
            ActivityType::Buy => ((net.abs() - commission).max(0), commission),
            ActivityType::Sell => (net.abs() + commission, commission),
            ActivityType::Split => (0, 0),
            _ => (net.abs(), 0),
        };
        note_account(&mut file.accounts, number, "", registration);
        file.activities.push(WsActivity {
            line,
            account: Some(number.to_string()),
            date,
            r#type: ty,
            symbol,
            name: at("name").to_string(),
            currency,
            quantity: units.abs(),
            price: parse_scaled(at("unit_price"), 8).map(|s| s.value),
            amount,
            fee,
            to_amount: None,
            to_currency: None,
            note: String::new(),
        });
    }
    // An exchange is two lines: money out in one currency, in in the other, on one day. Each out
    // leg takes the first unpaired other leg of its account and day.
    let mut paired = vec![false; legs.len()];
    for o in 0..legs.len() {
        if legs[o].net >= 0 || paired[o] {
            continue;
        }
        let other = (0..legs.len()).find(|&n| !paired[n] && legs[n].net >= 0 && legs[n].account == legs[o].account && legs[n].date == legs[o].date);
        let Some(n) = other else { continue };
        paired[o] = true;
        paired[n] = true;
        let (out, inn) = (&legs[o], &legs[n]);
        file.activities.push(WsActivity {
            line: out.line,
            account: Some(out.account.clone()),
            date: out.date,
            r#type: ActivityType::Fx,
            symbol: None,
            name: String::new(),
            currency: out.currency.clone(),
            quantity: 0,
            price: None,
            amount: out.net.abs(),
            fee: 0,
            to_amount: Some(inn.net),
            to_currency: Some(inn.currency.clone()),
            note: String::new(),
        });
    }
    for (leg, _) in legs.iter().zip(&paired).filter(|(_, p)| !**p) {
        skip(&mut file.skipped, leg.line, "A currency exchange without its other side");
    }
    file.activities.sort_by_key(|a| a.line);
    file.skipped.sort_by_key(|s| s.line);
    Ok(file)
}

// ── A monthly statement ──────────────────────────────────────────────────

// Java's `\s` is ASCII whitespace; spaces and tabs are spelled out so both read the same descriptions.
static TRADE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Za-z0-9.]+)[ \t]*-[ \t]*[^\n\r]+:[ \t]*(Bought|Sold)[ \t]+([0-9.,]+)[ \t]+shares?[ \t]+at[ \t]+\$([0-9.,]+)").unwrap()
});

fn statement(header: &[String], body: &[Vec<String>]) -> Result<WsFile, String> {
    let code_of = |row: &[String]| cell(header, row, "transaction").to_uppercase();
    if !body.iter().any(|r| INVESTMENT_CODES.contains(&code_of(r).as_str())) {
        return Err(NOT_INVESTMENT.into());
    }
    let mut file = WsFile { kind: WsKind::Statement, as_of: None, accounts: vec![], holdings: vec![], activities: vec![], skipped: vec![] };
    for (i, row) in body.iter().enumerate() {
        let line = i + 2;
        let at = |name: &str| cell(header, row, name);
        let Some(date) = statement_dates::parse(at("date"), false) else {
            skip(&mut file.skipped, line, format!("The date \"{}\" could not be read", at("date")));
            continue;
        };
        let code = code_of(row);
        let currency = currency_or_cad(at("currency"));
        let Some(signed) = parse_scaled(at("amount"), digits(&currency)).map(|s| s.value) else {
            skip(&mut file.skipped, line, format!("The amount \"{}\" could not be read", at("amount")));
            continue;
        };
        let amount = signed.abs();
        let description = at("description");
        let before_dash = description.split_once(" - ").map(|(s, _)| kt_trim(s).to_uppercase()).filter(|s| !s.is_empty());
        let mut activity = WsActivity {
            line,
            account: None,
            date,
            r#type: ActivityType::Deposit,
            symbol: None,
            name: String::new(),
            currency,
            quantity: 0,
            price: None,
            amount,
            fee: 0,
            to_amount: None,
            to_currency: None,
            note: description.to_string(),
        };
        match code.as_str() {
            "BUY" | "SELL" => {
                let trade = TRADE.captures(description).and_then(|m| {
                    let units = parse_scaled(&m[3], 8)?.value;
                    let price = parse_scaled(&m[4], 8)?.value;
                    Some((m[1].to_uppercase(), units, price))
                });
                let Some((symbol, units, price)) = trade else {
                    skip(&mut file.skipped, line, "The description does not say how many shares or at what price");
                    continue;
                };
                activity.r#type = if code == "BUY" { ActivityType::Buy } else { ActivityType::Sell };
                activity.symbol = Some(symbol);
                activity.quantity = units;
                activity.price = Some(price);
            }
            "DIV" => {
                activity.r#type = ActivityType::Dividend;
                activity.symbol = before_dash;
            }
            "CONT" => activity.r#type = ActivityType::Deposit,
            // These three take their direction from the amount's sign: withholding given back is
            // money in, negative interest a charge, a positive fee line a rebate.
            "NRT" => activity.r#type = if signed > 0 { ActivityType::Credit } else { ActivityType::Tax },
            "FPLINT" | "INT" => activity.r#type = if signed < 0 { ActivityType::Fee } else { ActivityType::Interest },
            "FEE" => activity.r#type = if signed > 0 { ActivityType::Credit } else { ActivityType::Fee },
            "WD" | "WDL" => activity.r#type = ActivityType::Withdrawal,
            "LOAN" | "RECALL" => {
                skip(&mut file.skipped, line, "Securities lending isn't tracked");
                continue;
            }
            _ => {
                skip(&mut file.skipped, line, format!("Tally doesn't read {code} lines yet"));
                continue;
            }
        }
        file.activities.push(activity);
    }
    Ok(file)
}

// ── The import plan ──────────────────────────────────────────────────────

/// What [`plan`] needs besides the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanInput {
    /// The ledger currency: only `CAD` takes a Wealthsimple file.
    pub currency: String,
    /// Wealthsimple account number to account uid. A number not here gets `ws:<number>`, the
    /// account the import creates.
    pub accounts: HashMap<String, String>,
    /// The uid of the account a statement belongs to: a statement names none.
    pub statement_account: Option<String>,
    pub fx_rates: Vec<FxRow>,
}

impl PlanInput {
    pub fn new(currency: impl Into<String>) -> Self {
        PlanInput { currency: currency.into(), ..PlanInput::default() }
    }
}

/// An INVESTMENT account the import creates: uid `ws:<number>`, `external_ref` the number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanAccount {
    pub uid: String,
    pub name: String,
    pub registration: Registration,
    pub institution: String,
    pub external_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSecurity {
    pub uid: String,
    pub symbol: String,
    pub name: String,
    pub currency: String,
    pub kind: SecurityKind,
    pub exchange: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanHolding {
    pub uid: String,
    pub account_uid: String,
    pub security_uid: String,
    pub date: Date,
    pub quantity: i64,
    pub book: i64,
    pub book_market: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanActivity {
    pub uid: String,
    pub line: usize,
    pub account_uid: String,
    pub security_uid: Option<String>,
    pub r#type: ActivityType,
    pub date: Date,
    pub quantity: i64,
    pub amount: i64,
    pub fee: i64,
    pub currency: String,
    pub to_amount: Option<i64>,
    pub to_currency: Option<String>,
    pub note: String,
}

/// A holding's market price on the report's day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPrice {
    pub uid: String,
    pub security_uid: String,
    pub date: Date,
    pub price: i64,
}

/// An account's market value in Canadian cents on the report's day: the `account_values` row the
/// import records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValue {
    pub uid: String,
    pub account_uid: String,
    pub date: Date,
    pub value: i64,
}

/// The rows an import writes, in file order. References are uids; a row goes in only when its
/// uid is neither present nor deleted. `accounts` are the ones to create (numbers
/// [`PlanInput::accounts`] does not map).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub kind: WsKind,
    pub accounts: Vec<PlanAccount>,
    pub securities: Vec<PlanSecurity>,
    pub holdings: Vec<PlanHolding>,
    pub activities: Vec<PlanActivity>,
    pub prices: Vec<PlanPrice>,
    pub values: Vec<PlanValue>,
    pub skipped: Vec<Skipped>,
}

/// A security's uid: its symbol, upper-cased and trimmed.
pub fn security_uid(symbol: &str) -> String {
    format!("sec:{}", kt_trim(symbol).to_uppercase())
}

fn type_name(t: ActivityType) -> String {
    serde_json::to_value(t).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn add_security(out: &mut ImportPlan, s: PlanSecurity) {
    if !out.securities.iter().any(|x| x.uid == s.uid) {
        out.securities.push(s);
    }
}

/// The rows `file` becomes, with the uids both devices derive. Pure, so the phone and the PC
/// write the same rows from the same file. `Err` says why nothing can be written.
pub fn plan(file: &WsFile, input: &PlanInput) -> Result<ImportPlan, String> {
    if input.currency != "CAD" {
        return Err(format!("Wealthsimple reports are in Canadian dollars; this ledger keeps {}", input.currency));
    }
    let mut out = ImportPlan {
        kind: file.kind,
        accounts: vec![],
        securities: vec![],
        holdings: vec![],
        activities: vec![],
        prices: vec![],
        values: vec![],
        skipped: file.skipped.clone(),
    };
    let account_uid = |out: &mut ImportPlan, number: &str| -> String {
        if let Some(uid) = input.accounts.get(number) {
            return uid.clone();
        }
        let uid = format!("ws:{number}");
        if !out.accounts.iter().any(|a| a.uid == uid) {
            let found = file.accounts.iter().find(|a| a.number == number);
            let registration = found.map_or(Registration::Other, |a| a.registration);
            out.accounts.push(PlanAccount {
                uid: uid.clone(),
                name: found.map_or_else(|| format!("Wealthsimple {}", registration.label()), |a| a.name.clone()),
                registration,
                institution: INSTITUTION.into(),
                external_ref: number.to_string(),
            });
        }
        uid
    };
    if file.kind == WsKind::Holdings {
        let Some(as_of) = file.as_of else {
            return Err("The holdings report has no \"As of\" line, so its day is unknown".into());
        };
        for h in &file.holdings {
            let (account, security) = (account_uid(&mut out, &h.account), security_uid(&h.symbol));
            let symbol = kt_trim(&h.symbol).to_uppercase();
            add_security(&mut out, PlanSecurity { uid: security.clone(), symbol, name: h.name.clone(), currency: h.currency.clone(), kind: h.kind, exchange: h.exchange.clone() });
            let uid = format!("hold:{account}:{security}:{as_of}");
            match out.holdings.iter_mut().find(|x| x.uid == uid) {
                Some(x) => {
                    x.quantity += h.quantity;
                    x.book += h.book;
                    x.book_market += h.book_market;
                }
                None => out.holdings.push(PlanHolding {
                    uid,
                    account_uid: account.clone(),
                    security_uid: security.clone(),
                    date: as_of,
                    quantity: h.quantity,
                    book: h.book,
                    book_market: h.book_market,
                }),
            }
            if let Some(price) = h.price {
                let uid = format!("px:{security}:{as_of}");
                if !out.prices.iter().any(|p| p.uid == uid) {
                    out.prices.push(PlanPrice { uid, security_uid: security, date: as_of, price });
                }
            }
            let uid = format!("val:{account}:{as_of}");
            let value = in_cad(h, as_of, &input.fx_rates);
            match out.values.iter_mut().find(|v| v.uid == uid) {
                Some(v) => v.value += value,
                None => out.values.push(PlanValue { uid, account_uid: account, date: as_of, value }),
            }
        }
        return Ok(out);
    }
    let mut seen: HashMap<Vec<String>, usize> = HashMap::new();
    for a in &file.activities {
        let account = match (&a.account, &input.statement_account) {
            (Some(number), _) => account_uid(&mut out, number),
            (None, Some(uid)) => uid.clone(),
            (None, None) => return Err("Say which account this statement belongs to".into()),
        };
        let security = a.symbol.as_deref().map(security_uid);
        if let (Some(uid), Some(symbol)) = (&security, &a.symbol) {
            let s = PlanSecurity {
                uid: uid.clone(),
                symbol: kt_trim(symbol).to_uppercase(),
                name: a.name.clone(),
                currency: a.currency.clone(),
                kind: SecurityKind::Other,
                exchange: String::new(),
            };
            add_security(&mut out, s);
        }
        let parts = vec![
            account.clone(),
            a.date.to_string(),
            type_name(a.r#type),
            security.clone().unwrap_or_default(),
            a.quantity.to_string(),
            a.amount.to_string(),
            a.currency.clone(),
            a.fee.to_string(),
        ];
        let occurrence = seen.entry(parts.clone()).or_insert(0);
        let uid = import_uid(&parts.iter().map(String::as_str).collect::<Vec<_>>(), *occurrence);
        *occurrence += 1;
        out.activities.push(PlanActivity {
            uid,
            line: a.line,
            account_uid: account,
            security_uid: security,
            r#type: a.r#type,
            date: a.date,
            quantity: a.quantity,
            amount: a.amount,
            fee: a.fee,
            currency: a.currency.clone(),
            to_amount: a.to_amount,
            to_currency: a.to_currency.clone(),
            note: a.note.clone(),
        });
    }
    Ok(out)
}

/// A holding's market value in Canadian cents on `date`: by a rate, else by its own book in both
/// currencies, else one to one; without a market value or a price, its book.
fn in_cad(h: &WsHolding, date: Date, rates: &[FxRow]) -> i64 {
    let fd = digits(&h.currency);
    let Some(market) = h.market_value.or_else(|| h.price.and_then(|p| market_value(h.quantity, p, fd))) else { return h.book };
    if h.currency == "CAD" {
        return market;
    }
    if let Some(r) = rate_on(rates, &h.currency, "CAD", date) {
        return convert(market, fd, 2, r).unwrap_or(0);
    }
    if h.book > 0 && h.book_market > 0 {
        return mul_div_half_even(market, h.book, h.book_market).unwrap_or(0);
    }
    convert(market, fd, 2, RATE_SCALE).unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
pub(crate) mod tests {
    // The shared tests of docs/INVESTMENTS.md, by id, under the names `WealthsimpleTest.kt` gives
    // them, on the same files byte for byte.
    use super::*;
    use crate::invest::{PRICE_SCALE, QTY_SCALE};
    use jiff::civil::date;

    /// W1: the holdings report's header and the demo rows of a public fixture, the AAPL row in USD.
    pub(crate) const W1: &str = "Account Name,Account Type,Account Classification,Account Number,Symbol,Exchange,MIC,Name,Security Type,Quantity,Position Direction,Market Price,Market Price Currency,Book Value (CAD),Book Value Currency (CAD),Book Value (Market),Book Value Currency (Market),Market Value,Market Value Currency,Market Unrealized Returns,Market Unrealized Returns Currency
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"AAPL\",\"NASDAQ\",\"XNAS\",\"Apple Inc\",\"EQUITY\",\"10\",\"LONG\",\"100\",\"USD\",\"1000\",\"CAD\",\"750\",\"USD\",\"1000\",\"USD\",\"0\",\"USD\"
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"XEQT\",\"TSX\",\"XTSE\",\"iShares Core Equity ETF Portfolio\",\"EXCHANGE_TRADED_FUND\",\"10\",\"LONG\",\"25\",\"CAD\",\"250\",\"CAD\",\"250\",\"CAD\",\"250\",\"CAD\",\"0\",\"CAD\"
\"Demo TFSA\",\"TFSA\",\"Trade\",\"DEMO0001CAD\",\"ARKK\",\"BATS\",\"BATS\",\"ARK Innovation ETF\",\"EXCHANGE_TRADED_FUND\",\"1\",\"LONG\",\"50\",\"USD\",\"50\",\"CAD\",\"50\",\"USD\",\"50\",\"USD\",\"0\",\"USD\"

\"As of 2026-05-08 12:00 GMT-04:00\"
";

    /// W2: an activities export: a buy, a dividend reinvested (a DRIP pair), a contribution, an
    /// exchange's two legs, an option trade, and a footer.
    pub(crate) const W2: &str = "transaction_date,settlement_date,account_id,account_type,activity_type,activity_sub_type,direction,symbol,name,currency,quantity,unit_price,commission,net_cash_amount
2026-08-08,2026-08-11,HQ7XFMC41CAD,TFSA,Trade,BUY,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,10,38.12,0,-381.20
2026-08-15,2026-08-15,HQ7XFMC41CAD,TFSA,MoneyMovement,CONTRIBUTION,,,,CAD,,,,500.00
2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Dividend,,,XEQT,iShares Core Equity ETF Portfolio,CAD,,,,4.21
2026-09-29,2026-09-29,HQ7XFMC41CAD,TFSA,Trade,DRIP,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,0.1104,38.13,0,-4.21
2026-10-01,2026-10-01,HQ7XFMC41CAD,TFSA,FxExchange,,,,,CAD,,,,-137.13
2026-10-01,2026-10-01,HQ7XFMC41CAD,TFSA,FxExchange,,,,,USD,,,,100.00
2026-10-02,2026-10-03,HQ7XFMC41CAD,TFSA,Trade,STO,SHORT,AAPL 261218C00250000,AAPL Dec 2026 250 Call,USD,-1,5.10,0.75,509.25
\"As of 2026-10-09 12:00 GMT-04:00\"
";

    /// W3: an investment account's monthly statement.
    pub(crate) const W3: &str = "date,transaction,description,amount,balance,currency
2026-01-02,CONT,Contribution (executed at 2026-01-02),500.00,500.00,CAD
2026-01-05,BUY,\"XEQT - iShares Core Equity ETF Portfolio: Bought 10.0000 shares at $38.12 per share\",-381.20,118.80,CAD
2026-01-29,DIV,\"XEQT - iShares Core Equity ETF Portfolio: Cash dividend distribution, received on 2026-01-29\",4.21,123.01,CAD
2026-01-30,LOAN,\"XEQT - iShares Core Equity ETF Portfolio: Securities lending loan\",0.00,123.01,CAD
";

    /// A Wealthsimple Cash statement: the bank import's, not this one's.
    const CASH_STATEMENT: &str = "date,transaction,description,amount,balance,currency
2026-09-01,SPEND,Metro,-42.10,500.00,CAD
2026-09-02,INT,Interest,0.85,500.85,CAD
";

    fn cad() -> PlanInput {
        PlanInput::new("CAD")
    }

    #[test]
    fn w1_the_holdings_report_reads_its_day_units_and_book() {
        let f = read(W1).unwrap();
        assert_eq!(WsKind::Holdings, f.kind);
        assert_eq!(Some(date(2026, 5, 8)), f.as_of);
        assert_eq!(vec![10 * QTY_SCALE, 10 * QTY_SCALE, QTY_SCALE], f.holdings.iter().map(|h| h.quantity).collect::<Vec<_>>());
        assert_eq!(vec![100_000, 25_000, 5_000], f.holdings.iter().map(|h| h.book).collect::<Vec<_>>());
        assert_eq!(vec!["USD", "CAD", "USD"], f.holdings.iter().map(|h| h.currency.as_str()).collect::<Vec<_>>());
        let aapl = &f.holdings[0];
        assert_eq!((aapl.book_market, aapl.market_value, aapl.price), (75_000, Some(100_000), Some(100 * PRICE_SCALE)));
        assert_eq!((aapl.kind, f.holdings[1].kind), (SecurityKind::Stock, SecurityKind::Etf));
        assert_eq!(vec![WsAccount { number: "DEMO0001CAD".into(), name: "Demo TFSA".into(), registration: Registration::Tfsa }], f.accounts);
        assert!(f.skipped.is_empty(), "{:?}", f.skipped);
    }

    #[test]
    fn a_holdings_report_reads_with_a_byte_order_mark_crlf_and_its_columns_in_any_order() {
        let text = "\u{FEFF}quantity,SYMBOL,Book Value (CAD),Account Number,Account Type,Security Type,Extra\r\n12.5,veqt,\"1,234.56\",ABC123CAD,RRSP,EXCHANGE_TRADED_FUND,x\r\n\r\n\"As of 2026-05-08 12:00 GMT-04:00\"\r\n";
        let f = read(text).unwrap();
        let h = &f.holdings[0];
        assert_eq!(("VEQT", 1_250_000_000, 123_456, "CAD"), (h.symbol.as_str(), h.quantity, h.book, h.currency.as_str()));
        assert_eq!("Wealthsimple RRSP", f.accounts[0].name, "no account name in the file");
        assert_eq!(Some(date(2026, 5, 8)), f.as_of);
    }

    #[test]
    fn w2_the_activities_export_maps_each_kind_of_line() {
        let f = read(W2).unwrap();
        assert_eq!(WsKind::Activities, f.kind);
        let got: Vec<_> = f.activities.iter().map(|a| (a.line, a.r#type, a.symbol.as_deref(), a.quantity, a.amount, a.fee, a.currency.as_str())).collect();
        assert_eq!(
            vec![
                (2, ActivityType::Buy, Some("XEQT"), 10 * QTY_SCALE, 381_20, 0, "CAD"),
                (3, ActivityType::Deposit, None, 0, 500_00, 0, "CAD"),
                (4, ActivityType::Dividend, Some("XEQT"), 0, 4_21, 0, "CAD"),
                (5, ActivityType::Buy, Some("XEQT"), 11_040_000, 4_21, 0, "CAD"),
                (6, ActivityType::Fx, None, 0, 137_13, 0, "CAD"),
            ],
            got
        );
        assert_eq!((Some(100_00), Some("USD")), (f.activities[4].to_amount, f.activities[4].to_currency.as_deref()));
        assert_eq!(vec![Skipped { line: 8, reason: "Options aren't tracked yet".into() }], f.skipped, "the footer is not a line");
        assert_eq!(vec![WsAccount { number: "HQ7XFMC41CAD".into(), name: "Wealthsimple TFSA".into(), registration: Registration::Tfsa }], f.accounts);
    }

    #[test]
    fn a_commission_is_the_fee_and_the_amount_is_the_rest() {
        let text = "transaction_date,account_id,account_type,activity_type,activity_sub_type,symbol,currency,quantity,commission,net_cash_amount
2026-08-08,A1,Personal,Trade,BUY,VFV,CAD,2,-4.95,-204.95
2026-08-09,A1,Personal,Trade,SELL,VFV,CAD,-1,-4.95,95.05
2026-08-10,A1,Personal,Dividend,,VFV,CAD,,,-1.00
2026-08-11,A1,Personal,FxExchange,,,USD,,,10.00
2026-08-12,A1,Personal,Giveaway,,,CAD,,,5.00
";
        let f = read(text).unwrap();
        let trades: Vec<(ActivityType, i64, i64)> = f.activities.iter().map(|a| (a.r#type, a.amount, a.fee)).collect();
        assert_eq!(vec![(ActivityType::Buy, 200_00, 4_95), (ActivityType::Sell, 100_00, 4_95)], trades);
        assert_eq!(Registration::NonRegistered, f.accounts[0].registration);
        let reasons: Vec<&str> = f.skipped.iter().map(|s| s.reason.as_str()).collect();
        assert_eq!(vec!["A reversed dividend", "A currency exchange without its other side", "Tally doesn't read Giveaway lines yet"], reasons);
    }

    #[test]
    fn w3_an_investment_statement_reads_its_trades_from_the_description_and_a_cash_one_is_refused() {
        let f = read(W3).unwrap();
        assert_eq!(WsKind::Statement, f.kind);
        let buy = &f.activities[1];
        assert_eq!((ActivityType::Buy, Some("XEQT"), 10 * QTY_SCALE, Some(3_812_000_000), 381_20), (buy.r#type, buy.symbol.as_deref(), buy.quantity, buy.price, buy.amount));
        let types: Vec<ActivityType> = f.activities.iter().map(|a| a.r#type).collect();
        assert_eq!(vec![ActivityType::Deposit, ActivityType::Buy, ActivityType::Dividend], types);
        assert_eq!(Some("XEQT"), f.activities[2].symbol.as_deref());
        assert_eq!(vec![5], f.skipped.iter().map(|s| s.line).collect::<Vec<_>>(), "securities lending");
        assert_eq!(Err(NOT_INVESTMENT.to_string()), read(CASH_STATEMENT));
    }

    #[test]
    fn interest_withholding_and_return_of_capital_take_their_direction_from_the_sign() {
        let text = "transaction_date,account_id,account_type,activity_type,symbol,currency,net_cash_amount
2026-08-01,A1,TFSA,Interest,,CAD,1.50
2026-08-02,A1,TFSA,Interest,,CAD,-12.34
2026-08-03,A1,TFSA,NonResidentTax,VFV,CAD,-0.63
2026-08-04,A1,TFSA,NonResidentTax,VFV,CAD,0.63
2026-08-05,A1,TFSA,ReturnOfCapital,VFV,CAD,2.00
2026-08-06,A1,TFSA,ReturnOfCapital,VFV,CAD,-2.00
";
        let f = read(text).unwrap();
        let got: Vec<(usize, ActivityType, i64)> = f.activities.iter().map(|a| (a.line, a.r#type, a.amount)).collect();
        assert_eq!(
            vec![
                (2, ActivityType::Interest, 1_50),
                (3, ActivityType::Fee, 12_34),
                (4, ActivityType::Tax, 63),
                (5, ActivityType::Credit, 63),
                (6, ActivityType::ReturnOfCapital, 2_00),
            ],
            got
        );
        assert_eq!(vec![Skipped { line: 7, reason: "A reversed return of capital".into() }], f.skipped);
    }

    #[test]
    fn a_statements_withholding_interest_and_fee_lines_take_their_direction_from_the_sign() {
        let text = "date,transaction,description,amount,balance,currency
2026-02-01,NRT,Non-resident tax,-0.63,0,CAD
2026-02-02,NRT,Non-resident tax refund,0.63,0,CAD
2026-02-03,INT,Interest,0.85,0,CAD
2026-02-04,FPLINT,Interest charge,-12.34,0,CAD
2026-02-05,FEE,Fee,-5.00,0,CAD
2026-02-06,FEE,Fee rebate,5.00,0,CAD
";
        let f = read(text).unwrap();
        let got: Vec<(ActivityType, i64)> = f.activities.iter().map(|a| (a.r#type, a.amount)).collect();
        assert_eq!(
            vec![
                (ActivityType::Tax, 63),
                (ActivityType::Credit, 63),
                (ActivityType::Interest, 85),
                (ActivityType::Fee, 12_34),
                (ActivityType::Fee, 5_00),
                (ActivityType::Credit, 5_00),
            ],
            got
        );
        assert!(f.skipped.is_empty(), "{:?}", f.skipped);
    }

    #[test]
    fn registrations_read_in_english_and_french() {
        assert_eq!(Registration::Fhsa, registration_of("CELIAPP"));
        assert_eq!(Registration::Tfsa, registration_of("CELI"));
        assert_eq!(Registration::Rrsp, registration_of("Spousal RRSP"));
        assert_eq!(Registration::Lira, registration_of("CRI"));
        assert_eq!(Registration::NonRegistered, registration_of("Non enregistré"));
        assert_eq!(Registration::NonRegistered, registration_of("Crypto"));
        assert_eq!(Registration::Other, registration_of("Chequing"));
    }

    #[test]
    fn something_else_is_not_read() {
        assert!(read("").is_err());
        assert!(read("Date,Description,Amount\n2026-09-02,Cafe,-4.50\n").is_err());
        assert_eq!(Err("Wealthsimple reports are in Canadian dollars; this ledger keeps EUR".to_string()), plan(&read(W1).unwrap(), &PlanInput::new("EUR")));
    }

    #[test]
    fn w4_the_plan_for_the_holdings_report_has_the_same_uids_on_both_devices() {
        let mapped = PlanInput { accounts: HashMap::from([("DEMO0001CAD".to_string(), "ws:DEMO0001CAD".to_string())]), ..cad() };
        let p = plan(&read(W1).unwrap(), &mapped).unwrap();
        assert_eq!(vec!["sec:AAPL", "sec:XEQT", "sec:ARKK"], p.securities.iter().map(|s| s.uid.as_str()).collect::<Vec<_>>());
        assert_eq!(
            vec!["hold:ws:DEMO0001CAD:sec:AAPL:2026-05-08", "hold:ws:DEMO0001CAD:sec:XEQT:2026-05-08", "hold:ws:DEMO0001CAD:sec:ARKK:2026-05-08"],
            p.holdings.iter().map(|h| h.uid.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(vec!["px:sec:AAPL:2026-05-08", "px:sec:XEQT:2026-05-08", "px:sec:ARKK:2026-05-08"], p.prices.iter().map(|x| x.uid.as_str()).collect::<Vec<_>>());
        assert_eq!(vec![100 * PRICE_SCALE, 25 * PRICE_SCALE, 50 * PRICE_SCALE], p.prices.iter().map(|x| x.price).collect::<Vec<_>>());
        // AAPL's 1,000 USD at its own 1,000 CAD / 750 USD, XEQT's 250 CAD, ARKK's 50 USD at 50 / 50.
        assert_eq!(vec![("val:ws:DEMO0001CAD:2026-05-08", 1_633_33)], p.values.iter().map(|v| (v.uid.as_str(), v.value)).collect::<Vec<_>>());
        assert_eq!(("USD", SecurityKind::Stock, "NASDAQ"), (p.securities[0].currency.as_str(), p.securities[0].kind, p.securities[0].exchange.as_str()));
        let unmapped = plan(&read(W1).unwrap(), &cad()).unwrap();
        assert_eq!(p, ImportPlan { accounts: vec![], ..unmapped.clone() }, "an account number nobody mapped is the account the import creates");
        let created = PlanAccount {
            uid: "ws:DEMO0001CAD".into(), name: "Demo TFSA".into(), registration: Registration::Tfsa, institution: "Wealthsimple".into(),
            external_ref: "DEMO0001CAD".into(),
        };
        assert_eq!(vec![created], unmapped.accounts);
    }

    #[test]
    fn the_plan_for_the_activities_export_counts_identical_lines() {
        let p = plan(&read(W2).unwrap(), &cad()).unwrap();
        let uids: Vec<&str> = p.activities.iter().map(|a| a.uid.as_str()).collect();
        let account = "ws:HQ7XFMC41CAD";
        assert_eq!(import_uid(&[account, "2026-08-08", "BUY", "sec:XEQT", "1000000000", "38120", "CAD", "0"], 0), uids[0]);
        assert_eq!(import_uid(&[account, "2026-10-01", "FX", "", "0", "13713", "CAD", "0"], 0), uids[4]);
        let twice = format!("{W2}2026-08-08,2026-08-11,HQ7XFMC41CAD,TFSA,Trade,BUY,LONG,XEQT,iShares Core Equity ETF Portfolio,CAD,10,38.12,0,-381.20\n");
        let again = plan(&read(&twice).unwrap(), &cad()).unwrap();
        assert_eq!(&again.activities[..5], &p.activities[..5]);
        let last = again.activities.last().unwrap();
        assert_eq!(import_uid(&[account, "2026-08-08", "BUY", "sec:XEQT", "1000000000", "38120", "CAD", "0"], 1), last.uid);
        let statement = plan(&read(W3).unwrap(), &PlanInput { statement_account: Some("a-uid".into()), ..cad() }).unwrap();
        assert!(statement.activities.iter().all(|a| a.account_uid == "a-uid"));
        assert!(plan(&read(W3).unwrap(), &cad()).is_err(), "a statement names no account");
    }

    #[test]
    fn the_plans_uids_are_these_literally() {
        // For the parity check against `WealthsimpleTest.kt`: W2 into an account the import creates.
        let p = plan(&read(W2).unwrap(), &cad()).unwrap();
        assert_eq!(
            vec![
                "imp:aaa32c148f3cea50442cb54fcf9cf2be",
                "imp:e4abbe25d44f7fabcaab617415bd4fba",
                "imp:c96fe3372f405ada2e34d69409d9fb60",
                "imp:f65185eaae8bb27219141fbb0f61a399",
                "imp:efac3a2723a76720310c10ea87894bc9",
            ],
            p.activities.iter().map(|a| a.uid.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_ledger_in_another_currency_is_refused_and_a_statement_goes_into_the_account_it_is_told() {
        assert!(plan(&read(W2).unwrap(), &PlanInput::new("USD")).is_err());
        let p = plan(&read(W3).unwrap(), &PlanInput { statement_account: Some("acct-9".into()), ..cad() }).unwrap();
        assert!(p.accounts.is_empty());
        assert_eq!("Contribution (executed at 2026-01-02)", p.activities[0].note, "the statement's description is the note");
    }
}
