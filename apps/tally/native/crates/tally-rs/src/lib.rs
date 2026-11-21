//! Tally, native: Tally's screens drawn with wgpu over relay-money's ledger.
//!
//! `app` holds the state and draws a frame; `overview` and `entry` are the screens ported so far;
//! `kit` is direction C's components; `draw` and `gfx` are the canvas and the GPU; `desktop` and
//! `android` are the two front ends.

pub mod accounts;
pub mod accounts_kit;
pub mod accounts_store;
pub mod activity;
pub mod activity_data;
pub mod activity_kit;
pub mod app;
pub mod banks;
pub mod banks_logic;
pub mod categories;
pub mod categories_kit;
pub mod categories_store;
pub mod category_icons;
pub mod clock;
pub mod draw;
pub mod entry;
pub mod entry_logic;
pub mod gfx;
pub mod glyphs;
pub mod import;
pub mod import_logic;
pub mod import_store;
pub mod insights;
pub mod insights_charts;
pub mod insights_data;
pub mod invest;
pub mod invest_logic;
pub mod kit;
pub mod model;
pub mod nav;
pub mod onboarding;
pub mod overview;
pub mod pictures;
pub mod plan;
pub mod plan_kit;
pub mod plan_logic;
pub mod plan_store;
pub mod settings;
pub mod settings_files;
pub mod settings_kit;
pub mod settings_logic;
pub mod settings_store;
pub mod sync;
pub mod sync_ledger;
pub mod sync_net;
pub mod sync_wire;
pub mod theme;
pub mod ui;
pub mod widgets;

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
pub mod desktop;
