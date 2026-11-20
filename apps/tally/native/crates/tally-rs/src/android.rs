//! The Android build: GameActivity (for the keyboard), everything else in Rust.
//!
//! One thread runs the app. Each turn of the loop takes pending system events, then every pending
//! touch and keyboard event, then draws a frame. When nothing is moving it sleeps until there is
//! input. The system bars and the keyboard are read from the window's insets.
//!
//! What only Android can do for the app goes through JNI: the browser (an `ACTION_VIEW` intent),
//! the file picker and the share sheet. The last two need Java callbacks (an activity result, and
//! a `FileProvider` class that the system class loader of this thread cannot see), so they are
//! methods of `TallyActivity` (native/android/src/main/java), a small subclass of GameActivity; a
//! picked file comes back through `nativeFilePicked` below.

use std::ffi::CString;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;

use android_activity::input::{InputEvent, InputType, ImeOptions, KeyAction, Keycode, MotionAction, TextInputAction, TextInputState, TextSpan};
use android_activity::{AndroidApp, AndroidAppWaker, InputStatus, MainEvent, PollEvent};
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::JNIEnv;
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};

use crate::app::{App, Host, Next};
use crate::clock::now_ns;
use crate::gfx::{Gfx, Screen};
use crate::ui::{Phase, Touch};

struct AndroidHost {
    app: AndroidApp,
}

/// The phone is set to dark (Settings, Display, Dark theme).
fn system_dark(app: &AndroidApp) -> bool {
    app.config().ui_mode_night() == ndk::configuration::UiModeNight::Yes
}

impl Host for AndroidHost {
    fn system_dark(&self) -> bool {
        system_dark(&self.app)
    }

    fn data_dir(&self) -> PathBuf {
        self.app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp"))
    }

    fn log(&self, text: &str) {
        log(text);
    }

    fn keyboard(&self, show: bool, text: &str, numeric: bool) {
        if show {
            let kind = if numeric {
                InputType::TYPE_CLASS_NUMBER | InputType::TYPE_NUMBER_FLAG_DECIMAL
            } else {
                InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_CAP_SENTENCES
            };
            self.app.set_ime_editor_info(
                kind,
                TextInputAction::Done,
                ImeOptions::empty(),
            );
            let end = text.len();
            self.app.set_text_input_state(TextInputState { text: text.to_string(), selection: TextSpan { start: end, end }, compose_region: None });
            self.app.show_soft_input(true);
        } else {
            self.app.hide_soft_input(false);
        }
    }

    fn open_url(&self, url: &str) {
        // Only web pages: a link from the server never opens anything else on the phone.
        if !url.starts_with("https://") {
            log(&format!("not opening {url}: not a web page"));
            return;
        }
        let done = with_activity(&self.app, |env, activity| {
            let text = env.new_string(url)?;
            let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&text)])?.l()?;
            let action = env.new_string("android.intent.action.VIEW")?;
            let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action), JValue::Object(&uri)])?;
            // FLAG_ACTIVITY_NEW_TASK: the browser is its own task, so Back from it returns here.
            env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x1000_0000)])?;
            env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
            Ok(())
        });
        if done.is_none() {
            log(&format!("no browser opened {url}"));
        }
    }

    fn pick_file(&self, mime: &str) {
        let done = with_activity(&self.app, |env, activity| {
            let m = env.new_string(mime)?;
            env.call_method(activity, "pickFile", "(Ljava/lang/String;)V", &[JValue::Object(&m)])?;
            Ok(())
        });
        if done.is_none() {
            crate::settings_files::deliver(String::new(), Vec::new());
        }
    }

    fn share_file(&self, name: &str, mime: &str, bytes: &[u8]) {
        // Written to the cache's `shared` folder, which the FileProvider (AndroidManifest.xml,
        // res/xml/file_paths.xml) lends to the app the person picks.
        let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_".contains(c) { c } else { '-' }).collect();
        let Some(cache): Option<String> = with_activity(&self.app, |env, activity| {
            let dir = env.call_method(activity, "getCacheDir", "()Ljava/io/File;", &[])?.l()?;
            let path = env.call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;
            Ok(env.get_string(&JString::from(path))?.into())
        }) else {
            log("share: no cache folder");
            return;
        };
        let dir = PathBuf::from(cache).join("shared");
        let path = dir.join(if safe.is_empty() { "tally-file".to_string() } else { safe });
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            log(&format!("share {}: {e}", path.display()));
            return;
        }
        let path = path.to_string_lossy().into_owned();
        let done = with_activity(&self.app, |env, activity| {
            let p = env.new_string(&path)?;
            let m = env.new_string(mime)?;
            env.call_method(activity, "shareFile", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&p), JValue::Object(&m)])?;
            Ok(())
        });
        if done.is_none() {
            log(&format!("share {path}: the share sheet did not open"));
        }
    }
}

/// Runs `f` with this thread attached to the VM and the activity at hand. A Java exception is
/// cleared and logged, and makes it answer none.
fn with_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let out = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    match out {
        Ok(v) => Some(v),
        Err(e) => {
            log(&format!("jni: {e}"));
            None
        }
    }
}

/// Wakes the app's loop from another thread: a picked file arrived.
static WAKER: std::sync::OnceLock<AndroidAppWaker> = std::sync::OnceLock::new();

/// `TallyActivity.nativeFilePicked`: the file the person picked, read by the activity on a thread
/// of its own (an empty array when it could not be read).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tally_rs_TallyActivity_nativeFilePicked<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>, name: JString<'local>, bytes: JByteArray<'local>) {
    let name: String = if name.is_null() { String::new() } else { env.get_string(&name).map(Into::into).unwrap_or_default() };
    let bytes = if bytes.is_null() { Vec::new() } else { env.convert_byte_array(&bytes).unwrap_or_default() };
    let bytes = if bytes.len() > crate::settings_files::MAX_PICK_BYTES { Vec::new() } else { bytes };
    crate::settings_files::deliver(name, bytes);
    if let Some(w) = WAKER.get() {
        w.wake();
    }
}

/// Back, from TallyActivity's back callback (AndroidX would otherwise finish the activity).
static BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tally_rs_TallyActivity_nativeBack<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    BACK.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(w) = WAKER.get() {
        w.wake();
    }
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
}

/// A line in the system log, under the tag `tally` (`adb logcat -s tally`).
pub fn log(text: &str) {
    if let Ok(text) = CString::new(text.replace('\0', " ")) {
        unsafe { __android_log_write(4, c"tally".as_ptr(), text.as_ptr()) };
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

/// The system bars' and the keyboard's insets, in pixels: (top, bottom bars, keyboard).
fn insets(app: &AndroidApp) -> Option<(f32, f32, f32)> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let result = (|| -> jni::errors::Result<Option<(f32, f32, f32)>> {
        let window = env.call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        if insets.is_null() {
            return Ok(None);
        }
        let mut read = |kind: &str| -> jni::errors::Result<(i32, i32)> {
            let t = env.call_static_method("android/view/WindowInsets$Type", kind, "()I", &[])?.i()?;
            let i = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(t)])?.l()?;
            Ok((env.get_field(&i, "top", "I")?.i()?, env.get_field(&i, "bottom", "I")?.i()?))
        };
        let (top, bottom) = read("systemBars")?;
        let (cut_top, _) = read("displayCutout")?;
        let (_, ime) = read("ime")?;
        Ok(Some((top.max(cut_top) as f32, bottom as f32, ime as f32)))
    })();
    let _ = env.exception_clear();
    result.ok().flatten()
}

/// Send the task to the back, as the back gesture does on an app's first screen.
fn leave(app: &AndroidApp) {
    let run = || -> Result<(), jni::errors::Error> {
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
        let mut env = vm.attach_current_thread()?;
        let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr().cast()) };
        env.call_method(&activity, "moveTaskToBack", "(Z)Z", &[jni::objects::JValue::Bool(1)])?;
        Ok(())
    };
    let _ = run();
}

#[unsafe(no_mangle)]
fn android_main(android: AndroidApp) {
    std::panic::set_hook(Box::new(|info| log(&format!("panic: {info}"))));
    let _ = WAKER.set(android.create_waker());
    crate::pictures::set_waker(Box::new(|| {
        if let Some(w) = WAKER.get() {
            w.wake();
        }
    }));
    let host = Arc::new(AndroidHost { app: android.clone() });
    let mut app = App::new(host, now_ns());
    let mut gfx: Option<Gfx> = None;
    let mut screen: Option<Screen<'static>> = None;
    // Kept alive for as long as `screen` draws to it.
    let mut window = None;
    let mut next = Next::Now;
    let mut foreground = false;
    let mut quit = false;
    let mut last_insets = (0.0, 0.0, 0.0);
    // The bars' icons and the launcher icon follow the palette: dark icons on Light, light on Dark.
    let mut bars_light: Option<bool> = None;

    while !quit {
        let drawing = screen.is_some() && foreground;
        // While the keyboard may be moving, look at the insets again a few times a second.
        let timeout = match (drawing, next) {
            (true, Next::Now) => Some(Duration::ZERO),
            (true, _) if app.focus.id.is_some() => Some(Duration::from_millis(120)),
            // A sync in the background, or the next one due: wake for it without input.
            (true, Next::At(at)) => Some(Duration::from_nanos(at.saturating_sub(now_ns()))),
            _ => None,
        };
        let mut resized = false;
        android.poll_events(timeout, |event| {
            let PollEvent::Main(event) = event else { return };
            match event {
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
                MainEvent::WindowResized { .. } | MainEvent::ContentRectChanged { .. } | MainEvent::ConfigChanged { .. } | MainEvent::InsetsChanged { .. } => {
                    resized = true;
                }
                MainEvent::Resume { .. } | MainEvent::GainedFocus => {
                    foreground = true;
                    next = Next::Now;
                    app.invalidate();
                }
                MainEvent::Pause | MainEvent::LostFocus => foreground = false,
                MainEvent::Destroy => quit = true,
                _ => {}
            }
        });

        if resized || foreground {
            // Appearance "System" follows the phone's dark theme as it changes.
            app.set_system_dark(system_dark(&android));
        }
        if let (Some(g), Some(s), Some(native)) = (&gfx, &mut screen, &window) {
            let (w, h) = (native.width().max(1) as u32, native.height().max(1) as u32);
            let found = insets(&android).unwrap_or_else(|| {
                let content = android.content_rect();
                (content.top.max(0) as f32, (h as i32 - content.bottom).max(0) as f32, 0.0)
            });
            if resized || found != last_insets {
                s.resize(g, w, h);
                last_insets = found;
                // The keyboard covers the bottom: the app lays out above it.
                app.resize(w as f32, h as f32, found.0, found.1.max(found.2));
                app.keyboard_up = found.2 > found.1 + 1.0;
                next = Next::Now;
            }
        }

        // A file the person picked (the picker answered while the loop slept).
        if let Some((name, bytes)) = crate::settings_files::take_delivered() {
            app.file_picked(name, bytes);
            next = Next::Now;
        }

        let mut go_back = BACK.swap(false, std::sync::atomic::Ordering::SeqCst);
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
                next = Next::Now;
            }
        }
        if let Some(t) = text {
            app.set_text(&t);
            next = Next::Now;
        }
        if done_typing || (go_back && app.focus.id.is_some()) {
            app.finish_typing();
            go_back = false;
            next = Next::Now;
        }
        if go_back && !app.back() {
            leave(&android);
        }

        if foreground && bars_light != Some(app.palette.light) {
            let light = app.palette.light;
            with_activity(&android, |env, activity| {
                env.call_method(activity, "setLightBars", "(Z)V", &[jni::objects::JValue::Bool(light as u8)])?;
                // The home screen icon follows too (icon E, dark or light).
                env.call_method(activity, "setIconDark", "(Z)V", &[jni::objects::JValue::Bool(!light as u8)]).map(|_| ())
            });
            bars_light = Some(light);
        }

        if foreground && let (Some(g), Some(s)) = (&mut gfx, &mut screen) {
            next = app.frame(now_ns());
            g.upload_glyphs(&mut app.canvas.uploads);
            g.upload_images(&mut app.canvas.image_uploads);
            let (globals, clear) = (app.canvas.globals, app.clear_colour());
            s.frame(g, &globals, clear, &app.canvas.insts, &app.canvas.batches);
        }
    }
}
