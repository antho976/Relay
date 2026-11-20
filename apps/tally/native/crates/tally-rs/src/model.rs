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
    /// [`Store::holding_prefs`] is running: writes wait for one save at its end.
    held: u32,
    unsaved: bool,
}

/// The tables whose live rows are the owner's own data: every synced one but the categories, which
/// every new ledger is given (relay-money's `holds_anything`).
const OWN_TABLES: &[&str] =
    &["accounts", "recurring", "goals", "transactions", "budgets", "contributions", "account_values", "securities", "holdings", "activities", "prices", "fx_rates", "room_facts"];

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
        let store = Store { ledger, locale: Locale::new("en-CA"), db: Some(db), dir, prefs, held: 0, unsaved: false };
        // A ledger synced before this build kept the server's sync record, which stops bills
        // posting here (see `forget_sync_records`).
        store.forget_sync_records();
        Ok(store)
    }

    pub fn in_memory() -> Result<Store> {
        let mut ledger = Ledger::open_in_memory().map_err(err)?;
        ledger.ensure_categories().map_err(err)?;
        Ok(Store { ledger, locale: Locale::new("en-CA"), db: None, dir: None, prefs: Default::default(), held: 0, unsaved: false })
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
        if self.prefs.get(key).is_some_and(|v| v == value) {
            return Ok(());
        }
        self.prefs.insert(key.to_string(), value.to_string());
        self.save_prefs()
    }

    pub fn clear_pref(&mut self, key: &str) -> Result<()> {
        if self.prefs.remove(key).is_none() {
            return Ok(());
        }
        self.save_prefs()
    }

    /// Runs `f` with prefs.json written once at the end instead of at every change (a sync sets a
    /// dozen). A write that fails then is lost with the rest: the prefs are read back as they were.
    pub fn holding_prefs<T>(&mut self, f: impl FnOnce(&mut Store) -> T) -> T {
        self.held += 1;
        let out = f(self);
        self.held -= 1;
        if self.held == 0 && std::mem::take(&mut self.unsaved) {
            let _ = self.write_prefs();
        }
        out
    }

    fn save_prefs(&mut self) -> Result<()> {
        if self.held > 0 {
            self.unsaved = true;
            return Ok(());
        }
        self.write_prefs()
    }

    fn write_prefs(&self) -> Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        let text = serde_json::to_string_pretty(&self.prefs).map_err(err)?;
        let tmp = dir.join("prefs.json.tmp");
        std::fs::write(&tmp, text).map_err(err)?;
        std::fs::rename(&tmp, dir.join("prefs.json")).map_err(err)
    }

    pub fn is_empty(&self) -> bool {
        self.ledger.is_empty().unwrap_or(true)
    }

    /// Whether anything of the owner's is here: a live row in any table but the categories.
    pub fn holds_anything(&self) -> bool {
        match &self.db {
            Some(db) => OWN_TABLES.iter().any(|t| db.prepare(&format!("SELECT 1 FROM {t} WHERE deleted = 0 LIMIT 1")).and_then(|mut st| st.exists([])).unwrap_or(false)),
            // In memory there is no second connection: the backup holds every table but the
            // investment ones, which nothing in memory writes.
            None => self.ledger.export_backup("").is_ok_and(|f| {
                !(f.accounts.is_empty() && f.transactions.is_empty() && f.budgets.is_empty() && f.recurring.is_empty() && f.goals.is_empty() && f.contributions.is_empty())
            }),
        }
    }

    /// The sample loads only into a ledger with nothing the owner made, the seeded categories aside
    /// (Kotlin: `canLoadSample`, `isEmptyForSample`).
    pub fn can_load_sample(&self) -> bool {
        !self.holds_anything()
    }

    /// Tally's labelled sample household, in this ledger's currency, its first account the default
    /// for new entries (Kotlin: DataRepository `loadSample`). Only into an empty ledger.
    pub fn load_sample(&mut self, today: Date) -> Result<usize> {
        if !self.can_load_sample() {
            return Err("Sample data loads only into an empty app. Erase everything first to try it.".into());
        }
        let currency = self.ledger.settings().map(|s| s.currency).unwrap_or_else(|_| "CAD".into());
        let n = self.ledger.load_sample(today, &currency).map_err(err)?;
        let first = self.ledger.accounts().map_err(err)?.into_iter().next().map(|a| a.id);
        // The sample is in either way: a default that cannot be kept (a ledger held in memory has
        // no second connection) leaves the usual fallback, the first open account.
        let _ = crate::accounts_store::set_default_account(self, first);
        Ok(n)
    }

    /// Deletes the records relay-money keeps of the devices this ledger synced with
    /// (`sync.device.*` settings). Each local `sync_page` writes one for the server; relay-money
    /// reads any as "a phone posts the bills, not this ledger" and `post_due` then posts nothing.
    /// This phone is never the server, and nothing in Tally reads the record. Needs the second
    /// connection, so a ledger held in memory keeps it.
    pub fn forget_sync_records(&self) {
        if let Some(db) = &self.db {
            let _ = db.execute("DELETE FROM settings WHERE key LIKE 'sync.device.%'", []);
        }
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
                // Never past the last period's own end: on Mar 29 to 31, February is all of it.
                let to = start.checked_add((s.pace.elapsed_days - 1).max(0).days()).unwrap_or(start);
                let to = match last.period.end_exclusive.parse::<Date>().ok().and_then(|e| e.yesterday().ok()) {
                    Some(end) if end >= start => to.min(end),
                    _ => to,
                };
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

#[cfg(test)]
// Amounts written as dollars and cents: 100_00 is $100.00.
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use relay_money::model::AccountType;

    fn income(s: &mut Store, account: i64, amount: i64, on: Date) {
        s.add(&TxInput { r#type: TxType::Income, amount, date: on.to_string(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
    }

    #[test]
    fn last_month_by_this_day_stops_at_last_months_end() {
        let mut s = Store::in_memory().unwrap();
        let a = s.ledger.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap().id;
        income(&mut s, a, 50_00, Date::constant(2026, 2, 10));
        income(&mut s, a, 20_00, Date::constant(2026, 2, 28));
        // This month's: never counted as February's, though Mar 30 is February's day 30.
        income(&mut s, a, 100_00, Date::constant(2026, 3, 1));
        income(&mut s, a, 7_00, Date::constant(2026, 3, 2));
        let home = s.home(Date::constant(2026, 3, 30)).unwrap();
        assert_eq!(home.income_last_same_day, 70_00);
        // Earlier in the month it counts February's first days only.
        let home = s.home(Date::constant(2026, 3, 10)).unwrap();
        assert_eq!(home.income_last_same_day, 50_00);
    }

    #[test]
    fn prefs_held_are_written_once_at_the_end() {
        let dir = std::env::temp_dir().join(format!("tally-model-prefs-{}", uuid::Uuid::new_v4()));
        let mut s = Store::open(&dir.join("tally.db")).unwrap();
        s.holding_prefs(|s| {
            s.set_pref("a", "1").unwrap();
            s.set_pref("b", "2").unwrap();
            assert!(!dir.join("prefs.json").exists(), "nothing written while held");
        });
        let text = std::fs::read_to_string(dir.join("prefs.json")).unwrap();
        assert!(text.contains("\"a\"") && text.contains("\"b\""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_sample_loads_only_into_an_empty_ledger_in_its_currency() {
        let dir = std::env::temp_dir().join(format!("tally-model-sample-{}", uuid::Uuid::new_v4()));
        let mut s = Store::open(&dir.join("tally.db")).unwrap();
        s.ledger.set_currency("EUR").unwrap();
        // A goal alone is the owner's: no sample over it.
        let (uid, now) = Store::stamp();
        s.sql()
            .unwrap()
            .execute(
                "INSERT INTO goals (uid, name, target, color, archived, kind, updated_at) VALUES (?1, 'Trip', 100000, 1, 0, 'SAVINGS', ?2)",
                rusqlite::params![uid, now],
            )
            .unwrap();
        assert!(!s.can_load_sample());
        assert!(s.load_sample(today()).is_err());
        s.sql().unwrap().execute("UPDATE goals SET deleted = 1", []).unwrap();
        assert!(s.can_load_sample());
        s.load_sample(today()).unwrap();
        assert_eq!(s.ledger.settings().unwrap().currency, "EUR");
        let first = s.ledger.accounts().unwrap()[0].id;
        assert_eq!(crate::accounts_store::default_account(&s), Some(first));
        let _ = std::fs::remove_dir_all(dir);
    }
}
