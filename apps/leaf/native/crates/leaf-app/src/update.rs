//! App updates from the owner's server: the app knows its own build number (`LEAF_BUILD`, the
//! commit count `tools/build-lib.sh` builds at, the same number Gradle's `versionCode` takes), asks
//! `GET /v1/app/latest` when it opens and at most every [`CHECK_EVERY_MS`], and offers a newer
//! build in a calm sheet: "Leaf 0.N is ready · 18 MB", Update or Later (Later hides that build for
//! a day).
//!
//! Update downloads `/v1/app/apk` into the cache on a thread of its own, checks its SHA-256
//! against the one `latest` named, then hands it to Android's installer through `LeafActivity`.
//! When Leaf may not install apps yet, Android's "Install unknown apps" page for Leaf opens
//! instead, and the sheet says to come back and tap Update again (the file is kept). The desktop
//! shows the sheet; its Update only logs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use leaf_proto::AppLatest;

use crate::app::{App, Install, Route};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit;
use crate::net::Server;
use crate::theme::{self as t, PAD, ty};

/// This app's build: the commit count it was built at (`LEAF_BUILD`), 0 for a build without one.
pub const BUILD: u32 = parse(option_env!("LEAF_BUILD"));

/// The server is asked for a newer app at most this often.
pub const CHECK_EVERY_MS: i64 = 6 * 3_600_000;
/// "Later" hides a build this long.
const LATER_MS: i64 = 24 * 3_600_000;

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

/// "0.312": what a build is called (Gradle's `versionName` says the same).
pub fn version_of(build: u32) -> String {
    format!("0.{build}")
}

#[derive(Default)]
pub struct State {
    pub latest: Option<AppLatest>,
    /// The sheet is showing.
    pub open: bool,
    /// Bytes downloaded so far, while downloading.
    got: Arc<AtomicU64>,
    /// The download's end: the file, checked, or why not.
    done: Arc<Mutex<Option<Result<PathBuf, String>>>>,
    pub downloading: bool,
    /// The file downloaded and checked, waiting to be installed.
    pub ready: Option<PathBuf>,
    /// One line under the title: why it waits, or what went wrong.
    pub note: Option<String>,
}

/// The server holds `l`: offered when it is newer than this app and not put off.
pub fn offered(app: &mut App, l: AppLatest) {
    let now = crate::shelf::now_ms();
    let later = app.update_later.is_some_and(|(b, until)| b == l.build && now < until);
    if l.build > BUILD && !later {
        let st = &mut app.st.update;
        if st.latest.as_ref().is_none_or(|old| old.build != l.build) {
            st.ready = None;
            st.note = None;
        }
        st.latest = Some(l);
        st.open = true;
    }
}

/// "18 MB".
fn size_words(bytes: u64) -> String {
    let mb = bytes as f64 / 1e6;
    if mb >= 10.0 { format!("{} MB", mb.round() as u64) } else { format!("{mb:.1} MB") }
}

/// Download the APK and check it, on a thread; [`take_done`] picks the end up.
fn start(app: &mut App, l: &AppLatest) {
    let Some(server) = app.server.clone() else { return };
    let st = &mut app.st.update;
    st.downloading = true;
    st.note = None;
    st.got.store(0, Ordering::Relaxed);
    let (got, done) = (st.got.clone(), st.done.clone());
    let dir = app.host.cache_dir().join("updates");
    let (build, sha) = (l.build, l.sha256.to_ascii_lowercase());
    let _ = std::thread::Builder::new().name("leaf-update".into()).spawn(move || {
        let r = fetch(&server, &dir, build, &sha, &got);
        *done.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        crate::wake::wake();
    });
}

fn fetch(server: &Server, dir: &std::path::Path, build: u32, sha: &str, got: &AtomicU64) -> Result<PathBuf, String> {
    // Only this build's file is kept: an older one is of no use.
    let _ = std::fs::remove_dir_all(dir);
    let file = dir.join(format!("leaf-{build}.apk"));
    let words = |f: crate::net::Fail| f.words(&crate::logic::server_label(&server.base));
    let have = crate::net::Client::new().download(server, "/v1/app/apk", &file, got).map_err(words)?;
    if have != sha {
        let _ = std::fs::remove_file(&file);
        return Err("The download didn't match what your server announced. Try again later".into());
    }
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
        Install::Started => {
            app.st.update.open = false;
        }
        Install::NeedsLeave => {
            app.st.update.note = Some("Allow Leaf to install apps on the page that opened, then come back and tap Update.".into());
        }
        Install::Unsupported | Install::Failed => {
            app.st.update.note = Some("Android didn't open its installer. Try again".into());
        }
    }
}

/// The sheet, over whatever page is open (never over a book or a strip being read). Call after
/// the page drew, with the tap held from it ([`App::frame`]).
pub fn draw(app: &mut App) {
    take_done(app);
    let Some(l) = app.st.update.latest.clone() else {
        app.st.update.open = false;
        return;
    };
    let st = &app.st.update;
    let (downloading, got, note, ready) = (st.downloading, st.got.load(Ordering::Relaxed), st.note.clone(), st.ready.is_some());
    let mut cx = cx!(app);
    let note_t = Text::new(ty::UI, if note.is_some() { t::INK2 } else { t::INK3 }).line(1.4);
    let x = PAD + 4.0;
    let w = WIDTH - 2.0 * x;
    let have = if BUILD == 0 { "This build has no number".to_string() } else { format!("You have {}", version_of(BUILD)) };
    let line = note.clone().unwrap_or_else(|| format!("{have}. It downloads from your server, then Android asks to install it."));
    let note_h = cx.c.para_h(&line, w, note_t, 4);
    let (top, outside) = kit::sheet(&mut cx, 30.0 + 26.0 + 10.0 + note_h + 22.0 + 48.0 + 18.0);
    let tt = Text::new(ty::SECTION, t::INK).semi();
    cx.c.text_fit(&format!("Leaf {} is ready · {}", version_of(l.build), size_words(l.size)), x, top + 30.0, w, tt);
    cx.c.para(&line, x, top + 30.0 + 26.0 + 10.0, w, note_t, 4);
    let ky = top + 30.0 + 26.0 + 10.0 + note_h + 22.0;
    let bw = (w - 10.0) / 2.0;
    let mut later = false;
    let mut update = false;
    if kit::plain(&mut cx, x, ky, bw, 48.0, "Later", None) {
        later = true;
    }
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
    cx.ui.tap = None;
    if outside {
        app.st.update.open = false;
    }
    if later {
        app.st.update.open = false;
        app.update_later = Some((l.build, crate::shelf::now_ms() + LATER_MS));
        app.save_config();
    } else if update {
        if !app.host.can_install() {
            app.host.log(&format!("update: build {} ({} bytes) is on the server; updates install on the phone", l.build, l.size));
            app.st.update.open = false;
            app.toast("Updates install on the phone");
        } else if app.st.update.ready.is_some() {
            install(app);
        } else {
            start(app, &l);
        }
    }
    if downloading {
        app.wake_at(app.now_ns + 200_000_000);
    }
}

/// The sheet may show over this page.
pub fn shows_over(route: &Route) -> bool {
    !matches!(route, Route::Reader | Route::Book)
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
}
