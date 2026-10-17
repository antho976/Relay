//! A scripted run of the board for the smoke harness (`RELAY_SMOKE_DESIGN_SCRIPT`): pointer
//! gestures and commands through the same code the pointer and the keys use, each step waiting
//! for the engine to answer, so a capture checks what the canvas does, not only how it looks.
//! Steps are `;`-separated; coordinates are page pixels.
//!
//! `select:a,b` `click:x,y` `dbl:x,y` `drag:x,y>x,y[:shift|alt]` `tool:rect` `type:words`
//! `endedit` `nudge:dx,dy` `set:{json}` `align:left` `layout:row` `duplicate` `delete` `group`
//! `frame` `ungroup` `copy` `paste` `undo` `redo` `ask:words` `zoom:1.5` `fit` `layers` `wait:ms`
//! `print`; and for the later tools `insert:kit/item@x,y` `pen-end` `points:id` `component`
//! `instance` `style:fill_style` `flip:x` `comment:x,y:words` `export:svg|pdf@path` `present`
//! `edit-range:a,b` `edit-style:weight` `assets` `version:label` `page:id`

use super::board::{Board, Tool};
use gtk::gdk;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::design::edit::Align;
use std::rc::Rc;

fn point(text: &str) -> Option<(f64, f64)> {
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn tool(name: &str) -> Option<Tool> {
    Some(match name {
        "select" => Tool::Select,
        "hand" => Tool::Hand,
        "frame" => Tool::Frame,
        "rect" => Tool::Rect,
        "ellipse" => Tool::Ellipse,
        "line" => Tool::Line,
        "arrow" => Tool::Arrow,
        "text" => Tool::Text,
        "sticky" => Tool::Sticky,
        "pen" => Tool::Pen,
        "highlighter" => Tool::Highlighter,
        "triangle" => Tool::Triangle,
        "diamond" => Tool::Diamond,
        "hexagon" => Tool::Hexagon,
        "star" => Tool::Star,
        "eraser" => Tool::Eraser,
        "vector" => Tool::Vector,
        "comment" => Tool::Comment,
        _ => return None,
    })
}

pub fn run(b: Rc<Board>, script: String) {
    let steps: Vec<String> = script.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    next(b, steps, 0);
}

fn next(b: Rc<Board>, steps: Vec<String>, at: usize) {
    let Some(step) = steps.get(at).cloned() else {
        println!("DESIGN_SCRIPT_DONE");
        return;
    };
    let (verb, arg) = step.split_once(':').unwrap_or((step.as_str(), ""));
    let mut pause = 0u64;
    match verb {
        "select" => b.select(arg.split(',').map(str::to_string).filter(|s| !s.is_empty()).collect()),
        "click" | "dbl" => {
            if let Some(p) = point(arg) {
                let times = if verb == "dbl" { 2 } else { 1 };
                for _ in 0..times {
                    super::tools::simulate(&b, p, p, gdk::ModifierType::empty());
                }
            }
        }
        "drag" => {
            let (path, mods) = arg.split_once(':').unwrap_or((arg, ""));
            let mods = match mods {
                "shift" => gdk::ModifierType::SHIFT_MASK,
                "alt" => gdk::ModifierType::ALT_MASK,
                _ => gdk::ModifierType::empty(),
            };
            if let Some((from, to)) = path.split_once('>').and_then(|(a, z)| Some((point(a)?, point(z)?))) {
                super::tools::simulate(&b, from, to, mods);
            }
        }
        "tool" => {
            if let Some(t) = tool(arg) {
                b.set_tool(t);
            }
        }
        "type" => {
            if let Some(edit) = b.edit.borrow().as_ref() {
                let buffer = edit.view.buffer();
                buffer.insert(&mut buffer.end_iter(), arg);
            }
        }
        "endedit" => b.end_edit(true),
        "nudge" => {
            if let Some((dx, dy)) = point(arg) {
                b.nudge(dx, dy);
            }
        }
        "set" => {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(arg) {
                b.set_selected(map);
            }
        }
        "align" => {
            let how = match arg {
                "left" => Some(Align::Left),
                "right" => Some(Align::Right),
                "top" => Some(Align::Top),
                "bottom" => Some(Align::Bottom),
                "cx" => Some(Align::CenterX),
                "cy" => Some(Align::CenterY),
                "dx" => Some(Align::DistributeX),
                "dy" => Some(Align::DistributeY),
                _ => None,
            };
            if let Some(how) = how {
                b.align(how);
            }
        }
        "layout" => b.set_layout(arg),
        "duplicate" => b.duplicate(),
        "delete" => b.delete_selection(),
        "group" => b.group(relay_bus::design::NodeKind::Group),
        "frame" => b.group(relay_bus::design::NodeKind::Frame),
        "ungroup" => b.ungroup(),
        "copy" => b.copy(),
        "paste" => {
            b.paste();
            pause = 300;
        }
        "ask" => {
            super::ask::ask_now(&b, arg);
            pause = 400;
        }
        "undo" => b.undo(),
        "undo-agent" => {
            b.undo_agent();
            pause = 600;
        }
        "redo" => b.redo(),
        "zoom" => {
            if let Ok(z) = arg.parse() {
                b.zoom_to(z);
            }
        }
        "fit" => {
            let r = b.page_bounds();
            b.fit(r, false);
        }
        "layers" => {
            if let Some(key) = super::tools::find_named(b.overlay.upcast_ref(), "design-layers-key").and_downcast::<gtk::ToggleButton>() {
                key.set_active(!key.is_active());
            }
        }
        "pan" => {
            // `pan:dx,dy,frames`: pan by (dx, dy) screen pixels on each of `frames` frames.
            let parts: Vec<f64> = arg.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if let [dx, dy, frames] = parts.as_slice() {
                let (dx, dy) = (*dx, *dy);
                let left = std::cell::Cell::new(*frames as u32);
                let board = b.clone();
                b.canvas.add_tick_callback(move |_, _| {
                    let mut cam = board.cam.get();
                    cam.pan(dx, dy);
                    board.set_cam(cam);
                    left.set(left.get().saturating_sub(1));
                    if left.get() == 0 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
                });
                pause = (*frames as u64) * 20 + 200;
            }
        }
        "image" => {
            // `image:path@x,y`: place a picture file at a page point.
            let (path, at) = arg.split_once('@').map_or((arg, None), |(p, at)| (p, point(at)));
            super::assets::place_files(&b, vec![std::path::PathBuf::from(path)], at);
            pause = 600;
        }
        "icon" => super::icons::choose(&b, arg, None),
        "frame-preset" => {
            if let Some((name, w, h)) = super::board::FRAME_PRESETS.iter().find(|p| p.0 == arg) {
                b.place_frame(name, *w as f64, *h as f64);
            }
        }
        "rotate-drag" => {
            // `rotate-drag:degrees`: turn the one selected node by dragging just outside its
            // top-right corner, as the pointer would.
            let id = b.selection.borrow().first().cloned();
            if let (Some(id), Ok(deg)) = (id, arg.parse::<f64>()) {
                if let Some(r) = b.placed().rect(&id) {
                    let zoom = b.cam.get().zoom;
                    let turn = b.doc.scene.borrow().node(&id).and_then(|n| n.rotation).unwrap_or(0.0);
                    let corner = relay_client::design::shapes::rotate(r.right() + 10.0 / zoom, r.y - 10.0 / zoom, r.cx(), r.cy(), turn);
                    let end = relay_client::design::shapes::rotate(corner.0, corner.1, r.cx(), r.cy(), deg);
                    super::tools::simulate(&b, corner, end, gdk::ModifierType::empty());
                }
            }
        }
        "insert" => {
            let (path, at) = arg.split_once('@').map_or((arg, None), |(p, at)| (p, point(at)));
            b.insert_item(path, at);
        }
        "pen-end" => b.finish_pen(),
        "points" => b.edit_points(arg),
        "component" => b.make_component(),
        "instance" => b.create_instance(),
        "style" => {
            let which: &'static str = match arg {
                "text_style" => "text_style",
                "effect_style" => "effect_style",
                _ => "fill_style",
            };
            b.make_style_of(which);
        }
        "flip" => b.flip(if arg == "y" { "flip_y" } else { "flip_x" }),
        "comment" => {
            // `comment:x,y:words`: pin a comment there and post it.
            let (at, words) = arg.split_once(':').unwrap_or((arg, "A comment"));
            if let Some(p) = point(at) {
                let parent = b.page.borrow().clone();
                let mut n = relay_bus::design::Node::new(relay_bus::design::NodeKind::Comment);
                n.id = b.doc.scene.borrow_mut().fresh_id(relay_bus::design::NodeKind::Comment);
                n.parent = Some(parent);
                (n.x, n.y) = p;
                n.text = Some(words.to_string());
                b.commit(vec![relay_bus::design::NodeOp::Add { node: n, index: None }]);
            }
        }
        "export" => {
            let (kind, path) = arg.split_once('@').unwrap_or((arg, "/tmp/design.svg"));
            let kind = if kind == "pdf" { super::export::Vector::Pdf } else { super::export::Vector::Svg };
            match super::export::vector_bytes(&b, &b.selected(), kind).and_then(|bytes| std::fs::write(path, &bytes).map(|_| bytes.len()).map_err(|e| e.to_string())) {
                Ok(n) => println!("DESIGN_EXPORT {path} {n}"),
                Err(e) => println!("DESIGN_EXPORT_FAILED {e}"),
            }
        }
        "present" => {
            b.present();
            pause = 500;
        }
        // `present-end[:path]`: capture the presentation (when given a path) and close it.
        "present-end" => super::present::capture_and_close((!arg.is_empty()).then_some(arg)),
        "edit-range" => {
            if let (Some((a, z)), Some(edit)) = (point(arg), b.edit.borrow().as_ref()) {
                let buffer = edit.view.buffer();
                buffer.select_range(&buffer.iter_at_offset(a as i32), &buffer.iter_at_offset(z as i32));
            }
        }
        "edit-style" => b.toggle_run(arg, None),
        "assets" => {
            if let Some(l) = b.layers.borrow().as_ref() {
                l.tabs.set_visible_child_name("assets");
            }
        }
        "page" => b.set_page(arg),
        "version" => {
            if let Some(ui) = b.ui.upgrade() {
                let design = b.doc.id;
                let label = arg.to_string();
                glib::spawn_future_local(async move {
                    let got = ui.call("design.version.save", serde_json::json!({"design_id": design, "label": label})).await;
                    println!("DESIGN_VERSION {}", got.map(|v| v.to_string()).unwrap_or_else(|e| format!("failed {e}")));
                });
            }
            pause = 300;
        }
        "wait" => pause = arg.parse().unwrap_or(200),
        "print" => {
            let nodes = b.doc.scene.borrow().to_nodes();
            println!("DESIGN_SCENE {}", serde_json::to_string(&nodes).unwrap_or_default());
            println!("DESIGN_SELECTION {}", serde_json::to_string(&*b.selection.borrow()).unwrap_or_default());
        }
        other => eprintln!("design script: unknown step {other:?}"),
    }
    // Wait for the engine to answer what the step sent, then a beat for the redraw.
    settle(b, steps, at, pause, 0);
}

fn settle(b: Rc<Board>, steps: Vec<String>, at: usize, pause: u64, tries: u32) {
    glib::timeout_add_local_once(std::time::Duration::from_millis(pause.max(60)), move || {
        if b.doc.busy() && tries < 60 {
            settle(b, steps, at, 0, tries + 1);
        } else {
            next(b, steps, at + 1);
        }
    });
}
