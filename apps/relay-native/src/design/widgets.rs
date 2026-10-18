//! Controls the selection toolbar and the inspector share: a colour key with its palette, a
//! number field you can type in, step with the arrow keys or scrub by dragging its label, and
//! a segmented choice.

use super::board::{swatch_button, Board};
use crate::app::label;
use gtk::prelude::*;
use gtk::gdk;
use gtk4 as gtk;
use relay_bus::design::parse_color;
use std::cell::Cell;
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
    let key = gtk::MenuButton::new();
    key.add_css_class("design-color-key");
    tip(&key, tooltip);
    let swatch = swatch_button(current, tooltip);
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
                if current.as_deref().is_some_and(|cur| cur.eq_ignore_ascii_case(c)) {
                    s.add_css_class("chosen");
                }
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

/// A number field: type and press Enter, step with Up/Down (Shift: 10), or drag the caption
/// sideways to scrub (Shift: finer). Esc puts back what it showed. `None` shows an empty field
/// (mixed values). A caption `@name` is a glyph.
pub fn number(caption: &str, value: Option<f64>, tooltip: &str, commit: impl Fn(f64) + 'static) -> gtk::Box {
    number_step(caption, value, 1.0, tooltip, commit)
}

/// What a field shows for the value it last sent.
fn shown(last: &Cell<Option<f64>>) -> String {
    last.get().map(super::board::fmt).unwrap_or_default()
}

/// A value without the float noise of adding tenths (1.3 + 0.1 is not 1.4000000000000001).
fn tidy(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// `number`, stepping by `step`: a line height's 0.1, not a whole 1.
pub fn number_step(caption: &str, value: Option<f64>, step: f64, tooltip: &str, commit: impl Fn(f64) + 'static) -> gtk::Box {
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
    entry.set_text(&value.map(super::board::fmt).unwrap_or_default());
    entry.set_placeholder_text(Some("Mixed"));
    entry.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    row.append(&entry);
    // The value last sent (or shown): leaving the field sends only what was typed since,
    // not the field's rounded text of a value it never changed.
    let last = Rc::new(Cell::new(value));
    let commit: Rc<dyn Fn(f64)> = {
        let last = last.clone();
        Rc::new(move |v: f64| {
            last.set(Some(v));
            commit(v);
        })
    };
    // What does not come to a number is marked until it is changed.
    let r = row.downgrade();
    entry.connect_changed(move |_| {
        if let Some(r) = r.upgrade() {
            r.remove_css_class("error");
        }
    });
    let c = commit.clone();
    let l = last.clone();
    let r = row.downgrade();
    entry.connect_activate(move |e| {
        let text = e.text();
        if text == shown(&l) {
            return;
        }
        match eval(&text) {
            Some(v) => {
                if Some(v) != l.get() {
                    c(v);
                }
                // What it came to, not "120+16".
                e.set_text(&super::board::fmt(v));
            }
            None if text.trim().is_empty() => e.set_text(&shown(&l)),
            None => {
                if let Some(r) = r.upgrade() {
                    r.add_css_class("error");
                }
            }
        }
    });
    let keys = gtk::EventControllerKey::new();
    let c = commit.clone();
    let e = entry.downgrade();
    let l = last.clone();
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
        let v = tidy(from + dir * by);
        e.set_text(&super::board::fmt(v));
        c(v);
        glib::Propagation::Stop
    });
    entry.add_controller(keys);
    let focus = gtk::EventControllerFocus::new();
    let c = commit.clone();
    let e = entry.downgrade();
    let l = last.clone();
    focus.connect_leave(move |_| {
        if let Some(e) = e.upgrade() {
            let shown = shown(&l);
            if e.text() != shown {
                match eval(&e.text()) {
                    Some(v) => {
                        if Some(v) != l.get() {
                            c(v);
                        }
                        e.set_text(&super::board::fmt(v));
                    }
                    // Not a number: nothing is sent, and the field shows its value again.
                    None => e.set_text(&shown),
                }
            }
        }
    });
    entry.add_controller(focus);
    let scrub = gtk::GestureDrag::new();
    let start = Rc::new(Cell::new(None::<f64>));
    let s = start.clone();
    let e = entry.downgrade();
    scrub.connect_drag_begin(move |_, _, _| {
        if let Some(e) = e.upgrade() {
            s.set(eval(&e.text()));
        }
    });
    let e = entry.downgrade();
    let s = start.clone();
    scrub.connect_drag_update(move |g, dx, _| {
        let fine = g.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
        let Some(from) = s.get() else {
            return;
        };
        // A step a pixel, on the field's steps; with Shift a tenth of one, on tenths.
        let q = if fine { step / 10.0 } else { step };
        let v = tidy(((from + dx * q) / q).round() * q);
        if let Some(e) = e.upgrade() {
            e.set_text(&super::board::fmt(v));
        }
    });
    let e = entry.downgrade();
    scrub.connect_drag_end(move |_, dx, _| {
        if dx.abs() < 1.0 || start.get().is_none() {
            return;
        }
        if let Some(v) = e.upgrade().and_then(|e| eval(&e.text())) {
            commit(v);
        }
    });
    cap.add_controller(scrub);
    row
}

/// A number, or simple arithmetic on one (`120+16`, `200/2`), as design tools accept; a unit
/// typed after it (`24px`, `50%`, `45°`) is let be.
pub fn eval(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches("px").trim_end_matches('%').trim_end_matches('°').trim();
    if let Ok(v) = t.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    for op in ['+', '*', '/', '-'] {
        if let Some((i, _)) = t.char_indices().skip(1).find(|(_, c)| *c == op) {
            let (a, b) = (t[..i].trim().parse::<f64>().ok()?, t[i + 1..].trim().parse::<f64>().ok()?);
            let v = match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                _ if b != 0.0 => a / b,
                _ => return None,
            };
            return v.is_finite().then_some(v);
        }
    }
    None
}

/// A key's tooltip, and its name for a screen reader: the tooltip less a shortcut or an aside
/// in brackets at its end ("Lock (Ctrl+Shift+L)" is named "Lock").
pub fn tip(widget: &impl IsA<gtk::Widget>, text: &str) {
    widget.set_tooltip_text(Some(text));
    let name = text.rsplit_once(" (").filter(|(_, rest)| rest.ends_with(')')).map_or(text, |(name, _)| name);
    widget.as_ref().update_property(&[gtk::accessible::Property::Label(name)]);
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
        if current == Some(*value) {
            key.add_css_class("selected");
        }
        let pick = pick.clone();
        let value = value.to_string();
        key.connect_clicked(move |_| pick(&value));
        row.append(&key);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::eval;

    #[test]
    fn fields_take_arithmetic() {
        assert_eq!(eval("12"), Some(12.0));
        assert_eq!(eval("120+16"), Some(136.0));
        assert_eq!(eval("200/2"), Some(100.0));
        assert_eq!(eval("-8"), Some(-8.0));
        assert_eq!(eval("10-4"), Some(6.0));
        assert_eq!(eval("24px"), Some(24.0));
        assert_eq!(eval("45°"), Some(45.0));
        assert_eq!(eval("1/0"), None);
        assert_eq!(eval("abc"), None);
    }
}
