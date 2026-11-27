//! Relay Code's link to the PC: the phone door's wire (`wire`), pairing links and routes
//! (`pairlink`), pairing through the owner's server (`server`), and the link itself (`link`),
//! which connects by itself once a PC is paired and stays connected. No UI, so it is tested on
//! its own, against a real door too (`tests/live_door.rs`).

pub mod link;
pub mod pairlink;
pub mod server;
pub mod wire;

pub use link::{Call, FileStore, Link, LinkState, Profile, Refusal, Store};
pub use pairlink::{PairLink, Route, RouteKind};
pub use wire::{BusError, Event};
