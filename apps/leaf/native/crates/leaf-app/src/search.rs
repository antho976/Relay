//! Search, the second tab: one field, every source on the server asked at once (`/v1/search`),
//! one row per work. Add sends every hit's manga id (`POST /v1/follow`), which puts the series in
//! My space with its bell off (adding is not following), and opens it; one already added opens.
//! Sources are not named: which one a series comes from is the server's business.

use leaf_proto::{Candidate, Follow, Series};

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit::{self, Kb};
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};

#[derive(Default)]
pub struct State {
    pub query: String,
    /// The query last sent, and what came back for it.
    pub asked: Option<String>,
    pub results: Option<Result<Vec<Candidate>, String>>,
    /// The title being followed now, and why the last follow failed.
    pub following: Option<String>,
    pub follow_error: Option<String>,
}

impl State {
    pub fn answered(&mut self, q: String, r: Result<Vec<Candidate>, String>) {
        if self.asked.as_deref() == Some(q.as_str()) {
            self.results = Some(r);
        }
    }

    pub fn followed(&mut self, s: &Series) {
        self.following = None;
        self.follow_error = None;
        if let Some(Ok(list)) = &mut self.results {
            for c in list.iter_mut() {
                if leaf_proto::same_work(&c.title, &s.title) || c.title == s.title {
                    c.followed = Some(s.id);
                }
            }
        }
    }

    pub fn follow_failed(&mut self, why: String) {
        self.following = None;
        self.follow_error = Some(why);
    }

    fn searching(&self) -> bool {
        self.asked.is_some() && self.results.is_none()
    }
}

const TOP: f32 = 60.0;
const ROW_H: f32 = 108.0;

pub fn draw(app: &mut App, dt: f32) -> bool {
    // Opened: the field takes the keyboard at once.
    if app.fresh && app.st.search.asked.is_none() && app.focus.id.is_none() && app.server.is_some() && !app.shot {
        app.focus.id = Some("search");
        app.focus.text = app.st.search.query.clone();
        app.focus.kind = Kb::Text;
        app.host.keyboard(true, &app.st.search.query, Kb::Text);
    }
    let submit = app.typed_done == Some("search");
    let pg = app.page_begin(dt, TOP);
    let now = app.now_ns;
    let st = &mut app.st.search;
    let mut cx = cx!(app);
    let x0 = PAD;
    let w = WIDTH - PAD * 2.0;
    let mut y = pg.top + 8.0;
    let mut follow: Option<Candidate> = None;
    let mut open: Option<i64> = None;
    let mut retry = false;
    let mut connect = false;
    let no_server = app.server.is_none();
    if let Some(e) = &st.follow_error {
        y += cx.c.para(e, x0, y, w, Text::new(ty::UI, t::DANGER).line(1.4), 3) + 12.0;
    }
    match (&st.asked, &st.results) {
        _ if no_server => {
            y += 60.0;
            let (h, c) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Comics come from your server", Some("Connect Leaf to your leaf-server to search its sources."), Some("Connect"));
            connect = c;
            y += h;
        }
        (None, _) => {
            y += 24.0;
            cx.c.para(
                "Search every source on your server at once. Adding a series puts it in My space; turn on its bell to hear when new chapters come.",
                x0 + 8.0,
                y,
                w - 16.0,
                Text::new(ty::UI, t::INK2).center().line(1.45),
                4,
            );
            y += 60.0;
        }
        (Some(q), None) => {
            y += 24.0;
            cx.c.text_fit(&format!("Searching your sources for \u{201C}{q}\u{201D}"), WIDTH / 2.0, y, w, Text::new(ty::UI, t::INK2).center());
            y += 40.0;
        }
        (Some(_), Some(Err(e))) => {
            y += 60.0;
            let (h, r) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, e, None, Some("Try again"));
            retry = r;
            y += h;
        }
        (Some(q), Some(Ok(list))) if list.is_empty() => {
            y += 40.0;
            let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &format!("Nothing found for \u{201C}{q}\u{201D}"), Some("Try the title as a source spells it, or a shorter part of it."), None);
            y += h;
        }
        (Some(_), Some(Ok(list))) => {
            let n = list.len();
            cx.c.text(&if n == 1 { "1 work".to_string() } else { format!("{n} works") }, x0, y, Text::new(ty::CAPTION, t::INK3).medium());
            y += 26.0;
            let view_top = -cx.c.dy - 40.0;
            let view_bottom = -cx.c.dy + cx.c.h + 40.0;
            for c in list {
                if y + ROW_H < view_top || y > view_bottom {
                    y += ROW_H;
                    continue;
                }
                let (cw, ch) = (64.0, 92.0);
                let cy = y + (ROW_H - ch) / 2.0;
                if cx.pressed(0.0, y, WIDTH, ROW_H) {
                    cx.c.rect(0.0, y, WIDTH, ROW_H, 0.0, t::WASH);
                }
                cx.c.rect(x0, cy, cw, ch, COVER_R - 2.0, t::S1);
                if let Some(m) = c.cover_of {
                    let of = CoverOf::Manga(m);
                    if app.images.cover(cx.c, of, now) {
                        cx.c.image_cover(&of.key(), x0, cy, cw, ch, COVER_R - 2.0, Rgba::WHITE);
                    }
                }
                let busy = st.following.as_deref() == Some(c.title.as_str());
                let words = if c.followed.is_some() {
                    "In My space"
                } else if busy {
                    "Adding…"
                } else {
                    "Add"
                };
                let (pw, pill_tap) = kit::pill(&mut cx, WIDTH - PAD, y + (ROW_H - 32.0) / 2.0, words, c.followed.is_some() || busy);
                let tx = x0 + cw + 14.0;
                let tw = WIDTH - PAD - pw - 12.0 - tx;
                let title_t = Text::new(ty::BODY, t::INK).medium().line(1.3);
                let lines = cx.c.wrap(&c.title, tw, title_t, 2);
                let line_t = Text::new(12.5, t::INK3);
                let block = lines.len() as f32 * title_t.lh() + 6.0 + line_t.lh();
                let mut ly = y + (ROW_H - block) / 2.0;
                for l in &lines {
                    cx.c.text(l, tx, ly, title_t);
                    ly += title_t.lh();
                }
                let found = match c.hits.len() {
                    1 => "Found on 1 source".to_string(),
                    n => format!("Found on {n} sources"),
                };
                cx.c.text_fit(&found, tx, ly + 6.0, tw, line_t);
                let row_tap = !pill_tap && cx.tap(0.0, y, WIDTH, ROW_H);
                match c.followed {
                    Some(id) if pill_tap || row_tap => open = Some(id),
                    None if (pill_tap || row_tap) && st.following.is_none() => follow = Some(c.clone()),
                    _ => {}
                }
                y += ROW_H;
            }
        }
    }
    let bottom = y;
    // The fixed top: the field (Back too, when Search was opened over another page).
    let mut back = false;
    let mut query = st.query.clone();
    let tabbed = app.nav.len() == 1;
    let bottom = if tabbed { bottom + kit::TABS_H } else { bottom };
    {
        let top = cx.c.inset_top;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + TOP, 0.0, t::BG);
        kit::fixed_top(&mut cx, TOP);
        let fx = if tabbed {
            PAD
        } else {
            if kit::icon_key(&mut cx, 6.0, top + 8.0, "arrow-left") {
                back = true;
            }
            54.0
        };
        let (fy, fw, fh) = (top + 8.0, WIDTH - fx - PAD, 44.0);
        cx.c.rect(fx, fy, fw, fh, t::KEY_R, t::S1);
        cx.c.icon("search", fx + 20.0, fy + fh / 2.0, 17.0, t::INK3);
        kit::inline(&mut cx, "search", fx + 38.0, fy, fw - 52.0, fh, "Search a title", &mut query, Kb::Text, Text::new(ty::BODY, t::INK));
    }
    st.query = query;
    let tab = if tabbed { kit::tabs(&mut cx, 1) } else { None };
    let searching = st.searching();
    app.page_end(&pg, bottom);
    let q = app.st.search.query.trim().to_string();
    if (submit || retry) && !q.is_empty() {
        let st = &mut app.st.search;
        st.asked = Some(q.clone());
        st.results = None;
        st.follow_error = None;
        app.nav.last_mut().expect("a page").1 = Default::default();
        app.ask(Job::Search(q));
    }
    if let Some(c) = follow {
        app.st.search.following = Some(c.title.clone());
        app.st.search.follow_error = None;
        app.ask(Job::Follow(Follow { title: c.title.clone(), manga_ids: c.hits.iter().map(|h| h.manga_id).collect(), quiet: false }));
    }
    if let Some(id) = open {
        app.push(Route::Series(id));
    } else if connect {
        app.push(Route::Connect);
    } else if back {
        app.pop();
    } else if let Some(i) = tab {
        app.go_tab(i);
    }
    pg.moving || searching && !crate::wake::has()
}
