//! The Terminal page (backlog D13): the agent's whole screen as its terminal draws it, read-only,
//! with the key strip at its foot. It is where you answer what the question card can't read
//! (`/model` with no argument, `/resume`, `/agents`, a rewind) and come back. Opened from a chat's
//! menu.
//!
//! The screen comes from `session.screen` with every row (rendered text, a few KB), never from
//! `session.scrollback`: re-read every 0.7 s while the agent works and every 2 s while it waits,
//! and a moment after a key, through `Link::call`, polled each frame.

use rc_link::Call;
use serde_json::{Value, json};

use crate::app::App;
use crate::draw::{Text, WIDTH, Weight};
use crate::kit;
use crate::theme::{self as t, ty};

const TOP_H: f32 = 56.0;
/// The most rows asked for: the PC answers the terminal's own height when it is less.
const ROWS: u32 = 200;
/// The screen's text at most this big, and at least this small to fit its columns.
const MONO_MAX: f32 = 12.5;
const MONO_MIN: f32 = 8.0;

#[derive(Default)]
pub struct State {
    /// The agent the screen is of, its text, columns, and why it could not be read.
    name: String,
    pub text: Option<String>,
    cols: u16,
    error: Option<String>,
    call: Option<Call>,
    /// When the screen is read again (ns).
    at: u64,
    /// Scrolled up from the bottom by the person.
    unstuck: bool,
    /// Which characters the mono face draws.
    faces: std::collections::HashMap<char, bool>,
}

/// Read the screen when it is time, and take the answer.
fn keep_up(app: &mut App, name: &str) {
    if app.st.terminal.name != name {
        app.st.terminal = State { name: name.to_string(), ..State::default() };
    }
    if let Some(call) = &app.st.terminal.call
        && let Some(r) = call.poll()
    {
        app.st.terminal.call = None;
        match r {
            Ok(v) => {
                app.st.terminal.text = v.get("text").and_then(Value::as_str).map(str::to_string);
                app.st.terminal.cols = v.get("cols").and_then(Value::as_u64).unwrap_or(0) as u16;
                app.st.terminal.error = None;
            }
            Err(e) if e.code == "session.not_spawned" => app.st.terminal.error = Some("This agent has no terminal running.".into()),
            Err(e) => app.st.terminal.error = Some(format!("Couldn't read its terminal: {}", e.message)),
        }
    }
    if app.online() && app.st.terminal.call.is_none() && app.now_ns >= app.st.terminal.at {
        app.st.terminal.call = Some(app.link.call("session.screen", json!({"session": name, "rows": ROWS})));
        let every = if app.busy(name) { 700_000_000 } else { 2_000_000_000 };
        app.st.terminal.at = app.now_ns + every;
    }
    if app.online() {
        app.wake_at(app.st.terminal.at);
    }
}

/// A key was just typed: read the screen again a moment after, when the program has redrawn.
pub fn soon(app: &mut App) {
    app.st.terminal.at = app.st.terminal.at.min(app.now_ns + 250_000_000);
}

/// The screen's text style: mono, as big as fits its columns across the page, within bounds.
fn mono(app: &mut App, cols: u16, w: f32) -> Text {
    let base = Text::new(MONO_MAX, t::INK).face(Weight::Mono).line(1.25);
    let adv = app.canvas.measure("M", base).max(0.1);
    let fit = if cols > 0 { MONO_MAX * w / (adv * cols as f32) } else { MONO_MAX };
    let size = (fit * 0.995).clamp(MONO_MIN * crate::draw::text_scale(), MONO_MAX * crate::draw::text_scale());
    Text { size, ..base }
}

/// A character of the screen as the app's mono face can draw it, one column wide whatever it is:
/// box drawing and the TUI's marks become their nearest ASCII, so the columns stay in line.
fn drawable(app: &mut App, c: char) -> char {
    if c.is_ascii() || c == ' ' {
        return c;
    }
    // Whether the mono face has it, asked once per character: a glyph it lacks measures nothing.
    let has = match app.st.terminal.faces.get(&c) {
        Some(h) => *h,
        None => {
            let h = app.canvas.measure(&c.to_string(), Text::new(MONO_MAX, t::INK).face(Weight::Mono)) > 0.0;
            app.st.terminal.faces.insert(c, h);
            h
        }
    };
    if has {
        return c;
    }
    match c {
        '│' | '┃' | '║' | '╎' | '┆' => '|',
        '─' | '━' | '═' | '╌' | '┄' => '-',
        '╭' | '╮' | '╰' | '╯' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '╔' | '╗' | '╚' | '╝' => '+',
        '❯' | '›' | '▶' | '►' | '⏵' => '>',
        '✔' | '✓' | '☑' => '*',
        '✗' | '✘' | '☐' => 'x',
        '●' | '•' | '◉' | '○' | '◯' | '⏺' => 'o',
        '…' => '.',
        _ => '?',
    }
}

/// A screen row cut into the lines that fit `per` characters (a wide screen on a narrow phone).
fn pieces(row: &str, per: usize) -> Vec<String> {
    let chars: Vec<char> = row.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(per.max(1)).map(|c| c.iter().collect()).collect()
}

pub fn draw(app: &mut App, name: &str, dt: f32) -> bool {
    if !app.shot {
        keep_up(app, name);
    }
    let foot_h = crate::keys::STRIP_H + 8.0;
    let page = app.page_begin(dt, TOP_H, foot_h);
    let x = t::PAD;
    let w = WIDTH - 2.0 * t::PAD;
    let mut y = page.top + 4.0;
    // Whose terminal, in the page (the bar above holds only Back).
    let title = app.agent(name).map_or(name.to_string(), |a| a.title().to_string());
    app.canvas.text_fit(&title, x, y, w, Text::new(ty::SECTION, t::INK).semi());
    y += 24.0;
    app.canvas.text_fit(&format!("{name}'s terminal · the keys below go to it"), x, y, w, Text::new(ty::CAPTION, t::INK3));
    y += 30.0;

    let st = &app.st.terminal;
    let (text, cols, error) = (st.text.clone(), st.cols, st.error.clone());
    match (&text, &error) {
        (_, Some(e)) => {
            let (h, _) = kit::notice(&mut crate::cx!(app), x, y + 60.0, w, e, None, None);
            y += 60.0 + h;
        }
        (None, None) => {
            let words = if app.online() { "Reading its terminal…" } else { "Your PC is out of reach. Its terminal shows once it is back." };
            app.canvas.para(words, x, y + 60.0, w, Text::new(ty::UI, t::INK3).center().line(1.45), 3);
            y += 120.0;
        }
        (Some(text), None) => {
            // Sized to the longest row really drawn (the PC trims each row's trailing spaces),
            // not the terminal's whole width.
            let shown: Vec<String> = text.split('\n').map(|r| r.chars().map(|c| drawable(app, c)).collect()).collect();
            let used = shown.iter().map(|r| r.chars().count()).max().unwrap_or(0).min(cols.max(1) as usize).max(1) as u16;
            let style = mono(app, used, w - 20.0);
            let adv = app.canvas.measure("M", style).max(0.1);
            let per = ((w - 20.0) / adv).floor() as usize;
            let lh = style.lh();
            let rows: Vec<String> = shown.iter().flat_map(|r| pieces(r, per)).collect();
            let h = rows.len() as f32 * lh + 20.0;
            // The screen on the code ground, every row as drawn; only what is in view is set.
            app.canvas.rect(x - 6.0, y, w + 12.0, h, 12.0, t::CODE);
            let off = -app.canvas.dy;
            for (i, r) in rows.iter().enumerate() {
                let ry = y + 10.0 + i as f32 * lh;
                if ry + lh >= page.top + off && ry <= page.bottom + off && !r.is_empty() {
                    app.canvas.text(r, x + 4.0, ry, style);
                }
            }
            y += h;
        }
    }
    // Kept at the bottom, where a TUI's menu and prompt are, until the person scrolls up.
    let max = y + 16.0 - page.bottom;
    let stuck = !app.st.terminal.unstuck;
    let sc = app.scroll();
    if stuck && !page.moving {
        sc.off = max.max(0.0);
    }
    let off = sc.off;
    app.st.terminal.unstuck = off < max - 24.0;
    app.page_end(&page, y);

    // The keys at the foot, over the page.
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - foot_h;
    let band = [-1000.0, top, WIDTH + 2000.0, foot_h + app.canvas.inset_bottom + 2.0];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(band[0], band[1], band[2], band[3], 0.0, t::BG);
    crate::keys::row(app, name, 12.0, top + 6.0, WIDTH - 24.0);

    app.top_band(TOP_H);
    let ty_ = app.canvas.inset_top + 6.0;
    let (back, _, _) = kit::top_bar(&mut crate::cx!(app), ty_, "", &[]);
    if back {
        app.pop();
    }
    page.moving
}

/// When the screen is read next (ns): tests.
#[cfg(test)]
pub fn next_read(app: &App) -> u64 {
    app.st.terminal.at
}

/// Screenshots: a terminal showing Claude Code's model menu.
pub fn sample(app: &mut App, name: &str) {
    app.st.terminal = State {
        name: name.to_string(),
        text: Some(
            [
                "╭───────────────────────────────────────────────────────────╮",
                "│ Select model                                              │",
                "│ Switch between Claude models. Applies to this session.    │",
                "│                                                           │",
                "│   1. Default (recommended)  Opus 5.5 for complex tasks    │",
                "│ ❯ 2. Opus                   Opus 5.5                    ✔ │",
                "│   3. Sonnet                 Sonnet 5.5 for daily coding   │",
                "│   4. Haiku                  Haiku 5.5 for simple tasks    │",
                "│                                                           │",
                "╰───────────────────────────────────────────────────────────╯",
                "   Enter to confirm · Esc to exit",
            ]
            .join("\n"),
        ),
        cols: 64,
        ..State::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_row_is_cut_into_lines_that_fit() {
        assert_eq!(pieces("abcdefg", 3), ["abc", "def", "g"]);
        assert_eq!(pieces("", 3), [""]);
        assert_eq!(pieces("│ ❯ 2", 10), ["│ ❯ 2"]);
    }
}
