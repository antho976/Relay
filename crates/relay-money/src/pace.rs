//! Where a budget stands against an even spend. Port of `Pace.kt`.
//!
//! The app's one signature reading: "you have spent 62%" says nothing on its own, "$84 over pace
//! with 9 days left" says what to do. `elapsed_days` counts today, so on day 1 of a 31-day month
//! the pace already allows 1/31 of the budget: money spent today is spent today.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaceStatus {
    NoBudget,
    UnderPace,
    OnPace,
    OverPace,
    OverBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PaceReading {
    #[schemars(extend("x-unit" = "minor"))]
    pub budget: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub spent: i64,
    pub total_days: i64,
    pub elapsed_days: i64,
    /// What an even spend would have used by the end of today.
    #[schemars(extend("x-unit" = "minor"))]
    pub expected: i64,
    #[schemars(extend("x-unit" = "minor"))]
    pub remaining: i64,
    /// Positive when spending runs faster than an even pace.
    #[schemars(extend("x-unit" = "minor"))]
    pub pace_delta: i64,
    /// Days still to spend in, today included.
    pub days_left: i64,
    /// What can go out each remaining day and still land on budget. Zero once the budget is gone.
    #[schemars(extend("x-unit" = "minor"))]
    pub daily_allowance: i64,
    /// Spent as a fraction of the budget; may exceed 1. Zero without a budget.
    #[schemars(extend("x-unit" = "fraction"))]
    pub spent_fraction: f32,
    /// Where the pace tick sits on the meter, 0..1.
    #[schemars(extend("x-unit" = "fraction"))]
    pub pace_fraction: f32,
    pub status: PaceStatus,
}

impl PaceReading {
    /// # Panics
    /// When `total_days` is not positive.
    pub fn new(budget: i64, spent: i64, total_days: i64, elapsed_days: i64) -> Self {
        assert!(total_days > 0, "A period has at least one day");
        let elapsed = elapsed_days.clamp(0, total_days);
        let expected = if budget <= 0 { 0 } else { (budget as f64 * elapsed as f64 / total_days as f64).round() as i64 };
        let remaining = budget - spent;
        let pace_delta = spent - expected;
        let days_left = (total_days - elapsed + i64::from((1..=total_days).contains(&elapsed))).max(0);
        let daily_allowance = if remaining <= 0 || days_left == 0 { 0 } else { remaining / days_left };
        let spent_fraction = if budget <= 0 { 0.0 } else { (spent as f64 / budget as f64) as f32 };
        let status = if budget <= 0 {
            PaceStatus::NoBudget
        } else if spent > budget {
            PaceStatus::OverBudget
        // A tolerance of half a day's allowance, so a single coffee does not flip the verdict.
        } else if pace_delta.abs() <= budget / total_days / 2 {
            PaceStatus::OnPace
        } else if pace_delta > 0 {
            PaceStatus::OverPace
        } else {
            PaceStatus::UnderPace
        };
        PaceReading {
            budget,
            spent,
            total_days,
            elapsed_days,
            expected,
            remaining,
            pace_delta,
            days_left,
            daily_allowance,
            spent_fraction,
            pace_fraction: elapsed as f32 / total_days as f32,
            status,
        }
    }
}

// Amounts in tests are written in cents as the Kotlin tests write them: 310_00 is $310.00.
#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;

    #[test]
    fn even_spend_is_on_pace() {
        let r = PaceReading::new(310_00, 100_00, 31, 10);
        assert_eq!(r.expected, 100_00);
        assert_eq!(r.status, PaceStatus::OnPace);
        assert_eq!(r.days_left, 22);
        assert_eq!(r.daily_allowance, 210_00 / 22);
    }

    #[test]
    fn spending_faster_is_over_pace_by_the_difference() {
        let r = PaceReading::new(310_00, 150_00, 31, 10);
        assert_eq!(r.status, PaceStatus::OverPace);
        assert_eq!(r.pace_delta, 50_00);
    }

    #[test]
    fn spending_slower_is_under_pace() {
        assert_eq!(PaceReading::new(310_00, 40_00, 31, 10).status, PaceStatus::UnderPace);
    }

    #[test]
    fn past_the_budget_is_over_budget_whatever_the_day() {
        let r = PaceReading::new(100_00, 120_00, 30, 29);
        assert_eq!(r.status, PaceStatus::OverBudget);
        assert_eq!(r.daily_allowance, 0);
    }

    #[test]
    fn half_a_days_allowance_is_still_on_pace() {
        // 300/30 = 10 a day; tolerance 5.
        assert_eq!(PaceReading::new(300_00, 104_00, 30, 10).status, PaceStatus::OnPace);
    }

    #[test]
    fn no_budget_never_claims_a_pace() {
        let r = PaceReading::new(0, 50_00, 30, 10);
        assert_eq!(r.status, PaceStatus::NoBudget);
        assert_eq!(r.spent_fraction, 0.0);
    }

    #[test]
    fn the_tick_sits_at_the_share_of_the_month_gone() {
        assert_eq!(PaceReading::new(100, 0, 30, 15).pace_fraction, 0.5);
    }
}
