//! Relay's remote door: the same bus lines the Unix socket carries (BUS.md §6.2), over a
//! WebSocket, for a phone that paired with this machine once.
//!
//! Two routes, one protocol:
//!
//! - **direct** (`direct.rs`): the engine listens on the LAN; the phone connects when it is on
//!   the same WiFi. Nothing leaves the network, but the network can read it: plain `ws://`.
//! - **via a rendezvous** (`rendezvous.rs`, `tunnel.rs`): the engine dials out to a server the
//!   person hosts, and phones anywhere join through it. The server only copies lines, and every
//!   credential check happens here — but it can read the lines, and write into them.
//!
//! Neither route seals the lines after the handshake or authenticates the PC to the phone, so
//! an on-path attacker can read and inject (docs/MOBILE.md §6). Sealing them is a wire change on
//! both ends, still to come.
//!
//! Both routes hand `bridge.rs` a pair of channels. It greets, verifies the device against
//! `remote.json` (`registry.rs`), then forwards bus lines to the socket door as actor `user`
//! and back — only the ops the app uses (`wire::PHONE_OPS`). The phone never gets a door the
//! CLI does not have, and gets less of it.

pub mod bridge;
pub mod cloud;
pub mod direct;
pub mod pairlink;
pub mod registry;
pub mod rendezvous;
pub mod tunnel;
pub mod wake;
pub mod wire;

pub use bridge::Ctx;
pub use registry::{Registry, DEFAULT_PORT, DEFAULT_RENDEZVOUS_PORT};
