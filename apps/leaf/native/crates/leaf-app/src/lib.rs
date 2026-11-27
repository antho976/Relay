//! Leaf, the phone app: books from the phone's own files (EPUB, PDF, text) and comics from the
//! owner's leaf-server, each read as one continuous column, drawn with wgpu on Tally RS's renderer.
//!
//! Books: `shelf` (the local SQLite store and the import), `epub` (reading a book's file),
//! `typeset` (setting text in lines), `book` (the reader), `notes` (highlights, review, find) and
//! `stats`.
//!
//! `app` holds the state and draws a frame; `home`, `space`, `library`, `search`, `series`,
//! `reader` and `connect` are the screens, `recs` the picks (Home's and a series' Readers also
//! like), `update` the app updates from the server, `offline` the downloads kept on the phone and their line (`saved` a series' downloads); `strip` and `logic` the rules they read (tested); `net` and `images` the
//! worker threads that talk to the server and decode; `draw` and `gfx` the canvas and the GPU;
//! `desktop` and `android` the two front ends.

pub mod app;
pub mod book;
pub mod booksync;
pub mod clock;
pub mod connect;
pub mod draw;
pub mod epub;
pub mod gfx;
pub mod glyphs;
pub mod home;
pub mod images;
pub mod kit;
pub mod known;
pub mod library;
pub mod lists;
pub mod logic;
pub mod net;
pub mod notes;
pub mod notices;
pub mod offline;
pub mod online;
pub mod panels;
pub mod places;
pub mod reader;
pub mod rhythm;
pub mod saved;
pub mod recs;
pub mod search;
pub mod series;
pub mod settings;
pub mod shelf;
pub mod smart;
pub mod space;
pub mod stats;
pub mod strip;
pub mod sweep;
pub mod theme;
pub mod typeset;
pub mod ui;
pub mod update;
pub mod wake;
pub mod wrapped;
pub mod wrapped_stats;
pub mod zoom;

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
pub mod desktop;

#[cfg(test)]
mod flows;
