//! The toolbar that floats over the selection, as Claude Design's does over a picked element:
//! the few things you change most for what is selected — a text's style, size, colour and
//! emphasis; a shape's fill, stroke and corners; a stroke's colour and weight; several
//! layers' alignment — and Ask, which hands the selection to the terminal's agent.

use super::board::{display_name, Board};
use super::paint::STICKY_FILL;
use super::glyphs;
use super::tools::STICKY_COLORS;
use super::widgets::{color_key, stored_number};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Layout, NodeKind, TextAlign};
use relay_client::design::edit::{self, Align};
use relay_client::design::shapes;
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// Text styles, Figma's type ramp in miniature: name, size, weight, line height, tracking.
pub const TEXT_STYLES: [(&str, f64, u16, f64, f64); 8] = [
    ("Display", 56.0, 700, 1.1, -1.4),
    ("Title", 40.0, 700, 1.15, -0.8),
    ("Heading", 28.0, 600, 1.2, -0.3),
    ("Subheading", 20.0, 600, 1.3, 0.0),
    ("Body", 16.0, 400, 1.5, 0.0),
    ("Small", 14.0, 400, 1.45, 0.0),
    ("Caption", 12.0, 500, 1.35, 0.1),
    ("Code", 13.0, 400, 1.5, 0.0),
];

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

fn same<T: PartialEq + Clone>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut it = values.into_iter();
    let first = it.next()?;
    it.all(|v| v == first).then_some(first)
}

impl Board {
    pub(super) fn rebuild_context(&self) {
        let old = self.context.borrow().clone();
        if let Some(old) = old {
            self.release(&old);
        }
        if let Some(old) = self.context.borrow_mut().take() {
            self.overlay.remove_overlay(&old);
        }
        self.context_values.borrow_mut().clear();
        let ids = self.selected();
        if ids.is_empty() {
            return;
        }
        let b = self.me();
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        bar.add_css_class("design-context");
        bar.set_widget_name("design-context");
        let scene = self.doc.scene.borrow();
        let nodes: Vec<_> = ids.iter().filter_map(|id| scene.node(id).cloned()).collect();
        if nodes.is_empty() {
            return;
        }
        // Up one level: the deep click selects what is under the pointer; this reaches its
        // container.
        if let [one] = nodes.as_slice() {
            if let Some(parent) = one.parent.as_deref().and_then(|p| scene.node(p)).filter(|p| p.kind != NodeKind::Page) {
                let up = gtk::Button::new();
                up.add_css_class("design-context-key");
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                row.append(&crate::icons::image("arrow-up", 12));
                let name = label(&display_name(parent), "design-crumb-label");
                name.add_css_class("design-context-word");
                name.set_max_width_chars(14);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                row.append(&name);
                up.set_child(Some(&row));
                super::widgets::tip(&up, &format!("Select {} (Esc)", display_name(parent)));
                let pid = parent.id.clone();
                let weak = Rc::downgrade(&b);
                up.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.select(vec![pid.clone()]);
                    }
                });
                bar.append(&up);
                bar.append(&sep());
            }
        }
        let all = |k: &[NodeKind]| nodes.iter().all(|n| k.contains(&n.kind));
        let kind;
        if all(&[NodeKind::Icon]) {
            kind = "icon";
            // Automatic shows the colour it is drawn in.
            let colour = {
                let scene = b.doc.scene.borrow();
                same(nodes.iter().map(|n| n.stroke.clone().unwrap_or_else(|| super::paint::text_color(&scene, n))))
            };
            let weak = Rc::downgrade(&b);
            bar.append(&color_key(&b, colour.as_deref(), "Icon colour", Some("Automatic"), move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke", json!(c))]));
                }
            }));
            if let [one] = nodes.as_slice() {
                let swap = gtk::MenuButton::new();
                swap.add_css_class("design-context-key");
                swap.set_child(Some(&super::icons::image(one.icon.as_deref().unwrap_or(""), 16)));
                super::widgets::tip(&swap, "Another icon");
                let pop = gtk::Popover::new();
                pop.add_css_class("design-menu");
                swap.set_popover(Some(&pop));
                let weak = Rc::downgrade(&b);
                let id = one.id.clone();
                pop.connect_show(move |pop| {
                    if let Some(b) = weak.upgrade() {
                        pop.set_child(Some(&super::icons::picker(&b, pop, Some(id.clone()))));
                    }
                });
                bar.append(&swap);
            }
        } else if all(&[NodeKind::Text, NodeKind::Sticky]) {
            kind = "text";
            self.text_controls(&b, &bar, &nodes);
        } else if all(&[NodeKind::Line, NodeKind::Path]) {
            kind = "stroke";
            // Closed, a free line is a shape of its own: no ends to route or put arrowheads on
            // (routing it would open it and drop the fill). One of three points or more fills,
            // closed or not (B98).
            let closed = nodes.iter().all(shapes::closed_shape);
            if nodes.iter().all(shapes::fillable_line) {
                let fill = same(nodes.iter().map(super::paint::fill_of)).flatten();
                let weak = Rc::downgrade(&b);
                bar.append(&color_key(&b, fill.as_deref(), "Fill", Some("None"), move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fill", json!(c)), ("gradient", Value::Null)]));
                    }
                }));
            }
            let stroke = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
            let weak = Rc::downgrade(&b);
            bar.append(&color_key(&b, stroke.as_deref(), "Stroke colour", None, move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke", json!(c))]));
                }
            }));
            let width = same(nodes.iter().map(|n| n.stroke_width.unwrap_or(2.0).to_bits())).map(f64::from_bits);
            let weak = Rc::downgrade(&b);
            // The inspector's weight: its glyph, 0 to 1000 (E-board-6), and what was stored
            // shown after it (B34).
            bar.append(&stored_number("@stroke-width", width, "Stroke weight", move |v| {
                let v = v.clamp(0.0, 1000.0);
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_width", json!(v))]));
                }
                v
            }));
            if all(&[NodeKind::Line]) && !closed {
                // Straight, elbow, curve: one key that steps through them.
                let route = same(nodes.iter().map(|n| n.route.unwrap_or(relay_bus::design::Route::Straight)))
                    .unwrap_or(relay_bus::design::Route::Straight);
                let (glyph, next, tip) = match route {
                    relay_bus::design::Route::Straight => ("line", "elbow", "Straight — make it an elbow"),
                    relay_bus::design::Route::Elbow => ("connector", "curve", "Elbow — make it a curve"),
                    relay_bus::design::Route::Curve => ("curve", "straight", "Curve — make it straight"),
                };
                let key = gtk::Button::new();
                key.add_css_class("design-context-key");
                key.set_child(Some(&glyphs::image(glyph, 16)));
                super::widgets::tip(&key, tip);
                let weak = Rc::downgrade(&b);
                key.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("route", if next == "straight" { Value::Null } else { json!(next) })]));
                    }
                });
                bar.append(&key);
            }
            // A pen path draws its arrowheads as a line does, so it gets the key too (B99).
            if !closed {
                let on = nodes.iter().all(|n| n.arrow_end == Some(true));
                let arrow = gtk::ToggleButton::new();
                arrow.add_css_class("design-context-key");
                arrow.set_child(Some(&glyphs::image("arrow", 16)));
                super::widgets::tip(&arrow, "Arrowhead");
                arrow.set_active(on);
                let weak = Rc::downgrade(&b);
                arrow.connect_toggled(move |k| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("arrow_end", if k.is_active() { json!(true) } else { Value::Null })]));
                    }
                });
                bar.append(&arrow);
            }
        } else if nodes.len() == 1 || all(&[NodeKind::Frame, NodeKind::Rect, NodeKind::Ellipse, NodeKind::Image]) {
            kind = "shape";
            // Only the keys every selected kind takes (FIELDS).
            let takes = |field: &str| nodes.iter().all(|n| relay_bus::design::takes(n.kind, field));
            if takes("fill") {
                let fill = same(nodes.iter().map(super::paint::fill_of)).flatten();
                let weak = Rc::downgrade(&b);
                bar.append(&color_key(&b, fill.as_deref(), "Fill", Some("Automatic"), move |c| {
                    if let Some(b) = weak.upgrade() {
                        // A gradient would cover the colour picked (TM-47).
                        b.set_selected(set(&[("fill", json!(c)), ("gradient", Value::Null)]));
                    }
                }));
            }
            if takes("stroke") {
                let stroke = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
                let weak = Rc::downgrade(&b);
                let key = color_key(&b, stroke.as_deref(), "Border", Some("None"), move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("stroke", json!(c))]));
                    }
                });
                key.add_css_class("design-stroke-key");
                bar.append(&key);
            }
            if takes("radius") {
                // The radius it shows: per-corner radii's largest, a sticky's own 6 (TM-47).
                let radius = same(nodes.iter().map(|n| shapes::radius(n).to_bits())).map(f64::from_bits);
                let weak = Rc::downgrade(&b);
                let field = stored_number("@radius", radius, "Corner radius", move |v| {
                    let v = v.max(0.0);
                    if let Some(b) = weak.upgrade() {
                        let ops = shapes::radius_ops(&b.doc.scene.borrow(), &b.selected(), v);
                        b.commit(ops);
                    }
                    v
                });
                bar.append(&field);
            }
            if let [one] = nodes.as_slice() {
                if one.kind == NodeKind::Frame {
                    let mode = one.layout.unwrap_or(Layout::None);
                    let key = gtk::Button::new();
                    key.add_css_class("design-context-key");
                    let (glyph, tip, next) = layout_key(mode);
                    key.set_child(Some(&glyphs::image(glyph, 16)));
                    super::widgets::tip(&key, tip);
                    let weak = Rc::downgrade(&b);
                    key.connect_clicked(move |_| {
                        if let Some(b) = weak.upgrade() {
                            b.set_layout(next);
                        }
                    });
                    bar.append(&key);
                }
            }
            // A shape's label gets the text keys too, once it has words or is being typed
            // (E-text-6).
            let editing = self.edit.borrow().as_ref().map(|e| e.id.clone());
            let labelled = |n: &relay_bus::design::Node| {
                n.text.as_deref().is_some_and(relay_client::design::names::visible) || editing.as_deref() == Some(n.id.as_str())
            };
            if takes("font_size") && nodes.iter().all(labelled) {
                bar.append(&sep());
                self.text_controls(&b, &bar, &nodes);
            }
        } else {
            kind = "align";
            for (how, glyph, tip) in [
                (Align::Left, "align-left", "Align left (Alt+A)"),
                (Align::CenterX, "align-center-x", "Align centres (Alt+H)"),
                (Align::Right, "align-right", "Align right (Alt+D)"),
                (Align::Top, "align-top", "Align top (Alt+W)"),
                (Align::CenterY, "align-center-y", "Align middles (Alt+V)"),
                (Align::Bottom, "align-bottom", "Align bottom (Alt+S)"),
            ] {
                bar.append(&self.align_key(&b, how, glyph, tip));
            }
            if nodes.len() > 2 {
                bar.append(&self.align_key(&b, Align::DistributeX, "distribute-x", "Distribute horizontally"));
                bar.append(&self.align_key(&b, Align::DistributeY, "distribute-y", "Distribute vertically"));
            }
            bar.append(&sep());
            let group = gtk::Button::new();
            group.add_css_class("design-context-key");
            group.set_child(Some(&glyphs::image("group", 16)));
            super::widgets::tip(&group, "Group (Ctrl+G)");
            group.set_action_name(Some("design.group"));
            bar.append(&group);
            let frame = gtk::Button::new();
            frame.add_css_class("design-context-key");
            frame.set_child(Some(&glyphs::image("frame", 16)));
            super::widgets::tip(&frame, "Frame selection (Ctrl+Alt+G)");
            frame.set_action_name(Some("design.frame"));
            bar.append(&frame);
        }
        drop(scene);
        bar.append(&sep());
        let ask = gtk::Button::new();
        ask.add_css_class("design-context-key");
        ask.add_css_class("design-context-ask");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        row.append(&glyphs::image("spark", 14));
        row.append(&label("Ask", "design-context-word"));
        ask.set_child(Some(&row));
        super::widgets::tip(&ask, "Ask the terminal's agent to change this (Ctrl+Enter)");
        let weak = Rc::downgrade(&b);
        ask.connect_clicked(move |key| {
            if let Some(b) = weak.upgrade() {
                b.ask_agent(key.upcast_ref());
            }
        });
        bar.append(&ask);
        let more = gtk::MenuButton::new();
        more.add_css_class("design-context-key");
        more.set_child(Some(&crate::icons::image("more", 16)));
        super::widgets::tip(&more, "More");
        let menu = gtk::gio::Menu::new();
        let item = super::board::menu_item;
        let (lock, hide) = self.flag_captions();
        for items in [
            [item("Duplicate", "duplicate", Some("<Control>d")), item("Copy", "copy", Some("<Control>c")), item("Delete", "delete", Some("Delete"))].as_slice(),
            &[
                item("Bring to front", "front", Some("<Control><Shift>bracketright")),
                item("Send to back", "back", Some("<Control><Shift>bracketleft")),
                item("Group", "group", Some("<Control>g")),
                item("Ungroup", "ungroup", Some("<Control><Shift>g")),
            ],
            &[
                item("Flip horizontally", "flip-h", Some("<Shift>h")),
                item("Flip vertically", "flip-v", Some("<Shift>v")),
                item("Copy style", "copy-style", Some("<Control><Alt>c")),
                item("Paste style", "paste-style", Some("<Control><Alt>v")),
            ],
            &[item(lock, "lock", Some("<Control><Shift>l")), item(hide, "hide", Some("<Control><Shift>h"))],
        ] {
            let section = gtk::gio::Menu::new();
            for i in items {
                section.append_item(i);
            }
            menu.append_section(None, &section);
        }
        menu.append_section(None, &super::board::export_menu());
        more.set_menu_model(Some(&menu));
        bar.append(&more);

        super::board::keep_canvas_focus(bar.upcast_ref());
        b.hand_back(&bar);
        self.overlay.add_overlay(&bar);
        *self.context.borrow_mut() = Some(bar.upcast());
        // Built with its words and measured as such on the next layout; a rebuild of a compact
        // bar for the same kind of selection (each edit rebuilds it) stays compact.
        if self.context_compact.get() && self.context_kind.replace(kind) == kind {
            self.set_context_compact(true);
        } else {
            self.context_kind.set(kind);
            self.context_full.set(None);
            self.context_compact.set(false);
        }
        self.overlay.queue_allocate();
    }

    /// Bring the toolbar's values up to date in place, as the selection is now: while a text is
    /// typed, when the toolbar is not rebuilt (E-text-4).
    pub(super) fn refresh_context_values(&self) {
        let nodes: Vec<relay_bus::design::Node> = {
            let scene = self.doc.scene.borrow();
            self.selected().iter().filter_map(|id| scene.node(id).cloned()).collect()
        };
        for refresh in self.context_values.borrow().iter() {
            refresh(&nodes);
        }
    }

    /// Hide the toolbar's words (the parent's name, the style's, Ask's) and tighten its keys,
    /// or bring them back. Their tooltips keep what the words said.
    pub(super) fn set_context_compact(&self, compact: bool) {
        self.context_compact.set(compact);
        let Some(bar) = self.context.borrow().clone() else {
            return;
        };
        if compact {
            bar.add_css_class("compact");
        } else {
            bar.remove_css_class("compact");
        }
        fn walk(w: &gtk::Widget, show: bool) {
            let mut child = w.first_child();
            while let Some(c) = child {
                if c.has_css_class("design-context-word") {
                    c.set_visible(show);
                }
                walk(&c, show);
                child = c.next_sibling();
            }
        }
        walk(&bar, !compact);
        self.overlay.queue_allocate();
    }

    fn align_key(&self, b: &Rc<Board>, how: Align, glyph: &str, tip: &str) -> gtk::Button {
        let key = gtk::Button::new();
        key.add_css_class("design-context-key");
        key.set_child(Some(&glyphs::image(glyph, 16)));
        super::widgets::tip(&key, tip);
        let weak = Rc::downgrade(b);
        key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.align(how);
            }
        });
        key
    }

    fn text_controls(&self, b: &Rc<Board>, bar: &gtk::Box, nodes: &[relay_bus::design::Node]) {
        let sticky = nodes.iter().all(|n| n.kind == NodeKind::Sticky);
        // Style: the preset that matches, or the size alone.
        let size = same(nodes.iter().map(|n| super::paint::font_size(n).to_bits())).map(f64::from_bits);
        let weight = same(nodes.iter().map(|n| n.font_weight.unwrap_or(400)));
        let mono = nodes.iter().all(|n| n.font.as_deref() == Some("Geist Mono"));
        let current = TEXT_STYLES
            .iter()
            .find(|(name, s, w, _, _)| Some(*s) == size && Some(*w) == weight && (*name == "Code") == mono)
            .map(|s| s.0)
            .unwrap_or("Custom");
        let style = gtk::MenuButton::new();
        style.add_css_class("design-context-key");
        style.add_css_class("design-style-key");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&label("Aa", "design-style-glyph"));
        row.append(&label(current, "design-context-word"));
        row.append(&crate::icons::image("chevron-down", 10));
        style.set_child(Some(&row));
        super::widgets::tip(&style, &format!("Text style: {current}"));
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        for (name, s, w, lh, ls) in TEXT_STYLES {
            let item = gtk::Button::new();
            item.add_css_class("design-menu-row");
            let sample = label(name, "");
            let attrs = gtk::pango::AttrList::new();
            attrs.insert(gtk::pango::AttrSize::new_size_absolute((s.min(26.0) * gtk::pango::SCALE as f64) as i32));
            attrs.insert(gtk::pango::AttrInt::new_weight(if w >= 600 { gtk::pango::Weight::Semibold } else { gtk::pango::Weight::Normal }));
            if name == "Code" {
                attrs.insert(gtk::pango::AttrFontDesc::new(&gtk::pango::FontDescription::from_string("Geist Mono")));
            }
            sample.set_attributes(Some(&attrs));
            sample.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.append(&sample);
            line.append(&label(&format!("{s}"), "design-keycap"));
            item.set_child(Some(&line));
            let weak = Rc::downgrade(b);
            let p = pop.downgrade();
            item.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[
                        ("font_size", json!(s)),
                        ("font_weight", if w == 400 { Value::Null } else { json!(w) }),
                        ("line_height", json!(lh)),
                        ("letter_spacing", if ls == 0.0 { Value::Null } else { json!(ls) }),
                        ("font", if name == "Code" { json!("Geist Mono") } else { Value::Null }),
                    ]));
                }
            });
            list.append(&item);
        }
        pop.set_child(Some(&list));
        style.set_popover(Some(&pop));
        bar.append(&style);
        let weak = Rc::downgrade(b);
        // Captioned with its glyph, to scrub, as the inspector's is.
        let size_field = stored_number("@text", size, "Font size", move |v| {
            let v = v.clamp(1.0, 1000.0);
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("font_size", json!(v))]));
            }
            v
        });
        bar.append(&size_field);
        // While a text is typed the toolbar is not rebuilt: these follow the scene in place
        // (E-text-4).
        let entry = size_field.last_child().and_downcast::<gtk::Entry>().map(|e| e.downgrade());
        self.context_values.borrow_mut().push(Box::new(move |nodes| {
            if let Some(e) = entry.as_ref().and_then(|e| e.upgrade()).filter(|e| !e.has_focus()) {
                let size = same(nodes.iter().map(|n| super::paint::font_size(n).to_bits())).map(f64::from_bits);
                e.set_text(&size.map(relay_client::design::number::fmt).unwrap_or_default());
            }
        }));
        let color = same(nodes.iter().map(|n| n.color.clone())).flatten();
        let weak = Rc::downgrade(b);
        bar.append(&color_key(b, color.as_deref(), "Text colour", Some("Automatic"), move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("color", json!(c))]));
            }
        }));
        if sticky {
            let paper = same(nodes.iter().map(|n| n.fill.clone().unwrap_or_else(|| STICKY_FILL.into())));
            let key = gtk::MenuButton::new();
            key.add_css_class("design-color-key");
            super::widgets::tip(&key, "Paper");
            let dot = super::board::swatch_button(paper.as_deref(), "Paper");
            dot.set_can_target(false);
            key.set_child(Some(&dot));
            let pop = gtk::Popover::new();
            pop.add_css_class("design-menu");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            for (name, c) in STICKY_COLORS {
                let s = super::board::swatch_button(Some(c), name);
                let weak = Rc::downgrade(b);
                let p = pop.downgrade();
                s.connect_clicked(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.popdown();
                    }
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fill", json!(c))]));
                    }
                });
                row.append(&s);
            }
            pop.set_child(Some(&row));
            key.set_popover(Some(&pop));
            bar.append(&key);
        }
        bar.append(&sep());
        let toggles: [(&str, &str, &str, bool); 4] = [
            ("B", "font_weight", "Bold (Ctrl+B)", nodes.iter().all(|n| n.font_weight.unwrap_or(400) >= 600)),
            ("I", "italic", "Italic (Ctrl+I)", nodes.iter().all(|n| n.italic == Some(true))),
            ("U", "underline", "Underline (Ctrl+U)", nodes.iter().all(|n| n.underline == Some(true))),
            ("S", "strike", "Strikethrough", nodes.iter().all(|n| n.strike == Some(true))),
        ];
        for (glyph, field, tip, on) in toggles {
            let key = gtk::ToggleButton::with_label(glyph);
            key.add_css_class("design-context-key");
            key.add_css_class(&format!("design-emphasis-{field}"));
            super::widgets::tip(&key, tip);
            key.set_active(on);
            let weak = Rc::downgrade(b);
            let handler = key.connect_toggled(move |k| {
                if let Some(b) = weak.upgrade() {
                    let value = match (field, k.is_active()) {
                        (_, false) => Value::Null,
                        ("font_weight", true) => json!(700),
                        _ => json!(true),
                    };
                    b.set_selected(set(&[(field, value)]));
                }
            });
            let k = key.downgrade();
            self.context_values.borrow_mut().push(Box::new(move |nodes| {
                if let Some(k) = k.upgrade() {
                    let on = match field {
                        "font_weight" => nodes.iter().all(|n| n.font_weight.unwrap_or(400) >= 600),
                        "italic" => nodes.iter().all(|n| n.italic == Some(true)),
                        "underline" => nodes.iter().all(|n| n.underline == Some(true)),
                        _ => nodes.iter().all(|n| n.strike == Some(true)),
                    };
                    k.block_signal(&handler);
                    k.set_active(on);
                    k.unblock_signal(&handler);
                }
            }));
            bar.append(&key);
        }
        let align = same(nodes.iter().map(|n| n.align.unwrap_or(TextAlign::Left))).unwrap_or(TextAlign::Left);
        let (glyph, next, tip) = match align {
            TextAlign::Left => ("text-left", "center", "Aligned left — centre it"),
            TextAlign::Center => ("text-center", "right", "Centred — align right"),
            TextAlign::Right => ("text-right", "left", "Aligned right — align left"),
        };
        let key = gtk::Button::new();
        key.add_css_class("design-context-key");
        key.set_child(Some(&glyphs::image(glyph, 16)));
        super::widgets::tip(&key, tip);
        let weak = Rc::downgrade(b);
        key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("align", if next == "left" { Value::Null } else { json!(next) })]));
            }
        });
        bar.append(&key);
    }

    /// Turn auto layout on the selected frames to `mode` (`none`, `column`, `row` or `wrap`),
    /// padding each the first time so its children do not touch its edges. Off, each child it
    /// laid out keeps the box it shows, and the frame its size where it hugged them
    /// (`edit::set_layout`, TM-8): `layout: null` alone dropped them at stale places.
    pub(super) fn set_layout(&self, mode: &str) {
        let Some(mode) = layout_mode(mode) else {
            return;
        };
        let ids = self.selected();
        let placed = self.placed();
        let ops = edit::set_layout(&self.doc.scene.borrow(), &placed, &ids, Some(mode));
        self.commit(ops);
    }
}

/// A free line joined back to its start: no connector ends, no route (`shapes::closed_shape`,
/// which also asks for three points or more, B98).
#[allow(dead_code)]
pub(super) fn closed_shape(n: &relay_bus::design::Node) -> bool {
    shapes::closed_shape(n)
}

/// A layout key's mode by its stored name; `none` is no layout.
fn layout_mode(name: &str) -> Option<Layout> {
    serde_json::from_value(json!(name)).ok()
}

/// A frame's layout key for its `mode`: its glyph, its tip and the mode a press turns on, round
/// none, column, row and wrap.
fn layout_key(mode: Layout) -> (&'static str, &'static str, &'static str) {
    match mode {
        Layout::None => ("free", "Auto layout: add (stack children in a column)", "column"),
        Layout::Column => ("column", "Auto layout: column — switch to a row", "row"),
        Layout::Row => ("row", "Auto layout: row — switch to wrap", "wrap"),
        Layout::Wrap => ("wrap", "Auto layout: wrap — remove", "none"),
    }
}

fn sep() -> gtk::Separator {
    let s = gtk::Separator::new(gtk::Orientation::Vertical);
    s.add_css_class("design-context-sep");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_key_goes_round_none_column_row_and_wrap() {
        let mut mode = Layout::None;
        let mut seen = Vec::new();
        for _ in 0..4 {
            let (glyph, _, next) = layout_key(mode);
            assert!(glyphs::known(glyph), "{glyph} has its own glyph");
            seen.push(glyph);
            mode = layout_mode(next).expect("a stored layout name");
        }
        assert_eq!(seen, ["free", "column", "row", "wrap"]);
        assert_eq!(mode, Layout::None, "wrap's press removes it");
        assert!(layout_key(Layout::Wrap).1.ends_with("remove"));
        assert_eq!(layout_mode("sideways"), None);
    }

    #[test]
    fn a_curved_connector_has_its_own_glyph() {
        assert!(glyphs::known("curve"));
        assert_ne!(glyphs::geometry("curve"), glyphs::geometry("rotate"));
    }
}
