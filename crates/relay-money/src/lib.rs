//! Tally's money rules, for the engine and for Tally RS (`apps/tally`).
//!
//! Ported from the Kotlin Tally's `core/` (antho976/Tally, now `apps/tally`'s history), module for
//! module, with its tests. The phone and the PC each keep a full copy of the ledger and both run
//! this crate, so they read the same rows to the same numbers; a change here is tested on Tally's
//! side too (`cd apps/tally/native && cargo test`).
//!
//! Money is an `i64` of MINOR units (cents for CAD) everywhere. Floating point never touches an
//! amount. Dates are [`jiff::civil::Date`].

pub mod amount_input;
pub mod backup;
pub mod bank_statements;
pub mod copy;
pub mod csv;
pub mod icon_hints;
pub mod invest;
#[cfg(feature = "ledger")]
pub mod ledger;
pub mod model;
pub mod money;
pub mod pace;
pub mod payees;
pub mod period;
#[cfg(feature = "ledger")]
pub mod plaid;
#[cfg(feature = "ledger")]
mod portfolio;
pub mod recurrence;
pub mod sample_data;
pub mod series;
#[cfg(feature = "ledger")]
pub mod sync;
pub mod text_match;
pub mod views;
pub mod wealthsimple;

pub use jiff::civil::Date;
