//! The inspector: every property of the selection, grouped as design tools group them —
//! position and size, auto layout, type, fill, stroke, corners, shadow, opacity, picture and
//! export. It shows while something is selected and the inspector key is on; values that
//! differ across a multiple selection show empty, and setting one sets it on all. A section
//! shows only where every selected node takes its fields (`relay_bus::design::takes`), and a
//! number field shows what was stored, not what was typed (B34).

use super::board::{kind_glyph, Board};
use super::glyphs;
use super::shape_panels::Slot;
use super::widgets::{color_key, color_key_named, segmented, stored_number, stored_number_step, tip};
use crate::app::label;
use glib::translate::IntoGlib;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{parse_color, takes, Blend, ImageFit, Items, Justify, Layout, Node, NodeKind, NodeOp, Shadow, Sizing, TextAlign, TextCase, MAX_SHADOWS};
use relay_client::design::names::{display_name, name_key, node_caption};
use relay_client::design::geom::Rect;
use relay_client::design::{color, edit, layout, shapes, Scene};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
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

/// A node's value as it is now, read when a field commits rather than when the inspector was
/// built (TM-37): a second field of the same value keeps the first one's edit, and an agent's
/// change made meanwhile is not written over. As stored where it holds one (a `$name` colour
/// stays a name), else as drawn (a style's, or a sticky's own shadow).
pub(super) fn current<T>(b: &Board, id: &str, f: impl Fn(&Node) -> Option<T>) -> Option<T> {
    let scene = b.doc.scene.borrow();
    scene.stored(id).and_then(&f).or_else(|| scene.node(id).and_then(&f))
}

/// The shadows the Shadow section shows and edits for a node as it is now: what it stores, else
/// what it draws (a style's, a sticky's own), less what draws nothing (a clear colour is how a
/// node says it casts none).
fn shown_shadows(b: &Board, id: &str) -> Vec<Shadow> {
    current(b, id, |n| (!n.shadow_list().is_empty()).then(|| shapes::effective_shadows(n)))
        .or_else(|| b.doc.scene.borrow().node(id).map(shapes::effective_shadows))
        .unwrap_or_default()
}

/// The fields that give a node `list` as its shadows, both always set so an undo puts back
/// either: one shadow is written as `shadow`, which older builds read; two or more as
/// `shadows`; none as neither.
fn write_shadows(list: &[Shadow]) -> Map<String, Value> {
    let value = |s: &Shadow| serde_json::to_value(s).unwrap_or_default();
    match list {
        [] => set(&[("shadow", Value::Null), ("shadows", Value::Null)]),
        [one] => set(&[("shadow", value(one)), ("shadows", Value::Null)]),
        many => set(&[("shadow", Value::Null), ("shadows", json!(many.iter().map(value).collect::<Vec<_>>()))]),
    }
}

/// Kinds that cast their shadows from what they draw (the letters, the stroke): outer only,
/// without spread.
fn glows(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Text | NodeKind::Line | NodeKind::Path | NodeKind::Icon)
}

/// What a node's shadows read as: none, a sticky's own ("default"), an inner one, a glow (no
/// offset) or a drop shadow, by the first it draws.
fn shadow_look(n: &Node) -> &'static str {
    if n.kind == NodeKind::Sticky && n.shadow_list().is_empty() {
        return "default";
    }
    match shapes::effective_shadows(n).first() {
        None => "none",
        Some(s) if s.inset => "inner",
        Some(s) if s.x == 0.0 && s.y == 0.0 => "glow",
        Some(_) => "drop",
    }
}

/// `hex` at alpha `a` (0 to 255), as `#RRGGBBAA`.
fn with_alpha(hex: &str, a: u8) -> String {
    let [r, g, b, _] = parse_color(hex).unwrap_or([0x4C, 0x9D, 0xFF, 255]);
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// A glow in a node's own colour: its stroke for a line, its words' for a text or an icon,
/// else its fill.
fn glow_of(scene: &Scene, n: &Node) -> Shadow {
    let own = match n.kind {
        NodeKind::Line | NodeKind::Path => n.stroke.clone().unwrap_or_else(|| color::STROKE.into()),
        NodeKind::Text => color::text_color(scene, n),
        NodeKind::Icon => n.stroke.clone().unwrap_or_else(|| color::text_color(scene, n)),
        _ => n.gradient.as_ref().and_then(|g| g.stops.first()).map(|s| s.color.clone()).or_else(|| color::fill_of(n)).unwrap_or_default(),
    };
    Shadow { x: 0.0, y: 0.0, blur: 16.0, spread: if glows(n.kind) { 0.0 } else { 2.0 }, color: with_alpha(&own, 0xCC), inset: false }
}

impl Board {
    pub(super) fn rebuild_inspector(&self) {
        self.release(&self.inspector_scroll);
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
        let all_take = |field: &str| nodes.iter().all(|n| takes(n.kind, field));
        let parent_lays_out = nodes.iter().all(|n| n.parent.as_deref().is_some_and(|p| edit::lays_out(&scene, p)));
        let parent_name = first.parent.as_deref().and_then(|p| scene.node(p)).map(display_name).unwrap_or_default();
        let placed = self.placed();
        // A connector held at both ends is drawn between them, whatever box it stores (B91).
        let joined = nodes.iter().all(|n| n.kind == NodeKind::Line && shapes::attached_ends(&scene, &placed, n) == (true, true));
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
            // What it is by what it draws: a closed line is a "Shape" (E-board-24).
            name.set_tooltip_text(Some(&format!("{} name", node_caption(&first))));
            // Enter or leaving the field names the layer; Esc puts back the name it had.
            let shown = Rc::new(std::cell::RefCell::new(display_name(&first).trim().to_string()));
            let weak = Rc::downgrade(&b);
            let s = shown.clone();
            let rename = Rc::new(move |e: &gtk::Entry| {
                let text = e.text().trim().to_string();
                if let Some(b) = weak.upgrade().filter(|_| *s.borrow() != text) {
                    *s.borrow_mut() = text.clone();
                    b.set_selected(set(&[("name", if text.is_empty() { Value::Null } else { json!(text) })]));
                }
            });
            let r = rename.clone();
            name.connect_activate(move |e| r(e));
            let focus = gtk::EventControllerFocus::new();
            let (r, e) = (rename.clone(), name.downgrade());
            focus.connect_leave(move |_| {
                if let Some(e) = e.upgrade() {
                    r(&e);
                }
            });
            name.add_controller(focus);
            let keys = gtk::EventControllerKey::new();
            let (s, e) = (shown.clone(), name.downgrade());
            keys.connect_key_pressed(move |_, key, _, _| {
                if let (gtk::gdk::Key::Escape, Some(e)) = (key, e.upgrade()) {
                    e.set_text(&s.borrow());
                }
                glib::Propagation::Proceed
            });
            name.add_controller(keys);
            head.append(&name);
        } else {
            let l = label(&format!("{} layers", nodes.len()), "design-inspector-title");
            l.set_hexpand(true);
            head.append(&l);
        }
        // Each says what a press does now: Unlock while locked, Show while hidden.
        for (flag, on_glyph, off_glyph, on_tip, off_tip) in [
            ("locked", "lock", "unlock", "Unlock (Ctrl+Shift+L)", "Lock (Ctrl+Shift+L)"),
            ("hidden", "eye-off", "eye", "Show (Ctrl+Shift+H)", "Hide (Ctrl+Shift+H)"),
        ] {
            let on = nodes.iter().all(|n| if flag == "locked" { n.is_locked() } else { n.is_hidden() });
            let key = gtk::ToggleButton::new();
            key.add_css_class("design-context-key");
            key.set_child(Some(&glyphs::image(if on { on_glyph } else { off_glyph }, 14)));
            tip(&key, if on { on_tip } else { off_tip });
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
        if !kinds(&[NodeKind::Page]) && all_take("style") {
            self.style_row(&b, body, &ids);
        }

        // Position and size.
        let geometry = section(body, "Position and size");
        if joined {
            let note = label("Follows the shapes it joins", "design-note");
            note.set_wrap(true);
            geometry.append(&note);
        } else {
            let absolute = nodes.iter().all(|n| n.is_absolute());
            if parent_lays_out {
                if !absolute {
                    let note = label(&format!("Placed by {parent_name}'s auto layout"), "design-note");
                    note.set_wrap(true);
                    geometry.append(&note);
                }
                // Out of the layout where it shows, taking no room in it (feature-layout §3.6).
                let check = gtk::CheckButton::with_label("Absolute position");
                check.set_tooltip_text(Some("Keeps its own place inside the auto layout and takes no room"));
                check.set_active(absolute);
                let weak = Rc::downgrade(&b);
                check.connect_toggled(move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_absolute(c.is_active());
                    }
                });
                geometry.append(&check);
            }
            if !parent_lays_out || absolute {
                // Where each shows from its parent's corner, written through its pins; a hidden
                // one, which shows nowhere, by what it stores.
                let shown = |axis: &str| {
                    let scene = self.doc.scene.borrow();
                    same(nodes.iter().map(|n| {
                        let at = shown_rel(&scene, &placed, &n.id).map_or(if axis == "x" { n.x } else { n.y }, |(r, _)| if axis == "x" { r.x } else { r.y });
                        bits(at.round())
                    }))
                    .map(f64::from_bits)
                };
                let field = |axis: &'static str, caption: &str, tip: &str| {
                    let w = Rc::downgrade(&b);
                    stored_number(caption, shown(axis), tip, move |v| w.upgrade().map_or(v, |b| b.set_position(axis, v.round())))
                };
                geometry.append(&pair(field("x", "X", "X from the parent's left edge"), field("y", "Y", "Y from the parent's top edge")));
            }
        }
        if !joined {
            let size = |axis: &'static str, caption: &str, tip: &str| {
                let value = same(ids.iter().filter_map(|id| placed.rect(id)).map(|r| bits(if axis == "w" { r.w } else { r.h }.round()))).map(f64::from_bits);
                let w = Rc::downgrade(&b);
                let field = stored_number(caption, value, tip, move |v| w.upgrade().map_or(v, |b| b.set_size(axis, v)));
                // A flat line has no size across it to type (B36).
                let along = usize::from(axis == "h");
                let flat = nodes.iter().all(|n| {
                    matches!(n.kind, NodeKind::Line | NodeKind::Path) && n.points.as_ref().is_some_and(|p| p.iter().all(|q| p.first().is_some_and(|f| q[along] == f[along])))
                });
                if flat {
                    field.set_sensitive(false);
                    field.set_tooltip_text(Some(if axis == "h" { "A flat line has no height: drag an end to tilt it" } else { "A flat line has no width: drag an end to tilt it" }));
                }
                // A group is as big as its layers (B90).
                if kinds(&[NodeKind::Group]) {
                    field.set_sensitive(false);
                    field.set_tooltip_text(Some("A group's size is its layers'"));
                }
                field
            };
            geometry.append(&pair(size("w", "W", "Width"), size("h", "H", "Height")));
        }
        // A line turns by its points (B99).
        if all_take("rotation") {
            let turn = same(nodes.iter().map(|n| bits(n.rotation.unwrap_or(0.0)))).map(f64::from_bits);
            let weak = Rc::downgrade(&b);
            let rotate = stored_number("@rotate", turn, "Rotation, degrees clockwise (drag just outside a corner to turn)", move |v| {
                let v = (v + 180.0).rem_euclid(360.0) - 180.0;
                let v = if v.abs() < 0.05 { 0.0 } else { v };
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("rotation", if v == 0.0 { Value::Null } else { json!(v) })]));
                }
                v
            });
            rotate.set_hexpand(true);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.append(&rotate);
            for (horizontal, glyph, caption) in [(true, "flip-h", "Flip horizontally (Shift+H)"), (false, "flip-v", "Flip vertically (Shift+V)")] {
                let key = gtk::Button::new();
                key.add_css_class("design-icon-field");
                key.set_child(Some(&glyphs::image(glyph, 16)));
                tip(&key, caption);
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
            for axis in ["w", "h"] {
                let current = same(nodes.iter().map(|n| if axis == "w" { layout::w_sizing(n) } else { layout::h_sizing(n) }));
                let options: Vec<(&str, &str)> = [("fixed", "Fixed"), ("hug", "Hug"), ("fill", "Fill")]
                    .into_iter()
                    // An absolute layer takes no room in the layout, so nothing to fill.
                    .filter(|(v, _)| *v != "fill" || (parent_lays_out && !nodes.iter().any(Node::is_absolute)))
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
                        b.set_rule(axis, v);
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
        // Min and max sizes: empty is unset.
        if !joined && nodes.iter().all(|n| !matches!(n.kind, NodeKind::Page | NodeKind::Group | NodeKind::Line | NodeKind::Path)) {
            let bound = |key: &'static str, caption: &str, tip: &str, get: fn(&Node) -> Option<f64>| {
                let value = same(nodes.iter().map(|n| get(n).map(bits))).flatten().map(f64::from_bits);
                let w = Rc::downgrade(&b);
                super::widgets::optional_number(caption, value, tip, move |v| w.upgrade().and_then(|b| b.set_bound(key, v)))
            };
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            row.set_homogeneous(true);
            row.append(&bound("min_w", "Min W", "Least width; empty for none", |n| n.min_w));
            row.append(&bound("max_w", "Max W", "Most width; empty for none", |n| n.max_w));
            row.append(&bound("min_h", "Min H", "Least height; empty for none", |n| n.min_h));
            row.append(&bound("max_h", "Max H", "Most height; empty for none", |n| n.max_h));
            geometry.append(&row);
        }

        // Constraints: how each layer follows a frame it sits free in as the frame's size
        // changes (feature-layout §2.1).
        let free_in_frame = {
            let scene = self.doc.scene.borrow();
            nodes.iter().all(|n| {
                n.parent.as_deref().and_then(|p| scene.node(p)).is_some_and(|p| p.kind == NodeKind::Frame)
                    && (!parent_lays_out || n.is_absolute())
                    && !(n.kind == NodeKind::Line && (n.from.is_some() || n.to.is_some()))
            })
        };
        if free_in_frame && !joined {
            let sec = section(body, "Constraints");
            let stretches = !nodes.iter().any(|n| matches!(n.kind, NodeKind::Line | NodeKind::Path | NodeKind::Group));
            for (across, caption, options) in [
                (true, "Across", vec![("left", "Left"), ("right", "Right"), ("left_right", "Left and right"), ("center", "Center"), ("scale", "Scale")]),
                (false, "Down", vec![("top", "Top"), ("bottom", "Bottom"), ("top_bottom", "Top and bottom"), ("center", "Center"), ("scale", "Scale")]),
            ] {
                let options: Vec<(&'static str, &'static str)> = options.into_iter().filter(|(v, _)| stretches || !v.contains('_')).collect();
                let current = same(nodes.iter().map(|n| {
                    let pin = if across { n.pin_x.map(|p| p.name()) } else { n.pin_y.map(|p| p.name()) };
                    pin.unwrap_or(if across { "left" } else { "top" })
                }));
                let dd = gtk::DropDown::from_strings(&options.iter().map(|o| o.1).collect::<Vec<_>>());
                dd.add_css_class("design-dropdown");
                dd.set_tooltip_text(Some(if across { "Follows its frame's width by" } else { "Follows its frame's height by" }));
                dd.set_selected(current.and_then(|c| options.iter().position(|(v, _)| *v == c)).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
                let weak = Rc::downgrade(&b);
                dd.connect_selected_notify(move |dd| {
                    let (Some(b), Some((v, _))) = (weak.upgrade(), options.get(dd.selected() as usize)) else { return };
                    b.set_pin(across, (!matches!(*v, "left" | "top")).then_some(*v));
                });
                dd.set_hexpand(true);
                sec.append(&labelled(caption, dd));
            }
        }

        // Auto layout.
        if single && first.kind == NodeKind::Frame {
            let sec = section(body, "Auto layout");
            let mode = first.layout.unwrap_or(Layout::None);
            let weak = Rc::downgrade(&b);
            sec.append(&segmented(
                &[
                    ("none", "free", "Free: children keep their own place"),
                    ("column", "column", "Column"),
                    ("row", "row", "Row"),
                    ("wrap", "wrap", "Wrap: rows that break onto new lines"),
                ],
                Some(match mode {
                    Layout::None => "none",
                    Layout::Column => "column",
                    Layout::Row => "row",
                    Layout::Wrap => "wrap",
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
                let wraps = mode == Layout::Wrap;
                sec.append(&pair(
                    stored_number("@gap", first.gap.or(Some(0.0)), if wraps { "Gap along a line" } else { "Gap between children" }, move |v| {
                        if let Some(b) = weak.upgrade() {
                            b.set_selected(set(&[("gap", json!(v))]));
                        }
                        v
                    }),
                    stored_number("@padding", (pad.iter().all(|p| *p == pad[0])).then_some(pad[0]), "Padding on every side", move |v| {
                        let v = v.max(0.0);
                        if let Some(b) = weak2.upgrade() {
                            b.set_selected(set(&[("padding", json!([v]))]));
                        }
                        v
                    }),
                ));
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                row.set_homogeneous(true);
                for (i, (cap, tip)) in [("T", "Top padding"), ("R", "Right padding"), ("B", "Bottom padding"), ("L", "Left padding")].into_iter().enumerate() {
                    let weak = Rc::downgrade(&b);
                    let id = first.id.clone();
                    row.append(&stored_number(cap, Some(pad[i]), tip, move |v| {
                        let v = v.max(0.0);
                        if let Some(b) = weak.upgrade() {
                            // The other sides as they are now, not as they were when the panel
                            // was built (TM-37): T, then R, keeps the new T.
                            let mut p = current(&b, &id, |n| n.padding.is_some().then(|| n.padding_edges())).unwrap_or([0.0; 4]);
                            p[i] = v;
                            b.commit(vec![NodeOp::Update { id: id.clone(), set: set(&[("padding", json!(p))]) }]);
                        }
                        v
                    }));
                }
                sec.append(&row);
                if wraps {
                    let weak = Rc::downgrade(&b);
                    // Shows the gap along a line while unset, which is what it takes then.
                    sec.append(&labelled("Lines", stored_number("@gap", first.row_gap.or(first.gap).or(Some(0.0)), "Gap between lines", move |v| {
                        let v = v.max(0.0);
                        if let Some(b) = weak.upgrade() {
                            b.set_selected(set(&[("row_gap", json!(v))]));
                        }
                        v
                    })));
                }
                let row_mode = matches!(mode, Layout::Row | Layout::Wrap);
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
            clip.set_active(first.clip != Some(false) || first.scroll.is_some());
            // A frame that scrolls clips what it scrolls.
            clip.set_sensitive(first.scroll.is_none());
            let weak = Rc::downgrade(&b);
            clip.connect_toggled(move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("clip", if c.is_active() { Value::Null } else { json!(false) })]));
                }
            });
            // Scrolling along an axis the frame hugs would be refused: it has nothing to scroll.
            let (hug_w, hug_h) = (first.w_rule() == Sizing::Hug, first.h_rule() == Sizing::Hug);
            let scrolls: Vec<(&str, &str)> = [("none", "No scroll"), ("x", "Scrolls across"), ("y", "Scrolls down"), ("both", "Scrolls both ways")]
                .into_iter()
                .filter(|(v, _)| !(hug_w && matches!(*v, "x" | "both")) && !(hug_h && matches!(*v, "y" | "both")))
                .collect();
            let dd = gtk::DropDown::from_strings(&scrolls.iter().map(|o| o.1).collect::<Vec<_>>());
            dd.add_css_class("design-dropdown");
            dd.set_tooltip_text(Some("Scroll: what it holds past its edges shows by scrolling (a still design shows the start)"));
            let now = match first.scroll {
                None => "none",
                Some(relay_bus::design::Scroll::X) => "x",
                Some(relay_bus::design::Scroll::Y) => "y",
                Some(relay_bus::design::Scroll::Both) => "both",
            };
            dd.set_selected(scrolls.iter().position(|(v, _)| *v == now).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
            let weak = Rc::downgrade(&b);
            dd.connect_selected_notify(move |dd| {
                if let (Some(b), Some((v, _))) = (weak.upgrade(), scrolls.get(dd.selected() as usize)) {
                    b.set_selected(set(&[("scroll", if *v == "none" { Value::Null } else { json!(v) })]));
                }
            });
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&clip);
            dd.set_hexpand(true);
            row.append(&dd);
            sec.append(&row);
        }

        // Type: a text, a sticky, and a shape with a label, or one being typed into (E-text-6).
        let editing = self.edit.borrow().as_ref().map(|e| e.id.clone());
        let worded = |n: &Node| {
            !super::textedit::is_shape(n.kind) || n.text.as_deref().is_some_and(|t| !t.is_empty()) || editing.as_deref() == Some(n.id.as_str())
        };
        if all_take("font_size") && nodes.iter().all(worded) {
            let sec = section(body, "Type");
            let fonts = fonts(&self.canvas);
            let current = same(nodes.iter().map(|n| super::paint::font_family(n).to_string()));
            // A family named in another case is the one installed; one that is not installed
            // shows first, by its name, so it can be seen and changed (E-text-7).
            let found = current.as_deref().map(|c| fonts.families.iter().position(|f| f.eq_ignore_ascii_case(c)));
            let missing = current.clone().filter(|_| found == Some(None));
            let mut shown: Vec<String> = missing.iter().map(|m| format!("{m} (not installed)")).collect();
            shown.extend(fonts.families.iter().cloned());
            let dd = gtk::DropDown::from_strings(&shown.iter().map(String::as_str).collect::<Vec<_>>());
            dd.set_enable_search(true);
            dd.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
            dd.add_css_class("design-dropdown");
            dd.set_tooltip_text(Some("Font"));
            // Mixed: nothing chosen, so any choice applies.
            let at = if missing.is_some() { Some(0) } else { found.flatten() };
            dd.set_selected(at.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
            let weak = Rc::downgrade(&b);
            let skip = usize::from(missing.is_some());
            let names = fonts.families.clone();
            dd.connect_selected_notify(move |dd| {
                let Some(i) = (dd.selected() as usize).checked_sub(skip) else {
                    return;
                };
                if let (Some(b), Some(f)) = (weak.upgrade(), names.get(i)) {
                    b.set_selected(set(&[("font", if f == "Geist" { Value::Null } else { json!(f) })]));
                }
            });
            sec.append(&dd);
            // The weights the family has faces for (TM-9), and the one it holds even if not.
            let weight = same(nodes.iter().map(|n| ((n.font_weight.unwrap_or(400) + 50) / 100 * 100).clamp(100, 900)));
            let mut weights = current.as_deref().map_or_else(|| WEIGHTS.iter().map(|w| w.0).collect(), |c| fonts.weights(&self.canvas, c));
            if let Some(w) = weight.filter(|w| !weights.contains(w)) {
                weights.push(w);
                weights.sort_unstable();
            }
            let captions: Vec<&str> = weights.iter().map(|w| WEIGHTS.iter().find(|x| x.0 == *w).map_or("Regular", |x| x.1)).collect();
            let wdd = gtk::DropDown::from_strings(&captions);
            wdd.add_css_class("design-dropdown");
            wdd.set_tooltip_text(Some("Weight"));
            wdd.set_selected(weight.and_then(|w| weights.iter().position(|v| *v == w)).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
            let weak = Rc::downgrade(&b);
            wdd.connect_selected_notify(move |dd| {
                if let (Some(b), Some(w)) = (weak.upgrade(), weights.get(dd.selected() as usize)) {
                    b.set_selected(set(&[("font_weight", if *w == 400 { Value::Null } else { json!(w) })]));
                }
            });
            let weak = Rc::downgrade(&b);
            // Captioned, so it scrubs as the other fields do (E-board-7).
            let size_field = stored_number("@text", same(nodes.iter().map(|n| bits(super::paint::font_size(n)))).map(f64::from_bits), "Size", move |v| {
                let v = v.clamp(1.0, 1000.0);
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("font_size", json!(v))]));
                }
                v
            });
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            wdd.set_hexpand(true);
            row.append(&wdd);
            size_field.set_size_request(84, -1);
            size_field.set_hexpand(false);
            row.append(&size_field);
            sec.append(&row);
            let weak = Rc::downgrade(&b);
            let weak2 = Rc::downgrade(&b);
            sec.append(&pair(
                stored_number_step("@line-height", same(nodes.iter().map(|n| bits(n.line_height.unwrap_or(1.3)))).map(f64::from_bits), 0.1, "Line height, times the size", move |v| {
                    let v = v.clamp(0.5, 4.0);
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("line_height", json!(v))]));
                    }
                    v
                }),
                stored_number_step("@letter-spacing", same(nodes.iter().map(|n| bits(n.letter_spacing.unwrap_or(0.0)))).map(f64::from_bits), 0.1, "Letter spacing, px", move |v| {
                    if let Some(b) = weak2.upgrade() {
                        b.set_selected(set(&[("letter_spacing", if v == 0.0 { Value::Null } else { json!(v) })]));
                    }
                    v
                }),
            ));
            let weak = Rc::downgrade(&b);
            // A shape's label is centred until told otherwise; a text starts at the left.
            let shape = |n: &Node| super::textedit::is_shape(n.kind);
            let shapes = nodes.iter().any(shape);
            let align = segmented(
                &[("left", "text-left", "Align left"), ("center", "text-center", "Centre"), ("right", "text-right", "Align right")],
                Some(match same(nodes.iter().map(|n| n.align.unwrap_or(if shape(n) { TextAlign::Center } else { TextAlign::Left }))) {
                    Some(TextAlign::Center) => "center",
                    Some(TextAlign::Right) => "right",
                    Some(TextAlign::Left) => "left",
                    None => "",
                }),
                move |v| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("align", if v == "left" && !shapes { Value::Null } else { json!(v) })]));
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
            let color = same(nodes.iter().map(|n| self::current(&b, &n.id, |s: &Node| s.color.clone()))).flatten();
            let weak = Rc::downgrade(&b);
            sec.append(&labelled("Colour", color_key_named(&b, color.as_deref(), "Text colour", Some("Automatic"), Some("color"), move |c| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("color", json!(c))]));
                }
            })));
            if all_take("max_lines") {
                sec.append(&self.lines_row(&b, &nodes));
            }
            // Parts of the words styled on their own (an agent's runs): said, and clearable.
            let parts: usize = nodes.iter().map(|n| n.runs.as_ref().map_or(0, Vec::len)).sum();
            if parts > 0 {
                sec.append(&parts_row(&b, parts));
            }
            if nodes.iter().all(|n| super::textedit::is_shape(n.kind)) {
                sec.append(&self.inset_row(&b, &nodes));
            }
        }

        // Polygon, icon, connector, path, arc.
        if single && first.kind == NodeKind::Polygon {
            super::shape_panels::polygon(&b, &section(body, "Polygon"), &first);
        }
        if single && first.kind == NodeKind::Icon {
            super::shape_panels::icon(&b, &section(body, "Icon"), &first);
        }
        if single && first.kind == NodeKind::Line {
            super::shape_panels::connector(&b, &section(body, "Line"), &first);
        }
        // Closing a path of one or two points draws nothing (B98).
        if single && first.kind == NodeKind::Path && first.points.as_ref().is_some_and(|p| p.len() > 2) {
            super::shape_panels::path(&b, &section(body, "Path"), &first);
        }
        if single && first.kind == NodeKind::Ellipse {
            super::shape_panels::arc(&b, &section(body, "Arc"), &first);
        }

        // Fill: a line or path only with an inside to fill, three points or more (B98).
        let fills = |n: &Node| takes(n.kind, "fill") && (!matches!(n.kind, NodeKind::Line | NodeKind::Path) || shapes::fillable_line(n));
        if nodes.iter().all(fills) {
            let sec = section(body, "Fill");
            if single {
                super::shape_panels::paint(&b, &sec, &first, Slot::Fill);
            } else {
                // As stored where a layer holds one, so a named colour shows as its name.
                let fill = same(nodes.iter().map(|n| self::current(&b, &n.id, |s: &Node| s.fill.clone()).or_else(|| color::fill_of(n))));
                let weak = Rc::downgrade(&b);
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                // Lines, paths and texts fill with nothing until given a colour.
                let clear = if nodes.iter().all(|n| color::fill_of(n).is_none()) { "None" } else { "Automatic" };
                row.append(&color_key_named(&b, fill.clone().flatten().as_deref(), "Fill", Some(clear), Some("fill"), move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("fill", json!(c)), ("gradient", Value::Null)]));
                    }
                }));
                let caption = match &fill {
                    Some(Some(c)) => c.as_str(),
                    Some(None) => "None",
                    None => "Mixed",
                };
                row.append(&label(caption, "design-hex-caption"));
                sec.append(&row);
            }
        }
        // Stroke or border. An icon's colour and weight are its own section's.
        if all_take("stroke") && !kinds(&[NodeKind::Icon]) {
            let lines = kinds(&[NodeKind::Line, NodeKind::Path]);
            let sec = section(body, if lines { "Stroke" } else { "Border" });
            if single {
                super::shape_panels::paint(&b, &sec, &first, Slot::Stroke);
            } else {
                let stroke = same(nodes.iter().map(|n| self::current(&b, &n.id, |s: &Node| s.stroke.clone()))).flatten();
                let weak = Rc::downgrade(&b);
                sec.append(&labelled("Colour", color_key_named(&b, stroke.as_deref(), "Colour", (!lines).then_some("None"), Some("stroke"), move |c| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("stroke", json!(c)), ("stroke_gradient", Value::Null)]));
                    }
                })));
            }
            let weak = Rc::downgrade(&b);
            let width = same(nodes.iter().map(|n| bits(n.stroke_width.unwrap_or(if lines { 2.0 } else { 1.0 })))).map(f64::from_bits);
            sec.append(&stored_number("@stroke-width", width, "Weight", move |v| {
                let v = v.clamp(0.0, 1000.0);
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_width", json!(v))]));
                }
                v
            }));
            if all_take("dash") {
                sec.append(&super::shape_panels::dash(&b, &nodes));
            }
            super::shape_panels::caps(&b, &sec, &nodes);
            // A closed shape draws no arrowheads; a pen path draws them as a line does (B99).
            if lines && !nodes.iter().all(shapes::closed_shape) {
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
        if all_take("radius") {
            let sec = section(body, "Corners");
            let weak = Rc::downgrade(&b);
            // A sticky's own corners are round (6): a rect at 0 beside one reads Mixed, and 0
            // squares both (B35).
            let radius = same(nodes.iter().map(|n| bits(shapes::radius(n)))).map(f64::from_bits);
            sec.append(&stored_number("@radius", radius, "Corner radius", move |v| {
                let v = v.max(0.0);
                if let Some(b) = weak.upgrade() {
                    let ops = shapes::radius_ops(&b.doc.scene.borrow(), &b.selected(), v);
                    b.commit(ops);
                }
                v
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
        if all_take("shadow") {
            self.shadow_section(&b, &section(body, "Shadow"), &nodes);
        }
        let sec = section(body, "Layer");
        let weak = Rc::downgrade(&b);
        let opacity = same(nodes.iter().map(|n| bits((n.opacity.unwrap_or(1.0) * 100.0).round()))).map(f64::from_bits);
        sec.append(&stored_number("@opacity", opacity, "Opacity, %", move |v| {
            let o = (v / 100.0).clamp(0.0, 1.0);
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("opacity", if o >= 1.0 { Value::Null } else { json!(o) })]));
            }
            o * 100.0
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
        sec.append(&stored_number("@blur", blur, "Layer blur, px", move |v| {
            let v = v.clamp(0.0, 200.0);
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("blur", if v <= 0.0 { Value::Null } else { json!(v) })]));
            }
            v
        }));

        // Export.
        let sec = section(body, "Export");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_homogeneous(true);
        for (format, caption, tip) in [
            (super::export::Format::Png(1.0), "PNG 1×", "A picture at 1×"),
            (super::export::Format::Png(2.0), "PNG 2×", "A picture at 2×, for sharp screens"),
            // Its words are drawn as outlines: they look right anywhere, and are not text (E-export-6).
            (super::export::Format::Svg, "SVG", "Vector, with text drawn as outlines (not editable as text)"),
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
        // Its keys leave the keyboard where it was, so the panel follows what they do (Column
        // shows its gap, Drop its offsets) and the canvas keeps its shortcuts.
        super::board::keep_canvas_focus(body.upcast_ref());
    }

    /// The inspector shows the scene as it is now, unless a field of it is being typed in or
    /// one of its popovers is open.
    pub(super) fn refresh_inspector_values(&self) {
        if !matches!(*self.gesture.borrow(), super::board::Gesture::Idle) {
            return;
        }
        if !self.busy_in(&self.inspector_scroll) {
            self.rebuild_inspector();
        }
    }

    /// A width or height typed in: a fixed size from now on (`edit::size_set`, within its min
    /// and max), the size it shows, so a node a stretch pin sizes is stored at what makes it
    /// show that (`layout::unpinned`). A flat line has none across it to give, and a group's
    /// size is its layers' (B90): both are left alone (B36). Answers the size the selection
    /// shows now, for the field.
    pub(super) fn set_size(&self, axis: &str, v: f64) -> f64 {
        let v = v.max(1.0).round();
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let placed = self.placed();
        let ops: Vec<NodeOp> = ids
            .iter()
            .filter_map(|id| scene.stored(id))
            .filter(|n| n.kind != NodeKind::Group)
            .filter_map(|n| {
                let stored = match shown_rel(&scene, &placed, &n.id) {
                    Some((mut rel, fit)) => {
                        if axis == "w" { rel.w = v } else { rel.h = v }
                        let r = layout::unpinned(n, fit, rel);
                        if axis == "w" { r.w } else { r.h }
                    }
                    None => v,
                };
                Some(NodeOp::Update { id: n.id.clone(), set: edit::size_set(n, axis, stored.round())? })
            })
            .collect();
        drop(scene);
        if !ops.is_empty() {
            self.commit(ops);
        }
        let placed = self.placed();
        same(ids.iter().filter_map(|id| placed.rect(id)).map(|r| bits(if axis == "w" { r.w } else { r.h }.round()))).map_or(v, f64::from_bits)
    }

    /// X or Y typed in: where the selection shows from its parent's corner, stored through its
    /// pins (`layout::unpinned`), so it lands where it was typed whatever its parent's size; a
    /// hidden layer, which shows nowhere, takes it as it is.
    fn set_position(&self, axis: &str, v: f64) -> f64 {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let placed = self.placed();
        let ops: Vec<NodeOp> = ids
            .iter()
            .filter_map(|id| scene.stored(id))
            .map(|n| {
                let stored = match shown_rel(&scene, &placed, &n.id) {
                    Some((mut rel, fit)) => {
                        if axis == "x" { rel.x = v } else { rel.y = v }
                        let r = layout::unpinned(n, fit, rel);
                        if axis == "x" { r.x } else { r.y }
                    }
                    None => v,
                };
                NodeOp::Update { id: n.id.clone(), set: set(&[(axis, json!(stored.round()))]) }
            })
            .collect();
        drop(scene);
        self.commit(ops);
        v
    }

    /// Take the selection out of its frame's auto layout where it shows (`absolute`), its box as
    /// placed and a fill made fixed so nothing moves (`edit::freeze`), or put it back in.
    fn set_absolute(&self, on: bool) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let placed = self.placed();
        let ops: Vec<NodeOp> = ids
            .iter()
            .filter_map(|id| {
                let n = scene.stored(id)?;
                let parent = n.parent.as_deref()?;
                if !on {
                    return n.is_absolute().then(|| NodeOp::Update { id: id.clone(), set: set(&[("absolute", Value::Null)]) });
                }
                if n.is_absolute() {
                    return None;
                }
                let mut s = edit::freeze(&scene, &placed, id, placed.origin(parent), layout::fit_of(&scene, &placed, parent)).unwrap_or_default();
                s.insert("absolute".into(), json!(true));
                Some(NodeOp::Update { id: id.clone(), set: s })
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }

    /// Pin the selection across (`across`) or down to `pin` (`None`: left or top, the default),
    /// stored again through the new pin so nothing moves ([`pin_set`]).
    fn set_pin(&self, across: bool, pin: Option<&'static str>) {
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let placed = self.placed();
        let ops: Vec<NodeOp> = ids.iter().filter_map(|id| Some(NodeOp::Update { id: id.clone(), set: pin_set(&scene, &placed, id, across, pin)? })).collect();
        drop(scene);
        self.commit(ops);
    }

    /// A min or max size (`key`) typed in, or cleared (`None`): [`bound_set`]. Answers what is
    /// stored, for the field.
    fn set_bound(&self, key: &'static str, v: Option<f64>) -> Option<f64> {
        let v = v.map(|v| v.max(0.0).round());
        let ids = self.selected();
        let scene = self.doc.scene.borrow();
        let ops: Vec<NodeOp> = ids
            .iter()
            .filter_map(|id| scene.stored(id))
            .filter_map(|n| Some(NodeOp::Update { id: n.id.clone(), set: bound_set(n, key, v)? }))
            .collect();
        drop(scene);
        self.commit(ops);
        v
    }

    /// The width or height rule chosen: fixed holds the size each layer shows now (TM-8), so
    /// nothing jumps; hug and fill are written as they are.
    fn set_rule(&self, axis: &str, rule: &str) {
        let field = if axis == "w" { "w_sizing" } else { "h_sizing" };
        let ids = self.selected();
        let placed = self.placed();
        let ops: Vec<NodeOp> = ids
            .iter()
            .map(|id| {
                let mut s = set(&[(field, json!(rule))]);
                if let (Some(r), "fixed") = (placed.rect(id), rule) {
                    s.insert(axis.into(), json!((if axis == "w" { r.w } else { r.h }).round()));
                }
                NodeOp::Update { id: id.clone(), set: s }
            })
            .collect();
        self.commit(ops);
    }

    /// The selection's style (reuse §4): a key naming it, or "No style", whose menu gives each of
    /// the design's styles, No style (each layer keeps its look), a new style from the layer,
    /// and for the layer's style, Update it from the layer and Reset the layer to it (both only
    /// while the layer sets its own over it) and Delete it (its layers keep their look).
    fn style_row(&self, b: &Rc<Board>, body: &gtk::Box, ids: &[String]) {
        let scene = self.doc.scene.borrow();
        let names: Vec<String> = scene.styles().styles.keys().cloned().collect();
        let current = same(ids.iter().map(|id| scene.stored(id).and_then(|n| n.style.clone())));
        let first = ids.first().cloned().unwrap_or_default();
        let (updates, resets) = match current.clone().flatten() {
            Some(_) if ids.len() == 1 => (
                !relay_client::design::styles::update_style(&scene, &first).is_empty(),
                !relay_client::design::styles::reset_to_style(&scene, ids).is_empty(),
            ),
            _ => (false, false),
        };
        let users = current.clone().flatten().map_or(0, |name| scene.users(&name).len());
        drop(scene);
        let key = gtk::MenuButton::new();
        key.add_css_class("design-dropdown");
        key.set_label(&match &current {
            Some(Some(name)) => name.clone(),
            Some(None) => "No style".to_string(),
            None => "Mixed styles".to_string(),
        });
        key.set_tooltip_text(Some("Style: a look this layer shares with others; changing it changes them all"));
        key.set_hexpand(true);
        let pop = gtk::Popover::new();
        pop.add_css_class("design-menu");
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let item = |caption: &str, act: MenuAct| {
            let button = gtk::Button::with_label(caption);
            button.add_css_class("design-menu-item");
            let (weak, p) = (Rc::downgrade(b), pop.downgrade());
            button.connect_clicked(move |button| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    act(&b, button);
                }
            });
            menu.append(&button);
        };
        for name in names {
            let n = name.clone();
            item(&name, Box::new(move |b, _| b.set_selected(set(&[("style", json!(n))]))));
        }
        item("No style", Box::new(|b, _| {
            let ops = relay_client::design::styles::detach(&b.doc.scene.borrow(), &b.selected());
            b.commit(ops);
        }));
        if ids.len() == 1 {
            let at = key.downgrade();
            item("New style from this layer…", Box::new(move |b, _| {
                let Some(at) = at.upgrade() else { return };
                super::widgets::ask_name(b, &at, "Name the style", |b, name| {
                    let id = b.selected().first().cloned().unwrap_or_default();
                    let ops = relay_client::design::styles::new_style(&b.doc.scene.borrow(), &id, name);
                    b.commit(ops);
                });
            }));
        }
        if let Some(Some(name)) = current {
            if updates {
                let f = first.clone();
                item(&format!("Update ‹{name}› from this layer"), Box::new(move |b, _| {
                    let ops = relay_client::design::styles::update_style(&b.doc.scene.borrow(), &f);
                    b.commit(ops);
                }));
            }
            if resets {
                item(&format!("Reset to ‹{name}›"), Box::new(|b, _| {
                    let ops = relay_client::design::styles::reset_to_style(&b.doc.scene.borrow(), &b.selected());
                    b.commit(ops);
                }));
            }
            let n = name.clone();
            let caption = format!("Delete ‹{name}› ({users} {} keep {} look)", if users == 1 { "layer" } else { "layers" }, if users == 1 { "its" } else { "their" });
            item(&caption, Box::new(move |b, _| {
                b.commit(vec![NodeOp::DeleteStyle { name: n.clone() }]);
            }));
        }
        pop.set_child(Some(&menu));
        key.set_popover(Some(&pop));
        body.append(&labelled("Style", key));
    }

    /// The most lines a text shows, the rest cut with "…"; empty shows them all.
    fn lines_row(&self, b: &Rc<Board>, nodes: &[Node]) -> gtk::Box {
        let value = same(nodes.iter().map(|n| n.max_lines)).flatten().map(f64::from);
        let weak = Rc::downgrade(b);
        let field = stored_number("Lines", value, "Most lines shown, the rest cut with …", move |v| {
            let v = v.round().clamp(1.0, 1000.0);
            if let Some(b) = weak.upgrade() {
                b.set_max_lines(Some(v as u16));
            }
            v
        });
        if let Some(entry) = field.last_child().and_downcast::<gtk::Entry>() {
            entry.set_placeholder_text(Some(if nodes.iter().all(|n| n.max_lines.is_none()) { "All" } else { "Mixed" }));
            // Emptied and entered, it shows every line again.
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let (weak, e) = (Rc::downgrade(b), entry.downgrade());
            let set_any = nodes.iter().any(|n| n.max_lines.is_some());
            keys.connect_key_pressed(move |_, key, _, _| {
                let enter = matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter);
                match (weak.upgrade(), e.upgrade()) {
                    (Some(b), Some(e)) if enter && set_any && e.text().trim().is_empty() => {
                        b.set_max_lines(None);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            entry.add_controller(keys);
        }
        field.set_hexpand(true);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&field);
        if nodes.iter().any(|n| n.max_lines.is_some()) {
            let all = gtk::Button::with_label("All");
            all.add_css_class("quiet");
            all.set_tooltip_text(Some("Show every line"));
            let weak = Rc::downgrade(b);
            all.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.set_max_lines(None);
                }
            });
            row.append(&all);
        }
        row
    }

    /// Show at most `lines` lines of each selected text holder (`None`: all). A text that hugs
    /// its words is given the width it shows, fixed, in the same edit: it needs one to wrap, and
    /// the engine refuses `max_lines` on a text without one.
    pub(super) fn set_max_lines(&self, lines: Option<u16>) {
        let placed = self.placed();
        let scene = self.doc.scene.borrow();
        let ops: Vec<NodeOp> = self
            .selected()
            .iter()
            .filter_map(|id| scene.stored(id))
            .filter(|n| takes(n.kind, "max_lines"))
            .map(|n| {
                let mut s = set(&[("max_lines", lines.map_or(Value::Null, |v| json!(v)))]);
                if let (Some(_), NodeKind::Text, Sizing::Hug) = (lines, n.kind, n.w_rule()) {
                    let w = placed.rect(&n.id).map_or(n.w, |r| r.w).ceil().max(1.0);
                    s.insert("w_sizing".into(), json!("fixed"));
                    s.insert("w".into(), json!(w));
                }
                NodeOp::Update { id: n.id.clone(), set: s }
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }

    /// A shape's label inset, top and bottom and left and right, as it shows now.
    fn inset_row(&self, b: &Rc<Board>, nodes: &[Node]) -> gtk::Box {
        let placed = self.placed();
        let inset = |n: &Node| n.label_inset(placed.rect(&n.id).map_or(n.w, |r| r.w));
        let field = |at: usize, caption: &str, tip: &str| {
            let value = same(nodes.iter().map(|n| bits(inset(n)[at]))).map(f64::from_bits);
            let weak = Rc::downgrade(b);
            stored_number(caption, value, tip, move |v| {
                let v = v.clamp(0.0, 10_000.0);
                if let Some(b) = weak.upgrade() {
                    b.set_label_inset(at, v);
                }
                v
            })
        };
        pair(field(0, "↕", "Label inset, top and bottom"), field(1, "↔", "Label inset, left and right"))
    }

    /// Set one side of each selected shape's label inset (`at` 0: top and bottom, 1: left and
    /// right), keeping the other as it shows now: `padding: [vertical, horizontal]`, or none
    /// at the default `[0, 12]`.
    pub(super) fn set_label_inset(&self, at: usize, v: f64) {
        let placed = self.placed();
        let ops: Vec<NodeOp> = self
            .selected()
            .iter()
            .filter_map(|id| {
                let edges = current(self, id, |n| Some(n.label_inset(placed.rect(&n.id).map_or(n.w, |r| r.w))))?;
                let mut pad = [edges[0], edges[1]];
                pad[at] = v;
                let value = if pad == [0.0, 12.0] { Value::Null } else { json!(pad) };
                Some(NodeOp::Update { id: id.clone(), set: set(&[("padding", value)]) })
            })
            .collect();
        self.commit(ops);
    }

    /// None, a sticky's own, drop, inner or glow; then each shadow's offset, blur, spread and
    /// colour, one block a shadow, to remove or add to.
    fn shadow_section(&self, b: &Rc<Board>, sec: &gtk::Box, nodes: &[Node]) {
        let glowing = nodes.iter().any(|n| glows(n.kind));
        let mut choices = vec![("none", "None", "No shadow")];
        if nodes.iter().any(|n| n.kind == NodeKind::Sticky) {
            choices.push(("default", "Default", "A sticky's own soft shadow"));
        }
        choices.push(("drop", "Drop", "A drop shadow under it"));
        if !glowing {
            choices.push(("inner", "Inner", "An inner shadow inside its edge"));
        }
        choices.push(("glow", "Glow", "A glow all round it, in its own colour"));
        let look = same(nodes.iter().map(shadow_look));
        let weak = Rc::downgrade(b);
        sec.append(&segmented(&choices, look, move |v| {
            if let Some(b) = weak.upgrade() {
                b.set_shadow_look(v);
            }
        }));
        // A block a shadow, where every node casts the same ones.
        let Some(list) = same(nodes.iter().map(|n| shown_shadows(b, &n.id))) else {
            return;
        };
        let count = list.len();
        for (i, s) in list.into_iter().enumerate() {
            sec.append(&shadow_block(b, i, &s, count, glowing));
        }
        if (1..MAX_SHADOWS).contains(&count) {
            let add = gtk::Button::with_label("Add a shadow");
            add.add_css_class("quiet");
            let weak = Rc::downgrade(b);
            add.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    // Another like the last, further and softer: a layered shadow's next step.
                    b.edit_shadows(|list| list.extend(list.last().cloned().map(|s| Shadow { y: s.y + 4.0, blur: (s.blur * 2.0).min(200.0), ..s })));
                }
            });
            sec.append(&add);
        }
    }

    /// Give the selection a look from the Shadow section's keys: none (a sticky, which casts its
    /// own unless told, is given a clear one, B37), a sticky's own, or one drop, inner or glow
    /// shadow.
    pub(super) fn set_shadow_look(&self, look: &str) {
        let scene = self.doc.scene.borrow();
        let ops: Vec<NodeOp> = self
            .selected()
            .iter()
            .filter_map(|id| scene.node(id))
            .filter(|n| takes(n.kind, "shadow"))
            .map(|n| {
                let list = match look {
                    "none" if n.kind == NodeKind::Sticky => vec![Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: "#00000000".into(), inset: false }],
                    "drop" => vec![Shadow { x: 0.0, y: 4.0, blur: 12.0, spread: 0.0, color: "#00000033".into(), inset: false }],
                    "inner" if !glows(n.kind) => vec![Shadow { x: 0.0, y: 2.0, blur: 6.0, spread: 0.0, color: "#00000040".into(), inset: true }],
                    "glow" => vec![glow_of(&scene, n)],
                    _ => Vec::new(),
                };
                NodeOp::Update { id: n.id.clone(), set: write_shadows(&list) }
            })
            .collect();
        drop(scene);
        self.commit(ops);
    }

    /// Change each selected node's shadows as they are now (`shown_shadows`) with `change`.
    pub(super) fn edit_shadows(&self, change: impl Fn(&mut Vec<Shadow>)) {
        let ids: Vec<String> = self.selected().into_iter().filter(|id| self.doc.scene.borrow().node(id).is_some_and(|n| takes(n.kind, "shadow"))).collect();
        let ops: Vec<NodeOp> = ids
            .into_iter()
            .map(|id| {
                let mut list = shown_shadows(self, &id);
                change(&mut list);
                NodeOp::Update { id, set: write_shadows(&list) }
            })
            .collect();
        self.commit(ops);
    }

    /// Change the `i`th shadow of each selected node, as it is now.
    pub(super) fn patch_shadow(&self, i: usize, change: impl Fn(&mut Shadow)) {
        self.edit_shadows(|list| {
            if let Some(s) = list.get_mut(i) {
                change(s);
            }
        });
    }
}

/// One shadow's fields: its offset, blur, spread (not for what casts without one), colour,
/// whether it is inner (among several), and a key to remove it.
fn shadow_block(b: &Rc<Board>, i: usize, s: &Shadow, count: usize, glowing: bool) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
    block.add_css_class("design-shadow-block");
    let field = |caption: &str, value: f64, tip: &str, key: &'static str| {
        let weak = Rc::downgrade(b);
        stored_number(caption, Some(value), tip, move |v| {
            let v = if key == "blur" { v.clamp(0.0, relay_bus::design::MAX_SHADOW_BLUR) } else { v.clamp(-10_000.0, 10_000.0) };
            if let Some(b) = weak.upgrade() {
                b.patch_shadow(i, |s| match key {
                    "x" => s.x = v,
                    "y" => s.y = v,
                    "blur" => s.blur = v,
                    _ => s.spread = v,
                });
            }
            v
        })
    };
    block.append(&pair(field("X", s.x, "Offset across", "x"), field("Y", s.y, "Offset down", "y")));
    let blur = field("@blur", s.blur, "Blur", "blur");
    // Lines, paths, text and icons cast without spread.
    if glowing {
        block.append(&blur);
    } else {
        block.append(&pair(blur, field("Spread", s.spread, "Spread", "spread")));
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let caption = label("Colour", "design-row-caption");
    caption.set_hexpand(true);
    row.append(&caption);
    if count > 1 && !glowing {
        let inner = gtk::CheckButton::with_label("Inner");
        inner.set_active(s.inset);
        let weak = Rc::downgrade(b);
        inner.connect_toggled(move |c| {
            let on = c.is_active();
            if let Some(b) = weak.upgrade() {
                b.patch_shadow(i, |s| s.inset = on);
            }
        });
        row.append(&inner);
    }
    let weak = Rc::downgrade(b);
    row.append(&color_key(b, Some(&s.color), "Shadow colour", None, move |c| {
        if let (Some(b), Some(c)) = (weak.upgrade(), c) {
            b.patch_shadow(i, |s| s.color = c.clone());
        }
    }));
    let remove = crate::app::icon_button("minus", "Remove this shadow");
    let weak = Rc::downgrade(b);
    remove.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            b.edit_shadows(|list| {
                if i < list.len() {
                    list.remove(i);
                }
            });
        }
    });
    row.append(&remove);
    block.append(&row);
    block
}

/// "N parts styled on their own", with a key that gives every part the text's own style.
fn parts_row(b: &Rc<Board>, parts: usize) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let words = if parts == 1 { "1 part styled on its own".to_string() } else { format!("{parts} parts styled on their own") };
    let note = label(&words, "design-note");
    note.set_hexpand(true);
    note.set_wrap(true);
    row.append(&note);
    let clear = gtk::Button::with_label("Clear");
    clear.add_css_class("quiet");
    clear.set_tooltip_text(Some("Every part in the text's own style"));
    let weak = Rc::downgrade(b);
    clear.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("runs", Value::Null)]));
        }
    });
    row.append(&clear);
    row
}

/// A blend mode as the menu names it: "Color dodge".
fn blend_caption(m: Blend) -> String {
    let name = m.name().replace('_', " ");
    let mut chars = name.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The update that pins layer `id` across (`across`) or down to `pin` (`None`: left or top, the
/// default) with nothing moving: its box stored again through the new pin (`layout::unpinned`),
/// and a stretch on an axis that hugs or fills made fixed at the size it shows. `None` for a
/// layer not placed.
fn pin_set(scene: &Scene, placed: &layout::Placed, id: &str, across: bool, pin: Option<&str>) -> Option<Map<String, Value>> {
    let n = scene.stored(id)?;
    let (rel, fit) = shown_rel(scene, placed, id)?;
    let (pin_key, pos, len, rule) = if across { ("pin_x", "x", "w", "w_sizing") } else { ("pin_y", "y", "h", "h_sizing") };
    let value = pin.map(Value::from).unwrap_or(Value::Null);
    let mut s = set(&[(pin_key, value.clone())]);
    let mut pinned = n.patched(&set(&[(pin_key, value)])).ok()?;
    // Both sides and scale change its length with its frame's.
    let stretch = (if across { pinned.pins().0 } else { pinned.pins().1 }).stretches();
    if stretch && (if across { pinned.w_rule() } else { pinned.h_rule() }) != Sizing::Fixed {
        s.insert(rule.into(), json!("fixed"));
        if across { pinned.w_sizing = Some(Sizing::Fixed) } else { pinned.h_sizing = Some(Sizing::Fixed) }
    }
    let r = layout::unpinned(&pinned, fit, rel);
    // Not to whole pixels: a scale pin stores a share of what shows, and rounding it would move it.
    let fine = |v: f64| (v * 1e4).round() / 1e4;
    let (at, long, was_at, was_long) = if across { (r.x, r.w, n.x, n.w) } else { (r.y, r.h, n.y, n.h) };
    if fine(at) != was_at {
        s.insert(pos.into(), json!(fine(at)));
    }
    // Leaving a pin that sized it rewrites its length as much as taking one.
    let sized = stretch || (if across { n.pins().0 } else { n.pins().1 }).stretches();
    if sized && fine(long) != was_long {
        s.insert(len.into(), json!(fine(long).max(0.0)));
    }
    Some(s)
}

/// The update that sets `n`'s min or max size `key` to `v` (`None` unsets it), and brings a
/// fixed size it rules out within it; `None` when the bound does not read.
fn bound_set(n: &Node, key: &str, v: Option<f64>) -> Option<Map<String, Value>> {
    let mut s = set(&[(key, v.map(Value::from).unwrap_or(Value::Null))]);
    // Read with the new bound but not checked, since the size it rules out is fixed below.
    let mut fields = n.to_map();
    fields.insert(key.into(), s[key].clone());
    let bounded = Node::read(Value::Object(fields)).ok()?;
    let (w, h) = (bounded.clamp_w(n.w), bounded.clamp_h(n.h));
    if n.w_rule() == Sizing::Fixed && w != n.w {
        s.insert("w".into(), json!(w));
    }
    if n.h_rule() == Sizing::Fixed && h != n.h {
        s.insert("h".into(), json!(h));
    }
    n.patched(&s).ok().map(|_| s)
}

/// What a menu item of the inspector does when chosen.
type MenuAct = Box<dyn Fn(&Rc<Board>, &gtk::Button)>;

/// Where layer `id` shows from its parent's corner (its own point, its placed size), and how its
/// parent is drawn against how it is stored; `None` for what is not placed, or sits in no parent.
fn shown_rel(scene: &Scene, placed: &layout::Placed, id: &str) -> Option<(Rect, Option<layout::Fit>)> {
    let parent = scene.parent(id)?;
    let r = placed.rect(id)?;
    let (px, py) = placed.point(id);
    let (ox, oy) = placed.origin(parent);
    Some((Rect::new(px - ox, py - oy, r.w, r.h), layout::fit_of(scene, placed, parent)))
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

/// The font families the font menu offers, and the weights of those it has shown: listed once,
/// and again only when fontconfig says the installed fonts changed, not on every rebuild of the
/// inspector (I-16).
struct Fonts {
    families: Vec<String>,
    weights: RefCell<HashMap<String, Vec<u16>>>,
}

impl Fonts {
    /// The weights `family` has faces for, to the nearest hundred, lightest first; every weight
    /// for a family this machine does not have (TM-9).
    fn weights(&self, widget: &impl IsA<gtk::Widget>, family: &str) -> Vec<u16> {
        let key = family.to_lowercase();
        if let Some(w) = self.weights.borrow().get(&key) {
            return w.clone();
        }
        let faces = widget.as_ref().pango_context().list_families().into_iter().find(|f| f.name().eq_ignore_ascii_case(family)).map(|f| f.list_faces());
        let mut weights: Vec<u16> = faces.unwrap_or_default().iter().map(|face| ((face.describe().weight().into_glib().clamp(100, 900) + 50) / 100 * 100) as u16).collect();
        weights.sort_unstable();
        weights.dedup();
        if weights.is_empty() {
            weights = WEIGHTS.iter().map(|w| w.0).collect();
        }
        self.weights.borrow_mut().insert(key, weights.clone());
        weights
    }
}

thread_local! {
    static FONTS: RefCell<Option<Rc<Fonts>>> = const { RefCell::new(None) };
    static WATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn fonts(widget: &impl IsA<gtk::Widget>) -> Rc<Fonts> {
    if let Some(fonts) = FONTS.with(|f| f.borrow().clone()) {
        return fonts;
    }
    let listed: Vec<String> = widget.as_ref().pango_context().list_families().iter().map(|f| f.name().to_string()).collect();
    let fonts = Rc::new(Fonts { families: family_order(listed), weights: RefCell::default() });
    FONTS.with(|f| *f.borrow_mut() = Some(fonts.clone()));
    // Fonts installed or removed while Relay runs are listed again the next time.
    if !WATCHING.with(|w| w.replace(true)) {
        if let Some(settings) = gtk::Settings::default() {
            settings.connect_gtk_fontconfig_timestamp_notify(|_| {
                FONTS.with(|f| f.borrow_mut().take());
            });
        }
    }
    fonts
}

/// The families the font menu lists: Relay's bundled faces first, then the rest, each once, in
/// the order a person reads names in (`name_key`: "École" before "Zebra", B104), then by name,
/// so names that differ only in accents keep one order.
fn family_order(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_cached_key(|n| (name_key(n), n.clone()));
    names.dedup();
    let mut out: Vec<String> = ["Geist", "Geist Mono", "Sora"].iter().map(|s| s.to_string()).collect();
    out.extend(names.into_iter().filter(|n| !matches!(n.as_str(), "Geist" | "Geist Mono" | "Sora")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_menu_reads_names_as_people_do() {
        let names = ["Zebra Sans", "École", "apple", "Été Serif", "banana", "Geist", "Sora"].map(String::from).to_vec();
        assert_eq!(family_order(names), ["Geist", "Geist Mono", "Sora", "apple", "banana", "École", "Été Serif", "Zebra Sans"]);
    }

    #[test]
    fn one_shadow_is_written_as_shadow_and_more_as_shadows() {
        let s = |y: f64| Shadow { x: 0.0, y, blur: 4.0, spread: 0.0, color: "#00000033".into(), inset: false };
        let none = write_shadows(&[]);
        assert_eq!((&none["shadow"], &none["shadows"]), (&Value::Null, &Value::Null));
        let one = write_shadows(&[s(2.0)]);
        assert_eq!((&one["shadow"]["y"], &one["shadows"]), (&json!(2.0), &Value::Null));
        let two = write_shadows(&[s(2.0), s(8.0)]);
        assert_eq!((&two["shadow"], &two["shadows"][1]["y"]), (&Value::Null, &json!(8.0)));
    }

    #[test]
    fn a_sticky_reads_default_until_told_otherwise() {
        let mut sticky = Node::new(NodeKind::Sticky);
        assert_eq!(shadow_look(&sticky), "default");
        // None is a clear shadow on a sticky, which draws nothing (B37).
        sticky.shadow = Some(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: "#00000000".into(), inset: false });
        assert_eq!(shadow_look(&sticky), "none");
        let mut rect = Node::new(NodeKind::Rect);
        assert_eq!(shadow_look(&rect), "none");
        rect.shadow = Some(Shadow { x: 0.0, y: 0.0, blur: 16.0, spread: 2.0, color: "#4C9DFFCC".into(), inset: false });
        assert_eq!(shadow_look(&rect), "glow");
        rect.shadow = Some(Shadow { x: 0.0, y: 2.0, blur: 6.0, spread: 0.0, color: "#00000040".into(), inset: true });
        assert_eq!(shadow_look(&rect), "inner");
        if let Some(s) = rect.shadow.as_mut() {
            s.inset = false;
        }
        assert_eq!(shadow_look(&rect), "drop");
    }

    #[test]
    fn a_new_pin_moves_nothing() {
        use relay_client::design::layout::{place, Measure};
        // No text here: nothing to measure.
        struct Mono;
        impl Measure for Mono {
            fn measure(&self, _: &Node, _: Option<f64>) -> (f64, f64) {
                (0.0, 0.0)
            }
        }
        let node = |id: &str, parent: Option<&str>, kind: NodeKind, r: [f64; 4]| {
            let mut n = Node::new(kind);
            (n.id, n.parent, n.x, n.y, n.w, n.h) = (id.into(), parent.map(Into::into), r[0], r[1], r[2], r[3]);
            n
        };
        // A column that fills a row is drawn 100 wider than it stores, so its children's pins act.
        let mut row = node("row", Some("p1"), NodeKind::Frame, [0.0, 0.0, 300.0, 200.0]);
        row.layout = Some(Layout::Row);
        let mut col = node("col", Some("row"), NodeKind::Frame, [0.0, 0.0, 200.0, 200.0]);
        col.w_sizing = Some(Sizing::Fill);
        let mut scene = Scene::new(vec![node("p1", None, NodeKind::Page, [0.0; 4]), row, col, node("key", Some("col"), NodeKind::Rect, [20.0, 10.0, 140.0, 20.0])]);
        let shown = |s: &Scene| place(s, "p1", &Mono).rect("key").unwrap();
        let before = shown(&scene);
        for pin in [Some("right"), Some("center"), Some("left_right"), Some("scale"), None] {
            let set = pin_set(&scene, &place(&scene, "p1", &Mono), "key", true, pin).unwrap();
            scene.apply_all(&[NodeOp::Update { id: "key".into(), set }]).unwrap();
            let now = shown(&scene);
            let near = |a: f64, b: f64| (a - b).abs() < 0.01;
            assert!(near(now.x, before.x) && near(now.y, before.y) && near(now.w, before.w) && near(now.h, before.h), "pinned {pin:?}: {now:?}");
        }
    }

    #[test]
    fn a_bound_brings_a_fixed_size_within_it() {
        let mut n = Node::new(NodeKind::Rect);
        (n.parent, n.w, n.h) = (Some("p1".into()), 300.0, 50.0);
        assert_eq!(bound_set(&n, "max_w", Some(200.0)).unwrap(), set(&[("max_w", json!(200.0)), ("w", json!(200.0))]));
        assert_eq!(bound_set(&n, "min_h", Some(80.0)).unwrap(), set(&[("min_h", json!(80.0)), ("h", json!(80.0))]));
        assert_eq!(bound_set(&n, "min_w", None).unwrap(), set(&[("min_w", Value::Null)]));
    }

    #[test]
    fn a_glow_takes_the_nodes_own_colour() {
        let scene = Scene::new(Vec::new());
        let mut line = Node::new(NodeKind::Line);
        line.stroke = Some("#FF3B30".into());
        let g = glow_of(&scene, &line);
        assert_eq!((g.color.as_str(), g.spread), ("#FF3B30CC", 0.0));
        let mut rect = Node::new(NodeKind::Rect);
        rect.fill = Some("#34C759".into());
        let g = glow_of(&scene, &rect);
        assert_eq!((g.color.as_str(), g.spread, g.x, g.y), ("#34C759CC", 2.0, 0.0, 0.0));
        assert_eq!(with_alpha("#12345678", 0x80), "#12345680");
    }
}
