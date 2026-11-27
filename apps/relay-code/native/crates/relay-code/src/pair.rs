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
    /// The pairing-link way is shown in place of the server's.
    pub by_link: bool,
    /// The built-in server is being changed: its field shows.
    pub edit_server: bool,
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
        let link = wait_for_link(|| s.claim(&asked), || std::thread::sleep(Duration::from_secs(2)), &stop, || send(Msg::Allowed));
        match link {
            Some(Ok(link)) => {
                let Some(pl) = PairLink::parse(&link) else { return send(Msg::Failed("Your server's link did not read".into())) };
                send(match rc_link::link::pair(&pl, DEVICE) {
                    Ok(p) => Msg::Paired(Box::new(p)),
                    Err(r) => Msg::Failed(r.text()),
                });
            }
            Some(Err(e)) => send(Msg::Failed(e)),
            None => {}
        }
    });
}

/// How many troubles in a row (the server out of reach, an answer that did not read) are waited
/// out before pairing gives up: a phone between networks, a server waking up.
const TROUBLES: u32 = 3;

/// Ask the server every two seconds, for ten minutes, until the owner answered: the link once
/// the PC opened its window, the words to show if not, none when stopped. `allowed` runs each
/// time the owner has allowed it and the PC is still opening.
fn wait_for_link(mut claim: impl FnMut() -> Reply, mut pause: impl FnMut(), stop: &std::sync::atomic::AtomicBool, mut allowed: impl FnMut()) -> Option<Result<String, String>> {
    let mut troubles = 0;
    for _ in 0..300 {
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        pause();
        let reply = claim();
        troubles = if matches!(reply, Reply::Trouble(_)) { troubles + 1 } else { 0 };
        match reply {
            Reply::Waiting => {}
            Reply::WaitingPc => allowed(),
            Reply::Ready(link) => {
                allowed();
                return Some(Ok(link));
            }
            Reply::Denied => return Some(Err("Declined on Discord.".into())),
            Reply::Expired => return Some(Err("Nobody answered in time. Ask again.".into())),
            Reply::Spent => return Some(Err("That request was used already. Ask again.".into())),
            Reply::Trouble(e) if troubles > TROUBLES => return Some(Err(e)),
            Reply::Trouble(_) => {}
        }
    }
    Some(Err("Nobody answered in time. Ask again.".into()))
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

/// One step of the server pairing, its mark and its words: done (a tick), now (the accent arc
/// going round), or to come (a quiet ring).
fn step(app: &mut App, x: f32, y: f32, w: f32, words: &str, done: bool, now: bool) -> f32 {
    let cy = y + 11.0;
    if done {
        app.canvas.circle(x + 10.0, cy, 10.0, t::OK.a(0.18));
        app.canvas.icon("check", x + 10.0, cy, 12.0, t::OK);
    } else if now {
        // A short arc going round: it is waiting on something, not stuck.
        let turn = (crate::clock::now_ns() % 1_200_000_000) as f32 / 1_200_000_000.0;
        app.canvas.arc(x + 10.0, cy, 8.0, 2.0, turn, turn + 0.7, t::ACCENT, false);
    } else {
        app.canvas.arc(x + 10.0, cy, 8.0, 1.5, 0.0, 1.0, t::S3, false);
    }
    let ink = if done || now { t::INK } else { t::INK3 };
    app.canvas.para(words, x + 32.0, y, w - 32.0, Text::new(ty::BODY, ink).line(1.4), 2) + 14.0
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
    let (lead, lead_ink) = match &app.link_state {
        LinkState::Revoked { reason } => (reason.clone(), t::WARN),
        _ => ("Once, here. After that this phone connects to your PC by itself, every time.".into(), t::INK2),
    };
    y += app.canvas.para(&lead, x, y, w, Text::new(ty::UI, lead_ink).line(1.45), 3) + 28.0;

    if let Some(code) = app.st.pair.code.clone() {
        // Waiting on the owner: the four letters the Discord DM shows too, and where it stands.
        app.canvas.text("Your code", x, y, Text::new(ty::CAPTION, t::INK3).medium());
        y += 22.0;
        app.canvas.text(&code, x, y, Text::new(40.0, t::INK).face(crate::draw::Weight::Mono));
        y += 64.0;
        let allowed = app.st.pair.allowed;
        y += step(app, x, y, w, "Allow it on Discord. The message shows the same four letters.", allowed, !allowed);
        y += step(app, x, y, w, "Pairing with the PC", false, allowed);
        y += 16.0;
        let mut cx = crate::cx!(app);
        if kit::plain(&mut cx, x, y, w, 46.0, "Cancel", None) {
            if let Some(c) = &app.st.pair.cancel {
                c.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            app.st.pair = State::default();
        }
        y += 46.0;
    } else if !app.st.pair.by_link {
        // Through the owner's server: the one action. The personal build knows the server, so
        // it shows as a line with Change; any other build asks for it.
        let mut server = app.st.pair.server.clone().unwrap_or_default();
        let known = !BUILT_IN_SERVER.is_empty() && !app.st.pair.edit_server && server == BUILT_IN_SERVER;
        if known {
            let st = Text::new(ty::UI, t::INK2);
            let tw = app.canvas.text(&format!("Through {server}"), x, y + 12.0, st);
            let mut cx = crate::cx!(app);
            let cxx = x + tw + 12.0;
            let ct = Text::new(ty::UI, t::ACCENT).medium();
            let cw = cx.c.measure("Change", ct);
            cx.c.text("Change", cxx, y + 12.0, ct);
            if cx.tap(cxx - 8.0, y, cw + 16.0, 44.0) {
                app.st.pair.edit_server = true;
            }
            y += 44.0 + 12.0;
        } else {
            y += kit::field(&mut crate::cx!(app), "pair-server", x, y, w, "Your server", "example.org", &mut server, Kb::Url) + 14.0;
        }
        app.st.pair.server = Some(server.clone());
        let busy = app.st.pair.busy;
        let mut cx = crate::cx!(app);
        let go = if server.trim().is_empty() || busy {
            kit::plain(&mut cx, x, y, w, 48.0, if busy { "Asking your server…" } else { "Pair through my server" }, None) && false
        } else {
            kit::primary(&mut cx, x, y, w, 48.0, "Pair through my server", None)
        };
        if go {
            app.finish_typing();
            start_server(app, server.trim().to_string());
        }
        y += 48.0 + 8.0;
        if !busy && quiet_key(app, x, y, w, "Use a pairing link instead") {
            app.finish_typing();
            app.st.pair.by_link = true;
            app.st.pair.error = None;
        }
        y += 44.0;
    } else {
        // With a pairing link from the PC.
        let mut link = app.st.pair.link.clone();
        y += kit::field(&mut crate::cx!(app), "pair-link", x, y, w, "From relay remote pair on the PC", "relay://pair?…", &mut link, Kb::Url) + 14.0;
        app.st.pair.link = link.clone();
        let parsed = PairLink::parse(&link);
        let busy = app.st.pair.busy;
        let mut cx = crate::cx!(app);
        let go = if link.trim().is_empty() || busy {
            kit::plain(&mut cx, x, y, w, 48.0, if busy { "Pairing…" } else { "Pair with this link" }, None) && false
        } else {
            kit::primary(&mut cx, x, y, w, 48.0, "Pair with this link", None)
        };
        if go {
            match parsed {
                Some(pl) => {
                    app.finish_typing();
                    start_link(app, pl);
                }
                None => app.st.pair.error = Some("That does not read as a pairing link. It starts with relay://pair?".into()),
            }
        }
        y += 48.0 + 8.0;
        if !busy && quiet_key(app, x, y, w, "Pair through my server instead") {
            app.finish_typing();
            app.st.pair.by_link = false;
            app.st.pair.error = None;
        }
        y += 44.0;
    }
    if let Some(e) = app.st.pair.error.clone() {
        y += 18.0;
        y += app.canvas.para(&e, x, y, w, Text::new(ty::UI, t::DANGER).line(1.45), 4);
    }
    app.page_end(&page, y + 24.0);
    page.moving || app.st.pair.busy
}

/// A key drawn as words only, centred: the other way to pair. True when tapped.
fn quiet_key(app: &mut App, x: f32, y: f32, w: f32, words: &str) -> bool {
    let mut cx = crate::cx!(app);
    if cx.pressed(x, y, w, 44.0) {
        cx.c.rect(x, y, w, 44.0, t::KEY_R, t::WASH);
    }
    let tt = Text::new(ty::UI, t::INK2).medium().center();
    cx.c.text(words, x + w / 2.0, y + (44.0 - tt.lh()) / 2.0, tt);
    cx.tap(x, y, w, 44.0)
}

/// Waiting on the server pairing (its code is up).
pub fn waiting(app: &App) -> bool {
    app.st.pair.code.is_some()
}

/// Screenshots: waiting on Discord's Allow, or already allowed.
pub fn sample_waiting(app: &mut App, allowed: bool) {
    app.st.pair.code = Some("K7QF".into());
    app.st.pair.allowed = allowed;
    app.st.pair.busy = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(replies: Vec<Reply>) -> (Option<Result<String, String>>, usize) {
        let mut replies = replies.into_iter();
        let mut asked = 0;
        let stop = std::sync::atomic::AtomicBool::new(false);
        let out = wait_for_link(
            || {
                asked += 1;
                replies.next().unwrap_or(Reply::Waiting)
            },
            || {},
            &stop,
            || {},
        );
        (out, asked)
    }

    #[test]
    fn a_passing_trouble_is_waited_out_and_a_lasting_one_ends_it() {
        let trouble = || Reply::Trouble("Your server could not be reached.".into());
        let (out, _) = run(vec![trouble(), trouble(), Reply::Waiting, trouble(), trouble(), trouble(), Reply::Ready("relay://pair?v=1".into())]);
        assert_eq!(out, Some(Ok("relay://pair?v=1".into())), "three in a row are waited out, and the count starts over");
        let (out, asked) = run(vec![trouble(), trouble(), trouble(), trouble(), Reply::Ready("x".into())]);
        assert_eq!((out, asked), (Some(Err("Your server could not be reached.".into())), 4), "a fourth in a row ends it");
        assert_eq!(run(vec![Reply::Denied]).0, Some(Err("Declined on Discord.".into())), "a refusal is never asked again");
    }
}
