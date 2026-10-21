//! The icon picker: Relay's icon set in a searchable grid. Choosing one places an icon node in
//! the middle of the view (inside the selected frame, as paste does), or swaps the selected
//! icon's.

use super::board::Board;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp, ICONS};
use serde_json::{json, Map};
use std::cell::RefCell;
use std::rc::Rc;

/// An icon of the set as a symbolic picture that takes the widget's text colour.
pub fn image(name: &str, size: i32) -> gtk::Image {
    outline(relay_client::design::icons::path(name).unwrap_or(""), size)
}

/// An outline on the 24-unit grid as a symbolic picture that takes the widget's text colour.
fn outline(d: &str, size: i32) -> gtk::Image {
    let d = glib::markup_escape_text(d);
    let doc = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="url(#gpa:foreground)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="{d}"/></svg>"##
    );
    let image = gtk::Image::from_paintable(Some(&gtk::Svg::from_bytes(&glib::Bytes::from_owned(doc.into_bytes()))));
    image.set_pixel_size(size);
    image
}

/// The picker's keys, each with its icon's name and a saved icon's keywords.
type Keys = Rc<RefCell<Vec<(String, Vec<String>, gtk::Button)>>>;

pub fn picker(b: &Rc<Board>, pop: &gtk::Popover, target: Option<String>) -> gtk::Widget {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.add_css_class("design-icon-picker");
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search icons"));
    body.append(&search);
    let grid = gtk::FlowBox::new();
    grid.set_max_children_per_line(8);
    grid.set_min_children_per_line(8);
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&grid));
    scroll.set_min_content_height(260);
    scroll.set_max_content_height(320);
    scroll.set_propagate_natural_height(true);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    body.append(&scroll);
    let nothing = crate::app::label("No icon matches", "dim");
    nothing.set_halign(gtk::Align::Center);
    nothing.set_margin_top(12);
    nothing.set_visible(false);
    body.append(&nothing);
    // Each key with the words it is found by: an icon's name, a saved one's keywords too.
    let keys: Keys = Rc::default();
    let cell = {
        let (b, pop, grid, keys) = (Rc::downgrade(b), pop.downgrade(), grid.clone(), keys.clone());
        move |name: String, own: Option<String>, words: Vec<String>, picture: gtk::Image| {
            let key = gtk::Button::new();
            key.add_css_class("design-icon-cell");
            key.set_child(Some(&picture));
            key.set_tooltip_text(Some(&name));
            let (weak, p, target, chosen) = (b.clone(), pop.clone(), target.clone(), name.clone());
            key.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(b) = weak.upgrade() {
                    choose(&b, &chosen, own.as_deref(), target.as_deref());
                }
            });
            grid.insert(&key, -1);
            keys.borrow_mut().push((name, words, key));
        }
    };
    for name in ICONS {
        cell(name.to_string(), None, Vec::new(), image(name, 20));
    }
    // The icons saved to Relay's set (design.icon.save), after Relay's own, when the engine says.
    if let Some(ui) = b.ui.upgrade() {
        let (cell, search) = (cell.clone(), search.downgrade());
        glib::spawn_future_local(async move {
            let Ok(listed) = ui.call("design.icon.list", json!({})).await else { return };
            for icon in listed["saved"].as_array().into_iter().flatten() {
                let (Some(name), Some(d)) = (icon["name"].as_str(), icon["path"].as_str()) else { continue };
                let words = icon["keywords"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(str::to_string)).collect();
                cell(name.to_string(), Some(d.to_string()), words, outline(d, 20));
            }
            // What is typed already sorts the new keys too.
            if let Some(search) = search.upgrade() {
                search.emit_by_name::<()>("search-changed", &[]);
            }
        });
    }
    let k = keys.clone();
    search.connect_search_changed(move |s| {
        let q = s.text();
        let mut any = false;
        for (name, words, key) in k.borrow().iter() {
            if let Some(cell) = key.parent() {
                let shown = matches(name, &q) || (!q.trim().is_empty() && words.iter().any(|w| matches(w, &q)));
                cell.set_visible(shown);
                any |= shown;
            }
        }
        nothing.set_visible(!any);
    });
    // Enter takes the first icon showing.
    search.connect_activate(move |_| {
        let first = keys.borrow().iter().find(|(_, _, key)| key.parent().is_some_and(|c| c.is_visible())).map(|(_, _, key)| key.clone());
        if let Some(key) = first {
            key.emit_clicked();
        }
    });
    // Typing searches as soon as the picker opens.
    let s = search.downgrade();
    glib::idle_add_local_once(move || {
        if let Some(s) = s.upgrade() {
            s.grab_focus();
        }
    });
    body.set_size_request(320, -1);
    body.upcast()
}

/// Words a person may look for an icon by, and the icon each finds (E-board-8).
const SYNONYMS: &[(&str, &str)] = &[
    ("close", "x"), ("cancel", "x"), ("cross", "x"), ("delete", "trash"), ("remove", "trash"), ("bin", "trash"),
    ("gear", "settings"), ("cog", "settings"), ("preferences", "settings"), ("find", "search"), ("magnifier", "search"),
    ("house", "home"), ("person", "user"), ("profile", "user"), ("account", "user"), ("people", "users"), ("team", "users"),
    ("notification", "bell"), ("email", "mail"), ("envelope", "mail"), ("like", "heart"), ("favourite", "star"),
    ("favorite", "star"), ("add", "plus"), ("new", "plus"), ("tick", "check"), ("done", "check"), ("ok", "check"),
    ("back", "arrow-left"), ("next", "arrow-right"), ("forward", "arrow-right"), ("hamburger", "menu"),
    ("pencil", "edit"), ("write", "edit"), ("duplicate", "copy"), ("photo", "image"), ("picture", "image"),
    ("cart", "shopping-cart"), ("basket", "shopping-cart"), ("payment", "credit-card"), ("location", "map-pin"),
    ("place", "map-pin"), ("chat", "message"), ("comment", "message"), ("reload", "refresh"), ("sync", "refresh"),
    ("logout", "log-out"), ("warning", "alert"), ("error", "alert"), ("question", "help"),
    ("chart", "bar-chart"), ("graph", "bar-chart"), ("stats", "bar-chart"), ("dark", "moon"), ("light", "sun"),
    ("visible", "eye"), ("hidden", "eye-off"), ("private", "lock"), ("time", "clock"), ("date", "calendar"),
    ("document", "file"), ("directory", "folder"), ("world", "globe"), ("web", "globe"), ("call", "phone"),
    ("lightning", "zap"), ("power", "zap"), ("branch", "git-branch"), ("console", "terminal"),
    ("money", "wallet"), ("budget", "wallet"), ("cash", "banknote"), ("banking", "bank"), ("finance", "bank"),
    ("bill", "receipt"), ("invoice", "receipt"), ("price", "tag"), ("label", "tag"), ("savings", "coins"),
    ("income", "trending-up"), ("expense", "trending-down"), ("food", "utensils"), ("restaurant", "utensils"),
    ("dining", "utensils"), ("cafe", "coffee"), ("drink", "coffee"), ("transport", "bus"), ("travel", "plane"),
    ("flight", "plane"), ("gas", "fuel"), ("petrol", "fuel"), ("work", "briefcase"), ("job", "briefcase"),
    ("school", "graduation-cap"), ("education", "graduation-cap"), ("health", "activity"), ("medicine", "pill"),
    ("fitness", "dumbbell"), ("gym", "dumbbell"), ("clothes", "shirt"), ("shopping", "shopping-bag"),
    ("present", "gift"), ("utilities", "lightbulb"), ("idea", "lightbulb"), ("water", "droplet"), ("fire", "flame"),
    ("nature", "leaf"), ("weather", "umbrella"), ("temperature", "thermometer"), ("attachment", "paperclip"),
    ("mobile", "smartphone"), ("computer", "laptop"), ("shipping", "truck"), ("shop", "store"), ("box", "package"),
    ("loop", "repeat"), ("random", "shuffle"), ("sound", "volume"), ("audio", "volume"), ("fullscreen", "maximize"),
    ("achievement", "award"), ("goal", "target"), ("security", "shield"), ("password", "key"),
    ("microphone", "mic"), ("movie", "film"), ("game", "gamepad"), ("song", "music"), ("reading", "book"),
    ("magic", "sparkles"), ("stopwatch", "timer"), ("undo", "rotate-ccw"), ("login", "log-in"), ("invite", "user-plus"),
];

/// Whether icon `name` answers search `query`: each word of it (split on spaces and hyphens, any
/// case) is part of one of the name's hyphen-separated words, or starts a word that stands for
/// the icon (`close` finds `x`, `del` finds `trash`). No words shows every icon.
pub(super) fn matches(name: &str, query: &str) -> bool {
    let query = query.to_lowercase();
    query.split(|c: char| c.is_whitespace() || c == '-').filter(|word| !word.is_empty()).all(|word| {
        name.split('-').any(|part| part.contains(word))
            || SYNONYMS.iter().any(|(synonym, icon)| *icon == name && word.len() >= 2 && synonym.starts_with(word))
    })
}

/// Put icon `name` on the selected icon (`target`) or a new one; `own` is a saved icon's outline,
/// which the node keeps as its `icon_path` (a built-in one drops it).
pub(super) fn choose(b: &Rc<Board>, name: &str, own: Option<&str>, target: Option<&str>) {
    if let Some(id) = target {
        let mut set = Map::new();
        set.insert("icon".into(), json!(name));
        set.insert("icon_path".into(), json!(own));
        b.commit(vec![NodeOp::Update { id: id.to_string(), set }]);
        return;
    }
    let parent = b.paste_target();
    let origin = b.origin_of(&parent);
    let (w, h) = b.size.get();
    let (cx, cy) = b.to_page(w / 2.0, h / 2.0);
    let mut n = Node::new(NodeKind::Icon);
    n.id = b.doc.scene.borrow_mut().fresh_id(NodeKind::Icon);
    n.parent = Some(parent);
    n.icon = Some(name.to_string());
    n.icon_path = own.map(str::to_string);
    n.name = Some(name.to_string());
    (n.w, n.h) = (24.0, 24.0);
    n.x = (cx - origin.x - 12.0).round();
    n.y = (cy - origin.y - 12.0).round();
    let id = n.id.clone();
    if b.commit(vec![NodeOp::Add { node: n, index: None }]) {
        b.select(vec![id]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(query: &str) -> Vec<&'static str> {
        ICONS.iter().copied().filter(|name| matches(name, query)).collect()
    }

    #[test]
    fn icon_search_takes_words_in_any_order_and_synonyms() {
        assert_eq!(found("arrow right"), ["arrow-right"]);
        assert_eq!(found("right arrow"), ["arrow-right"]);
        assert_eq!(found("Arrow-Right"), ["arrow-right"]);
        assert_eq!(found("close"), ["x"]);
        assert_eq!(found("delete"), ["trash"]);
        assert_eq!(found("del"), ["trash"], "a synonym is found as it is typed");
        assert_eq!(found("gear"), ["settings"]);
        assert!(found("right").contains(&"chevron-right") && found("right").contains(&"arrow-right"));
        assert_eq!(found("eye off"), ["eye-off"]);
        assert_eq!(found("  "), ICONS, "no words shows them all");
        assert!(found("zebra").is_empty());
        // Every synonym names an icon of the set.
        for (word, icon) in SYNONYMS {
            assert!(ICONS.contains(icon), "{word} names {icon}, not in the set");
        }
    }

    /// GSK reads every icon the painter strokes: each of the set, and each in the plain form a
    /// saved icon is stored in.
    #[test]
    fn gsk_reads_every_icon_as_stored() {
        for name in ICONS {
            let d = relay_client::design::icons::path(name).unwrap();
            assert!(gtk::gsk::Path::parse(d).is_ok(), "{name}");
            let plain = relay_bus::design::path_data::normalize(d).unwrap();
            assert!(gtk::gsk::Path::parse(&plain).is_ok(), "{name} as stored: {plain}");
        }
    }
}
