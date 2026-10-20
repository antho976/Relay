//! `design.find` and `design.fonts` (MODEL-SPEC §3.3; feature-agent-api §3.6-§3.7): a design's
//! nodes found by kind, name, words, colour or font as drawn, and the font families the desktop
//! app draws. Both unlocked queries. Filled by engine-find-fonts.

use crate::engine::Engine;

/// The find and fonts handlers, once written.
pub(super) fn register(_e: &mut Engine) {}
