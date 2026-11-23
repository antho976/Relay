//! A series (design 42, page 13, artboard B): a wide banner from its cover fading into the
//! ground, the small cover over it with the title and author, a line of facts (chapters, status,
//! chapters on this phone), the one key that continues reading, and two tabs.
//!
//! - **Chapters**: "339 chapters" with Newest ⇅ (remembered per series), then a row per chapter:
//!   its picture ([`crate::thumb`]: a crop from one of its own pages once it is downloaded; a quiet
//!   tile with a cloud until then, which downloads it when tapped), "Chapter N" or its title, the
//!   day it came out, and Read, Continue (the chapter you are on) or a check (read, dimmed). A run
//!   of chapters no source has gets a row of its own; the server fills most from other sources.
//! - **About**: the description and what is known of the series.
//!
//! The bell in the top bar is the follow: new chapters are announced while it is on. ⋯ holds
//! "Download next 5" and "Remove from My space" (behind a confirm). Sources are never named.

use leaf_proto::SeriesDetail;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::{CoverOf, thumb_key};
use crate::kit;
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
    /// About instead of Chapters.
    pub about: bool,
    /// When the series was last asked again while chapters download.
    polled_ns: u64,
    /// How many times it was asked since the last download began: each wait is longer.
    polls: u32,
}

impl State {
    /// Close the menu or the dialog, if one is open.
    pub fn close_over(&mut self) -> bool {
        let open = self.menu || self.confirm;
        self.menu = false;
        self.confirm = false;
        open
    }
}

const ROW_H: f32 = 84.0;
const GAP_H: f32 = 48.0;
/// A chapter's picture on its row.
const PIC: (f32, f32) = (120.0, 68.0);
/// While a chapter downloads on the server, the series is asked again after this, then after
/// twice as long each time (to [`POLL_MAX_NS`]), and given up on after [`POLLS`] asks: a download
/// the server took but could not finish is not waited for as long as the page is open.
const POLL_NS: u64 = 5_000_000_000;
const POLL_MAX_NS: u64 = 80_000_000_000;
const POLLS: u32 = 12;

/// The wait before the series is asked again, after `polls` asks.
fn poll_wait(polls: u32) -> u64 {
    (POLL_NS << polls.min(8)).min(POLL_MAX_NS)
}

enum Act {
    Open(usize, u32),
    Download(i64),
    Bell(bool),
}

pub fn draw(app: &mut App, id: i64, dt: f32) -> bool {
    if app.fresh {
        app.st.series = State::default();
        if !app.series.get(&id).is_some_and(|l| l.loading) {
            app.ask(Job::Series(id));
        }
    }
    // What the library knows stands in until the series itself arrives.
    let detail: Option<SeriesDetail> = app.series.get(&id).and_then(|l| l.data.clone());
    let from_library = app.library.data.as_ref().and_then(|l| l.iter().find(|s| s.id == id).cloned());
    let loaded = app.series.get(&id);
    let (loading, error, offline) = (loaded.is_some_and(|l| l.loading), loaded.and_then(|l| l.error.clone()), loaded.is_some_and(|l| l.offline()));
    // Chapters downloading on the server: asked again every few seconds until they are there.
    let mut waiting = detail.as_ref().is_some_and(|d| d.chapters.iter().any(|c| app.downloading.contains(&c.id) && !c.downloaded));
    if waiting && !loading && app.now_ns.saturating_sub(app.st.series.polled_ns) >= poll_wait(app.st.series.polls) {
        if app.st.series.polls >= POLLS {
            // The server took the download and never finished it: stop waiting, say so.
            if let Some(d) = &detail {
                for c in d.chapters.iter().filter(|c| !c.downloaded) {
                    app.downloading.remove(&c.id);
                }
            }
            app.st.series.polls = 0;
            waiting = false;
            app.toast("Your server couldn't download that chapter. Try again later");
        } else {
            app.st.series.polled_ns = app.now_ns;
            app.st.series.polls += 1;
            app.ask(Job::Series(id));
        }
    }
    if waiting {
        let at = app.st.series.polled_ns + poll_wait(app.st.series.polls);
        app.wake_at(at);
    }
    let covered = app.st.series.menu || app.st.series.confirm;
    let oldest = app.oldest_first.contains(&id);
    let cached = app.images.cached.clone().unwrap_or_default();
    let downloading = app.downloading.clone();
    let pg = app.page_begin(dt, 0.0);
    let now = app.now_ns;
    let mut cx = cx!(app);
    let held = if covered { kit::hold(&mut cx) } else { None };
    let series = detail.as_ref().map(|d| d.series.clone()).or(from_library);
    let x0 = PAD;
    let w = WIDTH - PAD * 2.0;
    let top = cx.c.inset_top;
    let of = CoverOf::Series(id);
    let has_cover = app.images.cover(cx.c, of, now);
    // The banner: the cover itself, wide, fading into the ground.
    let banner_h = top + 300.0;
    if app.images.hero(cx.c, of, now) {
        cx.c.image_cover_at(&of.hero_key(), 0.0, 0.0, WIDTH, banner_h, 0.0, Rgba::WHITE, (0.5, 0.25));
    } else if cx.c.has_image(&of.banner_key()) {
        cx.c.image_cover(&of.banner_key(), -40.0, -40.0, WIDTH + 80.0, banner_h + 40.0, 0.0, Rgba::WHITE.a(0.6));
    }
    cx.c.rect_grad(-1000.0, 0.0, WIDTH + 2000.0, top + 96.0, 0.0, t::BG.a(0.62), t::BG.a(0.0));
    cx.c.rect_grad(-1000.0, banner_h * 0.4, WIDTH + 2000.0, banner_h * 0.6 + 1.0, 0.0, t::BG.a(0.0), t::BG);
    let (cw, ch) = (92.0, 132.0);
    let cover_y = top + 150.0;
    let cxx = x0 + 4.0;
    cx.c.rect(cxx, cover_y, cw, ch, COVER_R, t::S1);
    if has_cover {
        cx.c.image_cover(&of.key(), cxx, cover_y, cw, ch, COVER_R, Rgba::WHITE);
    }
    cx.c.outline(cxx, cover_y, cw, ch, COVER_R, 1.0, t::INK.a(0.08));
    let tx = cxx + cw + 14.0;
    let tw = WIDTH - PAD - tx;
    let mut act: Option<Act> = None;
    let mut retry = false;
    let mut flip = false;
    let mut set_about = None;
    if let Some(s) = &series {
        let tt = Text::new(21.0, t::INK).brand().line(1.2);
        let lines = cx.c.wrap(&s.title, tw, tt, 3);
        let author = s.author.as_deref().map(|a| logic::showable(a, |c| cx.c.can_show(c))).filter(|a| !a.is_empty());
        let at = Text::new(13.0, t::INK2);
        let block = lines.len() as f32 * tt.lh() + if author.is_some() { 6.0 + at.lh() } else { 0.0 };
        let mut ty_ = (cover_y + ch - 6.0 - block).max(top + 60.0);
        for l in &lines {
            cx.c.text(l, tx, ty_, tt);
            ty_ += tt.lh();
        }
        if let Some(a) = &author {
            cx.c.text_fit(a, tx, ty_ + 6.0, tw, at);
        }
    }
    let mut y = cover_y + ch + 24.0;
    // The facts: chapters, status, on this phone.
    if let Some(s) = &series {
        let on_phone = detail.as_ref().map_or(0, |d| d.chapters.iter().filter(|c| cached.contains_key(&c.id)).count());
        let n = detail.as_ref().map_or(s.chapters as usize, |d| d.chapters.len());
        let mut facts: Vec<(String, &str)> = vec![(n.to_string(), if n == 1 { "chapter" } else { "chapters" })];
        if let Some(sw) = logic::status_word(&s.status) {
            facts.push((sw, "status"));
        }
        facts.push((on_phone.to_string(), "on this phone"));
        let vt = Text::new(16.0, t::INK).semi();
        let lt = Text::new(12.0, t::INK3);
        for (i, (v, l)) in facts.iter().enumerate() {
            let fx = cxx + i as f32 * 112.0;
            cx.c.text(v, fx, y, vt);
            cx.c.text(l, fx, y + vt.lh() + 2.0, lt);
        }
        y += vt.lh() + 2.0 + lt.lh() + 18.0;
    }
    match &detail {
        Some(d) => {
            let cont = logic::continue_point(&d.chapters);
            if let Some(c) = &cont {
                let label = c.label.replace("ch. ", "chapter ");
                if kit::primary(&mut cx, x0, y, w, 50.0, &label, Some("play")) {
                    act = Some(Act::Open(c.index, c.page));
                }
                y += 50.0 + 16.0;
            }
            if let Some(note) = &d.series.note {
                cx.c.icon("bolt", x0 + 8.0, y + 10.0, 16.0, t::WARN);
                y += cx.c.para(note, x0 + 26.0, y, w - 26.0, Text::new(ty::UI, t::WARN).line(1.4), 4) + 14.0;
            }
            if let Some(e) = error.as_ref().filter(|_| !offline) {
                y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            if let Some(e) = &app.st.series.error {
                y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            // Chapters | About
            let tt = Text::new(ty::BODY, t::INK3).medium();
            let mut tabx = x0 + 4.0;
            for (i, words) in ["Chapters", "About"].iter().enumerate() {
                let on = (i == 1) == app.st.series.about;
                let tw_ = cx.c.text(words, tabx, y + (40.0 - tt.lh()) / 2.0, if on { tt.color(t::INK) } else { tt });
                if on {
                    cx.c.rect(tabx, y + 38.0, tw_, 2.0, 1.0, t::ACCENT);
                }
                if cx.tap(tabx - 12.0, y, tw_ + 24.0, 40.0) {
                    set_about = Some(i == 1);
                }
                tabx += tw_ + 26.0;
            }
            cx.c.rect(-1000.0, y + 40.0, WIDTH + 2000.0, 1.0, 0.0, t::LINE);
            y += 41.0;
            if app.st.series.about {
                y += 18.0;
                let body = d.description.as_deref().map(logic::clean_description).filter(|s| !s.is_empty()).unwrap_or_else(|| "No description came with this series.".into());
                y += cx.c.para(&body, x0 + 4.0, y, w - 8.0, Text::new(ty::BODY, t::INK2).line(1.55), 40) + 24.0;
                let s = &d.series;
                let mut rows: Vec<(&str, String)> = Vec::new();
                if let Some(a) = s.author.as_deref().map(|a| logic::showable(a, |c| cx.c.can_show(c))).filter(|a| !a.is_empty()) {
                    rows.push(("Author", a));
                }
                if let Some(sw) = logic::status_word(&s.status) {
                    rows.push(("Status", sw));
                }
                rows.push(("Chapters", match s.latest {
                    Some(l) => format!("{}, up to {}", d.chapters.len(), logic::num(l)),
                    None => d.chapters.len().to_string(),
                }));
                if s.read_upto.is_some() {
                    rows.push(("Read", format!("{} of {}", d.chapters.iter().filter(|c| c.read).count(), d.chapters.len())));
                }
                if s.updated_at > 0 {
                    rows.push(("Updated", logic::local_date(s.updated_at)));
                }
                if s.added_at > 0 {
                    rows.push(("Added", logic::local_date(s.added_at)));
                }
                let lt = Text::new(ty::UI, t::INK3);
                let vt = Text::new(ty::UI, t::INK);
                for (l, v) in rows {
                    cx.c.text(l, x0 + 4.0, y, lt);
                    y += cx.c.para(&v, x0 + 110.0, y, w - 110.0, vt.line(1.4), 3) + 12.0;
                }
            } else {
                y += 14.0;
                let n = d.chapters.len();
                let ht = Text::new(ty::UI, t::INK).semi();
                cx.c.text(&logic::chapters_words(n), x0 + 4.0, y, ht);
                let ot = Text::new(13.0, t::INK2).right();
                let words = if oldest { "Oldest" } else { "Newest" };
                let ow = cx.c.measure(words, ot);
                cx.c.text(words, WIDTH - PAD - 24.0, y + 1.0, ot);
                cx.c.icon("transfer", WIDTH - PAD - 8.0, y + ht.lh() / 2.0, 15.0, t::INK2);
                if cx.tap(WIDTH - PAD - 34.0 - ow, y - 12.0, ow + 34.0 + PAD, ht.lh() + 24.0) {
                    flip = true;
                }
                y += ht.lh() + 12.0;
                let mut rows = logic::chapter_rows(&d.chapters, &d.series.gaps);
                if oldest {
                    rows.reverse();
                }
                let view_top = -cx.c.dy - 60.0;
                let view_bottom = -cx.c.dy + cx.c.h + 60.0;
                let current = cont.as_ref().map(|c| c.index);
                let mut thumbs: Vec<(i64, u32)> = Vec::new();
                for row in rows {
                    let h = if matches!(row, Row::Gap(_)) { GAP_H } else { ROW_H };
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
                            if cx.pressed(0.0, y, WIDTH, h) {
                                cx.c.rect(0.0, y, WIDTH, h, 0.0, t::WASH);
                            }
                            let here = current == Some(i) && !(c.read && cont.as_ref().is_some_and(|k| k.label.starts_with("Read")));
                            let dim = c.read && !here;
                            // The picture: the chapter's own once it is downloaded, a cloud until then.
                            let (px, py) = (x0, y + (h - PIC.1) / 2.0);
                            let on_phone = cached.contains_key(&c.id);
                            let fetching = downloading.contains(&c.id) && !c.downloaded;
                            if c.downloaded || on_phone {
                                thumbs.push((c.id, c.pages));
                                if !cx.c.image_cover(&thumb_key(c.id), px, py, PIC.0, PIC.1, 6.0, Rgba::WHITE.a(if dim { 0.45 } else { 1.0 })) {
                                    cx.c.rect(px, py, PIC.0, PIC.1, 6.0, t::S1);
                                }
                            } else {
                                cx.c.rect(px, py, PIC.0, PIC.1, 6.0, t::S2);
                                if fetching {
                                    let turn = (now as f64 / 1e9 * 0.9).fract() as f32;
                                    cx.c.arc(px + PIC.0 / 2.0, py + PIC.1 / 2.0, 9.0, 2.0, turn, turn + 0.7, t::INK3, false);
                                } else {
                                    cx.c.icon("cloud", px + PIC.0 / 2.0, py + PIC.1 / 2.0 - 1.0, 22.0, t::INK3);
                                    cx.c.icon("arrow-down", px + PIC.0 / 2.0, py + PIC.1 / 2.0 + 1.0, 10.0, t::INK3);
                                }
                            }
                            let tx = px + PIC.0 + 14.0;
                            // The key at the right: Continue, Read, or a check.
                            let ky = y + (h - 32.0) / 2.0;
                            let mut key_w = 24.0;
                            let mut key_tap = false;
                            if here {
                                let kt = Text::new(ty::UI, t::ON_ACCENT).semi();
                                let kw = cx.c.measure("Continue", kt) + 28.0;
                                let kx = WIDTH - PAD - kw;
                                let pressed = cx.pressed(kx, ky, kw, 32.0);
                                cx.c.rect(kx, ky, kw, 32.0, 16.0, if pressed { t::ACCENT.mix(Rgba::hex(0x000000), 0.12) } else { t::ACCENT });
                                cx.c.text("Continue", kx + 14.0, ky + (32.0 - kt.lh()) / 2.0, kt);
                                key_w = kw;
                            } else if dim {
                                cx.c.icon("check", WIDTH - PAD - 10.0, y + h / 2.0, 16.0, t::INK3);
                            } else {
                                let (kw, tapped) = kit::pill(&mut cx, WIDTH - PAD, ky, "Read", false);
                                key_w = kw;
                                key_tap = tapped;
                            }
                            let titled = logic::chapter_title(&c.name, c.number);
                            let title = titled.clone().unwrap_or_else(|| format!("Chapter {}", logic::num(c.number)));
                            let tw = WIDTH - PAD - key_w - 12.0 - tx;
                            let title_t = Text::new(15.5, if dim { t::INK3 } else { t::INK }).semi();
                            let line_t = Text::new(12.5, t::INK3);
                            let block = title_t.lh() + 4.0 + line_t.lh();
                            let ty0 = y + (h - block) / 2.0;
                            cx.c.text_fit(&title, tx, ty0, tw, title_t);
                            let mut line = Vec::new();
                            if titled.is_some() {
                                line.push(format!("Ch. {}", logic::num(c.number)));
                            }
                            if c.uploaded_at > 0 {
                                line.push(logic::local_date(c.uploaded_at));
                            }
                            if fetching {
                                line.push("Downloading".into());
                            }
                            // Left part way: the page first, in the accent.
                            let ly = ty0 + title_t.lh() + 4.0;
                            let mut lx = tx;
                            if !c.read && c.last_page > 0 {
                                let lead = format!("p. {} of {}", c.last_page + 1, c.pages.max(c.last_page + 1));
                                lx += cx.c.text_fit(&lead, lx, ly, tw, line_t.color(t::ACCENT));
                                if !line.is_empty() {
                                    lx += cx.c.text(" · ", lx, ly, line_t);
                                }
                            }
                            cx.c.text_fit(&line.join(" · "), lx, ly, (tx + tw - lx).max(0.0), line_t);
                            let pic_tap = !c.downloaded && !on_phone && !fetching && cx.tap(px - 6.0, py - 6.0, PIC.0 + 12.0, PIC.1 + 12.0);
                            if pic_tap {
                                act = Some(Act::Download(c.id));
                            } else if key_tap || cx.tap(0.0, y, WIDTH, h) {
                                let page = if !c.read { logic::resume_page(c) } else { 0 };
                                act = Some(Act::Open(i, page));
                            }
                        }
                    }
                    y += h;
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
    let bottom = y;
    // The fixed top: Back, the bell and ⋯, over the banner (a band of ground once it scrolls).
    let mut back = false;
    let mut more = false;
    {
        let scrolled = (-cx.c.dy).clamp(0.0, 60.0) / 60.0;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG.a(scrolled));
        // Over the picture the keys sit on a soft dark disc, so a bright cover doesn't swallow them.
        for kx in [6.0, WIDTH - 6.0 - 88.0, WIDTH - 6.0 - 44.0] {
            cx.c.circle(kx + 22.0, top + 28.0, 18.0, t::BG.a(0.32 * (1.0 - scrolled)));
        }
        if kit::icon_key(&mut cx, 6.0, top + 6.0, "arrow-left") {
            back = true;
        }
        if let Some(s) = &series {
            let tapped = if s.bell { kit::icon_key_ink(&mut cx, WIDTH - 6.0 - 88.0, top + 6.0, "bell-on", t::ACCENT) } else { kit::icon_key(&mut cx, WIDTH - 6.0 - 88.0, top + 6.0, "bell") };
            if tapped {
                act = Some(Act::Bell(!s.bell));
            }
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 6.0, "more") {
            more = true;
        }
        if offline && scrolled <= 0.5 {
            kit::tag(cx.c, 58.0, top + 18.0, "Offline");
        }
        if scrolled > 0.5
            && let Some(s) = &series
        {
            let tt = Text::new(ty::SECTION, t::INK.a(scrolled)).semi();
            cx.c.text_fit(&s.title, 56.0, top + 6.0 + (44.0 - tt.lh()) / 2.0, WIDTH - 56.0 - 100.0, tt);
        }
    }
    // Over the page: the menu, then the confirm.
    let st = &mut app.st.series;
    let mut remove = false;
    let mut next5 = false;
    if covered {
        kit::release(&mut cx, held);
        if st.menu {
            match kit::menu(&mut cx, top + 52.0, &["Download next 5", "Remove from My space"]) {
                Some(0) => {
                    st.menu = false;
                    next5 = true;
                }
                Some(1) => {
                    st.menu = false;
                    st.confirm = true;
                }
                Some(_) => st.menu = false,
                None => {}
            }
        } else if st.confirm {
            let name = series.as_ref().map(|s| s.title.as_str()).unwrap_or("this series");
            let body = "It leaves My space, and your server stops keeping its chapters. Your place in it is forgotten. You can add it again from Search.";
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
    }
    if let Some(a) = set_about {
        st.about = a;
    }
    app.page_end(&pg, bottom);
    if waiting {
        // The spinner turns.
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
    if next5 && let Some(d) = &detail {
        let ids = logic::next_to_download(&d.chapters, 5);
        if ids.is_empty() {
            app.toast("The next 5 chapters are already downloaded");
        } else {
            app.download(&ids, true);
            app.toast(&format!("Downloading {}", logic::chapters_words(ids.len())));
            app.st.series.polled_ns = app.now_ns;
            app.st.series.polls = 0;
        }
    }
    if retry {
        app.ask(Job::Series(id));
    }
    match act {
        Some(Act::Open(index, page)) if !covered => {
            if let Some(d) = &detail {
                app.open_chapter(d, index, page);
            }
        }
        Some(Act::Download(c)) if !covered => {
            app.download(&[c], true);
            app.st.series.polled_ns = app.now_ns;
            app.st.series.polls = 0;
        }
        Some(Act::Bell(on)) if !covered => app.set_bell(id, on),
        _ => {
            if back && !covered {
                app.pop();
            }
        }
    }
    pg.moving || loading && !crate::wake::has()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_is_waited_for_less_and_less_often_then_given_up() {
        let waits: Vec<u64> = (0..POLLS).map(poll_wait).collect();
        assert_eq!(&waits[..5], &[5_000_000_000, 10_000_000_000, 20_000_000_000, 40_000_000_000, 80_000_000_000]);
        assert!(waits.iter().all(|w| *w <= POLL_MAX_NS));
        let total: u64 = waits.iter().sum();
        assert!(total < 20 * 60 * 1_000_000_000, "given up within twenty minutes");
    }
}
