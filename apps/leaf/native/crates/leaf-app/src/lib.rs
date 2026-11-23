//! Leaf, the phone app: comics from the owner's leaf-server, read as one continuous strip, drawn
//! with wgpu on Tally RS's renderer.
//!
//! `app` holds the state and draws a frame; `library`, `search`, `series`, `reader` and `connect`
//! are the screens; `strip` and `logic` the rules they read (tested); `net` and `images` the
//! worker threads that talk to the server and decode; `draw` and `gfx` the canvas and the GPU;
//! `desktop` and `android` the two front ends.

pub mod app;
pub mod clock;
pub mod connect;
pub mod draw;
pub mod gfx;
pub mod glyphs;
pub mod images;
pub mod kit;
pub mod library;
pub mod logic;
pub mod net;
pub mod reader;
pub mod search;
pub mod series;
pub mod strip;
pub mod theme;
pub mod ui;
pub mod wake;

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
pub mod desktop;
