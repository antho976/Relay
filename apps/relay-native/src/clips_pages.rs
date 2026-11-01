//! The Clips space, the fourth beside Dev, Threads and Aide (docs/CLIPS.md): a stream's VODs in,
//! vertical Shorts out. The person imports a VOD (a file or a link) and says what they want from
//! it, in chips and in words; the engine transcribes it, finds the moments, frames, captions and
//! draws each one (`clips.*`), and the person reviews them here, edits what needs it, approves
//! them and sends them to their YouTube channel.
//!
//! The sidebar ([`sidebar`]) is Import, the Overview (what waits for review), the Queue (what goes
//! out next and what went out), the VODs, and Settings at its foot. A VOD's page shows its stages
//! while it is worked on and its clips as a grid; a clip opens in the editor ([`CLIP`]): the
//! drawn clip playing beside its words, its spans, framing, captions and hook, its title and tags.
//!
//! Everything is the engine's; this module reads it and sends the person's changes.
use super::{message, page, toast, Page};
use crate::app::{button, clear, confirm_inline, label, rows, text, Ui};
use crate::client::Error;
use crate::panel::Panel;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub const HOME: &str = "clips-home";
pub const VOD: &str = "clips-vod";
pub const CLIP: &str = "clips-clip";
pub const QUEUE: &str = "clips-queue";
pub const SETTINGS: &str = "clips-settings";
/// Every Clips page, for `money::add_pages`.
pub const ALL: [&str; 5] = [HOME, VOD, CLIP, QUEUE, SETTINGS];
/// The pages the palette lists: (stack name, caption).
pub const PAGES: [(&str, &str); 3] = [(HOME, "Overview"), (QUEUE, "Queue"), (SETTINGS, "Settings")];

/// Moment kinds: (stored, chip caption).
const KINDS: [(&str, &str); 8] = [
    ("funny", "Funny"),
    ("fail", "Fails"),
    ("story", "Stories"),
    ("guide", "Guides & tips"),
    ("hype", "Hype"),
    ("wholesome", "Wholesome"),
    ("rage", "Rage"),
    ("other", "Other"),
];
/// The kinds offered when asking (Other is the finder's, not something to ask for).
const ASKABLE: usize = 7;

/// A VOD's stages: (stored, what it says while it runs).
const STAGES: [(&str, &str); 8] = [
    ("probe", "Reading"),
    ("audio", "Sound"),
    ("transcribe", "Transcript"),
    ("signals", "Loudness"),
    ("layout", "Facecam"),
    ("find", "Moments"),
    ("cut", "Cutting"),
    ("render", "Drawing"),
];

/// Framings: (stored, caption, what it is). Split, Cam and Game need a facecam.
const LAYOUTS: [(&str, &str, &str); 5] = [
    ("split", "Split", "The facecam over the game"),
    ("fit", "Full", "The whole picture across the middle, on a blurred copy"),
    ("crop", "Follow", "A tall crop that follows the person"),
    ("cam", "Cam", "Only the facecam"),
    ("game", "Game", "Only the game"),
];
const CAPTIONS: [(&str, &str); 4] = [("pixel", "Pixel"), ("bold", "Bold"), ("clean", "Clean"), ("none", "None")];

#[derive(Default)]
struct State {
    vod: Cell<Option<i64>>,
    clip: Cell<Option<i64>>,
    /// The VOD page's kind filter: `None` for every kind.
    filter: RefCell<Option<String>>,
    /// A `clips.changed` is being waited on: a stage announces several a second.
    pending: Cell<bool>,
    sidebar: RefCell<Option<Rc<Sidebar>>>,
    editor: RefCell<Option<Rc<Editor>>>,
    /// Posters decoded, by path and size on disk: a refresh draws them again without decoding.
    posters: RefCell<HashMap<String, (u64, gtk::gdk::Texture)>>,
}

thread_local! {
    static STATE: State = State::default();
}

struct Sidebar {
    review: gtk::Button,
    review_count: gtk::Label,
    queue: gtk::Button,
    queue_count: gtk::Label,
    vods: gtk::Box,
    rows: RefCell<Vec<(i64, gtk::Button)>>,
    settings: gtk::Button,
    reading: Cell<bool>,
    again: Cell<bool>,
}

fn said(e: &Error) -> String {
    super::kit::said(e, "This Relay's engine has no Clips yet: restart Relay after updating it.")
}

fn the_ui() -> Option<Rc<Ui>> {
    super::threads::the_ui_pub()
}

fn kind_caption(kind: &str) -> &'static str {
    KINDS.iter().find(|(k, _)| *k == kind).map_or("Other", |(_, c)| c)
}

/// `0:34`, `1:02:03`.
fn clock(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// `1:02:03.4`, for an edit's edges.
fn clock_tenths(s: f64) -> String {
    let whole = s.max(0.0).floor();
    format!("{}.{}", clock(whole), ((s.max(0.0) - whole) * 10.0).floor() as u32)
}

/// "about 25 s left", "about 4 min left", "about 1 h 10 min left".
fn left(eta_s: f64) -> String {
    let s = eta_s.max(1.0);
    if s < 55.0 {
        format!("about {} s left", ((s / 5.0).ceil() * 5.0) as u64)
    } else if s < 3600.0 {
        format!("about {} min left", (s / 60.0).ceil() as u64)
    } else {
        let m = (s / 60.0).ceil() as u64;
        format!("about {} h {} min left", m / 60, m % 60)
    }
}

/// How long a VOD's work has left, when the engine said.
fn vod_left(v: &Value) -> Option<String> {
    v["eta_s"].as_f64().map(left)
}

/// Open the file manager on `path`: the file picked out in its folder, or the folder itself.
fn show_in_folder(ui: &Rc<Ui>, path: &str) {
    let file = gtk::gio::File::for_path(path);
    let launcher = gtk::FileLauncher::new(Some(&file));
    let weak = Rc::downgrade(ui);
    let done = move |r: Result<(), glib::Error>| {
        if let (Err(e), Some(ui)) = (r, weak.upgrade()) {
            if !e.matches(gtk::DialogError::Dismissed) {
                ui.show_error(&format!("Could not open the folder: {e}"));
            }
        }
    };
    if std::path::Path::new(path).is_dir() {
        launcher.launch(Some(&ui.window), None::<&gtk::gio::Cancellable>, done);
    } else {
        launcher.open_containing_folder(Some(&ui.window), None::<&gtk::gio::Cancellable>, done);
    }
}

fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A local time from RFC 3339: "Today 17:00", "Tomorrow 09:30", "Sat 2 Nov 17:00".
fn when(ts: &str) -> String {
    let Ok(at) = glib::DateTime::from_iso8601(ts, None) else { return ts.to_string() };
    let Ok(at) = at.to_local() else { return ts.to_string() };
    let Ok(now) = glib::DateTime::now_local() else { return ts.to_string() };
    let time = at.format("%H:%M").map(|g| g.to_string()).unwrap_or_default();
    let day = |d: &glib::DateTime| (d.year(), d.day_of_year());
    if day(&at) == day(&now) {
        format!("Today {time}")
    } else if now.add_days(1).ok().is_some_and(|t| day(&t) == day(&at)) {
        format!("Tomorrow {time}")
    } else if now.add_days(-1).ok().is_some_and(|t| day(&t) == day(&at)) {
        format!("Yesterday {time}")
    } else {
        at.format("%a %-d %b %H:%M").map(|g| g.to_string()).unwrap_or_default()
    }
}

/// A clip's state as a word, and the tone of its pill; `None` for one that only waits for review.
fn state_pill(c: &Value) -> Option<(String, &'static str)> {
    Some(match text(c, "state") {
        "draft" => ("Not drawn".into(), ""),
        "rendering" => ("Drawing".into(), "clips-pill-working"),
        "approved" => ("Approved".into(), "clips-pill-live"),
        "scheduled" => (format!("Out {}", c["publish_at"].as_str().map(when).unwrap_or_default()), "clips-pill-live"),
        "posting" => ("Uploading".into(), "clips-pill-working"),
        "posted" => ("Posted".into(), "clips-pill-posted"),
        "rejected" => ("Rejected".into(), "clips-pill-faint"),
        "failed" => ("Failed".into(), "clips-pill-failed"),
        _ => return None,
    })
}

fn pill(words: &str, tone: &str) -> gtk::Label {
    let l = label(words, "clips-pill");
    if !tone.is_empty() {
        l.add_css_class(tone);
    }
    l.set_valign(gtk::Align::Center);
    l.set_halign(gtk::Align::Start);
    l
}

/// A page's head: its title, a line under it, and keys on the right.
fn heading(head: &gtk::Box, title: &str, context: &str, actions: &[&gtk::Widget]) {
    clear(head);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_hexpand(true);
    let t = label(title, "money-title");
    t.set_wrap(true);
    t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    words.append(&t);
    if !context.is_empty() {
        let l = label(context, "money-context");
        l.set_wrap(true);
        words.append(&l);
    }
    row.append(&words);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    keys.set_valign(gtk::Align::Center);
    for action in actions {
        keys.append(*action);
    }
    row.append(&keys);
    head.append(&row);
}

/// A quiet way back above a page's title.
fn back_key(ui: &Rc<Ui>, head: &gtk::Box, caption: &str, run: impl Fn(&Rc<Ui>) + 'static) {
    let key = button("", "avex-back");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let chevron = crate::icons::image("chevron-left", 12);
    chevron.set_valign(gtk::Align::Center);
    row.append(&chevron);
    row.append(&label(caption, ""));
    key.set_child(Some(&row));
    key.set_halign(gtk::Align::Start);
    let said = format!("Back to {caption}");
    key.set_tooltip_text(Some(&said));
    key.update_property(&[gtk::accessible::Property::Label(&said)]);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            run(&ui);
        }
    });
    head.prepend(&key);
}

/// A section of a page: its caption over what it holds. No box: what is passive gets none.
fn section(body: &gtk::Box, caption: &str, aside: Option<&gtk::Widget>) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 10);
    wrap.add_css_class("clips-section");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let c = label(caption, "clips-section-title");
    c.set_hexpand(true);
    head.append(&c);
    if let Some(a) = aside {
        head.append(a);
    }
    wrap.append(&head);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
    wrap.append(&inner);
    body.append(&wrap);
    inner
}

/// Runs `op` from `key`: the key waits while it runs, a toast says what happened.
fn act(ui: &Rc<Ui>, key: &gtk::Button, op: &'static str, payload: Value, done: &'static str) {
    key.set_sensitive(false);
    let (ui, key) = (ui.clone(), key.clone());
    glib::spawn_future_local(async move {
        match ui.call(op, payload).await {
            Ok(_) => toast(&ui, done, None),
            Err(e) => ui.show_error(&said(&e)),
        }
        key.set_sensitive(true);
        ui.refresh_page();
    });
}

fn op_key(ui: &Rc<Ui>, caption: &str, class: &str, op: &'static str, payload: Value, done: &'static str, confirm: Option<&str>) -> gtk::Button {
    let key = button(caption, class);
    let weak = Rc::downgrade(ui);
    let run = move |key: &gtk::Button| {
        if let Some(ui) = weak.upgrade() {
            act(&ui, key, op, payload.clone(), done);
        }
    };
    match confirm {
        Some(words) => confirm_inline(&key, words, run),
        None => {
            key.connect_clicked(run);
        }
    }
    key
}

/// Open `url` in the browser.
fn open_url(ui: &Rc<Ui>, url: &str) {
    let launcher = gtk::UriLauncher::new(url);
    let shown = ui.clone();
    launcher.launch(Some(&ui.window), gtk::gio::Cancellable::NONE, move |result| {
        if let Err(error) = result {
            if !error.matches(gtk::DialogError::Dismissed) {
                shown.show_error(&format!("Could not open the browser: {}", error.message()));
            }
        }
    });
}

// ---- Pictures --------------------------------------------------------------------------------

/// A `width` × `height` frame with the picture of `path` filling it: the frame sets the size, the
/// picture's own (a 360 × 640 poster) never does.
fn framed(ui: &Rc<Ui>, path: Option<&str>, class: &str, width: i32, height: i32) -> gtk::Overlay {
    let frame = gtk::Overlay::new();
    frame.add_css_class(class);
    frame.set_overflow(gtk::Overflow::Hidden);
    let size = gtk::Box::new(gtk::Orientation::Vertical, 0);
    size.set_size_request(width, height);
    frame.set_child(Some(&size));
    frame.add_overlay(&picture(ui, path, ""));
    frame.set_halign(gtk::Align::Start);
    frame.set_valign(gtk::Align::Start);
    frame
}

/// A picture of `path` (a poster), decoded off the main loop and kept for the next draw.
fn picture(ui: &Rc<Ui>, path: Option<&str>, class: &str) -> gtk::Picture {
    let pic = gtk::Picture::new();
    if !class.is_empty() {
        pic.add_css_class(class);
    }
    pic.set_content_fit(gtk::ContentFit::Cover);
    pic.set_can_shrink(true);
    let Some(path) = path.filter(|p| !p.is_empty()).map(str::to_string) else { return pic };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if let Some(t) = STATE.with(|s| s.posters.borrow().get(&path).filter(|(n, _)| *n == size).map(|(_, t)| t.clone())) {
        pic.set_paintable(Some(&t));
        return pic;
    }
    let weak = pic.downgrade();
    let rt = ui.rt.clone();
    glib::spawn_future_local(async move {
        let p = path.clone();
        let Ok(Ok(texture)) = rt.spawn_blocking(move || gtk::gdk::Texture::from_filename(&p)).await else { return };
        STATE.with(|s| {
            let mut cache = s.posters.borrow_mut();
            if cache.len() > 400 {
                cache.clear();
            }
            cache.insert(path, (size, texture.clone()));
        });
        if let Some(pic) = weak.upgrade() {
            pic.set_paintable(Some(&texture));
        }
    });
    pic
}

// ---- Install, events, reads ------------------------------------------------------------------

/// A title and a first line on every page, before its first read.
pub fn install(_ui: &Rc<Ui>) {
    for (name, title) in ALL.into_iter().zip(["Clips", "VOD", "Clip", "Queue", "Settings"]) {
        if let Some(page) = page(name) {
            heading(&page.head, title, "", &[]);
            message(&page.body, "Reading Clips…");
        }
    }
}

pub(super) fn open_vod(ui: &Rc<Ui>, id: i64) {
    STATE.with(|s| {
        if s.vod.replace(Some(id)) != Some(id) {
            *s.filter.borrow_mut() = None;
        }
    });
    if *ui.page.borrow() == VOD {
        ui.refresh_page();
    } else {
        ui.navigate(VOD);
    }
}

pub(super) fn open_clip(ui: &Rc<Ui>, id: i64) {
    STATE.with(|s| s.clip.set(Some(id)));
    if *ui.page.borrow() == CLIP {
        ui.refresh_page();
    } else {
        ui.navigate(CLIP);
    }
}

/// `clips.changed`: a VOD's stage moved, a clip was drawn or posted, the settings changed. Several
/// in a moment are read once: the sidebar, and the page showing. The editor reads only its status
/// again, and is built again only when its own clip was drawn anew or moved to another state:
/// another VOD's progress (an event every half second while one is worked on) used to rebuild it,
/// and its player with it, so a clip never played through while anything else was being made.
pub(super) fn changed(ui: &Rc<Ui>) {
    if STATE.with(|s| s.pending.replace(true)) {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local_once(Duration::from_millis(400), move || {
        STATE.with(|s| s.pending.set(false));
        let Some(ui) = weak.upgrade() else { return };
        refresh_sidebar(&ui);
        let showing = ui.page.borrow().clone();
        if !showing.starts_with("clips-") || showing == SETTINGS {
            return;
        }
        if showing == CLIP {
            if let Some(ed) = STATE.with(|s| s.editor.borrow().clone()) {
                refresh_status(&ui, &ed);
                return;
            }
        }
        if typing(&ui) {
            return;
        }
        ui.refresh_page();
    });
}

fn typing(ui: &Ui) -> bool {
    gtk::prelude::GtkWindowExt::focus(&ui.window).is_some_and(|w| w.is::<gtk::Editable>() || w.is::<gtk::TextView>())
}

pub async fn refresh(ui: &Rc<Ui>, name: &str) {
    let Some(page) = page(name) else { return };
    match name {
        HOME => {
            let (summary, vods, review) = tokio::join!(
                ui.call("clips.summary", json!({})),
                ui.call("clips.vod.list", json!({})),
                ui.call("clips.clip.list", json!({"states": ["ready"]}))
            );
            if *ui.page.borrow() != name {
                return;
            }
            match (summary, vods) {
                (Ok(s), Ok(v)) => home(ui, &page, &s, &rows(&v, "vods"), &review.map(|r| rows(&r, "clips")).unwrap_or_default()),
                (Err(e), _) | (_, Err(e)) => failed(ui, &page, "Clips", &e),
            }
        }
        VOD => {
            let Some(id) = STATE.with(|s| s.vod.get()) else {
                ui.navigate(HOME);
                return;
            };
            let read = ui.call("clips.vod.get", json!({"id": id})).await;
            if *ui.page.borrow() != name || STATE.with(|s| s.vod.get()) != Some(id) {
                return;
            }
            match read {
                Ok(d) => vod_page(ui, &page, &d),
                Err(e) => failed(ui, &page, "VOD", &e),
            }
        }
        CLIP => {
            let Some(id) = STATE.with(|s| s.clip.get()) else {
                ui.navigate(HOME);
                return;
            };
            let (read, yt) = tokio::join!(ui.call("clips.clip.get", json!({"id": id, "words": true})), ui.call("clips.youtube.status", json!({})));
            if *ui.page.borrow() != name || STATE.with(|s| s.clip.get()) != Some(id) {
                return;
            }
            match read {
                Ok(d) => clip_page(ui, &page, &d, &yt.unwrap_or(Value::Null)),
                Err(e) => failed(ui, &page, "Clip", &e),
            }
        }
        QUEUE => {
            let (clips, summary) = tokio::join!(ui.call("clips.clip.list", json!({})), ui.call("clips.summary", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match clips {
                Ok(c) => queue_page(ui, &page, &rows(&c, "clips"), &summary.unwrap_or(Value::Null)),
                Err(e) => failed(ui, &page, "Queue", &e),
            }
        }
        SETTINGS => {
            let (settings, yt, tools) =
                tokio::join!(ui.call("clips.settings.get", json!({})), ui.call("clips.youtube.status", json!({})), ui.call("clips.tools", json!({})));
            if *ui.page.borrow() != name {
                return;
            }
            match settings {
                Ok(s) => settings_page(ui, &page, &s, &yt.unwrap_or(Value::Null), &tools.map(|t| rows(&t, "tools")).unwrap_or_default()),
                Err(e) => failed(ui, &page, "Settings", &e),
            }
        }
        _ => {}
    }
}

fn failed(ui: &Rc<Ui>, page: &Page, name: &str, error: &Error) {
    heading(&page.head, name, "", &[]);
    super::kit::failed(ui, &page.body, &said(error));
}

// ---- Cards and the stage strip ---------------------------------------------------------------

/// A clip as the grids show it: its first frame, tall, its title, and what it is.
fn clip_card(ui: &Rc<Ui>, c: &Value) -> gtk::Button {
    let key = button("", "clips-card");
    key.set_widget_name(&format!("clips-card-{}", c["id"].as_i64().unwrap_or(0)));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let frame = framed(ui, c["poster"].as_str(), "clips-poster", 152, 270);
    if let Some((words, tone)) = state_pill(c) {
        let p = pill(&words, tone);
        p.add_css_class("clips-pill-over");
        p.set_valign(gtk::Align::Start);
        p.set_halign(gtk::Align::Start);
        p.set_margin_top(8);
        p.set_margin_start(8);
        frame.add_overlay(&p);
    }
    let len = label(&clock(c["length_s"].as_f64().unwrap_or(0.0)), "clips-card-length");
    len.set_valign(gtk::Align::End);
    len.set_halign(gtk::Align::End);
    len.set_margin_bottom(8);
    len.set_margin_end(8);
    frame.add_overlay(&len);
    col.append(&frame);
    let title = label(text(c, "title"), "clips-card-title");
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    title.set_width_chars(18);
    title.set_xalign(0.0);
    col.append(&title);
    let meta = label(&format!("{} · {}/10", kind_caption(text(c, "kind")), c["score"].as_u64().unwrap_or(0)), "clips-card-meta");
    meta.set_xalign(0.0);
    col.append(&meta);
    key.set_child(Some(&col));
    key.set_tooltip_text(Some(text(c, "why")));
    let (weak, id) = (Rc::downgrade(ui), c["id"].as_i64().unwrap_or(0));
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_clip(&ui, id);
        }
    });
    key
}

/// Clips side by side, as many to a line as fit.
fn grid(ui: &Rc<Ui>, clips: &[Value]) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.add_css_class("clips-grid");
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_column_spacing(16);
    flow.set_row_spacing(22);
    flow.set_halign(gtk::Align::Start);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(8);
    flow.set_valign(gtk::Align::Start);
    for c in clips {
        flow.append(&clip_card(ui, c));
    }
    flow
}

/// Where a VOD's work stands: each stage, the one running lit with its bar and what it is doing.
fn stages(v: &Value) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 10);
    wrap.add_css_class("clips-stages");
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.add_css_class("clips-stage-line");
    let current = text(v, "stage");
    let state = text(v, "state");
    let at = STAGES.iter().position(|(s, _)| *s == current);
    for (i, (_, caption)) in STAGES.iter().enumerate() {
        let step = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        step.add_css_class("clips-stage");
        step.set_hexpand(true);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("clips-stage-dot");
        dot.set_valign(gtk::Align::Center);
        let tone = match at {
            Some(a) if i < a => "done",
            Some(a) if i == a && state == "failed" => "failed",
            Some(a) if i == a => "now",
            None if state == "ready" => "done",
            _ => "",
        };
        if !tone.is_empty() {
            step.add_css_class(tone);
        }
        step.append(&dot);
        step.append(&label(caption, "clips-stage-name"));
        line.append(&step);
    }
    wrap.append(&line);
    if matches!(state, "working" | "queued") {
        let bar = gtk::ProgressBar::new();
        bar.add_css_class("clips-bar");
        bar.set_fraction(v["progress"].as_f64().unwrap_or(0.0));
        wrap.append(&bar);
        let mut detail = match (state, v["detail"].as_str()) {
            ("queued", _) => "Waiting for the VOD before it".to_string(),
            (_, Some(d)) if !d.is_empty() => d.to_string(),
            _ => STAGES.get(at.unwrap_or(0)).map_or("Starting", |(_, c)| c).to_string(),
        };
        match (state, v["eta_s"].as_f64()) {
            ("queued", Some(eta)) => detail.push_str(&format!(" · takes {}", left(eta).trim_end_matches(" left"))),
            (_, Some(eta)) => detail.push_str(&format!(" · {} in all", left(eta))),
            _ => {}
        }
        let d = label(&detail, "money-row-detail");
        d.set_xalign(0.0);
        wrap.append(&d);
    }
    wrap
}

/// What a VOD is, in one line: its length, its clips, where its work stands.
fn vod_line(v: &Value) -> String {
    let mut parts = vec![clock(v["duration_s"].as_f64().unwrap_or(0.0))];
    let n = v["clips"].as_u64().unwrap_or(0);
    match text(v, "state") {
        "working" => {
            parts.push(format!("{}…", STAGES.iter().find(|(s, _)| *s == text(v, "stage")).map_or("Working", |(_, c)| c)));
            parts.extend(vod_left(v));
        }
        "queued" => parts.push("Waiting its turn".into()),
        "failed" => parts.push("Stopped on a problem".into()),
        "stopped" => parts.push("Stopped".into()),
        _ => {}
    }
    if n > 0 {
        parts.push(plural(n, "clip", "clips"));
    }
    let ready = v["clips_ready"].as_u64().unwrap_or(0);
    if ready > 0 {
        parts.push(format!("{ready} to review"));
    }
    let posted = v["clips_posted"].as_u64().unwrap_or(0);
    if posted > 0 {
        parts.push(format!("{posted} posted"));
    }
    parts.join(" · ")
}

/// A VOD as a row: its frame, its title, its line.
fn vod_row(ui: &Rc<Ui>, v: &Value) -> gtk::Button {
    let key = button("", "clips-vod-row");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.append(&framed(ui, v["poster"].as_str(), "clips-vod-thumb", 112, 63));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let t = label(text(v, "title"), "money-row-title");
    t.set_xalign(0.0);
    t.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&t);
    let d = label(&vod_line(v), "money-row-detail");
    d.set_xalign(0.0);
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    row.append(&words);
    if text(v, "state") == "working" {
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.add_css_class("threads-lamp");
        lamp.set_valign(gtk::Align::Center);
        row.append(&lamp);
    }
    key.set_child(Some(&row));
    let (weak, id) = (Rc::downgrade(ui), v["id"].as_i64().unwrap_or(0));
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_vod(&ui, id);
        }
    });
    key
}

fn import_key(ui: &Rc<Ui>) -> gtk::Button {
    let key = button("", "primary");
    key.add_css_class("money-hero-action");
    key.set_widget_name("clips-import");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&crate::icons::image_with_stroke("plus", 14, 2.0));
    row.append(&label("Import VOD", ""));
    key.set_child(Some(&row));
    key.set_tooltip_text(Some("A stream's VOD, from a file or a link · Ctrl N"));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import(&ui, None);
        }
    });
    key
}

// ---- Overview --------------------------------------------------------------------------------

fn home(ui: &Rc<Ui>, page: &Page, s: &Value, vods: &[Value], review: &[Value]) {
    let waiting = s["to_review"].as_u64().unwrap_or(0);
    let mut context = Vec::new();
    if waiting > 0 {
        context.push(format!("{} waiting for you", plural(waiting, "clip", "clips")));
    }
    if let Some(next) = s["next_post"].as_object() {
        if let Some(at) = next.get("publish_at").and_then(Value::as_str) {
            context.push(format!("next post {}", when(at).to_lowercase()));
        }
    }
    let posted = s["posted"].as_u64().unwrap_or(0);
    if posted > 0 {
        context.push(format!("{posted} posted"));
    }
    let import = import_key(ui);
    heading(&page.head, "Clips", &context.join(" · "), &[import.upcast_ref()]);
    clear(&page.body);

    let missing = rows(s, "missing");
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|t| text(t, "name")).collect();
        let note = notice(&format!("Clips needs {} on this PC before it can work.", names.join(", ")), Some(("Tools", Rc::new({
            let weak = Rc::downgrade(ui);
            move || {
                if let Some(ui) = weak.upgrade() {
                    ui.navigate(SETTINGS);
                }
            }
        }))));
        page.body.append(&note);
    }
    if vods.is_empty() {
        page.body.append(&drop_zone(ui));
        return;
    }
    for v in vods.iter().filter(|v| matches!(text(v, "state"), "working" | "queued")) {
        let block = section(&page.body, if text(v, "state") == "working" { "Working on" } else { "Next" }, None);
        block.append(&vod_row(ui, v));
        block.append(&stages(v));
    }
    for v in vods.iter().filter(|v| text(v, "state") == "failed") {
        let block = section(&page.body, "Stopped on a problem", None);
        block.append(&vod_row(ui, v));
        let why = label(text(v, "error"), "money-problem");
        why.set_wrap(true);
        why.set_xalign(0.0);
        block.append(&why);
        let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        keys.append(&op_key(ui, "Try again", "money-secondary", "clips.vod.retry", json!({"id": v["id"]}), "Started again where it stopped.", None));
        block.append(&keys);
    }
    if !review.is_empty() {
        let all = button(&format!("Review all {}", review.len()), "money-text-action");
        let weak = Rc::downgrade(ui);
        let first = review.first().and_then(|c| c["id"].as_i64());
        all.connect_clicked(move |_| {
            if let (Some(ui), Some(id)) = (weak.upgrade(), first) {
                open_clip(&ui, id);
            }
        });
        let block = section(&page.body, "Waiting for you", Some(all.upcast_ref()));
        block.append(&grid(ui, review));
    } else if vods.iter().all(|v| !matches!(text(v, "state"), "working" | "queued")) {
        let block = section(&page.body, "Waiting for you", None);
        let l = label("Nothing to review. Import a VOD, or look through one again for other moments.", "money-muted");
        l.set_wrap(true);
        l.set_xalign(0.0);
        block.append(&l);
    }
    let block = section(&page.body, "VODs", None);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for v in vods.iter().take(12) {
        list.append(&vod_row(ui, v));
    }
    block.append(&list);
}

/// A line across the page, with a key: something to do before the page works.
fn notice(words: &str, action: Option<(&str, super::Action)>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("aide-notice");
    let l = label(words, "aide-notice-text");
    l.set_wrap(true);
    l.set_hexpand(true);
    l.set_xalign(0.0);
    row.append(&l);
    if let Some((caption, run)) = action {
        let key = button(caption, "money-secondary");
        key.set_valign(gtk::Align::Center);
        key.connect_clicked(move |_| run());
        row.append(&key);
    }
    row
}

/// The empty Overview: where a VOD goes in, a file dropped on it or chosen.
fn drop_zone(ui: &Rc<Ui>) -> gtk::Box {
    let zone = gtk::Box::new(gtk::Orientation::Vertical, 14);
    zone.add_css_class("clips-drop");
    zone.set_halign(gtk::Align::Fill);
    let mark = crate::icons::image("film", 28);
    mark.add_css_class("clips-drop-mark");
    zone.append(&mark);
    let t = label("Drop a VOD here", "clips-drop-title");
    zone.append(&t);
    let d = label("A stream's recording, from a file or a link. Say what you want from it, and Clips finds the moments, cuts them into Shorts with captions, and lines them up for your channel.", "money-muted");
    d.set_wrap(true);
    d.set_justify(gtk::Justification::Center);
    d.set_max_width_chars(64);
    zone.append(&d);
    let key = button("Choose a VOD", "primary");
    key.add_css_class("money-hero-action");
    key.set_halign(gtk::Align::Center);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import(&ui, None);
        }
    });
    zone.append(&key);
    accept_drops(ui, &zone);
    zone
}

/// A file dropped on `widget` opens the import sheet with it.
fn accept_drops(ui: &Rc<Ui>, widget: &impl IsA<gtk::Widget>) {
    let target = gtk::DropTarget::new(gtk::gio::File::static_type(), gtk::gdk::DragAction::COPY);
    let weak = Rc::downgrade(ui);
    target.connect_drop(move |_, value, _, _| {
        let Some(ui) = weak.upgrade() else { return false };
        let Ok(file) = value.get::<gtk::gio::File>() else { return false };
        let Some(path) = file.path() else { return false };
        import(&ui, Some(path.display().to_string()));
        true
    });
    widget.add_controller(target);
}

// ---- Import ----------------------------------------------------------------------------------

/// The kinds to ask for, as toggles: All on its own, or any of the others.
fn kind_toggles(chosen: &[String]) -> (gtk::Box, Rc<Vec<(String, gtk::ToggleButton)>>, gtk::ToggleButton) {
    let flow = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    flow.add_css_class("clips-chips");
    let all = gtk::ToggleButton::with_label("All");
    all.add_css_class("clips-chip");
    all.set_active(chosen.is_empty());
    flow.append(&all);
    let mut keys = Vec::new();
    for (kind, caption) in KINDS.iter().take(ASKABLE) {
        let k = gtk::ToggleButton::with_label(caption);
        k.add_css_class("clips-chip");
        k.set_active(chosen.iter().any(|c| c == kind));
        flow.append(&k);
        keys.push((kind.to_string(), k));
    }
    let keys = Rc::new(keys);
    let others = keys.clone();
    all.connect_toggled(move |all| {
        if all.is_active() {
            for (_, k) in others.iter() {
                k.set_active(false);
            }
        }
    });
    for (_, k) in keys.iter() {
        let all = all.clone();
        let others = keys.clone();
        k.connect_toggled(move |k| {
            if k.is_active() {
                all.set_active(false);
            } else if others.iter().all(|(_, o)| !o.is_active()) {
                all.set_active(true);
            }
        });
    }
    (flow, keys, all)
}

fn chosen_kinds(keys: &[(String, gtk::ToggleButton)]) -> Vec<String> {
    keys.iter().filter(|(_, k)| k.is_active()).map(|(kind, _)| kind.clone()).collect()
}

fn field(body: &gtk::Box, caption: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let f = gtk::Box::new(gtk::Orientation::Vertical, 6);
    f.append(&label(caption, "money-label"));
    f.append(widget);
    body.append(&f);
    f
}

fn entry(name: &str, value: &str, placeholder: &str) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_widget_name(name);
    e.set_text(value);
    e.set_placeholder_text(Some(placeholder));
    e
}

/// The import sheet: the VOD (a file chosen or dropped, or a link), what to look for, and Start.
pub(super) fn import(ui: &Rc<Ui>, file: Option<String>) {
    let Some(panel) = Panel::toggle(ui, "Import a VOD", 520) else { return };
    panel.add_css_class("money-sheet");
    panel.add_css_class("clips-sheet");
    let body = panel.body.clone();
    let source = Rc::new(RefCell::new(file.unwrap_or_default()));

    let pick_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let chosen = label(if source.borrow().is_empty() { "No file chosen" } else { "" }, "clips-chosen");
    chosen.set_hexpand(true);
    chosen.set_xalign(0.0);
    chosen.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    if !source.borrow().is_empty() {
        chosen.set_text(&source.borrow());
    }
    let choose = button("Choose file…", "money-secondary");
    pick_row.append(&chosen);
    pick_row.append(&choose);
    let file_field = field(&body, "The VOD", &pick_row);
    accept_drops(ui, &file_field);
    let link = entry("clips-link", "", "Or paste a link: twitch.tv/videos/…, youtube.com/watch?v=…");
    body.append(&link);

    let (chips, kinds, _) = kind_toggles(&[]);
    let chips_wrap = gtk::FlowBox::new();
    chips_wrap.set_selection_mode(gtk::SelectionMode::None);
    chips_wrap.set_max_children_per_line(8);
    chips_wrap.set_column_spacing(6);
    chips_wrap.set_row_spacing(6);
    // The toggles move into a box that wraps on a narrow sheet.
    let mut child = chips.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        chips.remove(&c);
        chips_wrap.append(&c);
    }
    let ask = field(&body, "What do you want from it?", &chips_wrap);
    let words = entry("clips-request", "", "In your words: the IdleOn tips, anything with his dog…");
    ask.append(&words);
    let title = entry("clips-title", "", "Title (from the file or the link when empty)");
    field(&body, "Title", &title);
    let about = label("Clips transcribes it on your GPU, finds the moments with your Claude account, then cuts and draws each one. A few minutes for a long stream; you can leave it running.", "money-muted");
    about.set_wrap(true);
    about.set_xalign(0.0);
    body.append(&about);
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let cancel = button("Cancel", "quiet");
    let start = button("Start", "primary");
    start.set_widget_name("clips-start");
    start.set_hexpand(true);
    actions.append(&cancel);
    actions.append(&start);
    body.append(&actions);

    let sheet = Rc::downgrade(&panel);
    cancel.connect_clicked(move |_| {
        if let Some(s) = sheet.upgrade() {
            s.close();
        }
    });
    {
        let (weak, source, chosen) = (Rc::downgrade(ui), source.clone(), chosen.clone());
        choose.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Videos"));
            filter.add_mime_type("video/*");
            let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dialog = gtk::FileDialog::builder().title("Choose a VOD").filters(&filters).default_filter(&filter).build();
            let (source, chosen) = (source.clone(), chosen.clone());
            dialog.open(Some(&ui.window), gtk::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        *source.borrow_mut() = path.display().to_string();
                        chosen.set_text(&path.display().to_string());
                    }
                }
            });
        });
    }
    let (weak, sheet) = (Rc::downgrade(ui), Rc::downgrade(&panel));
    start.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        let link_text = link.text().trim().to_string();
        let src = if link_text.is_empty() { source.borrow().clone() } else { link_text };
        if src.is_empty() {
            problem.set_text("Choose a file, drop one here, or paste a link.");
            problem.set_visible(true);
            return;
        }
        let mut payload = json!({"source": src, "request": words.text().trim(), "kinds": chosen_kinds(&kinds)});
        if !title.text().trim().is_empty() {
            payload["title"] = json!(title.text().trim());
        }
        key.set_sensitive(false);
        problem.set_visible(false);
        let (key, problem, sheet) = (key.clone(), problem.clone(), sheet.clone());
        glib::spawn_future_local(async move {
            match ui.call("clips.vod.import", payload).await {
                Ok(v) => {
                    if let Some(s) = sheet.upgrade() {
                        s.close();
                    }
                    toast(&ui, "Started. It works through the stages on its own.", None);
                    open_vod(&ui, v["id"].as_i64().unwrap_or(0));
                }
                Err(e) => {
                    key.set_sensitive(true);
                    problem.set_text(&said(&e));
                    problem.set_visible(true);
                }
            }
        });
    });
    panel.present();
}

// ---- A VOD -----------------------------------------------------------------------------------

fn vod_page(ui: &Rc<Ui>, page: &Page, d: &Value) {
    let v = &d["vod"];
    let id = v["id"].as_i64().unwrap_or(0);
    let state = text(v, "state");
    let mut keys: Vec<gtk::Widget> = Vec::new();
    match state {
        "working" | "queued" => keys.push(op_key(ui, "Stop", "money-secondary", "clips.vod.stop", json!({"id": id}), "Stopped. Try again goes on from here.", None).upcast()),
        "failed" | "stopped" => keys.push(op_key(ui, "Try again", "money-secondary", "clips.vod.retry", json!({"id": id}), "Started again where it stopped.", None).upcast()),
        _ => {}
    }
    // Where its drawn clips are; before any is drawn, the file it came from.
    let drawn = rows(d, "clips").iter().find_map(|c| c["file"].as_str().map(|f| std::path::Path::new(f).parent().map(|p| p.display().to_string())))
        .flatten();
    let source = (!text(v, "source").starts_with("http")).then(|| text(v, "source").trim_start_matches("file://").to_string())
        .filter(|f| std::path::Path::new(f).exists());
    if let Some(path) = drawn.clone().or(source) {
        let show = button("Show in folder", "money-secondary");
        show.set_widget_name("clips-show-vod");
        show.set_tooltip_text(Some(if drawn.is_some() { "Open the folder its drawn clips are in" } else { "Open the folder the VOD is in" }));
        let weak = Rc::downgrade(ui);
        show.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                show_in_folder(&ui, &path);
            }
        });
        keys.push(show.upcast());
    }
    let delete = button("Delete", "money-destructive");
    {
        let weak = Rc::downgrade(ui);
        confirm_inline(&delete, "Delete VOD and clips?", move |key| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let ui2 = ui.clone();
            glib::spawn_future_local(async move {
                match ui2.call("clips.vod.delete", json!({"id": id})).await {
                    Ok(_) => {
                        toast(&ui2, "Deleted. The file it came from is untouched.", None);
                        ui2.navigate(HOME);
                    }
                    Err(e) => ui2.show_error(&said(&e)),
                }
            });
        });
    }
    keys.push(delete.upcast());
    let refs: Vec<&gtk::Widget> = keys.iter().collect();
    heading(&page.head, text(v, "title"), &vod_line(v), &refs);
    back_key(ui, &page.head, "Overview", |ui| ui.navigate(HOME));
    clear(&page.body);

    if matches!(state, "working" | "queued" | "failed") {
        page.body.append(&stages(v));
    }
    if state == "failed" {
        let why = label(text(v, "error"), "money-problem");
        why.set_wrap(true);
        why.set_xalign(0.0);
        page.body.append(&why);
    } else if state == "ready" && !text(v, "error").is_empty() {
        // Parts of the transcript the finder could not read: moments there may be missing.
        let note = label(text(v, "error"), "money-muted");
        note.set_wrap(true);
        note.set_xalign(0.0);
        page.body.append(&note);
    }

    let clips = rows(d, "clips");
    if clips.is_empty() {
        if state == "ready" {
            let l = label("No moments came out of this VOD for what was asked. Look again with other words, or other kinds.", "money-muted");
            l.set_wrap(true);
            l.set_xalign(0.0);
            page.body.append(&l);
        }
        look_again(ui, page, v);
        return;
    }
    // Kinds as a filter, with how many of each.
    let filter = STATE.with(|s| s.filter.borrow().clone());
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.add_css_class("clips-chips");
    let mut counts: Vec<(String, usize)> = Vec::new();
    for c in &clips {
        let k = text(c, "kind").to_string();
        match counts.iter_mut().find(|(n, _)| *n == k) {
            Some(e) => e.1 += 1,
            None => counts.push((k, 1)),
        }
    }
    let chip = |caption: String, kind: Option<String>| {
        let k = gtk::ToggleButton::with_label(&caption);
        k.add_css_class("clips-chip");
        k.set_active(filter == kind);
        let weak = Rc::downgrade(ui);
        k.connect_clicked(move |_| {
            STATE.with(|s| *s.filter.borrow_mut() = kind.clone());
            if let Some(ui) = weak.upgrade() {
                ui.refresh_page();
            }
        });
        bar.append(&k);
    };
    chip(format!("All {}", clips.len()), None);
    for (kind, n) in &counts {
        chip(format!("{} {n}", kind_caption(kind)), Some(kind.clone()));
    }
    page.body.append(&bar);
    let shown: Vec<Value> = clips.into_iter().filter(|c| filter.as_deref().is_none_or(|f| text(c, "kind") == f)).collect();
    page.body.append(&grid(ui, &shown));
    look_again(ui, page, v);
}

/// Look again: other moments of a transcribed VOD, in the person's words and kinds.
fn look_again(ui: &Rc<Ui>, page: &Page, v: &Value) {
    let id = v["id"].as_i64().unwrap_or(0);
    let state = text(v, "state");
    // Look again: other moments, in the person's words.
    if matches!(state, "ready" | "stopped" | "failed") && v["duration_s"].as_f64().unwrap_or(0.0) > 0.0 && std::path::Path::new(text(v, "dir")).join("words.json").exists() {
        let ask = section(&page.body, "Look again", None);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let words = entry("clips-find", "", "More of something: the IdleOn tips, the dog, the boss fight…");
        words.add_css_class("money-field");
        words.set_hexpand(true);
        row.append(&words);
        let find = button("Find", "money-secondary");
        row.append(&find);
        ask.append(&row);
        let (chips, kinds, _) = kind_toggles(&[]);
        ask.append(&chips);
        let weak = Rc::downgrade(ui);
        let go = move |key: &gtk::Button| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let payload = json!({"id": id, "request": words.text().trim(), "kinds": chosen_kinds(&kinds)});
            let (ui, key) = (ui.clone(), key.clone());
            glib::spawn_future_local(async move {
                match ui.call("clips.vod.find", payload).await {
                    Ok(_) => toast(&ui, "Looking. New clips join the ones here.", None),
                    Err(e) => ui.show_error(&said(&e)),
                }
                key.set_sensitive(true);
                ui.refresh_page();
            });
        };
        find.connect_clicked(go);
    }

}

// ---- The editor ------------------------------------------------------------------------------

/// The clip being edited: what was read, the edit as changed so far, and the parts of the page
/// that follow it.
struct Editor {
    id: i64,
    clip: RefCell<Value>,
    words: Vec<Value>,
    edit: RefCell<Value>,
    dirty: Cell<bool>,
    save: gtk::Button,
    revert: gtk::Button,
    span_line: gtk::Label,
    /// The words, one link each: `w:<index>`.
    transcript: gtk::Label,
    /// Where the transcript was last clicked, for the word's menu to point at.
    clicked: Cell<(f64, f64)>,
    status: gtk::Box,
    /// The keys that decide the clip (Approve, Reject, Take back…), at the page's head, where
    /// they are seen without scrolling past the player.
    actions: gtk::Box,
    title: gtk::Entry,
    description: gtk::TextView,
    tags: gtk::Entry,
    hook: gtk::Entry,
    kind: gtk::DropDown,
    /// A read of the drawing's progress is due: one at a time while it is drawn.
    ticking: Cell<bool>,
}

impl Editor {
    fn mark_dirty(&self) {
        self.dirty.set(true);
        self.span_line.set_text(&self.span_words());
        self.save.set_sensitive(true);
        self.save.add_css_class("primary");
        self.revert.set_visible(true);
    }

    fn keep(&self) -> Vec<[f64; 2]> {
        self.edit.borrow()["keep"].as_array().map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect()).unwrap_or_default()
    }

    fn set_keep(&self, mut keep: Vec<[f64; 2]>) {
        keep.retain(|[a, b]| b - a >= 0.3);
        if keep.is_empty() {
            return;
        }
        self.edit.borrow_mut()["keep"] = json!(keep.iter().map(|[a, b]| [(a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0]).collect::<Vec<_>>());
        self.mark_dirty();
        self.redraw_words();
    }

    fn bounds(&self) -> (f64, f64) {
        let keep = self.keep();
        (keep.iter().map(|k| k[0]).fold(f64::MAX, f64::min), keep.iter().map(|k| k[1]).fold(f64::MIN, f64::max))
    }

    /// Where the clip starts and ends in the VOD, and how many parts it plays.
    fn span_words(&self) -> String {
        let keep = self.keep();
        let (a, b) = self.bounds();
        let mut s = format!("{} to {} in the VOD", clock_tenths(a), clock_tenths(b));
        if keep.len() > 1 {
            s.push_str(&format!(" · {} parts", keep.len()));
        }
        if self.dirty.get() {
            s.push_str(" · changed, not drawn yet");
        }
        s
    }

    /// The transcript as text: every word a link, a kept word in ink, a cut one struck through,
    /// one outside the clip faint.
    fn redraw_words(&self) {
        self.span_line.set_text(&self.span_words());
        let keep = self.keep();
        let (a, b) = self.bounds();
        let mut markup = String::new();
        for (i, w) in self.words.iter().enumerate() {
            let (ws, we) = (w["start_ms"].as_f64().unwrap_or(0.0) / 1000.0, w["end_ms"].as_f64().unwrap_or(0.0) / 1000.0);
            let mid = (ws + we) / 2.0;
            let kept = keep.iter().any(|[x, y]| mid >= *x && mid <= *y);
            let word = glib::markup_escape_text(text(w, "text"));
            let shown = if kept {
                word.to_string()
            } else if mid > a && mid < b {
                format!("<s><span alpha=\"45%\">{word}</span></s>")
            } else {
                format!("<span alpha=\"38%\">{word}</span>")
            };
            if i > 0 {
                markup.push(' ');
            }
            markup.push_str(&format!("<a href=\"w:{i}\">{shown}</a>"));
        }
        self.transcript.set_markup(&markup);
    }
}

fn dropdown(captions: &[&str], selected: usize) -> gtk::DropDown {
    let d = gtk::DropDown::from_strings(captions);
    d.set_selected(selected as u32);
    d.add_css_class("money-picker");
    d
}

/// A row of keys, one of which is chosen.
fn segments(options: &[(&str, &str, &str)], chosen: &str, enabled: impl Fn(&str) -> bool, pick: impl Fn(&str) + 'static) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("clips-segments");
    let pick = Rc::new(pick);
    let mut first: Option<gtk::ToggleButton> = None;
    for (value, caption, about) in options {
        let k = gtk::ToggleButton::with_label(caption);
        if !about.is_empty() {
            k.set_tooltip_text(Some(about));
        }
        if let Some(f) = &first {
            k.set_group(Some(f));
        } else {
            first = Some(k.clone());
        }
        k.set_active(*value == chosen);
        k.set_sensitive(enabled(value));
        let (pick, value) = (pick.clone(), value.to_string());
        k.connect_toggled(move |k| {
            if k.is_active() {
                pick(&value);
            }
        });
        row.append(&k);
    }
    row
}

fn clip_page(ui: &Rc<Ui>, page: &Page, d: &Value, yt: &Value) {
    let c = &d["clip"];
    let v = &d["vod"];
    let id = c["id"].as_i64().unwrap_or(0);
    let vod_id = v["id"].as_i64().unwrap_or(0);
    // Keys at the head: back to the VOD, and what decides the clip.
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("clips-decide");
    heading(&page.head, text(c, "title"), &format!("{} · {}", kind_caption(text(c, "kind")), text(v, "title")), &[actions.upcast_ref()]);
    back_key(ui, &page.head, "VOD", move |ui| open_vod(ui, vod_id));
    clear(&page.body);

    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 28);
    columns.add_css_class("clips-editor");
    page.body.append(&columns);

    // The player, and what to do with the clip under it.
    let left = gtk::Box::new(gtk::Orientation::Vertical, 12);
    left.set_size_request(320, -1);
    left.set_hexpand(false);
    left.set_halign(gtk::Align::Start);
    // A fixed 9:16 frame: the video's own size (1080 × 1920) never sets it.
    let stage = gtk::Overlay::new();
    stage.add_css_class("clips-player");
    // Under the player, its first frame: what shows before it plays, not a black box.
    let under = framed(ui, c["poster"].as_str(), "clips-poster", 320, 568);
    stage.set_child(Some(&under));
    stage.set_halign(gtk::Align::Start);
    stage.set_valign(gtk::Align::Start);
    stage.set_overflow(gtk::Overflow::Hidden);
    match c["file"].as_str().filter(|_| text(c, "state") != "rendering") {
        Some(path) => {
            let file = gtk::MediaFile::for_filename(path);
            file.set_loop(true);
            if std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_some() {
                file.set_muted(true);
            }
            let player = gtk::Video::for_media_stream(Some(&file));
            // It plays when opened, as a Short does: approving one opens the next, playing.
            player.set_autoplay(std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_none());
            player.set_loop(true);
            player.set_vexpand(true);
            stage.set_tooltip_text(Some("Click to pause or play"));
            {
                // A click on the picture pauses or plays; the controls along its foot take their own.
                let click = gtk::GestureClick::new();
                click.set_button(gtk::gdk::BUTTON_PRIMARY);
                let file = file.clone();
                click.connect_released(move |g, n, _, _| {
                    if n == 1 {
                        file.set_playing(!file.is_playing());
                        g.set_state(gtk::EventSequenceState::Claimed);
                    }
                });
                stage.add_controller(click);
            }
            // GtkVideo paints black until its first frame (its offload's black background): the
            // poster under it shows until then.
            player.set_opacity(0.0);
            {
                let player = player.clone();
                file.connect_playing_notify(move |f| {
                    if f.is_playing() {
                        player.set_opacity(1.0);
                    }
                });
            }
            player.connect_unmap(|v| {
                if let Some(stream) = v.media_stream() {
                    stream.pause();
                }
            });
            stage.add_overlay(&player);
        }
        None => {
            let note = label(if text(c, "state") == "rendering" { "Drawing…" } else { "Not drawn yet" }, "clips-player-note");
            note.set_valign(gtk::Align::Center);
            note.set_halign(gtk::Align::Center);
            stage.add_overlay(&note);
        }
    }
    left.append(&stage);
    let status = gtk::Box::new(gtk::Orientation::Vertical, 10);
    left.append(&status);
    columns.append(&left);

    // The edit.
    let right = gtk::Box::new(gtk::Orientation::Vertical, 18);
    right.set_hexpand(true);
    columns.append(&right);

    let title = entry("clips-edit-title", text(c, "title"), "What happens, in under 60 characters");
    title.add_css_class("money-field");
    title.add_css_class("clips-title-field");
    title.set_max_length(100);
    field(&right, "Title", &title);
    let description = gtk::TextView::new();
    description.add_css_class("clips-text");
    description.set_wrap_mode(gtk::WrapMode::WordChar);
    description.buffer().set_text(text(c, "description"));
    description.set_size_request(-1, 64);
    field(&right, "Description", &description);
    let tags = entry("clips-edit-tags", &c["tags"].as_array().map(|t| t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default(), "tags, separated, by commas");
    tags.add_css_class("money-field");
    let kind_names: Vec<&str> = KINDS.iter().map(|(_, c)| *c).collect();
    let kind = dropdown(&kind_names, KINDS.iter().position(|(k, _)| *k == text(c, "kind")).unwrap_or(7));
    let meta_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    tags.set_hexpand(true);
    meta_row.append(&tags);
    meta_row.append(&kind);
    field(&right, "Tags and kind", &meta_row);
    if !text(c, "why").is_empty() {
        let why = label(&format!("{}/10 · {}", c["score"].as_u64().unwrap_or(0), text(c, "why")), "money-row-detail");
        why.set_wrap(true);
        why.set_xalign(0.0);
        right.append(&why);
    }

    let edit = c["edit"].clone();
    let layout_kind = v["layout"]["kind"].as_str().unwrap_or("camera").to_string();
    let hook = entry("clips-edit-hook", text(&edit, "hook"), "The opening line across the top");
    hook.add_css_class("money-field");

    // Spans: the edges, and the words to cut or keep.
    let span_line = label("", "clips-span-line");
    span_line.set_xalign(0.0);
    let transcript = label("", "clips-words");
    transcript.set_wrap(true);
    transcript.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    transcript.set_xalign(0.0);
    transcript.set_valign(gtk::Align::Start);

    let save = button("Save and draw again", "money-secondary");
    save.set_widget_name("clips-save");
    save.set_sensitive(false);
    let revert = button("Undo changes", "quiet");
    revert.set_visible(false);
    let ed = Rc::new(Editor {
        id,
        clip: RefCell::new(c.clone()),
        words: {
            let (a, b) = (c["start_s"].as_f64().unwrap_or(0.0) - 10.0, c["end_s"].as_f64().unwrap_or(0.0) + 10.0);
            rows(d, "words").into_iter().filter(|w| {
                let t = w["start_ms"].as_f64().unwrap_or(0.0) / 1000.0;
                t >= a && t <= b
            }).collect()
        },
        edit: RefCell::new(edit.clone()),
        dirty: Cell::new(false),
        save: save.clone(),
        revert: revert.clone(),
        span_line: span_line.clone(),
        transcript: transcript.clone(),
        clicked: Cell::new((0.0, 0.0)),
        status: status.clone(),
        actions: actions.clone(),
        title: title.clone(),
        description: description.clone(),
        tags: tags.clone(),
        hook: hook.clone(),
        kind: kind.clone(),
        ticking: Cell::new(false),
    });
    STATE.with(|s| *s.editor.borrow_mut() = Some(ed.clone()));
    for w in [title.upcast_ref::<gtk::Editable>(), tags.upcast_ref(), hook.upcast_ref()] {
        let ed = ed.clone();
        w.connect_changed(move |_| ed.mark_dirty());
    }
    {
        let ed2 = ed.clone();
        description.buffer().connect_changed(move |_| ed2.mark_dirty());
        let ed2 = ed.clone();
        kind.connect_selected_notify(move |_| ed2.mark_dirty());
    }

    let moment = section(&right, "The moment", None);
    let nudges = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    nudges.add_css_class("clips-nudges");
    let nudge = |caption: &str, tip: &str, start: bool, by: f64| {
        let k = button(caption, "clips-nudge");
        k.set_tooltip_text(Some(tip));
        let ed = ed.clone();
        k.connect_clicked(move |_| {
            let mut keep = ed.keep();
            let (a, b) = ed.bounds();
            if start {
                if let Some(span) = keep.iter_mut().find(|s| (s[0] - a).abs() < 1e-6) {
                    span[0] = (span[0] + by).max(0.0).min(span[1] - 0.3);
                }
            } else if let Some(span) = keep.iter_mut().find(|s| (s[1] - b).abs() < 1e-6) {
                span[1] = (span[1] + by).max(span[0] + 0.3);
            }
            ed.set_keep(keep);
        });
        k
    };
    nudges.append(&label("Start", "money-label"));
    nudges.append(&nudge("−1 s", "Start a second earlier", true, -1.0));
    nudges.append(&nudge("+1 s", "Start a second later", true, 1.0));
    let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gap.set_size_request(18, -1);
    nudges.append(&gap);
    nudges.append(&label("End", "money-label"));
    nudges.append(&nudge("−1 s", "End a second earlier", false, -1.0));
    nudges.append(&nudge("+1 s", "End a second later", false, 1.0));
    moment.append(&nudges);
    moment.append(&span_line);
    let hint = label("Click a word to cut it, keep it, or start or end there. Grey words are outside the clip.", "money-row-detail");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    moment.append(&hint);
    {
        // Where a word was clicked, for its menu; then the word's menu from its link.
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let ed2 = ed.clone();
        click.connect_pressed(move |_, _, x, y| ed2.clicked.set((x, y)));
        transcript.add_controller(click);
        let ed2 = ed.clone();
        transcript.connect_activate_link(move |l, uri| {
            if let Some(i) = uri.strip_prefix("w:").and_then(|i| i.parse::<usize>().ok()) {
                if let Some(w) = ed2.words.get(i).cloned() {
                    word_menu(l, &ed2, &w);
                }
            }
            glib::Propagation::Stop
        });
    }
    let well = gtk::Box::new(gtk::Orientation::Vertical, 0);
    well.add_css_class("clips-words-well");
    well.append(&transcript);
    moment.append(&well);
    let switches = gtk::Box::new(gtk::Orientation::Horizontal, 22);
    let tighten = gtk::CheckButton::with_label("Cut dead air");
    tighten.set_tooltip_text(Some("Pauses longer than 0.7 s are cut down"));
    tighten.set_active(edit["tighten_ms"].is_number());
    {
        let ed = ed.clone();
        tighten.connect_toggled(move |t| {
            ed.edit.borrow_mut()["tighten_ms"] = if t.is_active() { json!(700) } else { Value::Null };
            ed.mark_dirty();
        });
    }
    switches.append(&tighten);
    let end_card = gtk::CheckButton::with_label("End card");
    end_card.set_tooltip_text(Some("Like and subscribe, and where the streamer is live"));
    end_card.set_active(edit["end_card"] != false);
    {
        let ed = ed.clone();
        end_card.connect_toggled(move |t| {
            ed.edit.borrow_mut()["end_card"] = json!(t.is_active());
            ed.mark_dirty();
        });
    }
    switches.append(&end_card);
    moment.append(&switches);

    let look = section(&right, "The look", None);
    field(&look, "Opening line", &hook);
    let has_cam = layout_kind == "facecam";
    let layout_now = text(&edit, "layout").to_string();
    let layouts = {
        let ed = ed.clone();
        segments(&LAYOUTS, &layout_now, |l| match l {
            "split" | "cam" => has_cam,
            "game" => layout_kind != "camera",
            "crop" => layout_kind == "camera",
            _ => true,
        }, move |value| {
            let mut e = ed.edit.borrow_mut();
            if e["layout"] != json!(value) {
                e["layout"] = json!(value);
                // A crop chosen for one framing means nothing in another.
                e["view"] = Value::Null;
                e["reframe"] = json!({});
                drop(e);
                ed.mark_dirty();
            }
        })
    };
    field(&look, "Framing", &layouts);
    let caption_now = text(&edit, "captions").to_string();
    let captions = {
        let ed = ed.clone();
        segments(&CAPTIONS.map(|(v, c)| (v, c, "")), &caption_now, |_| true, move |value| {
            ed.edit.borrow_mut()["captions"] = json!(value);
            ed.mark_dirty();
        })
    };
    field(&look, "Captions", &captions);
    let fixes = edit["fixes"].as_object().map(|f| f.iter().map(|(k, v)| format!("{k} → {}", v.as_str().unwrap_or(""))).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    if !fixes.is_empty() {
        let f = label(&format!("Spelling fixed: {fixes}"), "money-row-detail");
        f.set_wrap(true);
        f.set_xalign(0.0);
        look.append(&f);
    }

    let foot = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    foot.add_css_class("clips-editor-foot");
    foot.append(&save);
    foot.append(&revert);
    right.append(&foot);
    {
        let weak = Rc::downgrade(ui);
        revert.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                STATE.with(|s| *s.editor.borrow_mut() = None);
                ui.refresh_page();
            }
        });
    }
    {
        let (weak, ed) = (Rc::downgrade(ui), ed.clone());
        save.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            let buffer = ed.description.buffer();
            let tags: Vec<String> = ed.tags.text().split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            ed.edit.borrow_mut()["hook"] = json!(ed.hook.text().trim());
            let payload = json!({
                "id": ed.id,
                "title": ed.title.text().trim(),
                "description": buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim(),
                "tags": tags,
                "kind": KINDS.get(ed.kind.selected() as usize).map_or("other", |(k, _)| *k),
                "edit": ed.edit.borrow().clone(),
            });
            key.set_sensitive(false);
            let (ui, ed) = (ui.clone(), ed.clone());
            glib::spawn_future_local(async move {
                match ui.call("clips.clip.update", payload).await {
                    Ok(c) => {
                        ed.dirty.set(false);
                        let drawn = c["state"] == "rendering";
                        toast(&ui, if drawn { "Saved. Drawing it again." } else { "Saved." }, None);
                        ui.refresh_page();
                    }
                    Err(e) => {
                        ed.save.set_sensitive(true);
                        ui.show_error(&said(&e));
                    }
                }
            });
        });
    }
    ed.redraw_words();
    ed.save.remove_css_class("primary");
    ed.dirty.set(false);
    ed.save.set_sensitive(false);
    ed.revert.set_visible(false);
    let mut with_drawing = c.clone();
    with_drawing["drawing"] = d["drawing"].clone();
    status_into(ui, &ed, &with_drawing, yt);
}

/// A word's menu: cut or keep it, start or end there, fix how it is spelled.
fn word_menu(at: &gtk::Label, ed: &Rc<Editor>, w: &Value) {
    let pop = gtk::Popover::new();
    pop.add_css_class("money-menu");
    pop.set_parent(at);
    let (x, y) = ed.clicked.get();
    pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let (ws, we) = (w["start_ms"].as_f64().unwrap_or(0.0) / 1000.0, w["end_ms"].as_f64().unwrap_or(0.0) / 1000.0);
    let keep = ed.keep();
    let mid = (ws + we) / 2.0;
    let kept = keep.iter().any(|[a, b]| mid >= *a && mid <= *b);
    let (a, b) = ed.bounds();
    let item = |caption: &str, run: Box<dyn Fn()>| {
        let k = button(caption, "money-menu-item");
        k.add_css_class("flat");
        let pop = pop.clone();
        k.connect_clicked(move |_| {
            run();
            pop.popdown();
        });
        col.append(&k);
    };
    if kept {
        let ed = ed.clone();
        item("Cut this word", Box::new(move || {
            let keep = ed.keep();
            let out: Vec<[f64; 2]> = keep.iter().flat_map(|&[x, y]| {
                if mid < x || mid > y {
                    vec![[x, y]]
                } else {
                    vec![[x, ws - 0.03], [we + 0.03, y]]
                }
            }).collect();
            ed.set_keep(out);
        }));
    } else if mid > a && mid < b {
        let ed = ed.clone();
        item("Keep this word", Box::new(move || {
            // The spans either side of it join again.
            let mut keep = ed.keep();
            let before = keep.iter().enumerate().filter(|(_, s)| s[1] <= mid).max_by(|x, y| x.1[1].total_cmp(&y.1[1])).map(|(i, _)| i);
            let after = keep.iter().enumerate().filter(|(_, s)| s[0] >= mid).min_by(|x, y| x.1[0].total_cmp(&y.1[0])).map(|(i, _)| i);
            if let (Some(i), Some(j)) = (before, after) {
                let joined = [keep[i][0], keep[j][1]];
                keep[i] = joined;
                keep.remove(j);
            }
            ed.set_keep(keep);
        }));
    }
    {
        let ed = ed.clone();
        item("Start here", Box::new(move || {
            let mut keep: Vec<[f64; 2]> = ed.keep().into_iter().filter(|s| s[1] > ws + 0.3).collect();
            let first = keep.iter().enumerate().min_by(|x, y| x.1[0].total_cmp(&y.1[0])).map(|(i, _)| i);
            match first {
                Some(i) => keep[i][0] = (ws - 0.12).max(0.0),
                None => keep.push([(ws - 0.12).max(0.0), we + 0.3]),
            }
            ed.set_keep(keep);
        }));
    }
    {
        let ed = ed.clone();
        item("End here", Box::new(move || {
            let mut keep: Vec<[f64; 2]> = ed.keep().into_iter().filter(|s| s[0] < we - 0.3).collect();
            let last = keep.iter().enumerate().max_by(|x, y| x.1[1].total_cmp(&y.1[1])).map(|(i, _)| i);
            match last {
                Some(i) => keep[i][1] = we + 0.22,
                None => keep.push([(ws - 0.12).max(0.0), we + 0.22]),
            }
            ed.set_keep(keep);
        }));
    }
    // Spelling: what whisper heard, as it should read in the captions.
    let heard = text(w, "text").trim().trim_end_matches(['.', ',', '!', '?']).to_string();
    let fix_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    fix_row.set_margin_top(6);
    let fix = entry("clips-fix", &heard, "How it is spelled");
    fix.set_width_chars(14);
    let ok = button("Fix", "money-secondary");
    fix_row.append(&fix);
    fix_row.append(&ok);
    col.append(&fix_row);
    {
        let (ed, pop) = (ed.clone(), pop.clone());
        let go = move || {
            let meant = fix.text().trim().to_string();
            if !meant.is_empty() && meant != heard {
                let mut e = ed.edit.borrow_mut();
                if !e["fixes"].is_object() {
                    e["fixes"] = json!({});
                }
                e["fixes"][heard.as_str()] = json!(meant);
                drop(e);
                ed.mark_dirty();
            }
            pop.popdown();
        };
        let go = Rc::new(go);
        let g = go.clone();
        ok.connect_clicked(move |_| g());
    }
    pop.set_child(Some(&col));
    pop.connect_closed(|p| p.unparent());
    pop.popup();
}

/// What a clip being drawn is doing, from `clips.clip.get`'s `drawing`.
fn drawing_line(d: &Value) -> String {
    let eta = d["eta_s"].as_f64().map(left);
    match (text(d, "phase"), eta) {
        ("queued", _) => "Waiting for the clips before it to be drawn. It plays here when done.".to_string(),
        ("drawing", Some(eta)) => format!("Drawing it: {:.0}%, {eta}.", d["progress"].as_f64().unwrap_or(0.0) * 100.0),
        ("drawing", None) => "Drawing it… it plays here when done.".to_string(),
        (_, Some(eta)) => format!("Cutting its footage, then drawing it: {eta}."),
        _ => "Drawing it… it plays here when done.".to_string(),
    }
}

/// The editor's status: where the clip stands, and the keys that move it on.
fn status_into(ui: &Rc<Ui>, ed: &Rc<Editor>, c: &Value, yt: &Value) {
    let status = &ed.status;
    clear(status);
    let id = ed.id;
    let state = text(c, "state");
    if let Some(file) = c["file"].as_str() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("clips-file-row");
        let f = label(std::path::Path::new(file).file_name().map_or(file.into(), |n| n.to_string_lossy()).as_ref(), "clips-path");
        f.set_xalign(0.0);
        f.set_hexpand(true);
        f.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        f.set_selectable(true);
        f.set_tooltip_text(Some(file));
        row.append(&f);
        let show = button("Show in folder", "money-secondary");
        show.set_widget_name("clips-show-file");
        show.set_tooltip_text(Some("Open the folder this clip's video is in, with it picked out"));
        let (weak, path) = (Rc::downgrade(ui), file.to_string());
        show.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                show_in_folder(&ui, &path);
            }
        });
        row.append(&show);
        status.append(&row);
    }
    let line = match state {
        "draft" => "Not drawn yet.".to_string(),
        "rendering" => drawing_line(&c["drawing"]),
        // A note on a clip back for review (an agent changed it after it was approved) comes first.
        "ready" if !text(c, "error").is_empty() => text(c, "error").to_string(),
        "ready" => format!("{} long. Approve it, and it goes out at the next posting time.", clock(c["length_s"].as_f64().unwrap_or(0.0))),
        "approved" => "Approved: it takes the next free posting time.".to_string(),
        "scheduled" => format!("Goes out {}.", c["publish_at"].as_str().map(when).unwrap_or_default().to_lowercase()),
        "posting" => "Uploading to YouTube…".to_string(),
        "posted" => format!("Posted {}.", c["posted_at"].as_str().map(when).unwrap_or_default().to_lowercase()),
        "rejected" => "Rejected. It stays here in case you change your mind.".to_string(),
        "failed" => format!("Failed: {}", text(c, "error")),
        _ => String::new(),
    };
    let l = label(&line, if state == "failed" { "money-problem" } else { "money-muted" });
    l.set_wrap(true);
    l.set_xalign(0.0);
    status.append(&l);
    if state == "rendering" {
        let bar = gtk::ProgressBar::new();
        bar.add_css_class("clips-bar");
        bar.set_fraction(c["drawing"]["progress"].as_f64().unwrap_or(0.0));
        status.append(&bar);
        // Read again in a moment: a render's progress is not a `clips.changed`.
        if !ed.ticking.replace(true) {
            let (weak, ed) = (Rc::downgrade(ui), ed.clone());
            glib::timeout_add_local_once(Duration::from_secs(2), move || {
                ed.ticking.set(false);
                let Some(ui) = weak.upgrade() else { return };
                let current = STATE.with(|s| s.editor.borrow().as_ref().is_some_and(|e| Rc::ptr_eq(e, &ed)));
                if current && *ui.page.borrow() == CLIP {
                    refresh_status(&ui, &ed);
                }
            });
        }
    }
    let keys = &ed.actions;
    clear(keys);
    match state {
        "ready" => {
            let vod = c["vod_id"].as_i64().unwrap_or(0);
            let reject = decide_key(ui, "Reject", "money-secondary", id, vod, "rejected", "Rejected.");
            reject.set_tooltip_text(Some("Reject, and open the next clip waiting for you"));
            keys.append(&reject);
            let approve = decide_key(ui, "Approve", "primary", id, vod, "approved", "Approved. It goes out at the next posting time.");
            approve.set_widget_name("clips-approve");
            approve.set_tooltip_text(Some("Approve, and open the next clip waiting for you"));
            keys.append(&approve);
        }
        "approved" | "scheduled" => {
            keys.append(&op_key(ui, "Take back", "money-secondary", "clips.clip.set", json!({"id": id, "state": "ready"}), "Taken off the schedule.", None));
        }
        "rejected" => {
            keys.append(&op_key(ui, "Restore", "money-secondary", "clips.clip.set", json!({"id": id, "state": "ready"}), "Back in review.", None));
        }
        "failed" | "draft" => {
            keys.append(&op_key(ui, "Draw again", "money-secondary", "clips.clip.render", json!({"id": id}), "Drawing it again.", None));
        }
        "posted" => {
            if let Some(url) = c["youtube_url"].as_str() {
                let open = button("Open on YouTube", "money-secondary");
                let (weak, url) = (Rc::downgrade(ui), url.to_string());
                open.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        open_url(&ui, &url);
                    }
                });
                keys.append(&open);
            }
        }
        _ => {}
    }
    // Posting now, past the schedule, when the channel is connected.
    if matches!(state, "ready" | "approved" | "scheduled" | "failed") && c["file"].is_string() {
        if yt["connected"] == true {
            let now = op_key(ui, "Post now", "money-secondary", "clips.clip.post", json!({"id": id}), "Uploading to YouTube.", Some("Post to YouTube now?"));
            now.set_halign(gtk::Align::Start);
            status.append(&now);
        } else {
            let l = label("Connect your YouTube channel in Settings to post from here.", "money-row-detail");
            l.set_wrap(true);
            l.set_xalign(0.0);
            status.append(&l);
        }
    }
}

/// Approve or reject clip `id`, then open the next clip waiting for review: this VOD's first,
/// then any; back to the VOD's page when none waits.
fn decide_key(ui: &Rc<Ui>, caption: &str, class: &str, id: i64, vod: i64, state: &'static str, done: &'static str) -> gtk::Button {
    let key = button(caption, class);
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("clips.clip.set", json!({"id": id, "state": state})).await {
                key.set_sensitive(true);
                ui.show_error(&said(&e));
                return;
            }
            toast(&ui, done, None);
            let waiting = ui.call("clips.clip.list", json!({"states": ["ready"]})).await.map(|v| rows(&v, "clips")).unwrap_or_default();
            let next = waiting.iter().filter(|c| c["id"].as_i64() != Some(id))
                .min_by_key(|c| (c["vod_id"].as_i64() != Some(vod), std::cmp::Reverse(c["score"].as_u64().unwrap_or(0))))
                .and_then(|c| c["id"].as_i64());
            match next {
                Some(next) => open_clip(&ui, next),
                None => open_vod(&ui, vod),
            }
        });
    });
    key
}

/// The editor's status read again without touching what is being edited.
fn refresh_status(ui: &Rc<Ui>, ed: &Rc<Editor>) {
    let (ui, ed) = (ui.clone(), ed.clone());
    glib::spawn_future_local(async move {
        let (read, yt) = tokio::join!(ui.call("clips.clip.get", json!({"id": ed.id})), ui.call("clips.youtube.status", json!({})));
        let Ok(d) = read else { return };
        let still = *ui.page.borrow() == CLIP && STATE.with(|s| s.clip.get()) == Some(ed.id);
        if !still {
            return;
        }
        // Drawn anew, or moved on (approved elsewhere, failed): the page again, player and all,
        // unless something is being typed into it.
        let was = ed.clip.borrow().clone();
        let moved = d["clip"]["state"] != was["state"] || d["clip"]["file"] != was["file"];
        if moved && !ed.dirty.get() && !typing(&ui) {
            ui.refresh_page();
            return;
        }
        *ed.clip.borrow_mut() = d["clip"].clone();
        let mut c = d["clip"].clone();
        c["drawing"] = d["drawing"].clone();
        status_into(&ui, &ed, &c, &yt.unwrap_or(Value::Null));
    });
}

// ---- Queue -----------------------------------------------------------------------------------

fn queue_row(ui: &Rc<Ui>, c: &Value, aside: &str, keys: Vec<gtk::Button>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("clips-queue-row");
    let open = button("", "clips-queue-open");
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    inner.append(&framed(ui, c["poster"].as_str(), "clips-queue-thumb", 45, 80));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let t = label(text(c, "title"), "money-row-title");
    t.set_xalign(0.0);
    t.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&t);
    let d = label(aside, "money-row-detail");
    d.set_xalign(0.0);
    d.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&d);
    inner.append(&words);
    open.set_child(Some(&inner));
    open.set_hexpand(true);
    let (weak, id) = (Rc::downgrade(ui), c["id"].as_i64().unwrap_or(0));
    open.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            open_clip(&ui, id);
        }
    });
    row.append(&open);
    for k in keys {
        k.set_valign(gtk::Align::Center);
        row.append(&k);
    }
    row
}

fn queue_page(ui: &Rc<Ui>, page: &Page, clips: &[Value], s: &Value) {
    let yt = &s["youtube"];
    let channel = if yt["connected"] == true { format!("Posting to {}", yt["channel"].as_str().unwrap_or("your channel")) } else { "No YouTube channel connected".to_string() };
    heading(&page.head, "Queue", &channel, &[]);
    clear(&page.body);
    if yt["connected"] != true {
        let weak = Rc::downgrade(ui);
        page.body.append(&notice("Approved clips wait here until your YouTube channel is connected.", Some(("Connect", Rc::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.navigate(SETTINGS);
            }
        })))));
    }
    let mut out: Vec<&Value> = clips.iter().filter(|c| matches!(text(c, "state"), "approved" | "scheduled" | "posting")).collect();
    out.sort_by(|a, b| a["publish_at"].as_str().unwrap_or("~").cmp(b["publish_at"].as_str().unwrap_or("~")));
    let going = section(&page.body, "Going out", None);
    if out.is_empty() {
        let l = label("Nothing scheduled. Approve a clip and it takes the next posting time.", "money-muted");
        l.set_xalign(0.0);
        l.set_wrap(true);
        going.append(&l);
    }
    for c in out {
        let id = c["id"].as_i64().unwrap_or(0);
        let when_words = match text(c, "state") {
            "posting" => "Uploading now".to_string(),
            "approved" => "At the next free posting time".to_string(),
            _ => c["publish_at"].as_str().map(when).unwrap_or_default(),
        };
        let mut keys = vec![op_key(ui, "Take back", "money-secondary", "clips.clip.set", json!({"id": id, "state": "ready"}), "Taken off the schedule.", None)];
        if yt["connected"] == true && text(c, "state") != "posting" {
            keys.insert(0, op_key(ui, "Post now", "money-secondary", "clips.clip.post", json!({"id": id}), "Uploading to YouTube.", Some("Post now?")));
        }
        going.append(&queue_row(ui, c, &format!("{when_words} · {}", clock(c["length_s"].as_f64().unwrap_or(0.0))), keys));
    }
    let failed_list: Vec<&Value> = clips.iter().filter(|c| text(c, "state") == "failed").collect();
    if !failed_list.is_empty() {
        let block = section(&page.body, "Failed", None);
        for c in failed_list {
            let id = c["id"].as_i64().unwrap_or(0);
            let upload = text(c, "error").starts_with("Upload");
            let retry = if upload {
                op_key(ui, "Try again", "money-secondary", "clips.clip.post", json!({"id": id}), "Uploading again.", None)
            } else {
                op_key(ui, "Draw again", "money-secondary", "clips.clip.render", json!({"id": id}), "Drawing it again.", None)
            };
            block.append(&queue_row(ui, c, text(c, "error"), vec![retry]));
        }
    }
    let mut posted: Vec<&Value> = clips.iter().filter(|c| text(c, "state") == "posted").collect();
    posted.sort_by(|a, b| b["posted_at"].as_str().unwrap_or("").cmp(a["posted_at"].as_str().unwrap_or("")));
    let block = section(&page.body, "Posted", None);
    if posted.is_empty() {
        let l = label("Nothing posted yet.", "money-muted");
        l.set_xalign(0.0);
        block.append(&l);
    }
    for c in posted.into_iter().take(40) {
        let mut keys = Vec::new();
        if let Some(url) = c["youtube_url"].as_str() {
            let open = button("Open", "money-secondary");
            let (weak, url) = (Rc::downgrade(ui), url.to_string());
            open.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    open_url(&ui, &url);
                }
            });
            keys.push(open);
        }
        block.append(&queue_row(ui, c, &format!("Posted {}", c["posted_at"].as_str().map(when).unwrap_or_default().to_lowercase()), keys));
    }
}

// ---- Settings --------------------------------------------------------------------------------

fn settings_page(ui: &Rc<Ui>, page: &Page, s: &Value, yt: &Value, tools: &[Value]) {
    heading(&page.head, "Settings", "The channel you clip, how clips look, and where they go", &[]);
    clear(&page.body);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 22);
    form.add_css_class("clips-settings");
    page.body.append(&form);

    let channel = section(&form, "The streamer", None);
    let streamer = entry("clips-streamer", text(s, "streamer"), "Their name, as titles say it");
    streamer.add_css_class("money-field");
    field(&channel, "Name", &streamer);
    let where_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let platforms = ["Twitch", "YouTube", "Kick"];
    let platform = dropdown(&platforms, platforms.iter().position(|p| p.eq_ignore_ascii_case(text(s, "platform"))).unwrap_or(0));
    let handle = entry("clips-handle", text(s, "handle"), "Their handle there");
    handle.add_css_class("money-field");
    handle.set_hexpand(true);
    where_row.append(&platform);
    where_row.append(&handle);
    field(&channel, "Where they stream", &where_row);
    let terms = entry("clips-terms", text(s, "terms"), "Names and game words to spell right: IdleOn, Spelunking…");
    terms.add_css_class("money-field");
    field(&channel, "Words the transcript should know", &terms);
    let notes = gtk::TextView::new();
    notes.add_css_class("clips-text");
    notes.set_wrap_mode(gtk::WrapMode::WordChar);
    notes.buffer().set_text(text(s, "notes"));
    notes.set_size_request(-1, 60);
    field(&channel, "What their viewers like", &notes);

    let look = section(&form, "How clips look", None);
    let caption_names: Vec<&str> = CAPTIONS.iter().map(|(_, c)| *c).collect();
    let captions = dropdown(&caption_names, CAPTIONS.iter().position(|(v, _)| *v == text(s, "captions")).unwrap_or(0));
    let highlight = entry("clips-highlight", text(s, "highlight"), "#FFE14D");
    highlight.add_css_class("money-field");
    highlight.set_width_chars(9);
    let look_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    look_row.append(&captions);
    look_row.append(&highlight);
    field(&look, "Captions, and the colour of key words", &look_row);
    let end_card = gtk::CheckButton::with_label("End on the card: like and subscribe, and where they are live");
    end_card.set_active(s["end_card"] != false);
    look.append(&end_card);

    let finding = section(&form, "Finding moments", None);
    let len_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let min_s = gtk::SpinButton::with_range(5.0, 120.0, 1.0);
    min_s.set_value(s["min_s"].as_f64().unwrap_or(15.0));
    min_s.add_css_class("money-field");
    let max_s = gtk::SpinButton::with_range(10.0, 180.0, 1.0);
    max_s.set_value(s["max_s"].as_f64().unwrap_or(58.0));
    max_s.add_css_class("money-field");
    let max_clips = gtk::SpinButton::with_range(1.0, 30.0, 1.0);
    max_clips.set_value(s["max_clips"].as_f64().unwrap_or(10.0));
    max_clips.add_css_class("money-field");
    len_row.append(&min_s);
    len_row.append(&label("to", "money-muted"));
    len_row.append(&max_s);
    len_row.append(&label("seconds, at most", "money-muted"));
    len_row.append(&max_clips);
    len_row.append(&label("clips a VOD", "money-muted"));
    field(&finding, "Length", &len_row);
    let model_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let models = ["opus", "sonnet"];
    let model = dropdown(&["Claude Opus (best)", "Claude Sonnet (lighter on your plan)"], models.iter().position(|m| *m == text(s, "model")).unwrap_or(0));
    let fps = dropdown(&["30 frames a second", "60 frames a second"], if s["fps"].as_u64() == Some(60) { 1 } else { 0 });
    model_row.append(&model);
    model_row.append(&fps);
    field(&finding, "The finder, and how clips are drawn", &model_row);

    let posting = section(&form, "Posting", None);
    let p = &s["posting"];
    let modes = ["ask", "auto"];
    let mode = dropdown(&["Ask me before each post", "Post good clips on their own"], modes.iter().position(|m| *m == text(p, "mode")).unwrap_or(0));
    let min_score = gtk::SpinButton::with_range(1.0, 10.0, 1.0);
    min_score.set_value(p["min_score"].as_f64().unwrap_or(8.0));
    min_score.add_css_class("money-field");
    let mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    mode_row.append(&mode);
    mode_row.append(&label("scoring at least", "money-muted"));
    mode_row.append(&min_score);
    field(&posting, "Approval", &mode_row);
    let privacies = ["public", "unlisted", "private"];
    let privacy = dropdown(&["Public", "Unlisted", "Private"], privacies.iter().position(|v| *v == text(p, "privacy")).unwrap_or(0));
    let slots = entry("clips-slots", &p["slots"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default(), "17:00, 21:00 (empty posts at once)");
    slots.add_css_class("money-field");
    slots.set_hexpand(true);
    let post_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    post_row.append(&privacy);
    post_row.append(&slots);
    field(&posting, "Who sees them, and the day's posting times", &post_row);
    let footer = entry("clips-footer", text(p, "footer"), "Added to every description");
    footer.add_css_class("money-field");
    field(&posting, "Description footer", &footer);

    let problem = label("", "money-problem");
    problem.set_visible(false);
    problem.set_wrap(true);
    form.append(&problem);
    let save = button("Save settings", "primary");
    save.set_widget_name("clips-settings-save");
    save.set_halign(gtk::Align::Start);
    form.append(&save);
    {
        let weak = Rc::downgrade(ui);
        save.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            let nb = notes.buffer();
            let slot_list: Vec<String> = slots.text().split([',', ' ']).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
            let body = json!({
                "streamer": streamer.text().trim(),
                "platform": platforms[platform.selected() as usize % platforms.len()],
                "handle": handle.text().trim(),
                "terms": terms.text().trim(),
                "notes": nb.text(&nb.start_iter(), &nb.end_iter(), false).trim(),
                "captions": CAPTIONS[captions.selected() as usize % CAPTIONS.len()].0,
                "highlight": highlight.text().trim(),
                "end_card": end_card.is_active(),
                "min_s": min_s.value() as u32,
                "max_s": max_s.value() as u32,
                "max_clips": max_clips.value() as u32,
                "model": models[model.selected() as usize % models.len()],
                "fps": if fps.selected() == 1 { 60 } else { 30 },
                "posting": {
                    "mode": modes[mode.selected() as usize % modes.len()],
                    "privacy": privacies[privacy.selected() as usize % privacies.len()],
                    "slots": slot_list,
                    "min_score": min_score.value() as u32,
                    "footer": footer.text().trim(),
                },
            });
            key.set_sensitive(false);
            let (key, problem) = (key.clone(), problem.clone());
            glib::spawn_future_local(async move {
                match ui.call("clips.settings.set", body).await {
                    Ok(_) => {
                        problem.set_visible(false);
                        toast(&ui, "Saved.", None);
                    }
                    Err(e) => {
                        problem.set_text(&said(&e));
                        problem.set_visible(true);
                    }
                }
                key.set_sensitive(true);
            });
        });
    }

    youtube_section(ui, &form, yt);

    let tools_box = section(&form, "On this PC", None);
    for t in tools {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("clips-tool");
        let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lamp.add_css_class(if t["found"] == true { "clips-lamp-ok" } else { "clips-lamp-missing" });
        lamp.set_valign(gtk::Align::Center);
        row.append(&lamp);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
        words.set_hexpand(true);
        words.append(&label(text(t, "name"), "money-row-title"));
        let d = label(text(t, "purpose"), "money-row-detail");
        d.set_xalign(0.0);
        d.set_wrap(true);
        words.append(&d);
        if t["found"] != true {
            if let Some(cmd) = t["install"].as_str() {
                let c = label(cmd, "clips-path");
                c.set_selectable(true);
                c.set_wrap(true);
                c.set_wrap_mode(gtk::pango::WrapMode::Char);
                c.set_xalign(0.0);
                words.append(&c);
            }
        }
        row.append(&words);
        tools_box.append(&row);
    }
}

/// The YouTube channel: how to connect it, or which one is connected.
fn youtube_section(ui: &Rc<Ui>, form: &gtk::Box, yt: &Value) {
    let block = section(form, "YouTube", None);
    if let Some(note) = yt["note"].as_str().filter(|n| !n.is_empty()) {
        let n = label(note, "money-problem");
        n.set_wrap(true);
        n.set_xalign(0.0);
        block.append(&n);
    }
    if yt["connected"] == true {
        let line = label(&format!("Connected to {}.", yt["channel"].as_str().unwrap_or("your channel")), "money-row-title");
        line.set_xalign(0.0);
        block.append(&line);
        let keys = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        keys.append(&op_key(ui, "Disconnect", "money-secondary", "clips.youtube.disconnect", json!({}), "Disconnected.", Some("Disconnect YouTube?")));
        block.append(&keys);
        let fine = label("Until Google audits your project, YouTube keeps uploads from it private: you set each one public in YouTube Studio.", "money-row-detail");
        fine.set_wrap(true);
        fine.set_xalign(0.0);
        block.append(&fine);
        return;
    }
    if yt["configured"] == true {
        let connect = button(if yt["pending"] == true { "Waiting for the browser…" } else { "Connect YouTube" }, "primary");
        connect.set_halign(gtk::Align::Start);
        connect.set_widget_name("clips-youtube-connect");
        let weak = Rc::downgrade(ui);
        connect.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let (ui, key) = (ui.clone(), key.clone());
            glib::spawn_future_local(async move {
                match ui.call("clips.youtube.connect", json!({})).await {
                    Ok(v) => {
                        if let Some(url) = v["url"].as_str() {
                            open_url(&ui, url);
                        }
                        toast(&ui, "Finish in the browser: Relay hears back from Google on its own.", None);
                    }
                    Err(e) => {
                        key.set_sensitive(true);
                        ui.show_error(&said(&e));
                    }
                }
            });
        });
        block.append(&connect);
    }
    // The OAuth client: the person's own.
    let how = label("Relay posts through a Google client of your own: in console.cloud.google.com make a project, turn on the YouTube Data API v3, add yourself as a test user on its consent screen, then create an OAuth client of type Desktop app and paste its ID and secret here.", "money-row-detail");
    how.set_wrap(true);
    how.set_xalign(0.0);
    block.append(&how);
    let id = entry("clips-client-id", "", "Client ID: ….apps.googleusercontent.com");
    id.add_css_class("money-field");
    let secret = gtk::PasswordEntry::new();
    secret.set_show_peek_icon(true);
    secret.set_widget_name("clips-client-secret");
    secret.add_css_class("money-field");
    secret.set_property("placeholder-text", "Client secret");
    block.append(&id);
    block.append(&secret);
    let save = button(if yt["configured"] == true { "Replace the client" } else { "Save the client" }, "money-secondary");
    save.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(ui);
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let payload = json!({"client_id": id.text().trim(), "client_secret": secret.text().trim()});
        let (ui, key) = (ui.clone(), key.clone());
        glib::spawn_future_local(async move {
            match ui.call("clips.youtube.setup", payload).await {
                Ok(_) => toast(&ui, "Saved in your keyring. Now connect your channel.", None),
                Err(e) => ui.show_error(&said(&e)),
            }
            key.set_sensitive(true);
            ui.refresh_page();
        });
    });
    block.append(&save);
}

// ---- The sidebar -----------------------------------------------------------------------------

fn nav_key(mark: &gtk::Image, caption: &str, name: &str, count: Option<&gtk::Label>, keycap: Option<&str>) -> gtk::Button {
    let key = button("", "nav");
    key.set_widget_name(name);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(mark);
    let words = label(caption, "nav-label");
    words.set_hexpand(true);
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&words);
    if let Some(c) = count {
        c.set_valign(gtk::Align::Center);
        row.append(c);
    }
    if let Some(cap) = keycap {
        let cap = label(cap, "keycap");
        cap.set_valign(gtk::Align::Center);
        row.append(&cap);
    }
    key.set_child(Some(&row));
    key
}

/// The Clips space's sidebar: Import, the Overview and the Queue with their counts, the VODs, and
/// Settings at its foot. Filled from the engine when it shows.
pub fn sidebar() -> gtk::Box {
    let nav = gtk::Box::new(gtk::Orientation::Vertical, 1);
    nav.add_css_class("navigation");
    nav.add_css_class("money-nav");
    nav.add_css_class("aide-nav");
    nav.set_vexpand(true);
    let import_nav = nav_key(&crate::icons::image("plus", 16), "Import VOD", "nav-clips-import", None, Some("Ctrl N"));
    import_nav.set_tooltip_text(Some("A stream's VOD, from a file or a link"));
    import_nav.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            import(&ui, None);
        }
    });
    nav.append(&import_nav);
    let review_count = label("", "clips-count");
    review_count.set_visible(false);
    let review = nav_key(&crate::icons::image("film", 16), "Overview", "nav-clips-home", Some(&review_count), None);
    review.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            ui.navigate(HOME);
        }
    });
    nav.append(&review);
    let queue_count = label("", "clips-count");
    queue_count.set_visible(false);
    let queue = nav_key(&crate::icons::image("send", 16), "Queue", "nav-clips-queue", Some(&queue_count), None);
    queue.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            ui.navigate(QUEUE);
        }
    });
    nav.append(&queue);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    head.add_css_class("aide-nav-heading");
    let caption = label("VODs", "section-label");
    caption.set_hexpand(true);
    head.append(&caption);
    nav.append(&head);
    let vods = gtk::Box::new(gtk::Orientation::Vertical, 1);
    vods.add_css_class("threads-list");
    let scroll = crate::app::scrolled(&vods);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    nav.append(&scroll);

    let foot = gtk::Box::new(gtk::Orientation::Vertical, 1);
    foot.add_css_class("aide-nav-foot");
    let settings = nav_key(&crate::icons::image("gear", 16), "Settings", "nav-clips-settings", None, None);
    settings.connect_clicked(|_| {
        if let Some(ui) = the_ui() {
            ui.navigate(SETTINGS);
        }
    });
    foot.append(&settings);
    nav.append(&foot);
    vods.connect_map(|_| {
        if let Some(ui) = the_ui() {
            refresh_sidebar(&ui);
        }
    });
    let built = Rc::new(Sidebar { review, review_count, queue, queue_count, vods, rows: RefCell::default(), settings, reading: Cell::new(false), again: Cell::new(false) });
    STATE.with(|s| *s.sidebar.borrow_mut() = Some(built));
    nav
}

fn refresh_sidebar(ui: &Rc<Ui>) {
    let Some(side) = STATE.with(|s| s.sidebar.borrow().clone()) else { return };
    if !side.vods.is_mapped() {
        return;
    }
    if side.reading.replace(true) {
        side.again.set(true);
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        loop {
            side.again.set(false);
            let (vods, summary) = tokio::join!(ui.call("clips.vod.list", json!({})), ui.call("clips.summary", json!({})));
            if let Ok(s) = &summary {
                let count = |l: &gtk::Label, n: u64| {
                    l.set_text(&n.to_string());
                    l.set_visible(n > 0);
                };
                count(&side.review_count, s["to_review"].as_u64().unwrap_or(0));
                count(&side.queue_count, s["approved"].as_u64().unwrap_or(0) + s["scheduled"].as_u64().unwrap_or(0));
            }
            clear(&side.vods);
            let mut built = Vec::new();
            match vods {
                Ok(v) => {
                    let list = rows(&v, "vods");
                    if list.is_empty() {
                        let note = label("No VODs yet", "threads-list-note");
                        side.vods.append(&note);
                    }
                    for vod in list {
                        let id = vod["id"].as_i64().unwrap_or(0);
                        let key = button("", "threads-row");
                        key.set_widget_name(&format!("nav-clips-vod-{id}"));
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                        let t = label(text(&vod, "title"), "threads-row-title");
                        t.set_hexpand(true);
                        t.set_xalign(0.0);
                        t.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        row.append(&t);
                        match text(&vod, "state") {
                            "working" => {
                                let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                                lamp.add_css_class("threads-lamp");
                                lamp.set_valign(gtk::Align::Center);
                                row.append(&lamp);
                            }
                            "failed" => {
                                let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                                lamp.add_css_class("clips-lamp-missing");
                                lamp.set_valign(gtk::Align::Center);
                                row.append(&lamp);
                            }
                            _ => {
                                let n = vod["clips_ready"].as_u64().unwrap_or(0);
                                if n > 0 {
                                    let c = label(&n.to_string(), "clips-count");
                                    c.set_valign(gtk::Align::Center);
                                    row.append(&c);
                                }
                            }
                        }
                        key.set_child(Some(&row));
                        key.set_tooltip_text(Some(&vod_line(&vod)));
                        key.connect_clicked(move |_| {
                            if let Some(ui) = the_ui() {
                                open_vod(&ui, id);
                            }
                        });
                        side.vods.append(&key);
                        built.push((id, key));
                    }
                }
                Err(e) => {
                    let note = label(&said(&e), "threads-list-note");
                    note.set_wrap(true);
                    side.vods.append(&note);
                }
            }
            *side.rows.borrow_mut() = built;
            if !side.again.get() {
                break;
            }
        }
        side.reading.set(false);
        mark_sidebar(&ui);
    });
}

/// Light the sidebar key of the page showing.
pub(super) fn mark_sidebar(ui: &Ui) {
    let Some(side) = STATE.with(|s| s.sidebar.borrow().clone()) else { return };
    let showing = ui.page.borrow().clone();
    let vod = STATE.with(|s| s.vod.get());
    for (key, on) in [(&side.review, showing == HOME), (&side.queue, showing == QUEUE), (&side.settings, showing == SETTINGS)] {
        if on {
            key.add_css_class("active");
        } else {
            key.remove_css_class("active");
        }
    }
    for (id, key) in side.rows.borrow().iter() {
        if (showing == VOD || showing == CLIP) && vod == Some(*id) {
            key.add_css_class("active");
        } else {
            key.remove_css_class("active");
        }
    }
}

/// The start screen's card: what Clips is doing now, and its lamp.
pub(super) fn start_reading(s: &Value) -> (String, crate::start::Lamp) {
    use crate::start::Lamp;
    if let Some(w) = s["working"].as_object() {
        let stage = w.get("stage").and_then(Value::as_str).unwrap_or("");
        let caption = STAGES.iter().find(|(k, _)| *k == stage).map_or("Working", |(_, c)| c);
        return (format!("{caption}: {}", w.get("title").and_then(Value::as_str).unwrap_or("a VOD")), Lamp::Live);
    }
    let review = s["to_review"].as_u64().unwrap_or(0);
    if review > 0 {
        return (format!("{} waiting for you", plural(review, "clip", "clips")), Lamp::Attention);
    }
    if let Some(at) = s["next_post"]["publish_at"].as_str() {
        return (format!("Next post {}", when(at).to_lowercase()), Lamp::Live);
    }
    if s["vods"].as_u64().unwrap_or(0) == 0 {
        return ("Import a VOD to start".into(), Lamp::Idle);
    }
    (format!("{} posted", s["posted"].as_u64().unwrap_or(0)), Lamp::Idle)
}

/// The start screen's rows: the newest VODs, with their line.
pub(super) fn start_rows(v: &Value) -> Vec<(String, String)> {
    rows(v, "vods").iter().take(2).map(|v| (text(v, "title").to_string(), vod_line(v))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_read_like_a_player() {
        assert_eq!(clock(34.4), "0:34");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(clock_tenths(61.37), "1:01.3");
    }

    #[test]
    fn time_left_reads_plainly() {
        assert_eq!(left(12.0), "about 15 s left");
        assert_eq!(left(240.0), "about 4 min left");
        assert_eq!(left(4210.0), "about 1 h 11 min left");
        assert_eq!(drawing_line(&json!({"phase": "drawing", "progress": 0.42, "eta_s": 20.0})), "Drawing it: 42%, about 20 s left.");
        assert!(drawing_line(&json!({"phase": "queued"})).starts_with("Waiting for the clips before it"));
    }

    #[test]
    fn states_say_what_waits() {
        assert!(state_pill(&json!({"state": "ready"})).is_none());
        assert_eq!(state_pill(&json!({"state": "failed"})).unwrap().0, "Failed");
        assert_eq!(state_pill(&json!({"state": "rendering"})).unwrap().1, "clips-pill-working");
    }

    #[test]
    fn a_vod_line_counts_its_clips() {
        let v = json!({"duration_s": 3723.0, "state": "ready", "clips": 8, "clips_ready": 3, "clips_posted": 1});
        assert_eq!(vod_line(&v), "1:02:03 · 8 clips · 3 to review · 1 posted");
    }
}
