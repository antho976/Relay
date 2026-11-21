//! The first run (Kotlin: ui/onboarding/OnboardingScreen.kt, its connect step): connect to the
//! owner's server and take the ledger kept there, or start on this phone.
//!
//! In the premium look: the wallet picture full-bleed at the top, fading into the ground, then
//! Tally in brass, the headline and one line, and the ways in as keys at the foot.
//!
//! Connecting is a tap here, not something the app does by itself: asking sends the owner a DM.
//! Once the server's ledger has arrived the app opens on it. The Kotlin app's set-up questions
//! (currency, accounts, a budget) are not ported: a fresh start opens an empty ledger, where each
//! page says what to add.

use crate::app::App;
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Ctx, PAD};
use crate::nav::Route;
use crate::settings_kit as sk;
use crate::sync::{self, Connect, Tone};
use crate::sync_ledger;
use crate::sync_wire;
use crate::widgets;

#[derive(Default)]
pub struct State {
    /// Where the words and keys at the foot start on screen as last drawn (0 before), so a toast
    /// sits above them, never over a key it would take the tap of.
    pub block_top: f32,
}

/// The picture's height at the top of the page.
const PIC_H: f32 = 480.0;

enum Act {
    Connect,
    Again,
    /// Paired, and the first pull failed: sync again now (turning sync on, if it is off).
    Retry,
    /// Paired with sync off (an Erase while off): turn it on and bring the server's ledger over.
    TurnOn,
    Dashboard(String),
    Fresh,
    Sample,
    /// The answer to the first pull's question ([`sync::FIRST_BODY`]).
    First(bool),
}

/// The one filled key, by where connecting stands.
#[derive(Clone, Copy)]
enum Key {
    Connect,
    Again,
    Empty,
    Retry,
    TurnOn,
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let paired = app.st.sync.paired(&app.store);
    let pulled = paired && app.store.pref(sync_ledger::PULLED).is_some();
    // The server's ledger arrived: nothing to ask.
    if pulled && !app.store.is_empty() {
        finish(app);
        return true;
    }
    let pg = app.page_begin(dt);
    let line = app.st.sync.line(&app.store, sync::now_ms());
    let connect = app.st.sync.connect.clone();
    let waiting = match (&connect, paired) {
        (Connect::Waiting { code, notified, server, .. }, false) => Some((code.clone(), *notified, server.clone())),
        _ => None,
    };
    let empty = pulled;
    let asking = app.st.sync.awaiting_yes();
    // Paired, but the server's ledger has not come and the last try failed (a failed first pull,
    // an Erase while paired): say why, and give a way on rather than waiting here for good.
    // Paired with sync off (Erase everything while it was off): nothing comes by itself, so it
    // asks to turn sync on, or to start here.
    let off = paired && !pulled && !asking && app.store.pref(sync::pref::PHONE_ONLY).is_some();
    let stuck = !off && paired && !pulled && !asking && line.tone != Tone::Live && app.store.pref(sync::pref::LAST_ERROR).is_some();
    let key = if empty {
        Some(Key::Empty)
    } else if off {
        Some(Key::TurnOn)
    } else if stuck {
        Some(Key::Retry)
    } else if paired {
        None
    } else if matches!(connect, Connect::Denied | Connect::Expired | Connect::Trouble { .. }) {
        Some(Key::Again)
    } else if matches!(connect, Connect::Idle | Connect::Connected) {
        Some(Key::Connect)
    } else {
        None
    };
    let mut act = None;
    let bottom;
    let block_top;
    {
        let mut cx = cx!(app);
        let held = if asking { widgets::hold(&mut cx) } else { None };
        let p = cx.p;
        // The picture, edge to edge from the top of the screen, fading into the ground.
        let pic_top = 0.0;
        kit::picture(&mut cx, "wallet", 0.0, pic_top, WIDTH, PIC_H, 0.0, (0.5, 0.42));
        let fade = 190.0;
        let g = p.ground[0];
        cx.c.rect_grad(-1.0, pic_top + PIC_H - fade, WIDTH + 2.0, fade + 1.0, 0.0, g.a(0.0), g);

        let w = WIDTH - PAD * 2.0 - 8.0;
        let x = PAD + 4.0;
        let brand = Text::new(16.0, p.signal).brand();
        let head_t = Text::new(32.0, p.ink).semi().line(1.15);
        let body_t = Text::new(15.0, p.ink2).line(1.45);
        let headline = "Every dollar, where you put it.";
        let lede = "Spending, bills and goals on your phone, kept in step with your own server.";
        // Measure, so the words and keys sit at the foot of the screen when they fit there.
        let hh = cx.c.para_h(headline, w, head_t, 3);
        let bh = cx.c.para_h(lede, w, body_t, 4);
        // Measured at the width it is painted at below, and with only the keys drawn below, so
        // the block's top (where the toast goes too) is where it really starts.
        let (status_h, _) = status_box(&mut cx, PAD, 0.0, WIDTH - PAD * 2.0, &line, waiting.as_ref(), paired, empty, stuck, off, false);
        let main_key = if key.is_some() || !paired { 56.0 + 10.0 } else { 0.0 };
        let start_here = if stuck || off { 40.0 } else { 0.0 };
        let sample_and_fresh = if paired { 0.0 } else { 56.0 + 6.0 + 40.0 };
        let keys_h = main_key + start_here + sample_and_fresh;
        let block = brand.lh() + 10.0 + hh + 12.0 + bh + 28.0 + if status_h > 0.0 { status_h + 16.0 } else { 0.0 } + keys_h;
        let foot = cx.c.h - cx.c.inset_bottom - 20.0;
        let mut y = (PIC_H - 34.0).max(foot - block);
        block_top = y + cx.c.dy;

        cx.c.text("Tally", x, y, brand);
        y += brand.lh() + 10.0;
        y += cx.c.para(headline, x, y, w, head_t, 3) + 12.0;
        y += cx.c.para(lede, x, y, w, body_t, 4) + 28.0;
        if status_h > 0.0 {
            let (h, tapped) = status_box(&mut cx, PAD, y, WIDTH - PAD * 2.0, &line, waiting.as_ref(), paired, empty, stuck, off, true);
            if tapped.is_some() {
                act = tapped;
            }
            y += h + 16.0;
        }
        let kw = WIDTH - PAD * 2.0;
        match key {
            Some(k) => {
                let (words, a) = match k {
                    Key::Connect => ("Connect to your server", Act::Connect),
                    Key::Again => ("Ask again", Act::Again),
                    Key::Empty => ("Start with an empty ledger", Act::Fresh),
                    Key::Retry => ("Try again", Act::Retry),
                    Key::TurnOn => ("Turn sync on and bring the ledger over", Act::TurnOn),
                };
                if kit::key(&mut cx, PAD, y, kw, 56.0, words, None, kit::KeyKind::Primary) {
                    act = Some(a);
                }
                y += 56.0 + 10.0;
            }
            None if !paired => {
                // Waiting on the owner: the key says so and does nothing.
                sk::tall_key(&mut cx, PAD, y, kw, 56.0, "Waiting for your Allow", None, false, false);
                y += 56.0 + 10.0;
            }
            None => {}
        }
        if stuck || off {
            // The way out: start now. Nothing here goes over the server's ledger: the first pull
            // still comes (once sync is on), and asks before it replaces anything made here.
            let t = Text::new(14.0, p.ink2).medium().center();
            let words = if off { "Start on this phone" } else { "Start on this phone for now" };
            let tw = cx.c.measure(words, t);
            if cx.pressed(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0) {
                cx.c.rect(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0, 20.0, p.wash);
            }
            cx.c.text(words, WIDTH / 2.0, y + (40.0 - t.lh()) / 2.0, t);
            if cx.tap(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0) {
                act = Some(Act::Fresh);
            }
            y += 40.0;
        }
        if !paired {
            if kit::key(&mut cx, PAD, y, kw, 56.0, "Try it with sample data", None, kit::KeyKind::Plain) {
                act = Some(Act::Sample);
            }
            y += 56.0 + 6.0;
            let t = Text::new(14.0, p.ink2).medium().center();
            let words = "Start fresh on this phone";
            let tw = cx.c.measure(words, t);
            if cx.pressed(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0) {
                cx.c.rect(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0, 20.0, p.wash);
            }
            cx.c.text(words, WIDTH / 2.0, y + (40.0 - t.lh()) / 2.0, t);
            if cx.tap(WIDTH / 2.0 - tw / 2.0 - 16.0, y, tw + 32.0, 40.0) {
                act = Some(Act::Fresh);
            }
            y += 40.0;
        }
        bottom = y;
        if asking {
            widgets::release(&mut cx, held);
            if let Some(yes) = widgets::confirm(&mut cx, sync::FIRST_TITLE, app.st.sync.question(), sync::FIRST_YES, true) {
                act = Some(Act::First(yes));
            }
        }
    }
    app.page_end(&pg, bottom);
    app.st.onboarding.block_top = block_top;
    match act {
        Some(Act::Connect) | Some(Act::Again) => app.st.sync.connect_now(&mut app.store),
        Some(Act::Retry) | Some(Act::TurnOn) => {
            // Sync on first if it is off: off, the ask to sync is dropped unrun. The Your server
            // page's switch, turned on: it syncs too.
            app.st.sync.set_sync(&mut app.store, true);
        }
        Some(Act::Dashboard(url)) => app.host.open_url(&url),
        Some(Act::Fresh) => finish(app),
        Some(Act::Sample) => match app.store.load_sample(crate::model::today()) {
            Ok(_) => {
                // The sample is for trying the app on this phone: it never goes to the server.
                // Only once it is in: a sample that failed leaves sync as it was.
                app.st.sync.keep_on_phone(&mut app.store);
                // Its bills are new: one due today posts today, not tomorrow.
                app.bills_changed();
                app.invalidate();
                finish(app);
            }
            Err(e) => app.say(format!("The sample could not be made: {e}"), None),
        },
        Some(Act::First(yes)) => {
            app.st.sync.answer_first(&mut app.store, yes);
            if !yes {
                finish(app);
            }
        }
        None => {}
    }
    pg.moving || app.st.sync.working() || asking
}

/// Back while the first pull's question is open answers it as its Cancel does: sync goes off,
/// this phone keeps its ledger, and the app starts. True when it did.
pub fn back(app: &mut App) -> bool {
    if !app.st.sync.awaiting_yes() {
        return false;
    }
    app.st.sync.answer_first(&mut app.store, false);
    finish(app);
    true
}

fn finish(app: &mut App) {
    let _ = app.store.set_pref("onboarded", "1");
    app.push(Route::Overview);
}

/// Where connecting stands, on a card: a state dot and a line, what to do while it waits, the
/// code, and the dashboard as the other way. Nothing (height 0) when there is nothing to say yet.
/// With `paint` false it only measures. Returns its height and what was tapped.
#[allow(clippy::too_many_arguments)]
fn status_box(cx: &mut Ctx, x: f32, y: f32, w: f32, line: &sync::Line, waiting: Option<&(String, bool, String)>, paired: bool, empty: bool, stuck: bool, off: bool, paint: bool) -> (f32, Option<Act>) {
    let p = cx.p;
    if !paired && waiting.is_none() && line.tone == Tone::Off {
        return (0.0, None);
    }
    let inner = w - 40.0;
    let title_t = Text::new(16.0, p.ink).medium().line(1.3);
    let ui_t = Text::new(14.0, p.ink2).line(1.45);
    let cap_t = Text::new(13.0, p.ink3).line(1.45);
    let title = if empty {
        "Connected. Your server has no ledger yet."
    } else if off {
        "Connected, but sync is off."
    } else if stuck {
        "Connected, but your ledger has not come over yet."
    } else if paired {
        "Connected. Bringing your ledger over."
    } else if waiting.is_some() {
        "Tap Allow in Discord"
    } else {
        line.text.as_str()
    };
    let explain = waiting
        .map(|(_, notified, _)| {
            if *notified {
                "Your bot just sent you a message on Discord. Tap Allow if it shows the same code. That is the only step, once."
            } else {
                "Your bot could not message you. Allow the request on its dashboard, Tally sync, where it shows the same code."
            }
        })
        .or(stuck.then_some("Tally keeps trying by itself. You can start now: when your ledger comes, Tally asks before it replaces anything you add here."))
        .or(off.then_some("Your ledger stays on your server, and nothing comes over while sync is off. Turn it on to bring it here, or start on this phone with sync off."));
    // Paired, the line's detail is the last try's error: shown only while it holds the page up.
    let detail = line.detail.as_deref().filter(|_| (!paired && waiting.is_none()) || stuck);

    let mut h = 20.0;
    let th = cx.c.para_h(title, inner - 16.0, title_t, 2);
    h += th;
    let eh = explain.map_or(0.0, |e| 8.0 + cx.c.para_h(e, inner, ui_t, 4));
    h += eh;
    if waiting.is_some() {
        h += 14.0 + 76.0 + 8.0 + 40.0;
    }
    let dh = detail.map_or(0.0, |d| 8.0 + cx.c.para_h(d, inner, cap_t, 3));
    h += dh;
    h += 20.0;
    if !paint {
        return (h, None);
    }

    let mut out = None;
    kit::card(cx, x, y, w, h);
    let mut iy = y + 20.0;
    let tone = if paired && !stuck && !off { p.live } else { line.tone.colour(&p) };
    cx.c.circle(x + 20.0 + 4.0, iy + title_t.lh() / 2.0, 4.0, tone);
    cx.c.para(title, x + 20.0 + 16.0, iy, inner - 16.0, title_t, 2);
    iy += th;
    if let Some(e) = explain {
        iy += 8.0;
        iy += cx.c.para(e, x + 20.0, iy, inner, ui_t, 4);
    }
    if let Some((code, notified, server)) = waiting {
        iy += 14.0;
        cx.c.rect(x + 20.0, iy, inner, 76.0, kit::KEY_R, p.card2);
        cx.c.text("YOUR PHONE'S CODE", x + w / 2.0, iy + 12.0, Text::new(11.0, p.ink3).semi().center());
        let shown: String = sync_wire::spaced_code(code).chars().map(|c| if c == ' ' { "  ".to_string() } else { format!("{c} ") }).collect::<String>().trim().to_string();
        cx.c.text(&shown, x + w / 2.0, iy + 32.0, Text::new(28.0, p.ink).medium().center().line(1.2));
        iy += 76.0 + 8.0;
        let label = "Open the dashboard";
        let t = Text::new(14.0, if *notified { p.ink2 } else { p.ink }).medium();
        let kw = cx.c.measure(label, t) + 50.0;
        if cx.pressed(x + 12.0, iy, kw, 40.0) {
            cx.c.rect(x + 12.0, iy, kw, 40.0, 20.0, p.wash);
        }
        cx.c.icon("external", x + 12.0 + 18.0, iy + 20.0, 15.0, t.color);
        cx.c.text(label, x + 12.0 + 36.0, iy + (40.0 - t.lh()) / 2.0, t);
        if cx.tap(x + 12.0, iy, kw, 40.0) {
            out = Some(Act::Dashboard(format!("{server}/dashboard/tally")));
        }
        iy += 40.0;
    }
    if let Some(d) = detail {
        iy += 8.0;
        cx.c.para(d, x + 20.0, iy, inner, cap_t, 3);
    }
    (h, out)
}
