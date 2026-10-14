//! Tally's money rules, for the engine.
//!
//! A port of `core/` in Tally's repository (antho976/Tally, Kotlin), module for module, held to the
//! same tests: the phone and the PC each keep a full copy of the ledger, so both must read the same
//! rows to the same numbers. When a rule changes in one, it changes in the other in the same piece
//! of work: a PR in each repository, cross-linked, tests included.
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
