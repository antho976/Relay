//! A scripted run of the board for the smoke harness (`RELAY_SMOKE_DESIGN_SCRIPT`): pointer
//! gestures and commands through the same code the pointer and the keys use, each step waiting
//! for the engine to answer, so a capture checks what the canvas does, not only how it looks.
//! Steps are `;`-separated; coordinates are page pixels.
//!
//! `select:a,b` `click:x,y` `dbl:x,y` `drag:x,y>x,y[:shift|alt]` `tool:rect` `type:words`
//! `endedit` `nudge:dx,dy` `set:{json}` `align:left` `layout:row` `duplicate` `delete` `group`
//! `frame` `ungroup` `copy` `paste` `undo` `redo` `ask:words` `zoom:1.5` `fit` `layers` `wait:ms`
//! `print`

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
