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
/// The screen's text: 11 unless A− / A+ changed it, and within these.
const MONO_DEFAULT: f32 = 11.0;
const MONO_LEAST: f32 = 9.0;
const MONO_MOST: f32 = 16.0;

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
    /// Sideways, for a screen wider than the page; and the screen it was last placed for.
    side: crate::ui::HScroll,
    marked_for: Option<String>,
    /// The screen's text size (A− / A+); 0 for the default. Kept across visits.
    pub size: f32,
}

/// Read the screen when it is time, and take the answer.
fn keep_up(app: &mut App, name: &str) {
    if app.st.terminal.name != name {
        let size = app.st.terminal.size;
        app.st.terminal = State { name: name.to_string(), size, ..State::default() };
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
            Err(e) => app.st.terminal.error = Some(crate::errors::words(&e)),
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

/// The screen's text style: mono at the size chosen here (A− / A+), at the app's text size.
fn mono(app: &App) -> Text {
    let size = if app.st.terminal.size > 0.0 { app.st.terminal.size } else { MONO_DEFAULT };
    Text::new(size, t::INK).face(Weight::Mono).line(1.3)
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
            let h = app.canvas.measure(&c.to_string(), Text::new(MONO_DEFAULT, t::INK).face(Weight::Mono)) > 0.0;
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


/// The column of the row a TUI marks as chosen (`❯`, `>` at a row's start) or, failing that, of
/// the prompt: kept in view when a wide screen scrolls sideways.
fn marked(rows: &[String]) -> Option<usize> {
    rows.iter().rev().find_map(|r| {
        let lead = r.chars().take_while(|c| matches!(c, ' ' | '|' | '+')).count();
        let c = r.chars().nth(lead)?;
        (c == '>').then_some(lead)
    })
}

pub fn draw(app: &mut App, name: &str, dt: f32) -> bool {
    if !app.shot {
        keep_up(app, name);
    }
    let foot_h = crate::keys::STRIP_H + 8.0;
    let page = app.page_begin(dt, TOP_H, foot_h);
    let x = t::PAD;
    let w = WIDTH - 2.0 * t::PAD;
    let mut y = page.top + 2.0;
    // Whose terminal, in the page (the bar above holds only Back and the text size).
    let title = app.agent(name).map_or(name.to_string(), |a| a.title().to_string());
    app.canvas.text_fit(&title, x, y, w, Text::new(ty::BODY, t::INK).semi());
    y += 22.0;
    // Away from the PC the screen is the last one read, and says so.
    let stale = app.st.terminal.text.is_some();
    match crate::home::reach(app).filter(|_| stale) {
        Some((dot, words)) => {
            app.canvas.circle(x + 3.0, y + 8.5, 3.0, dot);
            app.canvas.text_fit(&format!("{words} · the screen as it was"), x + 11.0, y, w - 11.0, Text::new(ty::CAPTION, t::INK3));
        }
        None => {
            app.canvas.text_fit(&format!("{name}'s terminal · the keys below go to it"), x, y, w, Text::new(ty::CAPTION, t::INK3));
        }
    }
    y += 26.0;

    let st = &app.st.terminal;
    let (text, error) = (st.text.clone(), st.error.clone());
    let mut moving = page.moving;
    match (&text, &error) {
        (_, Some(e)) => {
            let can = app.online();
            let (h, retry) = kit::problem(&mut crate::cx!(app), y, "Its terminal didn't read", e, can);
            if retry {
                app.st.terminal.error = None;
                app.st.terminal.at = 0;
            }
            y += h;
        }
        (None, None) => {
            let (h, _) = if app.online() {
                kit::empty(&mut crate::cx!(app), y, "Reading its terminal…", None, None)
            } else {
                kit::empty(&mut crate::cx!(app), y, "Waiting for your PC", Some("Its terminal shows once the PC answers."), None)
            };
            y += h;
        }
        (Some(text), None) => {
            let style = mono(app);
            let adv = app.canvas.measure("M", style).max(0.1);
            let lh = style.lh();
            let rows: Vec<String> = text.split('\n').map(|r| r.chars().map(|c| drawable(app, c)).collect()).collect();
            let longest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as f32;
            // The block fills the page down to the keys, the newest rows at its foot, as a
            // terminal shows them; a longer screen scrolls up.
            let (bx, bw) = (x - 6.0, w + 12.0);
            let inner = rows.len() as f32 * lh + 20.0;
            // Exactly to the page's end when the screen is short, so the page does not scroll.
            let h = inner.max(page.bottom - 16.0 - y);
            app.canvas.rect(bx, y, bw, h, 12.0, t::CODE);
            // Wider than the page: it scrolls sideways, keeping the chosen row's start in view
            // whenever the screen changes.
            let text_w = bw - 20.0;
            let wide = longest * adv > text_w;
            let mut off = 0.0;
            if wide {
                let area = [bx, y + app.canvas.dy, bw, h];
                app.ui.claim_h(area);
                let ts = &mut app.st.terminal;
                ts.side.max = longest * adv - text_w;
                if ts.marked_for.as_deref() != Some(text.as_str()) {
                    ts.marked_for = Some(text.clone());
                    if let Some(col) = marked(&rows) {
                        let cx = col as f32 * adv;
                        if cx < ts.side.off || cx > ts.side.off + text_w - 6.0 * adv {
                            ts.side.off = (cx - 2.0 * adv).clamp(0.0, ts.side.max.max(0.0));
                        }
                    }
                }
                moving |= ts.side.update(&app.ui, area, dt);
                off = app.st.terminal.side.off;
                app.canvas.push_clip(bx, y + app.canvas.dy, bw, h);
            } else {
                app.st.terminal.side = Default::default();
            }
            let top = y + h - inner + 10.0;
            let view = -app.canvas.dy;
            for (i, r) in rows.iter().enumerate() {
                let ry = top + i as f32 * lh;
                if ry + lh >= page.top + view && ry <= page.bottom + view && !r.is_empty() {
                    app.canvas.text(r, bx + 10.0 - off, ry, style);
                }
            }
            if wide {
                app.canvas.pop_clip();
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
    let (back, key, _) = kit::top_bar(&mut crate::cx!(app), ty_, "", &["minus", "plus"]);
    if back {
        app.pop();
    }
    // A− / A+: the screen's text size, one step at a time, kept for the next visit.
    if let Some(k) = key {
        let now = mono(app).size;
        let step = if k == 0 { -1.0 } else { 1.0 };
        let next = (now + step).clamp(MONO_LEAST * crate::draw::text_scale(), MONO_MOST * crate::draw::text_scale());
        app.st.terminal.size = next;
        app.st.terminal.marked_for = None;
        app.host.haptic();
    }
    moving
}

/// How far a wide screen is scrolled sideways, and the most it may go: tests.
#[cfg(test)]
pub fn sideways(app: &App) -> (f32, f32) {
    (app.st.terminal.side.off, app.st.terminal.side.max)
}

/// When the screen is read next (ns): tests.
#[cfg(test)]
pub fn next_read(app: &App) -> u64 {
    app.st.terminal.at
}

/// Screenshots: a 100-column terminal showing Claude Code's model menu.
pub fn sample(app: &mut App, name: &str) {
    let w = 98;
    let line = |s: &str| format!("│ {s:<w$} │", w = w - 4);
    let rows = [
        format!("╭{}╮", "─".repeat(w - 2)),
        line("Select model"),
        line("Switch between Claude models. Applies to this session and future Claude Code sessions."),
        line(""),
        line("  1. Default (recommended)  Opus 5.5 for complex tasks · Sonnet 5.5 when the limit is near"),
        line("❯ 2. Opus                   Opus 5.5 for complex tasks                                  ✔"),
        line("  3. Sonnet                 Sonnet 5.5 for daily coding"),
        line("  4. Haiku                  Haiku 5.5 for simple tasks"),
        line(""),
        format!("╰{}╯", "─".repeat(w - 2)),
        "   Enter to confirm · Esc to exit".to_string(),
    ];
    let size = app.st.terminal.size;
    app.st.terminal = State { name: name.to_string(), text: Some(rows.join("\n")), cols: w as u16, size, ..State::default() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_row_is_found_after_its_border() {
        let rows: Vec<String> = ["+------+", "|   1. Default", "| > 2. Opus", "|   3. Sonnet"].map(String::from).to_vec();
        assert_eq!(marked(&rows), Some(2));
        let prompt: Vec<String> = ["Done.", "> "].map(String::from).to_vec();
        assert_eq!(marked(&prompt), Some(0));
        assert_eq!(marked(&["plain".to_string()]), None);
    }
}
