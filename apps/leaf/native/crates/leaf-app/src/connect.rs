//! Connect: the server's address and its token, checked against `/v1/status`. The first screen of
//! a first run; after that this route is Settings ([`crate::settings`]), one page where the
//! address and token fold out of the Server section's Change.

use leaf_proto::Status;

use crate::app::{App, Route};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Kb};
use crate::logic;
use crate::net::{Job, Server};
use crate::theme::{self as t, PAD, ty};

#[derive(Default)]
pub struct State {
    pub address: String,
    pub token: String,
    /// The base last sent to `/v1/status`.
    pub tried: String,
    pub checking: bool,
    /// What the server said: its status, or why it could not be reached.
    pub result: Option<Result<Status, String>>,
    /// The address doesn't read as one.
    pub invalid: Option<&'static str>,
    /// A server that answered, waiting to be kept.
    answered_ok: Option<Server>,
    /// The AniList token being typed.
    pub anilist: String,
    /// Settings: the address and token are folded out (Change).
    pub editing: bool,
    /// Settings: AniList's setup steps are showing.
    pub anilist_open: bool,
}

impl State {
    /// Fill the fields with the server in use.
    pub fn load(&mut self, s: Option<&Server>) {
        if let Some(s) = s {
            // The label keeps the scheme whenever typing it bare would give another address.
            self.address = logic::server_label(&s.base);
            self.token = s.token.clone().unwrap_or_default();
        }
    }

    pub fn answered(&mut self, s: Server, r: Result<Status, String>) {
        if s.base != self.tried {
            return;
        }
        self.checking = false;
        if r.is_ok() {
            self.answered_ok = Some(s);
        }
        self.result = Some(r);
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let first_run = app.nav.len() == 1;
    // Settings: what the server says of itself (AniList) and of its sources, asked again each time.
    if app.fresh
        && !first_run
        && let Some(s) = app.server.clone()
    {
        app.status.loading = true;
        app.jobs.send(Some(&s), Job::Status(s.clone()));
        app.ask(Job::Health);
        app.ask(Job::HiddenPages);
        app.ask(Job::ServerInfo);
    }
    let pg = app.page_begin(dt, 56.0);
    if let Some(s) = app.st.connect.answered_ok.take() {
        app.set_server(s);
        // Settings: the new server answered. Fold the fields away and ask it about itself.
        if !first_run {
            app.st.connect.editing = false;
            if let Some(Ok(status)) = &app.st.connect.result {
                app.status.data = Some(status.clone());
                app.status.error = None;
            }
            app.ask(Job::Health);
            app.ask(Job::HiddenPages);
            app.ask(Job::ServerInfo);
        }
    }
    let label = app.server_label();
    let was_editing = app.st.connect.editing;
    let mut connect = false;
    let mut open_library = false;
    let st = &mut app.st.connect;
    let mut cx = cx!(app);
    let x = PAD;
    let w = WIDTH - PAD * 2.0;
    let mut y = pg.top + 8.0;
    let mut acts = Vec::new();
    if first_run || app.server.is_none() {
        cx.c.text("Connect to your server", x, y, Text::new(ty::HEADING, t::INK).brand());
        y += 34.0;
        y += cx.c.para("Leaf reads the comics your server fetches. Type the address it listens on, and its token if it has one.", x, y, w, Text::new(ty::UI, t::INK2).line(1.45), 4) + 24.0;
        let mut address = st.address.clone();
        y += kit::field(&mut cx, "address", x, y, w, "Address", "192.168.1.20:4580", &mut address, Kb::Url) + 16.0;
        st.address = address;
        let mut token = st.token.clone();
        y += kit::field(&mut cx, "token", x, y, w, "Token", "Only if your server has one", &mut token, Kb::Secret) + 24.0;
        st.token = token;
        let words = if st.checking { "Checking" } else { "Connect" };
        if kit::primary(&mut cx, x, y, w, 50.0, words, None) && !st.checking {
            connect = true;
        }
        y += 50.0 + 24.0;
        if let Some(why) = st.invalid {
            cx.c.text(why, x, y, Text::new(ty::UI, t::DANGER));
            y += 24.0;
        }
        match &st.result {
            Some(Ok(status)) => {
                cx.c.icon("check", x + 9.0, y + 10.0, 18.0, t::ACCENT);
                cx.c.text_fit(&format!("Connected to {label}"), x + 28.0, y, w - 28.0, Text::new(ty::BODY, t::INK).semi());
                y += 28.0;
                let mut lines = Vec::new();
                lines.push(match &status.suwayomi {
                    Some(v) => format!("Suwayomi {v}"),
                    None => "Suwayomi isn't answering your server".into(),
                });
                // How many, not which: no source is named anywhere in the app.
                match status.sources.len() {
                    0 => lines.push("No sources installed yet".into()),
                    1 => lines.push("1 source".into()),
                    n => lines.push(format!("{n} sources")),
                }
                lines.push(match status.series {
                    0 => "No series followed yet".to_string(),
                    1 => "1 series followed".to_string(),
                    n => format!("{n} series followed"),
                });
                for l in lines {
                    y += cx.c.para(&l, x + 28.0, y, w - 28.0, Text::new(ty::UI, t::INK2).line(1.45), 3) + 2.0;
                }
                y += 18.0;
                if kit::plain(&mut cx, x, y, w, 48.0, if first_run { "Open Leaf" } else { "Done" }, None) {
                    open_library = true;
                }
                y += 48.0;
            }
            Some(Err(why)) => {
                y += cx.c.para(why, x, y, w, Text::new(ty::UI, t::DANGER).line(1.45), 4);
            }
            None => {}
        }
    } else {
        let failed = match &st.result {
            Some(Err(why)) if !st.checking => Some(why.as_str()),
            _ => None,
        };
        let parts = crate::settings::Parts {
            prefs: &mut app.prefs,
            notify: &mut app.notify,
            status: &app.status,
            health: &app.health,
            backup: app.backup.as_ref(),
            backing_up: app.backing_up,
            books: crate::settings::Books {
                copied: app.books_copied,
                busy: app.booksync.busy(),
                restoring: app.booksync.restoring(),
                found: app.booksync.found,
                error: app.booksync.error.as_deref(),
                too_big: app.booksync.too_big,
                here: app.shelf.books.len(),
                confirm: app.restore_asked,
            },
            hidden: app.hidden_pages.data,
            server: &app.server_info,
            anilist: &mut st.anilist,
            anilist_open: &mut st.anilist_open,
            conn: crate::settings::Conn {
                label: label.clone(),
                editing: &mut st.editing,
                address: &mut st.address,
                token: &mut st.token,
                checking: st.checking,
                failed,
                invalid: st.invalid,
            },
            now_ms: crate::shelf::now_ms(),
        };
        let (end, a) = crate::settings::draw(&mut cx, parts, x, y, w, app.now_ns);
        y = end + 32.0;
        acts = a;
    }
    // The fixed top: the wordmark on a first run, Back in Settings.
    let mut back = false;
    {
        let top = cx.c.inset_top;
        let dy = cx.c.dy;
        cx.c.dy = 0.0;
        cx.c.rect(-1000.0, 0.0, WIDTH + 2000.0, top + 56.0, 0.0, t::BG);
        kit::fixed_top(&mut cx, 56.0);
        if first_run {
            cx.c.text("Leaf", PAD, top + 12.0, Text::new(24.0, t::INK).brand());
        } else {
            let (b, _, _) = kit::top_bar(&mut cx, top + 6.0, "Settings", &[]);
            back = b;
        }
        cx.c.dy = dy;
    }
    let done = app.typed_done;
    app.page_end(&pg, y);
    let busy = app.health.loading || app.backing_up;
    for a in acts {
        use crate::settings::Act;
        match a {
            Act::Save => app.save_config(),
            Act::Trim(on) => app.images.retrim(on),
            Act::Backup(now) => {
                app.backing_up = true;
                app.ask(Job::Backup(now));
                if app.shot {
                    app.backing_up = false;
                }
            }
            Act::Anilist(token) => {
                app.finish_typing();
                app.typed_done = None;
                app.ask(Job::Anilist(token));
            }
            Act::Health => app.ask(Job::Health),
            Act::ServerInfo => app.ask(Job::ServerInfo),
            Act::DiskCap(gb) => {
                if let Some(i) = &mut app.server_info.data {
                    i.room.cap_gb = gb;
                }
                app.ask(Job::DiskCap(gb));
            }
            Act::Promo(skip) => app.images.set_promo(skip),
            Act::ClearHidden => {
                app.ask(Job::ClearHidden);
                // Asked again: the server says which still to skip (its own rule's pages).
                app.images.clear_skips();
                app.toast("Pages you hid will show again");
            }
            Act::CopyBooks(over) => app.copy_books(over),
            Act::RestoreBooks => app.restore_books(),
            Act::KeepBooks => app.restore_asked = false,
            Act::Copy(words) => {
                app.host.copy_text(words);
                app.toast("Copied");
            }
            Act::Connect => connect = true,
        }
    }
    if done == Some("anilist") && !app.st.connect.anilist.trim().is_empty() {
        let token = app.st.connect.anilist.trim().to_string();
        app.ask(Job::Anilist(Some(token)));
    }
    // Done on the token's keyboard connects, as the key would.
    if done == Some("token") {
        connect = true;
    }
    // Change or Cancel: the fields show the server in use again, and no old answer.
    if app.st.connect.editing != was_editing && !connect {
        let st = &mut app.st.connect;
        st.load(app.server.as_ref());
        st.result = None;
        st.invalid = None;
        if !st.editing {
            app.finish_typing();
        }
    }
    if connect {
        start(app);
    }
    if open_library {
        app.reset_to(Route::Home);
    }
    if back {
        app.pop();
    }
    if busy {
        app.wake_at(app.now_ns + 50_000_000);
    }
    pg.moving
}

/// Check the address typed (Connect, or Done on the token's keyboard).
pub fn start(app: &mut App) {
    let st = &mut app.st.connect;
    st.invalid = None;
    match logic::normalize_server(&st.address) {
        Ok(base) => {
            let token = st.token.trim();
            let s = Server { base: base.clone(), token: (!token.is_empty()).then(|| token.to_string()), alt: None };
            st.tried = base;
            st.checking = true;
            st.result = None;
            app.finish_typing();
            app.typed_done = None;
            app.jobs.send(Some(&s), Job::Status(s.clone()));
        }
        Err(why) => st.invalid = Some(why),
    }
}
