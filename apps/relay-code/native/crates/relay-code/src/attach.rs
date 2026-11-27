//! A photo or screenshot for the agent (the composer's +): the platform's picker hands a file
//! here ([`picked`]; Android shrinks it to 2048 px first), a worker thread sends it to the agent's
//! checkout with `session.upload`, and the path it was written to goes into the message, which is
//! how Claude Code and Codex read a picture. Nothing of it runs on the UI thread.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rc_link::Link;
use serde_json::{Value, json};

/// Pictures picked, each with the agent whose chat asked for it.
static PICKED: Mutex<Vec<(Option<String>, PathBuf)>> = Mutex::new(Vec::new());
/// Uploads that finished, each with its agent.
static DONE: Mutex<Vec<(String, Result<String, String>)>> = Mutex::new(Vec::new());
/// The chat the picker was last opened from.
static PICK_FOR: Mutex<Option<String>> = Mutex::new(None);

/// The picker is opening from `agent`'s chat: what it brings goes to that agent.
pub fn pick_for(agent: &str) {
    *PICK_FOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(agent.to_string());
}

/// A small copy of each picture sent, by the path the PC wrote it to: your message shows it
/// instead of the path (RGBA, premultiplied, row by row).
pub struct Thumb {
    pub w: u32,
    pub h: u32,
    pub pixels: Vec<u8>,
}

/// The newest [`KEEP_THUMBS`] copies, oldest first (A23: a long chat of photos holds no more).
static THUMBS: Mutex<Vec<(String, Arc<Thumb>)>> = Mutex::new(Vec::new());

/// How many sent pictures keep their copy; older bubbles show the path's chip.
const KEEP_THUMBS: usize = 16;

fn keep(path: &str, t: Thumb) {
    let mut k = THUMBS.lock().unwrap_or_else(|e| e.into_inner());
    k.retain(|(p, _)| p != path);
    k.push((path.to_string(), Arc::new(t)));
    if k.len() > KEEP_THUMBS {
        let extra = k.len() - KEEP_THUMBS;
        k.drain(..extra);
    }
}

/// The longest side of a kept copy, in pixels: twice the size it is drawn at.
const THUMB: u32 = 440;

/// The copy of the picture sent as `path`, while the app has it.
pub fn thumb(path: &str) -> Option<Arc<Thumb>> {
    THUMBS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(p, _)| p == path.trim()).map(|(_, t)| t.clone())
}

/// Keep a copy for `path` (screenshots' sample data).
pub fn keep_thumb(path: &str, t: Thumb) {
    keep(path, t);
}

/// A line of a message that names a picture: a path ending in an image's extension.
pub fn is_picture(line: &str) -> bool {
    let l = line.trim().to_ascii_lowercase();
    l.contains('/') && !l.contains(char::is_whitespace) && [".png", ".jpg", ".jpeg", ".webp", ".gif"].iter().any(|e| l.ends_with(e))
}

/// A small copy of a picture's bytes (on the upload's worker thread, never the UI's).
fn make_thumb(bytes: &[u8]) -> Option<Thumb> {
    let p = crate::design_paint::pictures::decode(bytes, (THUMB, THUMB)).ok()?;
    let s = (f64::from(THUMB) / f64::from(p.w.max(p.h))).min(1.0);
    let (w, h) = (((f64::from(p.w) * s).round() as u32).max(1), ((f64::from(p.h) * s).round() as u32).max(1));
    let level = p.level_for(f64::from(p.w) / f64::from(w));
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = p.sample(level, (f64::from(x) + 0.5) / f64::from(w), (f64::from(y) + 0.5) / f64::from(h));
            pixels.extend(c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8));
        }
    }
    Some(Thumb { w, h, pixels })
}

/// The picker brought a file (any thread).
pub fn picked(p: PathBuf) {
    let agent = PICK_FOR.lock().unwrap_or_else(|e| e.into_inner()).clone();
    PICKED.lock().unwrap_or_else(|e| e.into_inner()).push((agent, p));
    crate::wake::wake();
}

pub fn take_picked() -> Vec<(Option<String>, PathBuf)> {
    std::mem::take(&mut *PICKED.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Uploads that finished, with their agent: the path to name in the message, or why it failed.
pub fn take_done() -> Vec<(String, Result<String, String>)> {
    std::mem::take(&mut *DONE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Send the file to `session`'s checkout, off the UI thread.
pub fn upload(link: Link, session: String, path: PathBuf) {
    let _ = std::thread::Builder::new().name("rc-upload".into()).spawn(move || {
        let out = match std::fs::read(&path) {
            Err(e) => Err(format!("Couldn't read the picture: {e}")),
            Ok(b) if b.len() > 4 << 20 => Err("That picture is over 4 MB".into()),
            Ok(b) => match link.call_with("session.upload", json!({"session": session, "bytes_b64": base64(&b)}), Duration::from_secs(90)).wait(Duration::from_secs(95)) {
                Ok(v) => {
                    let path = v.get("path").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "The PC kept no path".to_string());
                    if let (Ok(p), Some(t)) = (&path, make_thumb(&b)) {
                        keep(p, t);
                    }
                    path
                }
                Err(e) => Err(crate::errors::words(&e)),
            },
        };
        let _ = std::fs::remove_file(&path);
        DONE.lock().unwrap_or_else(|e| e.into_inner()).push((session, out));
        crate::wake::wake();
    });
}

/// Standard base64 with padding.
pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_is_standard() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_picture_is_told_by_its_path_and_shrunk_once() {
        assert!(super::is_picture("/home/a/.relay/uploads/shot-1.PNG"));
        assert!(!super::is_picture("see shot.png"));
        assert!(!super::is_picture("notes.md"));
        let mut png = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png, 1000, 500);
            e.set_color(png::ColorType::Rgba);
            let data: Vec<u8> = (0..1000 * 500).flat_map(|i| [(i % 256) as u8, 0, 0, 255]).collect();
            e.write_header().unwrap().write_image_data(&data).unwrap();
        }
        let t = super::make_thumb(&png).unwrap();
        assert_eq!((t.w, t.h), (super::THUMB, super::THUMB / 2));
        assert_eq!(t.pixels.len(), (t.w * t.h * 4) as usize);
    }

    #[test]
    fn only_the_newest_sent_pictures_keep_their_copy() {
        let tag = format!("/keep-test-{}", std::process::id());
        for i in 0..20 {
            super::keep(&format!("{tag}/{i}.png"), super::Thumb { w: 1, h: 1, pixels: vec![0; 4] });
        }
        assert!(super::thumb(&format!("{tag}/19.png")).is_some());
        assert!(super::thumb(&format!("{tag}/3.png")).is_none(), "the oldest went");
    }
}
