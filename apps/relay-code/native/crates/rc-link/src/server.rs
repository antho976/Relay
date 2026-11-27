//! Pairing through the owner's server (docs/CLOUD.md, "Connecting through your server"), as the
//! Kotlin app's `ServerConnect.kt` does it: the phone keeps a random verifier and sends only its
//! sha256; the owner taps Allow on the Discord DM the server sends; the PC opens a pairing window
//! and hands its link over through the server; the phone collects it with the verifier, then pairs
//! with that link ([`crate::link::pair`]). No scanning, no code to type. Blocking: run it on a
//! worker thread.

use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::wire::{hex, random_hex};

/// The Relay app asking for the PC: the PC opens the window and asks on its own screen.
pub const KIND_PC: &str = "relay";

/// A request the owner has not answered yet, and the secret that collects its link.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub id: String,
    /// The four characters the phone and the DM both show.
    pub code: String,
    pub notified: bool,
    pub verifier: String,
}

/// Where a request stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Waiting,
    /// Allowed on Discord; the PC has not opened its window yet (it must be on, running Relay).
    WaitingPc,
    Denied,
    Expired,
    Spent,
    Ready(String),
    Trouble(String),
}

pub struct Server {
    /// `https://host`, however it was typed.
    pub base: String,
    agent: ureq::Agent,
}

impl Server {
    pub fn new(typed: &str) -> Server {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(8)))
            // A server that sleeps wakes on the first request: give it the time it takes.
            .timeout_global(Some(Duration::from_secs(45)))
            .http_status_as_error(false)
            .build()
            .into();
        Server { base: normalize(typed), agent }
    }

    pub fn host(&self) -> &str {
        self.base.split_once("://").map_or(self.base.as_str(), |(_, h)| h)
    }

    fn post(&self, path: &str, body: Value) -> Result<(u16, Option<Value>), String> {
        let r = self.agent.post(format!("{}{path}", self.base)).header("content-type", "application/json").send(body.to_string());
        read(r)
    }

    /// Ask the owner to let this phone pair with the PC.
    pub fn ask(&self, device_name: &str) -> Result<Asked, String> {
        let verifier = random_hex(32);
        let body = json!({"kind": KIND_PC, "device": device_name, "challenge": challenge_of(&verifier)});
        let (mut status, mut o) = self.post("/api/connect/enroll", body.clone())?;
        // Another of the owner's apps on the same network may have asked a moment before.
        for _ in 0..3 {
            if status == 429 && err(&o) == Some("slow_down") {
                std::thread::sleep(Duration::from_secs(6));
                (status, o) = self.post("/api/connect/enroll", body.clone())?;
            }
        }
        if status == 201
            && let Some(a) = &o
        {
            let id = a.get("id").and_then(Value::as_str).ok_or("Your server's answer did not read")?.to_string();
            let code = a.get("code").and_then(Value::as_str).unwrap_or_default().to_string();
            let notified = a.get("notified").and_then(Value::as_bool) == Some(true);
            return Ok(Asked { id, code, notified, verifier });
        }
        Err(match err(&o) {
            Some("no_pc") => format!("No PC is connected to your server yet. On the PC, run: relay cloud connect {}", self.host()),
            Some("busy") => "Your server has requests waiting already. Answer those, then try again.".into(),
            Some("slow_down") => "Something on your network keeps asking. Wait a minute and try again.".into(),
            Some("off") => "Your server does not take requests to connect: it is turned off there.".into(),
            _ => format!("Your server could not take the request ({status})"),
        })
    }

    /// Where `asked` stands; once the PC opened its window, the link to it.
    pub fn claim(&self, asked: &Asked) -> Reply {
        match self.post("/api/connect/claim", json!({"id": asked.id, "verifier": asked.verifier})) {
            Err(e) => Reply::Trouble(e),
            Ok((status, None)) => Reply::Trouble(format!("Your server's answer did not read ({status})")),
            Ok((status, Some(o))) => read_reply(&o, status),
        }
    }
}

fn read(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<(u16, Option<Value>), String> {
    let mut r = r.map_err(|_| "Your server could not be reached. Check the connection, then try again.".to_string())?;
    let status = r.status().as_u16();
    let body = r.body_mut().with_config().limit(1 << 20).read_to_vec().unwrap_or_default();
    Ok((status, serde_json::from_slice::<Value>(&body).ok().filter(Value::is_object)))
}

fn err(o: &Option<Value>) -> Option<&str> {
    o.as_ref()?.get("error")?.as_str()
}

pub fn read_reply(o: &Value, status: u16) -> Reply {
    match o.get("status").and_then(Value::as_str) {
        Some("waiting") => Reply::Waiting,
        Some("waiting_pc") => Reply::WaitingPc,
        Some("denied") => Reply::Denied,
        Some("expired") => Reply::Expired,
        Some("spent") => Reply::Spent,
        Some("ready") => match o.get("link").and_then(Value::as_str) {
            Some(l) if l.starts_with("relay://pair?") => Reply::Ready(l.to_string()),
            _ => Reply::Trouble("Your server's link did not read".into()),
        },
        Some("error") => Reply::Trouble(o.get("reason").and_then(Value::as_str).unwrap_or("Your server could not answer").to_string()),
        _ => Reply::Trouble(format!("Your server answered {status}")),
    }
}

/// The sha256 of the verifier as text: what the server keeps and compares.
pub fn challenge_of(verifier: &str) -> String {
    hex(&Sha256::digest(verifier.as_bytes()))
}

/// `https://host` for whatever was typed or pasted: a bare host, a trailing slash, the
/// `wss://…/relay` address the PC prints, or an `/api/connect` address. Blank stays blank.
pub fn normalize(typed: &str) -> String {
    let mut s = typed.trim().trim_end_matches('/').to_string();
    if s.is_empty() {
        return s;
    }
    let lower = s.to_ascii_lowercase();
    s = if lower.starts_with("wss://") {
        format!("https://{}", &s[6..])
    } else if lower.starts_with("ws://") {
        format!("http://{}", &s[5..])
    } else if lower.starts_with("https://") || lower.starts_with("http://") {
        s
    } else {
        format!("https://{s}")
    };
    for suffix in ["/relay", "/api/connect", "/api/tally", "/dashboard/devices", "/dashboard"] {
        if s.to_ascii_lowercase().ends_with(suffix) {
            let cut = s.len() - suffix.len();
            return s[..cut].trim_end_matches('/').to_string();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_way_of_typing_the_server_is_the_same_server() {
        for typed in ["example.org", "https://example.org/", "wss://example.org/relay", "example.org/api/connect", "HTTPS://example.org/dashboard"] {
            let n = normalize(typed);
            assert!(n.eq_ignore_ascii_case("https://example.org"), "{typed} -> {n}");
        }
        assert_eq!(normalize("  "), "");
    }

    #[test]
    fn replies_read() {
        assert_eq!(read_reply(&json!({"status": "ready", "link": "relay://pair?v=1"}), 200), Reply::Ready("relay://pair?v=1".into()));
        assert_eq!(read_reply(&json!({"status": "ready", "link": "https://x"}), 200), Reply::Trouble("Your server's link did not read".into()));
        assert_eq!(read_reply(&json!({"status": "waiting_pc"}), 200), Reply::WaitingPc);
    }
}
