//! What the screens read, from relay-money's ledger: the Home summary plus the few readings
//! Overview adds (today, the week, where the money went, money in last month by this day), and the
//! wording rules the Kotlin app's HomeLogic.kt keeps.

use std::path::Path;

use jiff::ToSpan;
use relay_money::Date;
use relay_money::copy;
use relay_money::ledger::Ledger;
use relay_money::model::TxType;
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::pace::{PaceReading, PaceStatus};
use relay_money::views::{CategoryView, Summary, Tx, TxInput, TxQuery};

pub type Result<T> = std::result::Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Today where the phone is.
pub fn today() -> Date {
    jiff::Zoned::now().date()
}

/// A category's share of the period's spending.
#[derive(Clone, Debug)]
pub struct Share {
    pub category_id: Option<i64>,
    pub name: String,
    pub icon: String,
    pub color: Option<i64>,
    pub total: i64,
    pub count: usize,
}

/// Everything Overview draws, read in one go.
pub struct Home {
    pub today: Date,
    pub s: Summary,
    pub fmt: MoneyFormatter,
    pub spent_today: i64,
    /// Seven days' spending from `week_start`, and which of them is today.
    pub week: [i64; 7],
    pub week_start: Date,
    pub today_in_week: usize,
    pub income_last_same_day: i64,
    pub shares: Vec<Share>,
}

impl Home {
    pub fn reading(&self) -> Option<&PaceReading> {
        (self.s.pace.budget > 0).then_some(&self.s.pace)
    }
    pub fn week_spent(&self) -> i64 {
        self.week.iter().sum()
    }
    pub fn has_entries(&self) -> bool {
        !self.s.empty
    }
}

/// The ledger, as the app holds it, and this phone's own preferences (never synced).
pub struct Store {
    pub ledger: Ledger,
    pub locale: Locale,
    /// A second connection to the same file, for the rows relay-money has no calls for yet (bills,
    /// goals, categories). Writes through it set `uid`, `updated_at` and `deleted` like
    /// relay-money's own; the tables' triggers number them for sync. `None` for a ledger in memory.
    pub db: Option<rusqlite::Connection>,
    dir: Option<std::path::PathBuf>,
    prefs: std::collections::BTreeMap<String, String>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let dir = path.parent().map(Path::to_path_buf);
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let mut ledger = Ledger::open(path).map_err(err)?;
        ledger.ensure_categories().map_err(err)?;
        let db = rusqlite::Connection::open(path).map_err(err)?;
        db.busy_timeout(std::time::Duration::from_secs(2)).map_err(err)?;
        let prefs = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("prefs.json")).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Ok(Store { ledger, locale: Locale::new("en-CA"), db: Some(db), dir, prefs })
    }

    pub fn in_memory() -> Result<Store> {
        let mut ledger = Ledger::open_in_memory().map_err(err)?;
        ledger.ensure_categories().map_err(err)?;
        Ok(Store { ledger, locale: Locale::new("en-CA"), db: None, dir: None, prefs: Default::default() })
    }

    /// The second connection, or why there is none.
    pub fn sql(&self) -> Result<&rusqlite::Connection> {
        self.db.as_ref().ok_or_else(|| "This ledger is held in memory and cannot take that change".to_string())
    }

    /// A new row's identity and stamp, as relay-money writes them: a random uid and now in
    /// milliseconds.
    pub fn stamp() -> (String, i64) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        (uuid::Uuid::new_v4().to_string(), now)
    }

    /// One of this phone's preferences (the palette, the server pairing), kept in prefs.json beside
    /// the ledger and never synced.
    pub fn pref(&self, key: &str) -> Option<String> {
        self.prefs.get(key).cloned()
    }

    pub fn set_pref(&mut self, key: &str, value: &str) -> Result<()> {
        self.prefs.insert(key.to_string(), value.to_string());
        self.save_prefs()
    }

    pub fn clear_pref(&mut self, key: &str) -> Result<()> {
        self.prefs.remove(key);
        self.save_prefs()
    }

    fn save_prefs(&self) -> Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        let text = serde_json::to_string_pretty(&self.prefs).map_err(err)?;
        let tmp = dir.join("prefs.json.tmp");
        std::fs::write(&tmp, text).map_err(err)?;
        std::fs::rename(&tmp, dir.join("prefs.json")).map_err(err)
    }

    pub fn is_empty(&self) -> bool {
        self.ledger.is_empty().unwrap_or(true)
    }

    pub fn load_sample(&mut self, today: Date) -> Result<usize> {
        self.ledger.load_sample(today, "CAD").map_err(err)
    }

    pub fn formatter(&self) -> MoneyFormatter {
        let currency = self.ledger.settings().map(|s| s.currency).unwrap_or_else(|_| "CAD".into());
        MoneyFormatter::new(&currency, self.locale.clone())
    }

    pub fn home(&self, today: Date) -> Result<Home> {
        let s = self.ledger.summary(today, &self.locale).map_err(err)?;
        let fmt = self.formatter();
        let settings = self.ledger.settings().map_err(err)?;

        let range = |from: Date, to: Date| TxQuery { from: Some(from), to: Some(to), limit: Some(2000), ..Default::default() };
        let spent_today = self.ledger.tx_list(&range(today, today), today).map_err(err)?.spent;

        // The week holding today, from its first day as the person counts weeks.
        let back = if settings.week_starts_monday {
            today.weekday().to_monday_zero_offset()
        } else {
            today.weekday().to_sunday_zero_offset()
        } as i64;
        let week_start = today.checked_sub(back.days()).map_err(err)?;
        let week_end = week_start.checked_add(6.days()).map_err(err)?;
        let mut week = [0i64; 7];
        for t in self.ledger.tx_list(&range(week_start, week_end), today).map_err(err)?.transactions {
            if t.r#type != TxType::Expense {
                continue;
            }
            if let Ok(d) = t.date.parse::<Date>() {
                let i = (d - week_start).get_days();
                if (0..7).contains(&i) {
                    week[i as usize] += t.amount;
                }
            }
        }

        // Money in last period by this day: its first `elapsed` days.
        let last = self.ledger.tx_list(&TxQuery { period_offset: Some(-1), limit: Some(1), ..Default::default() }, today).map_err(err)?;
        let income_last_same_day = match last.period.start.parse::<Date>() {
            Ok(start) => {
                let to = start.checked_add((s.pace.elapsed_days - 1).max(0).days()).unwrap_or(start);
                self.ledger.tx_list(&range(start, to), today).map_err(err)?.income
            }
            Err(_) => 0,
        };

        // Where it went: the period's spending by category, largest first.
        let period = self.ledger.tx_list(&TxQuery { limit: Some(5000), ..Default::default() }, today).map_err(err)?;
        let categories = self.ledger.categories().map_err(err)?;
        let shares = shares(&period.transactions, &categories);

        Ok(Home {
            today,
            s,
            fmt,
            spent_today,
            week,
            week_start,
            today_in_week: back as usize,
            income_last_same_day,
            shares,
        })
    }

    pub fn add(&mut self, input: &TxInput) -> Result<Tx> {
        self.ledger.tx_add(input).map_err(err)
    }

    pub fn delete(&mut self, id: i64) -> Result<()> {
        self.ledger.tx_delete(id).map_err(err)
    }

    pub fn restore(&mut self, id: i64) -> Result<()> {
        self.ledger.tx_restore(id).map(|_| ()).map_err(err)
    }
}

/// The period's spending per category, largest first, the smallest folded into "Other" past five.
pub fn shares(txs: &[Tx], categories: &[CategoryView]) -> Vec<Share> {
    let mut by: Vec<Share> = Vec::new();
    for t in txs.iter().filter(|t| t.r#type == TxType::Expense) {
        match by.iter_mut().find(|s| s.category_id == t.category_id) {
            Some(s) => {
                s.total += t.amount;
                s.count += 1;
            }
            None => {
                let c = t.category_id.and_then(|id| categories.iter().find(|c| c.id == id));
                by.push(Share {
                    category_id: t.category_id,
                    name: c.map(|c| c.name.clone()).unwrap_or_else(|| "Uncategorised".into()),
                    icon: c.map(|c| c.icon.clone()).unwrap_or_else(|| "dots".into()),
                    color: c.map(|c| c.color),
                    total: t.amount,
                    count: 1,
                });
            }
        }
    }
    by.sort_by(|a, b| b.total.cmp(&a.total));
    if by.len() > 5 {
        let rest: Vec<Share> = by.split_off(4);
        by.push(Share {
            category_id: None,
            name: format!("{} more", rest.len()),
            icon: "dots".into(),
            color: Some(10),
            total: rest.iter().map(|s| s.total).sum(),
            count: rest.iter().map(|s| s.count).sum(),
        });
    }
    by
}

/// The limit is reached once nothing is left of it.
pub fn limit_reached(r: &PaceReading) -> bool {
    r.budget > 0 && r.remaining <= 0
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineTone {
    Quiet,
    Warn,
    Held,
}

/// The hero's line under the gauge (HomeLogic.kt `heroLine`).
pub fn hero_line(r: &PaceReading, fmt: &MoneyFormatter) -> (String, LineTone) {
    if !limit_reached(r) {
        let tone = if r.status == PaceStatus::OverPace { LineTone::Warn } else { LineTone::Quiet };
        (copy::days_line(r), tone)
    } else if r.remaining == 0 {
        (format!("All of your {} is spent", fmt.format_whole(r.budget)), LineTone::Held)
    } else {
        (format!("{} past your {} limit", fmt.format_whole(-r.remaining), fmt.format_whole(r.budget)), LineTone::Held)
    }
}

/// Today against its share (HomeLogic.kt `todayLine`).
pub fn today_line(spent_today: i64, r: Option<&PaceReading>, fmt: &MoneyFormatter) -> (String, LineTone) {
    let plain = || {
        let words = if spent_today == 0 { "Nothing spent yet today" } else { "Spent so far today" };
        (words.to_string(), LineTone::Quiet)
    };
    let Some(r) = r.filter(|r| r.budget > 0 && !limit_reached(r)) else { return plain() };
    let room = r.budget - (r.spent - spent_today);
    let share = if r.days_left > 0 { room / r.days_left } else { room };
    if spent_today == 0 {
        (format!("{} for today", fmt.format_whole(share)), LineTone::Quiet)
    } else if spent_today <= share {
        (format!("{} left for today", fmt.format_whole(share - spent_today)), LineTone::Quiet)
    } else {
        (format!("{} past today's share", fmt.format_whole(spent_today - share)), LineTone::Warn)
    }
}

/// Money in against last month by the same day (OverviewScreen.kt `versus`).
pub fn versus_in(now: i64, last: i64, fmt: &MoneyFormatter) -> String {
    if last <= 0 {
        return if now == 0 { "Nothing in yet".into() } else { "Nothing by this day last month".into() };
    }
    let diff = now - last;
    match diff.cmp(&0) {
        std::cmp::Ordering::Equal => "Level with last month".into(),
        std::cmp::Ordering::Greater => format!("{} more than last month", fmt.format_whole(diff)),
        std::cmp::Ordering::Less => format!("{} less than last month", fmt.format_whole(-diff)),
    }
}

/// "November", or "Oct 15 – Nov 14" for a period that does not start on the 1st.
pub fn period_name(start: &str, end_exclusive: &str) -> String {
    let (Ok(a), Ok(b)) = (start.parse::<Date>(), end_exclusive.parse::<Date>()) else { return String::new() };
    if a.day() == 1 {
        return a.strftime("%B").to_string();
    }
    let last = b.yesterday().unwrap_or(b);
    format!("{} – {}", a.strftime("%b %-d"), last.strftime("%b %-d"))
}

/// "1 Dec": a short date.
pub fn short_date(d: &str) -> String {
    d.parse::<Date>().map(|d| d.strftime("%-d %b").to_string()).unwrap_or_default()
}

/// A day relative to today: "Today", "Yesterday", or "Mon 17 Nov".
pub fn day_word(d: &str, today: Date) -> String {
    let Ok(d) = d.parse::<Date>() else { return String::new() };
    match (today - d).get_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => d.strftime("%a %-d %b").to_string(),
    }
}

pub fn weekday_initial(d: Date) -> &'static str {
    ["M", "T", "W", "T", "F", "S", "S"][d.weekday().to_monday_zero_offset() as usize]
}
