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
}

impl Host for DesktopHost {
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
    fn notify(&self, _id: i32, title: &str, body: &str) {
        eprintln!("relay-code: notice: {title}: {body}");
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
    let mut app = App::new(Arc::new(DesktopHost { data: dir.clone() }), clock);
    app.canvas.set_images_size(gfx.images_size());
    // Roughly a phone's status bar and gesture bar.
    app.resize(w as f32, h as f32, w as f32 * 0.085, w as f32 * 0.04);
    if screen != "first-run" {
        app.sample();
    } else {
        app.shot = true;
    }
    match screen.as_str() {
        "first-run" => app.reset_to(Route::Pair),
        "home" => app.reset_to(Route::Home),
        "drawer" | "repos" => {
            app.open("bold-puffin");
            app.st.drawer.open = true;
            app.st.drawer.repos = screen == "repos";
        }
        "chat" | "models" | "commands" | "chat-offline" => {
            app.open("bold-puffin");
            if screen == "chat-offline" {
                // A message written while the link reconnects waits for it.
                app.set_link_state(rc_link::LinkState::Offline { reason: "No route".into(), retry_at_ms: 0 });
            }
            app.st.chat.model_sheet = screen == "models";
            if screen == "commands" {
                app.focus.id = Some("chat");
                app.focus.text = "/".into();
            }
        }
        "asks" => app.open("umber-zebra"),
        "new-agent" => {
            app.reset_to(Route::Home);
            app.st.newagent.open = true;
            app.st.newagent.task = "Add a dark mode toggle to Settings".into();
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
        let mut app = App::new(Arc::new(DesktopHost { data: self.data.clone() }), now_ns());
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
