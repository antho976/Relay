//! The inspector: every property of the selection, grouped as design tools group them —
//! position and size, auto layout, fill, border, corners, shadow, opacity, type, picture and
//! export. It shows while something is selected and the inspector key is on; values that
//! differ across a multiple selection show empty, and setting one sets it on all.

use super::board::{display_name, kind_caption, kind_glyph, Board};
use super::glyphs;
use super::widgets::{color_key, number, segmented};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Blend, ImageFit, Items, Justify, Layout, Node, NodeKind, Sizing, TextAlign, TextCase};
use relay_client::design::{edit, layout};
use serde_json::{json, Map, Value};
use std::rc::Rc;

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

fn same<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

fn bits(v: f64) -> u64 {
    v.to_bits()
}

const WEIGHTS: [(u16, &str); 9] = [
    (100, "Thin"),
    (200, "Extra light"),
    (300, "Light"),
    (400, "Regular"),
    (500, "Medium"),
    (600, "Semibold"),
    (700, "Bold"),
    (800, "Extra bold"),
    (900, "Black"),
];

impl Board {
    pub(super) fn rebuild_inspector(&self) {
        let body = &self.inspector;
        while let Some(child) = body.first_child() {
            body.remove(&child);
        }
        let ids = self.selected();
        let show = self.inspector_key.is_active() && !ids.is_empty() && self.edit.borrow().is_none();
        self.inspector_scroll.set_visible(show);
        if !show {
            return;
        }
        let b = self.me();
        let scene = self.doc.scene.borrow();
        let nodes: Vec<Node> = ids.iter().filter_map(|id| scene.node(id).cloned()).collect();
        let Some(first) = nodes.first().cloned() else {
            return;
        };
        let single = nodes.len() == 1;
        let kinds = |k: &[NodeKind]| nodes.iter().all(|n| k.contains(&n.kind));
        let parent_lays_out = nodes.iter().all(|n| n.parent.as_deref().is_some_and(|p| edit::lays_out(&scene, p)));
        let parent_name = first.parent.as_deref().and_then(|p| scene.node(p)).map(display_name).unwrap_or_default();
        drop(scene);

        // Name, lock, hide.
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("design-inspector-head");
        head.append(&glyphs::image(if single { kind_glyph(first.kind) } else { "layers" }, 16));
        if single {
            let name = gtk::Entry::new();
            name.set_text(&display_name(&first));
            name.set_hexpand(true);
            name.add_css_class("design-name");
            name.set_tooltip_text(Some(&format!("{} name", kind_caption(first.kind))));
            let weak = Rc::downgrade(&b);
            let original = display_name(&first);
            name.connect_activate(move |e| {
                let text = e.text().trim().to_string();
                if let Some(b) = weak.upgrade() {
                    if text != original {
                        b.set_selected(set(&[("name", if text.is_empty() { Value::Null } else { json!(text) })]));
                    }
                }
            });
            head.append(&name);
        } else {
            let l = label(&format!("{} layers", nodes.len()), "design-inspector-title");
            l.set_hexpand(true);
            head.append(&l);
        }
        for (flag, on_glyph, off_glyph, tip) in [("locked", "lock", "unlock", "Lock (Ctrl+Shift+L)"), ("hidden", "eye-off", "eye", "Hide (Ctrl+Shift+H)")] {
            let on = nodes.iter().all(|n| if flag == "locked" { n.is_locked() } else { n.is_hidden() });
            let key = gtk::ToggleButton::new();
            key.add_css_class("design-context-key");
            key.set_child(Some(&glyphs::image(if on { on_glyph } else { off_glyph }, 14)));
            key.set_tooltip_text(Some(tip));
            key.set_active(on);
            let weak = Rc::downgrade(&b);
            key.connect_toggled(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.toggle_flag(flag);
                }
            });
            head.append(&key);
        }
        body.append(&head);

        // Position and size.
        let geometry = section(body, "Position and size");
        if parent_lays_out {
            let note = label(&format!("Placed by {parent_name}'s auto layout"), "design-note");
            note.set_wrap(true);
            geometry.append(&note);
        } else {
            let row = pair(
                number("X", same(nodes.iter().map(|n| bits(n.x))).map(f64::from_bits), "X from the parent's left edge", {
                    let w = Rc::downgrade(&b);
                    move |v| {
                        if let Some(b) = w.upgrade() {
                            b.set_selected(set(&[("x", json!(v.round()))]));
                        }
                    }
                }),
                number("Y", same(nodes.iter().map(|n| bits(n.y))).map(f64::from_bits), "Y from the parent's top edge", {
                    let w = Rc::downgrade(&b);
                    move |v| {
                        if let Some(b) = w.upgrade() {
                            b.set_selected(set(&[("y", json!(v.round()))]));
                        }
                    }
                }),
            );
            geometry.append(&row);
        }
        let placed = self.placed();
        let shown_w = same(ids.iter().filter_map(|id| placed.rect(id)).map(|r| bits(r.w.round()))).map(f64::from_bits);
        let shown_h = same(ids.iter().filter_map(|id| placed.rect(id)).map(|r| bits(r.h.round()))).map(f64::from_bits);
        let sizes = pair(
            number("W", shown_w, "Width", {
                let w = Rc::downgrade(&b);
                move |v| {
                    if let Some(b) = w.upgrade() {
                        b.set_size("w", v);
                    }
                }
            }),
            number("H", shown_h, "Height", {
                let w = Rc::downgrade(&b);
                move |v| {
                    if let Some(b) = w.upgrade() {
                        b.set_size("h", v);
                    }
                }
            }),
        );
        geometry.append(&sizes);
        if nodes.iter().all(|n| !matches!(n.kind, NodeKind::Line | NodeKind::Page)) {
            let turn = same(nodes.iter().map(|n| bits(n.rotation.unwrap_or(0.0)))).map(f64::from_bits);
            let weak = Rc::downgrade(&b);
            let rotate = number("@rotate", turn, "Rotation, degrees clockwise (drag just outside a corner to turn)", move |v| {
                if let Some(b) = weak.upgrade() {
                    let v = (v + 180.0).rem_euclid(360.0) - 180.0;
                    b.set_selected(set(&[("rotation", if v.abs() < 0.05 { Value::Null } else { json!(v) })]));
                }
            });
            rotate.set_hexpand(true);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.append(&rotate);
            for (horizontal, glyph, tip) in [(true, "flip-h", "Flip horizontally (Shift+H)"), (false, "flip-v", "Flip vertically (Shift+V)")] {
                let key = gtk::Button::new();
                key.add_css_class("design-icon-field");
                key.set_child(Some(&glyphs::image(glyph, 16)));
                key.set_tooltip_text(Some(tip));
                let weak = Rc::downgrade(&b);
                key.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.flip(horizontal);
                    }
                });
                row.append(&key);
            }
            geometry.append(&row);
        }
        let sizable = nodes.iter().all(|n| {
            n.kind == NodeKind::Text || n.layout_mode() != Layout::None || n.parent.as_deref().is_some_and(|_| parent_lays_out)
        });
        if sizable {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.set_homogeneous(true);
            for (axis, field) in [("w", "w_sizing"), ("h", "h_sizing")] {
                let current = same(nodes.iter().map(|n| if axis == "w" { layout::w_sizing(n) } else { layout::h_sizing(n) }));
                let options: Vec<(&str, &str)> = [("fixed", "Fixed"), ("hug", "Hug"), ("fill", "Fill")]
                    .into_iter()
                    .filter(|(v, _)| *v != "fill" || parent_lays_out)
                    .filter(|(v, _)| *v != "hug" || nodes.iter().all(|n| n.kind == NodeKind::Text || n.layout_mode() != Layout::None))
                    .collect();
                let names: Vec<&str> = options.iter().map(|o| o.1).collect();
                let dd = gtk::DropDown::from_strings(&names);
                dd.add_css_class("design-dropdown");
                dd.set_tooltip_text(Some(if axis == "w" { "Width rule" } else { "Height rule" }));
                let index = current
                    .and_then(|c| options.iter().position(|(v, _)| *v == sizing_name(c)))
                    .map(|i| i as u32)
                    .unwrap_or(gtk::INVALID_LIST_POSITION);
                dd.set_selected(index);
                let weak = Rc::downgrade(&b);
                let values: Vec<String> = options.iter().map(|o| o.0.to_string()).collect();
                dd.connect_selected_notify(move |dd| {
                    let Some(v) = values.get(dd.selected() as usize) else {
                        return;
                    };
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[(field, json!(v))]));
                    }
                });
                let cell = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                cell.append(&label(if axis == "w" { "W" } else { "H" }, "design-number-caption"));
                dd.set_hexpand(true);
                cell.append(&dd);
                row.append(&cell);
            }
            geometry.append(&row);
        }

        // Auto layout.
        if single && first.kind == NodeKind::Frame {
            let sec = section(body, "Auto layout");
            let mode = first.layout.unwrap_or(Layout::None);
            let weak = Rc::downgrade(&b);
            sec.append(&segmented(
                &[("none", "free", "Free: children keep their own place"), ("column", "column", "Column"), ("row", "row", "Row")],
                Some(match mode {
                    Layout::None => "none",
                    Layout::Column => "column",
                    Layout::Row => "row",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_layout(v);
                    }
                },
            ));
            if mode != Layout::None {
                let pad = first.padding_edges();
                let weak = Rc::downgrade(&b);
                let weak2 = Rc::downgrade(&b);
                sec.append(&pair(
                    number("@gap", first.gap.or(Some(0.0)), "Gap between children", move |v| {
                        if let Some(b) = weak.upgrade() {
                            b.set_selected(set(&[("gap", json!(v))]));
                        }
                    }),
                    number("@padding", (pad.iter().all(|p| *p == pad[0])).then_some(pad[0]), "Padding on every side", move |v| {
                        if let Some(b) = weak2.upgrade() {
                            b.set_selected(set(&[("padding", json!([v.max(0.0)]))]));
                        }
                    }),
                ));
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                row.set_homogeneous(true);
                for (i, (cap, tip)) in [("T", "Top padding"), ("R", "Right padding"), ("B", "Bottom padding"), ("L", "Left padding")].into_iter().enumerate() {
                    let weak = Rc::downgrade(&b);
                    row.append(&number(cap, Some(pad[i]), tip, move |v| {
                        if let Some(b) = weak.upgrade() {
                            let mut p = pad;
                            p[i] = v.max(0.0);
                            b.set_selected(set(&[("padding", json!(p))]));
                        }
                    }));
                }
                sec.append(&row);
                let row_mode = mode == Layout::Row;
                let weak = Rc::downgrade(&b);
                sec.append(&labelled(
                    "Along",
                    segmented(
                        &[
                            ("start", if row_mode { "align-left" } else { "align-top" }, "Start"),
                            ("center", if row_mode { "align-center-x" } else { "align-center-y" }, "Centre"),
                            ("end", if row_mode { "align-right" } else { "align-bottom" }, "End"),
                            ("space_between", if row_mode { "distribute-x" } else { "distribute-y" }, "Space between"),
                        ],
                        Some(match first.justify.unwrap_or(Justify::Start) {
                            Justify::Start => "start",
                            Justify::Center => "center",
                            Justify::End => "end",
                            Justify::SpaceBetween => "space_between",
                        }),
                        move |v| {
                            if let Some(b) = weak.upgrade() {
                                b.set_selected(set(&[("justify", if v == "start" { Value::Null } else { json!(v) })]));
                            }
                        },
                    ),
                ));
                let weak = Rc::downgrade(&b);
                sec.append(&labelled(
                    "Across",
                    segmented(
                        &[
                            ("start", if row_mode { "align-top" } else { "align-left" }, "Start"),
                            ("center", if row_mode { "align-center-y" } else { "align-center-x" }, "Centre"),
                            ("end", if row_mode { "align-bottom" } else { "align-right" }, "End"),
                            ("stretch", if row_mode { "distribute-y" } else { "distribute-x" }, "Stretch"),
                        ],
                        Some(match first.items.unwrap_or(Items::Start) {
                            Items::Start => "start",
                            Items::Center => "center",
                            Items::End => "end",
                            Items::Stretch => "stretch",
                        }),
                        move |v| {
                            if let Some(b) = weak.upgrade() {
                                b.set_selected(set(&[("items", if v == "start" { Value::Null } else { json!(v) })]));
                            }
                        },
                    ),
                ));
            }
            let clip = gtk::CheckButton::with_label("Clip content");
            clip.set_active(first.clip != Some(false));
            let weak = Rc::downgrade(&b);
            clip.connect_toggled(move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("clip", if c.is_active() { Value::Null } else { json!(false) })]));
                }
            });
            sec.append(&clip);
        }

        // Type.
        let labelled_shapes = nodes.iter().all(|n| match n.kind {
            NodeKind::Text | NodeKind::Sticky => true,
            NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon => n.text.as_deref().is_some_and(|t| !t.is_empty()),
            _ => false,
        });
        if labelled_shapes {
            let sec = section(body, "Type");
            let families = font_families(&self.canvas);
            let current = same(nodes.iter().map(|n| super::paint::font_family(n).to_string()));
            let dd = gtk::DropDown::from_strings(&families.iter().map(String::as_str).collect::<Vec<_>>());
            dd.set_enable_search(true);
            dd.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
            dd.add_css_class("design-dropdown");
            dd.set_tooltip_text(Some("Font"));
            // Mixed, or a family this machine lacks: nothing chosen, so any choice applies.
            dd.set_selected(current.and_then(|c| families.iter().position(|f| *f == c)).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
            let weak = Rc::downgrade(&b);
            let names = families.clone();
            dd.connect_selected_notify(move |dd| {
                if let (Some(b), Some(f)) = (weak.upgrade(), names.get(dd.selected() as usize)) {
                    b.set_selected(set(&[("font", if f == "Geist" { Value::Null } else { json!(f) })]));
                }
            });
            sec.append(&dd);
            let weights: Vec<&str> = WEIGHTS.iter().map(|w| w.1).collect();
            let wdd = gtk::DropDown::from_strings(&weights);
            wdd.add_css_class("design-dropdown");
            wdd.set_tooltip_text(Some("Weight"));
            let weight = same(nodes.iter().map(|n| n.font_weight.unwrap_or(400)));
            wdd.set_selected(
                weight
                    .and_then(|w| WEIGHTS.iter().position(|(v, _)| *v == ((w + 50) / 100 * 100).clamp(100, 900)))
                    .map_or(gtk::INVALID_LIST_POSITION, |i| i as u32),
            );
            let weak = Rc::downgrade(&b);
            wdd.connect_selected_notify(move |dd| {
                if let (Some(b), Some((w, _))) = (weak.upgrade(), WEIGHTS.get(dd.selected() as usize)) {
                    b.set_selected(set(&[("font_weight", if *w == 400 { Value::Null } else { json!(w) })]));
                }
            });
            let weak = Rc::downgrade(&b);
            let size_field = number("", same(nodes.iter().map(|n| bits(super::paint::font_size(n)))).map(f64::from_bits), "Size", move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("font_size", json!(v.clamp(1.0, 1000.0)))]));
                }
            });
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            wdd.set_hexpand(true);
            row.append(&wdd);
            size_field.set_size_request(72, -1);
            size_field.set_hexpand(false);
            row.append(&size_field);
            sec.append(&row);
            let weak = Rc::downgrade(&b);
            let weak2 = Rc::downgrade(&b);
            sec.append(&pair(
                number("@line-height", same(nodes.iter().map(|n| bits(n.line_height.unwrap_or(1.3)))).map(f64::from_bits), "Line height, times the size", move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("line_height", json!(v.clamp(0.5, 4.0)))]));
                    }
                }),
                number("@letter-spacing", same(nodes.iter().map(|n| bits(n.letter_spacing.unwrap_or(0.0)))).map(f64::from_bits), "Letter spacing, px", move |v| {
                    if let Some(b) = weak2.upgrade() {
                        b.set_selected(set(&[("letter_spacing", if v == 0.0 { Value::Null } else { json!(v) })]));
                    }
                }),
            ));
            let weak = Rc::downgrade(&b);
            let align = segmented(
                &[("left", "text-left", "Align left"), ("center", "text-center", "Centre"), ("right", "text-right", "Align right")],
                Some(match same(nodes.iter().map(|n| n.align.unwrap_or(TextAlign::Left))) {
                    Some(TextAlign::Center) => "center",
                    Some(TextAlign::Right) => "right",
                    Some(TextAlign::Left) => "left",
                    None => "",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("align", if v == "left" { Value::Null } else { json!(v) })]));
                    }
                },
            );
            let weak = Rc::downgrade(&b);
            let case = segmented(
                &[("none", "—", "As typed"), ("upper", "AA", "Upper case"), ("lower", "aa", "Lower case"), ("title", "Aa", "Title case")],
                Some(match same(nodes.iter().map(|n| n.text_case)) {
                    Some(None) => "none",
                    Some(Some(TextCase::Upper)) => "upper",
                    Some(Some(TextCase::Lower)) => "lower",
                    Some(Some(TextCase::Title)) => "title",
                    None => "",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("text_case", if v == "none" { Value::Null } else { json!(v) })]));
                    }
                },
            );
            sec.append(&align);
            sec.append(&case);
            let color = same(nodes.iter().map(|n| n.color.clone())).flatten();
            let weak = Rc::downgrade(&b);
            sec.append(&labelled("Colour", color_key(&b, color.as_deref(), "Text colour", true, move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("color", json!(c))]));
                }
            })));
        }

        // Polygon, icon, connector.
        if single && first.kind == NodeKind::Polygon {
            super::shape_panels::polygon(&b, &section(body, "Polygon"), &first);
        }
        if single && first.kind == NodeKind::Icon {
            super::shape_panels::icon(&b, &section(body, "Icon"), &first);
        }
        if single && first.kind == NodeKind::Line {
            super::shape_panels::connector(&b, &section(body, "Line"), &first);
        }
        if single && first.kind == NodeKind::Path {
            super::shape_panels::path(&b, &section(body, "Path"), &first);
        }
        if single && first.kind == NodeKind::Ellipse {
            super::shape_panels::arc(&b, &section(body, "Arc"), &first);
        }

        // Fill, border, corners.
        // A line or path fills too: closed, it is a shape of your own.
        let free_line = |n: &Node| matches!(n.kind, NodeKind::Line | NodeKind::Path) && n.from.is_none() && n.to.is_none();
        let fills = nodes.iter().all(|n| free_line(n) || !matches!(n.kind, NodeKind::Group | NodeKind::Line | NodeKind::Path | NodeKind::Image | NodeKind::Text | NodeKind::Icon));
        if fills {
            let sec = section(body, "Fill");
            if single {
                super::shape_panels::fill(&b, &sec, &first);
            } else {
                let fill = same(nodes.iter().map(super::paint::fill_of)).flatten();
                let weak = Rc::downgrade(&b);
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row.append(&color_key(&b, fill.as_deref(), "Fill", true, move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fill", json!(c)), ("gradient", Value::Null)]));
                    }
                }));
                row.append(&label(fill.as_deref().unwrap_or("Mixed"), "design-hex-caption"));
                sec.append(&row);
            }
        }
        if nodes.iter().all(|n| !matches!(n.kind, NodeKind::Group | NodeKind::Text | NodeKind::Icon)) {
            let lines = kinds(&[NodeKind::Line, NodeKind::Path]);
            let sec = section(body, if lines { "Stroke" } else { "Border" });
            let stroke = same(nodes.iter().map(|n| n.stroke.clone())).flatten();
            let weak = Rc::downgrade(&b);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&color_key(&b, stroke.as_deref(), "Colour", !lines, move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke", json!(c))]));
                }
            }));
            let weak = Rc::downgrade(&b);
            let width = same(nodes.iter().map(|n| bits(n.stroke_width.unwrap_or(if lines { 2.0 } else { 1.0 })))).map(f64::from_bits);
            row.append(&number("@stroke-width", width, "Weight", move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_width", json!(v.clamp(0.0, 1000.0)))]));
                }
            }));
            sec.append(&row);
            sec.append(&super::shape_panels::dash(&b, &nodes));
            if kinds(&[NodeKind::Line]) {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                for (field, caption) in [("arrow_start", "Start arrow"), ("arrow_end", "End arrow")] {
                    let c = gtk::CheckButton::with_label(caption);
                    c.set_active(nodes.iter().all(|n| if field == "arrow_start" { n.arrow_start == Some(true) } else { n.arrow_end == Some(true) }));
                    let weak = Rc::downgrade(&b);
                    c.connect_toggled(move |c| {
                        if let Some(b) = weak.upgrade() {
                            b.set_selected(set(&[(field, if c.is_active() { json!(true) } else { Value::Null })]));
                        }
                    });
                    row.append(&c);
                }
                sec.append(&row);
            }
        }
        if kinds(&[NodeKind::Frame, NodeKind::Rect, NodeKind::Image, NodeKind::Sticky]) {
            let sec = section(body, "Corners");
            let weak = Rc::downgrade(&b);
            let stickies = kinds(&[NodeKind::Sticky]);
            let default = if stickies { 6.0 } else { 0.0 };
            sec.append(&number("@radius", same(nodes.iter().map(|n| bits(n.radius.unwrap_or(default)))).map(f64::from_bits), "Corner radius", move |v| {
                if let Some(b) = weak.upgrade() {
                    let v = v.max(0.0);
                    // A sticky's own corners are round: square ones are written out.
                    b.set_selected(set(&[("radius", if v == default { Value::Null } else { json!(v) }), ("radii", Value::Null)]));
                }
            }));
            if single {
                super::shape_panels::corners(&b, &sec, &first);
            }
        }

        // Image.
        if single && first.kind == NodeKind::Image {
            let sec = section(body, "Picture");
            let weak = Rc::downgrade(&b);
            sec.append(&segmented(
                &[("cover", "Fill", "Fill the box, cropping"), ("contain", "Fit", "Show it whole"), ("stretch", "Stretch", "Stretch to the box")],
                Some(match first.fit.unwrap_or(ImageFit::Cover) {
                    ImageFit::Cover => "cover",
                    ImageFit::Contain => "contain",
                    ImageFit::Stretch => "stretch",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fit", if v == "cover" { Value::Null } else { json!(v) })]));
                    }
                },
            ));
            let replace = gtk::Button::with_label("Replace picture…");
            replace.add_css_class("quiet");
            let weak = Rc::downgrade(&b);
            let id = first.id.clone();
            replace.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    super::assets::replace_image(&b, &id);
                }
            });
            sec.append(&replace);
        }

        // Shadow and opacity.
        if nodes.iter().all(|n| !matches!(n.kind, NodeKind::Group | NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon)) {
            let sec = section(body, "Shadow");
            let shadow = same(nodes.iter().map(|n| n.shadow.clone())).flatten();
            let weak = Rc::downgrade(&b);
            sec.append(&segmented(
                &[("none", "None", "No shadow"), ("drop", "Drop", "A drop shadow under it"), ("inner", "Inner", "An inner shadow inside its edge")],
                Some(match &shadow {
                    None if nodes.iter().all(|n| n.shadow.is_none()) => "none",
                    None => "",
                    Some(s) if s.inset => "inner",
                    Some(_) => "drop",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        let v = match v {
                            "drop" => json!({"x": 0, "y": 4, "blur": 12, "spread": 0, "color": "#00000033"}),
                            "inner" => json!({"x": 0, "y": 2, "blur": 6, "spread": 0, "color": "#00000040", "inset": true}),
                            _ => Value::Null,
                        };
                        b.set_selected(set(&[("shadow", v)]));
                    }
                },
            ));
            if let Some(s) = shadow {
                let field = |cap: &'static str, value: f64, tip: &'static str, key: &'static str| {
                    let weak = Rc::downgrade(&b);
                    let s = s.clone();
                    number(cap, Some(value), tip, move |v| {
                        if let Some(b) = weak.upgrade() {
                            let mut s = serde_json::to_value(&s).unwrap_or_default();
                            s[key] = json!(if key == "blur" { v.max(0.0) } else { v });
                            b.set_selected(set(&[("shadow", s)]));
                        }
                    })
                };
                sec.append(&pair(field("X", s.x, "Offset across", "x"), field("Y", s.y, "Offset down", "y")));
                sec.append(&pair(field("Blur", s.blur, "Blur", "blur"), field("Spread", s.spread, "Spread", "spread")));
                let weak = Rc::downgrade(&b);
                let s2 = s.clone();
                sec.append(&labelled("Colour", color_key(&b, Some(&s.color), "Shadow colour", false, move |c| {
                    if let (Some(b), Some(c)) = (weak.upgrade(), c) {
                        let mut v = serde_json::to_value(&s2).unwrap_or_default();
                        v["color"] = json!(c);
                        b.set_selected(set(&[("shadow", v)]));
                    }
                })));
            }
        }
        let sec = section(body, "Layer");
        let weak = Rc::downgrade(&b);
        let opacity = same(nodes.iter().map(|n| bits((n.opacity.unwrap_or(1.0) * 100.0).round()))).map(f64::from_bits);
        sec.append(&number("@opacity", opacity, "Opacity, %", move |v| {
            if let Some(b) = weak.upgrade() {
                let o = (v / 100.0).clamp(0.0, 1.0);
                b.set_selected(set(&[("opacity", if o >= 1.0 { Value::Null } else { json!(o) })]));
            }
        }));
        let names: Vec<String> = Blend::ALL.iter().map(|m| blend_caption(*m)).collect();
        let dd = gtk::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
        dd.add_css_class("design-dropdown");
        dd.set_tooltip_text(Some("Blend: how it mixes with what is under it"));
        let current = same(nodes.iter().map(|n| n.blend.unwrap_or(Blend::Normal)));
        dd.set_selected(current.and_then(|m| Blend::ALL.iter().position(|x| *x == m)).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
        let weak = Rc::downgrade(&b);
        dd.connect_selected_notify(move |dd| {
            if let (Some(b), Some(mode)) = (weak.upgrade(), Blend::ALL.get(dd.selected() as usize)) {
                b.set_selected(set(&[("blend", if *mode == Blend::Normal { Value::Null } else { json!(mode.name()) })]));
            }
        });
        dd.set_hexpand(true);
        let blend_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        blend_row.add_css_class("design-number");
        let blend_glyph = glyphs::image("blend", 14);
        blend_glyph.add_css_class("design-number-caption");
        blend_row.append(&blend_glyph);
        blend_row.append(&dd);
        sec.append(&blend_row);
        let weak = Rc::downgrade(&b);
        let blur = same(nodes.iter().map(|n| bits(n.blur.unwrap_or(0.0)))).map(f64::from_bits);
        sec.append(&number("@blur", blur, "Layer blur, px", move |v| {
            if let Some(b) = weak.upgrade() {
                let v = v.clamp(0.0, 200.0);
                b.set_selected(set(&[("blur", if v <= 0.0 { Value::Null } else { json!(v) })]));
            }
        }));

        // Export.
        let sec = section(body, "Export");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_homogeneous(true);
        for (format, caption, tip) in [
            (super::export::Format::Png(1.0), "PNG", "A picture at 1×"),
            (super::export::Format::Png(2.0), "2×", "A picture at 2×, for sharp screens"),
            (super::export::Format::Svg, "SVG", "Vector, for the web and other design tools"),
            (super::export::Format::Pdf, "PDF", "Vector, to print or share"),
        ] {
            let key = gtk::Button::with_label(caption);
            key.add_css_class("design-export-key");
            key.set_tooltip_text(Some(tip));
            let weak = Rc::downgrade(&b);
            key.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    super::export::export_ids(&b, b.selected(), format);
                }
            });
            row.append(&key);
        }
        sec.append(&row);
    }

    /// The inspector shows the scene as it is now, unless the person is typing in it.
    pub(super) fn refresh_inspector_values(&self) {
        if !matches!(*self.gesture.borrow(), super::board::Gesture::Idle) {
            return;
        }
        let focused = self
            .root
            .root()
            .and_then(|r| gtk::prelude::GtkWindowExt::focus(r.downcast_ref::<gtk::Window>()?))
            .is_some_and(|f| f.is_ancestor(&self.inspector));
        if !focused {
            self.rebuild_inspector();
        }
    }

    /// A width or height typed in: a fixed size from now on.
    pub(super) fn set_size(&self, axis: &str, v: f64) {
        let v = v.max(1.0).round();
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let ops = ids
            .iter()
            .filter_map(|id| scene.node(id))
            .map(|n| {
                let mut s = set(&[(axis, json!(v))]);
                let rule = if axis == "w" { layout::w_sizing(n) } else { layout::h_sizing(n) };
                if rule != Sizing::Fixed {
                    s.insert(format!("{axis}_sizing"), json!("fixed"));
                }
                if let (Some(points), true) = (&n.points, matches!(n.kind, NodeKind::Line | NodeKind::Path)) {
                    let old = if axis == "w" { n.w } else { n.h };
                    let k = if old > 0.0 { v / old } else { 1.0 };
                    let scaled: Vec<[f64; 2]> = points.iter().map(|p| if axis == "w" { [p[0] * k, p[1]] } else { [p[0], p[1] * k] }).collect();
                    s.insert("points".into(), json!(scaled));
                }
                relay_bus::design::NodeOp::Update { id: n.id.clone(), set: s }
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }
}

/// A blend mode as the menu names it: "Color dodge".
fn blend_caption(m: Blend) -> String {
    let name = m.name().replace('_', " ");
    let mut chars = name.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn sizing_name(s: Sizing) -> &'static str {
    match s {
        Sizing::Fixed => "fixed",
        Sizing::Hug => "hug",
        Sizing::Fill => "fill",
    }
}

/// A titled group of the inspector.
fn section(body: &gtk::Box, title: &str) -> gtk::Box {
    let sec = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sec.add_css_class("design-section");
    sec.append(&label(title, "design-section-title"));
    body.append(&sec);
    sec
}

fn pair(a: gtk::Box, b: gtk::Box) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_homogeneous(true);
    row.append(&a);
    row.append(&b);
    row
}

fn labelled(caption: &str, widget: impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let l = label(caption, "design-row-caption");
    l.set_hexpand(true);
    row.append(&l);
    row.append(&widget);
    row
}

/// Relay's bundled faces first, then the system's, each once.
fn font_families(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    let mut names: Vec<String> = widget.as_ref().pango_context().list_families().iter().map(|f| f.name().to_string()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    let mut out: Vec<String> = ["Geist", "Geist Mono", "Sora"].iter().map(|s| s.to_string()).collect();
    out.extend(names.into_iter().filter(|n| !matches!(n.as_str(), "Geist" | "Geist Mono" | "Sora")));
    out
}
