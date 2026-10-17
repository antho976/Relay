//! The inspector's panels for the later properties: shared styles, constraints, mirroring,
//! layout grids, a picture's crop and colour adjustments, a picture as a shape's fill, lists
//! and links, a vector's path, and prototype links.

use super::board::{display_name, Board};
use super::widgets::{number, segmented};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Filters, GridKind, ListStyle, Node, NodeKind, Pin, Transition};
use serde_json::{json, Map, Value};
use std::cell::Cell;
use std::rc::Rc;

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

fn same<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

/// A titled group, as the inspector's own.
pub fn section(body: &gtk::Box, title: &str) -> gtk::Box {
    let sec = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sec.add_css_class("design-section");
    sec.append(&label(title, "design-section-title"));
    body.append(&sec);
    sec
}

fn row(caption: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let l = label(caption, "design-row-caption");
    l.set_hexpand(true);
    l.set_xalign(0.0);
    row.append(&l);
    row.append(widget);
    row
}

fn dropdown(names: &[&str], current: Option<usize>, tip: &str, pick: impl Fn(usize) + 'static) -> gtk::DropDown {
    let dd = gtk::DropDown::from_strings(names);
    dd.add_css_class("design-dropdown");
    dd.set_tooltip_text(Some(tip));
    dd.set_selected(current.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
    dd.connect_selected_notify(move |dd| {
        if dd.selected() != gtk::INVALID_LIST_POSITION {
            pick(dd.selected() as usize);
        }
    });
    dd
}

/// A slider that sends one edit when it comes to rest, not one per step.
fn slider(caption: &str, value: f64, range: (f64, f64), step: f64, tip: &str, commit: impl Fn(f64) + 'static) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_tooltip_text(Some(tip));
    let l = label(caption, "design-row-caption");
    l.set_width_chars(10);
    l.set_xalign(0.0);
    row.append(&l);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, range.0, range.1, step);
    scale.set_value(value);
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    scale.add_css_class("design-slider");
    let shown = label(&format!("{}", value.round()), "design-hex-caption");
    shown.set_width_chars(4);
    shown.set_xalign(1.0);
    let serial = Rc::new(Cell::new(0u64));
    let commit = Rc::new(commit);
    let s2 = shown.downgrade();
    scale.connect_value_changed(move |s| {
        let v = s.value();
        if let Some(l) = s2.upgrade() {
            l.set_text(&format!("{}", v.round()));
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let commit = commit.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(260), move || {
            if serial.get() == n {
                commit(v);
            }
        });
    });
    row.append(&scale);
    row.append(&shown);
    row
}

const PINS: [(Pin, &str, &str); 5] = [
    (Pin::Start, "Left", "Top"),
    (Pin::End, "Right", "Bottom"),
    (Pin::Both, "Left and right", "Top and bottom"),
    (Pin::Center, "Centre", "Centre"),
    (Pin::Scale, "Scale", "Scale"),
];

fn pin_name(p: Pin) -> &'static str {
    match p {
        Pin::Start => "start",
        Pin::End => "end",
        Pin::Both => "both",
        Pin::Center => "center",
        Pin::Scale => "scale",
    }
}

const TRANSITIONS: [(Transition, &str); 6] = [
    (Transition::Dissolve, "Dissolve"),
    (Transition::Instant, "Instant"),
    (Transition::SlideLeft, "Slide in from the right"),
    (Transition::SlideRight, "Slide in from the left"),
    (Transition::SlideUp, "Slide up"),
    (Transition::SlideDown, "Slide down"),
];

impl Board {
    /// Mirror keys, beside rotation.
    pub(super) fn flip_keys(self: &Rc<Self>, sec: &gtk::Box, nodes: &[Node]) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for (field, glyph, tip) in [("flip_x", "flip-h", "Flip horizontal (Shift+H)"), ("flip_y", "flip-v", "Flip vertical (Shift+V)")] {
            let on = nodes.iter().all(|n| if field == "flip_x" { n.flip_x == Some(true) } else { n.flip_y == Some(true) });
            let key = gtk::ToggleButton::new();
            key.add_css_class("design-context-key");
            key.set_child(Some(&super::glyphs::image(glyph, 14)));
            key.set_tooltip_text(Some(tip));
            key.set_active(on);
            let weak = Rc::downgrade(self);
            key.connect_toggled(move |k| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[(field, if k.is_active() { json!(true) } else { Value::Null })]));
                }
            });
            row.append(&key);
        }
        sec.append(&row);
    }

    /// How the selection keeps to its frame when the frame is resized.
    pub(super) fn constraints_panel(self: &Rc<Self>, body: &gtk::Box, nodes: &[Node]) {
        let sec = section(body, "Constraints");
        for (axis, vertical) in [("h", false), ("v", true)] {
            let current = same(nodes.iter().map(|n| n.constraints.map_or(Pin::Start, |c| if vertical { c.v } else { c.h })));
            let names: Vec<&str> = PINS.iter().map(|p| if vertical { p.2 } else { p.1 }).collect();
            let weak = Rc::downgrade(self);
            let dd = dropdown(&names, current.and_then(|c| PINS.iter().position(|p| p.0 == c)), if vertical { "Vertical constraint" } else { "Horizontal constraint" }, move |i| {
                let Some(b) = weak.upgrade() else { return };
                let pin = PINS[i].0;
                let ops = {
                    let scene = b.doc.scene.borrow();
                    b.selected()
                        .iter()
                        .filter_map(|id| scene.node(id))
                        .map(|n| {
                            let mut c = n.constraints.unwrap_or(relay_bus::design::Constraints { h: Pin::Start, v: Pin::Start });
                            if vertical { c.v = pin } else { c.h = pin }
                            let value = if c.h == Pin::Start && c.v == Pin::Start { Value::Null } else { json!({"h": pin_name(c.h), "v": pin_name(c.v)}) };
                            relay_bus::design::NodeOp::Update { id: n.id.clone(), set: set(&[("constraints", value)]) }
                        })
                        .collect()
                };
                b.commit(ops);
            });
            dd.set_hexpand(true);
            sec.append(&row(if vertical { "Vertical" } else { "Horizontal" }, &dd));
            let _ = axis;
        }
    }

    /// A frame's layout grids: each one's kind and measures, and adding another.
    pub(super) fn grids_panel(self: &Rc<Self>, body: &gtk::Box, n: &Node) {
        let sec = section(body, "Layout grids");
        let grids = n.grids.clone().unwrap_or_default();
        let write = {
            let weak = Rc::downgrade(self);
            Rc::new(move |g: Vec<relay_bus::design::Grid>| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("grids", if g.is_empty() { Value::Null } else { json!(g) })]));
                }
            })
        };
        for (i, g) in grids.iter().enumerate() {
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let kinds = [(GridKind::Columns, "Columns"), (GridKind::Rows, "Rows"), (GridKind::Square, "Grid")];
            let w = write.clone();
            let all = grids.clone();
            let dd = dropdown(&kinds.iter().map(|k| k.1).collect::<Vec<_>>(), kinds.iter().position(|k| k.0 == g.kind), "Grid kind", move |k| {
                let mut all = all.clone();
                all[i].kind = kinds[k].0;
                w(all);
            });
            dd.set_hexpand(true);
            head.append(&dd);
            let eye = gtk::ToggleButton::new();
            eye.add_css_class("design-context-key");
            eye.set_child(Some(&super::glyphs::image(if g.hidden == Some(true) { "eye-off" } else { "eye" }, 14)));
            eye.set_tooltip_text(Some("Show or hide this grid"));
            let w = write.clone();
            let all = grids.clone();
            eye.connect_clicked(move |_| {
                let mut all = all.clone();
                all[i].hidden = if all[i].hidden == Some(true) { None } else { Some(true) };
                w(all);
            });
            head.append(&eye);
            let remove = crate::app::icon_button("minus", "Remove this grid");
            let w = write.clone();
            let all = grids.clone();
            remove.connect_clicked(move |_| {
                let mut all = all.clone();
                all.remove(i);
                w(all);
            });
            head.append(&remove);
            sec.append(&head);
            let field = |cap: &str, value: Option<f64>, tip: &str, put: fn(&mut relay_bus::design::Grid, f64)| {
                let w = write.clone();
                let all = grids.clone();
                number(cap, value, tip, move |v| {
                    let mut all = all.clone();
                    put(&mut all[i], v.max(0.0));
                    w(all);
                })
            };
            let measures = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            measures.set_homogeneous(true);
            if g.kind == GridKind::Square {
                measures.append(&field("Size", Some(g.size.unwrap_or(8.0)), "Cell size", |g, v| g.size = Some(v.max(1.0))));
            } else {
                let rows = g.kind == GridKind::Rows;
                measures.append(&field("N", Some(g.count.unwrap_or(if rows { 4 } else { 12 }) as f64), "How many", |g, v| g.count = Some(v.round().clamp(1.0, 200.0) as u32)));
                measures.append(&field("Gap", Some(g.gutter.unwrap_or(if rows { 16.0 } else { 20.0 })), "Gutter", |g, v| g.gutter = Some(v)));
                measures.append(&field("Edge", Some(g.margin.unwrap_or(if rows { 0.0 } else { 24.0 })), "Margin", |g, v| g.margin = Some(v)));
            }
            sec.append(&measures);
        }
        let add = gtk::Button::with_label("Add a layout grid");
        add.add_css_class("quiet");
        let w = write.clone();
        add.connect_clicked(move |_| {
            let mut all = grids.clone();
            let kind = if all.iter().any(|g| g.kind == GridKind::Columns) { GridKind::Rows } else { GridKind::Columns };
            all.push(relay_bus::design::Grid { kind, count: None, size: None, gutter: None, margin: None, color: None, hidden: None });
            w(all);
        });
        sec.append(&add);
    }

    /// A picture's crop, a shape's picture fill, and colour adjustments for anything.
    pub(super) fn picture_panel(self: &Rc<Self>, body: &gtk::Box, n: &Node) {
        let pictured = n.kind == NodeKind::Image || n.src.is_some();
        if pictured {
            let sec = section(body, "Crop");
            let [cx, cy, cw, _ch] = n.crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let zoom = (1.0 / cw.max(0.01) * 100.0).round();
            let free = |c: f64| if cw >= 0.999 { 50.0 } else { (c / (1.0 - cw).max(1e-6) * 100.0).round() };
            let crop = {
                let weak = Rc::downgrade(self);
                let cur = [cx, cy, cw];
                Rc::new(move |z: Option<f64>, px: Option<f64>, py: Option<f64>| {
                    let Some(b) = weak.upgrade() else { return };
                    let w = z.map_or(cur[2], |z| (100.0 / z.max(100.0)).clamp(0.05, 1.0));
                    let room = 1.0 - w;
                    let fx = px.map_or(if cur[2] >= 0.999 { 0.5 } else { cur[0] / (1.0 - cur[2]).max(1e-6) }, |p| p / 100.0);
                    let fy = py.map_or(if cur[2] >= 0.999 { 0.5 } else { cur[1] / (1.0 - cur[2]).max(1e-6) }, |p| p / 100.0);
                    let value = if w >= 0.999 { Value::Null } else { json!([room * fx.clamp(0.0, 1.0), room * fy.clamp(0.0, 1.0), w, w]) };
                    b.set_selected(set(&[("crop", value)]));
                })
            };
            let c = crop.clone();
            sec.append(&slider("Zoom", zoom, (100.0, 500.0), 5.0, "How far in the picture is cropped, %", move |v| c(Some(v), None, None)));
            let c = crop.clone();
            sec.append(&slider("Across", free(cx), (0.0, 100.0), 1.0, "Which part shows, left to right", move |v| c(None, Some(v), None)));
            let c = crop.clone();
            sec.append(&slider("Down", free(cy), (0.0, 100.0), 1.0, "Which part shows, top to bottom", move |v| c(None, None, Some(v))));
            if n.kind != NodeKind::Image {
                let clear = gtk::Button::with_label("Remove the picture fill");
                clear.add_css_class("quiet");
                let weak = Rc::downgrade(self);
                clear.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.set_selected(set(&[("src", Value::Null), ("crop", Value::Null)]));
                    }
                });
                sec.append(&clear);
            }
        }
        let sec = section(body, "Colour adjustments");
        let f = n.filters.clone().unwrap_or_default();
        let write = {
            let weak = Rc::downgrade(self);
            let f = f.clone();
            Rc::new(move |put: &dyn Fn(&mut Filters)| {
                if let Some(b) = weak.upgrade() {
                    let mut f = f.clone();
                    put(&mut f);
                    b.set_selected(set(&[("filters", if f.is_none() { Value::Null } else { json!(f) })]));
                }
            })
        };
        let unit = |v: f64| (v * 100.0).round();
        type Put = fn(&mut Filters, f64);
        type Slider<'a> = (&'a str, f64, (f64, f64), &'a str, Put);
        let sliders: [Slider; 6] = [
            ("Brightness", unit(f.brightness), (-100.0, 100.0), "Darker or lighter", |f, v| f.brightness = v / 100.0),
            ("Contrast", unit(f.contrast), (-100.0, 100.0), "Flatter or punchier", |f, v| f.contrast = v / 100.0),
            ("Saturation", unit(f.saturation), (-100.0, 100.0), "Greyer or more vivid", |f, v| f.saturation = v / 100.0),
            ("Hue", f.hue.round(), (-180.0, 180.0), "Turn the colours round the wheel, degrees", |f, v| f.hue = v),
            ("Sepia", unit(f.sepia), (0.0, 100.0), "Toward an old photo", |f, v| f.sepia = v / 100.0),
            ("Invert", unit(f.invert), (0.0, 100.0), "Toward the negative", |f, v| f.invert = v / 100.0),
        ];
        for (cap, value, range, tip, put) in sliders {
            let w = write.clone();
            sec.append(&slider(cap, value, range, 1.0, tip, move |v| w(&|f: &mut Filters| put(f, v))));
        }
        if !f.is_none() {
            let reset = gtk::Button::with_label("Reset adjustments");
            reset.add_css_class("quiet");
            let w = write.clone();
            reset.connect_clicked(move |_| w(&|f: &mut Filters| *f = Filters::default()));
            sec.append(&reset);
        }
    }

    /// A shape filled with a picture instead of a colour.
    pub(super) fn picture_fill_key(self: &Rc<Self>, sec: &gtk::Box, n: &Node) {
        if n.src.is_some() {
            return;
        }
        let key = gtk::Button::with_label("Fill with a picture…");
        key.add_css_class("quiet");
        let weak = Rc::downgrade(self);
        let id = n.id.clone();
        key.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                super::assets::replace_image(&b, &id);
            }
        });
        sec.append(&key);
    }

    /// Lists and a link, for a text.
    pub(super) fn text_extras(self: &Rc<Self>, sec: &gtk::Box, nodes: &[Node]) {
        let current = same(nodes.iter().map(|n| n.list));
        let weak = Rc::downgrade(self);
        sec.append(&segmented(
            &[("none", "text-left", "No list"), ("bullet", "list-bullet", "Bullets"), ("number", "list-number", "Numbered"), ("check", "list-check", "Checklist (start a line with [x] to tick it)")],
            Some(match current {
                Some(None) => "none",
                Some(Some(ListStyle::Bullet)) => "bullet",
                Some(Some(ListStyle::Number)) => "number",
                Some(Some(ListStyle::Check)) => "check",
                None => "",
            }),
            move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("list", if v == "none" { Value::Null } else { json!(v) })]));
                }
            },
        ));
        let link = gtk::Entry::new();
        link.set_placeholder_text(Some("Link: https://…"));
        link.add_css_class("design-name");
        link.set_text(same(nodes.iter().map(|n| n.link.clone())).flatten().as_deref().unwrap_or(""));
        let weak = Rc::downgrade(self);
        link.connect_activate(move |e| {
            let url = e.text().trim().to_string();
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("link", if url.is_empty() { Value::Null } else { json!(url) })]));
            }
        });
        sec.append(&link);
        let hint = label("While typing: Ctrl+B, I, U, E (code), K (link), Shift+Ctrl+X (strike) style the selected words", "design-note");
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        sec.append(&hint);
    }

    /// A vector's path: open or closed, and its points.
    pub(super) fn vector_panel(self: &Rc<Self>, body: &gtk::Box, n: &Node) {
        let sec = section(body, "Vector");
        let open = gtk::CheckButton::with_label("Open path (a line, not a shape)");
        open.set_active(n.open == Some(true));
        let weak = Rc::downgrade(self);
        open.connect_toggled(move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("open", if c.is_active() { json!(true) } else { Value::Null })]));
            }
        });
        sec.append(&open);
        let edit = gtk::Button::with_label("Edit points");
        edit.add_css_class("quiet");
        edit.set_tooltip_text(Some("Drag the anchors and handles (or double-click the vector)"));
        let weak = Rc::downgrade(self);
        let id = n.id.clone();
        edit.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.edit_points(&id);
            }
        });
        sec.append(&edit);
    }

    /// Prototype: where a click goes when presenting, how, and where presenting starts.
    pub(super) fn prototype_panel(self: &Rc<Self>, body: &gtk::Box, n: &Node) {
        let sec = section(body, "Prototype");
        let page = self.page.borrow().clone();
        let frames: Vec<(String, String)> = {
            let scene = self.doc.scene.borrow();
            scene
                .children(&page)
                .iter()
                .filter(|id| **id != n.id)
                .filter_map(|id| scene.node(id).filter(|f| f.kind == NodeKind::Frame).map(|f| (id.clone(), display_name(f))))
                .collect()
        };
        let mut names = vec!["Nowhere".to_string()];
        names.extend(frames.iter().map(|f| f.1.clone()));
        let current = n.goto.as_ref().map_or(Some(0), |g| frames.iter().position(|f| f.0 == *g).map(|i| i + 1));
        let weak = Rc::downgrade(self);
        let ids: Vec<String> = frames.iter().map(|f| f.0.clone()).collect();
        let dd = dropdown(&names.iter().map(String::as_str).collect::<Vec<_>>(), current, "The frame a click on this goes to when presenting", move |i| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("goto", if i == 0 { Value::Null } else { json!(ids[i - 1]) })]));
            }
        });
        dd.set_hexpand(true);
        sec.append(&row("On click, go to", &dd));
        if n.goto.is_some() {
            let weak = Rc::downgrade(self);
            let t = dropdown(
                &TRANSITIONS.iter().map(|t| t.1).collect::<Vec<_>>(),
                TRANSITIONS.iter().position(|t| t.0 == n.transition.unwrap_or(Transition::Dissolve)),
                "How it gets there",
                move |i| {
                    if let Some(b) = weak.upgrade() {
                        let v = serde_json::to_value(TRANSITIONS[i].0).unwrap_or(Value::Null);
                        b.set_selected(set(&[("transition", if TRANSITIONS[i].0 == Transition::Dissolve { Value::Null } else { v })]));
                    }
                },
            );
            t.set_hexpand(true);
            sec.append(&row("Transition", &t));
        }
        if n.kind == NodeKind::Frame && n.parent.as_deref() == Some(page.as_str()) {
            let start = gtk::CheckButton::with_label("Presenting starts here");
            start.set_active(n.start == Some(true));
            let weak = Rc::downgrade(self);
            let id = n.id.clone();
            start.connect_toggled(move |c| {
                let Some(b) = weak.upgrade() else { return };
                // One start per page.
                let mut ops = Vec::new();
                if c.is_active() {
                    let scene = b.doc.scene.borrow();
                    let page = b.page.borrow().clone();
                    for other in scene.children(&page) {
                        if *other != id && scene.node(other).is_some_and(|o| o.start == Some(true)) {
                            ops.push(relay_bus::design::NodeOp::Update { id: other.clone(), set: set(&[("start", Value::Null)]) });
                        }
                    }
                }
                ops.push(relay_bus::design::NodeOp::Update { id: id.clone(), set: set(&[("start", if c.is_active() { json!(true) } else { Value::Null })]) });
                b.commit(ops);
            });
            sec.append(&start);
        }
        let play = gtk::Button::with_label("Present");
        play.add_css_class("quiet");
        let weak = Rc::downgrade(self);
        play.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.present();
            }
        });
        sec.append(&play);
    }
}
