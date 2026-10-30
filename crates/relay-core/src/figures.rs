//! A local model's figures checked against what it read.
//!
//! Qwen3.5 4B gives real figures for the wrong days more often than made-up ones: a three-month
//! total for "this year", the month before's spending for last month, May's workouts for August
//! (bench v3, 2026-10-30). Every place a figure stands, in the readings and in the reply, is read
//! for the days it is about ("this year", "last month (Aug 15 to Sep 14)", "these 3 months", "Mon
//! Sep 7 to Sun Sep 13", "January"). A figure the reply gives for days the data holds it only for
//! others is flagged ([`Flag::Days`]), with what the data has for those days, and so is a total
//! the figures listed before it do not add up to ([`Flag::Total`]). For the reply's answer in bold
//! with no days of its own, the question's days are the ones it is for.
//!
//! On the bench's saved replies it flagged 0.3% (v1), 1.2% (v2 and v3) and 0.8% (v3, round seven)
//! of those graded right, most of them a wrong figure no check read, and 13–17% of those that gave
//! a wrong figure. A figure that is in no reading and made from none is not flagged: a small model
//! works out more than one step often enough that flagging those would mostly be wrong.

use std::collections::HashMap;
use std::sync::LazyLock;

use jiff::civil::Date;
use jiff::{Span as Days, ToSpan};
use regex::Regex;
use serde_json::{json, Value};

/// The first and last day a figure is for.
type Span = (Date, Date);

/// A figure the data does not bear out.
#[derive(Debug, Clone, PartialEq)]
pub enum Flag {
    /// Given for `said` (the reply's days, or the question's when `by_question`), and held by the
    /// data only for `found`. `right` is what the data has for `said`; money with nothing there
    /// was not read for those days.
    Days { figure: String, said: String, by_question: bool, found: Vec<String>, right: Vec<String> },
    /// A total neither in the data nor made from the figures beside it; `listed` is how many of
    /// its kind were listed before it and what they add up to.
    Total { figure: String, listed: Option<(usize, String)> },
}

impl Flag {
    fn figure(&self) -> &str {
        match self {
            Flag::Days { figure, .. } | Flag::Total { figure, .. } => figure,
        }
    }

    /// The flag as the model is told it.
    fn told(&self) -> String {
        match self {
            Flag::Days { figure, said, by_question, found, right } => {
                let given = if *by_question { format!("the question asks about {said}") } else { format!("you gave it for {said}") };
                let then = if right.is_empty() {
                    format!(" Nothing you read covers {said}: read those days first (from and to).")
                } else {
                    format!(" For {said} the data has {}.", right.join(", "))
                };
                format!("- {figure}: {given}, but the data has {figure} for {}.{then}", found.join("; "))
            }
            Flag::Total { figure, listed: Some((n, sum)) } => format!("- {figure}: the {n} figures of its kind listed before it add up to {sum}."),
            Flag::Total { figure, listed: None } => format!("- {figure}: a total that neither the data nor the figures beside it make."),
        }
    }

    /// The flag as the person is told it.
    fn said(&self) -> String {
        match self {
            Flag::Days { figure, said, found, .. } => format!("{figure} is the figure for {}, not {said}", found.join("; ")),
            Flag::Total { figure, listed: Some((_, sum)) } => format!("{figure} is not what the figures listed add up to ({sum})"),
            Flag::Total { figure, listed: None } => format!("{figure} is a total the data does not give"),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Flag::Days { figure, said, by_question, found, right } => json!({"kind": "days", "figure": figure, "said": said, "by_question": by_question, "found": found, "right": right}),
            Flag::Total { figure, listed } => json!({"kind": "total", "figure": figure, "listed": listed.as_ref().map(|(n, sum)| json!({"count": n, "sum": sum}))}),
        }
    }
}

/// What the model is told when its answer had `flags`, to answer again.
pub fn correction(flags: &[Flag]) -> String {
    let lines: Vec<String> = flags.iter().map(Flag::told).collect();
    format!(
        "[Relay checked the figures in your answer against the data you read, and these do not match:\n{}\n\
         Answer my question again in full, with these figures put right. Keep any figure that is right as it is.]",
        lines.join("\n")
    )
}

/// What the person is told: the figures that did not match, and whether the model was asked again.
pub fn note(flags: &[Flag], asked_again: bool) -> String {
    let named: Vec<String> = flags.iter().map(Flag::said).collect();
    if asked_again {
        format!("Relay checked the figures against your data: {}. It asked for them to be put right.", named.join("; "))
    } else {
        format!("These figures still do not match your data: {}.", named.join("; "))
    }
}

// ── figures ──────────────────────────────────────────────────────────────

/// Month names in English and French, short and long; the longest first, as an alternation tries
/// them.
const MONTHS: &[(&str, i8)] = &[
    ("september", 9), ("septembre", 9), ("decembre", 12), ("décembre", 12), ("novembre", 11), ("november", 11), ("december", 12),
    ("february", 2), ("fevrier", 2), ("février", 2), ("juillet", 7), ("january", 1), ("janvier", 1), ("october", 10), ("octobre", 10),
    ("august", 8), ("april", 4), ("avril", 4), ("march", 3), ("june", 6), ("july", 7), ("juin", 6), ("mars", 3), ("août", 8),
    ("aout", 8), ("sept", 9), ("janv", 1), ("févr", 2), ("juil", 7), ("jan", 1), ("feb", 2), ("mar", 3), ("apr", 4), ("avr", 4),
    ("may", 5), ("mai", 5), ("jun", 6), ("jul", 7), ("aug", 8), ("sep", 9), ("oct", 10), ("nov", 11), ("dec", 12), ("déc", 12),
];
const WEEKDAY: &str = r"(?:(?:mon|tue|wed|thu|fri|sat|sun)[a-z]*|(?:lun|mar|mer|jeu|ven|sam|dim)[a-z]*)\.?,?\s+";

fn month_of(word: &str) -> Option<i8> {
    let lower = word.to_lowercase();
    MONTHS.iter().find(|(name, _)| *name == lower.trim_end_matches('.')).map(|(_, n)| *n)
}

static MONTH: LazyLock<String> = LazyLock::new(|| MONTHS.iter().map(|(name, _)| *name).collect::<Vec<_>>().join("|"));

/// Spans with digits in them that are no figure: dates, clock times, ids, years, list numbers,
/// a chart's JSON.
static BLANKS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let m = MONTH.as_str();
    [
        r"(?is)```(?:chart|import)\b.*?```".to_string(),
        r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?Z?)?\b".into(),
        r"(?i)\b\d{1,2}:\d{2}(?::\d{2})?\s?(?:[ap]\.?m\.?)?".into(),
        format!(r"(?i)\b(?:{m})\.?\s+\d{{1,2}}(?:st|nd|rd|th)?\b(?:\s*[-–]\s*\d{{1,2}}\b)?(?:,?\s+\d{{4}})?"),
        format!(r"(?i)\b\d{{1,2}}(?:er|st|nd|rd|th)?\s+(?:{m})\b\.?(?:\s+\d{{4}})?"),
        r"(?i)#\d+|\[id \d+\]|\bid:?\s*\d+|\b(?:19|20)\d{2}\b|period_offset:? ?-?\d+|\bQ[1-4]\b|\b\d+ (?:days?|weeks?|months?) (?:ago|left)\b|\bday \d+ of \d+".into(),
        r"(?m)^\s*(?:\d+[.)]|[-*])\s".into(),
    ]
    .iter()
    .map(|rx| Regex::new(rx).expect("a blank pattern"))
    .collect()
});

/// `text` with `re`'s matches turned to spaces, every byte keeping its place.
fn spaced(text: &str, re: &Regex) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for m in re.find_iter(text) {
        bytes[m.start()..m.end()].fill(b' ');
    }
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_string())
}

fn blank(text: &str) -> String {
    BLANKS.iter().fold(text.to_string(), |t, re| spaced(&t, re))
}

#[derive(Debug, Clone, Copy)]
struct Figure {
    value: f64,
    decimals: i32,
    start: usize,
    end: usize,
}

impl Figure {
    fn cents(&self) -> i64 {
        cents(self.value)
    }
}

fn cents(v: f64) -> i64 {
    (v * 100.0).round() as i64
}

/// Each figure in `text`: "$1,234.56", "1234.56", "29.4%", and French "109,15 $" and "1 234,56 $".
fn figures(text: &str) -> Vec<Figure> {
    let b = blank(text);
    let s = b.as_bytes();
    let digits = |from: usize| from + s.get(from..).map_or(0, |rest| rest.iter().take_while(|c| c.is_ascii_digit()).count());
    // ",15" before no digit or comma: a French decimal comma, not thousands.
    let comma_decimals = |at: usize| -> Option<usize> {
        if s.get(at) != Some(&b',') {
            return None;
        }
        let end = digits(at + 1);
        ((1..=2).contains(&(end - at - 1)) && s.get(end) != Some(&b',')).then_some(end)
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if !s[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let lead = digits(i);
        if b[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ',')) {
            i = lead;
            continue;
        }
        let mut whole = b[i..lead].to_string();
        let mut end = lead;
        if lead - i <= 3 {
            let mut j = end;
            while s.get(j) == Some(&b',') && digits(j + 1) == j + 4 {
                whole.push_str(&b[j + 1..j + 4]);
                j += 4;
            }
            if j > end {
                end = j;
            } else {
                let mut k = end;
                let mut groups = String::new();
                while let Some(c @ (' ' | '\u{a0}' | '\u{202f}')) = b[k..].chars().next() {
                    let n = k + c.len_utf8();
                    if digits(n) != n + 3 {
                        break;
                    }
                    groups.push_str(&b[n..n + 3]);
                    k = n + 3;
                }
                if !groups.is_empty() && comma_decimals(k).is_some() {
                    whole.push_str(&groups);
                    end = k;
                }
            }
        }
        let mut frac = "";
        if s.get(end) == Some(&b'.') && s.get(end + 1).is_some_and(u8::is_ascii_digit) {
            let e = digits(end + 1);
            frac = &b[end + 1..e];
            end = e;
        } else if let Some(e) = comma_decimals(end) {
            frac = &b[end + 1..e];
            end = e;
        }
        let value = format!("{whole}.{}", if frac.is_empty() { "0" } else { frac }).parse().unwrap_or(0.0);
        out.push(Figure { value, decimals: frac.len() as i32, start: i, end });
        i = end;
    }
    out
}

/// `i` moved back to the start of the character it falls in.
fn floor(text: &str, mut i: usize) -> usize {
    i = i.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// What a figure counts: "$" for money, else the word after it ("workout", "km", "min").
fn noun(text: &str, f: &Figure) -> String {
    let before = &text[floor(text, f.start.saturating_sub(3))..f.start];
    let after = &text[f.end..];
    if before.contains('$') || after.trim_start().starts_with('$') || f.decimals == 2 {
        return "$".into();
    }
    let rest = after.trim_start();
    let word: String = if rest.starts_with('%') { "%".into() } else { rest.chars().take_while(|c| c.is_alphabetic()).collect::<String>().to_lowercase() };
    let word = if word.ends_with('s') && word.chars().count() > 3 { word[..word.len() - 1].to_string() } else { word };
    match word.as_str() {
        "minute" | "mn" => "min".into(),
        "kilometre" | "kilometer" => "km".into(),
        "lbs" | "pound" => "lb".into(),
        "kilo" => "kg".into(),
        "session" | "entraînement" | "séance" => "workout".into(),
        "dollar" => "$".into(),
        _ => word,
    }
}

// ── days ─────────────────────────────────────────────────────────────────

/// Today, and the day of the month a budget month starts on.
struct Clock {
    today: Date,
    start_day: i8,
}

impl Clock {
    fn of(texts: &[&str], today: Date) -> Clock {
        static STARTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)THIS MONTH: (?:\w+ \d{{4}} \()?(?:{})\.? (\d+) to", MONTH.as_str())).expect("starts"));
        let start_day = texts.iter().find_map(|t| STARTS.captures(t)).and_then(|c| c[1].parse().ok()).filter(|d| (1..=28).contains(d)).unwrap_or(1);
        Clock { today, start_day }
    }

    /// `month`/`day` in the most recent year it is not well after today.
    fn day(&self, month: i8, day: i8) -> Option<Date> {
        let y = self.today.year();
        let d = Date::new(y, month, day).ok()?;
        if d <= self.today.checked_add(60.days()).ok()? { Some(d) } else { Date::new(y - 1, month, day.min(28)).ok() }
    }

    /// A calendar month, the most recent one that has begun, to today at most.
    fn month(&self, month: i8) -> Span {
        let y = if month <= self.today.month() { self.today.year() } else { self.today.year() - 1 };
        let first = Date::new(y, month, 1).unwrap_or(self.today);
        (first, first.last_of_month().min(self.today))
    }

    /// The budget month `back` months before this one, to today at most.
    fn budget(&self, back: i32) -> Span {
        let t = self.today;
        let this = Date::new(t.year(), t.month(), self.start_day).unwrap_or(t);
        let this = if t.day() >= self.start_day { this } else { this.checked_sub(1.month()).unwrap_or(this) };
        let first = this.checked_sub(Days::new().months(back)).unwrap_or(this);
        let last = first.checked_add(1.month()).ok().and_then(|n| n.yesterday().ok()).unwrap_or(first);
        (first, last.min(t))
    }

    fn week(&self, back: i64) -> Span {
        let t = self.today;
        let monday = t.checked_sub(Days::new().days(i64::from(t.weekday().to_monday_zero_offset()) + 7 * back)).unwrap_or(t);
        (monday, monday.checked_add(6.days()).unwrap_or(monday).min(t))
    }
}

static RANGE: LazyLock<Regex> = LazyLock::new(|| {
    let m = MONTH.as_str();
    Regex::new(&format!(
        r"(?i)(?:{WEEKDAY})?\b(?P<m1>{m})\.?\s+(?P<d1>\d{{1,2}})(?:,?\s+\d{{4}})?\s*(?:to|–|—|-|au|through)\s*(?:(?P<today>today)|(?:{WEEKDAY})?(?:(?P<m2>{m})\.?\s+)?(?P<d2>\d{{1,2}})\b)"
    ))
    .expect("range")
});
static MONTHS_RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\b(?P<a>{m})\.?\s*(?:-|–|to|à)\s*(?P<b>{m})\b", m = MONTH.as_str())).expect("months"));
static ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\b(?P<m>{})\b", MONTH.as_str())).expect("alone"));
static DAY_AFTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\.?\s*\d{1,2}\b").expect("day after"));
static QUARTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bQ([1-4])\b").expect("quarter"));

/// Days named by words, each with what may not stand right before or after it.
static WORDS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("today", r"\btoday\b|aujourd'hui"),
        ("yesterday", r"\byesterday\b|\bhier\b"),
        ("this week", r"\bthis week\b|cette semaine|\bweek so far\b"),
        ("last week", r"\blast week\b|la semaine dernière|semaine passée"),
        ("before last", r"\bmonth before(?: last)?\b|mois d'avant|avant-dernier mois|\b2 months ago\b"),
        ("last month", r"\blast month\b|\bprevious month\b|le mois dernier|mois passé|\b1 month ago\b"),
        ("this month", r"\bthis (?:budget )?month\b|ce mois-ci|\bce mois\b|\bcurrent month\b"),
        ("three months", r"\bthese 3 months\b|\b(?:last|past) (?:3|three) months\b|\bthe 3 months\b"),
        ("this year", r"\bthis year\b|cette année|\byear to date\b|\bytd\b|\bso far this year\b|\bin 20\d\d so far\b"),
        ("ever", r"\ball time\b|\bwhole ledger\b|\bever\b|\bdepuis toujours\b|\ball (?:my|your) entries\b"),
        ("last year", r"\blast year\b|l'année dernière|l'an dernier|l'an passé"),
    ]
    .into_iter()
    .map(|(name, rx)| (name, Regex::new(&format!("(?i){rx}")).expect("a word")))
    .collect()
});

fn word_found(name: &str, re: &Regex, text: &str) -> bool {
    re.find_iter(text).any(|m| {
        let before = text[..m.start()].to_lowercase();
        let after = text[m.end()..].to_lowercase();
        match name {
            // "the month before last" is not last month.
            "last month" => !before.ends_with("before "),
            // "the heaviest ever lifted" is no span.
            "ever" if m.as_str().eq_ignore_ascii_case("ever") => {
                !["heaviest ", "best ", "longest ", "most "].iter().any(|w| before.ends_with(w)) && !after.starts_with(" lifted")
            }
            _ => true,
        }
    })
}

/// The spans of days `text` names.
fn spans(text: &str, clock: &Clock) -> Vec<Span> {
    let t = clock.today;
    let mut out: Vec<Span> = Vec::new();
    let mut add = |s: Span| {
        if !out.contains(&s) {
            out.push(s);
        }
    };
    for c in RANGE.captures_iter(text) {
        let (Some(m1), Ok(d1)) = (month_of(&c["m1"]), c["d1"].parse::<i8>()) else { continue };
        let m2 = c.name("m2").and_then(|m| month_of(m.as_str())).unwrap_or(m1);
        let last = if c.name("today").is_some() { Some(t) } else { c.name("d2").and_then(|d| d.as_str().parse().ok()).and_then(|d| clock.day(m2, d)) };
        if let (Some(mut a), Some(b)) = (clock.day(m1, d1), last) {
            if b < a && a.year() == b.year() {
                a = Date::new(a.year() - 1, a.month(), a.day()).unwrap_or(a);
            }
            add((a, b.min(t)));
        }
    }
    let rest = spaced(text, &RANGE);
    for (name, re) in WORDS.iter() {
        if word_found(name, re, &rest) {
            add(match *name {
                "today" => (t, t),
                "yesterday" => (t.yesterday().unwrap_or(t), t.yesterday().unwrap_or(t)),
                "this week" => clock.week(0),
                "last week" => clock.week(1),
                "this month" => clock.budget(0),
                "last month" => clock.budget(1),
                "before last" => clock.budget(2),
                "three months" => (clock.budget(2).0, t),
                "this year" => (Date::new(t.year(), 1, 1).unwrap_or(t), t),
                "ever" => (Date::new(1970, 1, 1).unwrap_or(t), t),
                _ => (Date::new(t.year() - 1, 1, 1).unwrap_or(t), Date::new(t.year() - 1, 12, 31).unwrap_or(t)),
            });
        }
    }
    let mut months = rest.clone();
    for c in MONTHS_RANGE.captures_iter(&rest) {
        let whole = c.get(0).expect("the match");
        let before = rest[..whole.start()].chars().next_back();
        if before == Some('-') || DAY_AFTER.is_match(&rest[whole.end()..]) {
            continue;
        }
        if let (Some(a), Some(b)) = (month_of(&c["a"]), month_of(&c["b"])) {
            add((clock.month(a).0, clock.month(b).1));
            months = months[..whole.start()].to_string() + &" ".repeat(whole.len()) + &months[whole.end()..];
        }
    }
    for c in QUARTER.captures_iter(&months) {
        let q: i8 = c[1].parse().unwrap_or(1);
        let first = Date::new(t.year(), 3 * q - 2, 1).unwrap_or(t);
        let last = first.checked_add(3.months()).ok().and_then(|d| d.yesterday().ok()).unwrap_or(t);
        add((first, last.min(t)));
    }
    for m in ALONE.find_iter(&months) {
        let word = m.as_str();
        let (before, after) = (months[..m.start()].chars().next_back(), &months[m.end()..]);
        // "May" as a verb, "mar" and the like in a word; a month with a day is a date.
        let verb = ["may", "mar", "sep", "dec"].contains(&word) && word.starts_with(|c: char| c.is_lowercase());
        if verb || before == Some('-') || after.starts_with('-') || DAY_AFTER.is_match(after) {
            continue;
        }
        if let Some(n) = month_of(word) {
            add(clock.month(n));
        }
    }
    out
}

/// Two spans for the same days, give or take one at the start and three at the end (a month to
/// today, or a budget month cut at its last day).
fn same(a: Span, b: Span) -> bool {
    let apart = |x: Date, y: Date| (x - y).get_days().abs();
    apart(a.0, b.0) <= 1 && apart(a.1, b.1) <= 3
}

/// The same days as people say them: "August" for the budget month that starts in it, and
/// "today" for a figure that runs to today ("as of today, 2,540.98 left").
fn loose(a: Span, b: Span) -> bool {
    let starts_in = |cal: Span, s: Span| cal.0.day() == 1 && (s.0.year(), s.0.month()) == (cal.0.year(), cal.0.month());
    let to_today = |day: Span, s: Span| day.0 == day.1 && day.1 == s.1;
    same(a, b) || starts_in(a, b) || starts_in(b, a) || to_today(a, b) || to_today(b, a)
}

fn describe(s: Span, today: Date) -> String {
    let day = |d: Date| if d.year() == today.year() { d.strftime("%b %-d").to_string() } else { d.strftime("%b %-d %Y").to_string() };
    if s.0.year() == 1970 {
        "the whole ledger".into()
    } else if s.0 == s.1 {
        s.0.strftime("%a %b %-d").to_string()
    } else {
        format!("{} to {}", day(s.0), day(s.1))
    }
}

// ── the data ─────────────────────────────────────────────────────────────

/// Words in a reading that name no span of their own: the date it was read on, "so far", "by
/// today" in a month's running figures.
static NOT_A_SPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Today is [^.]*\.|\((?:today|yesterday|\d+ days? ago|in \d+ days?)\)|DATES \([^)]*\):.*|\b(?:by|until|through) (?:today|this day)\b|counting today|\bat the same day\b|\bso far\b")
        .expect("not a span")
});
/// The readings' headings that put the lines under them in a span.
static SCOPES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:THIS MONTH|LAST MONTH|RECENT DAYS|THIS YEAR|THE WHOLE LEDGER|ALL TIME)\b").expect("scopes"));
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z][A-Z ,'()&-]{3,}[A-Z)]").expect("heading"));
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-zéè&]+").expect("word"));

/// One place a figure stands in the data: its span (none when unknown or more than one), what it
/// counts, and the word before it ("spent", "income").
struct Place {
    span: Option<Span>,
    noun: String,
    label: String,
}

struct Data {
    places: HashMap<i64, Vec<Place>>,
}

impl Data {
    fn of(texts: &[&str], clock: &Clock) -> Data {
        let mut places: HashMap<i64, Vec<Place>> = HashMap::new();
        for text in texts {
            let mut section: Vec<Span> = Vec::new();
            for line in text.lines() {
                let trimmed = line.trim();
                if SCOPES.is_match(trimmed) {
                    section = spans(&spaced(trimmed, &NOT_A_SPAN), clock);
                } else if HEADING.is_match(trimmed) {
                    section.clear();
                }
                for part in line.split("; ") {
                    let part = spaced(part, &NOT_A_SPAN);
                    let figs = figures(&part);
                    for (n, f) in figs.iter().enumerate() {
                        // Its label: the words since the figure before it, else its clause.
                        let from = if n == 0 { 0 } else { figs[n - 1].end };
                        let own = spans(&part[from..f.start], clock);
                        let own = if own.is_empty() { spans(&part, clock) } else { own };
                        let span = if own.is_empty() { &section } else { &own };
                        let label = WORD.find_iter(&part[from..f.start]).last().map(|w| w.as_str().to_lowercase()).unwrap_or_default();
                        places.entry(f.cents()).or_default().push(Place { span: (span.len() == 1).then(|| span[0]), noun: noun(&part, f), label });
                    }
                }
            }
        }
        Data { places }
    }

    /// The places of a figure at the precision it is shown with; `None` when the data has none.
    fn at(&self, f: &Figure) -> Option<Vec<&Place>> {
        if let Some(found) = self.places.get(&f.cents()) {
            return Some(found.iter().collect());
        }
        if f.decimals >= 2 {
            return None;
        }
        let step = 10f64.powi(-f.decimals);
        let round = |v: f64| (v / step).round();
        let found: Vec<&Place> = self.places.iter().filter(|(c, _)| {
            let v = **c as f64 / 100.0;
            round(v) == round(f.value) && (v - f.value).abs() < step
        }).flat_map(|(_, p)| p).collect();
        (!found.is_empty()).then_some(found)
    }

    /// The figures of `noun` the data has for `span`: those with one of `labels` when there are.
    fn for_span(&self, span: Span, noun: &str, labels: &[&str]) -> Vec<i64> {
        let mut hits: Vec<(i64, &str)> = self.places.iter().flat_map(|(c, ps)| ps.iter().map(move |p| (*c, p)))
            .filter(|(_, p)| p.noun == noun && p.span.is_some_and(|s| same(span, s)))
            .map(|(c, p)| (c, p.label.as_str()))
            .collect();
        hits.sort_unstable();
        let mut named: Vec<i64> = hits.iter().filter(|(_, l)| labels.contains(l)).map(|(c, _)| *c).collect();
        if named.is_empty() {
            named = hits.iter().map(|(c, _)| *c).collect();
        }
        named.dedup();
        named
    }
}

// ── the reply ────────────────────────────────────────────────────────────

static SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.!?](?:\*\*)?\s+").expect("sentence"));
static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*[^*\n]+\*\*").expect("bold"));
static RULE_ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\|[\s:|-]+\|\s*$").expect("rule"));
static PERIOD_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)^(?:the |this |last |previous |month|week|year|today|yesterday|ce |cette |le |la |mois|semaine|{}|q[1-4]|\d)", MONTH.as_str())).expect("period heading")
});
static TOTAL_BEFORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:total|totaling|in all|altogether|combined|sum|overall|en tout|au total|=)[^\n\d]{0,40}$").expect("total"));
static TOTAL_AFTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:\w+\s+){0,2}(?:in all|in total|altogether|en tout|au total)\b").expect("total after"));

/// The sentence ends in `text` between `from` and `to`: a stop, then a capital or a bracket.
fn sentence_ends(text: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
    SENTENCE_END.find_iter(&text[from..to])
        .filter(|m| text[from + m.end()..].starts_with(|c: char| c.is_uppercase() || c == '*' || c == '('))
        .map(|m| (from + m.start(), from + m.end()))
        .collect()
}

/// The span the reply gives the figure at `start` for: the words since the figure before it in
/// its sentence; else its sentence's, when that names only one; for a line's first figure, its
/// line's words or the heading above it; in a table, its row's first cell and its column's head.
fn reply_spans(text: &str, start: usize, clock: &Clock) -> Vec<Span> {
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    let line = &text[line_start..line_end];
    if line.trim_start().starts_with('|') {
        let cells: Vec<&str> = line.split('|').collect();
        let col = text[line_start..start].matches('|').count();
        let mut out = spans(cells.get(1).copied().unwrap_or(""), clock);
        let head = text[..line_start].lines().find(|l| l.trim_start().starts_with('|') && !RULE_ROW.is_match(l));
        if let Some(cell) = head.and_then(|h| h.split('|').nth(col)) {
            out.extend(spans(cell, clock));
        }
        return out;
    }
    let sentence_start = sentence_ends(text, line_start, start).last().map_or(line_start, |e| e.1);
    // Its clause: "faster than last month's pace — you're $579.02 over" is this month's.
    let clause_start = [" — ", " – ", "; "].iter().filter_map(|sep| text[sentence_start..start].rfind(sep).map(|i| sentence_start + i + sep.len())).max().unwrap_or(sentence_start);
    let before = &text[clause_start..start];
    if let Some(prev) = figures(before).last() {
        let own = spans(&before[prev.end..], clock);
        if !own.is_empty() {
            return own;
        }
        // No days of its own: the ones its sentence named before it, when there is one. Not the
        // ones after it, which explain it ("14 (10 from June + 4 ...)").
        let earlier = spans(&text[sentence_start..start], clock);
        return if earlier.len() == 1 { earlier } else { Vec::new() };
    }
    let own = spans(before, clock);
    if !own.is_empty() || clause_start != line_start {
        return own;
    }
    for l in text[..line_start].lines().rev() {
        let s = l.trim();
        if s.is_empty() {
            continue;
        }
        if s.starts_with('#') || s.starts_with("**") || s.trim_end_matches('*').ends_with(':') {
            let head = s.trim_start_matches(['#', '*', ' ']);
            let found = spans(&head[..floor(head, 40)], clock);
            return if PERIOD_HEAD.is_match(head) { found } else { Vec::new() };
        }
        break;
    }
    Vec::new()
}

const CONSTANTS: [f64; 9] = [2.0, 3.0, 4.0, 7.0, 12.0, 24.0, 26.0, 52.0, 100.0];

fn near(x: f64, f: &Figure) -> bool {
    (x.abs() - f.value).abs() <= 0.5 * 10f64.powi(-f.decimals) + 1e-9
}

/// A difference or ratio of two figures given before it: "40 − 25 = 15".
fn worked_out(i: usize, figs: &[Figure]) -> bool {
    let before = &figs[..i];
    before.iter().enumerate().any(|(a, x)| before.iter().enumerate().any(|(b, y)| a != b && (near(x.value - y.value, &figs[i]) || (y.value != 0.0 && near(x.value / y.value, &figs[i])))))
}

/// One step of arithmetic on the reply's other figures: a sum of a run of them, of the ones of its
/// kind before it, or two of them (or one and a count like 12 or 52) added, taken, multiplied,
/// divided or as a percentage.
fn made_from(i: usize, figs: &[Figure], nouns: &[String]) -> bool {
    let f = &figs[i];
    let alike: Vec<f64> = (0..i).filter(|j| nouns[*j] == nouns[i]).map(|j| figs[j].value).collect();
    // Of its kind before it: a run of them added up, or what is left of a total once the ones
    // after it are taken ("406.95 in all: 113.46, 91.88, and 3 others totaling 201.61").
    if (0..alike.len().saturating_sub(1)).any(|a| near(alike[a..].iter().sum(), f) || near(alike[a] - alike[a + 1..].iter().sum::<f64>(), f)) {
        return true;
    }
    for a in 0..figs.len() {
        let mut total = 0.0;
        for (b, g) in figs.iter().enumerate().skip(a) {
            if b == i {
                break;
            }
            total += g.value;
            if b > a && near(total, f) {
                return true;
            }
        }
    }
    let others: Vec<f64> = figs.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, g)| g.value).collect();
    others.iter().any(|&x| others.iter().copied().chain(CONSTANTS).any(|y| {
        let mut made = vec![x + y, x - y, x * y];
        if y != 0.0 {
            made.extend([x / y, x / y * 100.0, (x - y) / y * 100.0]);
        }
        made.into_iter().any(|z| near(z, f))
    }))
}

/// "3,042.30": an amount as the readings write it, without its currency.
fn money(v: f64) -> String {
    let c = cents(v).abs();
    let digits = (c / 100).to_string();
    let mut whole = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            whole.push(',');
        }
        whole.push(d);
    }
    format!("{}{whole}.{:02}", if v < 0.0 { "-" } else { "" }, c % 100)
}

fn shown(c: i64, noun: &str) -> String {
    let v = c as f64 / 100.0;
    if noun == "$" {
        money(v)
    } else if c.rem_euclid(100) == 0 {
        format!("{}", c / 100)
    } else {
        format!("{v}")
    }
}

/// The figures in `reply` that `read` (everything the model read: readings and tool results) does
/// not bear out, for a question `asked` on `today`.
pub fn check(reply: &str, read: &[&str], asked: &str, today: Date) -> Vec<Flag> {
    let clock = Clock::of(read, today);
    let data = Data::of(read, &clock);
    let said: Vec<i64> = figures(asked).iter().map(Figure::cents).collect();
    let asked_spans = spans(asked, &clock);
    let figs = figures(reply);
    let nouns: Vec<String> = figs.iter().map(|f| noun(reply, f)).collect();
    let blanked = blank(reply);
    let bold: Vec<(usize, usize)> = BOLD.find_iter(reply).map(|m| (m.start(), m.end())).collect();
    let mut flags: Vec<Flag> = Vec::new();
    for (i, f) in figs.iter().enumerate() {
        let figure = reply[f.start..f.end].to_string();
        if said.contains(&f.cents()) || (f.decimals == 0 && f.value <= 2.0) || flags.iter().any(|g| g.figure() == figure) {
            continue;
        }
        if let Some(places) = data.at(f) {
            let mut mine = reply_spans(reply, f.start, &clock);
            let mut by_question = false;
            if mine.is_empty() && asked_spans.len() == 1 && bold.iter().any(|(a, b)| *a <= f.start && f.end <= *b) {
                (mine, by_question) = (asked_spans.clone(), true);
            }
            let alike: Vec<&&Place> = places.iter().filter(|p| p.noun == nouns[i]).collect();
            let alike: Vec<&&Place> = if alike.is_empty() { places.iter().collect() } else { alike };
            let [m] = mine.as_slice() else { continue };
            if alike.iter().any(|p| p.span.is_none_or(|s| loose(*m, s))) {
                continue;
            }
            // Shown rounded ("about $240"), it may be the reply's own working, not the data's.
            let rounded = !data.places.contains_key(&f.cents()) || (nouns[i] == "$" && f.decimals < 2);
            if (rounded && made_from(i, &figs, &nouns)) || worked_out(i, &figs) {
                continue;
            }
            let labels: Vec<&str> = alike.iter().map(|p| p.label.as_str()).collect();
            let mut right = data.for_span(*m, &nouns[i], &labels);
            // The one meant is most often near the one given: the nearest first.
            let off = |c: &i64| ((*c as f64).abs().max(1.0) / f.cents().abs().max(1) as f64).ln().abs();
            right.sort_by(|a, b| off(a).total_cmp(&off(b)));
            if right.is_empty() && nouns[i] != "$" {
                // A count for days the data does not add up: counted from a list, it may be right.
                continue;
            }
            let mut found: Vec<String> = alike.iter().filter_map(|p| p.span).map(|s| describe(s, today)).collect();
            found.sort();
            found.dedup();
            found.truncate(3);
            flags.push(Flag::Days {
                figure, said: describe(*m, today), by_question, found,
                right: right.iter().take(4).map(|c| shown(*c, &nouns[i])).collect(),
            });
            continue;
        }
        if made_from(i, &figs, &nouns) {
            continue;
        }
        let line_start = reply[..f.start].rfind('\n').map_or(0, |i| i + 1);
        let window = &blanked[floor(&blanked, line_start.max(f.start.saturating_sub(60)))..floor(&blanked, f.start)];
        if TOTAL_BEFORE.is_match(window) || TOTAL_AFTER.is_match(&reply[f.end..]) {
            let listed: Vec<f64> = (0..i).filter(|j| nouns[*j] == nouns[i]).map(|j| figs[j].value).collect();
            let sum = || {
                let total: f64 = listed.iter().sum();
                if nouns[i] == "$" { money(total) } else { format!("{:.*}", f.decimals as usize, total) }
            };
            flags.push(Flag::Total { figure, listed: (listed.len() >= 2).then(|| (listed.len(), sum())) });
        }
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> Date {
        Date::new(2026, 9, 18).unwrap()
    }

    const SUMMARY: &str = "TALLY (budget app). Today is Friday, September 18, 2026 (2026-09-18).\n\
        THIS MONTH: Sep 15 to Oct 14, day 4 of 30, 27 days left counting today.\n\
        - Spent so far (all expenses): $1,059.02\n\
        RECENT DAYS:\n\
        - This week so far (Mon Sep 14 to today): $1,084.68 on 14 expenses\n\
        - Last week (Mon Sep 7 to Sun Sep 13): $477.30 on 7 expenses\n\
        MONTH BY MONTH (budget periods, oldest first): Jun 15 to Jul 14 2026 (period_offset -3): spent $3,042.30, income $4,907.25; \
        Jul 15 to Aug 14 2026 (period_offset -2): spent $3,099.81, income $7,237.50; Aug 15 to Sep 14 2026 (period_offset -1): spent $3,787.75, income $5,886.40.\n\
        - No category: the month before last (Jul 15 to Aug 14) none logged in Tally; last month (Aug 15 to Sep 14) $85.59; this month (Sep 15 to Oct 14) so far $69.85; these 3 months together $155.44\n\
        THE WHOLE LEDGER (every entry, since Fri Aug 15 2025): 685 entries; spent $51,582.12; spent with no category: $214.19 (7 expenses).\n";

    #[test]
    fn figures_are_read_as_people_write_them() {
        let values: Vec<f64> = figures("Spent $1,059.02 (29.4%) on Sep 15, 2026, 109,15 $ and 1 234,56 $; #685 at 18:05, 3 workouts").iter().map(|f| f.value).collect();
        assert_eq!(values, [1059.02, 29.4, 109.15, 1234.56, 3.0]);
        assert_eq!(money(3042.3), "3,042.30");
        assert_eq!(money(214.19), "214.19");
    }

    #[test]
    fn days_are_read_as_spans() {
        let clock = Clock::of(&[SUMMARY], today());
        let d = |m, day| Date::new(2026, m, day).unwrap();
        assert_eq!(spans("last month (Aug 15 to Sep 14)", &clock), [(d(8, 15), d(9, 14))]);
        assert_eq!(spans("this year", &clock), [(d(1, 1), d(9, 18))]);
        assert_eq!(spans("the month before last", &clock), [(d(7, 15), d(8, 14))]);
        assert_eq!(spans("Mon Sep 14 to today", &clock), [(d(9, 14), d(9, 18))]);
        assert_eq!(spans("in August", &clock), [(d(8, 1), d(8, 31))]);
        assert_eq!(spans("Q2", &clock), [(d(4, 1), d(6, 30))]);
        assert_eq!(spans("May-June 2026", &clock), [(d(5, 1), d(6, 30))]);
        assert!(spans("you may want to", &clock).is_empty() && spans("the heaviest ever lifted", &clock).is_empty());
    }

    #[test]
    fn a_figure_given_for_the_wrong_month_names_the_right_one() {
        let reply = "**Month before last (Jul 15 to Aug 14):**\n- Spent: **$3,042.30**\n";
        let flags = check(reply, &[SUMMARY], "compare last month with the month before", today());
        assert_eq!(flags, [Flag::Days { figure: "3,042.30".into(), said: "Jul 15 to Aug 14".into(), by_question: false, found: vec!["Jun 15 to Jul 14".into()], right: vec!["3,099.81".into()] }]);
        assert!(correction(&flags).contains("- 3,042.30: you gave it for Jul 15 to Aug 14, but the data has 3,042.30 for Jun 15 to Jul 14. For Jul 15 to Aug 14 the data has 3,099.81."), "{}", correction(&flags));
        let right = "**Month before last (Jul 15 to Aug 14):**\n- Spent: **$3,099.81**\n";
        assert!(check(right, &[SUMMARY], "compare last month with the month before", today()).is_empty());
    }

    #[test]
    fn the_answer_in_bold_is_for_the_days_the_question_asks_about() {
        let flags = check("Total without a category: **$155.44**", &[SUMMARY], "how much have I left without a category, across all my entries ever?", today());
        assert!(matches!(&flags[..], [Flag::Days { figure, by_question: true, right, .. }] if figure == "155.44" && right[0] == "214.19"), "{flags:?}");
    }

    #[test]
    fn a_total_that_its_list_does_not_add_up_to_is_flagged() {
        let reply = "- Vet: $412.35\n- Food: $60.86\n- Toys: $54.66\n\n**Total: $527.80**";
        let flags = check(reply, &[SUMMARY], "how much has Miso cost me?", today());
        assert_eq!(flags, [Flag::Total { figure: "527.80".into(), listed: Some((3, "527.87".into())) }]);
        assert!(check("- Vet: $412.35\n- Food: $60.86\n- Toys: $54.66\n\n**Total: $527.87**", &[SUMMARY], "how much has Miso cost me?", today()).is_empty());
        assert!(check("You spent **$406.95**: $113.46 at Jean-Talon, $91.88 at Provigo, and 3 others totaling $201.61.", &[SUMMARY], "groceries?", today()).is_empty(), "what is left of a total");
    }

    #[test]
    fn working_and_loose_words_are_not_flagged() {
        // Rounded working: Q2 less Q1.
        assert!(check("Q1: $12,310.73. Q2: $12,550.94. Q2 cost about $240 more.", &[SUMMARY], "Q2 or Q1?", today()).is_empty());
        // "This month" said of the month to date, "today" of what is left as of today.
        assert!(check("This month you have spent **$1,059.02** so far.", &[SUMMARY], "how much this month?", today()).is_empty());
        // The right week.
        assert!(check("Last week you spent $477.30.", &[SUMMARY], "last week?", today()).is_empty());
        let flags = check("Last week you spent $1,084.68.", &[SUMMARY], "last week?", today());
        assert!(matches!(&flags[..], [Flag::Days { figure, right, .. }] if figure == "1,084.68" && right == &["477.30"]), "{flags:?}");
    }

    /// The check over the bench's saved replies, beside the Python prototype's flags it was ported
    /// from: `FIGURES_CASES=<cases.jsonl> FIGURES_OUT=<out.jsonl> cargo test -p relay-core figures -- --ignored`.
    #[test]
    #[ignore]
    fn saved_replies() {
        use std::io::{BufRead, Write};
        let (Ok(cases), Ok(out)) = (std::env::var("FIGURES_CASES"), std::env::var("FIGURES_OUT")) else { return };
        let mut out = std::fs::File::create(out).unwrap();
        for line in std::io::BufReader::new(std::fs::File::open(cases).unwrap()).lines() {
            let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
            let read: Vec<&str> = case["read"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            let flags = check(case["reply"].as_str().unwrap(), &read, case["asked"].as_str().unwrap(), case["today"].as_str().unwrap().parse().unwrap());
            let row = json!({"id": case["id"], "run": case["run"], "group": case["group"], "py": case["py"], "rs": flags.iter().map(|f| f.figure().to_string()).collect::<Vec<_>>(),
                             "flags": flags.iter().map(Flag::to_json).collect::<Vec<_>>()});
            writeln!(out, "{row}").unwrap();
        }
    }
}
