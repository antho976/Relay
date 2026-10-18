//! A synthetic household for testing the app with something in it. Port of `SampleData.kt`.
//!
//! Three months of plausible spending, a salary, rent and a few subscriptions. Deterministic for a
//! given `today` and seed, so screenshots and tests see the same month every time. Every account
//! it creates is named "Sample ..." so the data can never pass for the owner's own.
//!
//! The draws come from [`KotlinRandom`], a port of `kotlin.random.Random(seed)`, and amounts round
//! as `Math.round` does, so the PC builds the very file the phone does for the same day and seed.

use crate::backup::{AccountDto, AccountValueDto, BackupFile, BudgetDto, CategoryDto, ContributionDto, GoalDto, RecurringDto, TransactionDto};
use crate::model::{AccountType, GoalKind, TxType, DEFAULT_CATEGORIES};
use crate::money::pow10;
use crate::period::plus_months;
use crate::recurrence::{Frequency, Recurrence};
use jiff::civil::Date;
use jiff::ToSpan;

pub const ACCOUNT_PREFIX: &str = "Sample";

/// The seed the Kotlin's `build` defaults to.
pub const DEFAULT_SEED: i32 = 7;

/// `kotlin.random.Random(seed)`: Marsaglia's xorwow, as `XorWowRandom`, with the base class's
/// bounded draws. The same seed gives the same sequence as on the phone.
#[derive(Debug, Clone)]
pub struct KotlinRandom {
    x: i32,
    y: i32,
    z: i32,
    w: i32,
    v: i32,
    addend: i32,
}

impl KotlinRandom {
    pub fn new(seed: i32) -> Self {
        let (seed1, seed2) = (seed, seed >> 31);
        let mut r = KotlinRandom { x: seed1, y: seed2, z: 0, w: 0, v: !seed1, addend: (seed1 << 10) ^ ((seed2 as u32) >> 4) as i32 };
        // Some trivial seeds give several values with zeroes in the upper bits, so the first 64 go.
        for _ in 0..64 {
            r.next_int();
        }
        r
    }

    pub fn next_int(&mut self) -> i32 {
        let mut t = self.x;
        t ^= ((t as u32) >> 2) as i32;
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        let v0 = self.v;
        self.w = v0;
        t = (t ^ (t << 1)) ^ v0 ^ (v0 << 4);
        self.v = t;
        self.addend = self.addend.wrapping_add(362_437);
        t.wrapping_add(self.addend)
    }

    fn next_bits(&mut self, bit_count: u32) -> i32 {
        if bit_count == 0 { 0 } else { ((self.next_int() as u32) >> (32 - bit_count)) as i32 }
    }

    /// A value in `from..until`.
    ///
    /// # Panics
    /// When the range is empty, as the Kotlin throws.
    pub fn next_int_in(&mut self, from: i32, until: i32) -> i32 {
        assert!(until > from, "Random range is empty: [{from}, {until}).");
        let n = until.wrapping_sub(from);
        if n > 0 || n == i32::MIN {
            let rnd = if n.isolate_lowest_one() == n { // Kotlin's `n and -n == n`
                self.next_bits(31 - n.leading_zeros())
            } else {
                loop {
                    let bits = ((self.next_int() as u32) >> 1) as i32;
                    let v = bits % n;
                    if bits.wrapping_sub(v).wrapping_add(n - 1) >= 0 {
                        break v;
                    }
                }
            };
            from.wrapping_add(rnd)
        } else {
            loop {
                let rnd = self.next_int();
                if (from..until).contains(&rnd) {
                    return rnd;
                }
            }
        }
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 / (1 << 24) as f32
    }

    /// `Collection.random(random)`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.next_int_in(0, items.len() as i32) as usize]
    }
}

/// `Math.round`: the closest whole number, ties toward positive infinity.
fn java_round(x: f64) -> i64 {
    let floor = x.floor();
    (if x - floor >= 0.5 { floor + 1.0 } else { floor }) as i64
}

fn plus_days(d: Date, n: i64) -> Date {
    d.checked_add(n.days()).expect("date in range")
}

fn first_of_month(d: Date) -> Date {
    d.first_of_month()
}

pub fn build(today: Date, fraction_digits: u32, currency: &str, seed: i32) -> BackupFile {
    let mut rnd = KotlinRandom::new(seed);
    let unit = pow10(fraction_digits) as f64;
    let money = |major: f64| java_round(major * unit);

    let account = |id: i64, name: &str, r#type: AccountType, opening_balance: i64, sort_order: i32| AccountDto {
        sort_order,
        ..AccountDto::new(id, format!("{ACCOUNT_PREFIX} {name}"), r#type, opening_balance)
    };
    let accounts = vec![
        account(1, "chequing", AccountType::Chequing, money(2_400.0), 0),
        account(2, "Visa", AccountType::Credit, 0, 1),
        account(3, "savings", AccountType::Savings, money(6_500.0), 2),
        account(4, "cash", AccountType::Cash, money(120.0), 3),
        account(5, "TFSA", AccountType::Investment, money(8_200.0), 4),
    ];
    let categories: Vec<CategoryDto> = DEFAULT_CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, c)| CategoryDto {
            id: i as i64 + 1,
            name: c.name.to_string(),
            kind: c.kind,
            color: c.color,
            icon: c.icon.to_string(),
            archived: false,
            sort_order: i as i32,
        })
        .collect();
    let cat = |name: &str| categories.iter().find(|c| c.name == name).expect("a default category").id;

    let start = plus_months(first_of_month(today), -2);
    let mut tx: Vec<TransactionDto> = Vec::new();
    let mut id = 1i64;
    #[allow(clippy::too_many_arguments)]
    fn add(tx: &mut Vec<TransactionDto>, id: &mut i64, today: Date, r#type: TxType, amount: i64, date: Date, account: i64, category: Option<i64>, note: &str, to: Option<i64>, recurring: Option<i64>) {
        if date > today {
            return;
        }
        tx.push(TransactionDto {
            id: *id,
            r#type,
            amount,
            date: date.to_string(),
            account_id: account,
            to_account_id: to,
            category_id: category,
            note: note.to_string(),
            recurring_id: recurring,
        });
        *id += 1;
    }

    let groceries = ["Metro", "IGA", "Provigo", "Marché Jean-Talon", "Costco"];
    let dining = ["Café Olimpico", "Pho Lien", "Lunch with Sam", "Burger night", "Bagels", "Thai takeout"];
    let transport = ["OPUS refill", "Bixi", "Taxi home", "Gas"];
    let shopping = ["Winter boots", "Hardware store", "Books", "Kitchen stuff", "Phone case"];

    // Every draw is made in the Kotlin's argument order, so the sequence stays the phone's.
    let mut d = start;
    while d <= today {
        // Groceries twice a week, dining most days, the rest scattered.
        let weekday = d.weekday().to_monday_one_offset();
        if weekday == 2 || weekday == 6 {
            let amount = money(45.0 + f64::from(rnd.next_int_in(0, 95)) + f64::from(rnd.next_int_in(0, 99)) / 100.0);
            let note = *rnd.pick(&groceries);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, 2, Some(cat("Groceries")), note, None, None);
        }
        if rnd.next_float() < 0.55 {
            let amount = money(8.0 + f64::from(rnd.next_int_in(0, 42)) + f64::from(rnd.next_int_in(0, 99)) / 100.0);
            let account = if rnd.next_boolean() { 2 } else { 4 };
            let note = *rnd.pick(&dining);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, account, Some(cat("Dining")), note, None, None);
        }
        if rnd.next_float() < 0.25 {
            let amount = money(3.5 + f64::from(rnd.next_int_in(0, 60)));
            let note = *rnd.pick(&transport);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, 2, Some(cat("Transport")), note, None, None);
        }
        if rnd.next_float() < 0.10 {
            let amount = money(20.0 + f64::from(rnd.next_int_in(0, 140)) + 0.99);
            let note = *rnd.pick(&shopping);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, 2, Some(cat("Shopping")), note, None, None);
        }
        if rnd.next_float() < 0.06 {
            let amount = money(15.0 + f64::from(rnd.next_int_in(0, 60)));
            let note = *rnd.pick(&["Cinema", "Concert", "Board game cafe"]);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, 2, Some(cat("Entertainment")), note, None, None);
        }
        if rnd.next_float() < 0.04 {
            let amount = money(12.0 + f64::from(rnd.next_int_in(0, 80)));
            let note = *rnd.pick(&["Pharmacy", "Physio"]);
            add(&mut tx, &mut id, today, TxType::Expense, amount, d, 2, Some(cat("Health")), note, None, None);
        }
        d = plus_days(d, 1);
    }

    let mut recurring: Vec<RecurringDto> = Vec::new();
    let mut bill = |rid: i64, name: &str, r#type: TxType, amount: i64, account: i64, category: Option<i64>, freq: Frequency, interval: i32, anchor: Date, to: Option<i64>| {
        let rule = Recurrence::new(anchor, freq, interval);
        for date in rule.between(anchor, today) {
            add(&mut tx, &mut id, today, r#type, amount, date, account, category, name, to, Some(rid));
        }
        recurring.push(RecurringDto {
            id: rid,
            name: name.to_string(),
            r#type,
            amount,
            account_id: account,
            to_account_id: to,
            category_id: category,
            frequency: freq,
            interval,
            anchor_date: anchor.to_string(),
            next_date: rule.after(today).to_string(),
            end_date: None,
            auto_post: true,
            active: true,
        });
    };
    use Frequency::{Monthly, Weekly};
    use TxType::{Expense, Income, Transfer};
    bill(1, "Rent", Expense, money(1_350.0), 1, Some(cat("Housing")), Monthly, 1, start, None);
    bill(2, "Hydro-Québec", Expense, money(78.40), 1, Some(cat("Utilities")), Monthly, 1, plus_days(start, 17), None);
    bill(3, "Phone plan", Expense, money(45.0), 2, Some(cat("Phone & internet")), Monthly, 1, plus_days(start, 9), None);
    bill(4, "Internet", Expense, money(60.0), 2, Some(cat("Phone & internet")), Monthly, 1, plus_days(start, 21), None);
    bill(5, "Music streaming", Expense, money(11.99), 2, Some(cat("Subscriptions")), Monthly, 1, plus_days(start, 4), None);
    bill(6, "Gym", Expense, money(39.0), 2, Some(cat("Health")), Monthly, 1, plus_days(start, 13), None);
    bill(7, "Salary", Income, money(2_150.0), 1, Some(cat("Salary")), Weekly, 2, plus_days(start, 4), None);
    bill(8, "Card payment", Transfer, money(900.0), 1, None, Monthly, 1, plus_days(start, 24), Some(2));
    bill(9, "To savings", Transfer, money(250.0), 1, None, Monthly, 1, plus_days(start, 5), Some(3));
    bill(10, "To TFSA", Transfer, money(300.0), 1, None, Monthly, 1, plus_days(start, 6), Some(5));

    let budgets = vec![
        BudgetDto { id: 1, category_id: None, amount: money(2_900.0) },
        BudgetDto { id: 2, category_id: Some(cat("Groceries")), amount: money(500.0) },
        BudgetDto { id: 3, category_id: Some(cat("Dining")), amount: money(320.0) },
        BudgetDto { id: 4, category_id: Some(cat("Transport")), amount: money(120.0) },
        BudgetDto { id: 5, category_id: Some(cat("Shopping")), amount: money(150.0) },
        BudgetDto { id: 6, category_id: Some(cat("Entertainment")), amount: money(80.0) },
    ];
    let month_from_now = |n: i64| Some(first_of_month(plus_months(today, n)).to_string());
    let goals = vec![
        GoalDto { target_date: month_from_now(14), color: 1, ..GoalDto::new(1, "Emergency fund", money(10_000.0)) },
        GoalDto { target_date: month_from_now(6), color: 6, ..GoalDto::new(2, "Lisbon trip", money(2_400.0)) },
        GoalDto { color: 2, kind: GoalKind::Invest, percent: 10, ..GoalDto::new(3, "Invest a tenth of pay", 0) },
        GoalDto {
            target_date: month_from_now(12),
            color: 7,
            kind: GoalKind::Balance,
            start_date: Some(start.to_string()),
            start_amount: money(17_220.0),
            ..GoalDto::new(4, "Worth 25k", money(25_000.0))
        },
    ];
    let not_after_today = |date: Date| (date <= today).then(|| date.to_string());
    // The TFSA's value as read off the statement once, a little above what was put in.
    let values: Vec<AccountValueDto> = not_after_today(plus_days(plus_months(start, 1), 27))
        .map(|date| AccountValueDto { id: 1, account_id: 5, date, value: money(9_050.0) })
        .into_iter()
        .collect();
    let contribution = |id: i64, goal_id: i64, amount: i64, date: Date, note: &str| {
        not_after_today(date).map(|date| ContributionDto { id, goal_id, amount, date, note: note.to_string() })
    };
    let contributions: Vec<ContributionDto> = [
        contribution(1, 1, money(3_200.0), start, "Starting balance"),
        contribution(2, 1, money(250.0), plus_days(plus_months(start, 1), 5), ""),
        contribution(3, 2, money(400.0), plus_days(start, 12), ""),
        contribution(4, 2, money(300.0), plus_days(plus_months(start, 1), 12), ""),
    ]
    .into_iter()
    .flatten()
    .collect();

    tx.sort_by(|a, b| a.date.cmp(&b.date));
    BackupFile {
        accounts,
        categories,
        transactions: tx,
        budgets,
        recurring,
        goals,
        contributions,
        values,
        ..BackupFile::new(today.to_string(), currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{validate, BackupReadResult};
    use crate::csv::parse_iso_date;
    use jiff::civil::date;

    const TODAY: Date = date(2026, 10, 4);

    fn file() -> BackupFile {
        build(TODAY, 2, "CAD", DEFAULT_SEED)
    }

    fn day(s: &str) -> Date {
        parse_iso_date(s).expect("an ISO date")
    }

    #[test]
    fn nothing_is_dated_in_the_future() {
        assert!(file().transactions.iter().all(|t| day(&t.date) <= TODAY));
    }

    #[test]
    fn every_account_says_it_is_sample_data() {
        assert!(file().accounts.iter().all(|a| a.name.starts_with(ACCOUNT_PREFIX)));
    }

    #[test]
    fn is_deterministic() {
        assert_eq!(file(), build(TODAY, 2, "CAD", DEFAULT_SEED));
    }

    #[test]
    fn covers_three_months_with_ids_unique() {
        let f = file();
        let first = f.transactions.iter().map(|t| day(&t.date)).min().expect("transactions");
        assert!(first <= plus_months(first_of_month(TODAY), -2));
        let ids: std::collections::HashSet<i64> = f.transactions.iter().map(|t| t.id).collect();
        assert_eq!(f.transactions.len(), ids.len());
    }

    #[test]
    fn passes_backup_validation() {
        assert!(matches!(validate(file()), BackupReadResult::Ok(_)));
    }

    #[test]
    fn recurring_next_dates_are_after_today() {
        assert!(file().recurring.iter().all(|r| day(&r.next_date) > TODAY));
    }

    #[test]
    fn draws_match_kotlin_random() {
        // From kotlin-stdlib 2.2.10: `Random(7)` then nextInt(), nextInt(0, 95), nextInt(0, 99),
        // nextFloat(), nextBoolean(), nextInt(5), nextInt(0, 64), nextInt(0, 140); and `Random(-12345)`.
        let mut r = KotlinRandom::new(7);
        assert_eq!(-182_312_124, r.next_int());
        assert_eq!(74, r.next_int_in(0, 95));
        assert_eq!(60, r.next_int_in(0, 99));
        assert_eq!(0.372_558_95, r.next_float());
        assert!(!r.next_boolean());
        assert_eq!(0, r.next_int_in(0, 5));
        assert_eq!(56, r.next_int_in(0, 64));
        assert_eq!(114, r.next_int_in(0, 140));
        let mut n = KotlinRandom::new(-12_345);
        assert_eq!(247_018_746, n.next_int());
        assert_eq!(1, n.next_int_in(0, 42));
        assert_eq!(0.559_103_1, n.next_float());
    }

    #[test]
    fn builds_the_phones_sample() {
        // Read off `BackupCodec.encode(SampleData.build(LocalDate.of(2026, 10, 4), 2, "CAD"))` from
        // the Kotlin, whose whole file this one matched byte for byte when it was ported.
        let f = file();
        assert_eq!(109, f.transactions.len());
        assert_eq!(2_249_946, f.transactions.iter().map(|t| t.amount).sum::<i64>());
        let firsts: Vec<(i64, i64, &str)> = f.transactions[..3].iter().map(|t| (t.id, t.amount, t.note.as_str())).collect();
        assert_eq!(vec![(1, 9_695, "Marché Jean-Talon"), (2, 3_749, "Lunch with Sam"), (3, 650, "Bixi")], firsts);
        let last = f.transactions.last().expect("transactions");
        assert_eq!((85, 1_792, "Pho Lien", "2026-10-04"), (last.id, last.amount, last.note.as_str(), last.date.as_str()));
    }

    #[test]
    fn rounds_as_math_round() {
        assert_eq!(1199, java_round(11.99 * 100.0));
        assert_eq!(3, java_round(2.5));
        assert_eq!(-2, java_round(-2.5));
        assert_eq!(0, java_round(0.499_999_999_999_999_94));
    }
}
