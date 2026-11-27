//! The key strip (backlog D13): Esc, Tab, ↑, ↓, Shift-Tab and Enter, typed into the agent's
//! terminal with [`App::key`]. It answers what the question card can't read: `/model` with no
//! argument, `/resume`, `/agents`, rewinding with Esc Esc. It opens over the chat's message box
//! from a key in the composer, and sits at the foot of the Terminal page (`terminal.rs`).

use crate::app::App;
use crate::draw::{Text, WIDTH};
use crate::theme::{self as t, ty};

/// Each key: its label (or glyph) and what it types.
pub const KEYS: [(&str, Option<&str>, &str); 6] = [
    ("Esc", None, "\x1b"),
    ("Tab", None, "\t"),
    ("", Some("arrow-up"), "\x1b[A"),
    ("", Some("arrow-down"), "\x1b[B"),
    ("Shift Tab", None, "\x1b[Z"),
    ("Enter", None, "\r"),
];

/// The strip's height: one row of keys and its room.
pub const STRIP_H: f32 = 52.0;
const KEY_H: f32 = 40.0;
const GAP: f32 = 6.0;

#[derive(Default)]
pub struct State {
    /// The strip is up over the chat's message box.
    pub open: bool,
}

/// What the chat's foot grows by while the strip is up.
pub fn height(app: &App) -> f32 {
    if app.st.keys.open { STRIP_H } else { 0.0 }
}

/// The keys' widths: each its words and some room, the rest shared out, filling `w`.
fn widths(app: &mut App, w: f32) -> [f32; 6] {
    let tt = Text::new(ty::UI, t::INK).medium();
    let mut out = [0.0; 6];
    for (i, (label, glyph, _)) in KEYS.iter().enumerate() {
        out[i] = if glyph.is_some() { 20.0 } else { app.canvas.measure(label, tt) } + 20.0;
    }
    let spare = (w - out.iter().sum::<f32>() - GAP * 5.0).max(0.0) / 6.0;
    out.map(|k| k + spare)
}

/// One row of keys from `x`, `w` wide, at `y`; a tap types its key into `agent`'s terminal.
pub fn row(app: &mut App, agent: &str, x: f32, y: f32, w: f32) {
    let ws = widths(app, w);
    let mut kx = x;
    let mut sent = None;
    for (i, (label, glyph, bytes)) in KEYS.iter().enumerate() {
        let kw = ws[i];
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(kx, y, kw, KEY_H);
        cx.c.rect(kx, y, kw, KEY_H, t::KEY_R, if pressed { t::S3 } else { t::S2 });
        match glyph {
            Some(g) => cx.c.icon(g, kx + kw / 2.0, y + KEY_H / 2.0, 16.0, t::INK),
            None => {
                let tt = Text::new(ty::UI, t::INK).medium().center();
                cx.c.text(label, kx + kw / 2.0, y + (KEY_H - tt.lh()) / 2.0, tt);
            }
        }
        // The whole key's height and the gaps beside it: 44 units to the finger.
        if cx.tap(kx - GAP / 2.0, y - 2.0, kw + GAP, KEY_H + 4.0) {
            sent = Some(*bytes);
        }
        kx += kw + GAP;
    }
    if let Some(b) = sent {
        app.host.haptic();
        app.key(agent, b);
        crate::terminal::soon(app);
    }
}

/// The strip in the chat's foot, from `top` down, just above the message box: drawn while it is
/// up. Returns the height it took ([`height`]).
pub fn strip(app: &mut App, agent: &str, top: f32) -> f32 {
    if !app.st.keys.open {
        return 0.0;
    }
    let saved = app.canvas.dy;
    app.canvas.dy = 0.0;
    let band = [-1000.0, top, WIDTH + 2000.0, STRIP_H];
    crate::kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(band[0], band[1], band[2], band[3], 0.0, t::BG);
    row(app, agent, 12.0, top + (STRIP_H - KEY_H) / 2.0 - 4.0, WIDTH - 24.0);
    app.canvas.dy = saved;
    STRIP_H
}

/// The composer's key that shows or hides the strip, `d` square at (`x`, `y`). True when tapped.
pub fn toggle(app: &mut App, x: f32, y: f32, d: f32) -> bool {
    let on = app.st.keys.open;
    let mut cx = crate::cx!(app);
    if on {
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, t::S3);
    } else if cx.pressed(x, y, d, d) {
        cx.c.circle(x + d / 2.0, y + d / 2.0, d / 2.0, t::WASH);
    }
    cx.c.icon("terminal", x + d / 2.0, y + d / 2.0, 16.0, if on { t::INK } else { t::INK2 });
    // 44 units to the finger, whatever its drawn size.
    let pad = ((44.0 - d) / 2.0).max(0.0);
    if cx.tap(x - pad, y - pad, d + 2.0 * pad, d + 2.0 * pad) {
        app.st.keys.open = !on;
        app.host.haptic();
        return true;
    }
    false
}
