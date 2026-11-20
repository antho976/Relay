//! The pure pieces of Settings (Kotlin `SettingsLogic.kt`): the currency list, the summaries the
//! rows carry, ordinals, file names, the restore prompt. No drawing here, so tests hold every line.

use relay_money::backup::BackupFile;
use relay_money::copy;
use relay_money::money::{Locale, MoneyFormatter};

/// The currencies offered, the common ones, Canada first.
pub const COMMON_CURRENCIES: [&str; 16] = ["CAD", "USD", "EUR", "GBP", "AUD", "CHF", "JPY", "MXN", "NZD", "SEK", "NOK", "DKK", "INR", "BRL", "CNY", "ZAR"];

/// The common list, with `current` first when it is not one of them, so the pick is never hidden.
pub fn currency_codes(current: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !COMMON_CURRENCIES.contains(&current) {
        out.push(current.to_string());
    }
    out.extend(COMMON_CURRENCIES.iter().map(|c| c.to_string()));
    out
}

/// A currency's name in English, as the phone's locale names the common ones.
pub fn currency_name(code: &str) -> String {
    match code {
        "CAD" => "Canadian dollar",
        "USD" => "US dollar",
        "EUR" => "Euro",
        "GBP" => "British pound",
        "AUD" => "Australian dollar",
        "CHF" => "Swiss franc",
        "JPY" => "Japanese yen",
        "MXN" => "Mexican peso",
        "NZD" => "New Zealand dollar",
        "SEK" => "Swedish krona",
        "NOK" => "Norwegian krone",
        "DKK" => "Danish krone",
        "INR" => "Indian rupee",
        "BRL" => "Brazilian real",
        "CNY" => "Chinese yuan",
        "ZAR" => "South African rand",
        other => other,
    }
    .to_string()
}

/// One currency as a row reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrencyOption {
    pub code: String,
    pub name: String,
    pub symbol: String,
    /// 1234.56 in this currency, at its own number of decimals.
    pub sample: String,
}

impl CurrencyOption {
    /// "$ · e.g. $1,234.56"; the symbol drops out where it is just the code again.
    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if self.symbol != self.code {
            parts.push(self.symbol.clone());
        }
        parts.push(format!("e.g. {}", self.sample));
        parts.join(" · ")
    }
}

pub fn currency_option(code: &str, locale: &Locale) -> CurrencyOption {
    let fmt = MoneyFormatter::new(code, locale.clone());
    let sample = 123_456i64 * relay_money::money::pow10(fmt.fraction_digits) / 100;
    CurrencyOption { code: fmt.currency.clone(), name: currency_name(&fmt.currency), symbol: fmt.symbol.clone(), sample: fmt.format(sample) }
}

/// "1st", "2nd", "3rd", "4th", "11th", "21st".
pub fn ordinal(n: i64) -> String {
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
    format!("{n}{suffix}")
}

/// The Currency and dates row: "CAD · month starts on the 1st · week from Monday".
pub fn format_summary(currency: &str, month_start_day: i64, week_starts_monday: bool) -> String {
    format!("{currency} · month starts on the {} · week from {}", ordinal(month_start_day), if week_starts_monday { "Monday" } else { "Sunday" })
}

/// The Accounts row: "4 accounts · net $10,095".
pub fn accounts_summary(count: usize, net: &str) -> String {
    if count == 0 { "No accounts yet".into() } else { format!("{} · net {net}", copy::plural(count as i64, "account")) }
}

/// The Categories row: "13 for spending · 4 for income".
pub fn categories_summary(spending: usize, income: usize) -> String {
    if spending + income == 0 { "None yet".into() } else { format!("{spending} for spending · {income} for income") }
}

/// "1 entry", "1,284 entries", with the thousands grouped as the phone writes them.
pub fn entries_text(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{grouped} {}", if n == 1 { "entry" } else { "entries" })
}

/// An appearance's name: "System", "Light" or "Dark" (a palette from the old sky look reads as
/// the one it now opens on).
pub fn palette_name(name: &str) -> &'static str {
    match name {
        "light" | "sky" | "dusk" | "mint" => "Light",
        "dark" | "night" => "Dark",
        _ => "System",
    }
}

/// The line under an appearance.
pub fn palette_hint(name: &str) -> &'static str {
    match name {
        "light" | "sky" | "dusk" | "mint" => "White, for daylight",
        "dark" | "night" => "Black, for a dark room",
        _ => "Follows the phone",
    }
}

/// Settings' line under Tally: "1,284 entries since March 2024", or the count alone when the
/// first day cannot be read, or what to do when there are none.
pub fn ledger_line(entries: i64, first: Option<&str>) -> String {
    if entries == 0 {
        return "No entries yet".into();
    }
    let since = first.and_then(|d| d.get(..10)).and_then(relay_money::csv::parse_iso_date).map(|d| d.strftime("%B %Y").to_string());
    match since {
        Some(s) => format!("{} since {s}", entries_text(entries)),
        None => entries_text(entries),
    }
}

pub fn backup_file_name(today: &str) -> String {
    format!("tally-backup-{today}.json")
}

pub fn csv_file_name(today: &str) -> String {
    format!("tally-entries-{today}.csv")
}

/// What a picked backup holds, read before anything is replaced.
#[derive(Clone, Debug, PartialEq)]
pub struct RestorePreview {
    pub entries: usize,
    pub accounts: usize,
    /// The day the file was saved; none when the file does not say.
    pub saved_on: Option<relay_money::Date>,
}

pub fn restore_preview(file: &BackupFile) -> RestorePreview {
    RestorePreview {
        entries: file.transactions.len(),
        accounts: file.accounts.len(),
        saved_on: file.exported_at.get(..10).and_then(relay_money::csv::parse_iso_date),
    }
}

/// The restore confirm: "Replace everything with this backup? It holds 1,284 entries and 4
/// accounts, saved 3 Oct. Every entry, account, budget, bill and goal on this phone is replaced."
pub fn restore_prompt(p: &RestorePreview, saved_on: Option<&str>) -> String {
    format!(
        "Replace everything with this backup? It holds {} and {}{}. Every entry, account, budget, bill and goal on this phone is replaced; Tally keeps a copy of them first.",
        entries_text(p.entries as i64),
        copy::plural(p.accounts as i64, "account"),
        saved_on.map(|d| format!(", saved {d}")).unwrap_or_default(),
    )
}

/// What a restore or a read says when the file is not a backup.
pub fn read_backup(bytes: &[u8]) -> Result<BackupFile, String> {
    if bytes.is_empty() {
        return Err("That file could not be opened.".into());
    }
    let text = relay_money::bank_statements::decode_text(bytes);
    let text = text.strip_prefix(relay_money::csv::BOM).unwrap_or(&text);
    match relay_money::backup::decode(text) {
        relay_money::backup::BackupReadResult::Ok(f) => Ok(f),
        relay_money::backup::BackupReadResult::Invalid(why) => Err(why),
    }
}
