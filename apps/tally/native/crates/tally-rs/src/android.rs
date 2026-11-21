//! The Android build: GameActivity (for the keyboard), everything else in Rust.
//!
//! One thread runs the app. Each turn of the loop takes pending system events, then every pending
//! touch and keyboard event, then draws a frame if anything asked for one: input, a system event,
//! a wake from another thread, or the app itself (`Next`). Otherwise it sleeps until there is
//! input, or until the time the app named. The system bars and the keyboard are read from the
//! window's insets when they change, and a few times a second while the keyboard may be moving.
//!
//! JNI from this thread: it is attached for as long as `android_main` runs, and never returns to
//! Java, so local references would pile up until the VM aborts. Every JNI sequence runs inside a
//! local frame ([`with_activity`]), which frees them.
//!
//! What only Android can do for the app goes through JNI: the browser (an `ACTION_VIEW` intent),
//! the file picker and the share sheet. The last two need Java callbacks (an activity result, and
//! a `FileProvider` class that the system class loader of this thread cannot see), so they are
//! methods of `TallyActivity` (native/android/src/main/java), a small subclass of GameActivity; a
//! picked file comes back through `nativeFilePicked` below.

use std::ffi::CString;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use android_activity::input::{InputEvent, InputType, ImeOptions, KeyAction, Keycode, MotionAction, TextInputAction, TextInputState, TextSpan};
use android_activity::{AndroidApp, AndroidAppWaker, InputStatus, MainEvent, PollEvent};
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::JNIEnv;
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};

use crate::app::{App, Host, Next};
use crate::clock::now_ns;
use crate::gfx::{Gfx, Screen};
use crate::nav::Route;
use crate::ui::{Phase, Touch};
use crate::widgets::Kb;

struct AndroidHost {
    app: AndroidApp,
    /// This loop asked for a file (`pick_file`). Not the static [`PICKING`]: an activity made
    /// again in the same process (with the picker still out) starts a new loop, whose pages did
    /// not ask, while PICKING still holds the old loop's pick.
    asked: AtomicBool,
    /// When the keyboard was last asked to show or hide ([`now_ns`]): its insets are watched for
    /// a while after, as it slides.
    keyboard_at: AtomicU64,
    /// The keyboard was asked to show and not to hide since. False for a field restored typing
    /// (its keyboard stays down until it is tapped).
    keyboard_shown: AtomicBool,
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

    fn keyboard_kind(&self, show: bool, text: &str, kind: Kb) {
        self.keyboard_at.store(now_ns(), Ordering::SeqCst);
        self.keyboard_shown.store(show, Ordering::SeqCst);
        if show {
            let kind = match kind {
                Kb::Number => InputType::TYPE_CLASS_NUMBER | InputType::TYPE_NUMBER_FLAG_DECIMAL,
                // A minus too, for the few amounts that can be below zero.
                Kb::Signed => InputType::TYPE_CLASS_NUMBER | InputType::TYPE_NUMBER_FLAG_DECIMAL | InputType::TYPE_NUMBER_FLAG_SIGNED,
                Kb::Text => InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_CAP_SENTENCES,
                // A web address: no capitals, no autocorrect, no suggestions.
                Kb::Url => InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_VARIATION_URI | InputType::TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            };
            self.app.set_ime_editor_info(
                kind,
                TextInputAction::Done,
                ImeOptions::empty(),
            );
            // The caret after the last character. GameTextInput hands the span to Java's
            // `setSelection`, which counts UTF-16 units, not UTF-8 bytes ("Café", €, emoji).
            // (Spans coming back in `TextEvent`s count the same way; the app takes only the text.)
            let end = utf16_len(text);
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
        // A file an earlier pick left (its page gone) is not for this one.
        let pick = crate::settings_files::begin_pick();
        self.asked.store(true, Ordering::SeqCst);
        // Kept in the saved state while the picker is out, so a file that comes back to a new
        // process (the system stopped this one meanwhile) still finds its page.
        *PICKING.lock().unwrap_or_else(|e| e.into_inner()) = Some(mime.to_string());
        let done = with_activity(&self.app, |env, activity| {
            let m = env.new_string(mime)?;
            // The number goes out with the picker and comes back with its answer.
            env.call_method(activity, "pickFile", "(Ljava/lang/String;J)V", &[JValue::Object(&m), JValue::Long(pick as i64)])?;
            Ok(())
        });
        if done.is_none() {
            crate::settings_files::deliver_for(pick, String::new(), Vec::new());
        }
    }

    fn share_file(&self, name: &str, mime: &str, bytes: &[u8]) {
        // Written under the cache's `shared` folder, which the FileProvider (AndroidManifest.xml,
        // res/xml/file_paths.xml) lends, with the folders in it, to the app the person picks.
        let Some(cache): Option<String> = with_activity(&self.app, |env, activity| {
            let dir = env.call_method(activity, "getCacheDir", "()Ljava/io/File;", &[])?.l()?;
            let path = env.call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;
            Ok(env.get_string(&JString::from(path))?.into())
        }) else {
            log("share: no cache folder");
            return;
        };
        let path = match crate::settings_files::write_share(&PathBuf::from(cache).join("shared"), name, bytes) {
            Ok(path) => path,
            Err(e) => {
                log(&format!("share {name}: {e}"));
                return;
            }
        };
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

/// Length of `text` in UTF-16 units, the way Java's strings (and the keyboard's spans) count.
fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// Runs `f` with this thread attached to the VM and the activity at hand, inside a local frame:
/// every local reference `f` makes is freed when it returns (this thread never returns to Java,
/// which would otherwise free them), so `f` must not return one. A Java exception is cleared and
/// logged, and makes it answer none.
fn with_activity<T>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    with_activity_quiet(app, f).map_err(|e| log(&format!("jni: {e}"))).ok()
}

/// As [`with_activity`], answering the error rather than logging it.
fn with_activity_quiet<T>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> jni::errors::Result<T> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
    // The loop's thread is attached already: this only borrows the attachment (and never
    // detaches it).
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

/// Wakes the app's loop from another thread: a picked file arrived, Back, a decoded picture.
/// Replaced on each `android_main`: Android starts the loop again, with a new looper, for each new
/// activity, and a waker for the old one would wake nothing.
static WAKER: Mutex<Option<AndroidAppWaker>> = Mutex::new(None);

fn wake() {
    if let Some(w) = WAKER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w.wake();
    }
}

/// `TallyActivity.nativeFilePicked`: the file the person picked, read by the activity on a thread
/// of its own (an empty array when it could not be read), for the pick `tag` names. One that
/// answers an earlier pick (a slow read from a cloud drive, its page gone) is dropped.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tally_rs_TallyActivity_nativeFilePicked<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>, tag: jni::sys::jlong, name: JString<'local>, bytes: JByteArray<'local>) {
    let Some(pick) = crate::settings_files::pick_from_tag(tag as u64) else { return };
    let name: String = if name.is_null() { String::new() } else { env.get_string(&name).map(Into::into).unwrap_or_default() };
    let bytes = if bytes.is_null() { Vec::new() } else { env.convert_byte_array(&bytes).unwrap_or_default() };
    let bytes = if bytes.len() > crate::settings_files::MAX_PICK_BYTES { Vec::new() } else { bytes };
    crate::settings_files::deliver_for(pick, name, bytes);
    wake();
}

/// The kind of file the picker is out for (its mime), from `pick_file` until a file comes back
/// or the picker is cancelled.
static PICKING: Mutex<Option<String>> = Mutex::new(None);

/// The picker came back with no file (`TallyActivity.nativePickCancelled`).
static PICK_CANCELLED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tally_rs_TallyActivity_nativePickCancelled<'local>(_env: JNIEnv<'local>, _class: JClass<'local>, tag: jni::sys::jlong) {
    // An earlier pick's cancel says nothing about the one out now.
    let Some(pick) = crate::settings_files::pick_from_tag(tag as u64) else { return };
    // For the page that asked (`settings_files::take_cancelled`), and for the loop, which lets
    // go of the pick it keeps in the saved state.
    crate::settings_files::cancel(pick);
    PICK_CANCELLED.store(true, Ordering::SeqCst);
    wake();
}

/// Back, from TallyActivity's back callback (AndroidX would otherwise finish the activity).
static BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tally_rs_TallyActivity_nativeBack<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    BACK.store(true, std::sync::atomic::Ordering::SeqCst);
    wake();
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

/// How this Android reports the window's insets, decided once.
enum InsetsApi {
    /// Android 11 and later: `WindowInsets.getInsets(Type)`, with the masks of `Type.systemBars()`,
    /// `Type.displayCutout()` and `Type.ime()`.
    Types { bars: i32, cutout: i32, ime: i32 },
    /// Android 8 to 10 (`WindowInsets$Type` does not exist): the system window insets, which
    /// include the keyboard (adjustResize puts it there; the window itself stays whole, as
    /// TallyActivity lays it out under the bars), beside the stable ones, which do not.
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

    /// The system bars' and the keyboard's insets, in pixels: (top, bottom bars, keyboard). The
    /// keyboard's is measured from the screen's bottom (it includes the bar under it).
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

/// Send the task to the back, as the back gesture does on an app's first screen
/// (`TallyActivity.leave`, which finishes it instead when the launcher icon waits to switch).
fn leave(app: &AndroidApp) {
    with_activity(app, |env, activity| env.call_method(activity, "leave", "()V", &[]).map(|_| ()));
}

/// While the keyboard may be moving (it is up, or was just asked to show or hide), the insets are
/// read this often.
const KEYBOARD_POLL: Duration = Duration::from_millis(120);
/// How long after typing ends the insets are still watched: the keyboard slides away.
const KEYBOARD_SETTLE_NS: u64 = 1_500_000_000;
/// Said when Appearance's Light or Dark switches the launcher icon: some launchers take the
/// home-screen shortcut away with the old one.
const ICON_MOVED: &str = "The app icon changes to match. If it leaves your home screen, add it again from the app list.";
/// A frame the surface skipped (lost, out of date, late) is tried again this soon.
const RETRY_NS: u64 = 16_000_000;

/// A file came back that no page of this loop asked for: the picker was opened by the process
/// the system stopped (or the activity before this one), as `kind` (its mime). It goes to the
/// page that asks for that kind.
fn adopt_pick(app: &mut App, kind: &str) {
    let Some((name, bytes)) = app.picked.take() else { return };
    if kind.starts_with("text/") {
        if app.route() != &Route::Import {
            app.open_from_drawer(Route::Import);
        }
        crate::import::open_file(app, name, bytes);
    } else {
        // A backup to restore: Backup reads it and asks "Restore this backup?" as after its own
        // Restore; nothing is replaced without the person's yes.
        if app.route() != &Route::Backup {
            app.push(Route::Backup);
        }
        crate::settings::open_backup(app, name, bytes);
    }
}

#[unsafe(no_mangle)]
fn android_main(android: AndroidApp) {
    std::panic::set_hook(Box::new(|info| log(&format!("panic: {info}"))));
    // Called again for each new activity: the wakers must wake this loop, not the last one.
    *WAKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(android.create_waker());
    crate::pictures::set_waker(Box::new(wake));
    crate::sync::set_waker(Box::new(wake));
    let host = Arc::new(AndroidHost { app: android.clone(), asked: AtomicBool::new(false), keyboard_at: AtomicU64::new(0), keyboard_shown: AtomicBool::new(false) });
    let mut app = App::new(host.clone(), now_ns());
    let mut gfx: Option<Gfx> = None;
    let mut screen: Option<Screen<'static>> = None;
    // Kept alive for as long as `screen` draws to it.
    let mut window = None;
    let mut next = Next::Now;
    // Something happened that the next frame must show (input, a wake, the window or insets).
    let mut dirty = true;
    // On screen, so frames are drawn. Android 10 and later resume every app on screen (split
    // screen included), so drawing follows Resume and Pause; before it only the focused one is
    // resumed, and an app beside it is paused but still seen: there drawing follows Start and Stop.
    let multi_resume = AndroidApp::sdk_version() >= 29;
    let mut foreground = false;
    let mut quit = false;
    let mut last_insets = (0.0, 0.0, 0.0);
    let mut insets_api: Option<InsetsApi> = None;
    // Until when the keyboard's insets are watched without an event saying they changed.
    let mut watch_until = 0u64;
    // The bars' icons follow the palette: dark icons on Light, light on Dark.
    let mut bars_light: Option<bool> = None;
    // The launcher icon's look last handed to TallyActivity ([`App::icon_dark`]).
    let mut icon: Option<Option<bool>> = None;
    // The saved state (pages open, a pick out) is read at the first Resume only.
    let mut restored = false;
    // A pick the stopped process left out with the platform, while its file may still come.
    let mut orphan_pick: Option<String> = None;
    // Paused with a sync still running: once it answers (its thread wakes this loop), what was
    // changed while it ran goes too, though no frame comes to start it.
    let mut flush_after = false;

    while !quit {
        let drawing = screen.is_some() && foreground;
        let now = now_ns();
        // While the keyboard is up (the system can take it away), and for a while after it was
        // asked to show or hide. Not merely while a field is focused: one restored typing has its
        // keyboard down until tapped, and nothing would move.
        if app.keyboard_up {
            watch_until = now + KEYBOARD_SETTLE_NS;
        }
        watch_until = watch_until.max(host.keyboard_at.load(Ordering::SeqCst).saturating_add(KEYBOARD_SETTLE_NS));
        let timeout = if !drawing {
            None
        } else if dirty || matches!(next, Next::Now) {
            Some(Duration::ZERO)
        } else {
            // A sync in the background, the next one due, or a toast to take down: wake for it
            // without input. While the keyboard may be moving, look at the insets a few times a
            // second (drawing only if they changed).
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
        let mut config = false;
        android.poll_events(timeout, |event| match event {
            // Another thread woke the loop: a decoded picture, a picked file, Back.
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
                MainEvent::WindowResized { .. } | MainEvent::ContentRectChanged { .. } | MainEvent::InsetsChanged { .. } => resized = true,
                MainEvent::ConfigChanged { .. } => {
                    resized = true;
                    config = true;
                }
                MainEvent::RedrawNeeded { .. } => dirty = true,
                // Drawing follows Resume and Pause (Start and Stop before Android 10) and the
                // window, not focus: in split screen, or under the notification shade, the app is
                // still on screen without focus.
                MainEvent::Start => {
                    // Not before the first Resume, which opens the pages kept: no frame of the
                    // wrong page first.
                    if !multi_resume && restored {
                        foreground = true;
                        dirty = true;
                    }
                }
                MainEvent::Stop => {
                    if !multi_resume {
                        foreground = false;
                    }
                }
                MainEvent::Resume { loader, .. } => {
                    foreground = true;
                    config = true;
                    dirty = true;
                    flush_after = false;
                    // A ledger that would not open (locked by the last run, a full disk) is
                    // tried again each time the app comes back. Not at the first Resume:
                    // App::new tried just now, and a second try would read "Still could not
                    // open it" on the very first screen.
                    if app.ledger_error.is_some() && restored {
                        app.retry_ledger();
                    }
                    if !restored {
                        restored = true;
                        let saved = loader.load().and_then(|b| serde_json::from_slice::<crate::app::Saved>(&b).ok());
                        if let Some(saved) = saved {
                            app.restore(&saved);
                            orphan_pick = saved.picking;
                        }
                    }
                    // Read again (another day, news a sync brought); no sync of its own: those
                    // keep their schedule, which a return after a while meets at once.
                    app.refresh();
                }
                MainEvent::GainedFocus => {
                    config = true;
                    dirty = true;
                    app.refresh();
                }
                MainEvent::Pause => {
                    if multi_resume {
                        foreground = false;
                    }
                    // Frames may stop now: a change still in its debounce syncs now.
                    flush_after = crate::sync::flush(&mut app, now_ns());
                }
                MainEvent::SaveState { saver, .. } => {
                    let mut saved = app.saved();
                    // A pick out, this process's or one a stopped process left that may still
                    // come back here.
                    saved.picking = PICKING.lock().unwrap_or_else(|e| e.into_inner()).clone().or_else(|| orphan_pick.clone());
                    if let Ok(bytes) = serde_json::to_vec(&saved) {
                        saver.store(&bytes);
                    }
                }
                MainEvent::Destroy => quit = true,
                _ => {}
            },
            _ => {}
        });

        // Paused, a sync that was running answered (or this is some other wake): apply it and
        // start the follow-up for changes made while it ran. A pairing that finished meanwhile
        // (the person is in Discord tapping Allow) is kept now too: the process may be stopped
        // before Resume, and its link is spent. Nothing is drawn.
        if !foreground && (flush_after || crate::sync::has_news(&mut app)) {
            flush_after = crate::sync::flush(&mut app, now_ns());
        }

        if config || resized {
            // Appearance "System" follows the phone's dark theme as it changes.
            app.set_system_dark(system_dark(&android));
            dirty = true;
        }
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
                // The keyboard covers the bottom: the app lays out above it.
                app.resize(w as f32, h as f32, found.0, found.1.max(found.2));
                app.keyboard_up = found.2 > found.1 + 1.0;
                dirty = true;
            }
        }

        // A file the person picked (the picker answered while the loop slept).
        if let Some((pick, name, bytes)) = crate::settings_files::take_delivered() {
            *PICKING.lock().unwrap_or_else(|e| e.into_inner()) = None;
            app.file_delivered(pick, name, bytes);
            dirty = true;
        }
        // The picker came back empty: nothing is out any more, for this loop or a stopped one.
        // Taken only once the kept state is read (the answer can come before the first Resume),
        // so the stopped process's pick is let go too.
        if restored && PICK_CANCELLED.swap(false, Ordering::SeqCst) {
            *PICKING.lock().unwrap_or_else(|e| e.into_inner()) = None;
            orphan_pick = None;
        }
        // This loop asked for a file of its own: whatever comes back is for that page.
        if orphan_pick.is_some() && host.asked.load(Ordering::SeqCst) {
            orphan_pick = None;
        }
        // The file the stopped process asked for came back here, where no page waits for it.
        if app.picked.is_some()
            && app.ledger_error.is_none()
            && let Some(kind) = orphan_pick.take()
        {
            adopt_pick(&mut app, &kind);
            dirty = true;
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
                        // Only the text: its selection and compose spans count UTF-16 units, and
                        // the app keeps the caret at the end.
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
        // Back on a field whose keyboard never came up (one restored typing): there is no
        // keyboard to put away, so the field lets go and Back goes back as well.
        if go_back && !done_typing && app.focus.id.is_some() && !host.keyboard_shown.load(Ordering::SeqCst) {
            app.finish_typing();
        }
        if done_typing || (go_back && app.focus.id.is_some()) {
            // Text that came in this same batch reaches its field before the field lets go.
            app.done_typing(now_ns());
            go_back = false;
            dirty = true;
        }
        if go_back {
            dirty = true;
            if !app.back() {
                leave(&android);
            }
        }

        if foreground && bars_light != Some(app.palette.light) {
            let light = app.palette.light;
            with_activity(&android, |env, activity| env.call_method(activity, "setLightBars", "(Z)V", &[JValue::Bool(light as u8)]).map(|_| ()));
            bars_light = Some(light);
        }
        // The home screen icon (icon E, dark or light) follows Appearance only when the person
        // picked Light or Dark there, never the phone's own light and dark: turning a launcher
        // alias off makes the launcher drop its home-screen shortcut. TallyActivity.setIcon
        // switches it when it is safe to, and answers whether to say so (once per change).
        let want = app.icon_dark();
        if foreground && icon != Some(want) {
            let choice = match want {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            };
            let tell = with_activity(&android, |env, activity| env.call_method(activity, "setIcon", "(I)Z", &[JValue::Int(choice)])?.z());
            if tell == Some(true) {
                app.say(ICON_MOVED, None);
                dirty = true;
            }
            icon = Some(want);
        }

        // Draw only when something changed or the app asked for this frame.
        let due = dirty || matches!(next, Next::Now) || matches!(next, Next::At(at) if now_ns() >= at);
        if foreground
            && due
            && let (Some(g), Some(s)) = (&mut gfx, &mut screen)
        {
            dirty = false;
            next = app.frame(now_ns());
            // A picture found room in the atlas after the frame was drawn: show it.
            if app.canvas.take_late_pictures() {
                next = Next::Now;
            }
            g.upload_glyphs(&mut app.canvas.uploads);
            g.upload_images(&mut app.canvas.image_uploads);
            let (globals, clear) = (app.canvas.globals, app.clear_colour());
            if !s.frame(g, &globals, clear, &app.canvas.insts, &app.canvas.batches) {
                // Skipped: what it showed is not on screen yet, so draw again soon.
                next = Next::At(now_ns() + RETRY_NS);
            }
        }
    }
}
