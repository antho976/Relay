//! Whether the server answers, and what the app says when it stops. Every live answer says so
//! ([`crate::net::Done::reach`]): one that could not reach the server (no network, the server off,
//! a proxy saying it is down) begins a spell out of reach; any answer of the server's ends it.
//! Nothing here asks the platform about the network: the app's own requests already know.
//!
//! Once a spell, a calm sheet says "You're offline" over the page (never over a strip or a book
//! being read: it waits until the reader closes), with Open downloads (My space › Downloads) and
//! Not now, or only OK when nothing is downloaded. While out of reach the server is tried again
//! every [`PROBE_NS`] (a light `/v1/status`) and whenever the app comes back to the front; when it
//! answers, a quiet "Back online" and the open lists are asked again.

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::net::Job;
use crate::theme::{self as t, PAD, ty};

/// Out of reach, the server is tried again this often.
pub const PROBE_NS: u64 = 20_000_000_000;

#[derive(Default)]
pub struct State {
    /// What the last live answer said: Some(true) the server answered, Some(false) it could not be
    /// reached, None nothing heard yet.
    pub reach: Option<bool>,
    /// The prompt has shown in this spell out of reach (it shows once a spell).
    pub prompted: bool,
    /// The prompt is showing.
    pub open: bool,
    /// When the server was last tried while out of reach ([`crate::clock::now_ns`]'s clock).
    probed_ns: u64,
    /// No prompt and no tries (tests of other flows, on a server that isn't there).
    pub off: bool,
}

impl State {
    pub fn offline(&self) -> bool {
        self.reach == Some(false)
    }
}

/// A live answer came from the server (`reached`), or none could.
pub fn heard(app: &mut App, reached: bool) {
    let st = &mut app.st.online;
    if !reached {
        if st.reach != Some(false) {
            st.probed_ns = app.now_ns;
        }
        st.reach = Some(false);
        return;
    }
    let back = st.reach == Some(false);
    st.reach = Some(true);
    st.prompted = false;
    st.open = false;
    if back && !st.off {
        app.toast("Back online");
        app.images.retry_failed();
        if !app.library.loading {
            app.ask(Job::Library);
        }
    }
}

/// Out of reach: try the server again, `now` or once [`PROBE_NS`] passed since the last try.
pub fn probe(app: &mut App, now: bool) {
    let st = &app.st.online;
    if st.off || !st.offline() || app.server.is_none() {
        return;
    }
    let due = st.probed_ns + PROBE_NS;
    if now || app.now_ns >= due {
        app.st.online.probed_ns = app.now_ns;
        let s = app.server.clone().expect("a server");
        app.ask(Job::Status(s));
        app.wake_at(app.now_ns + PROBE_NS);
    } else {
        app.wake_at(due);
    }
}

/// The prompt may show over this page: not over a strip or a book being read, nor Settings'
/// server page.
pub fn shows_over(route: &Route) -> bool {
    !matches!(route, Route::Reader | Route::Book | Route::Connect)
}

/// The prompt shows over `route` this frame: opened the first time a spell out of reach finds a
/// page it may show over (once the downloads on this phone are known).
pub fn shows(app: &mut App, route: &Route) -> bool {
    let st = &app.st.online;
    if !st.off && st.offline() && !st.prompted && app.server.is_some() && shows_over(route) && app.saver.scanned() {
        app.st.online.open = true;
        app.st.online.prompted = true;
    }
    app.st.online.open && shows_over(route)
}

/// The words under the title: what still opens.
pub fn line(chapters: usize, books: bool) -> String {
    match (chapters, books) {
        (0, true) => "Nothing is downloaded on this phone yet. Your books still open.".into(),
        (0, false) => "Nothing is downloaded on this phone yet.".into(),
        (n, _) => format!("Your downloads still open: {} on this phone.", crate::logic::chapters_words(n)),
    }
}

/// The sheet, over the page (which takes no tap while it shows: [`App::frame`] holds it).
pub fn draw(app: &mut App) {
    let chapters: usize = app.saver.groups().iter().map(|g| g.chapters).sum();
    let words = line(chapters, !app.shelf.books.is_empty());
    let mut cx = cx!(app);
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let icon = 30.0;
    let tt = Text::new(ty::SECTION, t::INK).semi();
    let lt = Text::new(ty::UI, t::INK2).line(1.4);
    let lh = cx.c.para_h(&words, w, lt, 4);
    let (top, outside) = kit::sheet(&mut cx, 30.0 + icon + 14.0 + tt.lh() + 8.0 + lh + 22.0 + 48.0 + 18.0);
    let mut y = top + 30.0;
    cx.c.icon("cloud-off", x + icon / 2.0, y + icon / 2.0, icon, t::INK2);
    y += icon + 14.0;
    cx.c.text_fit("You're offline", x, y, w, tt);
    y += tt.lh() + 8.0;
    cx.c.para(&words, x, y, w, lt, 4);
    y += lh + 22.0;
    let mut close = outside;
    let mut go = false;
    if chapters > 0 {
        let bw = (w - 10.0) / 2.0;
        close |= kit::plain(&mut cx, x, y, bw, 48.0, "Not now", None);
        go = kit::primary(&mut cx, x + bw + 10.0, y, bw, 48.0, "Open downloads", None);
    } else {
        close |= kit::primary(&mut cx, x, y, w, 48.0, "OK", None);
    }
    // Every tap is the sheet's: none falls through to the page.
    cx.ui.tap = None;
    if go {
        app.st.online.open = false;
        app.st.space.tab = crate::space::DOWNLOADS;
        app.st.space.select = None;
        app.reset_to(Route::Space);
    } else if close {
        app.st.online.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_says_what_still_opens() {
        assert_eq!(line(12, false), "Your downloads still open: 12 chapters on this phone.");
        assert_eq!(line(1, true), "Your downloads still open: 1 chapter on this phone.");
        assert_eq!(line(0, true), "Nothing is downloaded on this phone yet. Your books still open.");
        assert_eq!(line(0, false), "Nothing is downloaded on this phone yet.");
    }
}
