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
    /// The door's own addresses (`ws://` on its network or a tailnet, or `wss://`).
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

/// Whether `url` is encrypted: `wss://` or `https://`.
pub fn is_secure(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("wss") || scheme.eq_ignore_ascii_case("https"))
}

/// Whether Tally RS may reach `url`: always over TLS; in the clear (`ws://`, `http://`) only when
/// its host is an address on this machine or a private network: loopback, RFC 1918, link-local,
/// the 100.64/10 block Tailscale gives its tailnet (WireGuard under it), IPv6 unique-local and
/// link-local, or `localhost`. A name is refused in the clear: a foreign network's DNS answers for
/// it. The door's proof is not bound to the connection, so whoever relays a clear one could
/// replay it; the Kotlin app refuses clear traffic altogether (network_security_config.xml).
pub fn may_reach(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    if scheme.eq_ignore_ascii_case("wss") || scheme.eq_ignore_ascii_case("https") {
        return true;
    }
    if !(scheme.eq_ignore_ascii_case("ws") || scheme.eq_ignore_ascii_case("http")) {
        return false;
    }
    let Some(host) = host_of(rest) else { return false };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(a)) => private_v4(a),
        Ok(std::net::IpAddr::V6(a)) => {
            let first = a.segments()[0];
            a.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80 || a.to_ipv4_mapped().is_some_and(private_v4)
        }
        Err(_) => false,
    }
}

/// The host of an address's `authority…` part, brackets off. None when a user part
/// ("ws://10.0.0.2@elsewhere") hides where it really goes, or a bracket is left open.
fn host_of(rest: &str) -> Option<&str> {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return None;
    }
    match authority.strip_prefix('[') {
        Some(h) => h.split_once(']').map(|(h, _)| h),
        None => Some(authority.rsplit_once(':').map_or(authority, |(h, _)| h)),
    }
}

/// Whether `url` reaches this machine itself: `localhost`, 127/8, `::1`. Nothing between the
/// phone and its network can answer there.
pub fn is_loopback(url: &str) -> bool {
    let Some(host) = url.split_once("://").and_then(|(_, rest)| host_of(rest)) else { return false };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::IpAddr>().is_ok_and(|a| match a {
            std::net::IpAddr::V4(a) => a.is_loopback(),
            std::net::IpAddr::V6(a) => a.is_loopback() || a.to_ipv4_mapped().is_some_and(|a| a.is_loopback()),
        })
}

fn private_v4(a: std::net::Ipv4Addr) -> bool {
    let [x, y, ..] = a.octets();
    a.is_loopback() || a.is_private() || a.is_link_local() || (x == 100 && (64..128).contains(&y))
}

/// `routes` in the order to try them, encrypted first, and without the clear ones
/// [`may_reach`] refuses. When there is an encrypted one, a clear one is no fallback but this
/// machine's own: the door never proves it knows this phone's key, so whatever answers at a
/// private address on the network the phone is on (a hostile Wi-Fi's gateway that blocks the
/// encrypted route) would be sent the ledger's changes and believed. Clear routes to a private
/// network are used only by a door that offers nothing encrypted.
pub fn ordered_routes(routes: &[String]) -> Vec<String> {
    let (secure, clear): (Vec<&String>, Vec<&String>) = routes.iter().filter(|r| may_reach(r)).partition(|r| is_secure(r));
    let clear = clear.into_iter().filter(|r| secure.is_empty() || is_loopback(r));
    secure.iter().copied().chain(clear).cloned().collect()
}

impl PairLink {
    /// Where to reach the door, in the order to try: [`ordered_routes`] of its own addresses and
    /// the rendezvous.
    pub fn routes(&self) -> Vec<String> {
        let all: Vec<String> = self.direct.iter().cloned().chain(self.via.clone()).collect();
        ordered_routes(&all)
    }

    /// A `relay://pair` link, or None for anything else, so a stray paste is not misread.
    pub fn parse(text: &str) -> Option<PairLink> {
        let t = text.trim();
        let prefix = "relay://pair?";
        // `get`: the server's text may hold any character, and a slice inside one panics.
        if !t.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)) {
            return None;
        }
        let mut params = std::collections::HashMap::new();
        for pair in t.get(prefix.len()..).unwrap_or("").split('&').filter(|p| !p.is_empty()) {
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
        format!("https://{}", s.get(6..).unwrap_or(""))
    } else if lower.starts_with("ws://") {
        format!("http://{}", s.get(5..).unwrap_or(""))
    } else if lower.starts_with("https://") || lower.starts_with("http://") {
        s
    } else {
        format!("https://{s}")
    };
    // The scheme and the host are the same server in any case ("HTTPS://Anthobot.bid" is
    // "https://anthobot.bid"), so they are kept in one case and compare equal; a path keeps its own.
    let host_end = s.find("://").map_or(0, |i| i + 3);
    let path_at = s[host_end..].find('/').map_or(s.len(), |i| host_end + i);
    s = format!("{}{}", s[..path_at].to_ascii_lowercase(), &s[path_at..]);
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
        assert_eq!(l.routes(), vec!["wss://s.example/join/r1".to_string()], "encrypted, with no clear fallback beside it");
        let lan = PairLink::parse("relay://pair?v=1&code=A&direct=ws%3A%2F%2F10.0.0.2%3A7420").unwrap();
        assert_eq!(lan.routes(), vec!["ws://10.0.0.2:7420".to_string()], "a door with nothing encrypted is reached in the clear");
        assert_eq!(l.instance, "stable");
        assert!(PairLink::parse("relay://pair?v=2&code=A&via=wss://x").is_none());
        assert!(PairLink::parse("relay://pair?v=1&via=wss://x").is_none());
        assert!(PairLink::parse("relay://pair?v=1&code=A").is_none(), "nowhere to go");
        assert!(PairLink::parse("https://example.com").is_none());
        // Text that is not ASCII where the prefix would end reads as no link, never a panic.
        for odd in ["relay://pairé?v=1", "relay://pai\u{e9}", "é", "relay://pair?v=1&code=é&via=wss://x/é", "\u{1F600}\u{1F600}\u{1F600}\u{1F600}"] {
            let _ = PairLink::parse(odd);
        }
        assert!(PairLink::parse("relay://pairé?v=1&code=A&via=wss://x").is_none());
    }

    #[test]
    fn the_clear_is_only_for_this_machine_and_private_networks() {
        for ok in [
            "wss://anywhere.example/join/r",
            "https://anthobot.bid",
            "ws://10.0.0.2:7420",
            "ws://192.168.1.20:7420",
            "ws://172.16.4.1",
            "ws://127.0.0.1:7430",
            "ws://169.254.3.3",
            "ws://100.74.208.9:7420",
            "ws://[::1]:7420",
            "ws://[fd7a:115c:a1e0::1]:7420",
            "ws://[fe80::1]",
            "http://10.0.2.2:3000",
            "http://localhost:3901",
        ] {
            assert!(may_reach(ok), "{ok}");
        }
        for no in [
            "ws://8.8.8.8:7420",
            "ws://relay.example.org/join/room",
            "ws://pc.tail.ts.net:7420",
            "http://anthobot.bid",
            "ws://10.0.0.2@evil.example",
            "ws://100.128.0.1",
            "ws://[2001:db8::1]",
            "ftp://10.0.0.2",
            "10.0.0.2",
        ] {
            assert!(!may_reach(no), "{no}");
        }
        let routes = ["ws://8.8.8.8:7420", "ws://192.168.1.20:7420", "wss://s.example/join/r"].map(String::from);
        assert_eq!(ordered_routes(&routes), vec!["wss://s.example/join/r".to_string()], "no clear fallback beside an encrypted route");
        let routes = ["ws://8.8.8.8:7420", "ws://192.168.1.20:7420", "ws://100.74.208.9:7420"].map(String::from);
        assert_eq!(ordered_routes(&routes), vec!["ws://192.168.1.20:7420".to_string(), "ws://100.74.208.9:7420".to_string()]);
        let routes = ["ws://127.0.0.1:7430", "wss://s.example/join/r", "ws://[::1]:7430", "ws://localhost:1"].map(String::from);
        assert_eq!(ordered_routes(&routes), ["wss://s.example/join/r", "ws://127.0.0.1:7430", "ws://[::1]:7430", "ws://localhost:1"].map(String::from).to_vec(), "this machine's own stays");
        assert!(!is_loopback("ws://127.0.0.1@10.0.0.2") && !is_loopback("ws://10.0.0.2") && is_loopback("http://127.0.0.2:80/x"));
    }

    #[test]
    fn a_server_address_reads_however_it_was_typed() {
        assert_eq!(normalize("antho.example.com/"), "https://antho.example.com");
        assert_eq!(normalize("wss://antho.example.com/relay"), "https://antho.example.com");
        assert_eq!(normalize("http://10.0.0.2:8080/api/tally"), "http://10.0.0.2:8080");
        // One server in any case: the scheme and host are lowered, a path is kept as typed.
        assert_eq!(normalize("HTTPS://Anthobot.bid"), normalize("anthobot.bid"));
        assert_eq!(normalize("WSS://Anthobot.BID/relay"), "https://anthobot.bid");
        assert_eq!(normalize("Anthobot.bid/Tally"), "https://anthobot.bid/Tally");
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
