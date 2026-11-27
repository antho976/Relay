//! App updates from the PC this phone is linked to (backlog D10, as Leaf has them): the app knows
//! its own build (`RELAY_CODE_BUILD`, the commit count `tools/build-lib.sh` builds at, the same
//! number Gradle's `versionCode` takes), asks the PC's `phone.app.latest` when it opens, when it
//! comes back to the front, and at most every [`CHECK_EVERY_MS`] (15 minutes) while in front, and
//! offers a newer build in a calm sheet: "Relay Code 0.N is ready · 18 MB", Update or Later (Later
//! hides that build for a day).
//!
//! Update downloads `phone.app.apk` in pieces into `updates/` on a thread of its own, checks its
//! SHA-256 against the one `latest` named, then hands it to Android's installer through
//! `CodeActivity`. When the app may not install apps yet, Android's "Install unknown apps" page
//! opens instead and the sheet says to come back and tap Update again (the file is kept). The
//! desktop shows the sheet; its Update only says updates install on the phone.
//!
//! The PC serves it (`apps/relay-code/tools/publish-apk.sh` puts it there): nothing to deploy.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use rc_link::{Call, Link};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;

use crate::app::{App, Route};
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};

/// This app's build: the commit count it was built at (`RELAY_CODE_BUILD`), 0 for one without.
pub const BUILD: u32 = parse(option_env!("RELAY_CODE_BUILD"));

/// The PC is asked for a newer app at most this often.
pub const CHECK_EVERY_MS: u64 = 15 * 60_000;
/// "Later" hides a build this long.
const LATER_MS: u64 = 24 * 3_600_000;
/// One piece of the APK per request: the PC answers at most 3 MiB.
const PIECE: u64 = 3 * 1024 * 1024;

const fn parse(s: Option<&str>) -> u32 {
    let Some(s) = s else { return 0 };
    let b = s.as_bytes();
    let mut i = 0;
    let mut n: u32 = 0;
    while i < b.len() {
        let d = b[i];
        if d < b'0' || d > b'9' {
            return 0;
        }
        n = n.saturating_mul(10).saturating_add((d - b'0') as u32);
        i += 1;
    }
    n
}

/// What the PC holds (`phone.app.latest`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Latest {
    pub build: u32,
    #[serde(default)]
    pub version: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub made_at: i64,
}

/// How Android took the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    /// Its installer opened.
    Started,
    /// The app may not install apps yet: Android's page to allow it opened.
    NeedsLeave,
    Failed,
}

/// Time to ask for a newer app again: never asked, `CHECK_EVERY_MS` since, or the clock went back.
pub fn due(last_ms: Option<u64>, now_ms: u64) -> bool {
    match last_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= CHECK_EVERY_MS || now_ms < last,
    }
}

/// "0.312": what a build is called (Gradle's `versionName` says the same).
pub fn version_of(build: u32) -> String {
    format!("0.{build}")
}

/// "18 MB".
pub fn size_words(bytes: u64) -> String {
    let mb = bytes as f64 / 1e6;
    if mb >= 10.0 { format!("{} MB", mb.round() as u64) } else { format!("{mb:.1} MB") }
}

#[derive(Default)]
pub struct State {
    pub latest: Option<Latest>,
    /// The sheet is up.
    pub open: bool,
    /// When the PC was last asked (ms), and the answer on its way.
    checked: Option<u64>,
    call: Option<Call>,
    /// Bytes downloaded so far, while downloading.
    got: Arc<AtomicU64>,
    /// The download's end: the file, checked, or why not.
    done: Arc<Mutex<Option<Result<PathBuf, String>>>>,
    pub downloading: bool,
    /// The file downloaded and checked, waiting to be installed.
    pub ready: Option<PathBuf>,
    /// One line under the title: why it waits, or what went wrong.
    pub note: Option<String>,
    /// "Check for updates now" asked: the answer is said in words, whatever it is, and Later
    /// does not hide a build asked for by hand.
    pub by_hand: bool,
}

impl State {
    pub fn close_over(&mut self) -> bool {
        std::mem::take(&mut self.open)
    }
}

/// Ask the PC when it is time, and take its answer. Call each frame while the app is in front.
pub fn keep_up(app: &mut App) {
    if let Some(call) = &app.st.update.call
        && let Some(r) = call.poll()
    {
        app.st.update.call = None;
        let by_hand = std::mem::take(&mut app.st.update.by_hand);
        match r {
            Ok(v) if !v.is_null() => {
                if let Ok(l) = serde_json::from_value::<Latest>(v) {
                    let mine = this_build();
                    if by_hand && l.build <= mine {
                        app.toast(&format!("You have the newest, {}", version_of(mine.max(l.build))));
                    }
                    if by_hand && l.build > mine {
                        app.clear_update_later();
                    }
                    offered(app, l);
                }
            }
            Ok(_) if by_hand => app.toast("No build is published on your PC yet"),
            Ok(_) => {}
            // An older PC without phone.app.* has nothing to offer: said only when asked by hand.
            Err(e) if by_hand => app.toast(&crate::errors::words(&e)),
            Err(e) => app.host.log(&format!("update: {}: {}", e.code, e.message)),
        }
    }
    let now = rc_link::link::now_ms();
    if app.online() && app.foreground && app.st.update.call.is_none() && due(app.st.update.checked, now) {
        app.st.update.checked = Some(now);
        app.st.update.call = Some(app.link.call("phone.app.latest", json!({})));
    }
    if let Some(at) = app.st.update.checked {
        app.wake_at(app.now_ns + (at + CHECK_EVERY_MS).saturating_sub(now) * 1_000_000);
    }
}

/// [`BUILD`], read as a value: a build without a number is 0, which is still compared at run time.
fn this_build() -> u32 {
    BUILD
}

/// "Check for updates now": ask the PC at once, and say what it answered.
pub fn check_now(app: &mut App) {
    if !app.online() {
        app.toast("Your PC is out of reach. Try again once it answers");
        return;
    }
    if app.st.update.call.is_some() {
        app.st.update.by_hand = true;
        return;
    }
    app.st.update.by_hand = true;
    app.st.update.checked = Some(rc_link::link::now_ms());
    app.st.update.call = Some(app.link.call("phone.app.latest", json!({})));
}

/// A check is on its way.
pub fn checking(app: &App) -> bool {
    app.st.update.call.is_some()
}

/// The PC holds `l`: offered when it is newer than this app and not put off.
pub fn offered(app: &mut App, l: Latest) {
    let now = rc_link::link::now_ms();
    let later = app.update_later().is_some_and(|(b, until)| b == l.build && now < until);
    if l.build > BUILD && !later {
        let st = &mut app.st.update;
        if st.latest.as_ref().is_none_or(|old| old.build != l.build) {
            st.ready = None;
            st.note = None;
            // A sheet already put away for this build stays away until the next check.
            st.open = true;
        }
        st.latest = Some(l);
    }
}

/// Download the APK and check it, on a thread; [`take_done`] picks the end up.
fn start(app: &mut App, l: &Latest) {
    let st = &mut app.st.update;
    st.downloading = true;
    st.note = None;
    st.got.store(0, Ordering::Relaxed);
    let (got, done) = (st.got.clone(), st.done.clone());
    let dir = app.data.join("updates");
    let link = app.link.clone();
    let l = l.clone();
    let _ = std::thread::Builder::new().name("rc-update".into()).spawn(move || {
        let r = fetch(&link, &dir, &l, &got);
        *done.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        crate::wake::wake();
    });
}

fn fetch(link: &Link, dir: &Path, l: &Latest, got: &AtomicU64) -> Result<PathBuf, String> {
    // Only this build's file is kept: an older one is of no use.
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the update: {e}"))?;
    let file = dir.join(format!("relay-code-{}.apk", l.build));
    let part = file.with_extension("part");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|e| format!("Couldn't save the update: {e}"))?);
    let mut sha = sha2::Sha256::new();
    let mut offset = 0u64;
    loop {
        let v = link
            .call("phone.app.apk", json!({"offset": offset, "len": PIECE}))
            .wait(Duration::from_secs(90))
            .map_err(|e| format!("The PC stopped sending the update. {}", crate::errors::words(&e)))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(v.get("bytes_b64").and_then(Value::as_str).unwrap_or_default())
            .map_err(|_| "The update came garbled. Try again".to_string())?;
        sha.update(&bytes);
        out.write_all(&bytes).map_err(|e| format!("Couldn't save the update: {e}"))?;
        offset += bytes.len() as u64;
        got.store(offset, Ordering::Relaxed);
        crate::wake::wake();
        if v.get("eof").and_then(Value::as_bool).unwrap_or(true) || bytes.is_empty() {
            break;
        }
    }
    out.flush().map_err(|e| format!("Couldn't save the update: {e}"))?;
    drop(out);
    let have: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if have != l.sha256.to_ascii_lowercase() {
        let _ = std::fs::remove_file(&part);
        return Err("The download didn't match what the PC announced. Publish it again, then try again".into());
    }
    std::fs::rename(&part, &file).map_err(|e| format!("Couldn't save the update: {e}"))?;
    Ok(file)
}

/// The download ended: install what came, or say why not.
fn take_done(app: &mut App) {
    let end = app.st.update.done.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some(end) = end else { return };
    app.st.update.downloading = false;
    match end {
        Ok(file) => {
            app.st.update.ready = Some(file);
            install(app);
        }
        Err(e) => app.st.update.note = Some(e),
    }
}

fn install(app: &mut App) {
    let Some(file) = app.st.update.ready.clone() else { return };
    match app.host.install_apk(&file) {
        Install::Started => app.st.update.open = false,
        Install::NeedsLeave => {
            app.st.update.note = Some("Allow Relay Code to install apps on the page that opened, then come back and tap Install.".into());
        }
        Install::Failed => app.st.update.note = Some("Android didn't open its installer. Try again".into()),
    }
}

/// The sheet may show over this page.
pub fn shows_over(route: &Route) -> bool {
    !matches!(route, Route::Pair)
}

/// The sheet, over whatever page is open. Call after the page drew. True while it moves.
pub fn draw(app: &mut App) -> bool {
    take_done(app);
    if !app.st.update.open || !shows_over(app.route()) {
        return false;
    }
    let Some(l) = app.st.update.latest.clone() else {
        app.st.update.open = false;
        return false;
    };
    let st = &app.st.update;
    let (downloading, got, note, ready) = (st.downloading, st.got.load(Ordering::Relaxed), st.note.clone(), st.ready.is_some());
    let mut cx = crate::cx!(app);
    let note_t = Text::new(ty::UI, if note.is_some() { t::INK2 } else { t::INK3 }).line(1.4);
    let x = t::PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let have = if BUILD == 0 { "This build has no number".to_string() } else { format!("You have {}", version_of(BUILD)) };
    let line = note.clone().unwrap_or_else(|| format!("{have}. It downloads from your PC, then Android asks to install it."));
    let note_h = cx.c.para_h(&line, w, note_t, 4);
    let (top, outside) = kit::sheet(&mut cx, 30.0 + 26.0 + 10.0 + note_h + 22.0 + 48.0 + 18.0);
    let tt = Text::new(ty::SECTION, t::INK).semi();
    cx.c.text_fit(&format!("Relay Code {} is ready · {}", version_of(l.build), size_words(l.size)), x, top + 30.0, w, tt);
    cx.c.para(&line, x, top + 30.0 + 26.0 + 10.0, w, note_t, 4);
    let ky = top + 30.0 + 26.0 + 10.0 + note_h + 22.0;
    let bw = (w - 10.0) / 2.0;
    let later = kit::plain(&mut cx, x, ky, bw, 48.0, "Later", None);
    let mut update = false;
    if downloading {
        // The key shows how far the download is, and takes no tap.
        let f = if l.size > 0 { (got as f32 / l.size as f32).clamp(0.0, 1.0) } else { 0.0 };
        let kx = x + bw + 10.0;
        cx.c.rect(kx, ky, bw, 48.0, t::KEY_R, t::S2);
        cx.c.rect(kx, ky, (bw * f).max(t::KEY_R * 2.0), 48.0, t::KEY_R, t::ACCENT_DIM);
        let kt = Text::new(ty::BODY, t::INK).semi().center();
        cx.c.text(&format!("{}%", (f * 100.0).round() as u32), kx + bw / 2.0, ky + (48.0 - kt.lh()) / 2.0, kt);
    } else if kit::primary(&mut cx, x + bw + 10.0, ky, bw, 48.0, if ready { "Install" } else { "Update" }, None) {
        update = true;
    }
    if outside && !downloading {
        app.st.update.open = false;
    }
    if later {
        app.st.update.open = false;
        app.set_update_later(l.build, rc_link::link::now_ms() + LATER_MS);
    } else if update {
        if !app.host.can_install() {
            app.host.log(&format!("update: build {} ({} bytes) is on the PC; updates install on the phone", l.build, l.size));
            app.st.update.open = false;
            app.toast("Updates install on the phone");
        } else if app.st.update.ready.is_some() {
            install(app);
        } else {
            start(app, &l);
        }
    }
    downloading
}

/// Screenshots: build `build` waiting, with how much of it came.
pub fn sample(app: &mut App, build: u32, got: Option<u64>) {
    let size = 18_400_000;
    app.st.update.latest = Some(Latest { build, version: version_of(build), size, sha256: String::new(), made_at: 0 });
    app.st.update.open = true;
    if let Some(g) = got {
        app.st.update.downloading = true;
        app.st.update.got.store(g, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_number_reads_from_its_digits() {
        assert_eq!(parse(Some("312")), 312);
        assert_eq!(parse(Some("")), 0);
        assert_eq!(parse(Some("3a")), 0);
        assert_eq!(parse(None), 0);
        assert_eq!(version_of(312), "0.312");
        assert_eq!(size_words(18_400_000), "18 MB");
        assert_eq!(size_words(4_260_000), "4.3 MB");
    }

    #[test]
    fn a_newer_app_is_asked_for_every_quarter_hour_at_most() {
        let t = 1_700_000_000_000;
        assert!(due(None, t), "never asked");
        assert!(!due(Some(t), t + 60_000), "a minute after");
        assert!(!due(Some(t), t + 14 * 60_000));
        assert!(due(Some(t), t + 15 * 60_000), "a quarter of an hour after");
        assert!(due(Some(t), t - 60_000), "the clock went back");
    }
}
