//! The desktop build: a phone-shaped window for working on Tally with the mouse and keyboard, and a
//! screenshot mode that needs no window.
//!
//! - `--shot out.png [--screen NAME] [--id N] [--category N] [--account N] [--kind K] [--size WxH]
//!   [--scroll UNITS] [--over 1] [--menu 1] [--palette night]` draws one frame
//!   to a file, on a ledger in memory (the sample one, unless the screen is `empty`).
//! - `--data DIR` keeps the ledger somewhere else than `~/.local/share/tally-rs`.
//!
//! Keys: digits and the point type an amount, Backspace deletes, Escape goes back.

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::app::{App, Host, Next};
use crate::nav::Route;
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
        eprintln!("tally: {text}");
    }

    /// The desktop's browser, through xdg-open. Only web pages, as on the phone.
    fn open_url(&self, url: &str) {
        if !url.starts_with("https://") {
            self.log(&format!("not opening {url}: not a web page"));
            return;
        }
        match std::process::Command::new("xdg-open").arg(url).spawn() {
            // Reaped on a thread of its own, so the window never waits on the browser.
            Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
            Err(e) => self.log(&format!("xdg-open {url}: {e}")),
        }
    }

    /// For trying the import and the restore by hand: the file named by `TALLY_PICK` is the one
    /// "picked" (there is no file dialog on the desktop). It reaches the page as Android's does.
    fn pick_file(&self, mime: &str) {
        let Some(path) = std::env::var_os("TALLY_PICK").map(PathBuf::from) else {
            self.log(&format!("pick a file ({mime}): set TALLY_PICK to its path"));
            return;
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = match std::fs::read(&path) {
            Ok(b) if b.len() <= crate::settings_files::MAX_PICK_BYTES => b,
            Ok(_) => Vec::new(),
            Err(e) => {
                self.log(&format!("{}: {e}", path.display()));
                Vec::new()
            }
        };
        crate::settings_files::deliver(name, bytes);
    }

    /// No share sheet on the desktop: the file goes into the data folder and the log says where.
    fn share_file(&self, name: &str, _mime: &str, bytes: &[u8]) {
        let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_".contains(c) { c } else { '-' }).collect();
        let dir = self.data.join("shared");
        let path = dir.join(if safe.is_empty() { "tally-file".to_string() } else { safe });
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.log(&format!("shared {}", path.display())),
            Err(e) => self.log(&format!("share {}: {e}", path.display())),
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
        base.join("tally-rs")
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
    let event_loop = EventLoop::new().expect("an event loop");
    let mut window = Windowed { app: None, gfx: None, window: None, next: Next::Now, cursor: (0.0, 0.0), mouse_down: false, data: data_dir(&args) };
    event_loop.run_app(&mut window).expect("the event loop ran");
}

/// One frame of a screen, drawn with no window, on a ledger of its own in a temporary folder.
fn shot(args: &[String], out: &str) -> Result<(), String> {
    let (w, h) = arg(args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))))
        .unwrap_or((1080, 2340));
    let screen = arg(args, "--screen").unwrap_or_else(|| "overview".into());
    let dir = std::env::temp_dir().join(format!("tally-rs-shot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut gfx = Gfx::headless()?;
    let mut clock = 1_000_000_000u64;
    let mut app = App::new(Arc::new(DesktopHost { data: dir.clone() }), clock);
    // Roughly a phone's status bar and gesture bar.
    app.resize(w as f32, h as f32, w as f32 * 0.085, w as f32 * 0.04);
    if screen != "empty" {
        app.store.load_sample(crate::model::today()).map_err(|e| format!("sample: {e}"))?;
        app.invalidate();
    }
    app.push(Route::Overview);
    let num = |name: &str| arg(args, name).and_then(|v| v.parse::<i64>().ok());
    let (id, category, account) = (num("--id"), num("--category"), num("--account"));
    let kind = arg(args, "--kind");
    let route = match screen.as_str() {
        "overview" | "empty" => None,
        "entry" | "entry-typed" => Some(Route::Entry { id, kind: None, account, category }),
        "entries" => Some(Route::Entries),
        "transactions" => Some(Route::Transactions { category, account }),
        "entry-view" => Some(Route::EntryView { id: id.unwrap_or(1) }),
        "insights" => Some(Route::Insights),
        "search" => Some(Route::Search),
        "budgets" => Some(Route::Budgets),
        "budget-edit" => Some(Route::BudgetEdit { category }),
        "bills" => Some(Route::Bills),
        "bill-edit" => Some(Route::BillEdit { id }),
        "goals" => Some(Route::Goals),
        "goal" => Some(Route::Goal { id: id.unwrap_or(1) }),
        "goal-edit" => Some(Route::GoalEdit { id }),
        "accounts" => Some(Route::Accounts),
        "account-edit" => Some(Route::AccountEdit { id }),
        "investments" => Some(Route::Investments),
        "room-edit" => Some(Route::RoomEdit { kind: kind.clone().unwrap_or_else(|| "TFSA".into()) }),
        "categories" => Some(Route::Categories),
        "category-edit" => Some(Route::CategoryEdit {
            id,
            kind: if kind.as_deref() == Some("income") { relay_money::model::CategoryKind::Income } else { relay_money::model::CategoryKind::Expense },
        }),
        "settings" => Some(Route::Settings),
        "sync" => Some(Route::Sync),
        "appearance" => Some(Route::Appearance),
        "format" => Some(Route::Format),
        "backup" => Some(Route::Backup),
        "about" => Some(Route::About),
        "import" => Some(Route::Import),
        "banks" => Some(Route::Banks),
        "onboarding" => Some(Route::Onboarding),
        other => return Err(format!("unknown screen: {other}")),
    };
    // A page the drawer opens is shot as the drawer opens it (its key); `--over 1` shoots it
    // opened from Overview instead (Back).
    if let Some(r) = route {
        if arg(args, "--over").is_some() {
            app.push_over(r);
        } else if r.is_top_level() {
            app.open_from_drawer(r);
        } else {
            app.push(r);
        }
    }
    if screen == "entry-typed" {
        app.type_text("12.5");
    }
    if let Some(name) = arg(args, "--palette") {
        app.set_appearance(&name);
    }
    if arg(args, "--menu").is_some() {
        app.drawer_open = true;
        app.drawer = 1.0;
    }
    app.frame(clock);
    if let Some(units) = arg(args, "--scroll").and_then(|s| s.parse::<f32>().ok()) {
        app.scroll().off = units;
    }
    // Frames until the pictures the page asked for have been decoded (a second at most).
    for i in 0..60 {
        clock += 16_666_666;
        app.frame(clock);
        gfx.upload_glyphs(&mut app.canvas.uploads);
        gfx.upload_images(&mut app.canvas.image_uploads);
        if i >= 3 && !crate::pictures::pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
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

impl ApplicationHandler for Windowed {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Tally").with_inner_size(LogicalSize::new(412.0, 900.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("a window"));
        let size = window.inner_size();
        let (gfx, screen) = Gfx::with_surface(window.clone(), size.width, size.height).expect("graphics");
        let mut app = App::new(Arc::new(DesktopHost { data: self.data.clone() }), now_ns());
        app.resize(size.width as f32, size.height as f32, 0.0, 0.0);
        self.app = Some(app);
        self.gfx = Some((gfx, screen));
        self.window = Some(window);
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
                let u = app.canvas.u.max(0.01);
                let s = app.scroll();
                s.off = (s.off - dy / u).clamp(0.0, s.max.max(0.0));
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
                Key::Named(NamedKey::Enter) => app.finish_typing(),
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
                // A picture found room in the atlas after the frame was drawn: show it.
                if app.canvas.take_late_pictures() {
                    self.next = Next::Now;
                }
                gfx.upload_glyphs(&mut app.canvas.uploads);
                gfx.upload_images(&mut app.canvas.image_uploads);
                let (globals, clear) = (app.canvas.globals, app.clear_colour());
                screen.frame(gfx, &globals, clear, &app.canvas.insts, &app.canvas.batches);
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
