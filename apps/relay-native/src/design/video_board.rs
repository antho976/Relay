//! One video, opened from the gallery (docs/VIDEO.md): its newest render playing, the props its
//! composition declares to change without code, its renders and versions, and Ask, which hands
//! the moment on screen to the video's terminal.
//!
//! Agents write a video; the person watches it, changes what its props allow, renders it and
//! asks for the rest. Applying props renders a preview, so what plays follows what was set. A
//! render that is older than the code says so, with the key to render it again.

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
    poster: gtk::Picture,
    stage_note: gtk::Label,
    play_key: gtk::Button,
    preview_key: gtk::Button,
    stale: gtk::Box,
    stale_note: gtk::Label,
    progress: gtk::Box,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    /// The render playing, and its file.
    playing: RefCell<Option<(i64, String)>>,
    composition: gtk::DropDown,
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
        stage.add_named(&player, Some("player"));
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
        // Its rows expand within it; it does not take the player's room.
        side.set_hexpand(false);
        let side_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (comp_section, comp_body) = section("Composition");
        let composition = gtk::DropDown::from_strings(&[]);
        composition.add_css_class("design-dropdown");
        comp_body.append(&composition);
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
            composition: composition.clone(),
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
            let newest = b.detail.borrow().as_ref().and_then(|d| d.renders.iter().find(|r| playable(r)).cloned());
            if let Some(r) = newest {
                b.play(r.id, r.path.unwrap_or_default());
            }
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
        composition.connect_selected_notify(move |d| {
            let Some(b) = weak.upgrade() else { return };
            if b.choosing.get() {
                return;
            }
            let chosen = b.compositions.borrow().get(d.selected() as usize).cloned();
            if let Some(chosen) = chosen {
                b.call("video.update", json!({"video_id": b.id, "composition": chosen}), None);
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

    fn composition_of(&self) -> Option<Composition> {
        let d = self.detail.borrow();
        let d = d.as_ref()?;
        d.compositions.iter().find(|c| c.id == d.video.composition).cloned()
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
                    // The video smoke's capture of the player (scripts/video-smoke.py).
                    if std::env::var_os("RELAY_SMOKE_VIDEO_PLAY").is_some() && b.play_key.is_visible() {
                        b.play_key.emit_clicked();
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

        // The compositions.
        let ids: Vec<String> = detail.compositions.iter().map(|c| c.id.clone()).collect();
        if *self.compositions.borrow() != ids {
            self.choosing.set(true);
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            self.composition.set_model(Some(&gtk::StringList::new(&names)));
            *self.compositions.borrow_mut() = ids.clone();
            self.choosing.set(false);
        }
        if let Some(at) = ids.iter().position(|c| *c == v.composition) {
            if self.composition.selected() as usize != at {
                self.choosing.set(true);
                self.composition.set_selected(at as u32);
                self.choosing.set(false);
            }
        }
        self.composition.set_visible(ids.len() > 1);
        self.about.set_text(&match self.composition_of() {
            Some(c) => format!("{} · {} × {} · {} fps · {}", c.id, c.width, c.height, c.fps, super::videos::clock(c.duration_frames, c.fps)),
            None => "Not checked yet: an agent's video.check lists the compositions.".into(),
        });

        self.show_props(&detail);
        self.show_stage(&detail);
        self.show_progress();
        self.show_renders(&detail);
        self.show_versions(&detail);
    }

    fn show_stage(&self, detail: &GetOut) {
        let v = &detail.video;
        let newest = detail.renders.iter().find(|r| playable(r));
        if let Some(path) = &v.poster {
            self.poster.set_filename(Some(path));
        }
        let in_player = self.stage.visible_child_name().as_deref() == Some("player");
        match newest {
            Some(r) => {
                let path = r.path.clone().unwrap_or_default();
                // Playing already: the newer render takes the old one's place, paused.
                let same = self.playing.borrow().as_ref().is_some_and(|(id, p)| *id == r.id && *p == path);
                if in_player && !same {
                    self.player.set_media_stream(Some(&gtk::MediaFile::for_filename(&path)));
                    *self.playing.borrow_mut() = Some((r.id, path));
                }
                if !in_player {
                    self.stage.set_visible_child_name("poster");
                }
                self.play_key.set_visible(true);
                self.preview_key.set_visible(false);
                let size = r.bytes.map(megabytes).unwrap_or_default();
                self.stage_note.set_text(&format!("{} {} of v{} · {size}", r.format.name().to_uppercase(),
                    if r.quality == relay_bus::ops::video::Quality::Preview { "preview" } else { "render" }, r.version));
                let older = r.version < v.version;
                self.stale.set_visible(older);
                self.stale_note.set_text(&format!("This render is of v{}; the code is at v{}.", r.version, v.version));
            }
            None => {
                self.stale.set_visible(false);
                self.stage.set_visible_child_name("poster");
                self.play_key.set_visible(false);
                self.preview_key.set_visible(true);
                self.stage_note.set_text(if v.poster.is_some() {
                    "No render to play yet. A preview renders it at half size, in seconds."
                } else {
                    "Nothing drawn yet. Once an agent has written the video, render a preview to watch it."
                });
            }
        }
    }

    /// Play render `id`'s file in the player.
    fn play(&self, id: i64, path: String) {
        let same = self.playing.borrow().as_ref().is_some_and(|(playing, p)| *playing == id && *p == path);
        if !same {
            self.player.set_media_stream(Some(&gtk::MediaFile::for_filename(&path)));
            *self.playing.borrow_mut() = Some((id, path));
        }
        self.stage.set_visible_child_name("player");
        if let Some(stream) = self.player.media_stream() {
            stream.play();
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
        let comp = detail.compositions.iter().find(|c| c.id == detail.video.composition);
        let sig = json!([
            detail.video.composition,
            comp.map(|c| (&c.props_schema, &c.default_props)),
            detail.props.get(&detail.video.composition),
        ])
        .to_string();
        if self.props_sig.borrow().as_deref() == Some(sig.as_str()) {
            return;
        }
        *self.props_sig.borrow_mut() = Some(sig);
        clear(&self.props_box);
        self.fields.borrow_mut().clear();
        let Some(comp) = detail.compositions.iter().find(|c| c.id == detail.video.composition) else {
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
        for r in detail.renders.iter().take(8) {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("video-row");
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
            words.append(&head);
            let when = r.finished_at.as_deref().or(Some(r.created_at.as_str())).and_then(|t| ago(t, Form::CompactAgo)).unwrap_or_default();
            let detail_text = match r.state {
                RenderState::Done => format!("{} · {}", r.bytes.map(megabytes).unwrap_or_default(), when),
                RenderState::Failed => format!("failed: {}", r.error.as_deref().unwrap_or_default()),
                RenderState::Cancelled => format!("stopped {when}"),
                RenderState::Queued => "queued".into(),
                RenderState::Rendering => format!("{}%", (r.progress * 100.0).round()),
            };
            let sub = label(&detail_text, "dim");
            sub.set_xalign(0.0);
            sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
            sub.set_tooltip_text(r.error.as_deref().or(r.path.as_deref()));
            words.append(&sub);
            // Its sound as measured: loudness and peak, red when it clips.
            if let Some(audio) = r.audio.as_ref().filter(|_| r.state == RenderState::Done) {
                let line = sound_line(audio);
                let warn = audio.warnings.iter().any(|w| w.kind == "clipping" || w.kind == "hot");
                let sound = label(&line, if warn { "video-warn" } else { "dim" });
                sound.set_xalign(0.0);
                sound.set_ellipsize(gtk::pango::EllipsizeMode::End);
                let tip = audio.warnings.iter().map(|w| w.detail.as_str()).collect::<Vec<_>>().join("\n");
                if !tip.is_empty() {
                    sound.set_tooltip_text(Some(&tip));
                }
                words.append(&sound);
            }
            row.append(&words);
            if let (true, Some(path)) = (playable(r), r.path.clone()) {
                let play = crate::app::icon_button("play", "Play this render");
                let (weak, id) = (self.me.clone(), r.id);
                let file = path.clone();
                play.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.play(id, file.clone());
                    }
                });
                row.append(&play);
            }
            if let Some(path) = r.path.clone().filter(|_| r.state == RenderState::Done) {
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

    fn show_versions(&self, detail: &GetOut) {
        clear(&self.versions_box);
        if detail.versions.is_empty() {
            self.versions_box.append(&label("Recorded as the code is checked, looked at and rendered.", "dim"));
            return;
        }
        for (i, version) in detail.versions.iter().take(10).enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("video-row");
            let words = label(&format!("v{} · {} · {}", version.n, version.actor, ago(&version.created_at, Form::CompactAgo).unwrap_or_default()), "");
            words.set_xalign(0.0);
            words.set_hexpand(true);
            row.append(&words);
            if i == 0 {
                row.append(&label("now", "dim"));
            } else {
                let restore = gtk::Button::with_label("Restore");
                restore.add_css_class("quiet");
                restore.set_tooltip_text(Some("Put the code back as this version had it; the code as it is now is kept as a version first"));
                let (weak, n) = (self.me.clone(), version.n);
                restore.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.call("video.version.restore", json!({"video_id": b.id, "version": n}), Some(format!("Restored v{n}; the code before is kept as a version.")));
                    }
                });
                row.append(&restore);
            }
            self.versions_box.append(&row);
        }
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
            b.call("video.delete", json!({"video_id": b.id}), None);
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

    fn render(&self, mut payload: Value) {
        payload["video_id"] = json!(self.id);
        self.call("video.render", payload, Some("Rendering; it plays here when it is done.".into()));
    }

    fn apply_props(&self, reset: bool) {
        let Some(comp) = self.video().map(|v| v.composition) else { return };
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

    /// The moment on screen: the frame the player shows, at the composition's rate.
    fn moment(&self) -> Option<(u32, f64)> {
        let fps = self.composition_of().map(|c| c.fps).or_else(|| self.video().map(|v| v.fps))?;
        let stream = self.player.media_stream().filter(|_| self.stage.visible_child_name().as_deref() == Some("player"))?;
        let seconds = stream.timestamp() as f64 / 1e6;
        Some(((seconds * fps).round() as u32, fps))
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
        let heading = match at {
            Some((frame, fps)) => format!("Ask {session} about {}", frame_label(frame, fps)),
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
            b.send(session.clone(), request, at);
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

    /// The prompt an Ask sends: one line naming the video, the moment and the request, with the
    /// call that shows the agent that frame.
    fn prompt(v: &Video, request: &str, at: Option<(u32, f64)>) -> String {
        let request = request.replace(['\n', '\r'], " ");
        match at {
            Some((frame, fps)) => format!(
                "[Relay video] \"{}\" (video_id {}, composition {}) at {}: {request} — see that frame with video.still {{\"video_id\": {}, \"at\": [\"f{frame}\"]}}; its code is in {}",
                v.title, v.id, v.composition, frame_label(frame, fps), v.id, v.dir,
            ),
            None => format!(
                "[Relay video] \"{}\" (video_id {}, composition {}): {request} — look with video.still {{\"video_id\": {}}}; its code is in {}",
                v.title, v.id, v.composition, v.id, v.dir,
            ),
        }
    }

    fn send(&self, session: String, request: String, at: Option<(u32, f64)>) {
        let (Some(ui), Some(v)) = (self.ui.upgrade(), self.video()) else { return };
        let message = Self::prompt(&v, &request, at);
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

    #[test]
    fn an_ask_names_the_video_the_moment_and_how_to_see_it() {
        let v = Video {
            id: 4, project_id: 1, session: Some("snug-zebra".into()), title: "Promo".into(), dir: "/v/p1/4-promo".into(),
            composition: "main".into(), width: 1080, height: 1920, fps: 30.0, duration_frames: 240, version: 3, poster: None,
            last_render: None, created_by: "user".into(), updated_by: "user".into(), created_at: String::new(), updated_at: String::new(), deleted_at: None,
        };
        let line = VideoBoard::prompt(&v, "make the title\nbigger", Some((45, 30.0)));
        assert_eq!(line, "[Relay video] \"Promo\" (video_id 4, composition main) at 1.5s · f45: make the title bigger — see that frame with video.still {\"video_id\": 4, \"at\": [\"f45\"]}; its code is in /v/p1/4-promo");
        assert!(!line.contains('\n'));
    }
}
