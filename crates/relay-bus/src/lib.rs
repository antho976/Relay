//! RELAY v4 command bus — the app's entire API, as types.
//!
//! This crate is the schema. It has no I/O: `relay-core` executes ops, the doors (the Unix
//! socket the native client, CLI and MCP share, and the phone door that forwards to it) carry
//! envelopes, and `schema/bus.v1.json` is rendered from here. `docs/engine/BUS.md` is the
//! human-readable form of the same contract.

// `BusError` is ~250 bytes and is the *normal* path for typed refusals; boxing it in every
// handler signature would obscure the API for no measurable gain.
#![allow(clippy::result_large_err)]

pub mod design;
pub mod envelope;
pub mod error;
pub mod registry;
pub mod schema;
pub mod types;
pub mod ops;

pub use envelope::{Actor, Event, MailHint, Request, Response, ENVELOPE_V};
pub use error::{BusError, Confirm, ErrorKind};
pub use registry::{
    Actors, Audit, Doors, Op, OpEntry, OpKind, OpMeta, Registry, Scope, Undo,
};
pub use types::Id;

/// For `Option<Option<T>>` patch fields: distinguishes "absent" (outer `None`, leave as is)
/// from an explicit `null` (`Some(None)`, clear it). Serde alone collapses both to `None`.
/// Use as `#[serde(default, deserialize_with = "relay_bus::nullable")]`.
pub fn nullable<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(d).map(Some)
}

/// Sentinel value for "no payload" / "no result".
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
