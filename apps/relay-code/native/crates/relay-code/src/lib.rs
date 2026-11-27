//! Relay Code, the phone app: Relay's agents and their conversations, coding only (design 44).
//!
//! `app` holds the state and draws a frame; `pair`, `home`, `chat` and `settings` are the pages,
//! `drawer` the drawer and its sheets (route A); `data` the rules that read what the PC says
//! (tested); the link to the PC is the `rc-link` crate. `draw` and `gfx` are the canvas and the GPU
//! (Leaf's and Tally RS's renderer); `desktop` and `android` the two front ends.

pub mod agents;
pub mod app;
pub mod attach;
pub mod background;
pub mod board;
pub mod changes;
pub mod design_paint;
pub mod designs;
pub mod inbox;
pub mod chat;
pub mod clock;
pub mod commands;
pub mod data;
pub mod draw;
pub mod drawer;
pub mod gfx;
pub mod glyphs;
pub mod home;
pub mod kit;
pub mod pair;
pub mod sample;
pub mod screen;
pub mod settings;
pub mod theme;
pub mod ui;
pub mod update;
pub mod wake;

#[cfg(test)]
mod flows;

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
pub mod desktop;
