//! Home: the repo's agents, when no conversation is open (the first run after pairing, or after
//! Back from a chat). What needs you first, then the newest. The drawer holds the same list; this
//! is where it lands when there is nothing to reopen.

use crate::app::App;
use crate::data::Agent;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

pub const TOP_H: f32 = 56.0;

/// The fixed top every page under the drawer shares: the menu key, a title, the repo chip.
pub fn top(app: &mut App, title: &str) {
    app.top_band(TOP_H);
    let y = app.canvas.inset_top + 6.0;
    let mut cx = crate::cx!(app);
    if kit::icon_key(&mut cx, 6.0, y, "menu") {
        app.st.drawer.open = true;
    }
    let cw = crate::drawer::repo_chip(app, WIDTH - 12.0, y + 5.0);
    app.canvas.text_fit(title, 56.0, y + (44.0 - 22.0) / 2.0, WIDTH - 56.0 - cw - 24.0, Text::new(ty::SECTION + 1.0, t::INK).semi());
}

/// The foot's height: a box to start an agent, as Claude's app opens on one.
const FOOT_H: f32 = 72.0;

pub fn draw(app: &mut App, dt: f32) -> bool {
    let page = app.page_begin(dt, TOP_H, FOOT_H);
    let agents: Vec<Agent> = app.repo_agents().into_iter().cloned().collect();
    let now_ms = rc_link::link::now_ms();
    let mut y = page.top + 4.0;
    let mut open = None;
    if agents.is_empty() {
        let (title, body) = if app.link_state.online() {
            ("No agents in this repo", "Start one from the box below, or switch repo with the chip above.")
        } else if app.projects.is_empty() {
            ("Waiting for your PC", "Your agents show here as soon as the PC answers. Nothing to tap: it connects by itself.")
        } else {
            ("No agents in this repo yet", "Start one from the box below.")
        };
        let (h, _) = kit::notice(&mut crate::cx!(app), 32.0, y + 120.0, WIDTH - 64.0, title, Some(body), None);
        y += 120.0 + h;
    }
    let (needs, rest): (Vec<&Agent>, Vec<&Agent>) = agents.iter().partition(|a| a.needs_you());
    for (label, list) in [("NEEDS YOU", needs), ("AGENTS", rest)] {
        if list.is_empty() {
            continue;
        }
        app.canvas.text(label, t::PAD, y + 12.0, Text::new(ty::SMALL, t::INK3).semi());
        y += 34.0;
        for a in list {
            if crate::drawer::agent_row(app, a, 0.0, y, WIDTH, now_ms) {
                open = Some(a.name.clone());
            }
            y += crate::drawer::AGENT_ROW_H;
        }
        y += 8.0;
    }
    // What stopped or finished, newest first; each opens its conversation, with Resume (D1).
    let earlier = crate::agents::earlier_section(app, 0.0, &mut y, WIDTH, None);
    open = open.or(earlier);
    app.page_end(&page, y);
    top(app, "Agents");
    start_box(app);
    if let Some(name) = open {
        app.open(&name);
    }
    page.moving
}

/// A box at the foot, worded as what it does: a tap opens New agent with the keyboard up.
fn start_box(app: &mut App) {
    let bottom = app.canvas.h - app.canvas.inset_bottom;
    let top = bottom - FOOT_H;
    let band = [-1000.0, top, WIDTH + 2000.0, FOOT_H + app.canvas.inset_bottom + 2.0];
    kit::above(&mut crate::cx!(app), crate::ui::layer::CHROME, band);
    app.canvas.rect(band[0], band[1], band[2], band[3], 0.0, t::BG);
    let (x, y, w, h) = (12.0, top + 10.0, WIDTH - 24.0, 50.0);
    let repo = app.repo_name();
    let mut cx = crate::cx!(app);
    let pressed = cx.pressed(x, y, w, h);
    cx.c.rect(x, y, w, h, 22.0, if pressed { t::S2 } else { t::S1 });
    cx.c.outline(x, y, w, h, 22.0, 1.0, t::LINE);
    let st = Text::new(ty::BODY, t::INK3);
    cx.c.text_fit(&format!("Start an agent in {repo}"), x + 18.0, y + (h - st.lh()) / 2.0, w - 70.0, st);
    cx.c.circle(x + w - 25.0, y + h / 2.0, 17.0, t::S2);
    cx.c.icon("plus", x + w - 25.0, y + h / 2.0, 15.0, t::INK2);
    if cx.tap(x, y, w, h) && app.repo.is_some() {
        app.st.newagent.open = true;
        let task = app.st.newagent.task.clone();
        app.focus.id = Some("new-agent");
        app.focus.text = task.clone();
        app.focus.kind = kit::Kb::Prose;
        app.host.keyboard(true, &task, kit::Kb::Prose);
    }
}
