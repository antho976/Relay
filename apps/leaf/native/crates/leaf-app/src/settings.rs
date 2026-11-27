//! Settings as a phone's settings page (the owner, 2026-11-26, after one long page: "group the
//! settings like a real settings page", showing Claude's): the server on a card at the top, then
//! rows in groups (this phone's Reading, Downloads, Notifications; your server's Sources, AniList,
//! Backups), each opening its own page ([`Part`]), and the build last. Settings read and write `config.json`; the ones the
//! server keeps (AniList, the room) go to it. The first run's Connect screen is `connect.rs`.

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
    /// Copy the books to the server now; `true` over a copy another phone made.
    CopyBooks(bool),
    /// Fetch the copy of the books back (a second tap when books are here).
    RestoreBooks,
    /// Not restore after all.
    KeepBooks,
    /// Ask the server about itself again.
    ServerInfo,
    /// The room the server's chapters may take, in GB (None: no limit).
    DiskCap(Option<u32>),
    /// Check the address and token typed, and use them once the server answers.
    Connect,
}

/// A page of Settings, opened from its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Reading,
    Downloads,
    Notifications,
    Server,
    Sources,
    Anilist,
    Backups,
}

impl Part {
    pub fn title(self) -> &'static str {
        match self {
            Part::Reading => "Reading",
            Part::Downloads => "Downloads",
            Part::Notifications => "Notifications",
            Part::Server => "Server",
            Part::Sources => "Sources",
            Part::Anilist => "AniList",
            Part::Backups => "Backups",
        }
    }

    /// By its name in lower case ("server"), for the desktop's shots.
    pub fn named(name: &str) -> Option<Part> {
        [Part::Reading, Part::Downloads, Part::Notifications, Part::Server, Part::Sources, Part::Anilist, Part::Backups].into_iter().find(|p| p.title().eq_ignore_ascii_case(name))
    }

    fn glyph(self) -> &'static str {
        match self {
            Part::Reading => "book-open",
            Part::Downloads => "download",
            Part::Notifications => "bell",
            Part::Server => "server",
            Part::Sources => "globe",
            Part::Anilist => "sync",
            Part::Backups => "archive",
        }
    }
}

/// The index's groups, in order: a heading and its rows.
const GROUPS: [(&str, &[Part]); 2] = [("THIS PHONE", &[Part::Reading, Part::Downloads, Part::Notifications]), ("YOUR SERVER", &[Part::Sources, Part::Anilist, Part::Backups])];

/// The server's address and token, folded under its row until Change.
pub struct Conn<'a> {
    /// The address in use, as it reads ("anthobot.bid/leaf").
    pub label: String,
    pub editing: &'a mut bool,
    pub address: &'a mut String,
    pub token: &'a mut String,
    pub checking: bool,
    /// The last address tried failed: why.
    pub failed: Option<&'a str>,
    /// The address typed doesn't read as one.
    pub invalid: Option<&'static str>,
}

/// The sizes offered for the server's chapters, in GB (none first: no limit).
pub const CAPS: [Option<u32>; 11] = [None, Some(5), Some(10), Some(20), Some(30), Some(50), Some(75), Some(100), Some(150), Some(200), Some(300)];

/// The next size up or down from `now` (a size not offered steps to the nearest).
pub fn cap_step(now: Option<u32>, up: bool) -> Option<u32> {
    let i = CAPS.iter().position(|c| *c == now).unwrap_or_else(|| CAPS.iter().position(|c| c.is_some_and(|g| Some(g) >= now)).unwrap_or(CAPS.len() - 1));
    let j = if up { (i + 1).min(CAPS.len() - 1) } else { i.saturating_sub(1) };
    CAPS[j]
}

/// The copy of the books on the server, as Settings shows it ([`crate::booksync`]).
pub struct Books<'a> {
    pub copied: crate::booksync::Copied,
    pub busy: bool,
    pub restoring: bool,
    /// A copy this phone didn't make, from when (ms).
    pub found: Option<i64>,
    pub error: Option<&'a str>,
    pub too_big: usize,
    /// Books on this phone now.
    pub here: usize,
    /// Restore was tapped once: the next tap replaces the books here.
    pub confirm: bool,
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
    pub books: Books<'a>,
    /// How many pages were hidden by hand, once the server said.
    pub hidden: Option<u32>,
    /// What the server says of itself.
    pub server: &'a Loaded<crate::net::ServerInfo>,
    /// The AniList token being typed.
    pub anilist: &'a mut String,
    /// AniList's setup steps are showing.
    pub anilist_open: &'a mut bool,
    pub conn: Conn<'a>,
    pub now_ms: i64,
}

/// Space between one section and the next heading.
const GAP: f32 = 28.0;

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
    let sub_h = cx.c.para_h(sub, tw, st, 4);
    let h = 12.0 + tt.lh() + 4.0 + sub_h + 12.0;
    if cx.pressed(0.0, y, WIDTH, h) {
        cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
    }
    cx.c.text_fit(words, x, y + 12.0, tw, tt);
    cx.c.para(sub, x, y + 12.0 + tt.lh() + 4.0, tw, st, 4);
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

/// Draw one page of Settings from `y` down; returns where it ends and what to do.
#[allow(clippy::too_many_arguments)]
pub fn draw(cx: &mut Ctx, p: Parts, part: Part, x: f32, mut y: f32, w: f32, now_ns: u64) -> (f32, Vec<Act>) {
    let mut acts = Vec::new();
    match part {
        Part::Reading => {
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
            let (h, tap) = toggle_row(cx, x, y, w, "Open where I left off", "Closed while reading, Leaf opens again in that chapter, at your place.", p.prefs.reopen);
            if tap {
                p.prefs.reopen = !p.prefs.reopen;
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
        }
        Part::Downloads => {
            let (h, tap) = toggle_row(
                cx,
                x,
                y,
                w,
                "Download Next up on Wi-Fi",
                "The next 2 chapters of the 3 series closest to their end are kept on this phone, ready to read with no connection.",
                p.prefs.auto_next,
            );
            if tap {
                p.prefs.auto_next = !p.prefs.auto_next;
                acts.push(Act::Save);
            }
            y += h;
            let (h, tap) = toggle_row(cx, x, y, w, "Delete downloads once read", "A chapter downloaded to this phone goes when you finish it and leave the reader.", p.prefs.delete_read);
            if tap {
                p.prefs.delete_read = !p.prefs.delete_read;
                acts.push(Act::Save);
            }
            y += h;
        }
        Part::Notifications => {
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
        }
        Part::Server => {
            // Where it is (Change folds out the address), then what it is doing and its room.
            y += conn_part(cx, p.conn, p.status, x, y, w, now_ns, &mut acts) + GAP;
            y += heading(cx, x, y, "CHAPTERS ON YOUR SERVER");
            y += server_part(cx, p.server, x, y, w, p.now_ms, now_ns, &mut acts);
        }
        Part::Sources => {
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
                (None, None) => y += asking(cx, x, y, now_ns),
            }
        }
        Part::Anilist => {
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
                known if !*p.anilist_open => {
                    let lead = if known.is_some() { "Not connected. " } else { "" };
                    y += cx.c.para(&format!("{lead}Your server can mark chapters you finish read on AniList."), x, y, w, body, 4) + 14.0;
                    if kit::plain(cx, x, y, 140.0, 44.0, "Set up", None) {
                        *p.anilist_open = true;
                    }
                    y += 44.0;
                }
                _ => {
                    y += cx.c.para("To get a token for your server:", x, y, w, body, 4) + 10.0;
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
                    let kw = (w - 10.0) / 2.0;
                    if kit::plain(cx, x, y, kw, 44.0, "Connect AniList", None) && ready {
                        acts.push(Act::Anilist(Some(p.anilist.trim().to_string())));
                    }
                    if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Cancel", None) {
                        *p.anilist_open = false;
                    }
                    y += 44.0;
                }
            }
        }
        Part::Backups => {
            let body = Text::new(ty::UI, t::INK2).line(1.45);
            let small = Text::new(ty::CAPTION, t::INK3).line(1.4);
            y += cx.c.para("Your server keeps a backup every night: what is in My space, your shelves and where you are in each series.", x, y, w, body, 4) + 8.0;
            if let Some(i) = &p.server.data {
                let backup = if i.backup_at > 0 { format!("Last made {}", logic::ago(i.backup_at, p.now_ms)) } else { "None made yet".to_string() };
                let pc = if i.offsite_at > 0 { format!("copied to your PC {}", logic::ago(i.offsite_at, p.now_ms)) } else { "not copied to your PC yet".to_string() };
                y += cx.c.para(&format!("{backup} · {pc}"), x, y, w, small, 3) + 4.0;
            }
            if let Some(b) = p.backup {
                let when = if b.made_ms > 0 { format!("The last one exported was made {}", made_words(b.made_ms, p.now_ms)) } else { format!("The last one exported: {}", b.name) };
                y += cx.c.para(&when, x, y, w, small, 2) + 4.0;
            }
            y += 12.0;
            let kw = (w - 10.0) / 2.0;
            let words = if p.backing_up { "Exporting" } else { "Export backup" };
            if kit::plain(cx, x, y, kw, 44.0, words, Some("upload")) && !p.backing_up {
                acts.push(Act::Backup(false));
            }
            if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Back up now", None) && !p.backing_up {
                acts.push(Act::Backup(true));
            }
            y += 44.0 + 28.0;
            y += books_part(cx, &p.books, x, y, w, p.now_ms, &mut acts);
        }
    }
    (y, acts)
}

/// What the index needs to say on its rows.
pub struct Index<'a> {
    pub label: &'a str,
    pub status: &'a Loaded<Status>,
    pub health: &'a Loaded<Vec<SourceHealth>>,
    pub server: &'a Loaded<crate::net::ServerInfo>,
    pub now_ms: i64,
}

/// The index: the server on a card, the groups of rows, the build. Returns where it ends and the
/// page tapped.
pub fn index(cx: &mut Ctx, ix: Index, x: f32, mut y: f32, w: f32, now_ns: u64) -> (f32, Option<Part>) {
    let mut open = None;
    // The server, as an account would be: where it is and how it answered.
    let tt = Text::new(ty::BODY, t::INK).semi();
    let lt = Text::new(ty::CAPTION, t::INK3);
    let (mark, line, ink) = status_line(ix.status);
    let h = 72.0;
    let pressed = cx.pressed(x, y, w, h);
    cx.c.rect(x, y, w, h, 14.0, if pressed { t::S2 } else { t::S1 });
    let ty0 = y + (h - tt.lh() - 3.0 - lt.lh()) / 2.0;
    cx.c.icon("server", x + 28.0, y + h / 2.0, 20.0, t::INK2);
    let tx = x + 52.0;
    cx.c.text_fit(ix.label, tx, ty0, w - 52.0 - 36.0, tt);
    if line.is_empty() {
        let turn = (now_ns as f64 / 1e9 * 0.9).fract() as f32;
        cx.c.arc(tx + 6.0, ty0 + tt.lh() + 3.0 + lt.lh() / 2.0, 5.0, 2.0, turn, turn + 0.7, t::INK3, false);
        cx.c.text("Asking your server", tx + 18.0, ty0 + tt.lh() + 3.0, lt);
    } else {
        let mut lx = tx;
        if let Some(m) = mark {
            cx.c.circle(tx + 3.5, ty0 + tt.lh() + 3.0 + lt.lh() / 2.0, 3.5, m);
            lx += 13.0;
        }
        cx.c.text_fit(&line, lx, ty0 + tt.lh() + 3.0, w - (lx - x) - 36.0, lt.color(ink));
    }
    cx.c.icon("chevron-right", x + w - 22.0, y + h / 2.0, 16.0, t::INK3);
    if cx.tap(x, y, w, h) {
        open = Some(Part::Server);
    }
    y += h + 26.0;
    for (title, parts) in GROUPS {
        y += heading(cx, x + 4.0, y, title);
        let rh = 56.0;
        let gh = rh * parts.len() as f32 + 2.0 * (parts.len() as f32 - 1.0);
        cx.c.rect(x, y, w, gh, 14.0, t::S1);
        for (i, &part) in parts.iter().enumerate() {
            let ry = y + i as f32 * (rh + 2.0);
            if cx.pressed(x, ry, w, rh) {
                // The press stays inside its row, the group's corners kept.
                cx.c.push_clip(x, ry, w, rh);
                cx.c.rect(x, y, w, gh, 14.0, t::S2);
                cx.c.pop_clip();
            }
            if i > 0 {
                cx.c.rect(x, ry - 2.0, w, 2.0, 0.0, t::BG);
            }
            cx.c.icon(part.glyph(), x + 28.0, ry + rh / 2.0, 20.0, t::INK2);
            let bt = Text::new(ty::BODY, t::INK);
            let (value, vink) = row_value(part, &ix);
            let vt = Text::new(ty::UI, vink).right();
            let vw = if value.is_empty() { 0.0 } else { cx.c.measure(&value, vt) + 12.0 };
            cx.c.text_fit(part.title(), x + 52.0, ry + (rh - bt.lh()) / 2.0, w - 52.0 - 16.0 - vw, bt);
            if !value.is_empty() {
                cx.c.text(&value, x + w - 16.0, ry + (rh - vt.lh()) / 2.0, vt);
            }
            if cx.tap(x, ry, w, rh) {
                open = Some(part);
            }
        }
        y += gh + 26.0;
    }
    // The build, last (a build made without a number says nothing).
    if crate::update::BUILD != 0 {
        let build = format!("Leaf {}", crate::update::version_of(crate::update::BUILD));
        y += 6.0;
        cx.c.text(&build, x + w / 2.0, y, Text::new(ty::CAPTION, t::INK3).center());
        y += Text::new(ty::CAPTION, t::INK3).lh();
    }
    (y, open)
}

/// The one reading a row shows at its right: how the sources are doing, whether AniList is
/// connected, when the last backup was made.
fn row_value(part: Part, ix: &Index) -> (String, Rgba) {
    match part {
        Part::Sources => match &ix.health.data {
            Some(list) if !list.is_empty() => {
                let failing = list.iter().filter(|h| !h.ok).count();
                if failing == 0 { (format!("{} working", list.len()), t::INK3) } else { (format!("{failing} failing"), t::WARN) }
            }
            _ => (String::new(), t::INK3),
        },
        Part::Anilist => match ix.status.data.as_ref().map(|s| s.anilist) {
            Some(true) => ("Connected".into(), t::INK3),
            Some(false) => ("Off".into(), t::INK3),
            None => (String::new(), t::INK3),
        },
        Part::Backups => match &ix.server.data {
            Some(i) if i.backup_at > 0 => (logic::ago(i.backup_at, ix.now_ms), t::INK3),
            Some(_) => ("None yet".into(), t::WARN),
            None => (String::new(), t::INK3),
        },
        _ => (String::new(), t::INK3),
    }
}

/// How the server answered, in a line: a mark, the words, their colour. Empty while asking.
fn status_line(status: &Loaded<Status>) -> (Option<Rgba>, String, Rgba) {
    match (&status.data, &status.error) {
        (Some(s), _) => {
            let sources = match s.sources.len() {
                0 => "no sources yet".to_string(),
                1 => "1 source".to_string(),
                n => format!("{n} sources"),
            };
            let series = match s.series {
                1 => "1 series followed".to_string(),
                n => format!("{n} series followed"),
            };
            let line = if s.suwayomi.is_some() { format!("Connected · {sources} · {series}") } else { "Connected, but its comics part isn't answering".to_string() };
            (Some(t::ACCENT), line, if s.suwayomi.is_some() { t::INK3 } else { t::WARN })
        }
        (None, Some(e)) => (Some(t::DANGER), e.clone(), t::DANGER),
        (None, None) => (None, String::new(), t::INK3),
    }
}

/// "Asking your server" with a turning arc. Returns its height.
fn asking(cx: &mut Ctx, x: f32, y: f32, now_ns: u64) -> f32 {
    let turn = (now_ns as f64 / 1e9 * 0.9).fract() as f32;
    cx.c.arc(x + 9.0, y + 18.0, 8.0, 2.0, turn, turn + 0.7, t::INK3, false);
    cx.c.text("Asking your server", x + 28.0, y + 9.0, Text::new(ty::UI, t::INK2));
    40.0
}

/// Where the server is: its address and how it answered, with Change at the right; Change folds
/// out the address and token, Connect and Cancel. Returns its height.
#[allow(clippy::too_many_arguments)]
fn conn_part(cx: &mut Ctx, c: Conn, status: &Loaded<Status>, x: f32, y0: f32, w: f32, now_ns: u64, acts: &mut Vec<Act>) -> f32 {
    let mut y = y0;
    if *c.editing {
        y += 4.0;
        y += kit::field(cx, "address", x, y, w, "Address", "192.168.1.20:4580", c.address, Kb::Url) + 16.0;
        y += kit::field(cx, "token", x, y, w, "Token", "Only if your server has one", c.token, Kb::Secret) + 16.0;
        if let Some(why) = c.invalid.or(c.failed) {
            y += cx.c.para(why, x, y, w, Text::new(ty::UI, t::DANGER).line(1.45), 4) + 12.0;
        }
        let kw = (w - 10.0) / 2.0;
        if kit::primary(cx, x, y, kw, 44.0, if c.checking { "Checking" } else { "Connect" }, None) && !c.checking {
            acts.push(Act::Connect);
        }
        if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Cancel", None) {
            *c.editing = false;
        }
        return y + 44.0 - y0;
    }
    let key_w = 96.0;
    let tt = Text::new(ty::BODY, t::INK).semi();
    let lt = Text::new(ty::CAPTION, t::INK3).line(1.4);
    let (mark, line, ink) = status_line(status);
    let tw = w - key_w - 12.0;
    let left = if mark.is_some() { 18.0 } else { 0.0 };
    if let Some(m) = mark {
        cx.c.circle(x + 4.0, y + 4.0 + tt.lh() / 2.0, 4.0, m);
    }
    cx.c.text_fit(&c.label, x + left, y + 4.0, tw - left, tt);
    let mut h = 4.0 + tt.lh() + 3.0;
    if line.is_empty() {
        let turn = (now_ns as f64 / 1e9 * 0.9).fract() as f32;
        cx.c.arc(x + left + 7.0, y + h + 8.0, 6.0, 2.0, turn, turn + 0.7, t::INK3, false);
        cx.c.text("Asking your server", x + left + 20.0, y + h, lt);
        h += lt.lh();
    } else {
        h += cx.c.para(&line, x + left, y + h, tw - left, lt.color(ink), 3);
    }
    h = h.max(48.0);
    if kit::plain(cx, x + w - key_w, y + (h - 40.0) / 2.0, key_w, 40.0, "Change", None) {
        *c.editing = true;
    }
    y += h;
    y - y0
}

/// The server's own readings: its last pass and download line, and the room its chapters take
/// with the size you set (− and +).
/// Returns its height.
#[allow(clippy::too_many_arguments)]
fn server_part(cx: &mut Ctx, s: &Loaded<crate::net::ServerInfo>, x: f32, y0: f32, w: f32, now_ms: i64, now_ns: u64, acts: &mut Vec<Act>) -> f32 {
    let mut y = y0;
    let body = Text::new(ty::UI, t::INK2).line(1.45);
    let small = Text::new(ty::CAPTION, t::INK3).line(1.4);
    let Some(i) = &s.data else {
        match &s.error {
            Some(e) => {
                y += cx.c.para(e, x, y + 6.0, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
                if kit::plain(cx, x, y, 120.0, 40.0, "Retry", None) {
                    acts.push(Act::ServerInfo);
                }
                y += 48.0;
            }
            None => y += asking(cx, x, y, now_ns),
        }
        return y - y0;
    };
    let mut lines = Vec::new();
    if i.pass_at > 0 {
        lines.push(format!("Looked for new chapters {}", logic::ago(i.pass_at, now_ms)));
    }
    lines.push(match i.queued {
        0 => "Nothing waiting to download".to_string(),
        n => format!("{} waiting to download", logic::chapters_words(n as usize)),
    });
    for l in &lines {
        y += cx.c.para(l, x, y, w, body, 2) + 2.0;
    }
    y += 16.0;
    // The room: a meter of what the chapters take against the size set, and the size.
    let r = &i.room;
    let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
    let used_words = r.used.map(crate::space::size_words);
    let head = match (&used_words, r.cap_gb) {
        (Some(u), Some(c)) => format!("Chapters take {u} of {c} GB"),
        (Some(u), None) => format!("Chapters take {u}"),
        (None, _) => "Your server doesn't share its downloads with Leaf's part".to_string(),
    };
    cx.c.text_fit(&head, x, y, w, Text::new(ty::BODY, t::INK));
    y += Text::new(ty::BODY, t::INK).lh() + 10.0;
    if let (Some(b), Some(c)) = (r.used, r.cap_gb) {
        let f = (gb(b) / c as f64).clamp(0.0, 1.0) as f32;
        let ink = if f >= 0.9 { t::WARN } else { t::ACCENT };
        cx.c.rect(x, y, w, 6.0, 3.0, t::S3);
        cx.c.rect(x, y, (w * f).max(6.0), 6.0, 3.0, ink);
        // Where cleaning starts.
        cx.c.rect(x + w * 0.9 - 1.0, y - 3.0, 2.0, 12.0, 1.0, t::INK3);
        y += 18.0;
    }
    let (h, step) = cap_row(cx, x, y, w, r.cap_gb);
    if let Some(c) = step {
        acts.push(Act::DiskCap(c));
    }
    y += h;
    let rule = if r.cap_gb.is_some() {
        "At 90% full, your server deletes the chapters you read longest ago, down to 80%. Never one you haven't read, nor the last two read of a series; a chapter deleted still opens, from its source."
    } else {
        "No limit set: your server only deletes chapters 20 or more behind where you are in a series. Set a size, and at 90% full the ones you read longest ago go first."
    };
    y += cx.c.para(rule, x, y, w, small, 5) + 8.0;
    if r.cleaned_at > 0 && r.cleaned > 0 {
        y += cx.c.para(&format!("Deleted {} read long ago {}", logic::chapters_words(r.cleaned as usize), logic::ago(r.cleaned_at, now_ms)), x, y, w, small, 2) + 4.0;
    }
    if r.stuck {
        y += cx.c.para("Over 90% with nothing read left to delete: chapters not read yet fill it. A larger size makes room.", x, y, w, small.color(t::WARN), 3) + 4.0;
    }
    y - y0
}

/// "Room for chapters" with − and + at the right. Returns its height and the size stepped to.
fn cap_row(cx: &mut Ctx, x: f32, y: f32, w: f32, cap: Option<u32>) -> (f32, Option<Option<u32>>) {
    let h = 48.0;
    let tt = Text::new(ty::UI, t::INK2);
    cx.c.text("Room for chapters", x, y + (h - tt.lh()) / 2.0, tt);
    let right = x + w + 10.0;
    let mut out = None;
    if kit::icon_key(cx, right - 44.0, y + 2.0, "plus") && cap != CAPS[CAPS.len() - 1] {
        out = Some(cap_step(cap, true));
    }
    let vt = Text::new(ty::BODY, t::INK).semi().center();
    let words = cap.map_or("No limit".to_string(), |g| format!("{g} GB"));
    cx.c.text(&words, right - 44.0 - 40.0, y + (h - vt.lh()) / 2.0, vt);
    if kit::icon_key(cx, right - 44.0 - 80.0 - 44.0, y + 2.0, "minus") && cap.is_some() {
        out = Some(cap_step(cap, false));
    }
    (h, out)
}

/// Your books' copy on the server: when it was made, Copy now, and Restore when there is one to
/// fetch. Returns its height.
fn books_part(cx: &mut Ctx, b: &Books, x: f32, y0: f32, w: f32, now_ms: i64, acts: &mut Vec<Act>) -> f32 {
    let mut y = y0;
    let body = Text::new(ty::UI, t::INK2).line(1.45);
    let small = Text::new(ty::CAPTION, t::INK3).line(1.4);
    cx.c.text("Your books", x, y, Text::new(ty::BODY, t::INK).semi());
    y += 26.0;
    y += cx.c.para("Your books, highlights, notes and places are copied to your server once a day, so a new phone gets them back.", x, y, w, body, 4) + 8.0;
    let state = if b.restoring {
        "Bringing your books back…".to_string()
    } else if b.busy {
        "Copying…".to_string()
    } else if let Some(at) = b.found {
        format!("Your server has a copy of your books made {} on another phone", made_words(at, now_ms))
    } else if b.copied.at_ms > 1 {
        let n = if b.copied.books == 1 { "1 book".to_string() } else { format!("{} books", b.copied.books) };
        format!("Copied {}, {n}", made_words(b.copied.at_ms, now_ms))
    } else {
        "Not copied yet".to_string()
    };
    y += cx.c.para(&state, x, y, w, small, 3) + 4.0;
    if b.too_big > 0 && !b.busy {
        let words = if b.too_big == 1 { "1 book is over 95 MB and stays on this phone only".to_string() } else { format!("{} books are over 95 MB and stay on this phone only", b.too_big) };
        y += cx.c.para(&words, x, y, w, small, 2) + 4.0;
    }
    if let Some(e) = b.error.filter(|_| !b.busy) {
        y += cx.c.para(e, x, y, w, Text::new(ty::CAPTION, t::WARN).line(1.4), 3) + 4.0;
    }
    y += 8.0;
    if b.confirm && !b.busy {
        let n = if b.here == 1 { "the book".to_string() } else { format!("the {} books", b.here) };
        y += cx.c.para(&format!("This replaces {n} on this phone, and their highlights and places, with the copy."), x, y, w, body, 3) + 10.0;
        let kw = (w - 10.0) / 2.0;
        if kit::primary(cx, x, y, kw, 44.0, "Replace", None) {
            acts.push(Act::RestoreBooks);
        }
        if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Keep mine", None) {
            acts.push(Act::KeepBooks);
        }
        return y + 44.0 - y0;
    }
    let kw = (w - 10.0) / 2.0;
    if b.found.is_some() {
        if kit::primary(cx, x, y, kw, 44.0, "Restore", Some("download")) && !b.busy {
            acts.push(Act::RestoreBooks);
        }
        if kit::plain(cx, x + kw + 10.0, y, kw, 44.0, "Use this phone's", None) && !b.busy {
            acts.push(Act::CopyBooks(true));
        }
    } else {
        if kit::plain(cx, x, y, kw, 44.0, if b.busy && !b.restoring { "Copying" } else { "Copy now" }, Some("upload")) && !b.busy {
            acts.push(Act::CopyBooks(false));
        }
        if b.copied.at_ms > 0 && kit::plain(cx, x + kw + 10.0, y, kw, 44.0, if b.restoring { "Restoring" } else { "Restore" }, None) && !b.busy {
            acts.push(Act::RestoreBooks);
        }
    }
    y + 44.0 - y0
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
    fn the_room_steps_through_its_sizes_from_no_limit() {
        assert_eq!(cap_step(None, true), Some(5));
        assert_eq!(cap_step(Some(5), false), None);
        assert_eq!(cap_step(Some(20), true), Some(30));
        assert_eq!(cap_step(Some(25), true), Some(50), "a size set elsewhere steps from the next one");
        assert_eq!(cap_step(Some(300), true), Some(300));
    }

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
