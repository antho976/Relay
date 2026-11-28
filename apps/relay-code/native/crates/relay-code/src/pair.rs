//! Pairing: the first screen, and the screen a PC that forgot this phone sends it back to. Through
//! the owner's server (Allow on Discord, then on the PC: no code to type), or with a pairing link
//! from `relay remote pair`. Once it pairs, the app goes straight to the agents and the link
//! connects by itself, now and on every launch after (the owner: "I hate having to click connect").

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use rc_link::server::{Asked, Reply, Server};
use rc_link::{LinkState, PairLink, Profile};

use crate::app::App;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Kb};
use crate::theme::{self as t, ty};

/// The owner's server, filled in by the personal build (`RELAY_CODE_SERVER` at build time), so
/// the first screen needs one tap. Blank in every other build.
pub const BUILT_IN_SERVER: &str = match option_env!("RELAY_CODE_SERVER") {
    Some(s) => s,
    None => "",
};

const DEVICE: &str = "Relay Code";

enum Msg {
    Asked(Asked),
    Allowed,
    Paired(Box<Profile>),
    Failed(String),
}

#[derive(Default)]
pub struct State {
    pub server: Option<String>,
    pub link: String,
    /// The four characters the phone and the Discord DM both show, while waiting.
    code: Option<String>,
    allowed: bool,
    busy: bool,
    pub error: Option<String>,
    rx: Option<Receiver<Msg>>,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

fn start_server(app: &mut App, server: String) {
    let (tx, rx) = channel();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let st = &mut app.st.pair;
    st.rx = Some(rx);
    st.busy = true;
    st.error = None;
    st.code = None;
    st.allowed = false;
    st.cancel = Some(stop.clone());
    let _ = std::thread::Builder::new().name("rc-pair".into()).spawn(move || {
        let send = |m: Msg| {
            let _ = tx.send(m);
            crate::wake::wake();
        };
        let s = Server::new(&server);
        let asked = match s.ask(DEVICE) {
            Ok(a) => a,
            Err(e) => return send(Msg::Failed(e)),
        };
        send(Msg::Asked(asked.clone()));
        // Ten minutes for the owner to answer, asked every two seconds.
        for _ in 0..300 {
            if stop.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_secs(2));
            match s.claim(&asked) {
                Reply::Waiting => {}
                Reply::WaitingPc => send(Msg::Allowed),
                Reply::Ready(link) => {
                    let Some(pl) = PairLink::parse(&link) else { return send(Msg::Failed("Your server's link did not read".into())) };
                    send(Msg::Allowed);
                    return send(match rc_link::link::pair(&pl, DEVICE) {
                        Ok(p) => Msg::Paired(Box::new(p)),
                        Err(r) => Msg::Failed(r.text()),
                    });
                }
                Reply::Denied => return send(Msg::Failed("Declined on Discord.".into())),
                Reply::Expired => return send(Msg::Failed("Nobody answered in time. Ask again.".into())),
                Reply::Spent => return send(Msg::Failed("That request was used already. Ask again.".into())),
                Reply::Trouble(e) => return send(Msg::Failed(e)),
            }
        }
        send(Msg::Failed("Nobody answered in time. Ask again.".into()));
    });
}

fn start_link(app: &mut App, pl: PairLink) {
    let (tx, rx) = channel();
    let st = &mut app.st.pair;
    st.rx = Some(rx);
    st.busy = true;
    st.error = None;
    let _ = std::thread::Builder::new().name("rc-pair".into()).spawn(move || {
        let m = match rc_link::link::pair(&pl, DEVICE) {
            Ok(p) => Msg::Paired(Box::new(p)),
            Err(r) => Msg::Failed(r.text()),
        };
        let _ = tx.send(m);
        crate::wake::wake();
    });
}

fn take(app: &mut App) {
    let Some(rx) = &app.st.pair.rx else { return };
    let mut paired = None;
    while let Ok(m) = rx.try_recv() {
        match m {
            Msg::Asked(a) => app.st.pair.code = Some(a.code),
            Msg::Allowed => app.st.pair.allowed = true,
            Msg::Paired(p) => paired = Some(p),
            Msg::Failed(e) => {
                app.st.pair.busy = false;
                app.st.pair.code = None;
                app.st.pair.error = Some(e);
            }
        }
    }
    if let Some(p) = paired {
        app.st.pair = State::default();
        app.paired(&p);
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    take(app);
    if app.st.pair.server.is_none() {
        app.st.pair.server = Some(BUILT_IN_SERVER.to_string());
    }
    let page = app.page_begin(dt, 0.0, 0.0);
    let x = 24.0;
    let w = WIDTH - 2.0 * x;
    let mut y = page.top + 48.0;
    app.canvas.icon("claude", x + 14.0, y + 14.0, 28.0, t::ACCENT);
    y += 48.0;
    app.canvas.text("Pair with your PC", x, y, Text::new(ty::TITLE, t::INK).semi());
    y += 36.0;
    let lead = match &app.link_state {
        LinkState::Revoked { reason } => reason.clone(),
        _ => "Once, here. After that this phone connects to your PC by itself, every time.".into(),
    };
    y += app.canvas.para(&lead, x, y, w, Text::new(ty::UI, t::INK2).line(1.45), 3) + 28.0;

    if let Some(code) = app.st.pair.code.clone() {
        // Waiting on the owner: the code the DM shows too.
        let tt = Text::new(40.0, t::INK).semi().center();
        app.canvas.text(&code, WIDTH / 2.0, y, tt);
        y += 60.0;
        let words = if app.st.pair.allowed { "Allowed. Pairing with the PC…" } else { "Allow it on Discord. The DM shows the same four letters." };
        y += app.canvas.para(words, WIDTH / 2.0, y, w, Text::new(ty::UI, t::INK2).center().line(1.45), 3) + 24.0;
        let mut cx = crate::cx!(app);
        if kit::plain(&mut cx, x, y, w, 46.0, "Cancel", None) {
            if let Some(c) = &app.st.pair.cancel {
                c.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            app.st.pair = State::default();
        }
        y += 46.0;
    } else {
        let mut server = app.st.pair.server.clone().unwrap_or_default();
        y += kit::field(&mut crate::cx!(app), "pair-server", x, y, w, "Your server", "example.org", &mut server, Kb::Url) + 14.0;
        app.st.pair.server = Some(server.clone());
        let busy = app.st.pair.busy;
        let mut cx = crate::cx!(app);
        let go = if server.trim().is_empty() || busy {
            kit::plain(&mut cx, x, y, w, 48.0, if busy { "Asking…" } else { "Pair through my server" }, None) && false
        } else {
            kit::primary(&mut cx, x, y, w, 48.0, "Pair through my server", None)
        };
        if go {
            app.finish_typing();
            start_server(app, server.trim().to_string());
        }
        y += 48.0 + 36.0;
        app.canvas.text("Or with a pairing link", x, y, Text::new(ty::CAPTION, t::INK3).medium());
        y += 26.0;
        let mut link = app.st.pair.link.clone();
        y += kit::field(&mut crate::cx!(app), "pair-link", x, y, w, "From relay remote pair on the PC", "relay://pair?…", &mut link, Kb::Url) + 14.0;
        app.st.pair.link = link.clone();
        let parsed = PairLink::parse(&link);
        let mut cx = crate::cx!(app);
        if kit::plain(&mut cx, x, y, w, 46.0, "Pair with this link", None) {
            match parsed {
                Some(pl) if !busy => {
                    app.finish_typing();
                    start_link(app, pl);
                }
                Some(_) => {}
                None => app.st.pair.error = Some("That does not read as a pairing link (relay://pair?…).".into()),
            }
        }
        y += 46.0;
    }
    if let Some(e) = app.st.pair.error.clone() {
        y += 18.0;
        y += app.canvas.para(&e, x, y, w, Text::new(ty::UI, t::DANGER).line(1.45), 4);
    }
    app.page_end(&page, y + 24.0);
    page.moving || app.st.pair.busy
}
