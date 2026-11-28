//! The desktop build: a phone-shaped window for working on Relay Code with the mouse and keyboard
//! (it pairs and connects like the phone), and a screenshot mode that needs no window and no PC.
//!
//! - `--shot out.png --screen NAME` draws one screen to a file from sample data (`sample.rs`):
//!   `pair`, `home`, `drawer`, `repos`, `chat`, `models`, `commands`, `new-agent`, `settings`.
//!   `--size WxH` (default 1080x2340).
//! - `--data DIR` keeps the app's files somewhere else than `~/.local/share/relay-code`.
//!
//! Keys: Escape goes back, Enter is the keyboard's Done, Backspace deletes.

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::app::{App, Host, Next, Route};
use crate::clock::now_ns;
use crate::gfx::{Gfx, Screen};
use crate::ui::{Phase, Touch};

struct DesktopHost {
    data: PathBuf,
    /// Pixels per dp a shot pretends to have (`--density`), 0 for a window.
    density: f32,
    /// Android's font scale a shot pretends to have (`--font-scale`), 1 for a window.
    font_scale: f32,
}

impl Host for DesktopHost {
    fn density(&self) -> f32 {
        self.density
    }

    fn font_scale(&self) -> f32 {
        self.font_scale
    }

    fn data_dir(&self) -> PathBuf {
        self.data.clone()
    }
    fn log(&self, text: &str) {
        eprintln!("relay-code: {text}");
    }
    fn copy_text(&self, text: &str) {
        eprintln!("relay-code: copied {} characters", text.chars().count());
    }
    fn open_url(&self, url: &str) {
        // Only web addresses, handed to the desktop's own opener as one argument.
        if url.starts_with("https://") || url.starts_with("http://") {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }
    /// The file named by `RELAY_CODE_PICK` is the one picked (a copy: the upload removes it).
    fn pick_photo(&self) {
        let Some(src) = std::env::var_os("RELAY_CODE_PICK") else {
            eprintln!("relay-code: pick a photo: set RELAY_CODE_PICK to its path");
            return;
        };
        let dest = self.data.join(format!("pick-{}", crate::clock::now_ns()));
        if std::fs::copy(&src, &dest).is_ok() {
            crate::attach::picked(dest);
        }
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn data_dir(args: &[String]) -> PathBuf {
    arg(args, "--data").map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("relay-code")
    })
}

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(out) = arg(&args, "--shot") {
        if let Err(e) = shot(&args, &out) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let event_loop = EventLoop::<()>::with_user_event().build().expect("an event loop");
    let proxy: EventLoopProxy<()> = event_loop.create_proxy();
    crate::wake::set(Box::new(move || {
        let _ = proxy.send_event(());
    }));
    let mut window = Windowed { app: None, gfx: None, window: None, next: Next::Now, cursor: (0.0, 0.0), mouse_down: false, data: data_dir(&args) };
    event_loop.run_app(&mut window).expect("the event loop ran");
}

fn settle(app: &mut App, gfx: &Gfx, clock: &mut u64) {
    for _ in 0..20 {
        *clock += 16_666_666;
        app.frame(*clock);
        gfx.upload_glyphs(&mut app.canvas.uploads);
        gfx.upload_images(&mut app.canvas.image_uploads);
        gfx.flush();
    }
}

/// One frame of a screen, drawn with no window, from sample data.
fn shot(args: &[String], out: &str) -> Result<(), String> {
    let (w, h) = arg(args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))))
        .unwrap_or((1080, 2340));
    let screen = arg(args, "--screen").unwrap_or_else(|| "chat".into());
    let dir = std::env::temp_dir().join(format!("relay-code-shot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut gfx = Gfx::headless()?;
    let mut clock = 1_000_000_000u64;
    // `--density 2` is a tablet's 320 dpi; `--text small|large` the Settings choice (D9).
    let density = arg(args, "--density").and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let font_scale = arg(args, "--font-scale").and_then(|d| d.parse().ok()).unwrap_or(1.0);
    let mut app = App::new(Arc::new(DesktopHost { data: dir.clone(), density, font_scale }), clock);
    app.canvas.set_images_size(gfx.images_size());
    // Roughly a phone's status bar and gesture bar.
    app.resize(w as f32, h as f32, w as f32 * 0.085, w as f32 * 0.04);
    match arg(args, "--text").as_deref() {
        Some("small") => app.set_text_size(0),
        Some("large") => app.set_text_size(2),
        _ => {}
    }
    if !screen.starts_with("first-run") && !screen.starts_with("pair-") {
        app.sample();
    } else {
        app.shot = true;
    }
    match screen.as_str() {
        "first-run" => app.reset_to(Route::Pair),
        // D8: the pairing screen waiting on Discord, allowed, by link, and with an error.
        "pair-waiting" | "pair-allowed" | "pair-link" | "pair-error" => {
            app.reset_to(Route::Pair);
            match screen.as_str() {
                "pair-waiting" | "pair-allowed" => crate::pair::sample_waiting(&mut app, screen == "pair-allowed"),
                "pair-link" => app.st.pair.by_link = true,
                _ => {
                    app.st.pair.server = Some("example.org".into());
                    app.st.pair.error = Some("Your server could not be reached. Check the connection, then try again.".into());
                }
            }
        }
        "home" => app.reset_to(Route::Home),
        "drawer" | "repos" => {
            app.open("bold-puffin");
            app.st.drawer.open = true;
            app.st.drawer.repos = screen == "repos";
        }
        "chat" | "models" | "commands" | "bang" | "chat-offline" => {
            app.open("bold-puffin");
            if screen == "chat-offline" {
                // A message written while the link reconnects waits for it.
                app.set_link_state(rc_link::LinkState::Offline { reason: "No route".into(), retry_at_ms: 0 });
            }
            app.st.chat.model_sheet = screen == "models";
            if screen == "commands" || screen == "bang" {
                app.focus.id = Some("chat");
                app.focus.text = if screen == "bang" { "!" } else { "/" }.into();
            }
        }
        "asks" => app.open("umber-zebra"),
        // D13: the key strip over the message box, and the Terminal page.
        "keys" => {
            app.open("bold-puffin");
            app.st.keys.open = true;
        }
        "terminal" => {
            app.open("bold-puffin");
            app.push(Route::Terminal("bold-puffin".into()));
            crate::terminal::sample(&mut app, "bold-puffin");
        }
        // D10: an update from the PC, offered and downloading.
        "update" | "update-downloading" => {
            app.reset_to(Route::Home);
            crate::update::sample(&mut app, 312, (screen == "update-downloading").then_some(7_300_000));
        }
        // D1: a stopped agent's chat, a closed one's, the chat menu, its confirm, and search.
        "stopped" => app.open("keen-newt"),
        "closed" => app.open("vivid-dingo"),
        "agent-menu" => {
            app.open("bold-puffin");
            app.st.agents.menu = Some("bold-puffin".into());
        }
        "close-agent" => {
            app.open("bold-puffin");
            app.st.agents.confirm = Some(("bold-puffin".into(), crate::agents::Confirm::Close));
        }
        "search" => {
            app.open("bold-puffin");
            app.st.drawer.open = true;
            app.st.agents.query = "leaf".into();
        }
        // A3: what it did, call by call.
        "markdown" => {
            crate::sample::markdown(&mut app);
            app.open("swift-ocelot");
            app.scroll().off = 0.0;
        }
        "plan" | "tools" => {
            crate::sample::tools(&mut app, screen == "plan");
            app.open("swift-ocelot");
        }
        "asks-long" => {
            // A7: long answers wrap on their keys.
            app.open("umber-zebra");
            app.screens.insert("umber-zebra".into(), crate::screen::read("│ Bash command │\n│   git push -u origin relay/umber-zebra │\n│ Do you want to proceed? │\n│ ❯ 1. Yes │\n│   2. Yes, and don't ask again for git push commands in /home/anthony/dev3/RelayV4/.relay/worktrees/umber-zebra │\n│   3. No, and tell Claude what to do differently (esc) │\n  ⏵⏵ accept edits on (shift+tab to cycle)"));
        }
        "photo" => {
            // A9: a picture you sent shows as the picture; one the app no longer holds, as its name.
            let (w, h) = (400u32, 300u32);
            let pixels = (0..w * h).flat_map(|i| [(40 + (i % w) * 180 / w) as u8, 60, (120 + (i / w) * 100 / h) as u8, 255]).collect();
            crate::attach::keep_thumb("/wt/.relay/uploads/shot-1.png", crate::attach::Thumb { w, h, pixels });
            let mut c = crate::data::Convo::default();
            c.take(&serde_json::json!({"messages": [
                {"id": "p1", "role": "user", "body": {"text": "The send key overlaps the chip here\n/wt/.relay/uploads/shot-1.png"}, "created_at": ""},
                {"id": "p2", "role": "assistant", "body": {"blocks": [{"type": "text", "text": "I see it: the chip is drawn after the key. Moving it."}]}, "created_at": ""},
                {"id": "p3", "role": "user", "body": {"text": "/wt/.relay/uploads/older-shot.jpg"}, "created_at": ""},
            ], "cursor": 1, "reset": true, "found": true}));
            app.convos.insert("swift-ocelot".into(), c);
            app.open("swift-ocelot");
        }
        "days" | "menu" => {
            crate::sample::days(&mut app);
            app.open("swift-ocelot");
            if screen == "menu" {
                // Held on the agent's last words (the sixth item).
                app.st.chat.menu = Some((5, 640.0));
            }
        }
        "queued" => {
            // A15: one taken back after the PC had it, struck; the menu open on another.
            app.open("bold-puffin");
            let mut q = crate::data::Sending::default();
            q.push("Use the old matcher after all", 0);
            q.push("Push it when the tests pass", 0);
            q.waiting[0].stage = crate::data::Stage::Entered;
            q.take_back(0);
            app.sending.insert("bold-puffin".into(), q);
            let id = app.sending["bold-puffin"].waiting[1].client_id.clone();
            app.st.chat.queued_menu = Some((id, 470.0));
        }
        "find" => {
            // A16: find in a conversation, on the markdown sample.
            crate::sample::days(&mut app);
            app.open("swift-ocelot");
            crate::chat::find_open(&mut app);
            app.set_text("draft");
        }
        "helper" => {
            // A18: a Task call opened on what its helper did.
            crate::sample::helper(&mut app);
            app.open("swift-ocelot");
            app.frame(clock);
            crate::chat::unfold_all(&mut app);
        }
        "blocked" => {
            // Blocked by its own report (session.done blocked), with no card on its screen.
            app.open("umber-zebra");
            app.screens.insert("umber-zebra".into(), crate::screen::read("● I could not push: the remote refused.\n────\n❯ \n────\n  ? for shortcuts"));
        }
        "new-agent" => {
            app.reset_to(Route::Home);
            app.st.newagent.open = true;
            app.st.newagent.task = "Add a dark mode toggle to Settings".into();
            app.set_choice(Some("opus"), Some("high"));
            app.st.newagent.linked = Some((41, "Dark mode for Settings".into()));
        }
        "new-agent-tasks" => {
            app.reset_to(Route::Home);
            app.st.newagent.open = true;
            app.st.newagent.picking = true;
            app.st.newagent.tasks = app.st.board.tasks.clone();
        }
        "changes" => {
            app.reset_to(Route::Home);
            app.push(Route::Changes);
        }
        "board" => {
            app.reset_to(Route::Home);
            app.push(Route::Board);
        }
        "task" => {
            app.reset_to(Route::Home);
            app.push(Route::Board);
            app.push(Route::Task(41));
        }
        "inbox" => {
            app.reset_to(Route::Home);
            app.push(Route::Inbox);
        }
        "file" => {
            crate::fileview::sample(&mut app);
            app.reset_to(Route::Home);
            app.push(Route::File { repo: 1, worktree: None, path: "apps/relay-code/native/crates/relay-code/src/ui.rs:6".into() });
        }
        "designs" => {
            app.reset_to(Route::Home);
            app.push(Route::Designs);
        }
        "design" | "design-page" | "artboard" => {
            app.reset_to(Route::Home);
            app.push(Route::Designs);
            app.push(Route::Design(44));
            if let Some(o) = app.st.designs.open.as_mut() {
                o.whole = screen == "design-page";
            }
            if screen == "artboard" {
                crate::designs::sample_view(&mut app);
            }
        }
        "settings" => {
            app.reset_to(Route::Home);
            app.push(Route::Settings);
        }
        other => return Err(format!("unknown screen: {other}")),
    }
    // D14's states, over any screen: `--state offline` (the PC out of reach, what was kept still
    // shown), `connecting` (a first start: nothing kept yet), `empty` (a repo with nothing in it).
    let state = arg(args, "--state");
    if let Some(st) = state.as_deref() {
        if st == "offline" {
            app.set_link_state(rc_link::LinkState::Offline { reason: "No route to the PC".into(), retry_at_ms: rc_link::link::now_ms() + 12_000 });
        }
        if st == "connecting" || st == "empty" {
            app.agents.clear();
            app.earlier.clear();
            app.convos.clear();
            app.sending.clear();
            app.screens.clear();
            app.holds = 0;
            app.st.changes = Default::default();
            app.st.board = Default::default();
            app.st.inbox = Default::default();
            app.st.designs = Default::default();
            app.st.fileview = Default::default();
            app.st.terminal = Default::default();
        }
        if st == "error" {
            // A17: the chat's own errors, on whichever chat is open.
            if let Some(name) = app.open_chat() {
                let mut q = crate::data::Sending::default();
                q.push("Push it when the tests pass", 0);
                q.waiting[0].failed = true;
                app.sending.insert(name.clone(), q);
                app.chat_errors.insert(name, "Its log file is missing: the agent was started outside Relay, or the log was moved.".into());
            }
        }
        // Each page's error, so its problem state can be looked at (C16).
        if st == "error" {
            crate::changes::sample_error(&mut app);
            crate::board::sample_error(&mut app);
            crate::fileview::sample_error(&mut app);
            crate::designs::sample_error(&mut app);
            crate::inbox::sample_error(&mut app);
            app.agents.clear();
        }
        if st == "connecting" {
            app.projects.clear();
            app.repo = None;
            app.set_link_state(rc_link::LinkState::Connecting { attempt: 2 });
        }
    }
    settle(&mut app, &gfx, &mut clock);
    clock += 16_666_666;
    app.frame(clock);
    gfx.upload_glyphs(&mut app.canvas.uploads);
    gfx.upload_images(&mut app.canvas.image_uploads);
    let pixels = gfx.render_to_pixels(w, h, &app.canvas.globals, app.clear_colour(), &app.canvas.insts, &app.canvas.batches);
    let _ = std::fs::remove_dir_all(&dir);
    let file = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&pixels).map_err(|e| e.to_string())?;
    println!("{out}: {screen}, {w}x{h}, {} shapes in {} batches, adapter {}", app.canvas.insts.len(), app.canvas.batches.len(), gfx.adapter_name());
    Ok(())
}

struct Windowed {
    app: Option<App>,
    gfx: Option<(Gfx, Screen<'static>)>,
    window: Option<Arc<Window>>,
    next: Next,
    cursor: (f32, f32),
    mouse_down: bool,
    data: PathBuf,
}

impl Windowed {
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler<()> for Windowed {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Relay Code").with_inner_size(LogicalSize::new(412.0, 900.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("a window"));
        let size = window.inner_size();
        let (gfx, screen) = Gfx::with_surface(window.clone(), size.width, size.height).expect("graphics");
        let mut app = App::new(Arc::new(DesktopHost { data: self.data.clone(), density: 0.0, font_scale: 1.0 }), now_ns());
        app.canvas.set_images_size(gfx.images_size());
        app.resize(size.width as f32, size.height as f32, 0.0, 0.0);
        self.app = Some(app);
        self.gfx = Some((gfx, screen));
        self.window = Some(window);
    }

    /// A worker finished something: draw it.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        self.next = Next::Now;
        self.redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(app) = &mut self.app else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some((gfx, screen)) = &mut self.gfx {
                    screen.resize(gfx, size.width, size.height);
                }
                app.resize(size.width as f32, size.height as f32, 0.0, 0.0);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if self.mouse_down {
                    app.touch(Touch { id: 1, phase: Phase::Move, x: self.cursor.0, y: self.cursor.1, t_ns: now_ns() });
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_down = state == ElementState::Pressed;
                let phase = if self.mouse_down { Phase::Down } else { Phase::Up };
                app.touch(Touch { id: 1, phase, x: self.cursor.0, y: self.cursor.1, t_ns: now_ns() });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                app.wheel(-dy);
            }
            WindowEvent::Touch(t) => {
                let phase = match t.phase {
                    TouchPhase::Started => Phase::Down,
                    TouchPhase::Moved => Phase::Move,
                    TouchPhase::Ended => Phase::Up,
                    TouchPhase::Cancelled => Phase::Cancel,
                };
                app.touch(Touch { id: 10 + t.id, phase, x: t.location.x as f32, y: t.location.y as f32, t_ns: now_ns() });
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => match &event.logical_key {
                Key::Named(NamedKey::Enter) => app.done_typing(),
                Key::Named(NamedKey::Escape) => {
                    if !app.back() {
                        event_loop.exit();
                    }
                }
                Key::Named(NamedKey::Backspace) => app.backspace(),
                Key::Named(NamedKey::Space) => app.type_text(" "),
                _ => {
                    if let Some(text) = &event.text {
                        app.type_text(text);
                    }
                }
            },
            WindowEvent::RedrawRequested => {
                let Some((gfx, screen)) = &mut self.gfx else { return };
                self.next = app.frame(now_ns());
                if app.canvas.take_late_pictures() {
                    self.next = Next::Now;
                }
                gfx.upload_glyphs(&mut app.canvas.uploads);
                gfx.upload_images(&mut app.canvas.image_uploads);
                let (globals, clear) = (app.canvas.globals, app.clear_colour());
                if !screen.frame(gfx, &globals, clear, &app.canvas.insts, &app.canvas.batches) {
                    self.next = Next::At(now_ns() + 16_000_000);
                }
                return;
            }
            _ => return,
        }
        self.next = Next::Now;
        self.redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.next {
            Next::Now => {
                event_loop.set_control_flow(ControlFlow::Poll);
                self.redraw();
            }
            Next::OnInput => event_loop.set_control_flow(ControlFlow::Wait),
            Next::At(at) => {
                let now = now_ns();
                if at <= now {
                    self.redraw();
                    event_loop.set_control_flow(ControlFlow::Poll);
                } else {
                    let wait = std::time::Duration::from_nanos(at - now);
                    event_loop.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + wait));
                }
            }
        }
    }
}
