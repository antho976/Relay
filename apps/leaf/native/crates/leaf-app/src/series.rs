//! A series (design 42, page 17, "Series page, webtoon's layout"): Back, the bell as a Follow
//! pill, the info key and ⋯ along the top; the cover centred over a wash of its own colours; the
//! title, authors, three lines of the synopsis (the info key shows it all, with the facts), a few
//! tags; then two tabs, and a fixed key at the bottom that continues reading.
//!
//! - **Chapters**: "339 chapters · Newest ⇅" (remembered per series), then a row per chapter:
//!   its picture (made by the server, `/v1/chapters/{id}/thumb`; a quiet tile until then), with a
//!   small download key on it until the chapter is on this phone ([`crate::offline`]: a tap puts it
//!   in the line), "Ch. 339 · title", the day it came out or the page you are on, what this phone
//!   holds of it ("Saved", "Saving · 12 of 40 pages", "Waiting", "Didn't save · Retry"), and Read
//!   or Continue; read rows dimmed. Never what the server keeps: that is its own business. Smaller than drawn (the owner, 2026-11-23): 72 × 54, so more fit. A run of
//!   chapters no source has gets a row of its own. Newer episodes Webtoon sells ahead get a row at
//!   the newest end, "Chapters 9–23 are on Webtoon (paid)", whose Open hands the link to the
//!   Webtoon app (or the browser). No paid episode is ever fetched. A series only Webtoon has gets
//!   a row under it, "Tell me when another source has it": one notice when one does.
//! - **Readers also like**: AniList's recommendations from this series' readers that your sources
//!   carry, three across, "Yours" on ones in My space ([`crate::recs`]).
//!
//! The download key beside Follow opens the download choices (to this phone): Select chapters,
//! Select range…, the next 5, every unread one.
//!
//! **A preview** (a negative id, [`App::open_preview`]): a title from Search or the picks not added
//! yet, shown from `/v1/preview` with its chapters, Follow, the download key and info, no ⋯ and no
//! Readers also like. Nothing is added by opening it: reading a chapter, Follow or a download adds
//! it ([`App::adopt`]), and the page becomes the series' own.
//!
//! The bell is the follow: new chapters are announced while it is on. ⋯ holds Select chapters,
//! "Download next 5" and "Download all unread" (to this phone), the shelf it sits on, "Wrong series?" (the other
//! entries found for the work, to read from instead) and "Remove from My space" (behind a
//! confirm). Sources are never named.
//!
//! **Select**: a long press on a chapter (or ⋯ → Select chapters) selects, and dragging on without
//! lifting sweeps every chapter passed ([`crate::sweep`]); taps add or take
//! chapters away, and the bottom key becomes Download N / Delete N / Mark read (Delete removes
//! this phone's copies only). "Select range…" takes a first and last chapter, to select them or
//! download them all at once.
//!
//! Offline (no server, or it didn't answer) with chapters downloaded, the list is those chapters
//! alone, under an "Offline" note: the others can't be read.

use std::collections::BTreeSet;

use leaf_proto::{Chapter, SeriesDetail};

use crate::app::{App, Then};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::{CoverOf, thumb_key};
use crate::kit::{self, Kb};
use crate::logic::{self, Row};
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};

#[derive(Default)]
pub struct State {
    pub menu: bool,
    pub confirm: bool,
    /// Removing.
    pub busy: bool,
    pub error: Option<String>,
    /// Readers also like instead of Chapters.
    pub also: bool,
    /// The shelf sheet is open.
    pub shelf: bool,
    /// "Wrong series?" is open, and an entry was picked and is being moved to.
    pub alternatives: bool,
    pub repinning: bool,
    /// Selecting: the chapters selected (ids). None when not selecting.
    pub select: Option<BTreeSet<i64>>,
    /// "Select range…" is open, with what is typed in its two fields.
    pub range: bool,
    pub range_from: String,
    pub range_to: String,
    /// "I've read up to…" is open, with the chapter typed.
    pub upto: bool,
    pub upto_text: String,
    /// The synopsis and the facts (the info key).
    pub info: bool,
    /// The story under the title is open in full (More).
    pub about_open: bool,
    /// The cover, full screen ([`crate::zoom`]).
    pub zoom: Option<crate::zoom::Zoom>,
    /// The download choices (the key beside Follow).
    pub download: bool,
    /// Your rating's sheet.
    pub stars: bool,
}

impl State {
    /// Close the menu, a sheet or the dialog, if one is open; else stop selecting.
    pub fn close_over(&mut self) -> bool {
        let open = self.covered();
        if !open && self.select.take().is_some() {
            return true;
        }
        self.menu = false;
        self.confirm = false;
        self.shelf = false;
        self.alternatives = false;
        self.range = false;
        self.upto = false;
        self.zoom = None;
        self.info = false;
        self.download = false;
        self.stars = false;
        open
    }

    fn covered(&self) -> bool {
        self.menu || self.confirm || self.shelf || self.alternatives || self.range || self.upto || self.info || self.download || self.stars || self.zoom.is_some()
    }
}

/// A finger held this long on a row opens its sheet (or starts selecting).
pub const LONG_PRESS_NS: u64 = 450_000_000;

/// The sheet of shelves for a series on `on`: Some(Some(shelf)) for one picked, Some(None) for a
/// tap outside.
pub fn shelf_sheet(cx: &mut kit::Ctx, title: &str, on: logic::Shelf) -> Option<Option<logic::Shelf>> {
    let items: Vec<(&str, bool)> = logic::Shelf::ALL.iter().map(|s| (s.words(), *s == on)).collect();
    match kit::choices(cx, title, Some("Dropped series stay out of Home"), &items)? {
        i if i < logic::Shelf::ALL.len() => Some(Some(logic::Shelf::ALL[i])),
        _ => Some(None),
    }
}

/// Your rating: Some(Some(stars)) picked, Some(None) cleared, None still open; a tap outside is
/// Some(Some(current)).
fn stars_sheet(cx: &mut kit::Ctx, title: &str, on: Option<u8>) -> Option<Option<u8>> {
    let words = ["5 stars", "4 stars", "3 stars", "2 stars", "1 star"];
    let mut items: Vec<(&str, bool)> = words.iter().enumerate().map(|(i, w)| (*w, on == Some(5 - i as u8))).collect();
    if on.is_some() {
        items.push(("No rating", false));
    }
    match kit::choices(cx, title, Some("Yours alone: picks lean on it, one or two stars lead none"), &items)? {
        i if i < 5 => Some(Some(5 - i as u8)),
        5 => Some(None),
        _ => Some(on),
    }
}

/// The pictures saved from a series: a row of them under its story.
const PANEL_H: f32 = 112.0;
const PANEL_W: f32 = 84.0;

/// A chapter's row, its picture, and a gap's row.
pub const ROW_H: f32 = 70.0;
const GAP_H: f32 = 48.0;
pub const PIC: (f32, f32) = (72.0, 54.0);
/// The paid episodes' row.
const PAID_H: f32 = 70.0;
/// The row under it, for a series only Webtoon has: tell me when another source has it.
pub const WATCH_H: f32 = 56.0;
/// The fixed key at the bottom, and the band it sits on.
const KEY_H: f32 = 50.0;
const BAR_H: f32 = KEY_H + 24.0;
/// The poster.
const POSTER: (f32, f32) = (150.0, 212.0);

enum Act {
    Open(usize, u32),
    /// Download a chapter to this phone (or try it again).
    Download(i64),
    /// A tap on a chapter waiting or saving: it stops.
    Cancel(i64),
    Bell(bool),
    /// A tap while selecting (a long press sweeps, [`crate::sweep`]).
    Toggle(i64),
    Paid(String),
    /// Hear once when a source other than Webtoon has it.
    Watch(bool),
    /// Every chapter before this one read: the holes progress brought from elsewhere left.
    FillHoles(f64),
}

/// What the selection's bar asked for.
enum Batch {
    Download(Vec<i64>),
    Delete(Vec<i64>),
    Read(Vec<i64>, bool),
}

/// The selection's verb for marking: Mark read, unless every chapter selected is read already.
fn mark_read_verb(d: &SeriesDetail, sel: &BTreeSet<i64>) -> bool {
    !d.chapters.iter().filter(|c| sel.contains(&c.id)).all(|c| c.read)
}

pub fn draw(app: &mut App, id: i64, dt: f32) -> bool {
    if app.fresh {
        app.st.series = State::default();
        if !app.series.get(&id).is_some_and(|l| l.loading) {
            app.ask(Job::Series(id));
        }
    }
    // A title not added yet: nothing here asks the server to change anything until it is added.
    let preview = id < 0;
    // What the library knows stands in until the series itself arrives. Offline, the chapters
    // downloaded to this phone are the list.
    let saved_n = app.saver.saved_of(id).len();
    let saved_only = saved_n > 0 && app.offline_for(id);
    let detail: Option<SeriesDetail> = app.detail_for(id);
    let from_library = app.library.data.as_ref().and_then(|l| l.iter().find(|s| s.id == id).cloned());
    let loaded = app.series.get(&id);
    let (loading, error) = (loaded.is_some_and(|l| l.loading), loaded.and_then(|l| l.error.clone()));
    // A preview kept from a moment ago is not "offline": a failed ask says so in words.
    let offline = !preview && (saved_only || loaded.is_some_and(|l| l.offline()));
    let also_tab = app.st.series.also && !preview;
    if also_tab && !app.also.contains_key(&id) && app.server.is_some() {
        app.also.entry(id).or_default().loading = true;
        app.ask(Job::Also(id));
    }
    let also = app.also.get(&id).map(|l| (l.data.clone(), l.loading, l.error.clone()));
    let covered = app.st.series.covered() || app.st.recs.sheet.is_some();
    let oldest = app.oldest_first.contains(&id);
    let paid = app.paid.get(&id).cloned();
    let watch = app.watch.get(&id).copied();
    let selecting = app.st.series.select.clone();
    let extra = app.more.get(&id).cloned();
    let panels = if id > 0 { app.panels_of(id) } else { Vec::new() };
    let pg = app.page_begin(dt, 0.0);
    let now = app.now_ns;
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    // A pick not yet answered for: what the pick knows, so the page shows at once.
    let series = detail.as_ref().map(|d| d.series.clone()).or(from_library).or_else(|| app.previews.get(&id).map(|p| p.stand_in(id)));
    let x0 = PAD;
    let w = WIDTH - PAD * 2.0;
    let top = cx.c.inset_top;
    let mut of = CoverOf::Series(id);
    let mut has_cover = app.images.cover(cx.c, of, now);
    // A pick's own cover is on the phone already: it stands in until the entry's comes.
    if !has_cover
        && let Some(a) = app.previews.get(&id).and_then(|p| p.anilist)
        && app.images.cover(cx.c, CoverOf::Rec(a), now)
    {
        of = CoverOf::Rec(a);
        has_cover = true;
    }
    // A wash of the cover's own colours behind the poster, fading into the ground.
    let wash_h = top + 56.0 + POSTER.1 + 40.0;
    if cx.c.has_image(&of.banner_key()) {
        cx.c.image_cover(&of.banner_key(), -40.0, -40.0, WIDTH + 80.0, wash_h + 40.0, 0.0, Rgba::WHITE.a(0.42));
    }
    cx.c.rect_grad(-1000.0, wash_h * 0.35, WIDTH + 2000.0, wash_h * 0.65 + 1.0, 0.0, t::BG.a(0.0), t::BG);
    let (pw, ph) = POSTER;
    let (px, py) = ((WIDTH - pw) / 2.0, top + 56.0 + 6.0);
    cx.c.shadow(px, py + 10.0, pw, ph, COVER_R, 28.0, Rgba::hex(0x000000).a(0.55));
    cx.c.rect(px, py, pw, ph, COVER_R, t::S1);
    if has_cover {
        cx.c.image_cover(&of.key(), px, py, pw, ph, COVER_R, Rgba::WHITE);
    }
    cx.c.outline(px, py, pw, ph, COVER_R, 1.0, t::INK.a(0.08));
    // A tap on the cover: it, full screen.
    let zoom_cover = (has_cover && !covered && cx.tap(px, py, pw, ph)).then_some(of);
    let mut y = py + ph + 22.0;
    let mut act: Option<Act> = None;
    let mut sweep_rows: Vec<crate::sweep::Row> = Vec::new();
    let mut retry = false;
    let mut flip = false;
    let mut set_also = None;
    let mut open_info = false;
    let mut set_about: Option<bool> = None;
    let mut open_stars = false;
    let mut zoom_panel: Option<CoverOf> = None;
    let mut also_hit: Option<(crate::recs::Hit, Vec<leaf_proto::Rec>)> = None;
    let mut spinning_any = false;
    if let Some(s) = &series {
        let tt = Text::new(24.0, t::INK).brand().line(1.18);
        for l in cx.c.wrap(&s.title, w, tt, 3) {
            cx.c.text(&l, x0, y, tt);
            y += tt.lh();
        }
        // One line of readings: the source's rating and bookmarks, where it stands, your shelf.
        let about = detail.as_ref().and_then(|d| d.description.as_deref()).map(logic::about).or_else(|| app.previews.get(&id).and_then(|p| p.about.as_deref()).map(logic::about)).unwrap_or_default();
        y += 8.0;
        let mut items: Vec<(Option<&str>, String, bool)> = Vec::new();
        if let Some(r) = &about.rating {
            items.push((Some("star"), r.clone(), true));
        }
        if let Some(bk) = &about.bookmarks {
            items.push((None, format!("{bk} bookmarks"), false));
        }
        items.extend(logic::status_word(&s.status).map(|w| (None, w, false)));
        if s.shelf.is_some() {
            items.push((None, logic::shelf_of(s).words().to_string(), false));
        }
        // Yours, after the source's: "You gave 4", or "Rate" (a tap on the line opens the sheet).
        let rate = id > 0 && !preview;
        let mine = extra.as_ref().and_then(|m| m.stars);
        let line_y = y;
        if !items.is_empty() || rate {
            let mut mx = x0;
            let lh = Text::new(14.0, t::INK).lh();
            for (icon, words, strong) in &items {
                let tt = if *strong { Text::new(14.0, t::INK).semi() } else { Text::new(14.0, t::INK2) };
                if let Some(ic) = icon {
                    cx.c.icon(ic, mx + 7.0, y + tt.lh() / 2.0, 14.0, t::WARN);
                    mx += 18.0;
                }
                cx.c.text(words, mx, y, tt);
                mx += cx.c.measure(words, tt) + 14.0;
            }
            if rate {
                let (words, tt) = match mine {
                    Some(n) => (format!("You gave {n}"), Text::new(14.0, t::INK).semi()),
                    None => ("Rate".to_string(), Text::new(14.0, t::ACCENT).semi()),
                };
                let ww = 18.0 + cx.c.measure(&words, tt);
                // A line too full puts yours under it.
                if mx + ww > x0 + w && mx > x0 {
                    mx = x0;
                    y += lh + 6.0;
                }
                cx.c.icon("star", mx + 7.0, y + tt.lh() / 2.0, 14.0, if mine.is_some() { t::ACCENT } else { t::INK3 });
                cx.c.text(&words, mx + 18.0, y, tt);
            }
            y += lh;
            if rate && !covered && cx.tap(x0 - 8.0, line_y - 10.0, w + 16.0, y - line_y + 20.0) {
                open_stars = true;
            }
            // When its chapters come: the usual day, due today, or a break.
            if let Some(words) = extra.as_ref().and_then(|m| crate::rhythm::words(&m.uploads, &s.status, crate::shelf::now_ms(), crate::shelf::zone_offset())) {
                y += 6.0;
                cx.c.text(&words, x0, y, Text::new(13.0, t::INK3));
                y += Text::new(13.0, t::INK3).lh();
            }
            y += 16.0;
        } else {
            y += 8.0;
        }
        // The tagline, then the story: two lines and More, or all of it, a quote standing alone.
        if let Some(tg) = &about.tagline {
            y += cx.c.para(tg, x0, y, w, Text::new(15.0, t::INK).semi().line(1.4), 3) + 8.0;
        }
        if !about.paras.is_empty() {
            let bt = Text::new(15.0, t::INK2).line(1.6);
            let open = app.st.series.about_open;
            let start_y = y;
            if open {
                for p in &about.paras {
                    y += match p {
                        logic::Para::Text(tx) => cx.c.para(tx, x0, y, w, bt, 40),
                        logic::Para::Quote(q) => cx.c.para(q, x0, y, w, Text::new(17.0, t::INK).semi().line(1.4), 6),
                    } + 10.0;
                }
            } else {
                y += cx.c.para(&about.story(), x0, y, w, bt, 2) + 2.0;
            }
            let mt = Text::new(14.0, t::ACCENT).semi();
            cx.c.text(if open { "Less" } else { "More" }, x0, y, mt);
            y += mt.lh();
            if !covered && cx.tap(x0, start_y, w, y - start_y) {
                set_about = Some(!open);
            }
            if open {
                let names: Vec<String> = about.also.iter().map(|a| logic::showable(a, |c| cx.c.can_show(c))).filter(|a| !a.is_empty()).take(3).collect();
                if !names.is_empty() {
                    y += 14.0;
                    y += cx.c.para(&format!("Also known as {}", names.join(", ")), x0, y, w, Text::new(13.0, t::INK3).line(1.5), 3);
                }
            }
            y += 22.0;
        } else {
            y += 8.0;
        }
        // Pictures saved from it, newest first: a tap opens one full screen.
        if !panels.is_empty() {
            cx.c.text("SAVED PICTURES", x0, y, Text::new(ty::SMALL, t::INK3).semi());
            y += 22.0;
            let fit = ((w + 10.0) / (PANEL_W + 10.0)).floor().max(1.0) as usize;
            for (i, p) in panels.iter().take(fit).enumerate() {
                let px = x0 + i as f32 * (PANEL_W + 10.0);
                let pof = CoverOf::Panel(p.id);
                cx.c.rect(px, y, PANEL_W, PANEL_H, 8.0, t::S1);
                if app.images.cover(cx.c, pof, now) {
                    cx.c.image_cover(&pof.key(), px, y, PANEL_W, PANEL_H, 8.0, Rgba::WHITE);
                }
                if !covered && cx.tap(px, y, PANEL_W, PANEL_H) {
                    zoom_panel = Some(pof);
                }
            }
            if panels.len() > fit {
                let more_t = Text::new(12.5, t::INK3);
                cx.c.text(&format!("{} more", panels.len() - fit), x0, y + PANEL_H + 6.0, more_t);
                y += more_t.lh() + 6.0;
            }
            y += PANEL_H + 24.0;
        }
    }
    let mut cont: Option<logic::Continue> = None;
    match &detail {
        Some(d) => {
            cont = crate::places::resume(&app.places, d);
            if let Some(note) = &d.series.note {
                cx.c.icon("bolt", x0 + 8.0, y + 10.0, 16.0, t::WARN);
                y += cx.c.para(note, x0 + 26.0, y, w - 26.0, Text::new(ty::UI, t::WARN).line(1.4), 4) + 14.0;
            }
            if saved_only {
                let words = format!("Offline: showing the {} downloaded to this phone.", if saved_n == 1 { "one chapter".to_string() } else { format!("{saved_n} chapters") });
                cx.c.icon("wifi", x0 + 8.0, y + 10.0, 16.0, t::INK3);
                y += cx.c.para(&words, x0 + 26.0, y, w - 26.0, Text::new(ty::UI, t::INK2).line(1.4), 3) + 14.0;
            }
            if let Some(e) = error.as_ref().filter(|_| !offline) {
                y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            if let Some(e) = &app.st.series.error {
                y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            // Chapters | Readers also like (a preview has its chapters only).
            let tt = Text::new(15.5, t::INK3).medium();
            let mut tabx = x0;
            let tab_words: &[&str] = if preview { &["Chapters"] } else { &["Chapters", "Readers also like"] };
            for (i, words) in tab_words.iter().enumerate() {
                let on = (i == 1) == also_tab;
                let tw_ = cx.c.text(words, tabx, y + (42.0 - tt.lh()) / 2.0, if on { tt.color(t::INK).semi() } else { tt });
                if on {
                    cx.c.rect(tabx, y + 40.0, tw_, 2.0, 1.0, t::ACCENT);
                }
                if !covered && selecting.is_none() && cx.tap(tabx - 12.0, y, tw_ + 24.0, 42.0) {
                    set_also = Some(i == 1);
                }
                tabx += tw_ + 24.0;
            }
            cx.c.rect(-1000.0, y + 42.0, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
            y += 43.0;
            if also_tab {
                y += 14.0;
                let title = d.series.title.as_str();
                let line = format!("From readers of {title} on AniList; only what your sources carry.");
                y += cx.c.para(&line, x0, y, w, Text::new(12.5, t::INK3).line(1.4), 2) + 14.0;
                match &also {
                    Some((Some(list), _, _)) if !list.is_empty() => {
                        let (h, hit) = crate::recs::grid(&mut cx, &mut app.images, list, y, now, covered, false);
                        if let Some(h) = hit {
                            also_hit = Some((h, list.clone()));
                        }
                        y += h + 16.0;
                    }
                    Some((Some(_), _, _)) => {
                        y += 20.0;
                        let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Nothing to suggest yet", Some("Picks are being made from what readers of it liked; back in a few hours."), None);
                        y += h;
                    }
                    Some((None, _, Some(e))) => {
                        y += 20.0;
                        let (h, r) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, e, None, Some("Retry"));
                        if r {
                            retry = true;
                        }
                        y += h;
                    }
                    _ => {
                        let cw = (WIDTH - 2.0 * PAD - 20.0) / 3.0;
                        for i in 0..3 {
                            cx.c.rect(PAD + i as f32 * (cw + 10.0), y, cw, (cw * 1.3).round(), COVER_R, t::S1);
                        }
                        y += (cw * 1.3).round() + 60.0;
                    }
                }
            } else {
                y += 14.0;
                let n = d.chapters.len();
                let ht = Text::new(ty::UI, t::INK).semi();
                cx.c.text(&logic::chapters_words(n), x0, y, ht);
                let ot = Text::new(13.0, t::INK2).right();
                let words = if oldest { "Oldest" } else { "Newest" };
                let ow = cx.c.measure(words, ot);
                cx.c.text(words, WIDTH - PAD - 24.0, y + 1.0, ot);
                cx.c.icon("transfer", WIDTH - PAD - 8.0, y + ht.lh() / 2.0, 15.0, t::INK2);
                if !covered && cx.tap(WIDTH - PAD - 34.0 - ow, y - 12.0, ow + 34.0 + PAD, ht.lh() + 24.0) {
                    flip = true;
                }
                y += ht.lh() + 12.0;
                // Unread chapters before the furthest read: holes left by progress brought from
                // elsewhere, filled with one tap.
                if id > 0
                    && let Some((n, at)) = logic::holes(&d.chapters)
                {
                    cx.c.icon("check", x0 + PIC.0 / 2.0, y + WATCH_H / 2.0, 18.0, t::INK3);
                    let (kw, tapped) = kit::pill(&mut cx, WIDTH - PAD, y + (WATCH_H - 32.0) / 2.0, "Mark read", false);
                    let tx = x0 + PIC.0 + 14.0;
                    let tt = Text::new(ty::UI, t::INK2).line(1.3);
                    let words = format!("{} before {} {} unread", logic::chapters_words(n), logic::num(at), if n == 1 { "is" } else { "are" });
                    let lines = cx.c.wrap(&words, WIDTH - PAD - kw - 12.0 - tx, tt, 2);
                    let mut ty0 = y + (WATCH_H - lines.len() as f32 * tt.lh()) / 2.0;
                    for l in &lines {
                        cx.c.text(l, tx, ty0, tt);
                        ty0 += tt.lh();
                    }
                    if (tapped || cx.tap(0.0, y, WIDTH, WATCH_H)) && !covered && selecting.is_none() {
                        act = Some(Act::FillHoles(at));
                    }
                    y += WATCH_H;
                }
                let mut rows = logic::chapter_rows(&d.chapters, &d.series.gaps);
                if oldest {
                    rows.reverse();
                }
                let view_top = -cx.c.dy - 60.0;
                let view_bottom = -cx.c.dy + cx.c.h + 60.0;
                let current = cont.as_ref().map(|c| c.index);
                let mut thumbs: Vec<(i64, bool)> = Vec::new();
                // Paid episodes ahead: a row at the newest end.
                let paid_row = |cx: &mut kit::Ctx, y: f32, p: &leaf_proto::PaidAhead| -> Option<String> {
                    let (px, py) = (x0, y + (PAID_H - PIC.1) / 2.0);
                    cx.c.rect(px, py, PIC.0, PIC.1, 6.0, t::S2);
                    cx.c.icon("coins", px + PIC.0 / 2.0, py + PIC.1 / 2.0, 20.0, t::WARN);
                    let range = if p.from == p.to { format!("Chapter {}", logic::num(p.from)) } else { format!("Chapters {}–{}", logic::num(p.from), logic::num(p.to)) };
                    let (kw, tapped) = kit::pill(cx, WIDTH - PAD, y + (PAID_H - 32.0) / 2.0, "Open", false);
                    let tx = px + PIC.0 + 14.0;
                    let tw = WIDTH - PAD - kw - 12.0 - tx;
                    let title_t = Text::new(14.5, t::INK).semi().line(1.3);
                    let lines = cx.c.wrap(&format!("{range} are on Webtoon (paid)"), tw, title_t, 2);
                    let mut ty0 = y + (PAID_H - lines.len() as f32 * title_t.lh()) / 2.0;
                    for l in &lines {
                        cx.c.text(l, tx, ty0, title_t);
                        ty0 += title_t.lh();
                    }
                    (tapped || cx.tap(0.0, y, WIDTH, PAID_H)).then(|| p.url.clone())
                };
                // Only Webtoon has it: a row to hear when another source does.
                let watch_row = |cx: &mut kit::Ctx, y: f32, on: bool| -> bool {
                    cx.c.icon(if on { "bell-on" } else { "bell" }, x0 + PIC.0 / 2.0, y + WATCH_H / 2.0, 18.0, if on { t::ACCENT } else { t::INK3 });
                    let (kw, tapped) = kit::pill(cx, WIDTH - PAD, y + (WATCH_H - 32.0) / 2.0, if on { "Watching" } else { "Watch" }, on);
                    let tx = x0 + PIC.0 + 14.0;
                    let tt = Text::new(ty::UI, t::INK2).line(1.3);
                    let lines = cx.c.wrap("Tell me when another source has it", WIDTH - PAD - kw - 12.0 - tx, tt, 2);
                    let mut ty0 = y + (WATCH_H - lines.len() as f32 * tt.lh()) / 2.0;
                    for l in &lines {
                        cx.c.text(l, tx, ty0, tt);
                        ty0 += tt.lh();
                    }
                    tapped || cx.tap(0.0, y, WIDTH, WATCH_H)
                };
                if !oldest && let Some(p) = &paid {
                    if let Some(u) = paid_row(&mut cx, y, p).filter(|_| !covered && selecting.is_none()) {
                        act = Some(Act::Paid(u));
                    }
                    y += PAID_H;
                }
                if !oldest && let Some(on) = watch {
                    if watch_row(&mut cx, y, on) && !covered && selecting.is_none() {
                        act = Some(Act::Watch(!on));
                    }
                    y += WATCH_H;
                }
                for row in rows {
                    let h = if matches!(row, Row::Gap(_)) { GAP_H } else { ROW_H };
                    if let Row::Chapter(i) = &row {
                        sweep_rows.push(crate::sweep::Row { id: d.chapters[*i].id, y, h });
                    }
                    if y + h < view_top || y > view_bottom {
                        y += h;
                        continue;
                    }
                    match row {
                        Row::Gap(g) => {
                            let gt = Text::new(ty::CAPTION, t::INK3);
                            cx.c.rect(x0, y + h / 2.0, 10.0, 1.0, 0.0, t::INK3.a(0.6));
                            cx.c.text_fit(&logic::gap_words(&g), x0 + 18.0, y + (h - gt.lh()) / 2.0, w - 18.0, gt);
                        }
                        Row::Chapter(i) => {
                            let c = &d.chapters[i];
                            let picked = selecting.as_ref().is_some_and(|s| s.contains(&c.id));
                            if picked {
                                cx.c.rect(0.0, y, WIDTH, h, 0.0, t::ACCENT_DIM);
                            }
                            if cx.pressed(0.0, y, WIDTH, h) {
                                cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
                            }
                            let here = current == Some(i) && !(c.read && cont.as_ref().is_some_and(|k| k.label.starts_with("Read")));
                            let dim = c.read && !here;
                            // The picture: the server's, once it has one; a quiet tile until then. On it, what
                            // this phone holds: a download key, a turning arc while it comes, a check once here.
                            let (px, py) = (x0, y + (h - PIC.1) / 2.0);
                            let kept = app.saver.state(c.id);
                            let has_pic = app.thumbs.contains(&c.id);
                            if has_pic || c.downloaded {
                                thumbs.push((c.id, has_pic));
                            }
                            let drawn = has_pic && cx.c.image_cover(&thumb_key(c.id), px, py, PIC.0, PIC.1, 6.0, Rgba::WHITE.a(if dim { 0.45 } else { 1.0 }));
                            if !drawn {
                                cx.c.rect(px, py, PIC.0, PIC.1, 6.0, t::S2);
                            }
                            let (bx, by) = (px + PIC.0 - 13.0, py + PIC.1 - 13.0);
                            let fade = if dim { 0.6 } else { 1.0 };
                            match &kept {
                                crate::offline::Kept::Saved => {
                                    cx.c.circle(bx, by, 10.0, t::ACCENT);
                                    cx.c.icon("check", bx, by, 12.0, t::ON_ACCENT);
                                }
                                crate::offline::Kept::Waiting | crate::offline::Kept::Saving(..) => {
                                    spinning_any = true;
                                    cx.c.circle(bx, by, 10.0, t::BG.a(0.75));
                                    let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
                                    cx.c.arc(bx, by, 6.0, 1.8, turn, turn + 0.7, t::INK2, false);
                                    // A tap on the picture stops it: the cross says so.
                                    cx.c.icon("close", bx, by, 7.0, t::INK2);
                                }
                                crate::offline::Kept::Failed(_) => {
                                    cx.c.circle(bx, by, 10.0, t::BG.a(0.75));
                                    cx.c.icon("flag", bx, by, 11.0, t::WARN);
                                }
                                crate::offline::Kept::None => {
                                    cx.c.circle(bx, by, 10.0, t::BG.a(0.75));
                                    cx.c.icon("download", bx, by, 12.0, t::INK.a(fade));
                                }
                            }
                            let tx = px + PIC.0 + 14.0;
                            // At the right: the selection's circle, else Continue or Read.
                            let ky = y + (h - 32.0) / 2.0;
                            let mut key_w = 24.0;
                            let mut key_tap = false;
                            if selecting.is_some() {
                                let (cxx, cyy) = (WIDTH - PAD - 11.0, y + h / 2.0);
                                if picked {
                                    cx.c.circle(cxx, cyy, 11.0, t::ACCENT);
                                    cx.c.icon("check", cxx, cyy, 13.0, t::ON_ACCENT);
                                } else {
                                    cx.c.outline(cxx - 11.0, cyy - 11.0, 22.0, 22.0, 11.0, 1.5, t::INK3);
                                }
                            } else if here {
                                let kt = Text::new(ty::UI, t::ON_ACCENT).semi();
                                let kw = cx.c.measure("Continue", kt) + 28.0;
                                let kx = WIDTH - PAD - kw;
                                let pressed = cx.pressed(kx, ky, kw, 32.0);
                                cx.c.rect(kx, ky, kw, 32.0, 16.0, if pressed { t::ACCENT.mix(Rgba::hex(0x000000), 0.12) } else { t::ACCENT });
                                cx.c.text("Continue", kx + 14.0, ky + (32.0 - kt.lh()) / 2.0, kt);
                                key_w = kw;
                            } else {
                                let (kw, tapped) = kit::pill(&mut cx, WIDTH - PAD, ky, "Read", dim);
                                key_w = kw;
                                key_tap = tapped;
                            }
                            let titled = logic::chapter_title(&c.name, c.number);
                            let title = match &titled {
                                Some(n) => format!("Ch. {} · {n}", logic::num(c.number)),
                                None => format!("Chapter {}", logic::num(c.number)),
                            };
                            let tw = WIDTH - PAD - key_w - 12.0 - tx;
                            let title_t = Text::new(14.5, if dim { t::INK3 } else { t::INK }).semi();
                            let line_t = Text::new(12.5, t::INK3);
                            let block = title_t.lh() + 3.0 + line_t.lh();
                            let ty0 = y + (h - block) / 2.0;
                            cx.c.text_fit(&title, tx, ty0, tw, title_t);
                            let ly = ty0 + title_t.lh() + 3.0;
                            let mut lx = tx;
                            let mut line: Vec<String> = Vec::new();
                            // What this phone holds says more than the day while it comes.
                            let state: Option<(String, Rgba)> = match &kept {
                                crate::offline::Kept::None => None,
                                crate::offline::Kept::Saved => Some(("Saved".into(), t::INK3)),
                                crate::offline::Kept::Waiting => Some(("Waiting".into(), t::INK3)),
                                crate::offline::Kept::Saving(done, all) => Some((format!("Saving · {done} of {all} pages"), t::ACCENT)),
                                crate::offline::Kept::Failed(_) => Some(("Didn't save · Retry".into(), t::WARN)),
                            };
                            let busy = matches!(kept, crate::offline::Kept::Saving(..) | crate::offline::Kept::Waiting | crate::offline::Kept::Failed(_));
                            if !c.read && c.last_page > 0 && !busy {
                                // Left part way: the page, in the accent.
                                let lead = format!("page {} of {}", c.last_page + 1, c.pages.max(c.last_page + 1));
                                lx += cx.c.text_fit(&lead, lx, ly, tw, line_t.color(t::ACCENT));
                            } else if c.uploaded_at > 0 && !busy {
                                line.push(logic::local_date(c.uploaded_at));
                            }
                            let rest = line.join(" · ");
                            if !rest.is_empty() {
                                if lx > tx {
                                    lx += cx.c.text(" · ", lx, ly, line_t);
                                }
                                lx += cx.c.text_fit(&rest, lx, ly, (tx + tw - lx).max(0.0), line_t);
                            }
                            if let Some((words, ink)) = state {
                                if lx > tx {
                                    lx += cx.c.text(" · ", lx, ly, line_t);
                                }
                                cx.c.text_fit(&words, lx, ly, (tx + tw - lx).max(0.0), line_t.color(ink));
                            }
                            if selecting.is_some() {
                                if !covered && cx.tap(0.0, y, WIDTH, h) {
                                    act = Some(Act::Toggle(c.id));
                                }
                            } else if !covered {
                                // The picture downloads (or tries again) what isn't on this phone; so does
                                // "Retry" on the line.
                                let can_get = matches!(kept, crate::offline::Kept::None | crate::offline::Kept::Failed(_));
                                let retry_tap = matches!(kept, crate::offline::Kept::Failed(_)) && cx.tap(tx, ly - 8.0, tw, line_t.lh() + 16.0);
                                let pic_tap = can_get && cx.tap(px - 6.0, py - 6.0, PIC.0 + 12.0, PIC.1 + 12.0);
                                let busy_now = matches!(kept, crate::offline::Kept::Waiting | crate::offline::Kept::Saving(..));
                                if pic_tap || retry_tap {
                                    act = Some(Act::Download(c.id));
                                } else if busy_now && cx.tap(px - 6.0, py - 6.0, PIC.0 + 12.0, PIC.1 + 12.0) {
                                    act = Some(Act::Cancel(c.id));
                                } else if key_tap || cx.tap(0.0, y, WIDTH, h) {
                                    let page = if !c.read { logic::resume_page(c) } else { 0 };
                                    act = Some(Act::Open(i, page));
                                }
                            }
                        }
                    }
                    y += h;
                }
                if oldest && let Some(p) = &paid {
                    if let Some(u) = paid_row(&mut cx, y, p).filter(|_| !covered && selecting.is_none()) {
                        act = Some(Act::Paid(u));
                    }
                    y += PAID_H;
                }
                if oldest && let Some(on) = watch {
                    if watch_row(&mut cx, y, on) && !covered && selecting.is_none() {
                        act = Some(Act::Watch(!on));
                    }
                    y += WATCH_H;
                }
                app.images.want_thumbs(cx.c, &thumbs, now);
            }
        }
        None if loading => {
            cx.c.rect(x0, y, w, 50.0, t::KEY_R, t::S1);
            y += 68.0;
        }
        None => {
            y += 30.0;
            let words = error.clone().unwrap_or_else(|| "This series didn't load".into());
            let (h, r) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &words, None, Some("Retry"));
            retry = r;
            y += h;
        }
    }
    let bottom = y + BAR_H;
    let page_dy = cx.c.dy;
    // The fixed bottom: Continue, or what to do with the chapters selected.
    let mut batch: Option<Batch> = None;
    let mut open_cont: Option<(usize, u32)> = None;
    if let Some(d) = &detail {
        let screen_bottom = cx.c.h - cx.c.inset_bottom;
        let band_y = screen_bottom - BAR_H;
        cx.c.dy = 0.0;
        cx.c.rect_grad(-1000.0, band_y - 20.0, WIDTH + 2000.0, 20.0, 0.0, t::BG.a(0.0), t::BG);
        cx.c.rect(-1000.0, band_y, WIDTH + 2000.0, BAR_H + cx.c.inset_bottom, 0.0, t::BG);
        let band_h = BAR_H + cx.c.inset_bottom;
        kit::chrome(&mut cx, -1000.0, band_y, WIDTH + 2000.0, band_h);
        let ky = band_y + 8.0;
        match &selecting {
            Some(sel) => {
                let n = sel.len();
                let ids: Vec<i64> = d.chapters.iter().filter(|c| sel.contains(&c.id)).map(|c| c.id).collect();
                let to_get: Vec<i64> = d.chapters.iter().filter(|c| sel.contains(&c.id) && matches!(app.saver.state(c.id), crate::offline::Kept::None | crate::offline::Kept::Failed(_))).map(|c| c.id).collect();
                let to_drop: Vec<i64> = d.chapters.iter().filter(|c| sel.contains(&c.id) && app.saver.state(c.id) != crate::offline::Kept::None).map(|c| c.id).collect();
                let read = mark_read_verb(d, sel);
                let gw = (w - 16.0) / 3.0;
                let get_words = if to_get.is_empty() { "Download".to_string() } else { format!("Download {}", to_get.len()) };
                let drop_words = if to_drop.is_empty() { "Delete".to_string() } else { format!("Delete {}", to_drop.len()) };
                let mark_words = if read { "Mark read" } else { "Mark unread" };
                // A preview has nothing on this phone, and marks nothing read until it is added.
                for (k, (words, live)) in [(get_words.as_str(), !to_get.is_empty()), (drop_words.as_str(), !to_drop.is_empty() && !preview), (mark_words, n > 0 && !preview)].iter().enumerate() {
                    let kx = x0 + k as f32 * (gw + 8.0);
                    let tapped = if !live {
                        cx.c.rect(kx, ky, gw, KEY_H, t::KEY_R, t::S1);
                        let tt = Text::new(ty::UI, t::INK3).semi().center();
                        cx.c.text_fit(words, kx + gw / 2.0, ky + (KEY_H - tt.lh()) / 2.0, gw - 12.0, tt);
                        false
                    } else if k == 0 {
                        kit::primary(&mut cx, kx, ky, gw, KEY_H, words, None)
                    } else {
                        kit::plain(&mut cx, kx, ky, gw, KEY_H, words, None)
                    };
                    if tapped && !covered {
                        batch = Some(match k {
                            0 => Batch::Download(to_get.clone()),
                            1 => Batch::Delete(to_drop.clone()),
                            _ => Batch::Read(ids.clone(), read),
                        });
                    }
                }
            }
            None => {
                if let Some(c) = &cont {
                    let label = c.label.replace("ch. ", "chapter ");
                    if kit::primary(&mut cx, x0, ky, w, KEY_H, &label, None) && !covered {
                        open_cont = Some((c.index, c.page));
                    }
                }
            }
        }
    }
    // The fixed top: Back, the bell as Follow, info and ⋯ (or, selecting: how many, and the range).
    let mut back = false;
    let mut more = false;
    let mut open_download = false;
    let mut stop_select = false;
    let mut to_range = false;
    {
        let scrolled = (-page_dy).clamp(0.0, 60.0) / 60.0;
        cx.c.dy = 0.0;
        let band = if selecting.is_some() { 1.0 } else { scrolled };
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG.a(band));
        kit::fixed_top(&mut cx, 56.0);
        match &selecting {
            Some(sel) => {
                if kit::icon_key(&mut cx, 6.0, top + 6.0, "close") {
                    stop_select = true;
                }
                let tt = Text::new(ty::SECTION, t::INK).semi();
                let words = if sel.is_empty() { "Tap chapters to select".to_string() } else { format!("{} selected", sel.len()) };
                cx.c.text_fit(&words, 56.0, top + 6.0 + (44.0 - tt.lh()) / 2.0, WIDTH - 56.0 - 140.0, tt);
                let rt = Text::new(ty::UI, t::INK).semi().right();
                let rw = cx.c.measure("Select range", rt);
                if cx.pressed(WIDTH - PAD - rw - 12.0, top + 8.0, rw + 24.0, 40.0) {
                    cx.c.rect(WIDTH - PAD - rw - 12.0, top + 8.0, rw + 24.0, 40.0, 8.0, t::WASH);
                }
                cx.c.text("Select range", WIDTH - PAD, top + 6.0 + (44.0 - rt.lh()) / 2.0, rt);
                if cx.ui.tapped(WIDTH - PAD - rw - 14.0, top + 4.0, rw + 14.0 + PAD, 48.0) {
                    to_range = true;
                }
            }
            None => {
                // Back at the left; at the right ⋯ (not on a preview), info, the download key and
                // Follow. Over the picture the keys sit on a soft dark disc, so a bright cover
                // doesn't swallow them.
                let keys = if preview { 2.0 } else { 3.0 };
                let (info_x, get_x) = (WIDTH - 6.0 - 44.0 * (keys - 1.0), WIDTH - 6.0 - 44.0 * keys);
                for kx in [6.0, get_x, info_x].into_iter().chain((!preview).then_some(WIDTH - 6.0 - 44.0)) {
                    cx.c.circle(kx + 22.0, top + 28.0, 18.0, t::BG.a(0.32 * (1.0 - scrolled)));
                }
                if kit::icon_key(&mut cx, 6.0, top + 6.0, "arrow-left") {
                    back = true;
                }
                if let Some(s) = &series {
                    // Follow: the bell, as a pill.
                    let (words, glyph, ink) = if s.bell { ("Following", "bell-on", t::ACCENT) } else { ("Follow", "bell", t::INK) };
                    let ft = Text::new(13.5, t::INK).medium();
                    let fw = cx.c.measure(words, ft) + 44.0;
                    let (fx, fy, fh) = (get_x - 4.0 - fw, top + 11.0, 34.0);
                    cx.c.rect(fx, fy, fw, fh, fh / 2.0, if s.bell { t::ACCENT_DIM } else { t::BG.a(0.45) });
                    if !s.bell {
                        cx.c.outline(fx, fy, fw, fh, fh / 2.0, 1.0, t::INK.a(0.3));
                    }
                    if cx.pressed(fx, fy, fw, fh) {
                        cx.c.rect(fx, fy, fw, fh, fh / 2.0, t::WASH);
                    }
                    cx.c.icon(glyph, fx + 20.0, fy + fh / 2.0, 15.0, ink);
                    cx.c.text(words, fx + 32.0, fy + (fh - ft.lh()) / 2.0, ft);
                    if cx.tap(fx - 4.0, fy - 6.0, fw + 8.0, fh + 12.0) {
                        act = Some(Act::Bell(!s.bell));
                    }
                }
                if detail.is_some() && kit::icon_key(&mut cx, get_x, top + 6.0, "download") {
                    open_download = true;
                }
                if kit::icon_key(&mut cx, info_x, top + 6.0, "info") {
                    open_info = true;
                }
                if !preview && kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 6.0, "more") {
                    more = true;
                }
                if offline && scrolled <= 0.5 {
                    kit::tag(cx.c, 58.0, top + 18.0, "Offline");
                }
            }
        }
    }
    // Over the page: the menu, the sheets, the confirm.
    let st = &mut app.st.series;
    let mut remove = false;
    let mut next5 = false;
    let mut all_unread = false;
    let mut new_shelf: Option<logic::Shelf> = None;
    let mut ask_alternatives = false;
    let mut repin: Option<i64> = None;
    let mut range_act: Option<(f64, f64, bool)> = None;
    let mut upto_act: Option<f64> = None;
    let mut rec_act = None;
    let on_shelf = series.as_ref().map_or(logic::Shelf::Plan, logic::shelf_of);
    let mut removed_panel: Option<i64> = None;
    let mut new_stars: Option<Option<u8>> = None;
    if covered {
        kit::release(&mut cx, held);
        if let Some(z) = &mut st.zoom {
            if crate::zoom::draw(&mut cx, &mut app.images, z, now) {
                if let (true, CoverOf::Panel(p)) = (z.removed, z.of) {
                    removed_panel = Some(p);
                }
                st.zoom = None;
            }
        } else if st.stars {
            let title = series.as_ref().map_or("This series", |s| s.title.as_str());
            if let Some(picked) = stars_sheet(&mut cx, title, extra.as_ref().and_then(|m| m.stars)) {
                st.stars = false;
                if picked != extra.as_ref().and_then(|m| m.stars) {
                    new_stars = Some(picked);
                }
            }
        } else if let Some(s) = &app.st.recs.sheet {
            rec_act = crate::recs::sheet(&mut cx, &mut app.images, s, now);
        } else if st.menu {
            let shelf_words = format!("Shelf: {}", on_shelf.words());
            match kit::menu(&mut cx, top + 52.0, &["Select chapters", "I've read up to…", "Download next 5", "Download all unread", &shelf_words, "Wrong series?", "Remove from My space"]) {
                Some(0) => {
                    st.menu = false;
                    st.select = Some(BTreeSet::new());
                }
                Some(1) => {
                    st.menu = false;
                    st.upto = true;
                    // Filled in with the furthest chapter read.
                    st.upto_text = detail.as_ref().and_then(|d| d.chapters.iter().filter(|c| c.read).map(|c| c.number).reduce(f64::max)).map(logic::num).unwrap_or_default();
                }
                Some(2) => {
                    st.menu = false;
                    next5 = true;
                }
                Some(3) => {
                    st.menu = false;
                    all_unread = true;
                }
                Some(4) => {
                    st.menu = false;
                    st.shelf = true;
                }
                Some(5) => {
                    st.menu = false;
                    st.alternatives = true;
                    ask_alternatives = true;
                }
                Some(6) => {
                    st.menu = false;
                    st.confirm = true;
                }
                Some(_) => st.menu = false,
                None => {}
            }
        } else if st.shelf {
            let title = series.as_ref().map_or("This series", |s| s.title.as_str());
            if let Some(picked) = shelf_sheet(&mut cx, title, on_shelf) {
                st.shelf = false;
                new_shelf = picked.filter(|p| *p != on_shelf);
            }
        } else if st.range {
            match range_sheet(&mut cx, &mut st.range_from, &mut st.range_to) {
                Some(Some(r)) => {
                    st.range = false;
                    range_act = Some(r);
                }
                Some(None) => st.range = false,
                None => {}
            }
        } else if st.upto {
            match upto_sheet(&mut cx, &mut st.upto_text) {
                Some(Some(n)) => {
                    st.upto = false;
                    upto_act = Some(n);
                }
                Some(None) => st.upto = false,
                None => {}
            }
        } else if st.info {
            if info_sheet(&mut cx, detail.as_ref()) {
                st.info = false;
            }
        } else if st.download {
            let items = [("Select chapters", false), ("Select range…", false), ("The next 5 chapters", false), ("Every unread chapter", false)];
            match kit::choices(&mut cx, "Download to this phone", Some("Kept here, to read with no connection"), &items) {
                Some(0) => {
                    st.download = false;
                    st.select = Some(BTreeSet::new());
                }
                Some(1) => {
                    st.download = false;
                    to_range = true;
                }
                Some(2) => {
                    st.download = false;
                    next5 = true;
                }
                Some(3) => {
                    st.download = false;
                    all_unread = true;
                }
                Some(_) => st.download = false,
                None => {}
            }
        } else if st.alternatives {
            let alts = app.alternatives.get(&id);
            match alternatives_sheet(&mut cx, alts, st.repinning, now) {
                Some(Some(m)) => repin = Some(m),
                Some(None) => st.alternatives = false,
                None => {}
            }
        } else if st.confirm {
            let name = series.as_ref().map(|s| s.title.as_str()).unwrap_or("this series");
            let body = "It leaves My space, and your place in it is forgotten. Chapters downloaded to this phone stay until you delete them in Downloads. You can add it again from Search.";
            match kit::confirm(&mut cx, &format!("Remove {name}?"), body, "Remove") {
                Some(true) => {
                    st.confirm = false;
                    remove = true;
                }
                Some(false) => st.confirm = false,
                None => {}
            }
        }
    } else if more {
        st.menu = true;
    } else if open_download {
        st.download = true;
    } else if open_info {
        st.info = true;
    } else if open_stars {
        st.stars = true;
    } else if let Some(of) = zoom_panel {
        st.zoom = Some(crate::zoom::Zoom::new(of));
    } else if stop_select {
        st.select = None;
    }
    if let Some(o) = set_about {
        st.about_open = o;
    }
    if let Some(of) = zoom_cover {
        st.zoom = Some(crate::zoom::Zoom::new(of));
    }
    if to_range {
        st.range = true;
        // Filled in from where you are: the first unread chapter to the newest.
        if let Some(d) = &detail {
            let from = d.chapters.iter().find(|c| !c.read).or(d.chapters.first()).map(|c| c.number);
            st.range_from = from.map(logic::num).unwrap_or_default();
            st.range_to = d.chapters.last().map(|c| logic::num(c.number)).unwrap_or_default();
        }
    }
    if let Some(a) = set_also {
        st.also = a;
    }
    let waiting_alts = st.alternatives && app.alternatives.get(&id).is_some_and(|l| l.loading) || st.repinning;
    app.page_end(&pg, bottom);
    // A long press and drag on the chapters selects (or lets go of) every one it passes; not while
    // Readers also like shows.
    let swept = if also_tab {
        false
    } else {
        let band = (app.canvas.inset_top + 56.0, app.canvas.h - app.canvas.inset_bottom - BAR_H);
        app.sweep_rows(sweep_rows, band, covered, dt, |st| &mut st.series.select)
    };
    if let Some(a) = rec_act {
        crate::recs::act(app, a);
    }
    if let Some((h, list)) = also_hit.filter(|_| !covered) {
        crate::recs::on_hit(app, h, &list, None);
    }
    if waiting_alts {
        // The spinner turns while every source is asked.
        app.wake_at(now + 50_000_000);
    }
    if ask_alternatives && !app.alternatives.get(&id).is_some_and(|l| l.loading || l.data.is_some()) {
        app.ask(Job::Alternatives(id));
    }
    if let Some(m) = repin {
        app.st.series.repinning = true;
        app.ask(Job::Repin(id, m));
        if app.shot {
            app.st.series.repinning = false;
        }
    }
    if let Some(sh) = new_shelf {
        app.set_shelf(id, sh);
    }
    if let Some(n) = new_stars {
        app.set_stars(id, n);
    }
    if let Some(p) = removed_panel {
        crate::panels::remove(&app.data, p);
        if let Some(all) = app.panels.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            all.retain(|x| x.id != p);
        }
        app.toast("Removed from Leaf; your gallery keeps its copy");
    }
    if let Some(n) = upto_act {
        app.read_upto(id, n);
        app.toast(&format!("Read up to chapter {}", logic::num(n)));
    }
    if let Some((from, to, download)) = range_act
        && let Some(d) = &detail
    {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        let ids: Vec<i64> = d.chapters.iter().filter(|c| c.number >= lo && c.number <= hi).map(|c| c.id).collect();
        if ids.is_empty() {
            app.toast(&format!("No chapters from {} to {}", logic::num(lo), logic::num(hi)));
        } else if download {
            let get: Vec<Chapter> = d.chapters.iter().filter(|c| c.number >= lo && c.number <= hi).cloned().collect();
            get_chapters(app, id, &get);
            app.st.series.select = None;
        } else {
            app.st.series.select = Some(ids.into_iter().collect());
        }
    }
    match batch {
        Some(Batch::Download(ids)) => {
            if let Some(d) = &detail {
                let get: Vec<Chapter> = d.chapters.iter().filter(|c| ids.contains(&c.id)).cloned().collect();
                get_chapters(app, id, &get);
            }
            app.st.series.select = None;
        }
        Some(Batch::Delete(ids)) => {
            let (n, bytes) = app.saver.delete(&ids);
            app.toast(&format!("Deleted {} from this phone{}", logic::chapters_words(n), if bytes > 0 { format!(" · {} freed", crate::space::size_words(bytes)) } else { String::new() }));
            app.st.series.select = None;
        }
        Some(Batch::Read(ids, read)) => {
            mark_many(app, id, &ids, read);
            app.st.series.select = None;
        }
        None => {}
    }
    if spinning_any {
        // The arcs turn, and the pages counted show as they come.
        app.wake_at(now + 50_000_000);
    }
    if flip {
        app.flip_order(id);
    }
    if remove {
        app.st.series.busy = true;
        app.st.series.error = None;
        app.ask(Job::Unfollow(id));
    }
    if let Some(d) = &detail {
        if next5 {
            let ids = logic::next_to_download(&d.chapters, 5, |c| app.saver.state(c) != crate::offline::Kept::None);
            if ids.is_empty() {
                app.toast("The next 5 chapters are on this phone already");
            } else {
                let get: Vec<Chapter> = d.chapters.iter().filter(|c| ids.contains(&c.id)).cloned().collect();
                get_chapters(app, id, &get);
            }
        }
        if all_unread {
            let get: Vec<Chapter> = d.chapters.iter().filter(|c| !c.read).cloned().collect();
            if get.is_empty() {
                app.toast("Every chapter is read");
            } else {
                get_chapters(app, id, &get);
            }
        }
    }
    if retry {
        if also_tab {
            app.also.remove(&id);
        } else {
            app.ask(Job::Series(id));
        }
    }
    match act {
        Some(_) if covered => {}
        Some(Act::Open(index, page)) => {
            if let Some(d) = &detail {
                read_chapter(app, id, d, index, page);
            }
        }
        Some(Act::Download(c)) => {
            if let Some(ch) = detail.as_ref().and_then(|d| d.chapters.iter().find(|k| k.id == c).cloned()) {
                get_chapters(app, id, &[ch]);
            }
        }
        Some(Act::Cancel(c)) => {
            app.saver.delete(&[c]);
            app.toast("Download stopped");
        }
        // A preview's Follow adds it, its bell on.
        Some(Act::Bell(on)) if preview => {
            if on {
                app.adopt(id, Then::Follow);
            }
        }
        Some(Act::Bell(on)) => app.set_bell(id, on),
        Some(Act::Toggle(c)) => {
            if let Some(sel) = &mut app.st.series.select
                && !sel.remove(&c)
            {
                sel.insert(c);
            }
        }
        Some(Act::Watch(on)) => app.set_watch(id, on),
        Some(Act::FillHoles(at)) => {
            let n = detail.as_ref().and_then(|d| logic::holes(&d.chapters)).map_or(0, |h| h.0);
            app.read_before(id, at);
            app.toast(&format!("{} marked read", logic::chapters_words(n)));
        }
        Some(Act::Paid(url)) => {
            if !app.host.open_url(&url) {
                app.toast("Nothing on this phone opens that link");
            }
        }
        None => {
            if let Some((index, page)) = open_cont.filter(|_| !covered)
                && let Some(d) = &detail
            {
                read_chapter(app, id, d, index, page);
            } else if back && !covered {
                app.pop();
            }
        }
    }
    pg.moving || swept || loading && !crate::wake::has()
}

/// Download chapters of the series shown to this phone; a preview is added first, and its
/// chapters downloaded by number once it is in.
fn get_chapters(app: &mut App, id: i64, chapters: &[Chapter]) {
    if id < 0 {
        app.adopt(id, Then::Download(chapters.iter().map(|c| c.number).collect()));
    } else {
        app.download(id, chapters);
    }
}

/// Read a chapter of the series shown; a preview is added first, and the chapter opens once it is.
fn read_chapter(app: &mut App, id: i64, d: &SeriesDetail, index: usize, page: u32) {
    if id < 0 {
        if let Some(c) = d.chapters.get(index) {
            app.adopt(id, Then::Read(c.number, page));
        }
    } else {
        app.open_chapter(d, index, page);
    }
}

/// Mark chapters read or unread, at once on the phone. A run from the first chapter is one request
/// ("everything before"); any other selection is one per chapter.
fn mark_many(app: &mut App, series: i64, ids: &[i64], read: bool) {
    let Some(d) = app.series.get(&series).and_then(|l| l.data.clone()) else { return };
    let sel: BTreeSet<i64> = ids.iter().copied().collect();
    let last = d.chapters.iter().rposition(|c| sel.contains(&c.id));
    let prefix = last.is_some_and(|l| d.chapters[..=l].iter().all(|c| sel.contains(&c.id)));
    match last {
        Some(l) if read && prefix && ids.len() > 1 => {
            let before = d.chapters.get(l + 1).map_or(d.chapters[l].number + 1.0, |c| c.number);
            app.read_before(series, before);
        }
        _ => {
            for &c in ids {
                app.set_read(c, read);
            }
        }
    }
    let n = ids.len();
    app.toast(&format!("{} marked {}", logic::chapters_words(n), if read { "read" } else { "unread" }));
}

/// "Select range…": the first and last chapter, then Select (Some((from, to, false))) or
/// Download them (true). Some(None) for a tap outside.
fn range_sheet(cx: &mut kit::Ctx, from: &mut String, to: &mut String) -> Option<Option<(f64, f64, bool)>> {
    let (top, outside) = kit::sheet(cx, 30.0 + 26.0 + 24.0 + 72.0 + 20.0 + 48.0 + 16.0);
    if outside {
        return Some(None);
    }
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    cx.c.text("Select range", x, top + 26.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.text_fit("The first and last chapter, by number", x, top + 52.0, w, Text::new(ty::CAPTION, t::INK3));
    let fy = top + 30.0 + 26.0 + 24.0;
    let fw = (w - 12.0) / 2.0;
    kit::field(cx, "range-from", x, fy, fw, "From", "1", from, Kb::Text);
    kit::field(cx, "range-to", x + fw + 12.0, fy, fw, "To", "", to, Kb::Text);
    let ky = fy + 72.0 + 20.0;
    let parse = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
    let ok = parse(from).zip(parse(to));
    let mut out = None;
    if kit::plain(cx, x, ky, fw, 48.0, "Select", None)
        && let Some((a, b)) = ok
    {
        out = Some(Some((a, b, false)));
    }
    if kit::primary(cx, x + fw + 12.0, ky, fw, 48.0, "Download", Some("download"))
        && let Some((a, b)) = ok
    {
        out = Some(Some((a, b, true)));
    }
    if out.is_some() && cx.focus.id.is_some_and(|f| f.starts_with("range-")) {
        cx.focus.id = None;
        cx.host.keyboard(false, "", Kb::Text);
    }
    cx.ui.tap = None;
    out
}

/// "I've read up to…": one chapter number; every chapter to it is marked read and every one after
/// it unread, to set right progress brought from elsewhere. Some(None) for a tap outside.
fn upto_sheet(cx: &mut kit::Ctx, text: &mut String) -> Option<Option<f64>> {
    let (top, outside) = kit::sheet(cx, 30.0 + 26.0 + 24.0 + 72.0 + 20.0 + 48.0 + 16.0);
    if outside {
        return Some(None);
    }
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    cx.c.text("I've read up to", x, top + 26.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.text_fit("Chapters to it are marked read, the ones after unread", x, top + 52.0, w, Text::new(ty::CAPTION, t::INK3));
    let fy = top + 30.0 + 26.0 + 24.0;
    kit::field(cx, "upto", x, fy, w, "Chapter", "", text, Kb::Text);
    let ky = fy + 72.0 + 20.0;
    let n = text.trim().replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
    let mut out = None;
    if kit::primary(cx, x, ky, w, 48.0, "Mark read", Some("check"))
        && let Some(n) = n
    {
        out = Some(Some(n));
    }
    if out.is_some() && cx.focus.id == Some("upto") {
        cx.focus.id = None;
        cx.host.keyboard(false, "", Kb::Text);
    }
    cx.ui.tap = None;
    out
}

/// About the series (the info key, the owner's pick A, 2026-11-25): its author and status, the
/// readings on one line (the source's rating and bookmarks, how far you are), the tagline, the
/// story in its paragraphs with a quote standing alone, and the other titles it goes by. True
/// when it closes.
fn info_sheet(cx: &mut kit::Ctx, d: Option<&SeriesDetail>) -> bool {
    let Some(d) = d else { return true };
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let s = &d.series;
    let a = d.description.as_deref().map(logic::about).unwrap_or_default();
    let title_t = Text::new(20.0, t::INK).semi().line(1.2);
    let meta_t = Text::new(14.0, t::INK3);
    let value_t = Text::new(22.0, t::INK).semi();
    let label_t = Text::new(12.0, t::INK3).medium();
    let tag_t = Text::new(15.0, t::INK).semi().line(1.4);
    let body_t = Text::new(15.0, t::INK2).line(1.6);
    let quote_t = Text::new(17.0, t::INK).semi().line(1.4);
    let head_t = Text::new(11.0, t::INK3).semi();
    let also_t = Text::new(13.0, t::INK3).line(1.5);
    let author = s.author.as_deref().map(|a| logic::showable(a, |c| cx.c.can_show(c))).filter(|a| !a.is_empty());
    let meta: Vec<String> = author.into_iter().chain(logic::status_word(&s.status)).collect();
    let read = d.chapters.iter().filter(|c| c.read).count();
    let mut readings: Vec<(Option<&str>, String, &str)> = Vec::new();
    if let Some(r) = &a.rating {
        readings.push((Some("star"), r.clone(), "Rating"));
    }
    if let Some(b) = &a.bookmarks {
        readings.push((None, b.clone(), "Bookmarks"));
    }
    if !d.chapters.is_empty() {
        readings.push((None, format!("{read} / {}", d.chapters.len()), "Read"));
    }
    let also: Vec<String> = a.also.iter().map(|n| logic::showable(n, |c| cx.c.can_show(c))).filter(|n| !n.is_empty() && *n != s.title).collect();
    let also_words = match also.len() {
        0 => String::new(),
        n if n > 3 => format!("{}  +{}", also[..3].join(", "), n - 3),
        _ => also.join(", "),
    };
    // Its height, the story cut to what the screen holds.
    let room = cx.c.h - cx.c.inset_top - 60.0;
    let mut h = 30.0 + cx.c.para_h(&s.title, w, title_t, 3) + 4.0;
    if !meta.is_empty() {
        h += meta_t.lh();
    }
    if !readings.is_empty() {
        h += 20.0 + value_t.lh() + 2.0 + label_t.lh();
    }
    h += 22.0;
    if let Some(tg) = &a.tagline {
        h += cx.c.para_h(tg, w, tag_t, 3) + 12.0;
    }
    let mut paras: Vec<(&logic::Para, usize)> = Vec::new();
    let tail = if also_words.is_empty() { 16.0 } else { 20.0 + head_t.lh() + 6.0 + cx.c.para_h(&also_words, w, also_t, 3) + 16.0 };
    for p in &a.paras {
        let (tx, tt) = match p {
            logic::Para::Text(tx) => (tx, body_t),
            logic::Para::Quote(q) => (q, quote_t),
        };
        let full = cx.c.para_h(tx, w, tt, 40);
        let left = room - h - tail;
        if left < tt.lh() * 2.0 {
            break;
        }
        let lines = if full <= left { 40 } else { (left / tt.lh()).floor() as usize };
        h += cx.c.para_h(tx, w, tt, lines) + 12.0;
        paras.push((p, lines));
    }
    if a.paras.is_empty() {
        h += body_t.lh() + 12.0;
    }
    h += tail;
    let (top, outside) = kit::sheet(cx, h);
    if outside {
        return true;
    }
    let mut y = top + 30.0;
    y += cx.c.para(&s.title, x, y, w, title_t, 3) + 4.0;
    if !meta.is_empty() {
        cx.c.text_fit(&meta.join(" \u{00B7} "), x, y, w, meta_t);
        y += meta_t.lh();
    }
    if !readings.is_empty() {
        y += 20.0;
        let cw = w / readings.len().max(3) as f32;
        for (i, (icon, value, label)) in readings.iter().enumerate() {
            let rx = x + i as f32 * cw;
            let mut vx = rx;
            if let Some(ic) = icon {
                cx.c.icon(ic, vx + 8.0, y + value_t.lh() / 2.0, 16.0, t::WARN);
                vx += 22.0;
            }
            cx.c.text(value, vx, y, value_t);
            cx.c.text(label, rx, y + value_t.lh() + 2.0, label_t);
        }
        y += value_t.lh() + 2.0 + label_t.lh();
    }
    y += 22.0;
    if let Some(tg) = &a.tagline {
        y += cx.c.para(tg, x, y, w, tag_t, 3) + 12.0;
    }
    for (p, lines) in paras {
        y += match p {
            logic::Para::Text(tx) => cx.c.para(tx, x, y, w, body_t, lines),
            logic::Para::Quote(q) => cx.c.para(q, x, y, w, quote_t, lines),
        } + 12.0;
    }
    if a.paras.is_empty() {
        cx.c.text("No description came with this series.", x, y, Text::new(15.0, t::INK3));
        y += body_t.lh() + 12.0;
    }
    if !also_words.is_empty() {
        y += 8.0;
        cx.c.text("ALSO KNOWN AS", x, y, head_t);
        y += head_t.lh() + 6.0;
        cx.c.para(&also_words, x, y, w, also_t, 3);
    }
    cx.ui.tap = None;
    false
}

/// "Wrong series?": every entry found for the work, with its chapters, the one read now checked.
/// Some(Some(manga)) for an entry picked, Some(None) for a tap outside.
fn alternatives_sheet(cx: &mut kit::Ctx, alts: Option<&crate::app::Loaded<Vec<leaf_proto::Alternative>>>, moving: bool, now: u64) -> Option<Option<i64>> {
    let list = alts.and_then(|l| l.data.as_ref());
    let loading = alts.is_none_or(|l| l.loading && l.data.is_none());
    let error = alts.and_then(|l| l.error.clone()).filter(|_| list.is_none());
    let rh = 64.0;
    let rows = list.map_or(0, |l| l.len());
    let body_h = if rows > 0 { rh * rows.min(6) as f32 } else { 96.0 };
    let (top, outside) = kit::sheet(cx, 82.0 + body_h + 16.0);
    if outside {
        return Some(None);
    }
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    cx.c.text("Wrong series?", x, top + 26.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.text_fit("Pick the right entry for this title. Your place comes along.", x, top + 50.0, w, Text::new(ty::CAPTION, t::INK3));
    let y0 = top + 82.0;
    let spinner = |cx: &mut kit::Ctx, cy: f32| {
        let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
        cx.c.arc(x + 10.0, cy, 9.0, 2.0, turn, turn + 0.7, t::INK3, false);
    };
    if loading {
        spinner(cx, y0 + 22.0);
        cx.c.para("Asking every source. This takes a few seconds.", x + 30.0, y0 + 12.0, w - 30.0, Text::new(ty::UI, t::INK2).line(1.4), 3);
    } else if let Some(e) = error {
        cx.c.para(&e, x, y0 + 12.0, w, Text::new(ty::UI, t::DANGER).line(1.4), 3);
    } else if rows == 0 {
        cx.c.para("No other entry for this title was found.", x, y0 + 12.0, w, Text::new(ty::UI, t::INK2).line(1.4), 3);
    }
    let mut out = None;
    for (i, a) in list.into_iter().flatten().take(6).enumerate() {
        let y = y0 + rh * i as f32;
        if !a.current && cx.pressed(0.0, y, WIDTH, rh) {
            cx.c.rect(8.0, y, WIDTH - 16.0, rh, 10.0, t::WASH);
        }
        let tt = Text::new(ty::BODY, t::INK).medium();
        let lt = Text::new(ty::CAPTION, t::INK3);
        let ty0 = y + (rh - tt.lh() - 4.0 - lt.lh()) / 2.0;
        cx.c.text_fit(&a.title, x, ty0, w - 40.0, tt);
        let mut line = logic::chapters_words(a.chapters as usize);
        if let Some(l) = a.latest {
            line.push_str(&format!(", up to {}", logic::num(l)));
        }
        if a.current {
            line.push_str(" · reading this one");
        }
        cx.c.text_fit(&line, x, ty0 + tt.lh() + 4.0, w - 40.0, lt.color(if a.current { t::ACCENT } else { t::INK3 }));
        if a.current {
            cx.c.icon("check", WIDTH - PAD - 12.0, y + rh / 2.0, 17.0, t::ACCENT);
        } else if moving {
            // Being moved to: the rows wait.
        } else if cx.tap(0.0, y, WIDTH, rh) {
            out = Some(Some(a.manga_id));
        }
    }
    if moving {
        spinner(cx, top + 30.0);
    }
    cx.ui.tap = None;
    out
}
