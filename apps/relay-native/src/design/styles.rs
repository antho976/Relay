//! Shared styles: colours, type and effects kept once and used everywhere. A style is a node
//! marked `shared` on the design's Styles page (a swatch, a sample of type, a card with the
//! shadow), so it is seen and edited like anything else; nodes name it in `fill_style`,
//! `text_style` or `effect_style`, and the engine keeps their fields in step with it. Setting
//! one of those fields by hand lets go of the style.

use super::board::{display_name, Board};
use super::paint::{fill_of, rgba};
use crate::app::label;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_bus::design::{Node, NodeKind, NodeOp};
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// The page styles live on.
pub const STYLES_PAGE: &str = "styles";

/// Which reference a style kind fills, and what a node must have to be one.
fn fits(which: &str, n: &Node) -> bool {
    n.shared == Some(true)
        && match which {
            "fill_style" => n.kind != NodeKind::Text && (n.fill.is_some() || n.gradient.is_some()),
            "text_style" => n.kind == NodeKind::Text,
            "effect_style" => n.shadow.is_some() || n.blur.is_some(),
            _ => false,
        }
}

impl Board {
    /// The design's styles of a kind, by name.
    pub(super) fn styles_of(&self, which: &str) -> Vec<Node> {
        let scene = self.doc.scene.borrow();
        let mut out: Vec<Node> = scene.nodes().filter(|n| fits(which, n)).cloned().collect();
        out.sort_by_key(|n| display_name(n).to_lowercase());
        out
    }

    /// A row choosing the selection's style for `which`, or none.
    pub(super) fn style_picker(self: &Rc<Self>, sec: &gtk::Box, nodes: &[Node], which: &'static str) {
        let styles = self.styles_of(which);
        let current = {
            let first = nodes.first().and_then(|n| n.style_ref(which).map(str::to_string));
            nodes.iter().all(|n| n.style_ref(which).map(str::to_string) == first).then_some(first).flatten()
        };
        if styles.is_empty() && current.is_none() {
            let make = gtk::Button::with_label(match which {
                "text_style" => "Make a text style of this",
                "effect_style" => "Make an effect style of this",
                _ => "Make a colour style of this",
            });
            make.add_css_class("quiet");
            make.set_halign(gtk::Align::Start);
            let weak = Rc::downgrade(self);
            make.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.make_style_of(which);
                }
            });
            sec.append(&make);
            return;
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.append(&super::glyphs::image("style", 14));
        let mut names = vec!["No style".to_string()];
        names.extend(styles.iter().map(display_name));
        let dd = gtk::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
        dd.add_css_class("design-dropdown");
        dd.set_hexpand(true);
        dd.set_tooltip_text(Some("A shared style: change it once and everything using it follows"));
        let index = match &current {
            Some(id) => styles.iter().position(|s| s.id == *id).map(|i| i + 1),
            None => Some(0),
        };
        dd.set_selected(index.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
        let weak = Rc::downgrade(self);
        let ids: Vec<String> = styles.iter().map(|s| s.id.clone()).collect();
        dd.connect_selected_notify(move |dd| {
            let Some(b) = weak.upgrade() else { return };
            let i = dd.selected() as usize;
            let mut set = Map::new();
            set.insert(which.into(), if i == 0 { Value::Null } else { ids.get(i - 1).map_or(Value::Null, |id| json!(id)) });
            b.set_selected(set);
        });
        row.append(&dd);
        let make = crate::app::icon_button("plus", "Make a new style of this");
        let weak = Rc::downgrade(self);
        make.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.make_style_of(which);
            }
        });
        row.append(&make);
        sec.append(&row);
    }

    /// The context menu's "Make a style": type for a text, else its colour (and its shadow).
    pub(super) fn make_style(&self) {
        let kind = self.selected().first().and_then(|id| self.doc.scene.borrow().node(id).map(|n| n.kind));
        match kind {
            Some(NodeKind::Text) => self.make_style_of("text_style"),
            Some(_) => self.make_style_of("fill_style"),
            None => {}
        }
    }

    /// A new style from the selected node, on the Styles page, which the node then uses.
    pub(super) fn make_style_of(&self, which: &str) {
        let Some(id) = self.selected().first().cloned() else { return };
        let Some(n) = self.doc.scene.borrow().node(&id).cloned() else { return };
        let mut ops = Vec::new();
        let page_exists = self.doc.scene.borrow().node(STYLES_PAGE).is_some();
        if !page_exists {
            let mut page = Node::new(NodeKind::Page);
            page.id = STYLES_PAGE.into();
            page.name = Some("Styles".into());
            ops.push(NodeOp::Add { node: page, index: None });
        }
        let count = self.styles_of(which).len();
        let mut style = match which {
            "text_style" => {
                let mut s = Node::new(NodeKind::Text);
                s.text = Some(format!("{} · {}px", super::paint::font_family(&n), super::paint::font_size(&n)));
                (s.font, s.font_size, s.font_weight, s.line_height, s.letter_spacing, s.italic) =
                    (n.font.clone(), n.font_size, n.font_weight, n.line_height, n.letter_spacing, n.italic);
                s.name = Some(format!("Text {}", count + 1));
                (s.x, s.y) = (480.0, count as f64 * 72.0);
                s
            }
            "effect_style" => {
                let mut s = Node::new(NodeKind::Rect);
                (s.w, s.h, s.radius, s.fill) = (120.0, 80.0, Some(12.0), Some("#FFFFFF".into()));
                (s.shadow, s.blur) = (n.shadow.clone(), n.blur);
                s.name = Some(format!("Effect {}", count + 1));
                (s.x, s.y) = (900.0, count as f64 * 120.0);
                s
            }
            _ => {
                let mut s = Node::new(NodeKind::Rect);
                (s.w, s.h, s.radius) = (80.0, 80.0, Some(12.0));
                (s.fill, s.gradient) = (fill_of(&n), n.gradient.clone());
                s.name = Some(n.fill.clone().map_or_else(|| format!("Colour {}", count + 1), |f| f.to_uppercase()));
                (s.x, s.y) = ((count % 4) as f64 * 100.0, (count / 4) as f64 * 100.0);
                s
            }
        };
        style.id = self.doc.scene.borrow_mut().fresh_named("style");
        style.parent = Some(STYLES_PAGE.into());
        style.shared = Some(true);
        let style_id = style.id.clone();
        ops.push(NodeOp::Add { node: style, index: None });
        let mut set = Map::new();
        set.insert(which.into(), json!(style_id));
        ops.push(NodeOp::Update { id, set });
        if self.commit(ops) {
            self.toast("Made a style on the Styles page: rename it there, change it once, and everything using it follows");
        }
    }

    /// The Assets panel's styles: a click gives the selection that style.
    pub(super) fn styles_list(self: &Rc<Self>, q: &str) -> Option<gtk::Box> {
        let all: Vec<(&'static str, Node)> = ["fill_style", "text_style", "effect_style"]
            .into_iter()
            .flat_map(|w| self.styles_of(w).into_iter().map(move |n| (w, n)))
            .filter(|(_, n)| q.is_empty() || display_name(n).to_lowercase().contains(q))
            .collect();
        if all.is_empty() {
            return None;
        }
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        list.set_margin_start(8);
        list.set_margin_end(8);
        for (which, n) in all {
            let key = gtk::Button::new();
            key.add_css_class("design-menu-row");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let swatch = gtk::DrawingArea::new();
            swatch.set_content_width(18);
            swatch.set_content_height(18);
            let color = fill_of(&n).unwrap_or_else(|| "#00000000".into());
            let text = which == "text_style";
            swatch.set_draw_func(move |_, cr, w, h| {
                if text {
                    cr.set_source_rgba(0.6, 0.6, 0.6, 1.0);
                    cr.select_font_face("Geist", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Bold);
                    cr.set_font_size(13.0);
                    cr.move_to(1.0, h as f64 - 4.0);
                    let _ = cr.show_text("Ag");
                    return;
                }
                let c = rgba(&color);
                cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
                cr.arc(w as f64 / 2.0, h as f64 / 2.0, w.min(h) as f64 / 2.0 - 1.0, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            });
            row.append(&swatch);
            let name = label(&display_name(&n), "");
            name.set_hexpand(true);
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&name);
            row.append(&label(match which {
                "text_style" => "Type",
                "effect_style" => "Effect",
                _ => "Colour",
            }, "faint"));
            key.set_child(Some(&row));
            key.set_tooltip_text(Some("Give the selection this style"));
            let weak = Rc::downgrade(self);
            let id = n.id.clone();
            key.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    if b.selected().is_empty() {
                        b.toast("Select what should take the style first");
                        return;
                    }
                    let mut set = Map::new();
                    set.insert(which.into(), json!(id));
                    b.set_selected(set);
                }
            });
            list.append(&key);
        }
        Some(list)
    }
}
