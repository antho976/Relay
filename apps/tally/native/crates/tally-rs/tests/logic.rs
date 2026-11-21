//! The app's own rules (the money rules are relay-money's, tested there).

use relay_money::money::{Locale, MoneyFormatter};
use relay_money::pace::PaceReading;
use tally::kit::Scene;
use tally::model::{self, LineTone, Store};

fn fmt() -> MoneyFormatter {
    MoneyFormatter::new("CAD", Locale::new("en-CA"))
}

#[test]
fn goals_get_the_scene_their_name_asks_for() {
    assert_eq!(Scene::for_goal("Trip to Japan", false), Scene::Mountain);
    assert_eq!(Scene::for_goal("Emergency fund", false), Scene::Beach);
    assert_eq!(Scene::for_goal("New car", false), Scene::Road);
    assert_eq!(Scene::for_goal("Condo down payment", false), Scene::City);
    assert_eq!(Scene::for_goal("Anything", true), Scene::Growth);
    // The same name always lands on the same scene.
    assert_eq!(Scene::for_goal("Guitar", false), Scene::for_goal("Guitar", false));
}

#[test]
fn today_reads_against_its_share_of_what_is_left() {
    // $300 budget, $100 spent before today, 10 of 30 days elapsed: $200 over 21 days is a $9.52
    // share for today.
    let r = PaceReading::new(30_000, 10_000 + 500, 30, 10);
    let (words, tone) = model::today_line(500, Some(&r), &fmt());
    assert_eq!((words.as_str(), tone), ("$5 left for today", LineTone::Quiet));
    let r = PaceReading::new(30_000, 10_000 + 1_500, 30, 10);
    let (words, tone) = model::today_line(1_500, Some(&r), &fmt());
    assert_eq!((words.as_str(), tone), ("$5 past today's share", LineTone::Warn));
    let (words, tone) = model::today_line(0, None, &fmt());
    assert_eq!((words.as_str(), tone), ("Nothing spent yet today", LineTone::Quiet));
}

#[test]
fn a_reached_limit_is_said_in_red_and_never_as_a_negative() {
    let r = PaceReading::new(10_000, 12_500, 30, 20);
    assert!(model::limit_reached(&r));
    let (words, tone) = model::hero_line(&r, &fmt());
    assert_eq!(tone, LineTone::Held);
    assert_eq!(words, "$25 past your $100 limit");
}

#[test]
fn overview_reads_the_sample_ledger() {
    let mut store = Store::in_memory().unwrap();
    let today = model::today();
    store.load_sample(today).unwrap();
    let home = store.home(today).unwrap();
    assert!(home.has_entries());
    assert!(home.today_in_week < 7);
    assert!(home.shares.len() <= 5);
    let spent: i64 = home.shares.iter().map(|s| s.total).sum();
    assert_eq!(spent, home.s.spent, "the donut adds up to the period's spending");
}

#[test]
fn period_names_read_as_a_month_or_a_range() {
    assert_eq!(model::period_name("2026-11-01", "2026-12-01"), "November");
    assert_eq!(model::period_name("2026-10-15", "2026-11-15"), "Oct 15 – Nov 14");
}
