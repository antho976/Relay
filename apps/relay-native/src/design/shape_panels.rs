//! The inspector's panels for the richer properties: a polygon's corners and star, an icon's
//! choice and colour, a line's route and the shapes it is attached to, solid or gradient fills,
//! per-corner radii and dashed strokes.

use super::board::{display_name, Board};
use super::widgets::{color_key, number, segmented};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{GradientKind, Node, NodeOp, Route};
use relay_client::design::shapes;
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
    let corners = number("@triangle", Some(sides), if star { "Points" } else { "Corners" }, move |v| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("sides", json!(v.round().clamp(3.0, 24.0) as u8))]));
        }
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
        sec.append(&number("@star", Some((inner * 100.0).round()), "Inner points, % of the outer", move |v| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("star", json!((v / 100.0).clamp(0.05, 0.95)))]));
            }
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
    let colour = color_key(b, n.stroke.as_deref(), "Icon colour", true, move |c| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("stroke", json!(c))]));
        }
    });
    let weak = Rc::downgrade(b);
    let weight = number("@stroke-width", Some(n.stroke_width.unwrap_or(2.0)), "Line weight (on the icon's 24-unit grid)", move |v| {
        if let Some(b) = weak.upgrade() {
            b.set_selected(set(&[("stroke_width", json!(v.clamp(0.5, 6.0)))]));
        }
    });
    sec.append(&pair(colour, weight));
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
    sec.append(&route);
    if n.from.is_none() && n.to.is_none() {
        let hint = label("Draw a line from one shape to another and it stays attached as they move.", "design-note");
        hint.set_wrap(true);
        sec.append(&hint);
        return;
    }
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

/// Free a connector's ends where they are drawn now: its points become its own again.
fn detach_line(b: &Board, id: &str) {
    let placed = b.placed();
    let scene = b.doc.scene.borrow();
    let Some(n) = scene.node(id) else {
        return;
    };
    let (pts, curved) = shapes::line_points(&scene, &placed, n);
    let pts = if curved { vec![pts[0], pts[3]] } else { pts };
    let origin = n.parent.as_deref().and_then(|p| placed.rect(p)).unwrap_or_default();
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

/// Solid colour, or a linear or radial gradient with its angle and stops.
pub fn fill(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let mode = match n.gradient.as_ref().map(|g| g.kind) {
        None => "solid",
        Some(GradientKind::Linear) => "linear",
        Some(GradientKind::Radial) => "radial",
    };
    let base = super::paint::fill_of(n).unwrap_or_else(|| "#4C9DFF".into());
    let weak = Rc::downgrade(b);
    let current = n.gradient.clone();
    sec.append(&segmented(
        &[("solid", "Solid", "One colour"), ("linear", "Linear", "Linear gradient"), ("radial", "Radial", "Radial gradient")],
        Some(mode),
        move |v| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            let value = match v {
                "solid" => Value::Null,
                kind => {
                    let mut g = current.clone().map(|g| serde_json::to_value(g).unwrap_or_default()).unwrap_or_else(|| {
                        json!({"stops": [{"at": 0.0, "color": base.clone()}, {"at": 1.0, "color": "#A178FF"}]})
                    });
                    g["kind"] = json!(kind);
                    if kind == "linear" && g.get("angle").is_none_or(Value::is_null) {
                        g["angle"] = json!(180);
                    }
                    g
                }
            };
            b.set_selected(set(&[("gradient", value)]));
        },
    ));
    let Some(g) = n.gradient.clone() else {
        let fill = super::paint::fill_of(n);
        let weak = Rc::downgrade(b);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&color_key(b, fill.as_deref(), "Fill", true, move |c| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("fill", json!(c))]));
            }
        }));
        row.append(&label(fill.as_deref().unwrap_or("None"), "design-hex-caption"));
        sec.append(&row);
        return;
    };
    let write = {
        let weak = Rc::downgrade(b);
        Rc::new(move |g: Value| {
            if let Some(b) = weak.upgrade() {
                b.set_selected(set(&[("gradient", g)]));
            }
        })
    };
    let value = serde_json::to_value(&g).unwrap_or_default();
    if g.kind == GradientKind::Linear {
        let w = write.clone();
        let v = value.clone();
        sec.append(&number("@rotate", Some(g.angle.unwrap_or(180.0)), "Angle: 180 runs top to bottom, 90 left to right", move |a| {
            let mut v = v.clone();
            v["angle"] = json!(a.rem_euclid(360.0));
            w(v);
        }));
    }
    for (i, stop) in g.stops.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let w = write.clone();
        let v = value.clone();
        row.append(&color_key(b, Some(&stop.color), "Stop colour", false, move |c| {
            if let Some(c) = c {
                let mut v = v.clone();
                v["stops"][i]["color"] = json!(c);
                w(v);
            }
        }));
        let w = write.clone();
        let v = value.clone();
        let at = number("", Some((stop.at * 100.0).round()), "Where it sits, %", move |p| {
            let mut v = v.clone();
            v["stops"][i]["at"] = json!((p / 100.0).clamp(0.0, 1.0));
            w(v);
        });
        at.set_hexpand(true);
        row.append(&at);
        if g.stops.len() > 2 {
            let remove = crate::app::icon_button("minus", "Remove this stop");
            let w = write.clone();
            let v = value.clone();
            remove.connect_clicked(move |_| {
                let mut v = v.clone();
                if let Some(stops) = v["stops"].as_array_mut() {
                    stops.remove(i);
                }
                w(v);
            });
            row.append(&remove);
        }
        sec.append(&row);
    }
    if g.stops.len() < 8 {
        let add = gtk::Button::with_label("Add a stop");
        add.add_css_class("quiet");
        let w = write.clone();
        let v = value.clone();
        add.connect_clicked(move |_| {
            let mut v = v.clone();
            if let Some(stops) = v["stops"].as_array_mut() {
                let color = stops.last().map(|s| s["color"].clone()).unwrap_or(json!("#FFFFFF"));
                stops.push(json!({"at": 1.0, "color": color}));
            }
            w(v);
        });
        sec.append(&add);
    }
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
    segmented(
        &[("solid", "dash-solid", "Solid"), ("dashed", "dash", "Dashed"), ("dotted", "dots", "Dotted")],
        current,
        move |v| {
            if let Some(b) = weak.upgrade() {
                let value = match v {
                    "dashed" => json!([6, 4]),
                    "dotted" => json!([0, 4]),
                    _ => Value::Null,
                };
                b.set_selected(set(&[("dash", value)]));
            }
        },
    )
}

/// Each corner its own radius.
pub fn corners(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let toggle = gtk::CheckButton::with_label("Each corner on its own");
    toggle.set_active(n.radii.is_some());
    let weak = Rc::downgrade(b);
    let radius = n.radius.unwrap_or(0.0);
    toggle.connect_toggled(move |c| {
        if let Some(b) = weak.upgrade() {
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
        row.append(&number(cap, Some(radii[i]), tip, move |v| {
            if let Some(b) = weak.upgrade() {
                let mut r = radii;
                r[i] = v.max(0.0);
                b.set_selected(set(&[("radii", json!(r))]));
            }
        }));
    }
    sec.append(&row);
}
