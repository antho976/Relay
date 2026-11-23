//! The Android build: GameActivity (for the keyboard), everything else in Rust. Modelled on
//! Tally RS's `android.rs`, without its launcher icons. Books come from the system's file picker:
//! `LeafActivity` copies each picked file into the app's inbox and hands its path here
//! (`nativeBookPicked`); a book's notes go out through the share sheet, words to the clipboard,
//! and PDF pages are drawn by the platform's `PdfRenderer` (`LeafActivity.pdfRender`), called from
//! the import and picture threads.
//!
//! One thread runs the app. Each turn of the loop takes pending system events, then every pending
//! touch and keyboard event, then draws a frame if anything asked for one: input, a system event,
//! a wake from a worker thread (a page decoded, a list fetched), or the app itself (`Next`).
//! Otherwise it sleeps until there is input, or until the time the app named. The system bars and
//! the keyboard are read from the window's insets when they change, and a few times a second while
//! the keyboard may be moving.
//!
//! JNI from this thread: it is attached for as long as `android_main` runs, and never returns to
//! Java, so every JNI sequence runs inside a local frame ([`with_activity`]), which frees its
//! local references. Back comes from `LeafActivity` (AndroidX would finish the activity first),
//! and the reader hides the system bars through `LeafActivity.setImmersive`.

use std::ffi::CString;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use android_activity::input::{ImeOptions, InputEvent, InputType, KeyAction, Keycode, MotionAction, TextInputAction, TextInputState, TextSpan};
use android_activity::{AndroidApp, AndroidAppWaker, InputStatus, MainEvent, PollEvent};
use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};

use crate::app::{App, Host, Next};
use crate::clock::now_ns;
use crate::gfx::{Gfx, Screen};
use crate::kit::Kb;
use crate::ui::{Phase, Touch};

struct AndroidHost {
    app: AndroidApp,
    /// When the keyboard was last asked to show or hide: its insets are watched for a while after.
    keyboard_at: AtomicU64,
    keyboard_shown: AtomicBool,
}

impl Host for AndroidHost {
    fn data_dir(&self) -> PathBuf {
        self.app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp"))
    }

    fn log(&self, text: &str) {
        log(text);
    }

    fn keyboard(&self, show: bool, text: &str, kind: Kb) {
        self.keyboard_at.store(now_ns(), Ordering::SeqCst);
        self.keyboard_shown.store(show, Ordering::SeqCst);
        if show {
            let kind = match kind {
                Kb::Text => InputType::TYPE_CLASS_TEXT,
                // A web address: no capitals, no autocorrect, no suggestions.
                Kb::Url => InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_VARIATION_URI | InputType::TYPE_TEXT_FLAG_NO_SUGGESTIONS,
                Kb::Secret => InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_VARIATION_VISIBLE_PASSWORD | InputType::TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            };
            self.app.set_ime_editor_info(kind, TextInputAction::Done, ImeOptions::empty());
            // The caret after the last character, counted in UTF-16 units as Java does.
            let end: usize = text.chars().map(char::len_utf16).sum();
            self.app.set_text_input_state(TextInputState { text: text.to_string(), selection: TextSpan { start: end, end }, compose_region: None });
            self.app.show_soft_input(true);
        } else {
            self.app.hide_soft_input(false);
        }
    }

    fn immersive(&self, on: bool) {
        with_activity(&self.app, |env, activity| env.call_method(activity, "setImmersive", "(Z)V", &[JValue::Bool(on as u8)]).map(|_| ()));
    }

    fn pick_books(&self) {
        with_activity(&self.app, |env, activity| env.call_method(activity, "pickBooks", "()V", &[]).map(|_| ()));
    }

    fn copy_text(&self, text: &str) {
        with_activity(&self.app, |env, activity| {
            let t = env.new_string(text)?;
            env.call_method(activity, "copyText", "(Ljava/lang/String;)V", &[JValue::Object(&t)]).map(|_| ())
        });
    }

    fn share_file(&self, name: &str, mime: &str, bytes: &[u8]) -> Option<String> {
        // Written under the cache's `shared` folder, which the FileProvider (AndroidManifest.xml,
        // res/xml/file_paths.xml) lends to the app the person picks.
        let cache: String = with_activity(&self.app, |env, activity| {
            let dir = env.call_method(activity, "getCacheDir", "()Ljava/io/File;", &[])?.l()?;
            let path = env.call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;
            Ok(env.get_string(&JString::from(path))?.into())
        })?;
        let dir = PathBuf::from(cache).join("shared");
        let _ = std::fs::remove_dir_all(&dir);
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " .-_".contains(c) { c } else { '-' }).collect();
        let path = dir.join(if safe.trim_matches('.').is_empty() { "notes.md".to_string() } else { safe });
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            return Some(format!("Couldn't write the file: {e}"));
        }
        let p = path.to_string_lossy().into_owned();
        let done = with_activity(&self.app, |env, activity| {
            let ps = env.new_string(&p)?;
            let m = env.new_string(mime)?;
            env.call_method(activity, "shareFile", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&ps), JValue::Object(&m)]).map(|_| ())
        });
        if done.is_none() { Some("The share sheet didn't open".into()) } else { None }
    }

    fn pdf_pages(&self, path: &std::path::Path) -> Option<u32> {
        let p = path.to_string_lossy().into_owned();
        let n = with_activity(&self.app, |env, activity| {
            let ps = env.new_string(&p)?;
            env.call_method(activity, "pdfPageCount", "(Ljava/lang/String;)I", &[JValue::Object(&ps)])?.i()
        })?;
        (n > 0).then_some(n as u32)
    }

    fn pdf_page(&self, path: &std::path::Path, n: u32, width: u32) -> Option<(u32, u32, Vec<u8>)> {
        let p = path.to_string_lossy().into_owned();
        let bytes = with_activity(&self.app, |env, activity| {
            let ps = env.new_string(&p)?;
            let arr = env.call_method(activity, "pdfRender", "(Ljava/lang/String;II)[B", &[JValue::Object(&ps), JValue::Int(n as i32), JValue::Int(width as i32)])?.l()?;
            if arr.is_null() {
                return Ok(Vec::new());
            }
            env.convert_byte_array(JByteArray::from(arr))
        })?;
        if bytes.len() < 8 {
            return None;
        }
        let w = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let h = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let px = bytes[8..].to_vec();
        (px.len() == (w * h * 4) as usize).then_some((w, h, px))
    }
}

/// Runs `f` with this thread attached to the VM and the activity at hand, inside a local frame.
/// A Java exception is cleared and logged, and makes it answer none.
fn with_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    with_activity_quiet(app, f).map_err(|e| log(&format!("jni: {e}"))).ok()
}

fn with_activity_quiet<T>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> jni::errors::Result<T> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    // A global reference that android-activity holds for the activity's life; never deleted here.
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let out = env.with_local_frame(16, |env| f(env, &activity));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    out
}

/// Wakes the app's loop from another thread. Replaced on each `android_main`.
static WAKER: Mutex<Option<AndroidAppWaker>> = Mutex::new(None);

fn wake() {
    if let Some(w) = WAKER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w.wake();
    }
}

/// A book the picker brought in, copied by `LeafActivity` into the inbox.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_quietsoftware_leaf_LeafActivity_nativeBookPicked<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>, path: JString<'local>, name: JString<'local>) {
    let path: String = if path.is_null() { return } else { env.get_string(&path).map(Into::into).unwrap_or_default() };
    let name: String = if name.is_null() { String::new() } else { env.get_string(&name).map(Into::into).unwrap_or_default() };
    if !path.is_empty() {
        crate::shelf::deliver(PathBuf::from(path), name);
    }
    wake();
}

/// Back, from LeafActivity's back callback.
static BACK: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_quietsoftware_leaf_LeafActivity_nativeBack<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    BACK.store(true, Ordering::SeqCst);
    wake();
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
}

/// A line in the system log, under the tag `leaf` (`adb logcat -s leaf`).
pub fn log(text: &str) {
    if let Ok(text) = CString::new(text.replace('\0', " ")) {
        unsafe { __android_log_write(4, c"leaf".as_ptr(), text.as_ptr()) };
    }
}

/// Ask the display for its fast mode where the panel has one (Android 11 and later).
fn prefer_high_refresh(window: NonNull<std::ffi::c_void>) {
    type SetFrameRate = unsafe extern "C" fn(*mut std::ffi::c_void, f32, i8) -> i32;
    unsafe {
        let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"ANativeWindow_setFrameRate".as_ptr());
        if !symbol.is_null() {
            let set: SetFrameRate = std::mem::transmute(symbol);
            set(window.as_ptr(), 120.0, 0);
        }
    }
}

/// How this Android reports the window's insets, decided once.
enum InsetsApi {
    /// Android 11 and later: `WindowInsets.getInsets(Type)`.
    Types { bars: i32, cutout: i32, ime: i32 },
    /// Android 8 to 10: the system window insets (with the keyboard) beside the stable ones.
    Legacy { cutout: bool },
}

impl InsetsApi {
    fn new(app: &AndroidApp) -> InsetsApi {
        let sdk = AndroidApp::sdk_version();
        let legacy = InsetsApi::Legacy { cutout: sdk >= 28 };
        if sdk < 30 {
            return legacy;
        }
        let types = with_activity(app, |env, _| {
            let mut mask = |kind: &str| env.call_static_method("android/view/WindowInsets$Type", kind, "()I", &[])?.i();
            Ok(InsetsApi::Types { bars: mask("systemBars")?, cutout: mask("displayCutout")?, ime: mask("ime")? })
        });
        types.unwrap_or(legacy)
    }

    /// The system bars' and the keyboard's insets, in pixels: (top, bottom bars, keyboard).
    fn read(&self, app: &AndroidApp) -> Option<(f32, f32, f32)> {
        let read = with_activity_quiet(app, |env, activity| -> jni::errors::Result<Option<(f32, f32, f32)>> {
            let window = env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
            let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
            let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
            if insets.is_null() {
                return Ok(None);
            }
            match *self {
                InsetsApi::Types { bars, cutout, ime } => {
                    let mut read = |mask: i32| -> jni::errors::Result<(i32, i32)> {
                        let i = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(mask)])?.l()?;
                        Ok((env.get_field(&i, "top", "I")?.i()?, env.get_field(&i, "bottom", "I")?.i()?))
                    };
                    let (top, bottom) = read(bars)?;
                    let (cut_top, _) = read(cutout)?;
                    let (_, ime) = read(ime)?;
                    Ok(Some((top.max(cut_top) as f32, bottom as f32, ime as f32)))
                }
                InsetsApi::Legacy { cutout } => {
                    let mut int = |name: &str| -> jni::errors::Result<i32> { env.call_method(&insets, name, "()I", &[])?.i() };
                    let top = int("getSystemWindowInsetTop")?;
                    let bars = int("getStableInsetBottom")?;
                    let with_keyboard = int("getSystemWindowInsetBottom")?;
                    let mut cut_top = 0;
                    if cutout {
                        let c = env.call_method(&insets, "getDisplayCutout", "()Landroid/view/DisplayCutout;", &[])?.l()?;
                        if !c.is_null() {
                            cut_top = env.call_method(&c, "getSafeInsetTop", "()I", &[])?.i()?;
                        }
                    }
                    Ok(Some((top.max(cut_top) as f32, bars as f32, with_keyboard.max(bars) as f32)))
                }
            }
        });
        read.ok().flatten()
    }
}

/// Send the task to the back, as Back does on an app's first screen.
fn leave(app: &AndroidApp) {
    with_activity(app, |env, activity| env.call_method(activity, "moveTaskToBack", "(Z)Z", &[JValue::Bool(1)]).map(|_| ()));
}

const KEYBOARD_POLL: Duration = Duration::from_millis(120);
const KEYBOARD_SETTLE_NS: u64 = 1_500_000_000;
const RETRY_NS: u64 = 16_000_000;

#[unsafe(no_mangle)]
fn android_main(android: AndroidApp) {
    std::panic::set_hook(Box::new(|info| log(&format!("panic: {info}"))));
    *WAKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(android.create_waker());
    crate::wake::set(Box::new(wake));
    let host = Arc::new(AndroidHost { app: android.clone(), keyboard_at: AtomicU64::new(0), keyboard_shown: AtomicBool::new(false) });
    let mut app = App::new(host.clone(), now_ns());
    let mut gfx: Option<Gfx> = None;
    let mut screen: Option<Screen<'static>> = None;
    let mut window = None;
    let mut next = Next::Now;
    let mut dirty = true;
    let multi_resume = AndroidApp::sdk_version() >= 29;
    let mut foreground = false;
    let mut quit = false;
    let mut last_insets = (0.0, 0.0, 0.0);
    let mut insets_api: Option<InsetsApi> = None;
    let mut watch_until = 0u64;

    while !quit {
        let drawing = screen.is_some() && foreground;
        let now = now_ns();
        if app.keyboard_up {
            watch_until = now + KEYBOARD_SETTLE_NS;
        }
        watch_until = watch_until.max(host.keyboard_at.load(Ordering::SeqCst).saturating_add(KEYBOARD_SETTLE_NS));
        let timeout = if !drawing {
            None
        } else if dirty || matches!(next, Next::Now) {
            Some(Duration::ZERO)
        } else {
            let due = match next {
                Next::At(at) => Some(Duration::from_nanos(at.saturating_sub(now))),
                _ => None,
            };
            let watch = (now < watch_until).then_some(KEYBOARD_POLL);
            match (due, watch) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        };
        let mut resized = false;
        android.poll_events(timeout, |event| match event {
            PollEvent::Wake => dirty = true,
            PollEvent::Main(event) => match event {
                MainEvent::InitWindow { .. } => {
                    let Some(native) = android.native_window() else { return };
                    let (w, h) = (native.width().max(1) as u32, native.height().max(1) as u32);
                    let ptr = native.ptr().cast::<std::ffi::c_void>();
                    prefer_high_refresh(ptr);
                    let display = RawDisplayHandle::Android(AndroidDisplayHandle::new());
                    let handle = RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(ptr));
                    let made = match &gfx {
                        Some(g) => unsafe { g.raw_screen(display, handle, w, h) },
                        None => unsafe { Gfx::with_raw(display, handle, w, h) }.map(|(g, s)| {
                            gfx = Some(g);
                            s
                        }),
                    };
                    match made {
                        Ok(s) => {
                            if let Some(g) = &gfx {
                                log(&format!("drawing {w}x{h} with {}", g.adapter_name()));
                                app.canvas.set_images_size(g.images_size());
                            }
                            screen = Some(s);
                            window = Some(native);
                            resized = true;
                        }
                        Err(e) => log(&format!("no screen: {e}")),
                    }
                }
                MainEvent::TerminateWindow { .. } => {
                    screen = None;
                    window = None;
                }
                MainEvent::WindowResized { .. } | MainEvent::ContentRectChanged { .. } | MainEvent::InsetsChanged { .. } | MainEvent::ConfigChanged { .. } => resized = true,
                MainEvent::RedrawNeeded { .. } => dirty = true,
                MainEvent::Start => {
                    if !multi_resume {
                        foreground = true;
                        dirty = true;
                    }
                }
                MainEvent::Stop => {
                    if !multi_resume {
                        foreground = false;
                    }
                }
                MainEvent::Resume { .. } => {
                    foreground = true;
                    dirty = true;
                    // Back to the front: the open page reads the server again.
                    app.fresh = true;
                }
                MainEvent::GainedFocus => dirty = true,
                MainEvent::Pause => {
                    app.paused();
                    if multi_resume {
                        foreground = false;
                    }
                }
                MainEvent::Destroy => quit = true,
                _ => {}
            },
            _ => {}
        });

        let watching = now_ns() < watch_until;
        if (resized || watching)
            && let (Some(g), Some(s), Some(native)) = (&gfx, &mut screen, &window)
        {
            let (w, h) = (native.width().max(1) as u32, native.height().max(1) as u32);
            let api = insets_api.get_or_insert_with(|| InsetsApi::new(&android));
            let found = api.read(&android).unwrap_or_else(|| {
                let content = android.content_rect();
                (content.top.max(0) as f32, (h as i32 - content.bottom).max(0) as f32, 0.0)
            });
            if resized || found != last_insets {
                s.resize(g, w, h);
                last_insets = found;
                app.resize(w as f32, h as f32, found.0, found.1.max(found.2));
                app.keyboard_up = found.2 > found.1 + 1.0;
                dirty = true;
            }
        }

        let mut go_back = BACK.swap(false, Ordering::SeqCst);
        let mut text: Option<String> = None;
        let mut done_typing = false;
        if let Ok(mut events) = android.input_events_iter() {
            loop {
                let more = events.next(|event| match event {
                    InputEvent::MotionEvent(motion) => {
                        let t_ns = motion.event_time().max(0) as u64;
                        let mut send = |index: usize, phase: Phase| {
                            let p = motion.pointer_at_index(index);
                            app.touch(Touch { id: p.pointer_id() as u64, phase, x: p.x(), y: p.y(), t_ns });
                        };
                        match motion.action() {
                            MotionAction::Down | MotionAction::PointerDown => send(motion.pointer_index(), Phase::Down),
                            MotionAction::Up | MotionAction::PointerUp => send(motion.pointer_index(), Phase::Up),
                            MotionAction::Move => (0..motion.pointer_count()).for_each(|i| send(i, Phase::Move)),
                            MotionAction::Cancel => (0..motion.pointer_count()).for_each(|i| send(i, Phase::Cancel)),
                            _ => {}
                        }
                        InputStatus::Handled
                    }
                    InputEvent::KeyEvent(key) if key.key_code() == Keycode::Back => {
                        if key.action() == KeyAction::Up {
                            go_back = true;
                        }
                        InputStatus::Handled
                    }
                    InputEvent::TextEvent(state) => {
                        text = Some(state.text.clone());
                        InputStatus::Handled
                    }
                    InputEvent::TextAction(_) => {
                        done_typing = true;
                        InputStatus::Handled
                    }
                    _ => InputStatus::Unhandled,
                });
                if !more {
                    break;
                }
                dirty = true;
            }
        }
        if let Some(t) = text {
            app.set_text(&t);
            dirty = true;
        }
        if done_typing || (go_back && app.focus.id.is_some()) {
            app.done_typing();
            go_back = false;
            dirty = true;
        }
        if go_back {
            dirty = true;
            if !app.back() {
                leave(&android);
            }
        }

        let due = dirty || matches!(next, Next::Now) || matches!(next, Next::At(at) if now_ns() >= at);
        if foreground
            && due
            && let (Some(g), Some(s)) = (&mut gfx, &mut screen)
        {
            dirty = false;
            next = app.frame(now_ns());
            if app.canvas.take_late_pictures() {
                next = Next::Now;
            }
            g.upload_glyphs(&mut app.canvas.uploads);
            g.upload_images(&mut app.canvas.image_uploads);
            let (globals, clear) = (app.canvas.globals, app.clear_colour());
            if !s.frame(g, &globals, clear, &app.canvas.insts, &app.canvas.batches) {
                next = Next::At(now_ns() + RETRY_NS);
            }
        }
    }
}
