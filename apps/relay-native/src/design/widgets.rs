//! Controls the selection toolbar and the inspector share: a colour key with its palette, a
//! number field you can type in, step with the arrow keys or scrub by dragging its label, and
//! a segmented choice.

use super::board::{swatch_button, Board};
use crate::app::label;
use gtk::prelude::*;
use gtk::gdk;
use gtk4 as gtk;
use relay_bus::design::parse_color;
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

/// A colour key showing `current`; choosing calls `pick` (`None` clears it back to automatic
/// when `clearable`).
pub fn color_key(
    b: &Rc<Board>,
    current: Option<&str>,
    tooltip: &str,
    clearable: bool,
    pick: impl Fn(Option<String>) + 'static,
) -> gtk::MenuButton {
    let key = gtk::MenuButton::new();
    key.add_css_class("design-color-key");
    key.set_tooltip_text(Some(tooltip));
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
        if clearable {
            let none = gtk::Button::with_label("None");
            none.add_css_class("quiet");
            none.set_tooltip_text(Some("Back to the automatic colour"));
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
/// sideways to scrub. `None` shows an empty field (mixed values). A caption `@name` is a glyph.
pub fn number(caption: &str, value: Option<f64>, tooltip: &str, commit: impl Fn(f64) + 'static) -> gtk::Box {
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
    row.append(&entry);
    let commit = Rc::new(commit);
    let c = commit.clone();
    entry.connect_activate(move |e| {
        if let Some(v) = eval(&e.text()) {
            c(v);
        }
    });
    let keys = gtk::EventControllerKey::new();
    let c = commit.clone();
    let e = entry.downgrade();
    keys.connect_key_pressed(move |_, key, _, mods| {
        let step = if mods.contains(gdk::ModifierType::SHIFT_MASK) { 10.0 } else { 1.0 };
        let dir = match key {
            gdk::Key::Up => 1.0,
            gdk::Key::Down => -1.0,
            _ => return glib::Propagation::Proceed,
        };
        let Some(e) = e.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let v = eval(&e.text()).unwrap_or(0.0) + dir * step;
        e.set_text(&super::board::fmt(v));
        c(v);
        glib::Propagation::Stop
    });
    entry.add_controller(keys);
    let focus = gtk::EventControllerFocus::new();
    let c = commit.clone();
    let e = entry.downgrade();
    let original = value;
    focus.connect_leave(move |_| {
        if let Some(e) = e.upgrade() {
            if let Some(v) = eval(&e.text()).filter(|v| Some(*v) != original) {
                c(v);
            }
        }
    });
    entry.add_controller(focus);
    let scrub = gtk::GestureDrag::new();
    let start = Rc::new(std::cell::Cell::new(0.0));
    let s = start.clone();
    let e = entry.downgrade();
    scrub.connect_drag_begin(move |_, _, _| {
        if let Some(e) = e.upgrade() {
            s.set(eval(&e.text()).unwrap_or(0.0));
        }
    });
    let e = entry.downgrade();
    let s = start.clone();
    scrub.connect_drag_update(move |g, dx, _| {
        let fine = g.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
        let v = (s.get() + (dx * if fine { 0.1 } else { 1.0 })).round();
        if let Some(e) = e.upgrade() {
            e.set_text(&super::board::fmt(v));
        }
    });
    let e = entry.downgrade();
    scrub.connect_drag_end(move |_, dx, _| {
        if dx.abs() < 1.0 {
            return;
        }
        if let Some(v) = e.upgrade().and_then(|e| eval(&e.text())) {
            commit(v);
        }
    });
    cap.add_controller(scrub);
    row
}

/// A number, or simple arithmetic on one (`120+16`, `200/2`), as design tools accept.
pub fn eval(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches("px").trim_end_matches('%').trim();
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
        key.set_tooltip_text(Some(tip));
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
        assert_eq!(eval("1/0"), None);
        assert_eq!(eval("abc"), None);
    }
}
