//! One video, opened from the gallery (docs/VIDEO.md): its newest render playing, the props its
//! composition declares to change without code, its renders and versions, and Ask, which hands
//! the moment on screen to the video's terminal.
//!
//! Agents write a video; the person watches it, changes what its props allow, renders it and
//! asks for the rest. Applying props renders a preview, so what plays follows what was set. A
//! render that is older than the code says so, with the key to render it again.
//!
//! A video with several compositions has them as cuts (a narrated one, a music-only one, a
//! vertical one): the board shows one at a time, its renders playing, its props, and Render and
//! Ask about it, without changing the composition the agent draws by default. Two renders can be
//! compared at the same moment (A/B), and a version kept by naming it.

use super::ask::{self, Route};
use crate::app::{label, rows, text, Ui};
use crate::relative::{ago, Form};
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use relay_bus::ops::video::{frame_label, Composition, GetOut, RenderState, Video, VideoRender};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

/// A field of the props panel, by what its schema says it holds.
enum Field {
    Text(gtk::Entry),
    Number(gtk::SpinButton, bool),
    Toggle(gtk::Switch),
    Choice(gtk::DropDown, Vec<Value>),
    Colour(gtk::ColorDialogButton),
    /// Anything else, as JSON.
    Raw(gtk::Entry),
}

impl Field {
    fn value(&self) -> Option<Value> {
        Some(match self {
            Field::Text(e) => Value::String(e.text().to_string()),
            Field::Number(s, integer) => if *integer { json!(s.value().round() as i64) } else { json!(s.value()) },
            Field::Toggle(s) => Value::Bool(s.is_active()),
            Field::Choice(d, values) => values.get(d.selected() as usize)?.clone(),
            Field::Colour(c) => {
                let rgba = c.rgba();
                let byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
                let hex = format!("#{:02X}{:02X}{:02X}", byte(rgba.red()), byte(rgba.green()), byte(rgba.blue()));
                Value::String(if rgba.alpha() < 0.999 { format!("{hex}{:02X}", byte(rgba.alpha())) } else { hex })
            }
            Field::Raw(e) => serde_json::from_str(&e.text()).ok()?,
        })
    }
}

/// Whether a prop holds a colour: zod-types' `zColor()` marks its schema so (unless the video
/// described it otherwise), and its value is one.
fn is_colour(schema: &Value, value: &Value) -> bool {
    let described = schema["description"].as_str().unwrap_or_default();
    described == "__remotion-color"
        || value.as_str().is_some_and(|v| v.starts_with('#') && matches!(v.len(), 4 | 7 | 9) && v[1..].chars().all(|c| c.is_ascii_hexdigit()))
}

/// A prop's label: its description, else its name.
fn caption(name: &str, schema: &Value) -> String {
    match schema["description"].as_str() {
        Some(d) if !d.is_empty() && !d.starts_with("__") => d.to_string(),
        _ => name.to_string(),
    }
}

/// Two renders loaded side by side, the player showing one: the same moment of either at a key.
struct Compare {
    renders: [VideoRender; 2],
    files: [gtk::MediaFile; 2],
    /// Which the player shows: 0 for A, 1 for B.
    showing: usize,
}

/// The moment an Ask is about: the render on screen, and the frame of its composition it shows.
#[derive(Clone)]
struct Moment {
    render: VideoRender,
    frame: u32,
    fps: f64,
}

/// A render as the compare keys name it: its name, else its cut; and its version.
fn render_tag(r: &VideoRender) -> String {
    format!("{} · v{}", r.name.as_deref().unwrap_or(&r.composition), r.version)
}

/// Seconds into its composition where render `r`'s file starts: a render of part of it starts
/// at its first frame.
fn start_s(r: &VideoRender, fps: f64) -> f64 {
    r.range.map_or(0.0, |[first, _]| first as f64 / fps.max(1.0))
}

/// The composition's frame at `seconds` into render `r`'s file.
fn frame_at(r: &VideoRender, seconds: f64, fps: f64) -> u32 {
    r.range.map_or(0, |[first, _]| first) + (seconds.max(0.0) * fps).round() as u32
}

/// Where in render `to`'s file (µs) the moment `seconds` into render `from`'s falls: the same
/// moment of their compositions, whatever part of it each covers and at whatever rate.
fn same_moment(from: &VideoRender, from_fps: f64, seconds: f64, to: &VideoRender, to_fps: f64) -> i64 {
    ((start_s(from, from_fps) + seconds - start_s(to, to_fps)).max(0.0) * 1e6).round() as i64
}

/// A render's file to play, looping. Silent in a screenshot run: a virtual compositor keeps the
/// window off the desktop, not its sound off the speakers.
fn media_file(r: &VideoRender) -> gtk::MediaFile {
    let file = gtk::MediaFile::for_filename(r.path.as_deref().unwrap_or_default());
    file.set_loop(true);
    if std::env::var_os("RELAY_NATIVE_SCREENSHOT").is_some() {
        file.set_muted(true);
    }
    file
}

/// Seek `file` to `at` (µs) once it can be, then play it if asked and `player` still shows it: a
/// file just loaded is not prepared yet, and a seek before then is lost.
fn seek_when_ready(file: &gtk::MediaFile, at: i64, play: bool, player: &gtk::Video) {
    let player = player.downgrade();
    let go = move |f: &gtk::MediaFile| {
        let end = f.duration();
        if f.is_seekable() {
            f.seek(if end > 0 { at.min(end - 1) } else { at });
        }
        let shown = player.upgrade().and_then(|p| p.media_stream()).is_some_and(|s| s == *f.upcast_ref::<gtk::MediaStream>());
        if play && shown {
            f.play();
        }
    };
    if file.is_prepared() {
        return go(file);
    }
    let held: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
    let kept = held.clone();
    *held.borrow_mut() = Some(file.connect_prepared_notify(move |f| {
        if f.is_prepared() {
            go(f);
            if let Some(id) = kept.borrow_mut().take() {
                f.disconnect(id);
            }
        }
    }));
}

pub struct VideoBoard {
    pub root: gtk::Box,
    ui: Weak<Ui>,
    pub id: i64,
    detail: RefCell<Option<GetOut>>,
    title: gtk::Label,
    terminal: gtk::Label,
    lamp: gtk::Box,
    status: gtk::Label,
    stage: gtk::Stack,
    player: gtk::Video,
    /// Play, pause, the time and a scrubber, always under the picture: GtkVideo's own hide
    /// until the pointer moves over it, so a playing video showed none.
    controls: gtk::MediaControls,
    poster: gtk::Picture,
    stage_note: gtk::Label,
    play_key: gtk::Button,
    preview_key: gtk::Button,
    stale: gtk::Box,
    stale_note: gtk::Label,
    progress: gtk::Box,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    /// The render in the player.
    playing: RefCell<Option<VideoRender>>,
    /// The player shows its cut's newest render, and takes the next one as it lands; a render
    /// the person picked from the list stays until they pick another.
    following: Cell<bool>,
    compare: RefCell<Option<Compare>>,
    compare_bar: gtk::Box,
    compare_keys: [gtk::ToggleButton; 2],
    /// The cut the board shows: the composition its stage, props, Render and Ask are about.
    cut: RefCell<String>,
    comp_title: gtk::Label,
    cuts: gtk::ListBox,
    compositions: RefCell<Vec<String>>,
    choosing: Cell<bool>,
    about: gtk::Label,
    props_box: gtk::Box,
    fields: RefCell<Vec<(String, Field)>>,
    /// What the props fields were built from: the composition, its schema and defaults, and
    /// the props stored. While they stay, a reread (a render finishing, an agent's change
    /// elsewhere) leaves the fields, and what is typed in them, alone.
    props_sig: RefCell<Option<String>>,
    renders_box: gtk::Box,
    versions_box: gtk::Box,
    voices_box: gtk::Box,
    toast: gtk::Revealer,
    toast_label: gtk::Label,
    toast_serial: Cell<u64>,
    ask_draft: RefCell<String>,
    pub back: RefCell<Option<Rc<dyn Fn()>>>,
    me: Weak<VideoBoard>,
}

fn section(title: &str) -> (gtk::Box, gtk::Box) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    section.add_css_class("design-section");
    section.append(&label(title, "design-section-title"));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    section.append(&body);
    (section, body)
}

fn clear(b: &gtk::Box) {
    while let Some(child) = b.first_child() {
        b.remove(&child);
    }
}

fn megabytes(bytes: i64) -> String {
    if bytes >= 1_000_000 { format!("{:.1} MB", bytes as f64 / 1e6) } else { format!("{} kB", (bytes as f64 / 1e3).round()) }
}

/// Whether a render's file plays here.
/// A render's sound in a line: `-14.0 LUFS · peak -1.1 dBTP`, and what is wrong with it.
fn sound_line(audio: &relay_bus::ops::video::AudioReport) -> String {
    let lufs = audio.lufs.map_or_else(|| "silent".to_string(), |l| format!("{l:.1} LUFS"));
    let mut line = format!("{lufs} · peak {:.1} dBTP", audio.true_peak);
    if let Some(w) = audio.warnings.iter().find(|w| w.kind == "clipping" || w.kind == "hot") {
        line.push_str(&format!(" · {}", if w.kind == "clipping" { "clips" } else { "hot" }));
    }
    line
}

/// 99000 as 99,000.
fn group(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// The render the progress bar shows and Stop cancels: the one rendering, else the oldest still
/// queued, the next to run. `renders` comes newest first.
fn current_render(renders: &[VideoRender]) -> Option<&VideoRender> {
    renders.iter().find(|r| r.state == RenderState::Rendering).or_else(|| renders.iter().rev().find(|r| !r.state.is_final()))
}

fn playable(r: &VideoRender) -> bool {
    r.state == RenderState::Done && r.path.is_some() && matches!(r.format.name(), "mp4" | "webm")
}

impl VideoBoard {
    pub fn new(ui: &Rc<Ui>, id: i64) -> Rc<VideoBoard> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("design-board");
        root.add_css_class("video-board");
        root.set_hexpand(true);
        root.set_vexpand(true);

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.add_css_class("design-topbar");
        let back_key = gtk::Button::new();
        back_key.add_css_class("quiet");
        back_key.add_css_class("design-crumb");
        let back_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        back_row.append(&crate::icons::image("chevron-left", 14));
        back_row.append(&label("Designs", ""));
        back_key.set_child(Some(&back_row));
        back_key.set_tooltip_text(Some("All designs and videos of this project"));
        back_key.set_widget_name("video-back");
        top.append(&back_key);
        top.append(&label("/", "faint"));
        let title = label("", "design-title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(48);
        let title_key = gtk::MenuButton::new();
        title_key.add_css_class("quiet");
        title_key.add_css_class("design-title-key");
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        title_row.append(&title);
        title_row.append(&crate::icons::image("chevron-down", 12));
        title_key.set_child(Some(&title_row));
        title_key.set_tooltip_text(Some("Rename, open the folder or delete this video"));
        top.append(&title_key);
        let lamp = ask::lamp("closed");
        lamp.set_visible(false);
        let terminal = label("", "video-terminal");
        let terminal_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        terminal_row.append(&lamp);
        terminal_row.append(&terminal);
        top.append(&terminal_row);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top.append(&spacer);
        let status = label("", "design-status");
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&status);
        // The live preview: Remotion Studio in the browser, with sound, no render needed.
        let live_key = gtk::Button::new();
        live_key.add_css_class("quiet");
        let live_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        live_row.append(&crate::icons::image("external", 14));
        live_row.append(&label("Watch live", ""));
        live_key.set_child(Some(&live_row));
        live_key.set_tooltip_text(Some("Play it with sound in your browser, scrub and step through frames, without rendering"));
        live_key.set_widget_name("video-live");
        top.append(&live_key);
        let render_key = gtk::MenuButton::new();
        render_key.add_css_class("video-render-key");
        let render_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        render_row.append(&crate::icons::image("download", 14));
        render_row.append(&label("Render", ""));
        render_row.append(&crate::icons::image("chevron-down", 12));
        render_key.set_child(Some(&render_row));
        render_key.set_tooltip_text(Some("Render the video to a file in its out/ folder"));
        top.append(&render_key);
        let ask_key = gtk::Button::new();
        ask_key.add_css_class("design-ask");
        let ask_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        ask_row.append(&super::glyphs::image("spark", 14));
        ask_row.append(&label("Ask agent", ""));
        ask_key.set_child(Some(&ask_row));
        ask_key.set_tooltip_text(Some("Ask the video's terminal to change it, about the moment on screen"));
        ask_key.set_widget_name("video-ask");
        top.append(&ask_key);
        let mut key = top.first_child();
        while let Some(widget) = key {
            key = widget.next_sibling();
            widget.set_valign(gtk::Align::Center);
        }
        root.append(&top);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        let stage_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        stage_box.add_css_class("video-stage");
        let stage = gtk::Stack::new();
        stage.set_vexpand(true);
        stage.set_hexpand(true);
        let player = gtk::Video::new();
        player.set_autoplay(false);
        player.set_loop(true);
        player.set_vexpand(true);
        // GtkVideo pauses itself on unmap only with autoplay on: left for another page, a video
        // playing would go on looping, sound and all.
        player.connect_unmap(|v| {
            if let Some(stream) = v.media_stream() {
                stream.pause();
            }
        });
        let player_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        player_box.append(&player);
        // Comparing two renders: A and B, the player showing one, either at the same moment.
        let compare_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        compare_bar.add_css_class("video-compare");
        compare_bar.set_halign(gtk::Align::Center);
        let compare_keys = [gtk::ToggleButton::new(), gtk::ToggleButton::new()];
        compare_keys[1].set_group(Some(&compare_keys[0]));
        for (key, side) in compare_keys.iter().zip(["A", "B"]) {
            key.add_css_class("video-compare-key");
            key.set_widget_name(&format!("video-compare-{}", side.to_lowercase()));
            compare_bar.append(key);
        }
        let compare_close = crate::app::icon_button("close", "Stop comparing: keep the one on screen");
        compare_bar.append(&compare_close);
        compare_bar.set_visible(false);
        player_box.append(&compare_bar);
        let controls = gtk::MediaControls::new(None::<&gtk::MediaStream>);
        controls.add_css_class("video-controls");
        player_box.append(&controls);
        stage.add_named(&player_box, Some("player"));
        // The poster takes the height the note and the keys under it leave, so a tall video's
        // never pushes them off the stage.
        let poster_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
        poster_box.set_valign(gtk::Align::Fill);
        poster_box.set_halign(gtk::Align::Fill);
        poster_box.set_margin_top(20);
        poster_box.set_margin_bottom(24);
        let poster = gtk::Picture::new();
        poster.set_content_fit(gtk::ContentFit::Contain);
        poster.set_can_shrink(true);
        poster.set_vexpand(true);
        poster.set_size_request(-1, 160);
        poster_box.append(&poster);
        let stage_note = label("", "dim");
        stage_note.set_wrap(true);
        stage_note.set_xalign(0.5);
        stage_note.set_justify(gtk::Justification::Center);
        poster_box.append(&stage_note);
        // The player loads a render when asked to play it, not when the board opens: a media
        // pipeline costs a decoder and its thread, for a video the person may only look at.
        let play_key = gtk::Button::new();
        play_key.add_css_class("primary");
        play_key.set_halign(gtk::Align::Center);
        let play_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        play_row.append(&crate::icons::image("play", 14));
        play_row.append(&label("Play", ""));
        play_key.set_child(Some(&play_row));
        play_key.set_widget_name("video-play");
        poster_box.append(&play_key);
        let preview_key = gtk::Button::with_label("Render a preview");
        preview_key.add_css_class("primary");
        preview_key.set_halign(gtk::Align::Center);
        poster_box.append(&preview_key);
        stage.add_named(&poster_box, Some("poster"));
        // The poster until the person plays a render.
        stage.set_visible_child_name("poster");
        stage_box.append(&stage);
        overlay.set_child(Some(&stage_box));

        // What floats over the stage: a render under way, a render older than the code.
        let float = gtk::Box::new(gtk::Orientation::Vertical, 8);
        float.set_halign(gtk::Align::Center);
        float.set_valign(gtk::Align::Start);
        float.set_margin_top(14);
        let progress = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        progress.add_css_class("video-pill");
        let progress_label = label("", "");
        progress.append(&progress_label);
        let progress_bar = gtk::ProgressBar::new();
        progress_bar.set_valign(gtk::Align::Center);
        progress_bar.set_size_request(140, -1);
        progress.append(&progress_bar);
        let cancel_key = gtk::Button::with_label("Stop");
        cancel_key.add_css_class("quiet");
        progress.append(&cancel_key);
        progress.set_visible(false);
        float.append(&progress);
        let stale = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        stale.add_css_class("video-pill");
        let stale_note = label("", "");
        stale.append(&stale_note);
        let stale_key = gtk::Button::with_label("Render a preview");
        stale_key.add_css_class("quiet");
        stale.append(&stale_key);
        stale.set_visible(false);
        float.append(&stale);
        overlay.add_overlay(&float);
        let toast = gtk::Revealer::new();
        toast.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        toast.set_halign(gtk::Align::Center);
        toast.set_valign(gtk::Align::End);
        toast.set_margin_bottom(72);
        let toast_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        toast_row.add_css_class("design-toast");
        let toast_label = label("", "");
        toast_label.set_wrap(true);
        toast_label.set_max_width_chars(70);
        toast_row.append(&toast_label);
        toast.set_child(Some(&toast_row));
        overlay.add_overlay(&toast);
        body.append(&overlay);

        let side = gtk::ScrolledWindow::new();
        side.add_css_class("design-inspector");
        side.add_css_class("video-side");
        side.set_hscrollbar_policy(gtk::PolicyType::Never);
        // As wide as its fields want, up to 340: a long prop must not take the player's room.
        side.set_size_request(320, -1);
        side.set_propagate_natural_width(true);
        side.set_max_content_width(340);
        // Its rows expand within it; it does not take the player's room. A card inset from the
        // window's edges, as the design board's inspector is, rather than flush against them.
        side.set_hexpand(false);
        side.set_margin_top(12);
        side.set_margin_bottom(12);
        side.set_margin_end(12);
        side.set_margin_start(12);
        let side_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (comp_section, comp_body) = section("Composition");
        let comp_title = comp_section.first_child().and_downcast::<gtk::Label>().unwrap_or_else(|| label("", ""));
        let cuts = gtk::ListBox::new();
        cuts.add_css_class("video-cuts");
        cuts.set_selection_mode(gtk::SelectionMode::Single);
        cuts.set_visible(false);
        comp_body.append(&cuts);
        let about = label("", "dim");
        about.set_xalign(0.0);
        about.set_wrap(true);
        comp_body.append(&about);
        side_box.append(&comp_section);
        let (props_section, props_outer) = section("Props");
        let props_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        props_outer.append(&props_box);
        let props_keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        props_keys.set_margin_top(4);
        let apply = gtk::Button::with_label("Apply and preview");
        apply.add_css_class("primary");
        let reset = gtk::Button::with_label("Reset");
        reset.set_tooltip_text(Some("Back to the props the code sets"));
        props_keys.append(&apply);
        props_keys.append(&reset);
        props_outer.append(&props_keys);
        side_box.append(&props_section);
        let (renders_section, renders_box) = section("Renders");
        side_box.append(&renders_section);
        let (versions_section, versions_box) = section("Versions");
        side_box.append(&versions_section);
        let (voices_section, voices_box) = section("ElevenLabs");
        side_box.append(&voices_section);
        side.set_child(Some(&side_box));
        body.append(&side);
        root.append(&body);

        let b = Rc::new_cyclic(|me| VideoBoard {
            root,
            ui: Rc::downgrade(ui),
            id,
            detail: RefCell::default(),
            title,
            terminal,
            lamp,
            status,
            stage,
            player,
            controls,
            poster,
            stage_note,
            play_key: play_key.clone(),
            preview_key: preview_key.clone(),
            stale,
            stale_note,
            progress,
            progress_bar,
            progress_label,
            playing: RefCell::default(),
            following: Cell::new(true),
            compare: RefCell::default(),
            compare_bar,
            compare_keys: compare_keys.clone(),
            cut: RefCell::default(),
            comp_title,
            cuts: cuts.clone(),
            compositions: RefCell::default(),
            choosing: Cell::new(false),
            about,
            props_box,
            fields: RefCell::default(),
            props_sig: RefCell::default(),
            renders_box,
            versions_box,
            voices_box,
            toast,
            toast_label,
            toast_serial: Cell::new(0),
            ask_draft: RefCell::default(),
            back: RefCell::default(),
            me: me.clone(),
        });
        let weak = Rc::downgrade(&b);
        back_key.connect_clicked(move |_| {
            let back = weak.upgrade().and_then(|b| b.back.borrow().clone());
            if let Some(back) = back {
                back();
            }
        });
        let weak = Rc::downgrade(&b);
        let title_pop = gtk::Popover::new();
        title_pop.add_css_class("design-menu");
        title_key.set_popover(Some(&title_pop));
        title_pop.connect_show(move |pop| {
            if let Some(b) = weak.upgrade() {
                pop.set_child(Some(&b.title_menu(pop)));
            }
        });
        let render_pop = gtk::Popover::new();
        render_pop.add_css_class("design-menu");
        render_key.set_popover(Some(&render_pop));
        let weak = Rc::downgrade(&b);
        render_pop.connect_show(move |pop| {
            if let Some(b) = weak.upgrade() {
                pop.set_child(Some(&b.render_menu(pop)));
            }
        });
        for key in [&preview_key, &stale_key] {
            let weak = Rc::downgrade(&b);
            key.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.render(json!({"quality": "preview"}));
                }
            });
        }
        let weak = Rc::downgrade(&b);
        play_key.connect_clicked(move |_| {
            let Some(b) = weak.upgrade() else { return };
            if let Some(r) = b.newest_of_cut() {
                b.play(&r, true);
            }
        });
        for (side, key) in compare_keys.iter().enumerate() {
            let weak = Rc::downgrade(&b);
            key.connect_toggled(move |key| {
                if let Some(b) = weak.upgrade().filter(|b| key.is_active() && !b.choosing.get()) {
                    b.show_side(side);
                }
            });
        }
        let weak = Rc::downgrade(&b);
        compare_close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.stop_comparing();
            }
        });
        let weak = Rc::downgrade(&b);
        cuts.connect_row_selected(move |_, row| {
            let Some(b) = weak.upgrade().filter(|b| !b.choosing.get()) else { return };
            let Some(cut) = row.and_then(|row| b.compositions.borrow().get(row.index().max(0) as usize).cloned()) else { return };
            // After the selection settles: showing the cut rebuilds this list.
            let weak = b.me.clone();
            glib::idle_add_local_once(move || {
                if let Some(b) = weak.upgrade() {
                    b.choose_cut(cut);
                }
            });
        });
        let weak = Rc::downgrade(&b);
        cancel_key.connect_clicked(move |_| {
            let Some(b) = weak.upgrade() else { return };
            let running = b.detail.borrow().as_ref().and_then(|d| current_render(&d.renders).map(|r| r.id));
            if let Some(render_id) = running {
                b.call("video.render.cancel", json!({"render_id": render_id}), None);
            }
        });
        let weak = Rc::downgrade(&b);
        apply.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.apply_props(false);
            }
        });
        let weak = Rc::downgrade(&b);
        reset.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.apply_props(true);
            }
        });
        let weak = Rc::downgrade(&b);
        live_key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.watch_live(key.upcast_ref());
            }
        });
        let weak = Rc::downgrade(&b);
        ask_key.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.ask(key.upcast_ref());
            }
        });
        b.reload();
        b.show_voices();
        b
    }

    /// Open the live preview in the browser: Studio starts (a few seconds the first time) and its
    /// link opens.
    fn watch_live(&self, anchor: &gtk::Widget) {
        let Some(ui) = self.ui.upgrade() else { return };
        self.say("Opening a live preview in your browser…");
        let (weak, id) = (self.me.clone(), self.id);
        let window = anchor.root().and_downcast::<gtk::Window>();
        glib::spawn_future_local(async move {
            let opened = ui.call("video.preview", json!({"video_id": id})).await;
            let Some(b) = weak.upgrade() else { return };
            match opened.ok().and_then(|v| v["url"].as_str().map(str::to_string)) {
                Some(url) => {
                    let weak = b.me.clone();
                    gtk::UriLauncher::new(&url).launch(window.as_ref(), gio::Cancellable::NONE, move |r| {
                        if let (Err(e), Some(b)) = (r, weak.upgrade()) {
                            b.say(&format!("Could not open the browser: {e}"));
                        }
                    });
                }
                None => b.say("The live preview could not start: check the code (Ask agent), or set the kit up again from the gallery."),
            }
        });
    }

    /// The ElevenLabs key: saved here by the person, for voice lines, music and sound effects.
    /// Agents use it and never see it.
    fn show_voices(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let status = ui.call("video.elevenlabs.status", json!({})).await;
            let Some(b) = weak.upgrade() else { return };
            clear(&b.voices_box);
            let saved = status.as_ref().ok().and_then(|v| v["saved"].as_bool()).unwrap_or(false);
            if saved {
                let v = status.unwrap_or_default();
                let mut text = "Key saved".to_string();
                if let Some(tier) = v["tier"].as_str() {
                    text.push_str(&format!(" · {tier}"));
                }
                if let Some(left) = v["characters_left"].as_i64() {
                    text.push_str(&format!(" · {} characters left", group(left)));
                }
                let note = label(&text, "dim");
                note.set_xalign(0.0);
                note.set_wrap(true);
                // A wrapped label asks for its whole text on one line unless held: the side
                // panel must not take the player's room.
                note.set_max_width_chars(36);
                b.voices_box.append(&note);
                let remove = gtk::Button::with_label("Remove key");
                remove.set_halign(gtk::Align::Start);
                let weak = b.me.clone();
                remove.connect_clicked(move |_| {
                    let Some(b) = weak.upgrade() else { return };
                    let Some(ui) = b.ui.upgrade() else { return };
                    let weak = b.me.clone();
                    glib::spawn_future_local(async move {
                        let _ = ui.call("video.elevenlabs.key_remove", json!({})).await;
                        if let Some(b) = weak.upgrade() {
                            b.show_voices();
                        }
                    });
                });
                b.voices_box.append(&remove);
                return;
            }
            let note = label("Voice lines, music and sound effects use your ElevenLabs key (elevenlabs.io, API keys). It stays in your keyring; agents never see it.", "dim");
            note.set_xalign(0.0);
            note.set_wrap(true);
            note.set_max_width_chars(36);
            b.voices_box.append(&note);
            let entry = gtk::PasswordEntry::new();
            entry.set_show_peek_icon(true);
            entry.set_placeholder_text(Some("ElevenLabs API key"));
            b.voices_box.append(&entry);
            let save = gtk::Button::with_label("Save key");
            save.add_css_class("primary");
            save.set_halign(gtk::Align::Start);
            let weak = b.me.clone();
            let field = entry.clone();
            save.connect_clicked(move |_| {
                let Some(b) = weak.upgrade() else { return };
                let Some(ui) = b.ui.upgrade() else { return };
                let key = field.text().to_string();
                field.set_text("");
                let weak = b.me.clone();
                glib::spawn_future_local(async move {
                    let saved = ui.call("video.elevenlabs.key_set", json!({"key": key})).await;
                    let Some(b) = weak.upgrade() else { return };
                    match saved {
                        Ok(_) => b.say("ElevenLabs key saved."),
                        Err(e) => b.say(&format!("The key was not saved: {e}")),
                    }
                    b.show_voices();
                });
            });
            b.voices_box.append(&save);
        });
    }

    fn video(&self) -> Option<Video> {
        self.detail.borrow().as_ref().map(|d| d.video.clone())
    }

    /// The cut on show, as the last check found it.
    fn composition_of(&self) -> Option<Composition> {
        let d = self.detail.borrow();
        let cut = self.cut.borrow();
        d.as_ref()?.compositions.iter().find(|c| c.id == *cut).cloned()
    }

    /// A composition's frame rate, as the last check found it; the video's when it was not.
    fn fps_of(&self, composition: &str) -> f64 {
        let d = self.detail.borrow();
        let Some(d) = d.as_ref() else { return 30.0 };
        d.compositions.iter().find(|c| c.id == composition).map_or(d.video.fps, |c| c.fps)
    }

    /// The newest render of the cut on show that plays here.
    fn newest_of_cut(&self) -> Option<VideoRender> {
        let cut = self.cut.borrow();
        self.detail.borrow().as_ref()?.renders.iter().find(|r| playable(r) && r.composition == *cut).cloned()
    }

    /// Show cut `cut`: its renders on the stage, its props, and Render and Ask about it.
    fn choose_cut(&self, cut: String) {
        if *self.cut.borrow() == cut {
            return;
        }
        *self.cut.borrow_mut() = cut;
        self.stop_comparing();
        self.following.set(true);
        self.show();
    }

    /// Read the video again and show it.
    pub fn reload(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let (weak, id) = (self.me.clone(), self.id);
        glib::spawn_future_local(async move {
            let got = ui.call("video.get", json!({"video_id": id})).await;
            let Some(b) = weak.upgrade() else { return };
            match got.map(serde_json::from_value::<GetOut>) {
                Ok(Ok(detail)) => {
                    let (project, session) = (detail.video.project_id, detail.video.session.clone());
                    *b.detail.borrow_mut() = Some(detail);
                    b.show();
                    // The video smoke's captures of the player and of two renders compared
                    // (scripts/video-smoke.py).
                    if std::env::var_os("RELAY_SMOKE_VIDEO_PLAY").is_some() && b.play_key.is_visible() {
                        b.play_key.emit_clicked();
                    }
                    if std::env::var_os("RELAY_SMOKE_VIDEO_VERSIONS").is_some() {
                        // Once laid out: the side panel scrolled to the versions.
                        let versions = b.versions_box.downgrade();
                        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                            let Some(versions) = versions.upgrade() else { return };
                            let side = versions.ancestor(gtk::ScrolledWindow::static_type()).and_downcast::<gtk::ScrolledWindow>();
                            let at = versions.parent().and_then(|section| section.compute_point(&versions.ancestor(gtk::Viewport::static_type())?, &gtk::graphene::Point::new(0.0, 0.0)));
                            if let (Some(side), Some(at)) = (side, at) {
                                side.vadjustment().set_value(at.y() as f64 - 12.0);
                            }
                        });
                    }
                    if std::env::var_os("RELAY_SMOKE_VIDEO_COMPARE").is_some() && b.compare.borrow().is_none() {
                        let other = b.newest_of_cut().and_then(|a| {
                            b.detail.borrow().as_ref()?.renders.iter().find(|r| playable(r) && r.id != a.id).cloned().map(|o| (a, o))
                        });
                        if let Some((a, other)) = other {
                            b.play(&a, true);
                            b.compare_with(&other);
                        }
                    }
                    // The lamp starts at the terminal's state now; session.changed moves it after.
                    if let Some(session) = session {
                        if let Ok(listed) = ui.call("session.list", json!({"project_id": project})).await {
                            if let Some(b) = weak.upgrade() {
                                b.set_lamp(&session_state(&listed, &session));
                            }
                        }
                    }
                }
                Ok(Err(e)) => b.say(&format!("Could not read the video: {e}")),
                Err(e) => b.say(&format!("Could not read the video: {e}")),
            }
        });
    }

    /// An event about this video, or one of its renders.
    pub fn event(&self, ev: &str, payload: &Value) {
        match ev {
            "video.render.changed" | "video.render.finished" => {
                let Ok(render) = serde_json::from_value::<VideoRender>(payload.clone()) else { return };
                if render.video_id != self.id {
                    return;
                }
                let finished = ev == "video.render.finished";
                if let Some(d) = self.detail.borrow_mut().as_mut() {
                    match d.renders.iter_mut().find(|r| r.id == render.id) {
                        Some(r) => *r = render.clone(),
                        None => d.renders.insert(0, render.clone()),
                    }
                }
                if finished {
                    match render.state {
                        RenderState::Done => self.say(&format!("Rendered: {}", render.path.as_deref().unwrap_or_default())),
                        RenderState::Failed => self.say(&format!("The render failed: {}", render.error.as_deref().unwrap_or_default())),
                        _ => {}
                    }
                    self.reload();
                } else {
                    self.show_progress();
                }
            }
            _ if payload["id"].as_i64() == Some(self.id) => self.reload(),
            _ => {}
        }
    }

    /// A terminal's state changed: the lamp follows the video's.
    pub fn session_changed(&self, payload: &Value) {
        let video = self.video();
        if video.as_ref().and_then(|v| v.session.as_deref()) == payload["name"].as_str() {
            self.set_lamp(text(payload, "state"));
        }
    }

    fn set_lamp(&self, state: &str) {
        for class in ["running", "idle", "created", "spawning", "restorable", "blocked", "parked", "exited", "closed"] {
            self.lamp.remove_css_class(class);
        }
        self.lamp.add_css_class(state);
    }

    fn show(&self) {
        let Some(detail) = self.detail.borrow().clone() else { return };
        let v = &detail.video;
        self.title.set_text(&v.title);
        match &v.session {
            Some(s) => {
                self.terminal.set_text(s);
                self.lamp.set_visible(true);
            }
            None => {
                self.terminal.set_text("No terminal");
                self.lamp.set_visible(false);
            }
        }
        self.status.set_text(&format!("v{} · {}", v.version, ago(&v.updated_at, Form::CompactAgo).map_or_else(|| v.updated_by.clone(), |when| format!("{} {when}", v.updated_by))));

        // The cut: the one the person picked while the video still has it, else its own.
        let ids: Vec<String> = detail.compositions.iter().map(|c| c.id.clone()).collect();
        let kept = !self.cut.borrow().is_empty() && ids.contains(&self.cut.borrow());
        if !kept {
            *self.cut.borrow_mut() = v.composition.clone();
        }
        self.show_cuts(&detail, &ids);

        self.show_props(&detail);
        self.show_stage(&detail);
        self.show_progress();
        self.show_renders(&detail);
        self.show_versions(&detail);
    }

    /// One composition: what it is. Several: a cut each, to pick, with its newest render.
    fn show_cuts(&self, detail: &GetOut, ids: &[String]) {
        let many = ids.len() > 1;
        self.comp_title.set_text(if many { "Cuts" } else { "Composition" });
        self.cuts.set_visible(many);
        self.about.set_visible(!many);
        *self.compositions.borrow_mut() = ids.to_vec();
        if !many {
            self.about.set_text(&match self.composition_of() {
                Some(c) => format!("{} · {} × {} · {} fps · {}", c.id, c.width, c.height, c.fps, super::videos::clock(c.duration_frames, c.fps)),
                None => "Not checked yet: an agent's video.check lists the compositions.".into(),
            });
            return;
        }
        self.choosing.set(true);
        self.cuts.remove_all();
        let cut = self.cut.borrow().clone();
        for c in &detail.compositions {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
            row.add_css_class("video-cut");
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let name = label(&c.id, "video-row-title");
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(24);
            head.append(&name);
            if c.id == detail.video.composition {
                let own = label("default", "faint");
                own.set_tooltip_text(Some("What agents draw and render unless they name another"));
                head.append(&own);
            }
            row.append(&head);
            let size = format!("{} × {} · {} fps · {}", c.width, c.height, c.fps, super::videos::clock(c.duration_frames, c.fps));
            let newest = detail.renders.iter().find(|r| r.composition == c.id && r.state == RenderState::Done && r.replaced_by.is_none());
            let rendered = newest.map_or_else(|| "no render yet".to_string(), |r| {
                format!("v{} {}", r.version, r.finished_at.as_deref().and_then(|t| ago(t, Form::CompactAgo)).unwrap_or_default())
            });
            let sub = label(&format!("{size} · {rendered}"), "dim");
            sub.set_xalign(0.0);
            sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
            sub.set_max_width_chars(36);
            row.append(&sub);
            self.cuts.append(&row);
            if c.id == cut {
                if let Some(r) = self.cuts.last_child().and_downcast::<gtk::ListBoxRow>() {
                    self.cuts.select_row(Some(&r));
                }
            }
        }
        self.choosing.set(false);
    }

    fn show_stage(&self, detail: &GetOut) {
        let v = &detail.video;
        let cut = self.cut.borrow().clone();
        let many = detail.compositions.len() > 1;
        let newest = detail.renders.iter().find(|r| playable(r) && r.composition == cut).cloned();
        if let Some(path) = &v.poster {
            self.poster.set_filename(Some(path));
        }
        let in_player = self.stage.visible_child_name().as_deref() == Some("player");
        // Comparing: the two renders stay as the person set them.
        let comparing = self.compare.borrow().is_some();
        match &newest {
            Some(r) => {
                // Watching the cut's newest: the next one takes its place, paused. A render the
                // person picked stays.
                let same = self.playing.borrow().as_ref().is_some_and(|p| p.id == r.id && p.path == r.path);
                if in_player && !same && !comparing && self.following.get() {
                    self.load(r);
                }
                if !in_player {
                    self.stage.set_visible_child_name("poster");
                }
                self.play_key.set_visible(true);
                self.preview_key.set_visible(false);
                let size = r.bytes.map(megabytes).unwrap_or_default();
                let of = if many { format!("{cut}, v{}", r.version) } else { format!("v{}", r.version) };
                self.stage_note.set_text(&format!("{} {} of {of} · {size}", r.format.name().to_uppercase(),
                    if r.quality == relay_bus::ops::video::Quality::Preview { "preview" } else { "render" }));
                let older = r.version < v.version;
                self.stale.set_visible(older);
                self.stale_note.set_text(&format!("This render is of v{}; the code is at v{}.", r.version, v.version));
            }
            None => {
                self.stale.set_visible(false);
                if !comparing {
                    self.stage.set_visible_child_name("poster");
                }
                self.play_key.set_visible(false);
                self.preview_key.set_visible(true);
                self.stage_note.set_text(&if many {
                    format!("No render of {cut} yet. A preview renders it at half size, in seconds.")
                } else if v.poster.is_some() {
                    "No render to play yet. A preview renders it at half size, in seconds.".to_string()
                } else {
                    "Nothing drawn yet. Once an agent has written the video, render a preview to watch it.".to_string()
                });
            }
        }
    }

    /// Put render `r`'s file in the player, paused where it starts.
    fn load(&self, r: &VideoRender) -> gtk::MediaFile {
        let file = media_file(r);
        self.player.set_media_stream(Some(&file));
        self.controls.set_media_stream(Some(&file));
        *self.playing.borrow_mut() = Some(r.clone());
        file
    }

    /// Play render `r`. `follow`: it is the cut's newest, and the next one takes its place.
    fn play(&self, r: &VideoRender, follow: bool) {
        self.stop_comparing();
        self.following.set(follow);
        let same = self.playing.borrow().as_ref().is_some_and(|p| p.id == r.id && p.path == r.path);
        if !same {
            self.load(r);
        }
        self.stage.set_visible_child_name("player");
        if let Some(stream) = self.player.media_stream() {
            stream.play();
        }
        self.mark_playing();
    }

    /// Compare render `b` with the one on screen (or the cut's newest): the player switches
    /// between them at the same moment.
    fn compare_with(&self, b: &VideoRender) {
        let shown = self.compare.borrow().as_ref().map(|c| c.renders[c.showing].clone());
        let on_screen = self.stage.visible_child_name().as_deref() == Some("player");
        let a = shown.or_else(|| self.playing.borrow().clone().filter(|_| on_screen))
            .filter(|a| a.id != b.id)
            .or_else(|| self.newest_of_cut().filter(|a| a.id != b.id));
        let Some(a) = a else { return self.play(b, false) };
        self.stop_comparing();
        // A keeps the stream it is playing in, so its place and its pause are kept.
        let playing_a = self.playing.borrow().as_ref().is_some_and(|p| p.id == a.id) && on_screen;
        let file_a = match self.player.media_stream().and_downcast::<gtk::MediaFile>().filter(|_| playing_a) {
            Some(file) => file,
            None => self.load(&a),
        };
        let file_b = media_file(b);
        self.choosing.set(true);
        for (key, (side, r)) in self.compare_keys.iter().zip([("A", &a), ("B", b)]) {
            key.set_label(&format!("{side}  {}", render_tag(r)));
            key.set_tooltip_text(Some(&format!("Render {} · {}", r.id, r.composition)));
        }
        self.compare_keys[0].set_active(true);
        self.choosing.set(false);
        *self.compare.borrow_mut() = Some(Compare { renders: [a, b.clone()], files: [file_a, file_b], showing: 0 });
        self.compare_bar.set_visible(true);
        self.stage.set_visible_child_name("player");
        self.following.set(false);
        self.show_side(1);
    }

    /// Show side `side` (0 for A, 1 for B) of a comparison, at the moment the other was at, playing
    /// if it was.
    fn show_side(&self, side: usize) {
        let switched = {
            let mut compare = self.compare.borrow_mut();
            let Some(c) = compare.as_mut() else { return };
            if c.showing == side && self.player.media_stream().is_some_and(|s| s == *c.files[side].upcast_ref::<gtk::MediaStream>()) {
                return;
            }
            let (from, to) = (c.showing, side);
            let (seconds, was_playing) = (c.files[from].timestamp() as f64 / 1e6, c.files[from].is_playing());
            c.files[from].pause();
            let at = same_moment(&c.renders[from], self.fps_of(&c.renders[from].composition), seconds, &c.renders[to], self.fps_of(&c.renders[to].composition));
            self.player.set_media_stream(Some(&c.files[to]));
            self.controls.set_media_stream(Some(&c.files[to]));
            seek_when_ready(&c.files[to], at, was_playing, &self.player);
            c.showing = to;
            c.renders[to].clone()
        };
        *self.playing.borrow_mut() = Some(switched);
        self.choosing.set(true);
        self.compare_keys[side].set_active(true);
        self.choosing.set(false);
        self.mark_playing();
    }

    /// End a comparison, keeping the render on screen where it is.
    fn stop_comparing(&self) {
        if self.compare.borrow_mut().take().is_some() {
            self.compare_bar.set_visible(false);
            self.mark_playing();
        }
    }

    /// The renders list marks the one on screen.
    fn mark_playing(&self) {
        let detail = self.detail.borrow().clone();
        if let Some(detail) = detail {
            self.show_renders(&detail);
        }
    }

    fn show_progress(&self) {
        let running = self.detail.borrow().as_ref().and_then(|d| current_render(&d.renders).cloned());
        match running {
            Some(r) => {
                self.progress.set_visible(true);
                self.progress_bar.set_fraction(r.progress);
                let what = format!("{} {}", if r.quality.eq(&relay_bus::ops::video::Quality::Preview) { "Preview" } else { "Render" }, r.format.name());
                self.progress_label.set_text(&if r.state == RenderState::Queued {
                    format!("{what} queued")
                } else {
                    format!("{what} {}%", (r.progress * 100.0).round())
                });
            }
            None => self.progress.set_visible(false),
        }
    }

    fn show_props(&self, detail: &GetOut) {
        let cut = self.cut.borrow().clone();
        let comp = detail.compositions.iter().find(|c| c.id == cut);
        let sig = json!([
            cut,
            comp.map(|c| (&c.props_schema, &c.default_props)),
            detail.props.get(&cut),
        ])
        .to_string();
        if self.props_sig.borrow().as_deref() == Some(sig.as_str()) {
            return;
        }
        *self.props_sig.borrow_mut() = Some(sig);
        clear(&self.props_box);
        self.fields.borrow_mut().clear();
        let Some(comp) = comp else {
            self.props_box.append(&label("The props show once the video has been checked.", "dim"));
            return;
        };
        let Some(schema) = comp.props_schema.as_ref().and_then(|s| s.get("properties")).and_then(Value::as_object) else {
            let note = label("This composition declares no props schema: ask its agent to describe what you can change.", "dim");
            note.set_wrap(true);
            note.set_xalign(0.0);
            self.props_box.append(&note);
            return;
        };
        let set = detail.props.get(&comp.id).and_then(Value::as_object).cloned().unwrap_or_default();
        for (name, field) in schema {
            let value = set.get(name).or_else(|| comp.default_props.get(name)).cloned().unwrap_or(Value::Null);
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let caption_label = label(&caption(name, field), "video-prop-label");
            caption_label.set_xalign(0.0);
            row.append(&caption_label);
            let kind = field["type"].as_str().unwrap_or_default();
            let made = if let Some(values) = field["enum"].as_array() {
                let names: Vec<String> = values.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string)).collect();
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                let drop = gtk::DropDown::from_strings(&refs);
                drop.add_css_class("design-dropdown");
                if let Some(at) = values.iter().position(|v| *v == value) {
                    drop.set_selected(at as u32);
                }
                row.append(&drop);
                Field::Choice(drop, values.clone())
            } else if kind == "string" && is_colour(field, &value) {
                let button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
                button.set_halign(gtk::Align::Start);
                if let Ok(rgba) = gdk::RGBA::parse(value.as_str().unwrap_or("#000000")) {
                    button.set_rgba(&rgba);
                }
                row.append(&button);
                Field::Colour(button)
            } else if kind == "string" {
                let entry = gtk::Entry::new();
                entry.set_text(value.as_str().unwrap_or_default());
                if let Some(max) = field["maxLength"].as_i64() {
                    entry.set_max_length(max as i32);
                }
                row.append(&entry);
                Field::Text(entry)
            } else if kind == "number" || kind == "integer" {
                let min = field["minimum"].as_f64().unwrap_or(-1e9);
                let max = field["maximum"].as_f64().unwrap_or(1e9);
                let integer = kind == "integer";
                let spin = gtk::SpinButton::with_range(min, max, if integer { 1.0 } else { 0.1 });
                spin.set_digits(if integer { 0 } else { 2 });
                spin.set_value(value.as_f64().unwrap_or(0.0));
                spin.set_halign(gtk::Align::Start);
                row.append(&spin);
                Field::Number(spin, integer)
            } else if kind == "boolean" {
                let switch = gtk::Switch::new();
                switch.set_active(value.as_bool().unwrap_or(false));
                switch.set_halign(gtk::Align::Start);
                row.append(&switch);
                Field::Toggle(switch)
            } else {
                let entry = gtk::Entry::new();
                entry.set_text(&value.to_string());
                entry.set_tooltip_text(Some("As JSON"));
                row.append(&entry);
                Field::Raw(entry)
            };
            self.props_box.append(&row);
            self.fields.borrow_mut().push((name.clone(), made));
        }
    }

    fn show_renders(&self, detail: &GetOut) {
        clear(&self.renders_box);
        if detail.renders.is_empty() {
            self.renders_box.append(&label("None yet.", "dim"));
            return;
        }
        let many = detail.compositions.len() > 1;
        let on_screen = self.playing.borrow().as_ref().map(|p| p.id).filter(|_| self.stage.visible_child_name().as_deref() == Some("player"));
        for r in detail.renders.iter().take(10) {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("video-row");
            if on_screen == Some(r.id) {
                row.add_css_class("video-row-playing");
            }
            let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
            words.set_hexpand(true);
            let quality = match r.quality {
                relay_bus::ops::video::Quality::Preview => " preview",
                relay_bus::ops::video::Quality::High => " high",
                relay_bus::ops::video::Quality::Standard => "",
            };
            let named = r.name.as_deref().map(|n| format!("{n} · ")).unwrap_or_default();
            let head = label(&format!("{named}{}{} · v{}", r.format.name().to_uppercase(), quality, r.version), "video-row-title");
            head.set_xalign(0.0);
            head.set_ellipsize(gtk::pango::EllipsizeMode::End);
            // An ellipsized label still asks for its whole text: held, so a long render name
            // never widens the panel into the player's room.
            head.set_max_width_chars(28);
            words.append(&head);
            let when = r.finished_at.as_deref().or(Some(r.created_at.as_str())).and_then(|t| ago(t, Form::CompactAgo)).unwrap_or_default();
            let cut = if many { format!("{} · ", r.composition) } else { String::new() };
            let detail_text = match (r.state, r.replaced_by) {
                (RenderState::Done, Some(by)) => format!("{cut}replaced by render {by} of the same name"),
                (RenderState::Done, None) => format!("{cut}{} · {}", r.bytes.map(megabytes).unwrap_or_default(), when),
                (RenderState::Failed, _) => format!("failed: {}", r.error.as_deref().unwrap_or_default()),
                (RenderState::Cancelled, _) => format!("{cut}stopped {when}"),
                (RenderState::Queued, _) => format!("{cut}queued"),
                (RenderState::Rendering, _) => format!("{cut}{}%", (r.progress * 100.0).round()),
            };
            let sub = label(&detail_text, "dim");
            sub.set_xalign(0.0);
            sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
            sub.set_max_width_chars(30);
            sub.set_tooltip_text(r.error.as_deref().or(r.path.as_deref().filter(|_| r.replaced_by.is_none())));
            words.append(&sub);
            // Its sound as measured: loudness and peak, red when it clips.
            if let Some(audio) = r.audio.as_ref().filter(|_| r.state == RenderState::Done && r.replaced_by.is_none()) {
                let line = sound_line(audio);
                let warn = audio.warnings.iter().any(|w| w.kind == "clipping" || w.kind == "hot");
                let sound = label(&line, if warn { "video-warn" } else { "dim" });
                sound.set_xalign(0.0);
                sound.set_ellipsize(gtk::pango::EllipsizeMode::End);
                sound.set_max_width_chars(30);
                let tip = audio.warnings.iter().map(|w| w.detail.as_str()).collect::<Vec<_>>().join("\n");
                if !tip.is_empty() {
                    sound.set_tooltip_text(Some(&tip));
                }
                words.append(&sound);
            }
            row.append(&words);
            if playable(r) {
                let play = crate::app::icon_button("play", "Play this render");
                let (weak, render) = (self.me.clone(), r.clone());
                play.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.play(&render, false);
                    }
                });
                row.append(&play);
                let compare = crate::app::icon_button("columns", "Compare with the render on screen, or the cut's newest: switch between them at the same moment");
                compare.set_widget_name(&format!("video-compare-render-{}", r.id));
                let (weak, render) = (self.me.clone(), r.clone());
                compare.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.compare_with(&render);
                    }
                });
                compare.set_sensitive(on_screen != Some(r.id));
                row.append(&compare);
            }
            if let Some(path) = r.path.clone().filter(|_| r.state == RenderState::Done && r.replaced_by.is_none()) {
                let show = crate::app::icon_button("folder-open", "Show the file in its folder");
                let weak = self.me.clone();
                show.connect_clicked(move |key| {
                    let window = key.root().and_downcast::<gtk::Window>();
                    let weak = weak.clone();
                    gtk::FileLauncher::new(Some(&gio::File::for_path(&path))).open_containing_folder(window.as_ref(), gio::Cancellable::NONE, move |r| {
                        if let (Err(e), Some(b)) = (r, weak.upgrade()) {
                            b.say(&format!("Could not show the folder: {e}"));
                        }
                    });
                });
                row.append(&show);
            }
            self.renders_box.append(&row);
        }
    }

    /// The versions, newest first, then the older ones kept by name: each with what it changed,
    /// its render to play when there is one, and a menu to name it or put it back.
    fn show_versions(&self, detail: &GetOut) {
        clear(&self.versions_box);
        if detail.versions.is_empty() {
            self.versions_box.append(&label("Recorded as the code is checked, looked at and rendered.", "dim"));
            return;
        }
        let cut = self.cut.borrow().clone();
        for (i, version) in detail.versions.iter().enumerate() {
            // video.get answers the newest twenty, then the older ones kept by name.
            if i == 20 {
                let kept = label("Kept", "video-versions-kept");
                kept.set_xalign(0.0);
                self.versions_box.append(&kept);
            }
            let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
            row.add_css_class("video-row");
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let n = label(&format!("v{}", version.n), "video-row-title");
            head.append(&n);
            let named = label(version.label.as_deref().unwrap_or_default(), "video-version-label");
            if let Some(name) = &version.label {
                head.append(&crate::icons::image("pin", 12));
                named.set_tooltip_text(Some(name));
            }
            named.set_xalign(0.0);
            named.set_hexpand(true);
            named.set_ellipsize(gtk::pango::EllipsizeMode::End);
            named.set_max_width_chars(16);
            head.append(&named);
            let current = version.n == detail.video.version;
            if current {
                head.append(&label("now", "dim"));
            }
            // Its render, of the cut on show first.
            let render = detail.renders.iter().filter(|r| playable(r) && r.version == version.n)
                .min_by_key(|r| (r.composition != cut, -r.id)).cloned();
            if let Some(r) = render {
                let play = crate::app::icon_button("play", &format!("Play render {} ({}), of this version", r.id, r.composition));
                let weak = self.me.clone();
                play.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.play(&r, false);
                    }
                });
                head.append(&play);
            }
            let menu = gtk::MenuButton::new();
            menu.set_child(Some(&crate::icons::image("more", 16)));
            menu.add_css_class("quiet");
            menu.add_css_class("icon-key");
            menu.set_tooltip_text(Some("Name this version, or put the code back as it had it"));
            menu.set_widget_name(&format!("video-version-{}", version.n));
            let pop = gtk::Popover::new();
            pop.add_css_class("design-menu");
            menu.set_popover(Some(&pop));
            let (weak, version_n, label_now) = (self.me.clone(), version.n, version.label.clone());
            pop.connect_show(move |pop| {
                if let Some(b) = weak.upgrade() {
                    pop.set_child(Some(&b.version_menu(pop, version_n, label_now.as_deref(), current)));
                }
            });
            head.append(&menu);
            row.append(&head);
            // Who and when, then what it changed.
            let mut line = format!("{} · {}", version.actor, ago(&version.created_at, Form::CompactAgo).unwrap_or_default());
            if !version.changed.is_empty() {
                let shown: Vec<&str> = version.changed.iter().take(3).map(String::as_str).collect();
                let more = version.changed.len().saturating_sub(3);
                line.push_str(&format!(" · {}", shown.join(", ")));
                if more > 0 {
                    line.push_str(&format!(" +{more}"));
                }
            }
            let changed = label(&line, "video-version-changed");
            changed.set_xalign(0.0);
            changed.set_ellipsize(gtk::pango::EllipsizeMode::End);
            changed.set_max_width_chars(36);
            if !version.changed.is_empty() {
                changed.set_tooltip_text(Some(&version.changed.join("\n")));
            }
            row.append(&changed);
            self.versions_box.append(&row);
        }
    }

    /// A version's menu: a name for it (Enter keeps it, empty takes it off), and Restore.
    fn version_menu(&self, pop: &gtk::Popover, n: i64, named: Option<&str>, current: bool) -> gtk::Box {
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 4);
        menu.set_size_request(260, -1);
        let caption = label(if named.is_some() { "Its name" } else { "Name this version to keep it" }, "dim");
        caption.set_xalign(0.0);
        menu.append(&caption);
        let name = gtk::Entry::new();
        name.set_text(named.unwrap_or_default());
        name.set_placeholder_text(Some("Narrated cut, warm grade"));
        name.set_max_length(relay_bus::ops::video::MAX_VERSION_LABEL as i32);
        let (weak, popw) = (self.me.clone(), pop.downgrade());
        name.connect_activate(move |e| {
            let text = e.text().trim().to_string();
            if let Some(b) = weak.upgrade() {
                let label = if text.is_empty() { Value::Null } else { json!(text) };
                b.call("video.version.label", json!({"video_id": b.id, "version": n, "label": label}), None);
            }
            if let Some(p) = popw.upgrade() {
                p.popdown();
            }
        });
        menu.append(&name);
        if !current {
            let restore = gtk::Button::with_label(&format!("Restore v{n}"));
            restore.add_css_class("quiet");
            restore.set_tooltip_text(Some("Put the code back as this version had it; the code as it is now is kept as a version first"));
            if let Some(child) = restore.child().and_downcast::<gtk::Label>() {
                child.set_xalign(0.0);
            }
            let (weak, popw) = (self.me.clone(), pop.downgrade());
            restore.connect_clicked(move |_| {
                if let Some(p) = popw.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    b.call("video.version.restore", json!({"video_id": b.id, "version": n}), Some(format!("Restored v{n}; the code before is kept as a version.")));
                }
            });
            menu.append(&restore);
        }
        menu
    }

    fn title_menu(&self, pop: &gtk::Popover) -> gtk::Box {
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let name = gtk::Entry::new();
        name.set_text(&self.title.text());
        let (weak, popw) = (self.me.clone(), pop.downgrade());
        name.connect_activate(move |e| {
            if let Some(b) = weak.upgrade() {
                b.call("video.update", json!({"video_id": b.id, "title": e.text().to_string()}), None);
            }
            if let Some(p) = popw.upgrade() {
                p.popdown();
            }
        });
        menu.append(&name);
        let folder = gtk::Button::with_label("Open its folder");
        folder.add_css_class("quiet");
        let weak = self.me.clone();
        folder.connect_clicked(move |key| {
            let Some(dir) = weak.upgrade().and_then(|b| b.video()).map(|v| v.dir) else { return };
            let window = key.root().and_downcast::<gtk::Window>();
            gtk::FileLauncher::new(Some(&gio::File::for_path(&dir))).launch(window.as_ref(), gio::Cancellable::NONE, |_| {});
        });
        menu.append(&folder);
        let delete = gtk::Button::with_label("Delete video");
        delete.add_css_class("quiet");
        delete.add_css_class("destructive-quiet");
        let (weak, popw) = (self.me.clone(), pop.downgrade());
        delete.connect_clicked(move |_| {
            if let Some(p) = popw.upgrade() {
                p.popdown();
            }
            let Some(b) = weak.upgrade() else { return };
            // At once, as a design's delete, with an Undo on the gallery it goes back to.
            if let Some(ui) = b.ui.upgrade() {
                let (id, title) = (b.id, b.video().map(|v| v.title).unwrap_or_default());
                glib::spawn_future_local(async move {
                    match ui.call("video.delete", json!({"video_id": id})).await {
                        Ok(_) => crate::design::video_deleted(&ui, id, &title),
                        Err(e) => ui.show_info(&format!("Could not delete the video: {e}")),
                    }
                });
            }
            let back = b.back.borrow().clone();
            if let Some(back) = back {
                back();
            }
        });
        menu.append(&delete);
        menu
    }

    fn render_menu(&self, pop: &gtk::Popover) -> gtk::Box {
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let choices: [(&str, Value); 9] = [
            ("Preview (half size, quick)", json!({"quality": "preview"})),
            ("MP4", json!({"format": "mp4"})),
            ("MP4, high quality", json!({"format": "mp4", "quality": "high"})),
            ("For YouTube, Shorts, Reels, TikTok (−14 LUFS)", json!({"format": "mp4", "quality": "high", "preset": "youtube", "name": "youtube"})),
            ("For Discord (−16 LUFS)", json!({"format": "mp4", "preset": "discord", "name": "discord"})),
            ("WebM", json!({"format": "webm"})),
            ("GIF", json!({"format": "gif"})),
            ("ProRes with transparency (.mov)", json!({"format": "prores"})),
            ("Sound only (WAV)", json!({"format": "wav"})),
        ];
        for (caption, payload) in choices {
            let key = gtk::Button::with_label(caption);
            key.add_css_class("quiet");
            key.set_halign(gtk::Align::Fill);
            if let Some(child) = key.child().and_downcast::<gtk::Label>() {
                child.set_xalign(0.0);
            }
            let (weak, popw) = (self.me.clone(), pop.downgrade());
            key.connect_clicked(move |_| {
                if let Some(p) = popw.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    b.render(payload.clone());
                }
            });
            menu.append(&key);
        }
        menu
    }

    /// Render the cut on show as `payload` asks.
    fn render(&self, mut payload: Value) {
        payload["video_id"] = json!(self.id);
        let cut = self.cut.borrow().clone();
        if !cut.is_empty() {
            payload["composition"] = json!(cut);
        }
        // A new preview supersedes those of the cut still waiting: Apply pressed three times
        // renders once more, not three times over.
        if payload["quality"] == "preview" {
            let waiting: Vec<i64> = self.detail.borrow().as_ref().map(|d| d.renders.iter()
                .filter(|r| r.state == RenderState::Queued && r.quality == relay_bus::ops::video::Quality::Preview && r.composition == cut)
                .map(|r| r.id).collect()).unwrap_or_default();
            if let (Some(ui), false) = (self.ui.upgrade(), waiting.is_empty()) {
                glib::spawn_future_local(async move {
                    for render_id in waiting {
                        let _ = ui.call("video.render.cancel", json!({"render_id": render_id})).await;
                    }
                });
            }
        }
        self.call("video.render", payload, Some("Rendering; it plays here when it is done.".into()));
    }

    fn apply_props(&self, reset: bool) {
        let comp = self.cut.borrow().clone();
        if comp.is_empty() {
            return;
        }
        let value = if reset {
            Value::Null
        } else {
            let mut props = Map::new();
            for (name, field) in self.fields.borrow().iter() {
                match field.value() {
                    Some(v) => {
                        props.insert(name.clone(), v);
                    }
                    None => return self.say(&format!("{name} is not valid JSON")),
                }
            }
            Value::Object(props)
        };
        let Some(ui) = self.ui.upgrade() else { return };
        let (weak, id) = (self.me.clone(), self.id);
        glib::spawn_future_local(async move {
            let set = ui.call("video.update", json!({"video_id": id, "props": {comp: value}})).await;
            let Some(b) = weak.upgrade() else { return };
            match set {
                Ok(_) => b.render(json!({"quality": "preview"})),
                Err(e) => b.say(&format!("Could not set the props: {e}")),
            }
        });
    }

    fn call(&self, op: &'static str, payload: Value, done: Option<String>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let result = ui.call(op, payload).await;
            let Some(b) = weak.upgrade() else { return };
            match result {
                Ok(_) => {
                    if let Some(done) = done {
                        b.say(&done);
                    }
                    b.reload();
                }
                Err(e) => b.say(&format!("Could not do it: {e}")),
            }
        });
    }

    pub fn say(&self, text: &str) {
        self.toast_label.set_text(text);
        self.toast.set_reveal_child(true);
        let serial = self.toast_serial.get() + 1;
        self.toast_serial.set(serial);
        let weak = self.me.clone();
        glib::timeout_add_local_once(Duration::from_secs(5), move || {
            if let Some(b) = weak.upgrade().filter(|b| b.toast_serial.get() == serial) {
                b.toast.set_reveal_child(false);
            }
        });
    }

    /// The moment on screen: the render in the player and the frame it shows of its composition,
    /// at that composition's rate (a render of part of it starts at its first frame).
    fn moment(&self) -> Option<Moment> {
        let render = self.playing.borrow().clone()?;
        let stream = self.player.media_stream().filter(|_| self.stage.visible_child_name().as_deref() == Some("player"))?;
        let fps = self.fps_of(&render.composition);
        Some(Moment { frame: frame_at(&render, stream.timestamp() as f64 / 1e6, fps), fps, render })
    }

    /// Ask the video's terminal about the moment on screen.
    fn ask(&self, anchor: &gtk::Widget) {
        let Some(v) = self.video() else { return };
        let Some(session) = v.session.clone() else {
            return self.say("This video has no terminal: an agent's video is its own. Ask from a terminal for a new one.");
        };
        if let Some(stream) = self.player.media_stream() {
            stream.pause();
        }
        let pop = gtk::Popover::new();
        pop.add_css_class("design-ask-pop");
        pop.set_parent(anchor);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_size_request(380, -1);
        let at = self.moment();
        let many = self.detail.borrow().as_ref().is_some_and(|d| d.compositions.len() > 1);
        let heading = match &at {
            Some(m) if many => format!("Ask {session} about {} of {}", frame_label(m.frame, m.fps), m.render.composition),
            Some(m) => format!("Ask {session} about {}", frame_label(m.frame, m.fps)),
            None => format!("Ask {session} about this video"),
        };
        body.append(&label(&heading, "design-section-title"));
        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_size_request(-1, 90);
        view.add_css_class("design-ask-text");
        view.buffer().set_text(&self.ask_draft.borrow());
        body.append(&view);
        let send = gtk::Button::with_label("Send");
        send.add_css_class("primary");
        send.set_halign(gtk::Align::End);
        body.append(&send);
        pop.set_child(Some(&body));
        let (weak, popw, buffer) = (self.me.clone(), pop.downgrade(), view.buffer());
        send.connect_clicked(move |_| {
            let request = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).trim().to_string();
            let Some(b) = weak.upgrade() else { return };
            *b.ask_draft.borrow_mut() = request.clone();
            if request.is_empty() {
                return;
            }
            if let Some(p) = popw.upgrade() {
                p.popdown();
            }
            b.send(session.clone(), request, at.clone());
        });
        let draft = self.me.clone();
        let kept = view.buffer();
        pop.connect_closed(move |pop| {
            if let Some(b) = draft.upgrade() {
                *b.ask_draft.borrow_mut() = kept.text(&kept.start_iter(), &kept.end_iter(), false).to_string();
            }
            pop.unparent();
        });
        pop.popup();
        view.grab_focus();
    }

    /// The prompt an Ask sends: one line naming the video, the cut, the moment and the request,
    /// with the call that shows the agent that frame, and the render the person watched when the
    /// code has moved on since it.
    fn prompt(v: &Video, cut: &str, request: &str, at: Option<&Moment>) -> String {
        let request = request.replace(['\n', '\r'], " ");
        let composition = at.map_or(cut, |m| m.render.composition.as_str());
        let head = format!("[Relay video] \"{}\" (video_id {}, composition {composition})", v.title, v.id);
        let seen = at.filter(|m| m.render.version < v.version)
            .map(|m| format!("; the person watched render {}, of v{} (the code is at v{})", m.render.id, m.render.version, v.version))
            .unwrap_or_default();
        match at {
            Some(m) => format!(
                "{head} at {}: {request} — see that frame with video.still {{\"video_id\": {}, \"composition\": \"{composition}\", \"at\": [\"f{}\"]}}{seen}; its code is in {}",
                frame_label(m.frame, m.fps), v.id, m.frame, v.dir,
            ),
            None => format!(
                "{head}: {request} — look with video.still {{\"video_id\": {}, \"composition\": \"{composition}\"}}; its code is in {}",
                v.id, v.dir,
            ),
        }
    }

    fn send(&self, session: String, request: String, at: Option<Moment>) {
        let (Some(ui), Some(v)) = (self.ui.upgrade(), self.video()) else { return };
        let message = Self::prompt(&v, &self.cut.borrow(), &request, at.as_ref());
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            // The terminal's state now decides the way, as a design's Ask (TM-1, TM-20).
            let way = match ui.call("session.list", json!({"project_id": v.project_id})).await {
                Ok(listed) => {
                    let state = ask::state_of(&listed, &session);
                    ask::recheck(&session, &state, || ask::unsent_note(&session, &ask::provider_of(&listed, &session)))
                }
                Err(e) => Err(format!("Could not check {session}'s terminal: {e}.")),
            };
            let sent = match way {
                Err(note) => Err(note),
                Ok(Route::Mail) => ui.call("mailbox.send", json!({"project_id": v.project_id, "to": session, "text": message, "priority": true})).await
                    .map(|_| format!("{session} is busy; it gets this with its next tool call.")).map_err(|e| e.to_string()),
                Ok(_) => match ui.call("session.input", json!({"session": session, "data": message, "paste": true})).await {
                    Ok(_) => {
                        glib::timeout_future(Duration::from_millis(60)).await;
                        match ui.call("session.input", json!({"session": session, "data": "\r"})).await {
                            Ok(_) => Ok(format!("Sent to {session}. Its changes show here as it renders them.")),
                            Err(_) => Ok(format!("{session} has the request typed but not sent: press Enter in its terminal.")),
                        }
                    }
                    Err(e) => Err(e.to_string()),
                },
            };
            let Some(b) = weak.upgrade() else { return };
            match sent {
                Ok(note) => {
                    b.ask_draft.borrow_mut().clear();
                    b.say(&note);
                }
                Err(note) => b.say(&format!("{note} Your request is kept for the next Ask.")),
            }
        });
    }
}

/// `session`'s state in a `session.list` answer, for the lamp's first state.
fn session_state(listed: &Value, session: &str) -> String {
    rows(listed, "sessions").iter().find(|s| text(s, "name") == session).map_or("closed", |s| text(s, "state")).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_renders_sound_reads_in_a_line() {
        let audio = relay_bus::ops::video::AudioReport {
            lufs: Some(-14.04), lra: None, short_term_max: None, true_peak: -1.12, sample_peak: -1.2, clipped: 0, seconds: 8.0, channels: 2,
            gain_db: None, lufs_before: None, true_peak_before: None, warnings: Vec::new(),
        };
        assert_eq!(sound_line(&audio), "-14.0 LUFS · peak -1.1 dBTP");
        assert_eq!(group(99000), "99,000");
        assert_eq!(group(1234567), "1,234,567");
        assert_eq!(group(12), "12");
    }

    #[test]
    fn the_bar_follows_the_render_running_not_the_newest_queued() {
        let render = |id, state| {
            serde_json::from_value::<VideoRender>(json!({
                "id": id, "video_id": 1, "project_id": 1, "composition": "main", "format": "mp4", "quality": "standard",
                "state": state, "progress": 0.0, "version": 1, "range": null, "path": null, "bytes": null, "error": null,
                "actor": "user", "created_at": "", "started_at": null, "finished_at": null
            }))
            .unwrap()
        };
        // Newest first, as the board keeps them.
        let renders = vec![render(4, "queued"), render(3, "queued"), render(2, "rendering"), render(1, "done")];
        assert_eq!(current_render(&renders).map(|r| r.id), Some(2));
        let waiting = vec![render(4, "queued"), render(3, "queued"), render(1, "done")];
        assert_eq!(current_render(&waiting).map(|r| r.id), Some(3));
        assert!(current_render(&[render(1, "failed")]).is_none());
    }

    #[test]
    fn colours_are_told_by_their_mark_or_their_value() {
        assert!(is_colour(&json!({"type": "string", "description": "__remotion-color"}), &json!("red")));
        assert!(is_colour(&json!({"type": "string", "description": "Accent"}), &json!("#7DD3FC")));
        assert!(is_colour(&json!({"type": "string"}), &json!("#fff")));
        assert!(!is_colour(&json!({"type": "string"}), &json!("#hashtag")));
        assert!(!is_colour(&json!({"type": "string"}), &json!("Hello")));
        assert_eq!(caption("accent", &json!({"description": "__remotion-color"})), "accent");
        assert_eq!(caption("title", &json!({"description": "Headline"})), "Headline");
    }

    fn render(id: i64, composition: &str, version: i64, range: Option<[u32; 2]>) -> VideoRender {
        serde_json::from_value(json!({
            "id": id, "video_id": 4, "project_id": 1, "composition": composition, "format": "mp4", "quality": "standard",
            "state": "done", "progress": 1.0, "version": version, "range": range, "path": format!("/v/out/{id}.mp4"), "bytes": 1, "error": null,
            "actor": "user", "created_at": "", "started_at": null, "finished_at": null
        }))
        .unwrap()
    }

    #[test]
    fn an_ask_names_the_video_the_cut_the_moment_and_how_to_see_it() {
        let v = Video {
            id: 4, project_id: 1, session: Some("snug-zebra".into()), title: "Promo".into(), dir: "/v/p1/4-promo".into(),
            composition: "main".into(), width: 1080, height: 1920, fps: 30.0, duration_frames: 240, version: 3, poster: None,
            last_render: None, created_by: "user".into(), updated_by: "user".into(), created_at: String::new(), updated_at: String::new(), deleted_at: None,
        };
        let at = Moment { render: render(9, "main", 3, None), frame: 45, fps: 30.0 };
        let line = VideoBoard::prompt(&v, "main", "make the title\nbigger", Some(&at));
        assert_eq!(line, "[Relay video] \"Promo\" (video_id 4, composition main) at 1.5s · f45: make the title bigger — see that frame with video.still {\"video_id\": 4, \"composition\": \"main\", \"at\": [\"f45\"]}; its code is in /v/p1/4-promo");
        assert!(!line.contains('\n'));
        // The render on screen names the cut, and says when the code has moved on since it.
        let old = Moment { render: render(7, "voice", 2, None), frame: 90, fps: 60.0 };
        let line = VideoBoard::prompt(&v, "main", "hold the end longer", Some(&old));
        assert!(line.contains("(video_id 4, composition voice) at 1.5s · f90:"), "{line}");
        assert!(line.contains("\"composition\": \"voice\", \"at\": [\"f90\"]}; the person watched render 7, of v2 (the code is at v3);"), "{line}");
        let none = VideoBoard::prompt(&v, "wide", "a square cut too", None);
        assert!(none.contains("composition wide): a square cut too — look with video.still {\"video_id\": 4, \"composition\": \"wide\"}"), "{none}");
    }

    /// A render of part of a composition starts at its first frame; compositions at other rates
    /// meet at the same second.
    #[test]
    fn moments_are_read_in_the_composition_whatever_the_render_covers() {
        let whole = render(1, "show", 28, None);
        let part = render(2, "show", 28, Some([600, 899]));
        assert_eq!(frame_at(&whole, 12.5, 60.0), 750);
        assert_eq!(frame_at(&part, 2.5, 60.0), 750);
        // 12.5 s into the whole is 2.5 s into the part from 10 s.
        assert_eq!(same_moment(&whole, 60.0, 12.5, &part, 60.0), 2_500_000);
        assert_eq!(same_moment(&part, 60.0, 2.5, &whole, 60.0), 12_500_000);
        // Before the part starts: its start.
        assert_eq!(same_moment(&whole, 60.0, 3.0, &part, 60.0), 0);
        // A 30 fps cut against a 60 fps one: the same second.
        let v1 = render(3, "v1-music", 9, Some([150, 299]));
        assert_eq!(same_moment(&whole, 60.0, 7.0, &v1, 30.0), 2_000_000);
        assert_eq!(render_tag(&whole), "show · v28");
        let mut named = whole.clone();
        named.name = Some("avex-1.0-changelog-music".into());
        assert_eq!(render_tag(&named), "avex-1.0-changelog-music · v28");
    }
}
