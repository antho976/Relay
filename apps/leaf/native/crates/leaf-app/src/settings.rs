//! Settings, under the server's address and token (`connect.rs`): reading (tap thirds, volume
//! keys, trimmed page edges), notices (quiet hours, one summary a day), how each source is doing,
//! AniList, and exporting a backup. Settings read and write `config.json`; the ones the server
//! keeps (AniList) go to it.

use leaf_proto::{SourceHealth, Status};

use crate::app::{BackupInfo, Loaded, ReadPrefs};
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, Kb};
use crate::logic;
use crate::notices::Notify;
use crate::theme::{self as t, ty};

/// What Settings changes, for the app to do after the page has drawn.
pub enum Act {
    /// A setting changed: keep it.
    Save,
    /// Trimming page edges was turned on or off.
    Trim(bool),
    /// Export the latest backup, or one made now.
    Backup(bool),
    /// Keep an AniList token on the server, or remove it.
    Anilist(Option<String>),
    /// Ask the sources' health again.
    Health,
    /// Put words on the clipboard.
    Copy(&'static str),
    /// Scanlators' promo pages skipped (true) or shown.
    Promo(bool),
    /// Every page hidden by hand shown again.
    ClearHidden,
}

const PIN_URL: &str = "https://anilist.co/api/v2/oauth/pin";
const AUTHORIZE_URL: &str = "https://anilist.co/api/v2/oauth/authorize?client_id=ID&response_type=token";

/// `s` cut into lines no wider than `w`, anywhere (a web address has no spaces to break at).
fn wrap_chars(cx: &mut Ctx, s: &str, w: f32, tt: Text) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for ch in s.chars() {
        line.push(ch);
        if cx.c.measure(&line, tt) > w && line.chars().count() > 1 {
            line.pop();
            out.push(std::mem::take(&mut line));
            line.push(ch);
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// What Settings shows and changes.
pub struct Parts<'a> {
    pub prefs: &'a mut ReadPrefs,
    pub notify: &'a mut Notify,
    pub status: &'a Loaded<Status>,
    pub health: &'a Loaded<Vec<SourceHealth>>,
    pub backup: Option<&'a BackupInfo>,
    pub backing_up: bool,
    /// How many pages were hidden by hand, once the server said.
    pub hidden: Option<u32>,
    /// The AniList token being typed.
    pub anilist: &'a mut String,
    pub now_ms: i64,
}

/// Quiet hours and the summary's time move by this many minutes a step.
const STEP_MIN: u16 = 30;

fn heading(cx: &mut Ctx, x: f32, y: f32, words: &str) -> f32 {
    cx.c.text(words, x, y, Text::new(ty::SMALL, t::INK3).semi());
    26.0
}

/// A row with a switch: its words, a line under them, the switch at the right; the whole row
/// takes the tap. Returns its height and whether it was tapped.
fn toggle_row(cx: &mut Ctx, x: f32, y: f32, w: f32, words: &str, sub: &str, on: bool) -> (f32, bool) {
    let tt = Text::new(ty::BODY, t::INK);
    let st = Text::new(ty::CAPTION, t::INK3).line(1.4);
    let tw = w - 64.0;
    let sub_h = cx.c.para_h(sub, tw, st, 3);
    let h = 12.0 + tt.lh() + 4.0 + sub_h + 12.0;
    if cx.pressed(0.0, y, WIDTH, h) {
        cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
    }
    cx.c.text_fit(words, x, y + 12.0, tw, tt);
    cx.c.para(sub, x, y + 12.0 + tt.lh() + 4.0, tw, st, 3);
    let flip = kit::switch(cx, x + w, y + h / 2.0, on);
    let row = !flip && cx.tap(0.0, y, WIDTH, h);
    (h, flip || row)
}

/// A time with − and + at the right of a row of words. Returns the new time when stepped.
fn time_row(cx: &mut Ctx, x: f32, y: f32, w: f32, words: &str, minute: u16) -> (f32, Option<u16>) {
    let h = 48.0;
    let tt = Text::new(ty::UI, t::INK2);
    cx.c.text(words, x, y + (h - tt.lh()) / 2.0, tt);
    let right = x + w + 10.0;
    let mut out = None;
    if kit::icon_key(cx, right - 44.0, y + 2.0, "plus") {
        out = Some((minute + STEP_MIN) % 1440);
    }
    let vt = Text::new(ty::BODY, t::INK).semi().center();
    cx.c.text(&logic::minute_words(minute), right - 44.0 - 32.0, y + (h - vt.lh()) / 2.0, vt);
    if kit::icon_key(cx, right - 44.0 - 64.0 - 44.0, y + 2.0, "minus") {
        out = Some((minute + 1440 - STEP_MIN) % 1440);
    }
    (h, out)
}

/// The words and colour for a source's state.
pub fn health_words(h: &SourceHealth, now_ms: i64) -> (&'static str, Rgba, String) {
    let (state, ink) = if h.ok {
        ("OK", t::ACCENT)
    } else if h.blocked {
        ("Blocked by a check", t::WARN)
    } else {
        ("Failing", t::DANGER)
    };
    let line = if h.last_ok_at > 0 { format!("Last worked {}", logic::ago(h.last_ok_at, now_ms)) } else if h.ok { "Not used since your server started".into() } else { "Hasn't worked since your server started".into() };
    (state, ink, line)
}

/// Draw the sections from `y` down; returns where they end and what to do.
pub fn draw(cx: &mut Ctx, p: Parts, x: f32, mut y: f32, w: f32, now_ns: u64) -> (f32, Vec<Act>) {
    let mut acts = Vec::new();
    // Reading
    y += heading(cx, x, y, "READING");
    let (h, tap) = toggle_row(cx, x, y, w, "Turn pages with a tap", "A tap in the top or bottom third of a page turns it. The middle shows the controls.", p.prefs.tap_zones);
    if tap {
        p.prefs.tap_zones = !p.prefs.tap_zones;
        acts.push(Act::Save);
    }
    y += h;
    let (h, tap) = toggle_row(cx, x, y, w, "Turn pages with the volume keys", "While a book or a comic is open.", p.prefs.volume_keys);
    if tap {
        p.prefs.volume_keys = !p.prefs.volume_keys;
        acts.push(Act::Save);
    }
    y += h;
    let (h, tap) = toggle_row(cx, x, y, w, "Trim blank page edges", "Long white or black runs at the top and bottom of a comic's pages are cut short.", p.prefs.trim_gutters);
    if tap {
        p.prefs.trim_gutters = !p.prefs.trim_gutters;
        acts.push(Act::Trim(p.prefs.trim_gutters));
        acts.push(Act::Save);
    }
    y += h;
    let (h, tap) = toggle_row(
        cx,
        x,
        y,
        w,
        "Skip scanlator promo pages",
        "The \u{201C}join our Discord\u{201D} and \u{201C}read only at\u{201D} pages groups put at a chapter's start or end are left out. A long press on a page hides it and the ones like it.",
        p.prefs.skip_promo,
    );
    if tap {
        p.prefs.skip_promo = !p.prefs.skip_promo;
        acts.push(Act::Promo(p.prefs.skip_promo));
        acts.push(Act::Save);
    }
    y += h;
    if let Some(n) = p.hidden.filter(|n| *n > 0) {
        let rh = 52.0;
        let tt = Text::new(ty::UI, t::INK2);
        let words = if n == 1 { "1 page hidden by you".to_string() } else { format!("{n} pages hidden by you") };
        cx.c.text_fit(&words, x, y + (rh - tt.lh()) / 2.0, w - 110.0, tt);
        if kit::plain(cx, x + w - 96.0, y + 4.0, 96.0, 44.0, "Clear", None) {
            acts.push(Act::ClearHidden);
        }
        y += rh;
    }
    y += 24.0;

    // Notices
    y += heading(cx, x, y, "NOTIFICATIONS");
    let (h, tap) = toggle_row(cx, x, y, w, "Quiet hours", "Notices that come in these hours wait until they end.", p.notify.quiet);
    if tap {
        p.notify.quiet = !p.notify.quiet;
        acts.push(Act::Save);
    }
    y += h;
    if p.notify.quiet {
        let (h, m) = time_row(cx, x, y, w, "From", p.notify.quiet_from);
        if let Some(m) = m {
            p.notify.quiet_from = m;
            acts.push(Act::Save);
        }
        y += h;
        let (h, m) = time_row(cx, x, y, w, "To", p.notify.quiet_to);
        if let Some(m) = m {
            p.notify.quiet_to = m;
            acts.push(Act::Save);
        }
        y += h + 4.0;
    }
    let (h, tap) = toggle_row(cx, x, y, w, "One summary a day", "Instead of a notice per series as chapters come: \u{201C}12 new chapters in 5 series\u{201D}.", p.notify.summary);
    if tap {
        p.notify.summary = !p.notify.summary;
        acts.push(Act::Save);
    }
    y += h;
    if p.notify.summary {
        let (h, m) = time_row(cx, x, y, w, "At", p.notify.summary_at);
        if let Some(m) = m {
            p.notify.summary_at = m;
            acts.push(Act::Save);
        }
        y += h + 4.0;
    }
    y += 24.0;

    // Sources
    y += heading(cx, x, y, "SOURCES");
    match (&p.health.data, &p.health.error) {
        (Some(list), _) if !list.is_empty() => {
            for hl in list {
                let (state, ink, line) = health_words(hl, p.now_ms);
                let rh = 58.0;
                let tt = Text::new(ty::BODY, t::INK).medium();
                let lt = Text::new(ty::CAPTION, t::INK3);
                let ty0 = y + (rh - tt.lh() - 3.0 - lt.lh()) / 2.0;
                let st_t = Text::new(ty::UI, ink).medium().right();
                let sw = cx.c.measure(state, st_t);
                cx.c.circle(x + 4.0, ty0 + tt.lh() / 2.0, 4.0, ink);
                cx.c.text_fit(&hl.name, x + 18.0, ty0, w - 18.0 - sw - 12.0, tt);
                cx.c.text(state, x + w, ty0 + (tt.lh() - st_t.lh()) / 2.0, st_t);
                let mut l = line;
                if !hl.ok
                    && let Some(e) = &hl.last_error
                {
                    l = format!("{l} · {e}");
                }
                cx.c.text_fit(&l, x + 18.0, ty0 + tt.lh() + 3.0, w - 18.0, lt);
                y += rh;
            }
        }
        (Some(_), _) => {
            y += cx.c.para("Your server has no sources installed.", x, y + 6.0, w, Text::new(ty::UI, t::INK2).line(1.4), 3) + 12.0;
        }
        (None, Some(e)) => {
            y += cx.c.para(e, x, y + 6.0, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            if kit::plain(cx, x, y, 120.0, 40.0, "Retry", None) {
                acts.push(Act::Health);
            }
            y += 48.0;
        }
        (None, None) => {
            let turn = (now_ns as f64 / 1e9 * 0.9).fract() as f32;
            cx.c.arc(x + 9.0, y + 18.0, 8.0, 2.0, turn, turn + 0.7, t::INK3, false);
            cx.c.text("Asking your server", x + 28.0, y + 9.0, Text::new(ty::UI, t::INK2));
            y += 40.0;
        }
    }
    y += 24.0;

    // AniList
    y += heading(cx, x, y, "ANILIST");
    let body = Text::new(ty::UI, t::INK2).line(1.45);
    match p.status.data.as_ref().map(|s| s.anilist) {
        Some(true) => {
            cx.c.icon("check", x + 8.0, y + 10.0, 16.0, t::ACCENT);
            y += cx.c.para("Connected. Chapters you finish are marked read on AniList by your server.", x + 24.0, y, w - 24.0, body, 4) + 14.0;
            if kit::plain(cx, x, y, 140.0, 44.0, "Disconnect", None) {
                acts.push(Act::Anilist(None));
            }
            y += 44.0;
        }
        known => {
            let lead = if known.is_some() { "Not connected. " } else { "" };
            y += cx.c.para(&format!("{lead}Your server can mark chapters you finish read on AniList. To get a token:"), x, y, w, body, 4) + 10.0;
            let step = Text::new(ty::UI, t::INK2).line(1.45);
            let url = Text::new(ty::CAPTION, t::INK).line(1.4);
            let steps: [(&str, Option<&str>); 3] = [
                ("On anilist.co, open Settings, then Developer, and create a client with this redirect:", Some(PIN_URL)),
                ("Open this, with your client's ID in place of ID:", Some(AUTHORIZE_URL)),
                ("Paste the token it shows here.", None),
            ];
            for (i, (words, link)) in steps.iter().enumerate() {
                cx.c.text(&format!("{}.", i + 1), x, y, step);
                y += cx.c.para(words, x + 20.0, y, w - 20.0, step, 4) + 4.0;
                if let Some(link) = link {
                    for line in wrap_chars(cx, link, w - 20.0 - 44.0, url) {
                        cx.c.text(&line, x + 20.0, y, url);
                        y += url.lh();
                    }
                    if *link == AUTHORIZE_URL && kit::icon_key_ink(cx, x + w - 36.0, y - url.lh() - 30.0, "copy", t::INK2) {
                        acts.push(Act::Copy(AUTHORIZE_URL));
                    }
                }
                y += 8.0;
            }
            y += 6.0;
            y += kit::field(cx, "anilist", x, y, w, "Token", "Paste the token", p.anilist, Kb::Secret) + 12.0;
            let ready = !p.anilist.trim().is_empty();
            if kit::plain(cx, x, y, 170.0, 44.0, "Connect AniList", None) && ready {
                acts.push(Act::Anilist(Some(p.anilist.trim().to_string())));
            }
            y += 44.0;
        }
    }
    y += 32.0;

    // Backup
    y += heading(cx, x, y, "BACKUP");
    y += cx.c.para("Your server keeps a backup every night: what is in My space, your shelves and where you are in each series.", x, y, w, body, 4) + 8.0;
    if let Some(b) = p.backup {
        let when = if b.made_ms > 0 { format!("The last one exported was made {}", made_words(b.made_ms, p.now_ms)) } else { format!("The last one exported: {}", b.name) };
        y += cx.c.para(&when, x, y, w, Text::new(ty::CAPTION, t::INK3).line(1.4), 2) + 8.0;
    }
    y += 8.0;
    let kw = (w - 10.0) / 2.0;
    let words = if p.backing_up { "Exporting" } else { "Export backup" };
    if kit::plain(cx, x, y, kw, 44.0, words, Some("upload")) && !p.backing_up {
        acts.push(Act::Backup(false));
    }
    if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Back up now", None) && !p.backing_up {
        acts.push(Act::Backup(true));
    }
    y += 44.0;
    (y, acts)
}

/// "today at 03:00", "on Nov 21, 2026 at 03:00".
fn made_words(ms: i64, now_ms: i64) -> String {
    let off = crate::shelf::zone_offset();
    let day = (ms / 1000 + off).div_euclid(86_400);
    let today = (now_ms / 1000 + off).div_euclid(86_400);
    let time = logic::clock_words(ms, off);
    match today - day {
        0 => format!("today at {time}"),
        1 => format!("yesterday at {time}"),
        _ => format!("on {} at {time}", logic::date(day * 86_400_000)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_says_how_it_is_doing() {
        let h = |ok, blocked, last_ok_at| SourceHealth { name: "X".into(), ok, last_ok_at, last_error: None, blocked };
        let now = 100 * 3_600_000;
        assert_eq!(health_words(&h(true, false, now - 3 * 3_600_000), now).0, "OK");
        assert_eq!(health_words(&h(true, false, now - 3 * 3_600_000), now).2, "Last worked 3 h ago");
        assert_eq!(health_words(&h(false, true, 0), now).0, "Blocked by a check");
        assert_eq!(health_words(&h(false, false, 0), now).0, "Failing");
        assert_eq!(health_words(&h(false, false, 0), now).2, "Hasn't worked since your server started");
    }
}
