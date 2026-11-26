//! Search, the second tab: one field, one row per work. The server asks a few broad sources
//! (`/v1/search?source=`, one request each, so results show as they come) and keeps only titles
//! with every word searched; "Search all sources" under the results asks the rest. A search goes
//! once the typing pauses ([`PAUSE_NS`]) or on Enter, never for fewer than [`MIN_CHARS`]
//! characters, never twice for the same words. A word may be off by a letter or two
//! ([`leaf_proto::title_has_words`]); when nothing is found, AniList is asked once what the words
//! probably meant (`/v1/search/suggest`), offered as "Did you mean …?", which searches that.
//!
//! A row opens the title's page: a series already added opens as it is; any other as a preview
//! (`/v1/preview`, [`App::open_preview`]), with Follow and Download, nothing added until a chapter
//! is read, it is followed or something is downloaded (the owner, 2026-11-23). Sources are not
//! named: which one a series comes from is the server's business.

use leaf_proto::{Candidate, Series};

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::images::CoverOf;
use crate::kit::{self, Kb};
use crate::net::Job;
use crate::theme::{self as t, COVER_R, PAD, ty};

/// A search goes this long after the last key, unless Enter sends it first.
pub const PAUSE_NS: u64 = 700_000_000;
/// Fewer characters than this are not searched.
pub const MIN_CHARS: usize = 3;
/// Series of yours shown above the sources' answer.
pub const MINE_SHOWN: usize = 5;
/// The most works shown (the server sends at most this many per source).
pub const MAX_SHOWN: usize = 20;

#[derive(Default)]
pub struct State {
    pub query: String,
    /// The query last sent, and what came back for it so far.
    pub asked: Option<String>,
    /// That search asks every source ("Search all sources").
    pub all: bool,
    pub results: Option<Result<Vec<Candidate>, String>>,
    /// Parts of the search still to come.
    pub more: bool,
    /// The next part replaces what is shown (the all-sources answer), rather than joining it.
    replace: bool,
    /// The words as last seen, and when they last changed.
    typed: String,
    typed_ns: u64,
    /// "Did you mean": the words it was asked for, and what AniList said they meant.
    pub suggest: Option<(String, Option<String>)>,
    /// Words that found nothing, to ask AniList about (the page sends it).
    pub suggest_due: Option<String>,
}

/// One source's works joined to those shown: the same work (as the server tells works apart)
/// gains its hits; found on more sources first; at most [`MAX_SHOWN`].
pub fn merge(shown: &mut Vec<Candidate>, more: Vec<Candidate>) {
    for c in more {
        match shown.iter_mut().find(|s| leaf_proto::same_work(&s.title, &c.title)) {
            Some(s) => {
                for h in c.hits {
                    if !s.hits.iter().any(|x| x.manga_id == h.manga_id || x.source == h.source) {
                        s.hits.push(h);
                    }
                }
                s.followed = s.followed.or(c.followed);
            }
            None => shown.push(c),
        }
    }
    shown.sort_by_key(|c| std::cmp::Reverse(c.hits.len()));
    shown.truncate(MAX_SHOWN);
}

/// Whether `q` (trimmed) is to be searched now: long enough, not the words last searched, and
/// either sent (Enter) or the typing paused.
pub fn due(q: &str, asked: Option<&str>, sent: bool, typed_ns: u64, now_ns: u64) -> bool {
    q.chars().count() >= MIN_CHARS && asked != Some(q) && (sent || now_ns.saturating_sub(typed_ns) >= PAUSE_NS)
}

impl State {
    /// An answer, or one source's part of it.
    pub fn answered(&mut self, q: String, all: bool, r: Result<Vec<Candidate>, String>, last: bool) {
        if self.asked.as_deref() != Some(q.as_str()) || self.all != all {
            return;
        }
        match r {
            Ok(list) => match &mut self.results {
                Some(Ok(shown)) if !self.replace => merge(shown, list),
                _ => {
                    let mut shown = Vec::new();
                    merge(&mut shown, list);
                    self.results = Some(Ok(shown));
                }
            },
            // A source that failed leaves the others' results; alone, it says why.
            Err(e) => {
                if self.results.is_none() && last {
                    self.results = Some(Err(e));
                }
            }
        }
        if last {
            self.more = false;
            self.replace = false;
            if self.results.is_none() {
                self.results = Some(Ok(Vec::new()));
            }
            // Nothing at all: what did the words mean? Asked once for the same words.
            if matches!(&self.results, Some(Ok(l)) if l.is_empty()) && self.suggest.as_ref().is_none_or(|(sq, _)| *sq != q) {
                self.suggest = Some((q.clone(), None));
                self.suggest_due = Some(q);
            }
        } else if self.results.is_some() {
            self.replace = false;
        }
    }

    /// The search sent: the broad sources, or (`all`) every one, the results shown kept until it
    /// answers.
    fn ask(&mut self, q: &str, all: bool) {
        self.asked = Some(q.to_string());
        self.all = all;
        self.more = true;
        if all {
            self.replace = true;
        } else {
            self.results = None;
            self.replace = false;
        }
    }

    /// AniList's answer for words that found nothing.
    pub fn suggested(&mut self, q: String, title: Option<String>) {
        if self.suggest.as_ref().is_some_and(|(sq, _)| *sq == q) {
            self.suggest = Some((q, title));
        }
    }

    /// The title "Did you mean" offers for the words shown, if any.
    pub fn meant(&self) -> Option<&str> {
        let (q, t) = self.suggest.as_ref()?;
        (self.asked.as_deref() == Some(q.as_str())).then_some(())?;
        t.as_deref()
    }

    pub fn followed(&mut self, s: &Series) {
        if let Some(Ok(list)) = &mut self.results {
            for c in list.iter_mut() {
                if leaf_proto::same_work(&c.title, &s.title) || c.title == s.title {
                    c.followed = Some(s.id);
                }
            }
        }
    }

    fn searching(&self) -> bool {
        self.asked.is_some() && (self.results.is_none() || self.more)
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
    let mut preview: Option<Candidate> = None;
    let mut open: Option<i64> = None;
    let mut meant: Option<String> = None;
    let mut retry = false;
    let mut search_all = false;
    let mut connect = false;
    let no_server = app.server.is_none();
    let did_you_mean = st.meant().map(str::to_string);
    // Yours first: series in My space whose title holds the words, opened with no source asked.
    let typed = st.query.trim().to_string();
    let mine: Vec<Series> = if typed.chars().count() >= 2 {
        app.library.data.iter().flatten().filter(|s| crate::logic::title_matches(&s.title, &typed)).take(MINE_SHOWN).cloned().collect()
    } else {
        Vec::new()
    };
    // The very title of one of yours: the sources are asked only when you say so.
    let exact = mine.iter().any(|s| crate::logic::fold(&s.title) == crate::logic::fold(&typed));
    let mut forget: Option<usize> = None;
    let mut again: Option<String> = None;
    let mut sources_too = false;
    if !mine.is_empty() {
        cx.c.text("IN MY SPACE", x0, y, Text::new(ty::SMALL, t::INK3).semi());
        y += 22.0;
        for s in &mine {
            let rh = 64.0;
            if cx.pressed(0.0, y, WIDTH, rh) {
                cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
            }
            let of = CoverOf::Series(s.id);
            let (cw, ch) = (40.0, 56.0);
            cx.c.rect(x0, y + (rh - ch) / 2.0, cw, ch, 6.0, t::S1);
            if app.images.cover(cx.c, of, now) {
                cx.c.image_cover(&of.key(), x0, y + (rh - ch) / 2.0, cw, ch, 6.0, Rgba::WHITE);
            }
            let tx = x0 + cw + 14.0;
            let tw = WIDTH - PAD - 24.0 - tx;
            let title_t = Text::new(ty::BODY, t::INK).medium();
            let line_t = Text::new(12.5, t::INK3);
            let ty0 = y + (rh - title_t.lh() - 4.0 - line_t.lh()) / 2.0;
            cx.c.text_fit(&s.title, tx, ty0, tw, title_t);
            let line = match (s.read_upto, s.latest) {
                (Some(r), Some(l)) => format!("Read to chapter {} of {}", crate::logic::num(r), crate::logic::num(l)),
                (Some(r), None) => format!("Read to chapter {}", crate::logic::num(r)),
                _ => "Not started".to_string(),
            };
            cx.c.text_fit(&line, tx, ty0 + title_t.lh() + 4.0, tw, line_t);
            cx.c.icon("chevron-right", WIDTH - PAD - 10.0, y + rh / 2.0, 16.0, t::INK3);
            if cx.tap(0.0, y, WIDTH, rh) {
                open = Some(s.id);
            }
            y += rh;
        }
        y += 14.0;
        if exact && st.asked.as_deref() != Some(typed.as_str()) && !no_server {
            if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Search your sources too", Some("search")) {
                sources_too = true;
            }
            y += 46.0 + 14.0;
        }
    }
    match (&st.asked, &st.results) {
        _ if no_server => {
            y += 60.0;
            let (h, c) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, "Comics come from your server", Some("Connect Leaf to your leaf-server to search its sources."), Some("Connect"));
            connect = c;
            y += h;
        }
        (None, _) if typed.is_empty() && !app.searches.is_empty() => {
            // The searches that led somewhere, newest first: a tap searches it again.
            y += 8.0;
            cx.c.text("RECENT", x0, y, Text::new(ty::SMALL, t::INK3).semi());
            y += 14.0;
            for (i, q) in app.searches.iter().enumerate() {
                let rh = 48.0;
                if cx.pressed(0.0, y, WIDTH - 56.0, rh) {
                    cx.c.rect(0.0, y, WIDTH, rh, 0.0, t::WASH);
                }
                cx.c.icon("history", x0 + 9.0, y + rh / 2.0, 16.0, t::INK3);
                cx.c.text_fit(q, x0 + 32.0, y + (rh - Text::new(ty::BODY, t::INK).lh()) / 2.0, w - 32.0 - 44.0, Text::new(ty::BODY, t::INK));
                if kit::icon_key_ink(&mut cx, WIDTH - PAD - 40.0, y + 2.0, "close", t::INK3) {
                    forget = Some(i);
                } else if cx.tap(0.0, y, WIDTH - 56.0, rh) {
                    again = Some(q.clone());
                }
                y += rh;
            }
        }
        (None, _) if !mine.is_empty() => {}
        (None, _) => {
            y += 24.0;
            cx.c.para(
                "Type a title: Leaf searches your server's main sources as you type. A title opens its page; reading a chapter, following it or downloading one puts it in My space.",
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
        (Some(q), Some(Ok(list))) if list.is_empty() && !st.more => {
            y += 40.0;
            let (h, _) = kit::notice(&mut cx, x0 + 16.0, y, w - 32.0, &format!("Nothing found for \u{201C}{q}\u{201D}"), Some("Try the title as a source spells it, or a shorter part of it."), None);
            y += h + 16.0;
            if let Some(t) = &did_you_mean {
                let (h, tapped) = did_you_mean_row(&mut cx, x0, y, w, t);
                if tapped {
                    meant = Some(t.clone());
                }
                y += h + 12.0;
            }
            if !st.all && kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Search all sources", Some("search")) {
                search_all = true;
            }
            y += 46.0;
        }
        (Some(_), Some(Ok(list))) => {
            let n = list.len();
            let ct = Text::new(ty::CAPTION, t::INK3).medium();
            let words = if n == 1 { "1 work".to_string() } else { format!("{n} works") };
            cx.c.text(&words, x0, y, ct);
            if st.more {
                let ww = cx.c.measure(&words, ct);
                cx.c.text(if st.all { "· searching every source" } else { "· still searching" }, x0 + ww + 6.0, y, ct.color(t::INK3.a(0.7)));
            }
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
                // Added already: said quietly. Any row opens the title's page.
                let pw = if c.followed.is_some() {
                    kit::pill(&mut cx, WIDTH - PAD, y + (ROW_H - 32.0) / 2.0, "Added", true).0
                } else {
                    cx.c.icon("chevron-right", WIDTH - PAD - 10.0, y + ROW_H / 2.0, 16.0, t::INK3);
                    24.0
                };
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
                // Only Webtoon has it, and sells its newer chapters: said before you open it.
                if let Some(p) = &c.paid {
                    let range = if p.from == p.to { format!("chapter {}", crate::logic::num(p.from)) } else { format!("chapters {}–{}", crate::logic::num(p.from), crate::logic::num(p.to)) };
                    let warn = line_t.color(t::WARN);
                    cx.c.icon("coins", tx + 7.0, ly + 6.0 + warn.lh() / 2.0, 14.0, t::WARN);
                    cx.c.text_fit(&format!("Webtoon only, {range} paid"), tx + 20.0, ly + 6.0, tw - 20.0, warn);
                } else {
                    let found = match c.hits.len() {
                        1 => "Found on 1 source".to_string(),
                        n => format!("Found on {n} sources"),
                    };
                    cx.c.text_fit(&found, tx, ly + 6.0, tw, line_t);
                }
                if cx.tap(0.0, y, WIDTH, ROW_H) {
                    match c.followed {
                        Some(id) => open = Some(id),
                        None => preview = Some(c.clone()),
                    }
                }
                y += ROW_H;
            }
            // The rest of the sources, only when asked (each search costs every source a request).
            if !st.all && !st.more {
                y += 14.0;
                if kit::plain(&mut cx, x0 + 40.0, y, w - 80.0, 46.0, "Search all sources", Some("search")) {
                    search_all = true;
                }
                y += 46.0 + 8.0;
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
    let st = &mut app.st.search;
    if st.typed != q {
        st.typed = q.clone();
        st.typed_ns = now;
    }
    if retry && !q.is_empty() {
        // Asked again by hand: the same words may go twice.
        st.asked = None;
    }
    let all_of = st.asked.clone().filter(|_| search_all);
    if let Some(asked) = all_of {
        st.ask(&asked, true);
        app.ask(Job::Search(asked, true));
    } else if (sources_too || (!exact || submit) && due(&q, st.asked.as_deref(), submit || retry, st.typed_ns, now)) && app.server.is_some() {
        st.ask(&q, false);
        app.nav.last_mut().expect("a page").1 = Default::default();
        app.ask(Job::Search(q, false));
    } else if !exact && q.chars().count() >= MIN_CHARS && st.asked.as_deref() != Some(q.as_str()) {
        let at = st.typed_ns + PAUSE_NS;
        app.wake_at(at);
    }
    if let Some(q) = app.st.search.suggest_due.take() {
        app.ask(Job::Suggest(q));
    }
    let meant = meant.or(again);
    if let Some(t) = meant {
        // "Did you mean": those words, searched now (in the field too, as if typed).
        app.st.search.query = t.clone();
        if app.focus.is("search") {
            app.focus.text = t.clone();
            app.host.keyboard(true, &t, Kb::Text);
        }
        let st = &mut app.st.search;
        st.typed = t.clone();
        st.typed_ns = now;
        st.ask(&t, false);
        app.nav.last_mut().expect("a page").1 = Default::default();
        app.ask(Job::Search(t, false));
    }
    if let Some(i) = forget {
        app.searches.remove(i);
        app.save_config();
    }
    if preview.is_some() || open.is_some() {
        let q = app.st.search.query.clone();
        app.remember_search(&q);
    }
    if let Some(c) = preview {
        app.open_preview(&c.title, &c.hits, None);
    } else if let Some(id) = open {
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

/// "Did you mean The Awakening Contract?", the title in the accent, the whole line a key. Returns
/// its height and whether it was tapped.
fn did_you_mean_row(cx: &mut kit::Ctx, x: f32, y: f32, w: f32, title: &str) -> (f32, bool) {
    let lead = "Did you mean ";
    let tt = Text::new(ty::BODY, t::INK2);
    let at = tt.color(t::ACCENT).semi();
    let h = 44.0;
    let lw = cx.c.measure(lead, tt);
    let shown = cx.c.fit(&format!("{title}?"), (w - lw).max(40.0), at);
    let total = lw + cx.c.measure(&shown, at);
    let x0 = x + ((w - total) / 2.0).max(0.0);
    if cx.pressed(x, y, w, h) {
        cx.c.rect(x, y, w, h, t::KEY_R, t::WASH);
    }
    let ty0 = y + (h - tt.lh()) / 2.0;
    cx.c.text(lead, x0, ty0, tt);
    cx.c.text(&shown, x0 + lw, ty0, at);
    (h, cx.tap(x, y, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, manga: i64, source: &str) -> Candidate {
        Candidate { title: title.into(), cover_of: Some(manga), hits: vec![leaf_proto::Hit { manga_id: manga, source: source.into() }], followed: None, paid: None }
    }

    #[test]
    fn a_search_waits_for_the_typing_to_pause_and_never_goes_twice() {
        let s = 1_000_000_000u64;
        assert!(!due("so", None, true, 0, 10 * s), "two characters, even with Enter");
        assert!(!due("sol", None, false, s, s + PAUSE_NS - 1), "still typing");
        assert!(due("sol", None, false, s, s + PAUSE_NS), "paused");
        assert!(due("sol", None, true, s, s), "Enter goes at once");
        assert!(!due("sol", Some("sol"), true, 0, 10 * s), "never the same words twice");
        assert!(due("solo", Some("sol"), false, 0, 10 * s));
    }

    #[test]
    fn results_join_as_each_source_answers() {
        let mut st = State::default();
        st.ask("solo leveling", false);
        st.answered("solo leveling".into(), false, Ok(vec![cand("Solo Leveling", 1, "A")]), false);
        assert_eq!(st.results.as_ref().unwrap().as_ref().unwrap().len(), 1, "shown at once");
        assert!(st.searching(), "more to come");
        // An old search's answer is not shown.
        st.answered("solo".into(), false, Ok(vec![cand("Solo", 9, "A")]), true);
        // A failed source leaves the rest; the same work found again gains a hit.
        st.answered("solo leveling".into(), false, Err("down".into()), false);
        st.answered("solo leveling".into(), false, Ok(vec![cand("Solo Leveling: Ragnarok", 3, "B"), cand("solo leveling", 2, "B")]), true);
        let list = st.results.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(list.iter().map(|c| (c.title.as_str(), c.hits.len())).collect::<Vec<_>>(), vec![("Solo Leveling", 2), ("Solo Leveling: Ragnarok", 1)]);
        assert!(!st.searching());
        // Every source: what is shown stays until that answer replaces it.
        st.ask("solo leveling", true);
        assert_eq!(st.results.as_ref().unwrap().as_ref().unwrap().len(), 2);
        st.answered("solo leveling".into(), true, Ok(vec![cand("Solo Leveling", 1, "A")]), true);
        assert_eq!(st.results.as_ref().unwrap().as_ref().unwrap().len(), 1);
        // Never more than twenty.
        let mut shown = Vec::new();
        merge(&mut shown, (0..30).map(|i| cand(&format!("Work number {i} of many"), i, "A")).collect());
        assert_eq!(shown.len(), MAX_SHOWN);
    }
}
