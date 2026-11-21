//! Entries, Search, a category's page, Insights and the entry editor: their readings and rules,
//! over the sample ledger (and a few entries made by hand).

use relay_money::Date;
use relay_money::model::TxType;
use relay_money::money::{Locale, MoneyFormatter};
use relay_money::period::BudgetPeriod;
use relay_money::views::{Tx, TxInput};
use tally::activity_data::{self as data, Filter};
use tally::entry::Entry;
use tally::entry_logic::{self as logic, Draft};
use tally::insights_data as ins;
use tally::model::{LineTone, Store};

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn tx(id: i64, ty: TxType, amount: i64, date: &str, category: Option<i64>, note: &str) -> Tx {
    Tx {
        id,
        uid: format!("u{id}"),
        r#type: ty,
        amount,
        date: date.into(),
        account_id: 1,
        account: "Chequing".into(),
        to_account_id: (ty == TxType::Transfer).then_some(2),
        to_account: (ty == TxType::Transfer).then(|| "Visa".into()),
        category_id: category,
        category: category.map(|c| format!("C{c}")),
        icon: None,
        color: None,
        note: note.into(),
    }
}

/// A ledger on disk (the second connection the bill and balance reads use needs a file).
fn sample() -> (Store, Date, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("tally-activity-test-{}-{}", std::process::id(), uuid_like()));
    let mut store = Store::open(&dir.join("tally.db")).unwrap();
    let today = tally::model::today();
    store.load_sample(today).unwrap();
    (store, today, dir)
}

fn uuid_like() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::SeqCst)
}

#[test]
fn days_group_newest_first_with_their_own_in_and_out() {
    let rows = vec![
        tx(3, TxType::Expense, 6_842, "2026-10-14", Some(1), "Metro"),
        tx(2, TxType::Income, 215_000, "2026-10-12", Some(9), "Salary"),
        tx(4, TxType::Expense, 465, "2026-10-14", Some(2), ""),
        tx(1, TxType::Transfer, 90_000, "2026-10-11", None, "Card payment"),
    ];
    let days = data::group_by_day(&rows);
    assert_eq!(days.iter().map(|g| g.date).collect::<Vec<_>>(), [d("2026-10-14"), d("2026-10-12"), d("2026-10-11")]);
    assert_eq!((days[0].spent, days[0].income, days[0].rows.len()), (7_307, 0, 2));
    assert_eq!((days[1].spent, days[1].income), (0, 215_000));
    assert_eq!((days[2].spent, days[2].income), (0, 0), "a transfer is neither in nor out");
    let t = data::totals(&rows);
    assert_eq!((t.count, t.spent, t.income, t.active_days), (4, 7_307, 215_000, 3));
    assert_eq!(data::context_line(&t, &fmt()), "4 entries · $73 out · $2,150 in");
}

#[test]
fn a_row_is_named_by_its_note_then_its_category() {
    let fmt_row = tx(1, TxType::Expense, 100, "2026-10-14", Some(1), "Metro");
    assert_eq!(data::entry_name(&fmt_row), "Metro");
    assert_eq!(data::entry_line(&fmt_row, None), "C1 · Chequing");
    let bare = tx(2, TxType::Expense, 100, "2026-10-14", Some(1), "  ");
    assert_eq!(data::entry_name(&bare), "C1");
    assert_eq!(data::entry_line(&bare, Some("Today")), "Today · Chequing");
    let none = tx(3, TxType::Expense, 100, "2026-10-14", None, "");
    assert_eq!(data::entry_name(&none), "Uncategorized");
    let moved = tx(4, TxType::Transfer, 100, "2026-10-14", None, "");
    assert_eq!(data::entry_name(&moved), "Transfer");
    assert_eq!(data::entry_line(&moved, None), "Chequing to Visa");
}

#[test]
fn filters_narrow_by_type() {
    assert!(Filter::All.admits(TxType::Transfer));
    assert!(Filter::Spent.admits(TxType::Expense) && !Filter::Spent.admits(TxType::Income));
    assert!(Filter::Transfers.admits(TxType::Transfer) && !Filter::Transfers.admits(TxType::Expense));
    assert_eq!(Filter::ALL[Filter::Income.index()], Filter::Income);
}

#[test]
fn a_day_reads_over_the_days_so_far_or_every_day_of_a_past_month() {
    let p = BudgetPeriod::containing(d("2026-10-10"), 1);
    assert_eq!(data::per_day(10_000, &p, true, d("2026-10-10")), Some(1_000));
    assert_eq!(data::per_day(31_000, &p, false, d("2026-11-19")), Some(1_000));
}

#[test]
fn account_flows_count_transfers_on_both_sides() {
    let mut to_one = tx(3, TxType::Transfer, 500, "2026-10-01", None, "");
    to_one.account_id = 2;
    to_one.to_account_id = Some(1);
    let rows = vec![
        tx(1, TxType::Expense, 1_000, "2026-10-01", Some(1), ""),
        tx(2, TxType::Income, 9_000, "2026-10-01", Some(9), ""),
        tx(4, TxType::Transfer, 2_000, "2026-10-01", None, ""),
        to_one,
    ];
    assert_eq!(data::account_flows(&rows, 1), (9_500, 3_000, 2, 2));
}

#[test]
fn the_sample_month_lists_and_searches() {
    let (store, today, dir) = sample();
    let (period, all) = data::period_listing(&store, today, 0, Filter::All).unwrap();
    assert!(period.contains(today));
    assert!(all.totals.count > 0);
    let summed: i64 = all.days.iter().map(|g| g.spent).sum();
    assert_eq!(summed, all.totals.spent, "the days add up to the month");
    let (_, spent) = data::period_listing(&store, today, 0, Filter::Spent).unwrap();
    assert!(spent.days.iter().all(|g| g.rows.iter().all(|t| t.r#type == TxType::Expense)));
    assert_eq!(spent.totals.income, 0);
    // A bill's posting is marked.
    assert!(!all.repeats.is_empty(), "the sample posts bills");

    // Search reads every month, notes and categories alike, in any case.
    let found = data::search_listing(&store, today, "groceries", Filter::All).unwrap();
    assert!(found.totals.count > 0);
    assert!(found.days.iter().flat_map(|g| &g.rows).all(|t| t.category.as_deref() == Some("Groceries") || t.note.to_lowercase().contains("groceries") || t.account.to_lowercase().contains("groceries")));
    let none = data::search_listing(&store, today, "zzzz nothing like this", Filter::All).unwrap();
    assert!(none.days.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_category_page_reads_its_month_and_an_account_its_money() {
    let (store, today, dir) = sample();
    let groceries = store.ledger.categories().unwrap().into_iter().find(|c| c.name == "Groceries").unwrap();
    let (s, l) = data::subject_listing(&store, today, Some(groceries.id), None, "").unwrap().unwrap();
    assert_eq!(s.name, "Groceries");
    assert!(!s.is_account());
    assert_eq!(s.entry_count, l.totals.count);
    assert!(l.days.iter().flat_map(|g| &g.rows).all(|t| t.category_id == Some(groceries.id)));
    let account = store.ledger.accounts().unwrap()[0].clone();
    let (a, _) = data::subject_listing(&store, today, None, Some(account.id), "").unwrap().unwrap();
    assert!(a.is_account());
    assert_eq!(a.name, account.name);
    assert!(data::subject_listing(&store, today, Some(987_654), None, "").unwrap().is_none(), "a deleted category reads as gone");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn insights_reads_the_month_from_the_ledgers_series() {
    let (store, today, dir) = sample();
    let s = ins::read(&store, today, 0).unwrap();
    assert_eq!(s.daily.len() as i64, s.period.days());
    assert_eq!(s.cumulative.len() as i64, s.elapsed);
    assert_eq!(s.cumulative.last().copied().unwrap_or(0), s.spent, "the line ends at what was spent");
    assert_eq!(s.trend.len(), 6);
    assert_eq!(s.trend.last().unwrap().spent, s.spent);
    assert_eq!(s.trend.last().unwrap().offset, 0);
    let by_cat: i64 = s.categories.iter().map(|c| c.total).sum();
    assert_eq!(by_cat, s.spent, "nothing is dropped from where it went");
    assert!(s.categories.len() <= ins::TOP_CATEGORIES);
    assert!(s.largest.windows(2).all(|w| w[0].amount >= w[1].amount));
    let past = ins::read(&store, today, -1).unwrap();
    assert!(!past.current());
    assert_eq!(past.elapsed, past.period.days());
    assert_eq!(past.projected(), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn insights_rules_hold() {
    assert_eq!(ins::heat_step(0, 100), 0);
    assert_eq!(ins::heat_step(1, 100), 1, "a coffee still lights the day");
    assert_eq!(ins::heat_step(60, 100), 3);
    assert_eq!(ins::heat_step(100, 100), 4);
    assert_eq!(ins::pace_at(3_000, 9, 30), 1_000);
    assert_eq!(ins::kept_share(1_000, 250), Some(75));
    assert_eq!(ins::kept_share(0, 250), None);
    let p = BudgetPeriod::containing(d("2026-10-01"), 1);
    let pt = |o: i64, spent: i64, income: i64| ins::TrendPoint { period: p.shift(o), offset: o, spent, income };
    let t = ins::summarize_trend(&[pt(-2, 0, 0), pt(-1, 3_000, 5_000), pt(0, 1_000, 0)], Some(2_000));
    assert_eq!((t.average_spent, t.average_income, t.months, t.over_budget), (3_000, 5_000, 1, 1), "the running month stays out of the average");
    let cal = ins::summarize_calendar(&[0, 500, 0, 0, 0, 200], &p, 6, true);
    assert_eq!((cal.active_days, cal.longest_quiet_run, cal.busiest), (2, 3, Some((d("2026-10-02"), 500))));
    assert_eq!(cal.weekdays.len(), 7);
    assert_eq!(cal.weekdays[0].day, jiff::civil::Weekday::Monday);
}

#[test]
fn a_breakdown_folds_its_tail_into_everything_else() {
    let rows: Vec<Tx> = (1..=10).map(|c| tx(c, TxType::Expense, c * 100, "2026-10-03", Some(c), "")).collect();
    let cats: Vec<relay_money::views::CategoryView> = (1..=10)
        .map(|c| relay_money::views::CategoryView { id: c, name: format!("C{c}"), kind: relay_money::model::CategoryKind::Expense, icon: "dots".into(), color: c, archived: false })
        .collect();
    let s = ins::rank_slices(&rows, TxType::Expense, &cats, 8);
    assert_eq!(s.len(), 8);
    assert_eq!(s[0].name, "C10");
    assert_eq!(s[7].name, "Everything else");
    assert_eq!(s[7].folded, 3);
    assert_eq!(s.iter().map(|x| x.total).sum::<i64>(), 5_500);
    let payees = ins::payees(&[tx(1, TxType::Expense, 500, "2026-10-03", None, "Metro"), tx(2, TxType::Expense, 700, "2026-10-04", None, "metro "), tx(3, TxType::Expense, 900, "2026-10-04", None, "IGA")], 5);
    // Payees group by the exact note, as the Kotlin app does: "Metro" and "metro " are two rows.
    assert_eq!(payees.len(), 3);
    assert_eq!((payees[0].note.as_str(), payees[0].total, payees[0].count), ("IGA", 900, 1));
    assert!(payees.iter().all(|p| p.count == 1), "nothing merged");
}

fn draft(kind: TxType, amount: i64) -> Draft {
    Draft { kind, amount, date: d("2026-10-10"), account: Some(1), to_account: Some(2), category: Some(1) }
}

#[test]
fn a_save_needs_what_its_type_needs() {
    assert_eq!(logic::validate(&draft(TxType::Expense, 100)), None);
    assert_eq!(logic::validate(&draft(TxType::Expense, 0)), Some("Type an amount"));
    assert_eq!(logic::validate(&Draft { category: None, ..draft(TxType::Expense, 100) }), Some("Pick a category"));
    assert_eq!(logic::validate(&Draft { account: None, ..draft(TxType::Expense, 100) }), Some("Pick an account"));
    assert_eq!(logic::validate(&Draft { category: None, ..draft(TxType::Transfer, 100) }), None, "a transfer has no category");
    assert_eq!(logic::validate(&Draft { to_account: Some(1), ..draft(TxType::Transfer, 100) }), Some("Pick two different accounts"));
}

#[test]
fn the_grid_folds_and_keeps_the_pick_in_view() {
    let ids: Vec<i64> = (1..=12).collect();
    let (shown, hidden) = logic::grid_cells(&ids, None, 8);
    assert_eq!((shown.len(), hidden), (7, 5));
    let (shown, hidden) = logic::grid_cells(&ids, Some(11), 8);
    assert_eq!(shown.last(), Some(&10), "the picked category takes the last cell");
    assert_eq!(hidden, 5);
    let (shown, hidden) = logic::grid_cells(&ids[..6], Some(3), 8);
    assert_eq!((shown.len(), hidden), (6, 0));
    assert_eq!(logic::by_use(&[1, 2, 3], &[(3, 5), (2, 1)]), vec![2, 1, 0]);
}

#[test]
fn an_edit_counts_the_month_without_its_old_self() {
    let p = BudgetPeriod::containing(d("2026-10-10"), 1);
    let original = tx(9, TxType::Expense, 5_000, "2026-10-05", Some(1), "");
    // $300 spent, $50 of it this entry, now $80: $330.
    assert_eq!(logic::spent_after(30_000, Some(&original), &draft(TxType::Expense, 8_000), &p), 33_000);
    assert_eq!(logic::spent_after(30_000, None, &draft(TxType::Income, 8_000), &p), 30_000);
    assert_eq!(logic::category_month(20_000, 4, Some(&original), &draft(TxType::Expense, 8_000), &p), (23_000, 4));
    let f = fmt();
    assert_eq!(logic::after_line(TxType::Expense, Some(157_500), None, &f), Some(("$1,575 left this month after this".into(), LineTone::Quiet)));
    assert_eq!(logic::after_line(TxType::Expense, Some(-3_100), None, &f), Some(("Over this month's limit by $31".into(), LineTone::Held)));
    assert_eq!(logic::after_line(TxType::Income, Some(5), None, &f), None);
    assert_eq!(logic::category_line(TxType::Expense, "Groceries", (43_000, 5), Some(60_000), &f).0, "$430 of $600 in Groceries this month, $170 left");
    assert_eq!(logic::category_line(TxType::Expense, "Groceries", (0, 0), None, &f).0, "Nothing in Groceries yet this month");
}

#[test]
fn notices_say_what_happened() {
    let f = fmt();
    assert_eq!(logic::title(TxType::Expense, true), "Edit expense");
    assert_eq!(logic::title(TxType::Transfer, false), "New transfer");
    assert_eq!(logic::saved_notice(TxType::Expense, Some("Groceries"), Some(157_500), &f), "Groceries expense saved · $1,575 left this month");
    assert_eq!(logic::saved_notice(TxType::Income, None, None, &f), "Income saved");
    assert_eq!(logic::saved_notice(TxType::Transfer, None, Some(1), &f), "Transfer saved");
    assert_eq!(logic::deleted_notice(TxType::Expense, Some("Dining")), "Dining entry deleted");
    assert_eq!(logic::deleted_notice(TxType::Transfer, Some("Dining")), "Transfer deleted");
    assert_eq!(logic::every_label("MONTHLY", 1), "Every month");
    assert_eq!(logic::every_label("WEEKLY", 2), "Every 2 weeks");
}

#[test]
fn an_edit_opens_folded_with_its_entry_and_a_delete_can_be_undone() {
    let (mut store, today, dir) = sample();
    let account = store.ledger.accounts().unwrap()[0].id;
    let category = store.ledger.categories().unwrap().into_iter().find(|c| c.name == "Dining").unwrap().id;
    let t = store
        .add(&TxInput { r#type: TxType::Expense, amount: 1_234, date: today.to_string(), account_id: account, to_account_id: None, category_id: Some(category), note: Some("Lunch".into()) })
        .unwrap();
    let e = Entry::open(&store, Some(t.id), None, None, None);
    assert_eq!(e.editing, Some(t.id));
    assert!(!e.pad_open, "an edit opens with the keypad folded");
    assert_eq!((e.amount.minor(), e.category, e.note.as_str(), e.date), (1_234, Some(category), "Lunch", today));
    let fresh = Entry::open(&store, None, Some(TxType::Income), Some(account), None);
    assert!(fresh.pad_open && fresh.editing.is_none());
    assert_eq!((fresh.kind, fresh.account), (TxType::Income, Some(account)));

    store.delete(t.id).unwrap();
    assert!(store.ledger.tx(t.id).is_err());
    store.restore(t.id).unwrap();
    assert_eq!(store.ledger.tx(t.id).unwrap().note, "Lunch");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_usual_are_notes_logged_twice_and_suggestions_finish_a_note() {
    let rows = vec![
        tx(4, TxType::Expense, 925, "2026-10-14", Some(2), "Café Olimpico"),
        tx(3, TxType::Expense, 1_500, "2026-10-12", Some(2), "Burger night"),
        tx(2, TxType::Expense, 465, "2026-10-10", Some(2), "Café Olimpico"),
        tx(1, TxType::Expense, 6_842, "2026-10-09", Some(1), "Metro"),
    ];
    let u = logic::usual(&rows, 8);
    assert_eq!(u.len(), 1, "a note logged once is not usual yet");
    assert_eq!((u[0].note.as_str(), u[0].last_amount, u[0].uses), ("Café Olimpico", 925, 2), "the newest amount is the one offered");
    let notes = vec![("Metro".to_string(), Some(1)), ("Metro Plus".to_string(), Some(1)), ("Marché".to_string(), Some(1))];
    assert_eq!(logic::suggestions(&notes, "met", 6), ["Metro", "Metro Plus"]);
    assert_eq!(logic::suggestions(&notes, "metro", 6), ["Metro Plus"], "never the note as typed");
    assert!(logic::suggestions(&notes, "  ", 6).is_empty());
}

#[test]
fn entries_chips_offer_the_months_categories_and_narrow_to_one() {
    let rows = vec![
        tx(1, TxType::Expense, 500, "2026-10-03", Some(2), "a"),
        tx(2, TxType::Expense, 9_000, "2026-10-04", Some(1), "b"),
        tx(3, TxType::Expense, 700, "2026-10-04", Some(2), "c"),
        tx(4, TxType::Transfer, 50_000, "2026-10-05", None, "d"),
    ];
    assert_eq!(data::categories_by_size(&rows), vec![(1, "C1".to_string()), (2, "C2".to_string())]);

    let (store, today, dir) = sample();
    let (_, all) = data::period_listing_in(&store, today, 0, Filter::All, None).unwrap();
    assert!(!all.categories.is_empty(), "the sample month has categories to pick");
    let (id, _) = all.categories[0].clone();
    let (_, one) = data::period_listing_in(&store, today, 0, Filter::All, Some(id)).unwrap();
    assert!(one.days.iter().flat_map(|g| &g.rows).all(|t| t.category_id == Some(id)));
    assert!(one.totals.count > 0 && one.totals.count < all.totals.count);
    assert_eq!(one.categories, all.categories, "the chips stay put while one is chosen");

    let months = data::recent_periods(&store, today, 12);
    assert_eq!(months.len(), 12);
    assert_eq!(months[0].0, 0);
    assert_eq!(months[11].0, -11);
    assert!(months.iter().all(|(_, n)| !n.is_empty()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rows_name_their_days_and_sign_their_sums() {
    use tally::activity_kit as ak;
    let today = d("2026-11-19");
    assert_eq!(ak::day_name(d("2026-11-19"), today), "Today");
    assert_eq!(ak::day_name(d("2026-11-18"), today), "Yesterday");
    assert_eq!(ak::day_name(d("2026-11-17"), today), "Tuesday 17");
    assert_eq!(ak::day_name(d("2026-10-31"), today), "Saturday 31 Oct");
    assert_eq!(ak::short_day("2026-11-17", today), "17 Nov");
    assert_eq!(ak::short_day("2025-06-16", today), "16 Jun 2025");
    assert_eq!(ak::signed(-7_087, &fmt()), "−$70.87");
    assert_eq!(ak::signed(200_300, &fmt()), "+$2,003.00");
}

#[test]
fn the_pace_card_reads_against_last_month() {
    assert_eq!(ins::running(&[100, 0, 250]), vec![100, 100, 350]);
    let this = [100, 300, 400];
    let last = [200, 250, 700];
    assert_eq!(ins::ahead_by(&this, &last, 3), Some(300), "ahead: less spent than by then last month");
    assert_eq!(ins::ahead_by(&this, &last, 2), Some(-50), "behind");
    assert_eq!(ins::ahead_by(&this, &last[..2], 3), Some(-150), "a shorter last month reads its last day");
    assert_eq!(ins::ahead_by(&this, &[], 1), None);
    assert_eq!(ins::ahead_by(&this, &last, 0), None);
    assert_eq!(ins::ticks(319_200, 100), vec![0, 100_000, 200_000, 300_000, 400_000]);
    assert_eq!(ins::ticks(0, 100), vec![0, 100]);
    assert!(ins::ticks(4_500, 100).len() <= 6);

    let (store, today, dir) = sample();
    let s = ins::read(&store, today, 0).unwrap();
    let prev = ins::read(&store, today, -1).unwrap();
    assert_eq!(s.last_cumulative.last().copied().unwrap_or(0), prev.spent, "the other line is last month's whole spend");
    let _ = std::fs::remove_dir_all(dir);
}


#[test]
fn an_entrys_overview_says_when_and_whether_it_repeats() {
    let today = d("2026-11-19");
    assert_eq!(data::full_date(d("2026-11-19"), today), "Today, November 19");
    assert_eq!(data::full_date(d("2026-11-18"), today), "Yesterday, November 18");
    assert_eq!(data::full_date(d("2026-11-17"), today), "Tuesday, November 17");
    assert_eq!(data::full_date(d("2025-06-16"), today), "Monday, June 16, 2025");

    let bill = |frequency: &str, interval: i64, next: &str, active: bool| logic::LinkedBill { id: 1, frequency: frequency.into(), interval, next: d(next), active, auto_post: true };
    assert_eq!(logic::repeats_line(None, today), "Does not repeat");
    assert_eq!(logic::repeats_line(Some(&bill("MONTHLY", 1, "2026-12-17", true)), today), "Repeats monthly · next on December 17");
    assert_eq!(logic::repeats_line(Some(&bill("WEEKLY", 2, "2027-01-02", true)), today), "Repeats every 2 weeks · next on January 2, 2027");
    assert_eq!(logic::repeats_line(Some(&bill("YEARLY", 1, "2027-01-02", false)), today), "Repeats yearly · paused");

    // The overview reads an entry as the editor does; a bill's posting knows its bill.
    let (store, today, dir) = sample();
    let (_, l) = data::period_listing(&store, today, 0, Filter::All).unwrap();
    let posted = l.days.iter().flat_map(|g| &g.rows).find(|t| l.repeats.contains(&t.id)).unwrap();
    let e = Entry::open(&store, Some(posted.id), None, None, None);
    assert_eq!(e.original.as_ref().map(|t| t.id), Some(posted.id));
    assert!(e.bill.is_some(), "a bill's posting links to its bill");
    assert!(logic::repeats_line(e.bill.as_ref(), today).starts_with("Repeats "));
    let gone = Entry::open(&store, Some(987_654), None, None, None);
    assert!(gone.original.is_none(), "a deleted entry reads as gone");
    let _ = std::fs::remove_dir_all(dir);
}
