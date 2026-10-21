//! The inspector's panels for the richer properties: a polygon's corners and star, an icon's
//! choice and colour, a line's route and the shapes it is attached to, a path's closing and
//! smoothing, an ellipse's arc and its ends, solid or gradient fills and strokes, per-corner
//! radii, dashed strokes and a stroke's ends and joins. Each field writes what the node holds
//! when it commits, not what it held when the panel was built (TM-37).

use super::board::Board;
use super::inspector::current;
use super::widgets::{color_key, segmented, stored_number};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{takes, Gradient, GradientKind, Node, NodeKind, NodeOp, Route, StrokeCap, StrokeJoin};
use relay_client::design::names::display_name;
use relay_client::design::{color, shapes};
use serde_json::{json, Map, Value};
use std::rc::Rc;

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

fn pair(a: impl IsA<gtk::Widget>, b: impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_homogeneous(true);
    row.append(&a);
    row.append(&b);
    row
}

pub fn polygon(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let star = n.star.is_some();
    let sides = n.sides.unwrap_or(if star { 5 } else { 3 }) as f64;
    let weak = Rc::downgrade(b);
    let corners = stored_number("@triangle", Some(sides), if star { "Points" } else { "Corners" }, move |v| {
        let v = v.round().clamp(3.0, 24.0);
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("sides", json!(v as u8))]));
        }
        v
    });
    let toggle = gtk::CheckButton::with_label("Star");
    toggle.set_active(star);
    let weak = Rc::downgrade(b);
    toggle.connect_toggled(move |c| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("star", if c.is_active() { json!(0.4) } else { Value::Null })]));
        }
    });
    sec.append(&pair(corners, toggle));
    if let Some(inner) = n.star {
        let weak = Rc::downgrade(b);
        sec.append(&stored_number("@star", Some((inner * 100.0).round()), "Inner points, % of the outer", move |v| {
            let k = (v / 100.0).clamp(0.05, 0.95);
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("star", json!(k))]));
            }
            k * 100.0
        }));
    }
}

pub fn icon(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let name = n.icon.clone().unwrap_or_default();
    let change = gtk::MenuButton::new();
    change.add_css_class("design-dropdown-key");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&super::icons::image(&name, 16));
    let caption = label(&name, "");
    caption.set_hexpand(true);
    row.append(&caption);
    row.append(&crate::icons::image("chevron-down", 10));
    change.set_child(Some(&row));
    change.set_tooltip_text(Some("Another icon"));
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    change.set_popover(Some(&pop));
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    pop.connect_show(move |pop| {
        if let Some(b) = weak.upgrade() {
            pop.set_child(Some(&super::icons::picker(&b, pop, Some(id.clone()))));
        }
    });
    sec.append(&change);
    let weak = Rc::downgrade(b);
    // Automatic shows the colour it is drawn in, not an empty key.
    let shown = n.stroke.clone().unwrap_or_else(|| super::paint::text_color(&b.doc.scene.borrow(), n));
    let colour = color_key(b, Some(&shown), "Icon colour", Some("Automatic"), move |c| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("stroke", json!(c))]));
        }
    });
    let weak = Rc::downgrade(b);
    let weight = stored_number("@stroke-width", Some(n.stroke_width.unwrap_or(2.0)), "Line weight (on the icon's 24-unit grid)", move |v| {
        let v = v.clamp(0.5, 6.0);
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("stroke_width", json!(v))]));
        }
        v
    });
    // The key with its value beside it, as Fill has it, not a lone dot centred in its half.
    let colour_cell = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    colour_cell.append(&colour);
    let hex = label(if n.stroke.is_some() { &shown } else { "Automatic" }, "design-hex-caption");
    hex.set_ellipsize(gtk::pango::EllipsizeMode::End);
    colour_cell.append(&hex);
    sec.append(&pair(colour_cell, weight));
    // An icon draws its dashes on its 24-unit grid (B96).
    sec.append(&dash(b, std::slice::from_ref(n)));
}

pub fn connector(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let weak = Rc::downgrade(b);
    let current = match n.route.unwrap_or(Route::Straight) {
        Route::Straight => "straight",
        Route::Elbow => "elbow",
        Route::Curve => "curve",
    };
    let route = segmented(
        &[("straight", "Straight", "Straight"), ("elbow", "Elbow", "Elbow: right angles"), ("curve", "Curve", "Curve")],
        Some(current),
        move |v| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("route", if v == "straight" { Value::Null } else { json!(v) })]));
            }
        },
    );
    if n.from.is_none() && n.to.is_none() {
        let corners = n.points.as_ref().map_or(0, Vec::len);
        if corners > 2 {
            // A line of several points is a shape of its own: close it, smooth it.
            sec.append(&path_flags(b, n, true));
        } else {
            sec.append(&route);
            let hint = label("Draw a line from one shape to another and it stays attached as they move.", "design-note");
            hint.set_wrap(true);
            sec.append(&hint);
        }
        return;
    }
    sec.append(&route);
    let scene = b.doc.scene.borrow();
    let name = |id: &Option<String>| id.as_deref().and_then(|i| scene.node(i)).map(display_name).unwrap_or_else(|| "a free end".into());
    let caption = label(&format!("Attached: {} → {}", name(&n.from), name(&n.to)), "design-note");
    caption.set_wrap(true);
    drop(scene);
    sec.append(&caption);
    let detach = gtk::Button::with_label("Detach the ends");
    detach.add_css_class("quiet");
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    detach.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            detach_line(&b, &id);
        }
    });
    sec.append(&detach);
}

/// A pen path: closed or open.
pub fn path(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    sec.append(&path_flags(b, n, false));
}

/// Closed (and, for a line, smooth) as checks.
fn path_flags(b: &Rc<Board>, n: &Node, smooth: bool) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let flags: &[(&'static str, &str, bool)] = if smooth {
        &[("closed", "Closed", n.closed == Some(true)), ("smooth", "Smooth", n.smooth == Some(true))]
    } else {
        &[("closed", "Closed", n.closed == Some(true))]
    };
    for (field, caption, on) in flags {
        let c = gtk::CheckButton::with_label(caption);
        c.set_active(*on);
        c.set_tooltip_text(Some(if *field == "closed" { "Join the end back to the start; a fill paints inside" } else { "Curve through the points" }));
        let weak = Rc::downgrade(b);
        let field = *field;
        c.connect_toggled(move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[(field, if c.is_active() { json!(true) } else { Value::Null })]));
            }
        });
        row.append(&c);
    }
    row
}

/// An ellipse's arc as it is now, the whole ellipse when it has none.
fn arc_of(n: &Node) -> relay_bus::design::Arc {
    n.arc.clone().unwrap_or(relay_bus::design::Arc { start: 0.0, sweep: 360.0, inner: 0.0, cap: None })
}

/// The arc an ellipse is given: none for the whole ellipse, and its ends' `cap` only on a ring
/// segment (a hole and less than a whole turn), the one arc with ends to shape, so switching to
/// a pie or a whole ring is never refused for a cap left behind.
fn arc_value(a: &relay_bus::design::Arc) -> Value {
    if a.sweep.abs() >= 360.0 && a.inner <= 0.0 {
        return Value::Null;
    }
    let segment = a.inner > 0.0 && a.sweep.abs() < 360.0;
    let mut v = json!({"start": a.start, "sweep": a.sweep, "inner": a.inner});
    if let (true, Some(cap)) = (segment, a.cap.filter(|c| *c != StrokeCap::Butt)) {
        v["cap"] = json!(cap);
    }
    v
}

/// An ellipse's arc: where it starts, how far it goes, the hole in its middle and, on a ring
/// segment, how its ends are drawn, with the shapes people reach for.
pub fn arc(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let a = arc_of(n);
    // Each change is made to the arc as it is when it is made (TM-37).
    let write = {
        let weak = Rc::downgrade(b);
        let id = n.id.clone();
        Rc::new(move |change: &dyn Fn(&mut relay_bus::design::Arc)| {
            if let Some(b) = weak.upgrade() {
                let mut a = current(&b, &id, |n| Some(arc_of(n))).unwrap_or_else(|| arc_of(&Node::new(NodeKind::Ellipse)));
                change(&mut a);
                b.set_selected(set(&[("arc", arc_value(&a))]));
            }
        })
    };
    let presets = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    presets.add_css_class("design-segmented");
    let round = Some(StrokeCap::Round);
    for (caption, tip, start, sweep, inner, cap) in [
        ("Whole", "The whole ellipse", 0.0, 360.0, 0.0, None),
        ("Pie", "A three-quarter pie", 0.0, 270.0, 0.0, None),
        ("Donut", "A ring", 0.0, 360.0, 0.6, None),
        ("Gauge", "A half ring, open at the bottom", 270.0, 180.0, 0.7, None),
        ("Progress", "A progress ring with round ends", 0.0, 252.0, 0.82, round),
    ] {
        let key = gtk::Button::with_label(caption);
        key.add_css_class("quiet");
        key.set_tooltip_text(Some(tip));
        let lit = (a.start, a.sweep, a.inner) == (start, sweep, inner) && a.cap.filter(|c| *c != StrokeCap::Butt) == cap;
        if lit {
            key.add_css_class("selected");
        }
        key.update_state(&[gtk::accessible::State::Pressed(if lit { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        let w = write.clone();
        key.connect_clicked(move |_| {
            w(&|a| {
                (a.start, a.sweep, a.inner, a.cap) = (start, sweep, inner, cap);
            })
        });
        presets.append(&key);
    }
    sec.append(&presets);
    let w = write.clone();
    let start_field = stored_number("@rotate", Some(a.start), "Start, degrees clockwise from the top", move |v| {
        let v = v.rem_euclid(360.0);
        w(&|a| a.start = v);
        v
    });
    let w = write.clone();
    let sweep_field = stored_number("@arc", Some(a.sweep), "Sweep, degrees (negative runs anticlockwise)", move |v| {
        let v = v.clamp(-360.0, 360.0);
        w(&|a| a.sweep = v);
        v
    });
    sec.append(&pair(start_field, sweep_field));
    let w = write.clone();
    sec.append(&stored_number("@ellipse", Some((a.inner * 100.0).round()), "Hole, % of the radius", move |v| {
        let k = (v / 100.0).clamp(0.0, 0.99);
        w(&|a| a.inner = k);
        k * 100.0
    }));
    // A ring segment's ends: flat, round (a progress ring's) or square.
    if a.inner > 0.0 && a.sweep.abs() < 360.0 {
        let current = match a.cap.unwrap_or(StrokeCap::Butt) {
            StrokeCap::Butt => "butt",
            StrokeCap::Round => "round",
            StrokeCap::Square => "square",
        };
        let w = write.clone();
        sec.append(&labelled(
            "Ends",
            segmented(&[("butt", "Flat", "Flat ends"), ("round", "Round", "Round ends"), ("square", "Square", "Square ends")], Some(current), move |v| {
                let cap = match v {
                    "round" => Some(StrokeCap::Round),
                    "square" => Some(StrokeCap::Square),
                    _ => None,
                };
                w(&|a| a.cap = cap);
            }),
        ));
    }
}

fn labelled(caption: &str, widget: impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let l = label(caption, "design-row-caption");
    l.set_hexpand(true);
    row.append(&l);
    row.append(&widget);
    row
}

/// Free a connector's ends where they are drawn now: its points become its own again.
fn detach_line(b: &Board, id: &str) {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let Some(n) = scene.node(id) else {
        return;
    };
    let (pts, curved) = shapes::line_points(&scene, &placed, n);
    let pts = if curved { vec![pts[0], pts[3]] } else { pts };
    let origin = n.parent.as_deref().map(|p| b.origin_of(p)).unwrap_or_default();
    let x0 = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let y0 = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let x1 = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let y1 = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let points: Vec<[f64; 2]> = pts.iter().map(|p| [(p.0 - x0).round(), (p.1 - y0).round()]).collect();
    drop(scene);
    b.commit(vec![NodeOp::Update {
        id: id.to_string(),
        set: set(&[
            ("from", Value::Null),
            ("to", Value::Null),
            ("x", json!((x0 - origin.x).round())),
            ("y", json!((y0 - origin.y).round())),
            ("w", json!((x1 - x0).round())),
            ("h", json!((y1 - y0).round())),
            ("points", json!(points)),
        ]),
    }]);
}

/// Which of a node's paints a `paint` panel edits: its fill (`fill`, `gradient`) or its stroke
/// (`stroke`, `stroke_gradient`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Fill,
    Stroke,
}

impl Slot {
    /// Its solid colour's field and its gradient's.
    fn fields(self) -> (&'static str, &'static str) {
        match self {
            Slot::Fill => ("fill", "gradient"),
            Slot::Stroke => ("stroke", "stroke_gradient"),
        }
    }

    fn gradient(self, n: &Node) -> Option<Gradient> {
        match self {
            Slot::Fill => n.gradient.clone(),
            Slot::Stroke => n.stroke_gradient.clone(),
        }
    }

    /// The colour it paints when solid, its kind's default when it names none.
    fn colour(self, n: &Node) -> Option<String> {
        match self {
            Slot::Fill => color::fill_of(n),
            Slot::Stroke => n.stroke.clone(),
        }
    }
}

/// The angle a gradient of `kind` starts at when it names none: a linear one runs top to
/// bottom (180), a conic one starts at the top (0).
fn default_angle(kind: GradientKind) -> Option<f64> {
    match kind {
        GradientKind::Linear => Some(180.0),
        GradientKind::Conic => Some(0.0),
        GradientKind::Radial => None,
    }
}

/// A fill or a stroke: one colour, or a linear, radial or conic gradient with its angle and
/// stops (where the node's kind takes one).
pub fn paint(b: &Rc<Board>, sec: &gtk::Box, n: &Node, slot: Slot) {
    let (solid, field) = slot.fields();
    let gradient = slot.gradient(n);
    let id = n.id.clone();
    if takes(n.kind, field) {
        let mode = match gradient.as_ref().map(|g| g.kind) {
            None => "solid",
            Some(GradientKind::Linear) => "linear",
            Some(GradientKind::Radial) => "radial",
            Some(GradientKind::Conic) => "conic",
        };
        let base = slot.colour(n).unwrap_or_else(|| "#4C9DFF".into());
        let weak = Rc::downgrade(b);
        let id = id.clone();
        sec.append(&segmented(
            &[("solid", "Solid", "One colour"), ("linear", "Linear", "Linear gradient"), ("radial", "Radial", "Radial gradient"), ("conic", "Conic", "Conic gradient, round the centre")],
            Some(mode),
            move |v| {
                let Some(b) = weak.upgrade() else {
                    return;
                };
                let value = match v {
                    "solid" => Value::Null,
                    kind => {
                        let now = current(&b, &id, |n| slot.gradient(n));
                        let mut g = now.map(|g| serde_json::to_value(g).unwrap_or_default()).unwrap_or_else(|| {
                            json!({"stops": [{"at": 0.0, "color": base.clone()}, {"at": 1.0, "color": "#A178FF"}]})
                        });
                        // Each kind starts where it starts by default: a linear one's 180 would
                        // start a conic one at the bottom.
                        let kind_of = match kind {
                            "radial" => GradientKind::Radial,
                            "conic" => GradientKind::Conic,
                            _ => GradientKind::Linear,
                        };
                        g["kind"] = json!(kind_of);
                        match (default_angle(kind_of), g.as_object_mut()) {
                            (Some(a), Some(o)) => o.insert("angle".into(), json!(a)),
                            (None, Some(o)) => o.remove("angle"),
                            _ => None,
                        };
                        g
                    }
                };
                b.set_selected(set(&[(field, value)]));
            },
        ));
    }
    let Some(g) = gradient else {
        let shown = slot.colour(n);
        let lines = matches!(n.kind, NodeKind::Line | NodeKind::Path);
        // A line or path fills with nothing until given a colour, and a text; a shape with its
        // kind's. A line strokes in the default grey; a box has no border.
        let clear = match slot {
            Slot::Fill if lines || n.kind == NodeKind::Text => Some("None"),
            Slot::Fill => Some("Automatic"),
            Slot::Stroke if lines => None,
            Slot::Stroke => Some("None"),
        };
        let weak = Rc::downgrade(b);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let tooltip = if slot == Slot::Fill { "Fill" } else { "Colour" };
        // As stored where the layer holds one, so a named colour shows as its name.
        let stored = b.doc.scene.borrow().stored(&n.id).and_then(|s| slot.colour(s));
        let current = stored.or(shown.clone());
        let field = if slot == Slot::Fill { "fill" } else { "stroke" };
        row.append(&super::widgets::color_key_named(b, current.as_deref(), tooltip, clear, Some(field), move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[(solid, json!(c))]));
            }
        }));
        let caption = current.as_deref().unwrap_or(if clear.is_none() { "Automatic" } else { "None" });
        row.append(&label(caption, "design-hex-caption"));
        sec.append(&row);
        return;
    };
    // Each change is made to the gradient as it is when it is made (TM-37): a stop's colour,
    // then its place, keeps both.
    let write = {
        let weak = Rc::downgrade(b);
        Rc::new(move |change: &dyn Fn(&mut Value)| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            if let Some(g) = current(&b, &id, |n| slot.gradient(n)) {
                let mut v = serde_json::to_value(g).unwrap_or_default();
                change(&mut v);
                b.set_selected(set(&[(field, v)]));
            }
        })
    };
    if let Some(start) = default_angle(g.kind) {
        let tip = if g.kind == GradientKind::Conic { "Start, degrees clockwise from the top" } else { "Angle: 180 runs top to bottom, 90 left to right" };
        let w = write.clone();
        sec.append(&stored_number("@rotate", Some(g.angle.unwrap_or(start)), tip, move |a| {
            let a = a.rem_euclid(360.0);
            w(&|v| v["angle"] = json!(a));
            a
        }));
    }
    for (i, stop) in g.stops.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let w = write.clone();
        row.append(&color_key(b, Some(&stop.color), "Stop colour", None, move |c| {
            if let Some(c) = c {
                w(&|v| v["stops"][i]["color"] = json!(c));
            }
        }));
        let w = write.clone();
        // Captioned, so it scrubs as the other fields do (E-board-7).
        let at = stored_number("%", Some((stop.at * 100.0).round()), "Where it sits, %", move |p| {
            let at = (p / 100.0).clamp(0.0, 1.0);
            w(&|v| v["stops"][i]["at"] = json!(at));
            at * 100.0
        });
        at.set_hexpand(true);
        row.append(&at);
        if g.stops.len() > 2 {
            let remove = crate::app::icon_button("minus", "Remove this stop");
            let w = write.clone();
            remove.connect_clicked(move |_| {
                w(&|v| {
                    if let Some(stops) = v["stops"].as_array_mut().filter(|s| s.len() > 2 && i < s.len()) {
                        stops.remove(i);
                    }
                })
            });
            row.append(&remove);
        }
        sec.append(&row);
    }
    if g.stops.len() < 8 {
        let add = gtk::Button::with_label("Add a stop");
        add.add_css_class("quiet");
        let w = write.clone();
        add.connect_clicked(move |_| {
            w(&|v| {
                if let Some(stops) = v["stops"].as_array_mut().filter(|s| s.len() < 8) {
                    let color = stops.last().map(|s| s["color"].clone()).unwrap_or(json!("#FFFFFF"));
                    stops.push(json!({"at": 1.0, "color": color}));
                }
            })
        });
        sec.append(&add);
    }
}

/// How a stroke's ends are drawn (butt, round or square), for lines and paths and dashed
/// strokes, and its corners (mitred, round or bevelled) where every node takes them.
pub fn caps(b: &Rc<Board>, sec: &gtk::Box, nodes: &[Node]) {
    let all = |field: &str| nodes.iter().all(|n| takes(n.kind, field));
    let lines = nodes.iter().all(|n| matches!(n.kind, NodeKind::Line | NodeKind::Path));
    let dotted = |n: &Node| n.dash.as_ref().is_some_and(|d| d.first().is_some_and(|v| *v <= 0.0));
    if all("stroke_cap") && (lines || nodes.iter().all(|n| n.dash.is_some())) {
        // Today's ends where it names none: round dots, butt dashes, round solid strokes.
        let cap = |n: &Node| match n.stroke_cap {
            Some(c) => c,
            None if dotted(n) => StrokeCap::Round,
            None if n.dash.is_some() => StrokeCap::Butt,
            None => StrokeCap::Round,
        };
        let current = same_str(nodes.iter().map(|n| match cap(n) {
            StrokeCap::Butt => "butt",
            StrokeCap::Round => "round",
            StrokeCap::Square => "square",
        }));
        // Butt ends draw nothing of a dot's 0-length dashes.
        let mut choices = Vec::new();
        if !nodes.iter().any(dotted) {
            choices.push(("butt", "Flat", "Flat ends, cut at the end"));
        }
        choices.extend([("round", "Round", "Round ends"), ("square", "Square", "Square ends, past the end")]);
        let weak = Rc::downgrade(b);
        sec.append(&labelled(
            "Ends",
            segmented(&choices, current, move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_cap", json!(v))]));
                }
            }),
        ));
    }
    if all("stroke_join") {
        let current = same_str(nodes.iter().map(|n| match n.stroke_join.unwrap_or(StrokeJoin::Round) {
            StrokeJoin::Miter => "miter",
            StrokeJoin::Round => "round",
            StrokeJoin::Bevel => "bevel",
        }));
        let weak = Rc::downgrade(b);
        sec.append(&labelled(
            "Joins",
            segmented(&[("miter", "Sharp", "Sharp corners"), ("round", "Round", "Round corners"), ("bevel", "Bevel", "Cut corners")], current, move |v| {
                if let Some(b) = weak.upgrade() {
                    b.set_selected(set(&[("stroke_join", if v == "round" { Value::Null } else { json!(v) })]));
                }
            }),
        ));
    }
}

/// The one value every item has, if they agree.
fn same_str<'a>(mut values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

/// Solid, dashed or dotted.
pub fn dash(b: &Rc<Board>, nodes: &[Node]) -> gtk::Box {
    let style = |n: &Node| match n.dash.as_deref() {
        None => "solid",
        Some([d, ..]) if *d <= 0.5 => "dotted",
        Some(_) => "dashed",
    };
    let current = nodes.first().map(style).filter(|s| nodes.iter().all(|n| style(n) == *s));
    let weak = Rc::downgrade(b);
    // Gaps grow with the weight, so dots stay dots on a thick stroke.
    let lines = nodes.iter().all(|n| matches!(n.kind, NodeKind::Line | NodeKind::Path));
    let width = nodes.first().and_then(|n| n.stroke_width).unwrap_or(if lines { 2.0 } else { 1.0 });
    // Butt ends draw nothing of a dot: dots take today's round ends back.
    let butt = nodes.iter().any(|n| n.stroke_cap == Some(StrokeCap::Butt));
    segmented(
        &[("solid", "dash-solid", "Solid"), ("dashed", "dash", "Dashed"), ("dotted", "dots", "Dotted")],
        current,
        move |v| {
            if let Some(b) = weak.upgrade() {
                let value = match v {
                    "dashed" => json!([(width * 3.0).max(6.0).round(), (width * 2.0).max(4.0).round()]),
                    "dotted" => json!([0, (width * 2.0).max(4.0).round()]),
                    _ => Value::Null,
                };
                let mut s = set(&[("dash", value)]);
                if v == "dotted" && butt {
                    s.insert("stroke_cap".into(), Value::Null);
                }
                b.set_selected(s);
            }
        },
    )
}

/// Each corner its own radius.
pub fn corners(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let toggle = gtk::CheckButton::with_label("Each corner on its own");
    toggle.set_active(n.radii.is_some());
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    toggle.connect_toggled(move |c| {
        if let Some(b) = weak.upgrade() {
            // Every corner at the radius it has now (a sticky's own 6, B35).
            let radius = current(&b, &id, |n| Some(n.radius.unwrap_or(shapes::default_radius(n.kind)))).unwrap_or(0.0);
            let v = if c.is_active() { json!([radius, radius, radius, radius]) } else { Value::Null };
            b.set_selected(set(&[("radii", v)]));
        }
    });
    sec.append(&toggle);
    let Some(radii) = n.radii else {
        return;
    };
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_homogeneous(true);
    for (i, (cap, tip)) in [("TL", "Top left"), ("TR", "Top right"), ("BR", "Bottom right"), ("BL", "Bottom left")].into_iter().enumerate() {
        let weak = Rc::downgrade(b);
        let id = n.id.clone();
        row.append(&stored_number(cap, Some(radii[i]), tip, move |v| {
            let v = v.max(0.0);
            if let Some(b) = weak.upgrade() {
                // The other corners as they are now (TM-37).
                let mut r = current(&b, &id, |n| n.radii).unwrap_or(radii);
                r[i] = v;
                b.set_selected(set(&[("radii", json!(r))]));
            }
            v
        }));
    }
    sec.append(&row);
}

/// A box's border on each side (`stroke_widths`): a switch between one weight all round and a
/// weight per side, then the sides, top, right, bottom, left.
pub fn sides(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let toggle = gtk::CheckButton::with_label("Each side on its own");
    toggle.set_active(n.stroke_widths.is_some());
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    toggle.connect_toggled(move |c| {
        if let Some(b) = weak.upgrade() {
            // Every side at the weight it shows now; back to one weight, the widest side's.
            let w = current(&b, &id, |n| Some(shapes::stroke_weight(n, 1.0))).unwrap_or(1.0);
            let set = if c.is_active() { set(&[("stroke_widths", json!([w, w, w, w]))]) } else { set(&[("stroke_widths", Value::Null), ("stroke_width", json!(w))]) };
            b.set_selected(set);
        }
    });
    sec.append(&toggle);
    let Some(sides) = n.stroke_widths else {
        return;
    };
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_homogeneous(true);
    for (i, (cap, tip)) in [("T", "Top"), ("R", "Right"), ("B", "Bottom"), ("L", "Left")].into_iter().enumerate() {
        let weak = Rc::downgrade(b);
        let id = n.id.clone();
        row.append(&stored_number(cap, Some(sides[i]), &format!("{tip} border, px"), move |v| {
            let v = v.clamp(0.0, 1000.0);
            if let Some(b) = weak.upgrade() {
                // The other sides as they are now (TM-37).
                let mut w = current(&b, &id, |n| n.stroke_widths).unwrap_or(sides);
                w[i] = v;
                b.set_selected(set(&[("stroke_widths", json!(w))]));
            }
            v
        }));
    }
    sec.append(&row);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_segment_alone_keeps_its_ends() {
        let arc = |start: f64, sweep: f64, inner: f64, cap: Option<StrokeCap>| relay_bus::design::Arc { start, sweep, inner, cap };
        // A progress ring keeps its round ends.
        assert_eq!(arc_value(&arc(0.0, 252.0, 0.82, Some(StrokeCap::Round))), json!({"start": 0.0, "sweep": 252.0, "inner": 0.82, "cap": "round"}));
        // A pie or a whole ring has no ends: a cap left behind is not sent, so it is not refused.
        assert_eq!(arc_value(&arc(0.0, 270.0, 0.0, Some(StrokeCap::Round))), json!({"start": 0.0, "sweep": 270.0, "inner": 0.0}));
        assert_eq!(arc_value(&arc(0.0, 360.0, 0.6, Some(StrokeCap::Square))), json!({"start": 0.0, "sweep": 360.0, "inner": 0.6}));
        // Flat ends are the default, and the whole ellipse is no arc at all.
        assert_eq!(arc_value(&arc(90.0, 180.0, 0.5, Some(StrokeCap::Butt))), json!({"start": 90.0, "sweep": 180.0, "inner": 0.5}));
        assert_eq!(arc_value(&arc(0.0, 360.0, 0.0, None)), Value::Null);
    }

    #[test]
    fn each_gradient_starts_where_its_kind_does() {
        assert_eq!(default_angle(GradientKind::Linear), Some(180.0));
        assert_eq!(default_angle(GradientKind::Conic), Some(0.0));
        assert_eq!(default_angle(GradientKind::Radial), None);
    }
}
