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
    let tw = WIDTH - 56.0 - cw - 24.0;
    let tt = Text::new(ty::SECTION + 1.0, t::INK).semi();
    match reach(app) {
        // Away from the PC: the title and, under it, that what shows is what was kept (D14).
        Some((dot, words)) => {
            let ct = Text::new(ty::SMALL, t::INK3);
            let ty0 = y + (44.0 - (tt.lh() + ct.lh())) / 2.0;
            app.canvas.text_fit(title, 56.0, ty0, tw, tt);
            app.canvas.circle(56.0 + 3.0, ty0 + tt.lh() + ct.lh() / 2.0, 3.0, dot);
            app.canvas.text_fit(&words, 56.0 + 11.0, ty0 + tt.lh(), tw - 11.0, ct);
        }
        None => {
            app.canvas.text_fit(title, 56.0, y + (44.0 - tt.lh()) / 2.0, tw, tt);
        }
    }
}

/// While the PC is not answering: its dot and a few words, for under a page's title. None while
/// it answers (and while pairing, which says so itself).
pub fn reach(app: &App) -> Option<(crate::draw::Rgba, String)> {
    use rc_link::LinkState;
    let kept = !app.agents.is_empty() || !app.projects.is_empty();
    let name = app.link.pc_name();
    let pc = if name.is_empty() { "your PC" } else { name.as_str() };
    match &app.link_state {
        LinkState::Online { .. } | LinkState::Unpaired | LinkState::Revoked { .. } => None,
        LinkState::Starting { .. } => Some((t::WARN, format!("Starting Relay on {pc}…"))),
        LinkState::Connecting { .. } if !kept => Some((t::WARN, "Connecting…".into())),
        LinkState::Connecting { .. } => Some((t::INK3, format!("Reconnecting to {pc}…"))),
        // It dials again by itself: when, to the second (the app draws once a second for it).
        LinkState::Offline { retry_at_ms, .. } => Some((t::INK3, match retry_at_ms.saturating_sub(rc_link::link::now_ms()).div_ceil(1000) {
            0 => "Out of reach · trying now".into(),
            left => format!("Out of reach · retry in {left} s"),
        })),
    }
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
        let (h, _) = kit::empty(&mut crate::cx!(app), y, title, Some(body), None);
        y += h;
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
            y += crate::drawer::agent_row_h();
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
    // Without a repo yet (the PC has not answered) it says what it waits on, and takes no tap.
    let ready = app.repo.is_some();
    let words = if ready { format!("Start an agent in {repo}") } else { "Agents start here once your PC answers".to_string() };
    let mut cx = crate::cx!(app);
    let pressed = ready && cx.pressed(x, y, w, h);
    cx.c.rect(x, y, w, h, 22.0, if pressed { t::S2 } else { t::S1 });
    cx.c.outline(x, y, w, h, 22.0, 1.0, t::LINE);
    let st = Text::new(ty::BODY, t::INK3);
    cx.c.text_fit(&words, x + 18.0, y + (h - st.lh()) / 2.0, w - 70.0, st);
    if ready {
        cx.c.circle(x + w - 25.0, y + h / 2.0, 17.0, t::S2);
        cx.c.icon("plus", x + w - 25.0, y + h / 2.0, 15.0, t::INK2);
    }
    if cx.tap(x, y, w, h) && ready {
        app.st.newagent.open = true;
        let task = app.st.newagent.task.clone();
        app.focus.id = Some("new-agent");
        app.focus.text = task.clone();
        app.focus.kind = kit::Kb::Prose;
        app.host.keyboard(true, &task, kit::Kb::Prose);
    }
}
