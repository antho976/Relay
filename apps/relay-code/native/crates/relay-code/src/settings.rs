//! Settings (backlog D6): the PC this phone is paired with, its name and where the link stands in
//! words; which notifications come; the app's version; Forget this PC, which asks first. No
//! Connect and no Disconnect: the link is always wanted.

use rc_link::{LinkState, RouteKind};

use crate::app::App;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

#[derive(Default)]
pub struct State {
    confirm: bool,
}

/// What may post a notification while the app is away (each on unless switched off here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notify {
    /// An agent asks you something.
    Asks,
    /// An agent finished its turn and waits at its prompt.
    Done,
    /// A command is held for you to allow.
    Holds,
}

pub const NOTIFY: [(Notify, &str); 3] =
    [(Notify::Asks, "When an agent asks you"), (Notify::Done, "When an agent finishes"), (Notify::Holds, "When a command is held for you")];

/// Where the link stands, in words a person reads, and its dot.
pub fn link_words(s: &LinkState) -> (crate::draw::Rgba, String) {
    match s {
        LinkState::Online { route, .. } => (
            t::OK,
            match route.kind {
                RouteKind::Lan => "Connected over your home network",
                RouteKind::Tailscale => "Connected over Tailscale",
                RouteKind::Rendezvous => "Connected through your server",
                RouteKind::Other => "Connected",
            }
            .to_string(),
        ),
        LinkState::Starting { .. } => (t::WARN, "Starting Relay on the PC".into()),
        LinkState::Connecting { .. } => (t::WARN, "Connecting…".into()),
        LinkState::Offline { .. } => (t::INK3, "Out of reach. It tries again by itself.".into()),
        LinkState::Revoked { .. } => (t::DANGER, "It forgot this phone. Pair again to use it.".into()),
        LinkState::Unpaired => (t::INK3, "No PC paired".into()),
    }
}

fn label(app: &mut App, y: f32, words: &str) -> f32 {
    app.canvas.text(words, t::PAD, y + 8.0, Text::new(ty::SMALL, t::INK3).semi());
    30.0
}

/// A switch: a short track with its knob at the end it is set to, the accent when on, so on
/// reads at a glance by colour and by where the knob sits. Drawn only; the row takes the tap.
pub fn switch(c: &mut crate::draw::Canvas, right: f32, cy: f32, on: bool) {
    let (w, h) = (40.0, 24.0);
    let x = right - w;
    c.rect(x, cy - h / 2.0, w, h, h / 2.0, if on { t::ACCENT } else { t::S3 });
    let kx = if on { x + w - h / 2.0 } else { x + h / 2.0 };
    c.circle(kx, cy, h / 2.0 - 3.0, if on { t::ON_ACCENT } else { t::INK3 });
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let page = app.page_begin(dt, 56.0, 0.0);
    let profile = app.link.profile();
    let mut y = page.top;
    // A settings page's large title, in the page: the bar above holds only Back (house style).
    app.canvas.text("Settings", t::PAD, y + 4.0, Text::new(ty::TITLE, t::INK).semi());
    y += 52.0;

    // The PC: its name large, the link in words under it.
    y += label(app, y, "YOUR PC");
    let mut name = profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    if let (true, LinkState::Online { host, .. } | LinkState::Starting { host }) = (name.trim().is_empty(), &app.link_state) {
        name = host.clone();
    }
    let name = if name.trim().is_empty() { "Your PC".to_string() } else { name };
    app.canvas.text_fit(&name, t::PAD, y + 4.0, WIDTH - 2.0 * t::PAD, Text::new(ty::TITLE - 2.0, t::INK).semi());
    y += 36.0;
    let (dot, words) = link_words(&app.link_state);
    app.canvas.circle(t::PAD + 4.0, y + 10.0, 4.0, dot);
    app.canvas.text_fit(&words, t::PAD + 16.0, y + 1.0, WIDTH - 2.0 * t::PAD - 16.0, Text::new(ty::UI, t::INK2));
    y += 24.0;
    if let Some(p) = &profile
        && !p.version.is_empty()
    {
        let v = format!("Relay {} on the PC", p.version);
        app.canvas.text_fit(&v, t::PAD + 16.0, y + 1.0, WIDTH - 2.0 * t::PAD - 16.0, Text::new(ty::CAPTION, t::INK3));
        y += 22.0;
    }
    y += 22.0;

    // Notifications: one row each, the whole row its switch.
    y += label(app, y, "NOTIFICATIONS");
    for (kind, words) in NOTIFY {
        let on = app.notifies(kind);
        let h = 52.0;
        let mut cx = crate::cx!(app);
        if cx.pressed(0.0, y, WIDTH, h) {
            cx.c.rect(8.0, y, WIDTH - 16.0, h, 10.0, t::WASH);
        }
        cx.c.text_fit(words, t::PAD, y + (h - 20.0) / 2.0, WIDTH - 2.0 * t::PAD - 56.0, Text::new(ty::BODY, t::INK));
        switch(cx.c, WIDTH - t::PAD, y + h / 2.0, on);
        if cx.tap(0.0, y, WIDTH, h) {
            app.host.haptic();
            app.set_notifies(kind, !on);
            // Android asks once for the right to post: when one is switched on.
            if !on && !app.shot {
                app.host.ask_notifications();
            }
        }
        y += h;
    }
    y += 22.0;

    // The app, and forgetting the PC.
    y += label(app, y, "THIS APP");
    let st = Text::new(ty::BODY, t::INK);
    app.canvas.text("Relay Code", t::PAD, y + 14.0, st);
    app.canvas.text(env!("CARGO_PKG_VERSION"), WIDTH - t::PAD, y + 14.0, Text::new(ty::UI, t::INK2).right());
    y += 52.0 + 12.0;
    let forget = profile.is_some() || !matches!(app.link_state, LinkState::Unpaired);
    if forget {
        let (x, w, h) = (t::PAD, WIDTH - 2.0 * t::PAD, 46.0);
        let mut cx = crate::cx!(app);
        let pressed = cx.pressed(x, y, w, h);
        cx.c.rect(x, y, w, h, t::KEY_R, if pressed { t::S3 } else { t::S2 });
        let tt = Text::new(ty::BODY, t::DANGER).semi().center();
        cx.c.text("Forget this PC", x + w / 2.0, y + (h - tt.lh()) / 2.0, tt);
        if cx.tap(x, y, w, h) {
            app.st.settings.confirm = true;
        }
        y += h;
    }
    app.page_end(&page, y + 24.0);

    app.top_band(56.0);
    let ty_ = app.canvas.inset_top + 6.0;
    let mut cx = crate::cx!(app);
    let (back, _, _) = kit::top_bar(&mut cx, ty_, "", &[]);
    if back {
        app.pop();
        return false;
    }
    if app.st.settings.confirm {
        let title = format!("Forget {name}?");
        let mut cx = crate::cx!(app);
        match kit::confirm(&mut cx, &title, "This phone stops connecting to it. To use it again, pair again with a new link from the PC.", "Forget") {
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

#[cfg(test)]
pub fn confirming(app: &App) -> bool {
    app.st.settings.confirm
}
