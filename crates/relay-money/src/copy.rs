//! Every generated sentence the app shows about money. Port of `Copy.kt`.
//!
//! One place, so the voice stays one voice and a test can hold it to the rules: dry, specific, no
//! exclamation marks, no em dashes, no praise the numbers do not support. Lines vary with the
//! numbers rather than repeating one cue.

use crate::money::MoneyFormatter;
use crate::pace::{PaceReading, PaceStatus};

/// "1 day", "2 days": `one` takes an "s" for any count but one.
pub fn plural(n: i64, one: &str) -> String {
    plural_as(n, one, &format!("{one}s"))
}

/// [`plural`] for a word whose plural is not `one` plus "s".
pub fn plural_as(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The margin bar's reading: what each remaining day can take, or how far over it went.
pub fn margin_line(r: &PaceReading, fmt: &MoneyFormatter) -> String {
    match r.status {
        PaceStatus::NoBudget => "No budget set for this month".to_string(),
        PaceStatus::OverBudget => format!("{} over budget, {} left", fmt.format_whole(-r.remaining), plural(r.days_left, "day")),
        _ if r.days_left <= 1 => format!("{} left for today", fmt.format_whole(r.remaining)),
        _ => format!("{} a day for {}", fmt.format_whole(r.daily_allowance), plural(r.days_left, "day")),
    }
}

/// The pace verdict, in money rather than percent, so it says what to change.
pub fn pace_line(r: &PaceReading, fmt: &MoneyFormatter) -> String {
    match r.status {
        PaceStatus::NoBudget => String::new(),
        PaceStatus::OnPace => "On pace".to_string(),
        PaceStatus::OverPace => format!("{} over pace", fmt.format_whole(r.pace_delta)),
        PaceStatus::UnderPace => format!("{} under pace", fmt.format_whole(-r.pace_delta)),
        PaceStatus::OverBudget => "Over budget".to_string(),
    }
}

/// The pace verdict in days: how many days of an even spend the month runs ahead of, or has in
/// hand. Never zero days: a reading off pace is at least one day off it.
pub fn days_line(r: &PaceReading) -> String {
    let days = || crate::invest::mul_div_half_even(r.pace_delta.abs(), r.total_days, r.budget).unwrap_or(0).max(1);
    match r.status {
        PaceStatus::NoBudget => String::new(),
        PaceStatus::OverBudget => "Over budget".to_string(),
        PaceStatus::OnPace => "On pace".to_string(),
        PaceStatus::OverPace => format!("{} ahead of your money", plural(days(), "day")),
        PaceStatus::UnderPace => format!("{} of room in hand", plural(days(), "day")),
    }
}

/// One envelope's reading, for a budget row: "$212 of $300".
pub fn of_budget(spent: i64, budget: i64, fmt: &MoneyFormatter) -> String {
    format!("{} of {}", fmt.format_whole(spent), fmt.format_whole(budget))
}

/// The month against the one before, by the same day. Nothing by this day last month says just
/// that: it is not proof of a first month, since earlier months can hold entries on later days.
pub fn versus_last_line(this_period: i64, last_period_same_day: i64, fmt: &MoneyFormatter) -> String {
    if last_period_same_day <= 0 {
        return if this_period == 0 { "Nothing spent yet" } else { "Nothing by this day last month" }.to_string();
    }
    let diff = this_period - last_period_same_day;
    match diff {
        0 => "Level with last month by this day".to_string(),
        d if d > 0 => format!("{} more than last month by this day", fmt.format_whole(d)),
        d => format!("{} less than last month by this day", fmt.format_whole(-d)),
    }
}

/// A bill's due line.
pub fn due_line(days_until: i64) -> String {
    match days_until {
        d if d < 0 => format!("Overdue by {}", plural(-d, "day")),
        0 => "Due today".to_string(),
        1 => "Due tomorrow".to_string(),
        d => format!("Due in {}", plural(d, "day")),
    }
}

/// A goal's pace: what each month needs to land on the date.
pub fn goal_line(saved: i64, target: i64, months_left: Option<i64>, fmt: &MoneyFormatter) -> String {
    let left = target - saved;
    match months_left {
        _ if left <= 0 => "Reached".to_string(),
        None => format!("{} to go", fmt.format_whole(left)),
        Some(m) if m <= 0 => format!("{} to go, date passed", fmt.format_whole(left)),
        Some(m) => format!("{} a month for {}", fmt.format_whole((left + m - 1) / m), plural(m, "month")),
    }
}

/// A balance goal's pace, from `start` (what the balance read when the goal was set) toward
/// `target`. One heading down (a card to pay off, a debt to clear) counts what is left down to the
/// target, and is reached at or below it. Port of the BALANCE head of `goalRowLine` (PlanLogic.kt).
pub fn balance_goal_line(saved: i64, target: i64, start: i64, months_left: Option<i64>, fmt: &MoneyFormatter) -> String {
    let falling = target < start;
    let reached = if falling { saved <= target } else { saved >= target && target != start };
    let left = if falling { saved - target } else { target - saved }.max(0);
    match months_left {
        _ if reached => "Reached".to_string(),
        None => format!("{} to go", fmt.format_whole(left)),
        Some(m) if m <= 0 => format!("{} to go, date passed", fmt.format_whole(left)),
        Some(m) => format!("{} a month for {}", fmt.format_whole((left + m - 1) / m), plural(m, "month")),
    }
}

/// Characters and words the app never renders. The doctrine test scans every string with it.
pub const BANNED: &[&str] = &["!", "—", "awesome", "crush", "amazing", "great job", "oops"];

#[cfg(test)]
// Amounts written as the Kotlin writes them: 310_00 is $310.00 in cents.
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use crate::money::Locale;

    fn fmt() -> MoneyFormatter {
        MoneyFormatter::new("CAD", Locale::new("en-CA"))
    }

    #[test]
    fn margin_line_names_the_daily_allowance() {
        let r = PaceReading::new(310_00, 100_00, 31, 10);
        assert_eq!(margin_line(&r, &fmt()), "$10 a day for 22 days");
    }

    #[test]
    fn over_budget_names_the_overrun() {
        let r = PaceReading::new(100_00, 130_00, 30, 20);
        assert_eq!(margin_line(&r, &fmt()), "$30 over budget, 11 days left");
    }

    #[test]
    fn last_day_reads_as_today() {
        let r = PaceReading::new(100_00, 90_00, 30, 30);
        assert_eq!(margin_line(&r, &fmt()), "$10 left for today");
    }

    #[test]
    fn plural_is_right_at_one() {
        assert_eq!(plural(1, "day"), "1 day");
        assert_eq!(plural(2, "day"), "2 days");
    }

    #[test]
    fn goal_line_divides_what_is_left_over_the_months() {
        assert_eq!(goal_line(700_00, 1_000_00, Some(3), &fmt()), "$100 a month for 3 months");
        assert_eq!(goal_line(1_000_00, 1_000_00, Some(3), &fmt()), "Reached");
    }

    #[test]
    fn a_balance_goal_heading_down_counts_down_to_its_target() {
        // A card owing $2,000 (-2,000) to be cleared (0): rising, from -2,000 toward 0.
        assert_eq!(balance_goal_line(-1_400_00, 0, -2_000_00, Some(2), &fmt()), "$700 a month for 2 months");
        // A balance of $5,000 to bring down to $1,000: falling. Not reached on day one.
        assert_eq!(balance_goal_line(5_000_00, 1_000_00, 5_000_00, Some(4), &fmt()), "$1,000 a month for 4 months");
        assert_eq!(balance_goal_line(2_200_00, 1_000_00, 5_000_00, None, &fmt()), "$1,200 to go");
        assert_eq!(balance_goal_line(1_000_00, 1_000_00, 5_000_00, Some(4), &fmt()), "Reached");
        assert_eq!(balance_goal_line(900_00, 1_000_00, 5_000_00, Some(0), &fmt()), "Reached");
        // Heading up, as before.
        assert_eq!(balance_goal_line(3_000_00, 5_000_00, 1_000_00, Some(0), &fmt()), "$2,000 to go, date passed");
        assert_eq!(balance_goal_line(5_000_00, 5_000_00, 1_000_00, Some(1), &fmt()), "Reached");
    }

    #[test]
    fn nothing_by_this_day_last_month_never_claims_a_first_month() {
        let f = fmt();
        assert_eq!(versus_last_line(4_00, 0, &f), "Nothing by this day last month");
        assert_eq!(versus_last_line(0, 0, &f), "Nothing spent yet");
        assert_eq!(versus_last_line(10_00, 4_00, &f), "$6 more than last month by this day");
        assert_eq!(versus_last_line(4_00, 4_00, &f), "Level with last month by this day");
    }

    #[test]
    fn days_line_counts_the_days_spending_runs_ahead_of_an_even_pace() {
        // 3,100 over 31 days is 100 a day; by day 10 an even spend is 1,000.
        assert_eq!("4 days ahead of your money", days_line(&PaceReading::new(310_000, 140_000, 31, 10)));
        assert_eq!("1 day ahead of your money", days_line(&PaceReading::new(310_000, 108_000, 31, 10)));
    }

    #[test]
    fn days_line_counts_the_days_of_room_in_hand() {
        assert_eq!("3 days of room in hand", days_line(&PaceReading::new(310_000, 70_000, 31, 10)));
        assert_eq!("1 day of room in hand", days_line(&PaceReading::new(310_000, 92_000, 31, 10)));
    }

    #[test]
    fn days_line_says_nothing_without_a_budget() {
        assert_eq!("", days_line(&PaceReading::new(0, 10_000, 31, 10)));
        assert_eq!("On pace", days_line(&PaceReading::new(310_000, 100_000, 31, 10)));
        assert_eq!("Over budget", days_line(&PaceReading::new(100_00, 130_00, 30, 20)));
    }

    #[test]
    fn no_generated_line_breaks_the_voice_rules() {
        let f = fmt();
        let readings = [
            PaceReading::new(310_00, 100_00, 31, 10), PaceReading::new(310_00, 200_00, 31, 10),
            PaceReading::new(310_00, 10_00, 31, 10), PaceReading::new(100_00, 300_00, 31, 31),
            PaceReading::new(0, 10_00, 31, 10),
        ];
        let mut lines: Vec<String> = readings.iter().flat_map(|r| [margin_line(r, &f), pace_line(r, &f), days_line(r)]).collect();
        lines.extend([due_line(-3), due_line(0), due_line(1), due_line(9)]);
        lines.extend([versus_last_line(10, 0, &f), versus_last_line(10, 20, &f), versus_last_line(30, 20, &f)]);
        for line in &lines {
            for bad in BANNED {
                assert!(!line.to_lowercase().contains(&bad.to_lowercase()), "\"{line}\" contains \"{bad}\"");
            }
        }
    }
}
