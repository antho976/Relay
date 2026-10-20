//! Controls the selection toolbar and the inspector share: a colour key with its palette, a
//! number field you can type in, step with the arrow keys or scrub by dragging its label, and
//! a segmented choice.

use super::board::{swatch_button, Board};
use crate::app::label;
use gtk::prelude::*;
use gtk::gdk;
use gtk4 as gtk;
use relay_bus::design::parse_color;
use relay_client::design::number::{eval, fmt};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Neutrals, then hues: the system colours mockups are usually made of, and Relay's own.
const PALETTE: [&str; 24] = [
    "#000000", "#1A1A1A", "#333333", "#666666", "#999999", "#CCCCCC", "#EEEEEE", "#FFFFFF",
    "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#00C7BE", "#32ADE6", "#007AFF", "#5856D6",
    "#AF52DE", "#FF2D55", "#131211", "#EDE9E2", "#2EC469", "#E5382E", "#F0A828", "#4C9DFF",
];

/// The colours this design already uses, most used first: picking one keeps a design to its
/// own palette.
fn document_colors(b: &Board) -> Vec<String> {
    let scene = b.doc.scene.borrow();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for n in scene.to_nodes() {
        for c in [&n.fill, &n.stroke, &n.color].into_iter().flatten() {
            if parse_color(c).is_some_and(|c| c[3] > 0) {
                *counts.entry(c.to_uppercase()).or_default() += 1;
            }
        }
    }
    let mut colors: Vec<(String, usize)> = counts.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    colors.into_iter().take(16).map(|(c, _)| c).collect()
}

/// A colour key showing `current`; choosing calls `pick`. With `clear`, a key so named
/// ("Automatic" for a fill or a text, "None" for a border) picks `None`.
pub fn color_key(
    b: &Rc<Board>,
    current: Option<&str>,
    tooltip: &str,
    clear: Option<&'static str>,
    pick: impl Fn(Option<String>) + 'static,
) -> gtk::MenuButton {
    color_key_named(b, current, tooltip, clear, None, pick)
}

/// The colour `$name` names in the design `b` shows, or `text` itself when it names none.
fn resolved(b: &Board, text: &str) -> String {
    match text.strip_prefix('$') {
        Some(name) => b.doc.scene.borrow().styles().colors.get(name).cloned().unwrap_or_else(|| text.to_string()),
        None => text.to_string(),
    }
}

/// [`color_key`], with the design's named colours first (reuse §4): `current` may be a `$name`,
/// shown as the colour it names; picking a named one gives `$name`; a right click on one offers
/// Change… (its value, for every layer naming it) and Delete (each layer keeps the colour). With
/// `field` (`fill`, `stroke` or `color`), a Name… key names the selection's colour in it.
pub fn color_key_named(
    b: &Rc<Board>,
    current: Option<&str>,
    tooltip: &str,
    clear: Option<&'static str>,
    field: Option<&'static str>,
    pick: impl Fn(Option<String>) + 'static,
) -> gtk::MenuButton {
    let key = gtk::MenuButton::new();
    key.add_css_class("design-color-key");
    tip(&key, &match current.filter(|c| c.starts_with('$')) {
        Some(name) => format!("{tooltip}: {name}"),
        None => tooltip.to_string(),
    });
    let shown = current.map(|c| resolved(b, c));
    let swatch = swatch_button(shown.as_deref(), tooltip);
    swatch.set_can_target(false);
    swatch.set_focusable(false);
    key.set_child(Some(&swatch));
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    key.set_popover(Some(&pop));
    let pick = Rc::new(pick);
    let weak = Rc::downgrade(b);
    let current = current.map(str::to_string);
    pop.connect_show(move |pop| {
        let Some(b) = weak.upgrade() else {
            return;
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.add_css_class("design-colors");
        named_grid(&b, &body, pop, current.as_deref(), pick.clone());
        let doc_colors = document_colors(&b);
        let add_grid = |title: &str, colors: &[String]| {
            if colors.is_empty() {
                return;
            }
            body.append(&label(title, "design-menu-heading"));
            let grid = gtk::FlowBox::new();
            grid.set_max_children_per_line(8);
            grid.set_min_children_per_line(8);
            grid.set_selection_mode(gtk::SelectionMode::None);
            grid.set_column_spacing(4);
            grid.set_row_spacing(4);
            for c in colors {
                let s = swatch_button(Some(c), c);
                // The colour in use, lit and said to be pressed to a screen reader (B106).
                let chosen = current.as_deref().is_some_and(|cur| cur.eq_ignore_ascii_case(c));
                if chosen {
                    s.add_css_class("chosen");
                }
                s.update_state(&[gtk::accessible::State::Pressed(pressed(chosen))]);
                let pick = pick.clone();
                let c = c.clone();
                let p = pop.downgrade();
                s.connect_clicked(move |_| {
                    pick(Some(c.clone()));
                    if let Some(p) = p.upgrade() {
                        p.popdown();
                    }
                });
                grid.insert(&s, -1);
            }
            body.append(&grid);
        };
        add_grid("In this design", &doc_colors);
        add_grid("Colours", &PALETTE.iter().map(|c| c.to_string()).collect::<Vec<_>>());
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("#RRGGBB or #RRGGBBAA"));
        entry.set_text(current.as_deref().unwrap_or(""));
        entry.set_width_chars(12);
        entry.set_hexpand(true);
        entry.add_css_class("design-hex");
        // Marked while what it holds is no colour, until it is changed (E-board-17).
        entry.connect_changed(|e| e.remove_css_class("error"));
        let pick2 = pick.clone();
        let p = pop.downgrade();
        entry.connect_activate(move |e| {
            let mut t = e.text().trim().to_string();
            if !t.starts_with('#') {
                t.insert(0, '#');
            }
            if parse_color(&t).is_some() {
                pick2(Some(t.to_uppercase()));
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
            } else {
                e.add_css_class("error");
            }
        });
        row.append(&entry);
        if let Some(field) = field {
            let name = gtk::Button::with_label("Name…");
            name.add_css_class("quiet");
            name.set_tooltip_text(Some("Name this colour, so every layer using it follows when it changes"));
            let (weak, p) = (Rc::downgrade(&b), pop.downgrade());
            name.connect_clicked(move |key| {
                if let (Some(b), Some(p)) = (weak.upgrade(), p.upgrade()) {
                    p.popdown();
                    ask_name(&b, key, "Name the colour", move |b, name| {
                        let ops = relay_client::design::styles::name_color(&b.doc.scene.borrow(), &b.selected(), field, name);
                        b.commit(ops);
                    });
                }
            });
            row.append(&name);
        }
        if let Some(caption) = clear {
            let none = gtk::Button::with_label(caption);
            none.add_css_class("quiet");
            none.set_tooltip_text(Some(if caption == "None" { "No colour" } else { "Back to the automatic colour" }));
            let pick3 = pick.clone();
            let p = pop.downgrade();
            none.connect_clicked(move |_| {
                pick3(None);
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
            });
            row.append(&none);
        }
        body.append(&row);
        pop.set_child(Some(&body));
    });
    key
}

/// The design's named colours atop the colour popover `pop`: each picks `$name`; a right click
/// changes or deletes it.
fn named_grid(b: &Rc<Board>, body: &gtk::Box, pop: &gtk::Popover, current: Option<&str>, pick: Rc<dyn Fn(Option<String>)>) {
    let named: Vec<(String, String)> = b.doc.scene.borrow().styles().colors.iter().map(|(n, v)| (n.clone(), v.clone())).collect();
    if named.is_empty() {
        return;
    }
    body.append(&label("Named", "design-menu-heading"));
    let grid = gtk::FlowBox::new();
    grid.set_max_children_per_line(8);
    grid.set_min_children_per_line(8);
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    for (name, value) in named {
        let s = swatch_button(Some(&value), &format!("${name} · {value}"));
        let chosen = current == Some(format!("${name}").as_str());
        if chosen {
            s.add_css_class("chosen");
        }
        s.update_state(&[gtk::accessible::State::Pressed(pressed(chosen))]);
        let (pick, n, p) = (pick.clone(), name.clone(), pop.downgrade());
        s.connect_clicked(move |_| {
            pick(Some(format!("${n}")));
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
        });
        let menu = gtk::GestureClick::new();
        menu.set_button(gdk::BUTTON_SECONDARY);
        let (weak, n, p) = (Rc::downgrade(b), name.clone(), pop.downgrade());
        menu.connect_pressed(move |g, _, _, _| {
            let (Some(b), Some(p), Some(at)) = (weak.upgrade(), p.upgrade(), g.widget()) else { return };
            p.popdown();
            named_menu(&b, &at, &n);
        });
        s.add_controller(menu);
        grid.insert(&s, -1);
    }
    body.append(&grid);
}

/// Change… and Delete for named colour `name`, by `at`.
fn named_menu(b: &Rc<Board>, at: &gtk::Widget, name: &str) {
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.set_parent(at);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let users = b.doc.scene.borrow().users(name).len();
    body.append(&label(&format!("${name} · used by {users}"), "design-menu-heading"));
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("#RRGGBB or #RRGGBBAA"));
    entry.set_text(&resolved(b, &format!("${name}")));
    entry.add_css_class("design-hex");
    entry.set_tooltip_text(Some("Change it: every layer naming it follows"));
    let (weak, n, p) = (Rc::downgrade(b), name.to_string(), pop.downgrade());
    entry.connect_activate(move |e| {
        let mut t = e.text().trim().to_string();
        if !t.starts_with('#') {
            t.insert(0, '#');
        }
        match (parse_color(&t), weak.upgrade()) {
            (Some(_), Some(b)) => {
                b.commit(vec![relay_bus::design::NodeOp::Color { name: n.clone(), value: t.to_uppercase() }]);
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
            }
            _ => e.add_css_class("error"),
        }
    });
    body.append(&entry);
    let delete = gtk::Button::with_label("Delete the name");
    delete.add_css_class("quiet");
    delete.set_tooltip_text(Some("Each layer using it keeps its colour"));
    let (weak, n, p) = (Rc::downgrade(b), name.to_string(), pop.downgrade());
    delete.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            b.commit(vec![relay_bus::design::NodeOp::DeleteStyle { name: n.clone() }]);
        }
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
    });
    body.append(&delete);
    pop.set_child(Some(&body));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

/// Ask for a colour's or a style's name by `at`: one the design already has is refused there,
/// since the edit would change that one. `done` takes a name that is free.
pub fn ask_name(b: &Rc<Board>, at: &impl IsA<gtk::Widget>, title: &str, done: impl Fn(&Board, &str) + 'static) {
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    pop.set_parent(at);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&label(title, "design-menu-heading"));
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("accent, card, title…"));
    body.append(&entry);
    let note = label("", "design-note");
    note.set_visible(false);
    body.append(&note);
    let (weak, p) = (Rc::downgrade(b), pop.downgrade());
    entry.connect_activate(move |e| {
        let Some(b) = weak.upgrade() else { return };
        let name = e.text().trim().trim_start_matches('$').to_string();
        let styles = b.doc.scene.borrow().styles().clone();
        let why = if !relay_bus::design::valid_id(&name) {
            Some("Letters, digits, - _ . or :, up to 64")
        } else if styles.colors.contains_key(&name) || styles.styles.contains_key(&name) {
            Some("The design already has that name")
        } else {
            None
        };
        if let Some(why) = why {
            note.set_text(why);
            note.set_visible(true);
            return;
        }
        done(&b, &name);
        if let Some(p) = p.upgrade() {
            p.popdown();
        }
    });
    pop.set_child(Some(&body));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
    entry.grab_focus();
}

/// A number that may be unset (a min or max size): Enter or leaving the field commits what it
/// holds, empty as `None`. `commit` answers what was stored, which it then shows.
pub fn optional_number(caption: &str, value: Option<f64>, tooltip: &str, commit: impl Fn(Option<f64>) -> Option<f64> + 'static) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("design-number");
    row.set_tooltip_text(Some(tooltip));
    let cap = gtk::Label::new(Some(caption));
    cap.add_css_class("design-number-caption");
    row.append(&cap);
    let entry = gtk::Entry::new();
    entry.set_width_chars(4);
    entry.set_max_width_chars(6);
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("—"));
    entry.set_text(&value.map(fmt).unwrap_or_default());
    entry.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    row.append(&entry);
    let last = Rc::new(Cell::new(value));
    let commit: Rc<dyn Fn(Option<f64>) -> Option<f64>> = Rc::new(commit);
    let send = {
        let (last, commit) = (last.clone(), commit.clone());
        Rc::new(move |e: &gtk::Entry| {
            let text = e.text();
            let wanted = if text.trim().is_empty() { Some(None) } else { eval(&text).map(Some) };
            match wanted {
                Some(v) if v != last.get() => {
                    let stored = commit(v);
                    last.set(stored);
                    e.set_text(&stored.map(fmt).unwrap_or_default());
                }
                _ => e.set_text(&last.get().map(fmt).unwrap_or_default()),
            }
        })
    };
    let s = send.clone();
    entry.connect_activate(move |e| s(e));
    let focus = gtk::EventControllerFocus::new();
    let e = entry.downgrade();
    focus.connect_leave(move |_| {
        if let Some(e) = e.upgrade() {
            send(&e);
        }
    });
    entry.add_controller(focus);
    row
}

/// A number field: type and press Enter, step with Up/Down (Shift: 10), or drag the caption
/// sideways to scrub (Shift: finer). Esc puts back what it showed. `None` shows an empty field
/// (mixed values). A caption `@name` is a glyph.
pub fn number(caption: &str, value: Option<f64>, tooltip: &str, commit: impl Fn(f64) + 'static) -> gtk::Box {
    stored_number_step(caption, value, 1.0, tooltip, move |v| {
        commit(v);
        v
    })
}

/// `number`, whose `commit` answers the value it stored, in the field's own units (an opacity
/// typed as 150 is kept as 100, an X of 10.5 as 11): the field shows that after Enter, a step,
/// a scrub or leaving it, not what was typed (B34).
pub fn stored_number(caption: &str, value: Option<f64>, tooltip: &str, commit: impl Fn(f64) -> f64 + 'static) -> gtk::Box {
    stored_number_step(caption, value, 1.0, tooltip, commit)
}

/// What a field shows for the value it last sent.
fn shown(last: &Cell<Option<f64>>) -> String {
    last.get().map(fmt).unwrap_or_default()
}

/// A value without the float noise of adding tenths (1.3 + 0.1 is not 1.4000000000000001).
fn tidy(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Send `v` and keep what `commit` says it stored as the field's value: the text it shows now.
fn settle(commit: &dyn Fn(f64) -> f64, last: &Cell<Option<f64>>, v: f64) -> String {
    let stored = tidy(commit(v));
    last.set(Some(stored));
    fmt(stored)
}

/// What a scrub `dx` pixels from `from` comes to: a step a pixel on the field's steps, with
/// `fine` a tenth of one on tenths. Under a pixel either way is no scrub (B39): a hand that
/// came back, or a touchpad's jitter, leaves the value as it was.
fn scrub_value(from: f64, dx: f64, step: f64, fine: bool) -> Option<f64> {
    if dx.abs() < 1.0 {
        return None;
    }
    let q = if fine { step / 10.0 } else { step };
    Some(tidy(((from + dx * q) / q).round() * q))
}

/// What a field says when what was typed does not come to a number.
const NOT_A_NUMBER: &str = "Not a number: try 24, 120+16 or 50%";

/// Mark the field as holding what is not a number (red, its tooltip saying why, and invalid to
/// a screen reader), or clear the mark back to its own tooltip.
fn mark(row: &gtk::Box, entry: &gtk::Entry, tooltip: &str, bad: bool) {
    if bad {
        row.add_css_class("error");
        row.set_tooltip_text(Some(NOT_A_NUMBER));
    } else {
        row.remove_css_class("error");
        row.set_tooltip_text(Some(tooltip));
    }
    let state = if bad { gtk::AccessibleInvalidState::True } else { gtk::AccessibleInvalidState::False };
    entry.update_state(&[gtk::accessible::State::Invalid(state)]);
}

/// `stored_number`, stepping by `step`: a line height's 0.1, not a whole 1.
pub fn stored_number_step(caption: &str, value: Option<f64>, step: f64, tooltip: &str, commit: impl Fn(f64) -> f64 + 'static) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("design-number");
    row.set_tooltip_text(Some(tooltip));
    // "@glyph" captions with a glyph instead of letters.
    let cap: gtk::Widget = match caption.strip_prefix('@') {
        Some(glyph) => super::glyphs::image(glyph, 14).upcast(),
        None => gtk::Label::new(Some(caption)).upcast(),
    };
    cap.add_css_class("design-number-caption");
    cap.set_cursor_from_name(Some("ew-resize"));
    cap.set_visible(!caption.is_empty());
    row.append(&cap);
    let entry = gtk::Entry::new();
    entry.set_width_chars(4);
    entry.set_max_width_chars(6);
    entry.set_hexpand(true);
    entry.set_text(&value.map(fmt).unwrap_or_default());
    entry.set_placeholder_text(Some("Mixed"));
    entry.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    row.append(&entry);
    // The value last sent (or shown): leaving the field sends only what was typed since,
    // not the field's rounded text of a value it never changed.
    let last = Rc::new(Cell::new(value));
    let commit: Rc<dyn Fn(f64) -> f64> = Rc::new(commit);
    let tooltip: Rc<str> = tooltip.into();
    // What does not come to a number is marked until it is changed.
    let (r, t) = (row.downgrade(), tooltip.clone());
    entry.connect_changed(move |e| {
        if let Some(r) = r.upgrade().filter(|r| r.has_css_class("error")) {
            mark(&r, e, &t, false);
        }
    });
    let (c, l, r) = (commit.clone(), last.clone(), row.downgrade());
    entry.connect_activate(move |e| {
        let text = e.text();
        if text == shown(&l) {
            return;
        }
        match eval(&text) {
            // What it came to as stored, not "120+16" or a value past the field's range.
            Some(v) if Some(v) != l.get() => e.set_text(&settle(&*c, &l, v)),
            Some(v) => e.set_text(&fmt(v)),
            None if text.trim().is_empty() => e.set_text(&shown(&l)),
            None => {
                if let Some(r) = r.upgrade() {
                    mark(&r, e, &tooltip, true);
                }
            }
        }
    });
    let keys = gtk::EventControllerKey::new();
    let (c, e, l) = (commit.clone(), entry.downgrade(), last.clone());
    keys.connect_key_pressed(move |_, key, _, mods| {
        let Some(e) = e.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let dir = match key {
            gdk::Key::Up => 1.0,
            gdk::Key::Down => -1.0,
            // Back to what it showed, so leaving sends nothing; the panel's own Esc then hands
            // the keyboard back to the canvas.
            gdk::Key::Escape => {
                e.set_text(&shown(&l));
                return glib::Propagation::Proceed;
            }
            _ => return glib::Propagation::Proceed,
        };
        // Mixed values have no one value to step from.
        let Some(from) = eval(&e.text()).or(l.get()) else {
            return glib::Propagation::Stop;
        };
        let by = if mods.contains(gdk::ModifierType::SHIFT_MASK) { step * 10.0 } else { step };
        e.set_text(&settle(&*c, &l, tidy(from + dir * by)));
        glib::Propagation::Stop
    });
    entry.add_controller(keys);
    let focus = gtk::EventControllerFocus::new();
    let (c, e, l) = (commit.clone(), entry.downgrade(), last.clone());
    focus.connect_leave(move |_| {
        if let Some(e) = e.upgrade() {
            let shown = shown(&l);
            if e.text() != shown {
                match eval(&e.text()) {
                    Some(v) if Some(v) != l.get() => e.set_text(&settle(&*c, &l, v)),
                    Some(v) => e.set_text(&fmt(v)),
                    // Not a number: nothing is sent, and the field shows its value again.
                    None => e.set_text(&shown),
                }
            }
        }
    });
    entry.add_controller(focus);
    let scrub = gtk::GestureDrag::new();
    // Where the scrub started: the value, and the text the field showed then.
    let start: Rc<RefCell<Option<(f64, String)>>> = Rc::default();
    let (s, e) = (start.clone(), entry.downgrade());
    scrub.connect_drag_begin(move |_, _, _| {
        if let Some(e) = e.upgrade() {
            *s.borrow_mut() = eval(&e.text()).map(|v| (v, e.text().to_string()));
        }
    });
    let (s, e) = (start.clone(), entry.downgrade());
    scrub.connect_drag_update(move |g, dx, _| {
        let fine = g.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
        let (Some((from, text)), Some(e)) = (s.borrow().clone(), e.upgrade()) else {
            return;
        };
        // A preview only: a scrub back under a pixel shows what it started from again.
        e.set_text(&scrub_value(from, dx, step, fine).map_or(text, fmt));
    });
    let (e, l) = (entry.downgrade(), last.clone());
    scrub.connect_drag_end(move |g, dx, _| {
        let fine = g.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
        let (Some((from, text)), Some(e)) = (start.take(), e.upgrade()) else {
            return;
        };
        // From where it started and how far it went, not the preview's rounded text.
        match scrub_value(from, dx, step, fine) {
            None => e.set_text(&text),
            Some(v) if Some(v) == l.get() => e.set_text(&fmt(v)),
            Some(v) => e.set_text(&settle(&*commit, &l, v)),
        }
    });
    cap.add_controller(scrub);
    row
}

/// A key's tooltip, and its name for a screen reader: the tooltip less a shortcut or an aside
/// in brackets at its end ("Lock (Ctrl+Shift+L)" is named "Lock").
pub fn tip(widget: &impl IsA<gtk::Widget>, text: &str) {
    widget.set_tooltip_text(Some(text));
    let name = text.rsplit_once(" (").filter(|(_, rest)| rest.ends_with(')')).map_or(text, |(name, _)| name);
    widget.as_ref().update_property(&[gtk::accessible::Property::Label(name)]);
}

/// A key's pressed state as a screen reader reads it.
fn pressed(on: bool) -> gtk::AccessibleTristate {
    if on {
        gtk::AccessibleTristate::True
    } else {
        gtk::AccessibleTristate::False
    }
}

/// A row of glyph keys of which one is lit.
pub fn segmented(choices: &[(&str, &str, &str)], current: Option<&str>, pick: impl Fn(&str) + 'static) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("design-segmented");
    let pick = Rc::new(pick);
    for (value, glyph, tip) in choices {
        let key = gtk::Button::new();
        key.add_css_class("quiet");
        // A glyph when the set has one by that name; otherwise the word itself.
        if super::glyphs::known(glyph) {
            key.set_child(Some(&super::glyphs::image(glyph, 16)));
        } else {
            key.set_label(glyph);
        }
        self::tip(&key, tip);
        // The key lit is the one pressed, to a screen reader too (B106).
        let on = current == Some(*value);
        if on {
            key.add_css_class("selected");
        }
        key.update_state(&[gtk::accessible::State::Pressed(pressed(on))]);
        let pick = pick.clone();
        let value = value.to_string();
        key.connect_clicked(move |_| pick(&value));
        row.append(&key);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::{scrub_value, settle};
    use std::cell::Cell;

    #[test]
    fn a_field_shows_what_was_stored() {
        // An opacity typed as 150 is kept as 100, and the field says so (B34).
        let clamp = |v: f64| v.clamp(0.0, 100.0);
        let last = Cell::new(Some(40.0));
        assert_eq!(settle(&clamp, &last, 150.0), "100");
        assert_eq!(last.get(), Some(100.0));
        // A step down from there goes from what was stored, not from 150.
        assert_eq!(settle(&clamp, &last, last.get().unwrap() - 1.0), "99");
        assert_eq!(last.get(), Some(99.0));
        // An X of 10.5 is placed at 11.
        let round = |v: f64| v.round();
        assert_eq!(settle(&round, &last, 10.5), "11");
        // Tenths stay tenths.
        assert_eq!(settle(&|v| v, &last, 1.3 + 0.1), "1.4");
        assert_eq!(last.get(), Some(1.4));
    }

    #[test]
    fn a_scrub_under_a_pixel_is_none() {
        // A hand that came back, or a touchpad's jitter, scrubs nothing (B39).
        assert_eq!(scrub_value(12.0, 0.6, 1.0, false), None);
        assert_eq!(scrub_value(12.0, -0.9, 1.0, true), None);
        assert_eq!(scrub_value(12.0, 5.0, 1.0, false), Some(17.0));
        assert_eq!(scrub_value(12.0, -5.0, 1.0, false), Some(7.0));
        assert_eq!(scrub_value(12.0, 3.0, 1.0, true), Some(12.3));
        assert_eq!(scrub_value(1.3, 1.0, 0.1, false), Some(1.4));
        assert_eq!(scrub_value(12.0, 1.4, 1.0, false), Some(13.0));
    }
}
