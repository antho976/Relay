//! The toolbar that floats over the selection, as Claude Design's does over a picked element:
//! the few things you change most for what is selected — a text's style, size, colour and
//! emphasis; a shape's fill, stroke and corners; a stroke's colour and weight; several
//! layers' alignment — and Ask, which hands the selection to the terminal's agent.

use super::board::{display_name, Board};
use super::paint::STICKY_FILL;
use super::glyphs;
use super::tools::STICKY_COLORS;
use super::widgets::{color_key, number};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Layout, NodeKind, TextAlign};
use relay_client::design::edit::Align;
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
        if let Some(old) = self.context.borrow_mut().take() {
            self.overlay.remove_overlay(&old);
        }
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
                name.set_max_width_chars(14);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                row.append(&name);
                up.set_child(Some(&row));
                up.set_tooltip_text(Some(&format!("Select {} (Esc)", display_name(parent))));
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
        if all(&[NodeKind::Icon]) {
            let colour = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
            let weak = Rc::downgrade(&b);
            bar.append(&color_key(&b, colour.as_deref(), "Icon colour", true, move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke", json!(c))]));
                }
            }));
            if let [one] = nodes.as_slice() {
                let swap = gtk::MenuButton::new();
                swap.add_css_class("design-context-key");
                swap.set_child(Some(&super::icons::image(one.icon.as_deref().unwrap_or(""), 16)));
                swap.set_tooltip_text(Some("Another icon"));
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
            self.text_controls(&b, &bar, &nodes);
        } else if all(&[NodeKind::Line, NodeKind::Path]) {
            let stroke = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
            let weak = Rc::downgrade(&b);
            bar.append(&color_key(&b, stroke.as_deref(), "Stroke colour", false, move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke", json!(c))]));
                }
            }));
            let width = same(nodes.iter().map(|n| n.stroke_width.unwrap_or(2.0).to_bits())).map(f64::from_bits);
            let weak = Rc::downgrade(&b);
            bar.append(&number("W", width, "Stroke weight", move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_width", json!(v.clamp(0.5, 200.0)))]));
                }
            }));
            if all(&[NodeKind::Line]) {
                // Straight, elbow, curve: one key that steps through them.
                let route = same(nodes.iter().map(|n| n.route.unwrap_or(relay_bus::design::Route::Straight)))
                    .unwrap_or(relay_bus::design::Route::Straight);
                let (glyph, next, tip) = match route {
                    relay_bus::design::Route::Straight => ("line", "elbow", "Straight — make it an elbow"),
                    relay_bus::design::Route::Elbow => ("connector", "curve", "Elbow — make it a curve"),
                    relay_bus::design::Route::Curve => ("rotate", "straight", "Curve — make it straight"),
                };
                let key = gtk::Button::new();
                key.add_css_class("design-context-key");
                key.set_child(Some(&glyphs::image(glyph, 16)));
                key.set_tooltip_text(Some(tip));
                let weak = Rc::downgrade(&b);
                key.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("route", if next == "straight" { Value::Null } else { json!(next) })]));
                    }
                });
                bar.append(&key);
                let on = nodes.iter().all(|n| n.arrow_end == Some(true));
                let arrow = gtk::ToggleButton::new();
                arrow.add_css_class("design-context-key");
                arrow.set_child(Some(&glyphs::image("arrow", 16)));
                arrow.set_tooltip_text(Some("Arrowhead"));
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
            let fillable = nodes.iter().all(|n| n.kind != NodeKind::Group);
            if fillable {
                let fill = same(nodes.iter().map(super::paint::fill_of)).flatten();
                let weak = Rc::downgrade(&b);
                bar.append(&color_key(&b, fill.as_deref(), "Fill", true, move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fill", json!(c))]));
                    }
                }));
                let stroke = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
                let weak = Rc::downgrade(&b);
                let key = color_key(&b, stroke.as_deref(), "Border", true, move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("stroke", json!(c))]));
                    }
                });
                key.add_css_class("design-stroke-key");
                bar.append(&key);
            }
            if nodes.iter().all(|n| matches!(n.kind, NodeKind::Frame | NodeKind::Rect | NodeKind::Image)) {
                let radius = same(nodes.iter().map(|n| n.radius.unwrap_or(0.0).to_bits())).map(f64::from_bits);
                let weak = Rc::downgrade(&b);
                let field = number("@radius", radius, "Corner radius", move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("radius", if v <= 0.0 { Value::Null } else { json!(v) })]));
                    }
                });
                bar.append(&field);
            }
            if let [one] = nodes.as_slice() {
                if one.kind == NodeKind::Frame {
                    let mode = one.layout.unwrap_or(Layout::None);
                    let key = gtk::Button::new();
                    key.add_css_class("design-context-key");
                    let (glyph, tip, next) = match mode {
                        Layout::None => ("free", "Auto layout: add (stack children in a column)", "column"),
                        Layout::Column => ("column", "Auto layout: column — switch to a row", "row"),
                        Layout::Row => ("row", "Auto layout: row — remove", "none"),
                    };
                    key.set_child(Some(&glyphs::image(glyph, 16)));
                    key.set_tooltip_text(Some(tip));
                    let weak = Rc::downgrade(&b);
                    key.connect_clicked(move |_| {
                        if let Some(b) = weak.upgrade() {
                            b.set_layout(next);
                        }
                    });
                    bar.append(&key);
                }
            }
        } else {
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
            group.set_tooltip_text(Some("Group (Ctrl+G)"));
            group.set_action_name(Some("design.group"));
            bar.append(&group);
            let frame = gtk::Button::new();
            frame.add_css_class("design-context-key");
            frame.set_child(Some(&glyphs::image("frame", 16)));
            frame.set_tooltip_text(Some("Frame selection (Ctrl+Alt+G)"));
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
        row.append(&label("Ask", ""));
        ask.set_child(Some(&row));
        ask.set_tooltip_text(Some("Ask the terminal's agent to change this (Ctrl+Enter)"));
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
        more.set_tooltip_text(Some("More"));
        let menu = gtk::gio::Menu::new();
        let section = gtk::gio::Menu::new();
        section.append(Some("Duplicate"), Some("design.duplicate"));
        section.append(Some("Copy"), Some("design.copy"));
        section.append(Some("Delete"), Some("design.delete"));
        menu.append_section(None, &section);
        let section = gtk::gio::Menu::new();
        section.append(Some("Bring to front"), Some("design.front"));
        section.append(Some("Send to back"), Some("design.back"));
        section.append(Some("Group"), Some("design.group"));
        section.append(Some("Ungroup"), Some("design.ungroup"));
        menu.append_section(None, &section);
        let section = gtk::gio::Menu::new();
        section.append(Some("Lock"), Some("design.lock"));
        section.append(Some("Hide"), Some("design.hide"));
        section.append(Some("Export as PNG…"), Some("design.export"));
        menu.append_section(None, &section);
        more.set_menu_model(Some(&menu));
        bar.append(&more);

        super::board::keep_canvas_focus(bar.upcast_ref());
        self.overlay.add_overlay(&bar);
        *self.context.borrow_mut() = Some(bar.upcast());
        self.overlay.queue_allocate();
    }

    fn align_key(&self, b: &Rc<Board>, how: Align, glyph: &str, tip: &str) -> gtk::Button {
        let key = gtk::Button::new();
        key.add_css_class("design-context-key");
        key.set_child(Some(&glyphs::image(glyph, 16)));
        key.set_tooltip_text(Some(tip));
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
        row.append(&label(current, ""));
        row.append(&crate::icons::image("chevron-down", 10));
        style.set_child(Some(&row));
        style.set_tooltip_text(Some("Text style"));
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
        bar.append(&number("", size, "Font size", move |v| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("font_size", json!(v.clamp(1.0, 1000.0)))]));
            }
        }));
        let color = same(nodes.iter().map(|n| n.color.clone())).flatten();
        let weak = Rc::downgrade(b);
        bar.append(&color_key(b, color.as_deref(), "Text colour", true, move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("color", json!(c))]));
            }
        }));
        if sticky {
            let paper = same(nodes.iter().map(|n| n.fill.clone().unwrap_or_else(|| STICKY_FILL.into())));
            let key = gtk::MenuButton::new();
            key.add_css_class("design-color-key");
            key.set_tooltip_text(Some("Paper"));
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
            key.set_tooltip_text(Some(tip));
            key.set_active(on);
            let weak = Rc::downgrade(b);
            key.connect_toggled(move |k| {
                if let Some(b) = weak.upgrade() {
                    let value = match (field, k.is_active()) {
                        (_, false) => Value::Null,
                        ("font_weight", true) => json!(700),
                        _ => json!(true),
                    };
                    b.set_selected(set(&[(field, value)]));
                }
            });
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
        key.set_tooltip_text(Some(tip));
        let weak = Rc::downgrade(b);
        key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("align", if next == "left" { Value::Null } else { json!(next) })]));
            }
        });
        bar.append(&key);
    }

    /// Turn auto layout on a frame to `mode` (`none`, `row` or `column`), padding it the first
    /// time so its children do not touch its edges.
    pub(super) fn set_layout(&self, mode: &str) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let ops = ids
            .iter()
            .filter_map(|id| scene.node(id).filter(|n| n.kind == NodeKind::Frame))
            .map(|n| {
                let mut s = set(&[("layout", if mode == "none" { Value::Null } else { json!(mode) })]);
                if mode != "none" && n.layout.is_none() {
                    if n.gap.is_none() {
                        s.insert("gap".into(), json!(12));
                    }
                    if n.padding.is_none() {
                        s.insert("padding".into(), json!([16]));
                    }
                }
                relay_bus::design::NodeOp::Update { id: n.id.clone(), set: s }
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }
}

fn sep() -> gtk::Separator {
    let s = gtk::Separator::new(gtk::Orientation::Vertical);
    s.add_css_class("design-context-sep");
    s
}
