//! A photo or screenshot for the agent (the composer's +): the platform's picker hands a file
//! here ([`picked`]; Android shrinks it to 2048 px first), a worker thread sends it to the agent's
//! checkout with `session.upload`, and the path it was written to goes into the message, which is
//! how Claude Code and Codex read a picture. Nothing of it runs on the UI thread.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use rc_link::Link;
use serde_json::{Value, json};

static PICKED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static DONE: Mutex<Vec<Result<String, String>>> = Mutex::new(Vec::new());

/// The picker brought a file (any thread).
pub fn picked(p: PathBuf) {
    PICKED.lock().unwrap_or_else(|e| e.into_inner()).push(p);
    crate::wake::wake();
}

pub fn take_picked() -> Vec<PathBuf> {
    std::mem::take(&mut *PICKED.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Uploads that finished: the path to name in the message, or why it failed.
pub fn take_done() -> Vec<Result<String, String>> {
    std::mem::take(&mut *DONE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Send the file to `session`'s checkout, off the UI thread.
pub fn upload(link: Link, session: String, path: PathBuf) {
    let _ = std::thread::Builder::new().name("rc-upload".into()).spawn(move || {
        let out = match std::fs::read(&path) {
            Err(e) => Err(format!("Couldn't read the picture: {e}")),
            Ok(b) if b.len() > 4 << 20 => Err("That picture is over 4 MB".into()),
            Ok(b) => match link.call("session.upload", json!({"session": session, "bytes_b64": base64(&b)})).wait(Duration::from_secs(60)) {
                Ok(v) => v.get("path").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "The PC kept no path".to_string()),
                Err(e) => Err(e.message),
            },
        };
        let _ = std::fs::remove_file(&path);
        DONE.lock().unwrap_or_else(|e| e.into_inner()).push(out);
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
}
