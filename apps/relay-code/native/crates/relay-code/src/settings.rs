//! Settings: the PC this phone is paired with and how the link reaches it. No Connect and no
//! Disconnect (the link is always wanted); Unpair is the one act, and it asks first.

use rc_link::{LinkState, RouteKind};

use crate::app::App;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

#[derive(Default)]
pub struct State {
    confirm: bool,
}

fn row(app: &mut App, y: f32, label: &str, value: &str) -> f32 {
    app.canvas.text(label, t::PAD, y + 14.0, Text::new(ty::UI, t::INK2));
    let st = Text::new(ty::UI, t::INK).medium();
    let shown = app.canvas.fit(value, WIDTH - 130.0 - t::PAD, st);
    app.canvas.text(&shown, WIDTH - t::PAD, y + 14.0, st.right());
    48.0
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let page = app.page_begin(dt, 56.0, 0.0);
    let profile = app.link.profile();
    let mut y = page.top + 8.0;
    app.canvas.text("YOUR PC", t::PAD, y + 8.0, Text::new(ty::SMALL, t::INK3).semi());
    y += 30.0;
    // The name it paired under, else the one it answers with now.
    let mut name = profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    if let (true, LinkState::Online { host, .. } | LinkState::Starting { host }) = (name.trim().is_empty(), &app.link_state) {
        name = host.clone();
    }
    y += row(app, y, "Name", if name.is_empty() { "Unnamed PC" } else { &name });
    let state = match &app.link_state {
        LinkState::Online { .. } => "Connected".to_string(),
        LinkState::Starting { .. } => "Starting Relay".into(),
        LinkState::Connecting { .. } => "Connecting".into(),
        LinkState::Offline { reason, .. } => reason.clone(),
        LinkState::Revoked { reason } => reason.clone(),
        LinkState::Unpaired => "Not paired".into(),
    };
    y += row(app, y, "Link", &state);
    let route = match &app.link_state {
        LinkState::Online { route, .. } => match route.kind {
            RouteKind::Lan => "Home network",
            RouteKind::Tailscale => "Tailscale",
            RouteKind::Rendezvous => "Your server",
            RouteKind::Other => "Direct",
        },
        _ => "Not connected now",
    };
    y += row(app, y, "Through", route);
    if let Some(p) = &profile
        && !p.version.is_empty()
    {
        let v = p.version.clone();
        y += row(app, y, "Relay", &v);
    }
    y += 12.0;
    app.canvas.para(
        "It connects by itself whenever the app is open, and keeps connected in the background while a PC is paired.",
        t::PAD,
        y,
        WIDTH - 2.0 * t::PAD,
        Text::new(ty::CAPTION, t::INK3).line(1.45),
        3,
    );
    y += 64.0;
    let mut cx = crate::cx!(app);
    if kit::plain(&mut cx, t::PAD, y, WIDTH - 2.0 * t::PAD, 46.0, "Unpair this phone", None) {
        app.st.settings.confirm = true;
    }
    y += 46.0;
    app.page_end(&page, y + 24.0);

    app.top_band(56.0);
    let ty_ = app.canvas.inset_top + 6.0;
    let mut cx = crate::cx!(app);
    let (back, _, _) = kit::top_bar(&mut cx, ty_, "Settings", &[]);
    if back {
        app.pop();
        return false;
    }
    if app.st.settings.confirm {
        let mut cx = crate::cx!(app);
        match kit::confirm(&mut cx, "Unpair this phone?", "It forgets this PC. To use it again, pair it again from the first screen.", "Unpair") {
            Some(true) => {
                app.st.settings.confirm = false;
                app.unpair();
            }
            Some(false) => app.st.settings.confirm = false,
            None => {}
        }
    }
    page.moving
}
