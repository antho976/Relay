//! Aide, the person's own agent (docs/AIDE.md): one agent that keeps working while nobody is
//! typing to it, watches the person's apps, speaks first when something needs them, and asks
//! before anything that matters.
//!
//! This crate is Aide's rules, kept free of the engine so they can be tested alone and read by the
//! bus contract: what a job is ([`model`]), when it is due ([`schedule`]), which model may read
//! which data and which ops a run is given ([`policy`]), and the SQLite file that keeps it all,
//! `aide.db` beside the store ([`store`], behind the `store` feature). The engine (`relay-core`'s
//! `aide`) wakes jobs, runs them as thread turns and carries the outbox.
//!
//! Times are RFC 3339 UTC strings, the bus's one format; schedules are in the PC's own time zone.

pub mod model;
pub mod policy;
pub mod schedule;
pub mod templates;

#[cfg(feature = "store")]
pub mod store;
