//! A scripted run of the board for the smoke harness (`RELAY_SMOKE_DESIGN_SCRIPT`): pointer
//! gestures and commands through the same code the pointer and the keys use, each step waiting
//! for the engine to answer, so a capture checks what the canvas does, not only how it looks.
//! Steps are `;`-separated; coordinates are page pixels.
//!
//! `select:a,b` `click:x,y` `dbl:x,y` `drag:x,y>x,y[:shift|alt]` `tool:rect` `type:words`
//! `endedit` `nudge:dx,dy` `set:{json}` `align:left` `layout:row` `duplicate` `delete` `group`
//! `frame` `ungroup` `copy` `paste` `undo` `redo` `ask:words` `zoom:1.5` `fit` `layers` `wait:ms`
//! `print` (the scene, the selection and the colours and styles); `mark:name` prints
//! `DESIGN_MARK name`, for a harness that acts between steps (as an agent would); `chrome`
//! prints `DESIGN_CHROME` with what the top bar and the toast say, whether Undo agent shows and
//! whether a text edit is open and on screen.
//!
//! A click is three steps, `press:x,y[:shift|alt]`, `idle` (the main loop runs, frames draw)
//! and `release:x,y`, so a press that lasts frames is exercised as a person's is (I-12); `click`
//! is those three, and `release` elsewhere than the press drags there.
//!
//! Motion (feature-motion §3): `play` (from the start, or on from a pause), `pause`, `seek:ms`
//! (a paused frame; starts Play if it is off), `stop`; `sheet:n[@/abs/path.png]` writes a
//! contact sheet of the page's motion and prints `DESIGN_SHEET`; `apng:/abs/path.png[:fps]`
//! and `frames:/abs/dir[:fps]` export the page's motion and print `DESIGN_APNG` and
//! `DESIGN_FRAMES`, with how long the canvas's longest frame took meanwhile;
//! `png:/abs/path.png[@ms]` writes the page as the PNG export draws it, at 1×, at rest or at
//! `ms` of its motion, and prints `DESIGN_PNG`.
//!
//! Stock photos (pictures-SPEC-v2 §16, C8): `pictures:words[@pexels]` opens the image key's
//! popover and searches for the words, on Pexels if named (or shows the Turn on note where
//! Pictures is off); `picture-pick:n` places result `n` (0 first) as a click on it does;
//! `picture-caption` puts the selected photo's credit under it, as the inspector's Add caption
//! does. `inspector:Title` scrolls the inspector to its section of that title
//! (`inspector:Credit`), for a capture of it.

use super::board::{Board, Tool};
use super::export;
use gtk::gdk;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_client::design::edit::Align;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

thread_local! {
    /// Where the pointer was pressed, for the `release:` that ends it.
    static PRESSED: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
}

fn mods(name: &str) -> gdk::ModifierType {
    match name {
        "shift" => gdk::ModifierType::SHIFT_MASK,
        "alt" => gdk::ModifierType::ALT_MASK,
        _ => gdk::ModifierType::empty(),
    }
}

/// The script's steps, each `click:x,y` as `press:x,y`, `idle` and `release:x,y`.
fn steps(script: &str) -> Vec<String> {
    script
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .flat_map(|s| match s.strip_prefix("click:") {
            Some(at) => vec![format!("press:{at}"), "idle".to_string(), format!("release:{at}")],
            None => vec![s.to_string()],
        })
        .collect()
}

/// `path[:fps]`: a file or folder and frames a second (30 when not given or not a number).
fn path_fps(arg: &str) -> (&str, u16) {
    match arg.rsplit_once(':') {
        Some((path, fps)) if !path.is_empty() => match fps.parse::<u16>() {
            Ok(fps) if fps > 0 => (path, fps),
            _ => (arg, export::ANIMATION_FPS),
        },
        _ => (arg, export::ANIMATION_FPS),
    }
}

/// `text@ms`: the text and the time after its last `@`, if that is a number.
fn at_time(arg: &str) -> (&str, Option<f64>) {
    match arg.rsplit_once('@') {
        Some((head, ms)) => ms.parse().ok().filter(|ms: &f64| ms.is_finite()).map_or((arg, None), |ms| (head, Some(ms))),
        None => (arg, None),
    }
}

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
        _ => return None,
    })
}

pub fn run(b: Rc<Board>, script: String) {
    next(b, steps(&script), 0);
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
        "dbl" => {
            if let Some(p) = point(arg) {
                for _ in 0..2 {
                    super::tools::simulate(&b, p, p, gdk::ModifierType::empty());
                }
            }
        }
        "press" => {
            let (at, how) = arg.split_once(':').unwrap_or((arg, ""));
            if let Some(p) = point(at) {
                PRESSED.with(|c| c.set(Some(p)));
                super::tools::press(&b, p, mods(how));
            }
        }
        // The main loop runs between the steps around it: input, layout and frames.
        "idle" => {}
        "release" => {
            let (at, how) = arg.split_once(':').unwrap_or((arg, ""));
            if let Some(p) = point(at) {
                let from = PRESSED.with(|c| c.take()).unwrap_or(p);
                super::tools::release(&b, from, p, mods(how));
            }
        }
        "drag" => {
            let (path, how) = arg.split_once(':').unwrap_or((arg, ""));
            if let Some((from, to)) = path.split_once('>').and_then(|(a, z)| Some((point(a)?, point(z)?))) {
                super::tools::simulate(&b, from, to, mods(how));
            }
        }
        "play" => {
            if b.playing() {
                b.resume_play();
            } else {
                b.toggle_play();
            }
        }
        "pause" => b.pause_play(),
        "seek" => {
            if let Ok(ms) = arg.parse::<f64>() {
                if !b.playing() {
                    b.toggle_play();
                }
                b.seek(ms);
            }
        }
        "stop" => b.stop_play(),
        "sheet" => {
            let (n, path) = arg.split_once('@').unwrap_or((arg, ""));
            let frames = n.parse::<u8>().unwrap_or(export::ASK_FRAMES).clamp(2, 24);
            let path = if path.is_empty() { glib::user_runtime_dir().join(format!("relay-design-sheet-{}.png", std::process::id())) } else { path.into() };
            match export::contact_sheet(&b, &[], true, 1.0, 4096, frames) {
                Ok((texture, times)) => {
                    let written = std::fs::write(&path, texture.save_to_png_bytes());
                    let times: Vec<String> = times.iter().map(|t| format!("{t:.1}")).collect();
                    match written {
                        Ok(()) => println!("DESIGN_SHEET {} {}x{} times {}", path.display(), texture.width(), texture.height(), times.join(",")),
                        Err(e) => println!("DESIGN_SHEET failed: {e}"),
                    }
                }
                Err(e) => println!("DESIGN_SHEET failed: {}", e.message()),
            }
        }
        "png" => {
            let (path, at) = at_time(arg);
            let (shown, backdrop) = export::page_target(&b);
            let drawn = match at {
                Some(ms) => export::render_at(&b, &shown, backdrop, 1.0, ms).map(|(t, _, _)| t),
                None => export::extent(&b, &shown).ok_or(export::Failure::Empty).and_then(|area| export::render(&b, &shown, backdrop, export::Format::Png(1.0).scale(&area))).map(|(t, _)| t),
            };
            match drawn.map_err(|e| e.message().to_string()).and_then(|t| std::fs::write(path, t.save_to_png_bytes()).map(|_| t).map_err(|e| e.to_string())) {
                Ok(t) => println!("DESIGN_PNG {path} {}x{}", t.width(), t.height()),
                Err(e) => println!("DESIGN_PNG failed: {e}"),
            }
        }
        // Long: the next step waits for the export, however long it takes.
        "apng" | "frames" => return animate(b, steps, at, verb == "apng", arg.to_string()),
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
        "flip" => b.flip(arg != "v"),
        "copy-style" => b.copy_style(),
        "paste-style" => b.paste_style(),
        // The page as SVG or PDF, written to a path: what the Export keys save.
        "svg" | "pdf" => {
            let format = if verb == "svg" { super::export::Format::Svg } else { super::export::Format::Pdf };
            match super::export::vector(&b, &[], true, format) {
                Ok(bytes) => {
                    let _ = std::fs::write(arg, &bytes);
                    println!("DESIGN_EXPORT {verb} {} bytes", bytes.len());
                }
                Err(e) => println!("DESIGN_EXPORT {verb} failed: {e}"),
            }
        }
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
        "icon" => super::icons::choose(&b, arg, None, None),
        "pictures" => {
            super::pictures::drive_search(&b, arg);
            pause = 800;
        }
        // The download is not the board's: the next step waits for it a while.
        "picture-pick" => {
            super::pictures::drive_pick(&b, arg.parse().unwrap_or(0));
            pause = 1500;
        }
        "inspector" => {
            // The section's title is its first child; scroll so it sits at the top.
            let scroll = &b.inspector_scroll;
            let mut sec = b.inspector.first_child();
            while let Some(s) = sec {
                let titled = s.first_child().and_downcast::<gtk::Label>().is_some_and(|l| l.text() == arg);
                if titled {
                    if let Some(at) = s.compute_point(&b.inspector, &gtk::graphene::Point::new(0.0, 0.0)) {
                        scroll.vadjustment().set_value(at.y() as f64);
                    }
                    break;
                }
                sec = s.next_sibling();
            }
        }
        "picture-caption" => {
            let id = b.selection.borrow().first().cloned();
            if let Some(id) = id {
                super::pictures::add_caption(&b, &id);
            }
        }
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
        "wait" => pause = arg.parse().unwrap_or(200),
        "print" => {
            let scene = b.doc.scene.borrow();
            println!("DESIGN_SCENE {}", serde_json::to_string(&scene.to_nodes()).unwrap_or_default());
            println!("DESIGN_SELECTION {}", serde_json::to_string(&*b.selection.borrow()).unwrap_or_default());
            println!("DESIGN_STYLES {}", serde_json::to_string(scene.styles()).unwrap_or_default());
        }
        "mark" => println!("DESIGN_MARK {arg}"),
        "chrome" => {
            let edit = b.edit.borrow().as_ref().map(|e| e.view.is_mapped());
            // Where the edit view sits on the canvas, and the canvas's size: whether the typing shows.
            let edit_rect = b.edit.borrow().as_ref().and_then(|e| e.view.compute_bounds(&b.canvas)).map(|r| [r.x(), r.y(), r.width(), r.height()]);
            let class = ["design-status-agent", "design-status-unsaved"].into_iter().find(|c| b.status.has_css_class(c));
            let chrome = serde_json::json!({
                "status": b.status.text().as_str(),
                "status_class": class,
                "toast": b.toast_label.text().as_str(),
                "toast_shown": b.toast.reveals_child(),
                "agent_undo": b.agent_undo.is_visible(),
                "agent_undo_tip": b.agent_undo.tooltip_text().map(|t| t.to_string()),
                "editing": edit.is_some(),
                "edit_mapped": edit == Some(true),
                "edit_rect": edit_rect,
                "canvas": [b.canvas.width(), b.canvas.height()],
                "layers_open": b.layers_panel.reveals_child(),
            });
            println!("DESIGN_CHROME {chrome}");
        }
        other => eprintln!("design script: unknown step {other:?}"),
    }
    // Wait for the engine to answer what the step sent, then a beat for the redraw.
    settle(b, steps, at, pause, 0);
}

/// Export the page's motion (`apng`: one APNG; else numbered PNGs in a folder) as the export
/// menu does, printing what was written, how long it took and the canvas's longest frame
/// meanwhile; then the next step.
fn animate(b: Rc<Board>, steps: Vec<String>, at: usize, apng: bool, arg: String) {
    // The canvas's frames while the export runs: the longest gap between two is what a person
    // would feel.
    let gaps = Rc::new(Cell::new((0i64, 0i64, 0u32)));
    let g = gaps.clone();
    let tick = b.canvas.add_tick_callback(move |_, clock| {
        let (last, most, n) = g.get();
        let now = clock.frame_time();
        g.set((now, if last > 0 { most.max(now - last) } else { most }, n + 1));
        glib::ControlFlow::Continue
    });
    let weak = Rc::downgrade(&b);
    glib::spawn_future_local(async move {
        let (path, fps) = path_fps(&arg);
        let (shown, backdrop) = export::page_target(&b);
        let started = Instant::now();
        let said = if apng {
            match export::animation_bytes(&b, shown, backdrop, fps).await {
                Ok((bytes, drawn)) => std::fs::write(path, &bytes)
                    .map(|_| format!("DESIGN_APNG {path} {} frames {}x{} {} bytes", drawn.count, drawn.size.0, drawn.size.1, bytes.len()))
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            }
        } else {
            let stem = "frame".to_string();
            match export::write_frames(&b, shown, backdrop, fps, path.into(), stem).await {
                Ok((count, _, drawn)) => Ok(format!("DESIGN_FRAMES {path} {count} frames {}x{}", drawn.size.0, drawn.size.1)),
                Err(e) => Err(e),
            }
        };
        tick.remove();
        let (_, most, frames) = gaps.get();
        let took = started.elapsed().as_secs_f64() * 1e3;
        match said {
            Ok(line) => println!("{line} in {took:.0} ms; canvas {frames} frames, longest {:.1} ms", most as f64 / 1e3),
            Err(e) => println!("DESIGN_{} failed: {e}", if apng { "APNG" } else { "FRAMES" }),
        }
        drop(b);
        if let Some(b) = weak.upgrade() {
            settle(b, steps, at, 0, 0);
        }
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// I-12: a click is a press, a turn of the main loop and a release, so a press that lasts
    /// frames is what the script exercises; nothing else is rewritten.
    #[test]
    fn a_click_is_a_press_an_idle_and_a_release() {
        assert_eq!(steps(" click:10,20 ; drag:0,0>5,5:shift;;print "), ["press:10,20", "idle", "release:10,20", "drag:0,0>5,5:shift", "print"]);
        assert_eq!(steps("dbl:1,2"), ["dbl:1,2"]);
        assert_eq!(mods("shift"), gdk::ModifierType::SHIFT_MASK);
        assert_eq!(mods(""), gdk::ModifierType::empty());
    }

    #[test]
    fn motion_steps_read_their_paths_and_times() {
        assert_eq!(path_fps("/tmp/x.png"), ("/tmp/x.png", export::ANIMATION_FPS));
        assert_eq!(path_fps("/tmp/x.png:12"), ("/tmp/x.png", 12));
        assert_eq!(path_fps("/tmp/x.png:0"), ("/tmp/x.png:0", export::ANIMATION_FPS));
        assert_eq!(path_fps("/tmp/dir"), ("/tmp/dir", export::ANIMATION_FPS));
        assert_eq!(at_time("/tmp/a.png@1200"), ("/tmp/a.png", Some(1200.0)));
        assert_eq!(at_time("/tmp/a.png"), ("/tmp/a.png", None));
        assert_eq!(at_time("/tmp/me@home.png"), ("/tmp/me@home.png", None));
    }
}
