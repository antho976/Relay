//! The phone door's handshake (crates/relay-remote/src/wire.rs) and the bus envelopes it carries
//! afterwards (crates/relay-bus/src/envelope.rs, docs/engine/BUS.md §1): one JSON object per line.
//! A port of the Kotlin app's `Wire.kt`, so both phones say the same thing to the same door.
//!
//! Requests are strict on the PC (`deny_unknown_fields`): nothing here adds a field the engine
//! does not know. What the PC sends is read leniently, so a newer engine's extra fields never
//! break an older phone.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub const V: u64 = 1;
pub const DEFAULT_PORT: u16 = 7420;
/// The op list a PC's door opens, as its welcome names it (`OpProfile` in wire.rs).
pub const OPS_PC: &str = "phone";

/// `sha256(challenge:token)` in lower-case hex: what a paired phone shows instead of its token.
pub fn proof(challenge: &str, token: &str) -> String {
    hex(&Sha256::digest(format!("{challenge}:{token}").as_bytes()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 random bytes as hex: request ids, the server's verifier.
pub fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("the OS has randomness");
    hex(&b)
}

/// A request id: a version-4 UUID in its usual hyphenated form. The engine reads ids as UUIDs and
/// answers with this form, so an id written any other way would never match its answer.
pub fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS has randomness");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

pub fn pair_hello(code: &str, device_name: &str) -> String {
    let name: String = device_name.chars().take(64).collect();
    json!({"v": V, "pair": code.trim(), "device_name": name}).to_string()
}

pub fn proof_hello(device: &str, challenge: &str, token: &str) -> String {
    json!({"v": V, "device": device, "proof": proof(challenge, token)}).to_string()
}

pub fn request(op: &str, payload: &Value, id: &str) -> String {
    json!({"v": V, "id": id, "actor": "user", "op": op, "payload": payload}).to_string()
}

fn parse(line: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(o)) => Some(o),
        _ => None,
    }
}

fn s(o: &Map<String, Value>, k: &str) -> Option<String> {
    o.get(k).and_then(Value::as_str).map(str::to_string)
}

/// The door's first line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Greeting {
    pub host: String,
    pub host_id: String,
    pub instance: String,
    pub version: String,
    pub challenge: String,
    /// `running`, `starting` or `stopped` when the door fronts an engine it can start.
    pub engine: Option<String>,
    /// The op list, once the welcome said (`phone` or `cloud`); None from an older door.
    pub ops: Option<String>,
}

impl Greeting {
    /// A door this new names the PC in the welcome, not the greeting; an older one in the greeting.
    pub fn admitted_by(&self, w: &Welcome) -> Greeting {
        let mut g = self.clone();
        if let Some(h) = &w.host {
            g.host = h.clone();
        }
        if let Some(h) = &w.host_id {
            g.host_id = h.clone();
        }
        if let Some(v) = &w.version {
            g.version = v.clone();
        }
        if w.ops.is_some() {
            g.ops = w.ops.clone();
        }
        g
    }
}

/// The greeting the door sends first; None when the line is not one.
pub fn greeting(line: &str) -> Option<Greeting> {
    let o = parse(line)?;
    if o.get("relay").and_then(Value::as_str) != Some("remote") {
        return None;
    }
    Some(Greeting {
        host: s(&o, "host").unwrap_or_default(),
        host_id: s(&o, "host_id").unwrap_or_default(),
        instance: s(&o, "instance").unwrap_or_default(),
        version: s(&o, "version").unwrap_or_default(),
        challenge: s(&o, "challenge")?,
        engine: s(&o, "engine"),
        ops: None,
    })
}

/// A rendezvous with no PC behind it answers `{ok:false, error:"host.offline"}` in place of a greeting.
pub fn offline(line: &str) -> Option<String> {
    let o = parse(line)?;
    (o.get("ok").and_then(Value::as_bool) == Some(false)).then(|| s(&o, "error").unwrap_or_else(|| "host.offline".into()))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Welcome {
    pub ok: bool,
    pub device: Option<String>,
    pub token: Option<String>,
    pub error: Option<String>,
    pub host: Option<String>,
    pub host_id: Option<String>,
    pub version: Option<String>,
    pub ops: Option<String>,
}

pub fn welcome(line: &str) -> Option<Welcome> {
    let o = parse(line)?;
    Some(Welcome {
        ok: o.get("ok").and_then(Value::as_bool)?,
        device: s(&o, "device"),
        token: s(&o, "token"),
        error: s(&o, "error"),
        host: s(&o, "host"),
        host_id: s(&o, "host_id"),
        version: s(&o, "version"),
        ops: s(&o, "ops"),
    })
}

/// BUS.md §1.2. `kind` is one of invalid, not_found, conflict, refused, held, unavailable, internal.
#[derive(Clone, Debug, PartialEq)]
pub struct BusError {
    pub kind: String,
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
}

impl BusError {
    pub fn link(code: &str, message: &str) -> BusError {
        BusError { kind: "unavailable".into(), code: code.into(), message: message.into(), hint: None }
    }

    fn from(o: &Map<String, Value>) -> BusError {
        let code = s(o, "code").unwrap_or_else(|| "unknown".into());
        // The door's own wording ("… is not open to a paired phone") reads as a fault; it means
        // the PC runs a Relay older than this app.
        let message = if code == "remote.op" { UPDATE_PC.into() } else { s(o, "message").unwrap_or_default() };
        BusError { kind: s(o, "kind").unwrap_or_else(|| "internal".into()), code, message, hint: s(o, "hint") }
    }

    /// Worth sending again later: the link was down or slow, or the PC busy for a moment.
    pub fn transient(&self) -> bool {
        self.code.starts_with("link.")
            || self.code == "app.quitting"
            || (self.kind == "unavailable" && (self.code.ends_with(".busy") || self.code.ends_with(".rate_limited")))
    }
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

pub const UPDATE_PC: &str = "Your PC runs an older Relay that does not open this to a phone yet. Update Relay on the PC and restart it to use it here.";

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub id: Option<String>,
    pub ok: bool,
    pub result: Value,
    pub error: Option<BusError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub ev: String,
    pub ts: String,
    pub project_id: Option<i64>,
    pub payload: Value,
}

/// What a line from the engine is, told apart by the key that follows `v` (envelope.rs).
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    Res(Response),
    Ev(Event),
    /// A data-plane frame (`pty`): this app shows no terminal, so it only skips them.
    Frame,
}

pub fn classify(line: &str) -> Option<Incoming> {
    let o = parse(line)?;
    if o.contains_key("ev") {
        return Some(Incoming::Ev(Event {
            ev: s(&o, "ev").unwrap_or_default(),
            ts: s(&o, "ts").unwrap_or_default(),
            project_id: o.get("project_id").and_then(Value::as_i64),
            payload: o.get("payload").cloned().unwrap_or(Value::Object(Map::new())),
        }));
    }
    if o.contains_key("stream") {
        return Some(Incoming::Frame);
    }
    if o.contains_key("ok") {
        return Some(Incoming::Res(Response {
            id: s(&o, "id"),
            ok: o.get("ok").and_then(Value::as_bool) == Some(true),
            result: o.get("result").cloned().unwrap_or(Value::Null),
            error: o.get("error").and_then(Value::as_object).map(BusError::from),
        }));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_proof_is_the_hex_sha256_of_challenge_colon_token() {
        // printf 'abc:def' | sha256sum
        assert_eq!(proof("abc", "def"), "ec5952851b8051e1ecf6b6076d99d05646cd90a9f293c17250105742b9e4a19e");
    }

    #[test]
    fn a_greeting_needs_relay_remote_and_a_challenge() {
        let g = greeting(r#"{"relay":"remote","v":1,"challenge":"c1","instance":"stable","engine":"running","host":"","host_id":"","version":""}"#).unwrap();
        assert_eq!(g.challenge, "c1");
        assert_eq!(g.engine.as_deref(), Some("running"));
        assert!(greeting(r#"{"relay":"other","challenge":"c"}"#).is_none());
        assert!(greeting(r#"{"relay":"remote"}"#).is_none());
        assert_eq!(offline(r#"{"ok":false,"error":"host.offline"}"#).as_deref(), Some("host.offline"));
    }

    #[test]
    fn the_welcome_names_the_pc() {
        let w = welcome(r#"{"ok":true,"host":"desk","host_id":"h1","version":"0.9","ops":"phone","extra":1}"#).unwrap();
        let g = Greeting { challenge: "c".into(), ..Default::default() }.admitted_by(&w);
        assert_eq!((g.host.as_str(), g.host_id.as_str(), g.ops.as_deref()), ("desk", "h1", Some("phone")));
    }

    #[test]
    fn lines_are_told_apart() {
        assert!(matches!(classify(r#"{"v":1,"ev":"session.updated","ts":"t","actor":"system","payload":{}}"#), Some(Incoming::Ev(_))));
        assert!(matches!(classify(r#"{"v":1,"stream":"pty","seq":1,"data":"x"}"#), Some(Incoming::Frame)));
        let Some(Incoming::Res(r)) = classify(r#"{"v":1,"id":"a","ok":false,"error":{"kind":"refused","code":"remote.op","message":"no"}}"#) else {
            panic!()
        };
        assert_eq!(r.error.unwrap().message, UPDATE_PC);
        assert!(classify("not json").is_none());
    }

    #[test]
    fn ids_are_uuids_as_the_engine_writes_them() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        let parts: Vec<_> = id.split('-').map(str::len).collect();
        assert_eq!(parts, [8, 4, 4, 4, 12]);
        assert_eq!(&id[14..15], "4");
        assert_ne!(uuid_v4(), id);
    }

    #[test]
    fn requests_carry_only_what_the_engine_knows() {
        let line = request("bus.ping", &json!({}), "id1");
        let v: Value = serde_json::from_str(&line).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["actor", "id", "op", "payload", "v"]);
        assert_eq!(v["actor"], "user");
    }
}
