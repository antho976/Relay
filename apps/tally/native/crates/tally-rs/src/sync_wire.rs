//! Relay's phone door, wire v1, and the owner's server's request to connect, as plain functions
//! (Kotlin: link/Wire.kt and the pure half of link/Cloud.kt).
//!
//! The door greets with a challenge; the phone answers with a pairing code once, or a proof every
//! time after; the door admits it; then each line is one bus request or response. Connecting asks
//! the owner's server (antho-s-bot's `/api/tally`) to let this phone in: the phone makes a random
//! verifier and sends only its sha256, the owner taps Allow, and the phone collects a
//! `relay://pair` link for Relay's cloud copy.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub const V: u64 = 1;

/// The port `relay remote serve` listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7420;

/// The server this build talks to unless the person types another (Kotlin: `BuildConfig.TALLY_SERVER`,
/// set in app/build.gradle.kts). `TALLY_SERVER` at build time names another.
pub const DEFAULT_SERVER: &str = match option_env!("TALLY_SERVER") {
    Some(s) => s,
    None => "https://anthobot.bid",
};

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub fn sha256_hex(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// What a phone sends instead of its token: sha256 of "challenge:token", lowercase hex.
pub fn proof(challenge: &str, token: &str) -> String {
    sha256_hex(&format!("{challenge}:{token}"))
}

/// Pairing: the code, and the name the door lists this phone under.
pub fn pair_hello(code: &str, device_name: &str) -> String {
    json!({"v": V, "pair": code.trim(), "device_name": device_name}).to_string()
}

/// Every connection after pairing: the device id and a proof good for this challenge only.
pub fn proof_hello(device: &str, challenge: &str, token: &str) -> String {
    json!({"v": V, "device": device, "proof": proof(challenge, token)}).to_string()
}

/// A bus request as the phone sends it: always the user, a fresh id per request.
pub fn request(op: &str, payload: &Value, id: &str) -> String {
    json!({"v": V, "id": id, "actor": "user", "op": op, "payload": payload}).to_string()
}

fn parse(line: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str(line) {
        Ok(Value::Object(o)) => Some(o),
        _ => None,
    }
}

fn s(o: &Map<String, Value>, key: &str) -> Option<String> {
    o.get(key).and_then(Value::as_str).map(str::to_string)
}

fn b(o: &Map<String, Value>, key: &str) -> Option<bool> {
    o.get(key).and_then(Value::as_bool)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Greeting {
    pub host: String,
    pub host_id: String,
    pub instance: String,
    pub challenge: String,
    /// Set when the route answered but there is no Relay behind it (`host.offline`).
    pub refusal: Option<String>,
}

/// The door's first line, or None for anything that is not one.
pub fn greeting(line: &str) -> Option<Greeting> {
    let o = parse(line)?;
    if b(&o, "ok") == Some(false) {
        return Some(Greeting { refusal: Some(s(&o, "error").unwrap_or_else(|| "refused".into())), ..Default::default() });
    }
    if s(&o, "relay").as_deref() != Some("remote") {
        return None;
    }
    Some(Greeting {
        host: s(&o, "host").unwrap_or_default(),
        host_id: s(&o, "host_id").unwrap_or_default(),
        instance: s(&o, "instance").unwrap_or_default(),
        challenge: s(&o, "challenge")?,
        refusal: None,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Welcome {
    pub ok: bool,
    pub device: Option<String>,
    pub token: Option<String>,
    pub error: Option<String>,
    /// Who the phone reached (newer doors say it here, not in the greeting).
    pub host: Option<String>,
}

pub fn welcome(line: &str) -> Option<Welcome> {
    let o = parse(line)?;
    Some(Welcome { ok: b(&o, "ok")?, device: s(&o, "device"), token: s(&o, "token"), error: s(&o, "error"), host: s(&o, "host") })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub ok: bool,
    pub result: Value,
    pub code: Option<String>,
    pub message: Option<String>,
}

/// The response to request `id`, or None when `line` is anything else (an event, a frame).
pub fn response(line: &str, id: &str) -> Option<Response> {
    let o = parse(line)?;
    if s(&o, "id").as_deref() != Some(id) {
        return None;
    }
    let ok = b(&o, "ok")?;
    let error = o.get("error").and_then(Value::as_object);
    Some(Response {
        ok,
        result: o.get("result").cloned().unwrap_or(Value::Null),
        code: error.and_then(|e| s(e, "code")),
        message: error.and_then(|e| s(e, "message")),
    })
}

/// Where a door can be reached and the code that pairs with it: the `relay://pair?…` link.
#[derive(Clone, Debug, PartialEq)]
pub struct PairLink {
    pub code: String,
    pub host: String,
    pub host_id: String,
    pub instance: String,
    /// `ws://…` addresses tried first, then the rendezvous `via`.
    pub direct: Vec<String>,
    pub via: Option<String>,
}

fn is_ws(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    (l.starts_with("ws://") && l.len() > 5) || (l.starts_with("wss://") && l.len() > 6)
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

impl PairLink {
    pub fn routes(&self) -> Vec<String> {
        self.direct.iter().cloned().chain(self.via.clone()).collect()
    }

    /// A `relay://pair` link, or None for anything else, so a stray paste is not misread.
    pub fn parse(text: &str) -> Option<PairLink> {
        let t = text.trim();
        let prefix = "relay://pair?";
        if t.len() < prefix.len() || !t[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return None;
        }
        let mut params = std::collections::HashMap::new();
        for pair in t[prefix.len()..].split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            params.insert(decode(k), decode(v));
        }
        let code = params.get("code").filter(|c| !c.trim().is_empty())?.clone();
        if params.get("v").map(String::as_str) != Some("1") {
            return None;
        }
        let direct: Vec<String> = params.get("direct").map(|d| d.split(',').map(str::trim).filter(|r| is_ws(r)).map(str::to_string).collect()).unwrap_or_default();
        let via = params.get("via").filter(|v| is_ws(v)).cloned();
        if direct.is_empty() && via.is_none() {
            return None;
        }
        let nonblank = |k: &str, d: &str| params.get(k).filter(|v| !v.trim().is_empty()).cloned().unwrap_or_else(|| d.to_string());
        Some(PairLink { code, host: nonblank("host", "Relay PC"), host_id: nonblank("id", ""), instance: nonblank("instance", "stable"), direct, via })
    }
}

// The request to connect (Cloud.kt).

/// `https://host` for whatever was typed: a bare host, a trailing slash, a wss:// address, a
/// pasted Relay or API address of the same server.
pub fn normalize(server: &str) -> String {
    let mut s = server.trim().trim_end_matches('/').to_string();
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
    for suffix in ["/relay", "/api/tally"] {
        if let Some(t) = s.strip_suffix(suffix) {
            s = t.to_string();
        }
    }
    s.trim_end_matches('/').to_string()
}

pub fn api(server: &str) -> String {
    normalize(server) + "/api/tally"
}

/// 32 random bytes as lowercase hex: what only this phone knows.
pub fn new_verifier() -> String {
    let mut bytes = [0u8; 32];
    if getrandom::fill(&mut bytes).is_err() {
        // No system randomness: two v4 uuids (themselves random) rather than nothing.
        return format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    }
    hex(&bytes)
}

pub fn challenge_of(verifier: &str) -> String {
    sha256_hex(verifier)
}

/// The four characters the phone and the owner's DM both show.
pub fn code_of(challenge: &str) -> String {
    challenge.chars().take(4).collect::<String>().to_uppercase()
}

/// A pairing code with room between its characters, as the bot prints it: "K74Q9D" reads
/// "K7 · 4Q · 9D"; anything else as it is.
pub fn spaced_code(code: &str) -> String {
    if code.chars().count() == 6 && !code.chars().any(char::is_whitespace) {
        let c: Vec<char> = code.chars().collect();
        c.chunks(2).map(|p| p.iter().collect::<String>()).collect::<Vec<_>>().join(" · ")
    } else {
        code.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proof_is_the_sha256_of_challenge_and_token() {
        // sha256("abc:xyz")
        assert_eq!(proof("abc", "xyz"), sha256_hex("abc:xyz"));
        assert_eq!(sha256_hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let hello: Value = serde_json::from_str(&proof_hello("d1", "abc", "xyz")).unwrap();
        assert_eq!(hello, json!({"v": 1, "device": "d1", "proof": proof("abc", "xyz")}));
        assert!(!proof_hello("d1", "abc", "xyz").contains("xyz\""), "the token itself never goes");
    }

    #[test]
    fn the_door_lines_read() {
        let g = greeting(r#"{"v":1,"relay":"remote","host":"","host_id":"","instance":"cloud","challenge":"c1"}"#).unwrap();
        assert_eq!((g.challenge.as_str(), g.instance.as_str(), g.refusal), ("c1", "cloud", None));
        assert_eq!(greeting(r#"{"ok":false,"error":"host.offline"}"#).unwrap().refusal.as_deref(), Some("host.offline"));
        assert_eq!(greeting(r#"{"hello":"world"}"#), None);
        assert_eq!(greeting("not json"), None);
        let w = welcome(r#"{"v":1,"ok":true,"device":"d1","token":"t1","host":"Cloud"}"#).unwrap();
        assert_eq!((w.ok, w.device.as_deref(), w.token.as_deref(), w.host.as_deref()), (true, Some("d1"), Some("t1"), Some("Cloud")));
        let r = response(r#"{"id":"x","ok":false,"error":{"code":"money.sync_stale","message":"m"}}"#, "x").unwrap();
        assert_eq!((r.ok, r.code.as_deref()), (false, Some("money.sync_stale")));
        assert_eq!(response(r#"{"id":"y","ok":true,"result":{}}"#, "x"), None, "another request's answer");
        let req: Value = serde_json::from_str(&request("money.sync", &json!({"a": 1}), "id1")).unwrap();
        assert_eq!(req, json!({"v": 1, "id": "id1", "actor": "user", "op": "money.sync", "payload": {"a": 1}}));
    }

    #[test]
    fn a_pair_link_reads_and_a_stray_paste_does_not() {
        let l = PairLink::parse("relay://pair?v=1&code=AB%2012&host=Cloud&direct=ws%3A%2F%2F10.0.0.2%3A7420,http://no&via=wss://s.example/join/r1").unwrap();
        assert_eq!(l.code, "AB 12");
        assert_eq!(l.host, "Cloud");
        assert_eq!(l.routes(), vec!["ws://10.0.0.2:7420".to_string(), "wss://s.example/join/r1".to_string()]);
        assert_eq!(l.instance, "stable");
        assert!(PairLink::parse("relay://pair?v=2&code=A&via=wss://x").is_none());
        assert!(PairLink::parse("relay://pair?v=1&via=wss://x").is_none());
        assert!(PairLink::parse("relay://pair?v=1&code=A").is_none(), "nowhere to go");
        assert!(PairLink::parse("https://example.com").is_none());
    }

    #[test]
    fn a_server_address_reads_however_it_was_typed() {
        assert_eq!(normalize("antho.example.com/"), "https://antho.example.com");
        assert_eq!(normalize("wss://antho.example.com/relay"), "https://antho.example.com");
        assert_eq!(normalize("http://10.0.0.2:8080/api/tally"), "http://10.0.0.2:8080");
        assert_eq!(api("https://a.b"), "https://a.b/api/tally");
        assert!(DEFAULT_SERVER.starts_with("https://"));
    }

    #[test]
    fn the_code_is_the_challenges_first_four() {
        let v = new_verifier();
        assert_eq!(v.len(), 64);
        assert_ne!(v, new_verifier());
        let c = challenge_of(&v);
        assert_eq!(code_of(&c), c[..4].to_uppercase());
        assert_eq!(spaced_code("K74Q9D"), "K7 · 4Q · 9D");
        assert_eq!(spaced_code("7F3A"), "7F3A");
    }
}
