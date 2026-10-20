//! The inspector's Motion and Timeline sections (native-motion-panel; feature-motion §3): a
//! layer's keys and presets, a frame's clock. Every change reads the layer's motion as it is when
//! it is made, edits it and writes it whole: one `design.apply`, one undo step.

use super::board::Board;
use super::inspector::current;
use super::widgets::{color_key, optional_number, stored_number, tip};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::motion::{MAX_KEYS, MAX_MS};
use relay_bus::design::{Ease, Key, Motion, Node, NodeKind};
use relay_client::design::motion::{preset, presets_for};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The eases a key or a motion takes, by their stored names, with what the inspector calls them.
const EASES: [(&str, &str); 7] = [
    ("ease", "Ease"),
    ("linear", "Linear"),
    ("ease_in", "Ease in"),
    ("ease_out", "Ease out"),
    ("ease_in_out", "Ease in and out"),
    ("overshoot", "Overshoot"),
    ("hold", "Hold"),
];

/// The points a motion turns and scales about, as fractions of the box.
const ORIGINS: [([f64; 2], &str); 9] = [
    ([0.5, 0.5], "Centre"),
    ([0.5, 0.0], "Top"),
    ([0.5, 1.0], "Bottom"),
    ([0.0, 0.5], "Left"),
    ([1.0, 0.5], "Right"),
    ([0.0, 0.0], "Top left"),
    ([1.0, 0.0], "Top right"),
    ([0.0, 1.0], "Bottom left"),
    ([1.0, 1.0], "Bottom right"),
];

/// A key's numbers: its field, what the inspector calls it, its value at rest.
const NUMBERS: [(&str, &str, f64); 11] = [
    ("dx", "Move across", 0.0),
    ("dy", "Move down", 0.0),
    ("rotate", "Turn", 0.0),
    ("scale", "Scale", 1.0),
    ("scale_x", "Scale across", 1.0),
    ("scale_y", "Scale down", 1.0),
    ("opacity", "Opacity", 1.0),
    ("blur", "Blur", 0.0),
    ("sweep", "Sweep", 360.0),
    ("angle", "Gradient angle", 180.0),
    ("trim", "Drawn", 1.0),
];

/// A key's colours: its field and what the inspector calls it.
const COLOURS: [(&str, &str); 3] = [("fill", "Fill"), ("stroke", "Stroke"), ("color", "Text colour")];

fn ease_name(e: Ease) -> String {
    serde_json::to_value(e).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Whether a key of node `n` can set `field`: a sweep only an ellipse's, the drawn part only a
/// line's or path's, a gradient's angle only where it has one, a text colour only where it has
/// words, and a fill or stroke only where it draws one.
fn takes(n: &Node, field: &str) -> bool {
    match field {
        "sweep" => n.kind == NodeKind::Ellipse,
        "trim" => matches!(n.kind, NodeKind::Line | NodeKind::Path),
        "angle" => n.gradient.is_some(),
        "color" => matches!(n.kind, NodeKind::Text | NodeKind::Sticky) || n.text.as_deref().is_some_and(|t| !t.is_empty()),
        "fill" | "stroke" => relay_bus::design::takes(n.kind, field),
        _ => true,
    }
}

/// What key `k` sets, in a few words: "dy 24 · opacity 0".
fn summary(k: &Key) -> String {
    let fields = serde_json::to_value(k).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let words: Vec<String> = fields
        .iter()
        .filter(|(f, _)| !matches!(f.as_str(), "at_ms" | "ease"))
        .map(|(f, v)| match v.as_f64() {
            Some(n) => format!("{f} {}", relay_client::design::number::fmt(n)),
            None => format!("{f} {}", v.as_str().unwrap_or_default()),
        })
        .collect();
    if words.is_empty() { "nothing set".to_string() } else { words.join(" · ") }
}

/// Write `m` (or none) as node `id`'s motion, its keys in time order.
fn send(b: &Board, m: Option<Motion>) {
    let value = match m {
        Some(mut m) => {
            m.keys.sort_by(|a, c| a.at_ms.total_cmp(&c.at_ms));
            serde_json::to_value(&m).unwrap_or(Value::Null)
        }
        None => Value::Null,
    };
    b.set_selected(Map::from_iter([("motion".to_string(), value)]));
}

/// Change node `id`'s motion as it is now with `f`, and write it.
fn change(weak: &Weak<Board>, id: &str, f: impl Fn(&mut Motion)) {
    let Some(b) = weak.upgrade() else { return };
    let Some(mut m) = current(&b, id, |n| n.motion.clone()) else { return };
    f(&mut m);
    send(&b, Some(m));
}

/// The key a row of the Motion section edits: where it sat and what it held when the row last
/// wrote it. `send` puts the keys in time order, and the inspector is not rebuilt while one of
/// its fields has the keyboard, so a row finds its key by what it holds, not by where it was
/// when the row was built: 600 typed into the first of keys at 0 and 300 ms makes it the second.
type Slot = Rc<RefCell<(usize, Key)>>;

/// Where key `was`, last seen at `at`, sits in `keys` now: there still, else the first that
/// holds the same. `None` when no key holds it any more (an agent changed it meanwhile).
fn find_key(keys: &[Key], at: usize, was: &Key) -> Option<usize> {
    if keys.get(at) == Some(was) {
        return Some(at);
    }
    keys.iter().position(|k| k == was)
}

/// Change key `i` of `keys` with `f` and put them in time order, as `send` writes them (a stable
/// sort: keys at one time keep their order): where it sits after.
fn edit_key(keys: &mut Vec<Key>, i: usize, f: impl FnOnce(&mut Key)) -> usize {
    if let Some(k) = keys.get_mut(i) {
        f(k);
    }
    let mut order: Vec<(usize, Key)> = std::mem::take(keys).into_iter().enumerate().collect();
    order.sort_by(|a, c| a.1.at_ms.total_cmp(&c.1.at_ms));
    let at = order.iter().position(|(j, _)| *j == i).unwrap_or(i);
    *keys = order.into_iter().map(|(_, k)| k).collect();
    at
}

/// Change the key `slot` holds, in node `id`'s motion as it is now, with `f`, and write it; the
/// slot then follows the key to where it sorts. Nothing when the key is no longer there.
fn change_key(weak: &Weak<Board>, id: &str, slot: &Slot, f: impl FnOnce(&mut Key)) {
    let Some(b) = weak.upgrade() else { return };
    let Some(mut m) = current(&b, id, |n| n.motion.clone()) else { return };
    let (at, was) = slot.borrow().clone();
    let Some(i) = find_key(&m.keys, at, &was) else { return };
    let to = edit_key(&mut m.keys, i, f);
    let now = m.keys[to].clone();
    send(&b, Some(m));
    // Only what the scene took: a refused edit leaves the key as it was.
    if current(&b, id, |n| n.motion.clone()).is_some_and(|m| m.keys.get(to) == Some(&now)) {
        *slot.borrow_mut() = (to, now);
    }
}

/// A key `i`'s field through JSON, as the stored names have it.
fn set_field(k: &mut Key, field: &str, value: Value) {
    let mut fields = serde_json::to_value(&*k).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
    if value.is_null() {
        fields.remove(field);
    } else {
        fields.insert(field.to_string(), value);
    }
    if let Ok(next) = serde_json::from_value(Value::Object(fields)) {
        *k = next;
    }
}

fn get_field(k: &Key, field: &str) -> Option<Value> {
    serde_json::to_value(k).ok()?.get(field).cloned()
}

/// A menu key of `choices`, each calling `pick` with its index.
fn menu(caption: &str, tooltip: &str, choices: &[String], pick: impl Fn(usize) + 'static) -> gtk::MenuButton {
    let key = gtk::MenuButton::new();
    key.set_label(caption);
    key.add_css_class("design-dropdown");
    tip(&key, tooltip);
    let pop = gtk::Popover::new();
    pop.add_css_class("design-menu");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let pick = Rc::new(pick);
    for (i, choice) in choices.iter().enumerate() {
        let item = gtk::Button::with_label(choice);
        item.add_css_class("design-menu-item");
        let (pick, p) = (pick.clone(), pop.downgrade());
        item.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            pick(i);
        });
        body.append(&item);
    }
    pop.set_child(Some(&body));
    key.set_popover(Some(&pop));
    key
}

/// A dropdown of `names` showing `at`, calling `pick` with the index chosen.
fn dropdown(names: &[&str], at: Option<usize>, tooltip: &str, pick: impl Fn(usize) + 'static) -> gtk::DropDown {
    let dd = gtk::DropDown::from_strings(names);
    dd.add_css_class("design-dropdown");
    dd.set_tooltip_text(Some(tooltip));
    dd.set_selected(at.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
    dd.connect_selected_notify(move |dd| {
        if dd.selected() != gtk::INVALID_LIST_POSITION {
            pick(dd.selected() as usize);
        }
    });
    dd
}

/// The Motion section for node `n`: its presets, its keys, how it turns, eases and loops.
pub(super) fn motion(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let presets = presets_for(n.kind);
    let labels: Vec<String> = presets.iter().map(|p| p.label().to_string()).collect();
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    let choose = {
        let (weak, id) = (weak.clone(), id.clone());
        move |i: usize| {
            let (Some(b), Some(p)) = (weak.upgrade(), presets.get(i)) else { return };
            let Some(node) = current(&b, &id, |n| Some(n.clone())) else { return };
            send(&b, Some(preset(*p, &node)));
        }
    };
    let Some(m) = n.motion.clone() else {
        sec.append(&menu("Add motion", "How it moves when the design plays", &labels, choose));
        return;
    };
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let replace = menu("Replace with", "Start again from a preset", &labels, choose);
    replace.set_hexpand(true);
    head.append(&replace);
    let remove = gtk::Button::with_label("Remove");
    remove.add_css_class("quiet");
    remove.set_tooltip_text(Some("It stays still"));
    let w = weak.clone();
    remove.connect_clicked(move |_| {
        if let Some(b) = w.upgrade() {
            send(&b, None);
        }
    });
    head.append(&remove);
    sec.append(&head);
    for (i, k) in m.keys.iter().enumerate() {
        sec.append(&key_row(b, n, Rc::new(RefCell::new((i, k.clone()))), m.keys.len()));
    }
    if m.keys.len() < MAX_KEYS {
        let add = gtk::Button::with_label("Add key");
        add.add_css_class("quiet");
        add.set_tooltip_text(Some("A copy of the last key, 300 ms later"));
        let (w, id2) = (weak.clone(), id.clone());
        add.connect_clicked(move |_| {
            change(&w, &id2, |m| {
                if let Some(mut last) = m.keys.last().cloned() {
                    last.at_ms = (last.at_ms + 300.0).min(MAX_MS);
                    m.keys.push(last);
                }
            });
        });
        sec.append(&add);
    }
    let origin = m.origin.unwrap_or([0.5, 0.5]);
    let names: Vec<&str> = ORIGINS.iter().map(|o| o.1).collect();
    let (w, id2) = (weak.clone(), id.clone());
    let at = ORIGINS.iter().position(|o| o.0 == origin);
    sec.append(&labelled("Turns about", dropdown(&names, at, "The point it turns and scales about", move |i| {
        let point = ORIGINS[i].0;
        change(&w, &id2, |m| m.origin = (point != [0.5, 0.5]).then_some(point));
    })));
    let names: Vec<&str> = EASES.iter().map(|e| e.1).collect();
    let now = ease_name(m.ease.unwrap_or(Ease::Standard));
    let (w, id2) = (weak.clone(), id.clone());
    sec.append(&labelled("Ease", dropdown(&names, EASES.iter().position(|e| e.0 == now), "How each move speeds up and slows down", move |i| {
        let ease: Option<Ease> = serde_json::from_value(json!(EASES[i].0)).ok();
        change(&w, &id2, |m| m.ease = ease.filter(|e| *e != Ease::Standard));
    })));
    let looped = gtk::CheckButton::with_label("Loop its keys");
    looped.set_active(m.looped == Some(true));
    looped.set_tooltip_text(Some("Plays its keys again from the start, whatever its frame's timeline does"));
    let (w, id2) = (weak, id);
    looped.connect_toggled(move |c| {
        let on = c.is_active();
        change(&w, &id2, |m| m.looped = on.then_some(true));
    });
    sec.append(&looped);
}

/// The key `slot` holds, of `count`: its time, what it sets, its ease and a delete key; it opens
/// to a field per property it sets and a menu adding another.
fn key_row(b: &Rc<Board>, n: &Node, slot: Slot, count: usize) -> gtk::Box {
    let key = slot.borrow().1.clone();
    let k = &key;
    let weak = Rc::downgrade(b);
    let id = n.id.clone();
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("design-motion-key");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let (w, id2, s) = (weak.clone(), id.clone(), slot.clone());
    let at = stored_number("ms", Some(k.at_ms), "When, in ms from the start of its timeline", move |v| {
        let v = v.clamp(0.0, MAX_MS).round();
        change_key(&w, &id2, &s, |k| k.at_ms = v);
        v
    });
    at.set_width_request(84);
    top.append(&at);
    let names: Vec<&str> = std::iter::once("Default").chain(EASES.iter().map(|e| e.1)).collect();
    let now = k.ease.map(ease_name);
    let pos = now.as_deref().and_then(|now| EASES.iter().position(|e| e.0 == now)).map_or(0, |p| p + 1);
    let (w, id2, s) = (weak.clone(), id.clone(), slot.clone());
    let ease = dropdown(&names, Some(pos), "How the move from this key to the next eases", move |p| {
        let ease: Option<Ease> = p.checked_sub(1).and_then(|p| serde_json::from_value(json!(EASES[p].0)).ok());
        change_key(&w, &id2, &s, |k| k.ease = ease);
    });
    ease.set_hexpand(true);
    top.append(&ease);
    if count > 2 {
        let delete = gtk::Button::new();
        delete.add_css_class("design-context-key");
        delete.set_child(Some(&super::glyphs::image("trash", 14)));
        tip(&delete, "Delete this key");
        let (w, id2, s) = (weak.clone(), id.clone(), slot.clone());
        delete.connect_clicked(move |_| change(&w, &id2, |m| {
            let (at, was) = s.borrow().clone();
            if let Some(i) = find_key(&m.keys, at, &was).filter(|_| m.keys.len() > 2) {
                m.keys.remove(i);
            }
        }));
        top.append(&delete);
    }
    row.append(&top);
    let fields = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for (field, caption, _) in NUMBERS.iter().filter(|(f, _, _)| get_field(k, f).is_some()) {
        let (w, id2, s, field) = (weak.clone(), id.clone(), slot.clone(), *field);
        let value = get_field(k, field).and_then(|v| v.as_f64());
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let number = stored_number(caption, value, caption, move |v| {
            change_key(&w, &id2, &s, |k| set_field(k, field, json!(v)));
            v
        });
        number.set_hexpand(true);
        line.append(&number);
        line.append(&unset(&weak, &id, &slot, field));
        fields.append(&line);
    }
    for (field, caption) in COLOURS.iter().filter(|(f, _)| get_field(k, f).is_some()) {
        let (w, id2, s, field) = (weak.clone(), id.clone(), slot.clone(), *field);
        let value = get_field(k, field).and_then(|v| v.as_str().map(str::to_string));
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        line.append(&label(caption, "design-number-caption"));
        // Keys take literal colours only (MODEL-SPEC §1 #22).
        line.append(&color_key(b, value.as_deref(), caption, None, move |c| {
            if let Some(c) = c.filter(|c| !c.starts_with('$')) {
                change_key(&w, &id2, &s, |k| set_field(k, field, json!(c)));
            }
        }));
        line.append(&unset(&weak, &id, &slot, field));
        fields.append(&line);
    }
    let missing: Vec<(&str, String)> = NUMBERS
        .iter()
        .map(|(f, c, _)| (*f, c.to_string()))
        .chain(COLOURS.iter().map(|(f, c)| (*f, c.to_string())))
        .filter(|(f, _)| get_field(k, f).is_none() && takes(n, f))
        .collect();
    if !missing.is_empty() {
        let captions: Vec<String> = missing.iter().map(|m| m.1.clone()).collect();
        let (w, id2, s, rest) = (weak.clone(), id.clone(), slot.clone(), rest_values(n));
        fields.append(&menu("+ property", "Set another property at this key, from its look at rest", &captions, move |p| {
            let field = missing[p].0;
            let value = rest.get(field).cloned().unwrap_or(Value::Null);
            change_key(&w, &id2, &s, |k| set_field(k, field, value));
        }));
    }
    let more = gtk::Expander::new(Some(&summary(k)));
    more.set_child(Some(&fields));
    row.append(&more);
    row
}

/// A key that takes `field` off the key `slot` holds.
fn unset(weak: &Weak<Board>, id: &str, slot: &Slot, field: &'static str) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("design-context-key");
    key.set_child(Some(&super::glyphs::image("x", 12)));
    tip(&key, "Take it off this key");
    let (w, id, s) = (weak.clone(), id.to_string(), slot.clone());
    key.connect_clicked(move |_| change_key(&w, &id, &s, |k| set_field(k, field, Value::Null)));
    key
}

/// Each property's value at rest for node `n`: what a key added from "+ property" starts at.
fn rest_values(n: &Node) -> Map<String, Value> {
    let mut rest: Map<String, Value> = NUMBERS.iter().map(|(f, _, v)| (f.to_string(), json!(v))).collect();
    if let Some(o) = n.opacity {
        rest.insert("opacity".into(), json!(o));
    }
    if let Some(a) = n.gradient.as_ref().and_then(|g| g.angle) {
        rest.insert("angle".into(), json!(a));
    }
    if let Some(arc) = &n.arc {
        rest.insert("sweep".into(), json!(arc.sweep));
    }
    let colour = |c: &Option<String>| c.clone().filter(|c| !c.starts_with('$')).unwrap_or_else(|| "#000000".into());
    rest.insert("fill".into(), json!(colour(&n.fill)));
    rest.insert("stroke".into(), json!(colour(&n.stroke)));
    rest.insert("color".into(), json!(colour(&n.color)));
    rest
}

/// The Timeline section for frame `n`: how long its clock runs (blank: as long as its latest key)
/// and whether it starts again.
pub(super) fn timeline(b: &Rc<Board>, sec: &gtk::Box, n: &Node) {
    let weak = Rc::downgrade(b);
    let t = n.timeline.clone();
    let write = |b: &Board, duration: Option<f64>, looped: bool| {
        let value = if duration.is_none() && looped { Value::Null } else {
            let mut o = Map::new();
            if let Some(d) = duration {
                o.insert("duration_ms".into(), json!(d));
            }
            if !looped {
                o.insert("loop".into(), json!(false));
            }
            Value::Object(o)
        };
        b.set_selected(Map::from_iter([("timeline".to_string(), value)]));
    };
    let (w, id) = (weak.clone(), n.id.clone());
    sec.append(&optional_number("Length", t.as_ref().and_then(|t| t.duration_ms), "Its length in ms; empty for as long as its latest key", move |v| {
        let v = v.map(|v| v.clamp(50.0, MAX_MS).round());
        if let Some(b) = w.upgrade() {
            let looped = current(&b, &id, |n| n.timeline.as_ref().map(|t| t.looped != Some(false))).unwrap_or(true);
            write(&b, v, looped);
        }
        v
    }));
    let looped = gtk::CheckButton::with_label("Loop");
    looped.set_active(t.as_ref().is_none_or(|t| t.looped != Some(false)));
    looped.set_tooltip_text(Some("Starts again when it ends; off, it holds its last frame"));
    let id = n.id.clone();
    looped.connect_toggled(move |c| {
        if let Some(b) = weak.upgrade() {
            let duration = current(&b, &id, |n| n.timeline.as_ref().and_then(|t| t.duration_ms));
            write(&b, duration, c.is_active());
        }
    });
    sec.append(&looped);
}

fn labelled(caption: &str, widget: impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&label(caption, "design-number-caption"));
    widget.set_hexpand(true);
    row.append(&widget);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_says_what_it_sets_and_takes_fields_by_name() {
        let mut k = Key::at(300.0);
        assert_eq!(summary(&k), "nothing set");
        set_field(&mut k, "dy", json!(24.0));
        set_field(&mut k, "opacity", json!(0.0));
        assert_eq!(summary(&k), "dy 24 · opacity 0");
        set_field(&mut k, "dy", Value::Null);
        assert_eq!((k.dy, k.opacity), (None, Some(0.0)));
    }

    #[test]
    fn a_key_offers_only_what_its_layer_can_move() {
        let line = Node::new(NodeKind::Line);
        let ring = Node::new(NodeKind::Ellipse);
        assert!(takes(&line, "trim") && !takes(&line, "sweep"));
        assert!(takes(&ring, "sweep") && !takes(&ring, "trim") && !takes(&ring, "angle"));
        assert!(!takes(&Node::new(NodeKind::Rect), "color"), "a box without words has no text colour");
    }

    /// A row follows its key through the re-sort its own edit makes: 600 typed into the first of
    /// keys at 0 and 300 ms makes it the second, and the row's next edit is of it still.
    #[test]
    fn a_row_follows_its_key_when_the_keys_sort_again() {
        let (mut k0, k1) = (Key::at(0.0), Key::at(300.0));
        set_field(&mut k0, "dy", json!(24.0));
        let mut keys = vec![k0.clone(), k1.clone()];
        let to = edit_key(&mut keys, 0, |k| k.at_ms = 600.0);
        assert_eq!((to, keys[0].at_ms, keys[1].at_ms), (1, 300.0, 600.0));
        let moved = keys[to].clone();
        assert_eq!(find_key(&keys, to, &moved), Some(1));
        // A row that kept its old place finds its key by what it holds.
        assert_eq!(find_key(&keys, 0, &moved), Some(1));
        assert_eq!(find_key(&keys, 1, &k1), Some(0));
        // Keys at one time keep their order; a key no one holds any more is not edited.
        let mut same = vec![Key::at(100.0), Key::at(100.0), Key::at(200.0)];
        assert_eq!(edit_key(&mut same, 1, |k| k.opacity = Some(0.5)), 1);
        assert_eq!(find_key(&keys, 0, &k0), None);
    }

    /// Every ease the panel offers reads as one the model has.
    #[test]
    fn every_ease_is_a_stored_name() {
        for (name, _) in EASES {
            assert!(serde_json::from_value::<Ease>(json!(name)).is_ok(), "{name}");
        }
        assert_eq!(ease_name(Ease::Standard), "ease");
    }
}
