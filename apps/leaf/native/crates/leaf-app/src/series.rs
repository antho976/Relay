//! A series: its cover over a wash of its own colours, the title and where it comes from, the one
//! key that continues reading, and every chapter newest first, read ones dimmed, with a row in
//! place of each run of chapters no source has. Unfollow sits in the ⋯ menu, behind a confirm.

use leaf_proto::SeriesDetail;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit;
use crate::logic::{self, Row};
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};

#[derive(Default)]
pub struct State {
    pub menu: bool,
    pub confirm: bool,
    /// Unfollowing.
    pub busy: bool,
    pub error: Option<String>,
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

const ROW_H: f32 = 50.0;
const GAP_H: f32 = 48.0;

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
    let covered = app.st.series.menu || app.st.series.confirm;
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
    // The banner: the cover's colours, blurred, fading into the ground.
    let banner_h = top + 260.0;
    if cx.c.has_image(&of.banner_key()) {
        cx.c.image_cover(&of.banner_key(), -40.0, -40.0, WIDTH + 80.0, banner_h + 40.0, 0.0, Rgba::WHITE.a(0.55));
    }
    cx.c.rect_grad(-1000.0, -40.0, WIDTH + 2000.0, banner_h + 40.0, 0.0, t::BG.a(0.3), t::BG);
    let mut y = top + 64.0;
    let (cw, ch) = (112.0, 160.0);
    cx.c.rect(x0, y, cw, ch, COVER_R, t::S1);
    if has_cover {
        cx.c.image_cover(&of.key(), x0, y, cw, ch, COVER_R, Rgba::WHITE);
    }
    let tx = x0 + cw + 16.0;
    let tw = WIDTH - PAD - tx;
    let mut ty_ = y + 4.0;
    let mut open_at: Option<(usize, u32)> = None;
    let mut retry = false;
    if let Some(s) = &series {
        let tt = Text::new(ty::HEADING, t::INK).brand().line(1.25);
        ty_ += cx.c.para(&s.title, tx, ty_, tw, tt, 4) + 8.0;
        let author = s.author.as_deref().map(|a| logic::showable(a, |c| cx.c.can_show(c))).filter(|a| !a.is_empty());
        if let Some(a) = &author {
            ty_ += cx.c.para(a, tx, ty_, tw, Text::new(ty::UI, t::INK2).line(1.35), 2) + 2.0;
        }
        let mut meta = vec![s.source.clone()];
        meta.extend(logic::status_word(&s.status));
        cx.c.text_fit(&meta.join(" · "), tx, ty_, tw, Text::new(ty::UI, t::INK2));
        ty_ += 22.0;
        if offline {
            kit::tag(cx.c, tx, ty_ + 4.0, "Offline");
        }
    }
    y = (y + ch).max(ty_) + 20.0;
    match &detail {
        Some(d) => {
            let cont = logic::continue_point(&d.chapters);
            if let Some(c) = &cont {
                if kit::primary(&mut cx, x0, y, w, 50.0, &c.label, Some("play")) {
                    open_at = Some((c.index, c.page));
                }
                y += 50.0 + 18.0;
            }
            if let Some(note) = &d.series.note {
                cx.c.icon("bolt", x0 + 8.0, y + 10.0, 16.0, t::WARN);
                y += cx.c.para(note, x0 + 26.0, y, w - 26.0, Text::new(ty::UI, t::WARN).line(1.4), 4) + 14.0;
            }
            if let Some(e) = cx_error(&error, offline) {
                y += cx.c.para(&e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            if let Some(e) = &app.st.series.error {
                y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
            }
            let n = d.chapters.len();
            cx.c.text(&if n == 1 { "1 chapter".into() } else { format!("{n} chapters") }, x0, y, Text::new(ty::SECTION, t::INK).semi());
            cx.c.text("Newest first", WIDTH - PAD, y + 2.0, Text::new(ty::CAPTION, t::INK3).right());
            y += 34.0;
            let rows = logic::chapter_rows(&d.chapters, &d.series.gaps);
            let view_top = -cx.c.dy - 60.0;
            let view_bottom = -cx.c.dy + cx.c.h + 60.0;
            let current = cont.as_ref().map(|c| c.index);
            // A number column only when chapters have titles of their own to sit beside it.
            let titled = d.chapters.iter().any(|c| logic::chapter_title(&c.name, c.number).is_some());
            let tx = if titled { x0 + 44.0 } else { x0 };
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
                        let here = current == Some(i);
                        let (num_ink, ink) = if c.read { (t::INK3.a(0.7), t::INK3) } else { (t::INK3, t::INK) };
                        if titled {
                            let nt = Text::new(ty::UI, if here { t::ACCENT } else { num_ink }).medium();
                            cx.c.text_fit(&logic::num(c.number), x0, y + (h - nt.lh()) / 2.0, 40.0, nt);
                        }
                        let right = if !c.read && c.last_page > 0 {
                            format!("p. {} of {}", c.last_page + 1, c.pages.max(c.last_page + 1))
                        } else {
                            logic::date(c.uploaded_at)
                        };
                        let rt = Text::new(ty::CAPTION, if !c.read && c.last_page > 0 { t::ACCENT } else { t::INK3 }).right();
                        let rw = cx.c.measure(&right, rt);
                        cx.c.text(&right, WIDTH - PAD, y + (h - rt.lh()) / 2.0, rt);
                        let title = logic::chapter_title(&c.name, c.number).unwrap_or_else(|| format!("Chapter {}", logic::num(c.number)));
                        let tt = Text::new(ty::BODY, if here && !titled { t::ACCENT } else { ink });
                        let tt = if here { tt.medium() } else { tt };
                        cx.c.text_fit(&title, tx, y + (h - tt.lh()) / 2.0, WIDTH - PAD - rw - 16.0 - tx, tt);
                        if cx.tap(0.0, y, WIDTH, h) {
                            let page = if !c.read { c.last_page.min(c.pages.saturating_sub(1)) } else { 0 };
                            open_at = Some((i, page));
                        }
                    }
                }
                y += h;
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
    // The fixed top: Back and ⋯, over the banner (a band of ground once the page scrolls).
    let mut back = false;
    let mut more = false;
    {
        let scrolled = (-cx.c.dy).clamp(0.0, 60.0) / 60.0;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG.a(scrolled));
        if kit::icon_key(&mut cx, 6.0, top + 6.0, "arrow-left") {
            back = true;
        }
        if kit::icon_key(&mut cx, WIDTH - 6.0 - 44.0, top + 6.0, "more") {
            more = true;
        }
        if scrolled > 0.5
            && let Some(s) = &series
        {
            let tt = Text::new(ty::SECTION, t::INK.a(scrolled)).semi();
            cx.c.text_fit(&s.title, 56.0, top + 6.0 + (44.0 - tt.lh()) / 2.0, WIDTH - 56.0 - 56.0, tt);
        }
    }
    // Over the page: the menu, then the confirm.
    let st = &mut app.st.series;
    let mut unfollow = false;
    if covered {
        kit::release(&mut cx, held);
        if st.menu {
            match kit::menu(&mut cx, top + 52.0, &["Unfollow"]) {
                Some(0) => {
                    st.menu = false;
                    st.confirm = true;
                }
                Some(_) => st.menu = false,
                None => {}
            }
        } else if st.confirm {
            let name = series.as_ref().map(|s| s.title.as_str()).unwrap_or("this series");
            let body = "It leaves your library, and your server stops fetching its chapters. You can follow it again from Search.";
            match kit::confirm(&mut cx, &format!("Unfollow {name}?"), body, "Unfollow") {
                Some(true) => {
                    st.confirm = false;
                    unfollow = true;
                }
                Some(false) => st.confirm = false,
                None => {}
            }
        }
    } else if more {
        st.menu = true;
    }
    app.page_end(&pg, bottom);
    if unfollow {
        app.st.series.busy = true;
        app.st.series.error = None;
        app.ask(Job::Unfollow(id));
    }
    if retry {
        app.ask(Job::Series(id));
    }
    if let (Some((index, page)), Some(d)) = (open_at, detail) {
        app.reader = Some(crate::reader::Reader::open(&d, index, page));
        app.host.immersive(true);
        app.push(Route::Reader);
    } else if back && !covered {
        app.pop();
    }
    pg.moving || loading && !crate::wake::has()
}

/// The series' own error, when it isn't simply "offline" (which the tag says).
fn cx_error(error: &Option<String>, offline: bool) -> Option<String> {
    if offline { None } else { error.clone() }
}
