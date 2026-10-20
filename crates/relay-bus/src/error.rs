//! Typed errors (BUS.md §1.2). Agents react to `kind` and `code`, never to `message`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Invalid,
    NotFound,
    Conflict,
    Refused,
    Held,
    Unavailable,
    Internal,
}

impl ErrorKind {
    /// CLI exit code (BUS.md §8.2).
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorKind::Invalid | ErrorKind::NotFound | ErrorKind::Conflict | ErrorKind::Internal => 1,
            ErrorKind::Refused => 2,
            ErrorKind::Held => 3,
            ErrorKind::Unavailable => 4,
        }
    }
    /// The audit `kind` column for a request that ended with this error.
    pub fn audit_kind(self) -> &'static str {
        match self {
            ErrorKind::Held => "held",
            ErrorKind::Refused => "refused",
            _ => "error",
        }
    }
}

/// The op that lifts a hold (present iff `kind == held`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Confirm {
    pub op: String,
    pub payload: Value,
}

/// The most of a `bus.schema` refusal's reason kept, in bytes: the longest a `design.apply` edit
/// is explained in (an id of 64 characters included) fits whole, and a value echoed back does not
/// fill an agent's context.
pub const SCHEMA_WHY_MAX: usize = 300;

/// `why` as text, cut at `max` bytes on a character boundary with "…". Written into a string
/// that stops growing at `max`, so a 2 MB echo is never formatted whole.
fn clipped(why: &impl std::fmt::Display, max: usize) -> String {
    struct Clip {
        text: String,
        max: usize,
        cut: bool,
    }
    impl std::fmt::Write for Clip {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if self.cut {
                return Ok(());
            }
            if self.text.len() + s.len() <= self.max {
                self.text.push_str(s);
                return Ok(());
            }
            let mut end = self.max - self.text.len();
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&s[..end]);
            self.cut = true;
            Ok(())
        }
    }
    let mut clip = Clip { text: String::new(), max, cut: false };
    let _ = std::fmt::write(&mut clip, format_args!("{why}"));
    if clip.cut {
        clip.text.push('…');
    }
    clip.text
}

// No `deny_unknown_fields` here or on `Confirm`: clients ignore unknown fields (BUS.md §12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[error("{kind:?} {code}: {message}")]
pub struct BusError {
    pub kind: ErrorKind,
    /// Stable, dotted, e.g. `task.not_found`, `guardrail.destructive_write`.
    pub code: String,
    /// Human sentence; may change; never parse it.
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Confirm>,
}

impl BusError {
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> Self {
        BusError { kind, code: code.into(), message: message.into(), details: None, hint: None, confirm: None }
    }
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn with_confirm(mut self, op: impl Into<String>, payload: Value) -> Self {
        self.confirm = Some(Confirm { op: op.into(), payload });
        self
    }

    // ---- constructors for the codes every door and handler shares ----

    pub fn invalid(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid, code, message)
    }
    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, code, message)
    }
    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, code, message)
    }
    pub fn refused(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Refused, code, message)
    }
    pub fn held(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Held, code, message)
    }
    pub fn unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unavailable, code, message)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, "internal", message)
    }

    /// `invalid` / `bus.unknown_op`.
    pub fn unknown_op(op: &str) -> Self {
        Self::invalid("bus.unknown_op", format!("unknown op {op:?}"))
            .with_hint("call bus.ops to list what exists")
    }
    /// `invalid` / `bus.schema` — payload failed validation. The reason is clipped at
    /// [`SCHEMA_WHY_MAX`] bytes: serde echoes the value it could not read, and a list sent as one
    /// string came back whole, escaped twice (TM-6). The hint names the op an agent can call, and
    /// the command for a shell.
    pub fn schema(op: &str, why: impl std::fmt::Display) -> Self {
        Self::invalid("bus.schema", format!("payload for {op} does not match its schema: {}", clipped(&why, SCHEMA_WHY_MAX)))
            .with_hint(format!("bus.schema {{\"op\":\"{op}\"}} (CLI: relay schema {op})"))
    }
    /// `invalid` / `bus.actor`.
    pub fn actor(why: impl Into<String>) -> Self {
        Self::invalid("bus.actor", why)
    }
    /// `invalid` / `bus.parse` — not even JSON, or JSON with no readable `id`.
    pub fn parse(why: impl std::fmt::Display) -> Self {
        Self::invalid("bus.parse", format!("could not parse request: {why}"))
    }
    /// `invalid` / `bus.envelope`.
    pub fn envelope(why: impl std::fmt::Display) -> Self {
        Self::invalid("bus.envelope", format!("malformed envelope: {why}"))
    }
    /// `unavailable` / `bus.not_implemented` — registered, not yet built (BUS.md §15).
    pub fn not_implemented(op: &str, phase: u8) -> Self {
        Self::unavailable("bus.not_implemented", format!("{op} is registered but not implemented yet"))
            .with_details(serde_json::json!({ "phase": phase }))
    }
    /// `refused` / `actor.allowlist`.
    pub fn allowlist(op: &str, role: &str) -> Self {
        Self::refused("actor.allowlist", format!("role {role:?} may not call {op}"))
            .with_details(serde_json::json!({ "op": op, "role": role }))
    }
    /// `refused` / `actor.scope` — an agent reaching for something that isn't its own.
    pub fn not_own(what: &str) -> Self {
        Self::refused("actor.scope", format!("agents may only act on their own {what}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TM-6: a 40 KB list sent as one string was echoed back whole, escaped twice.
    #[test]
    fn a_schema_refusal_clips_what_it_echoes() {
        let list = format!("[{}]", "{\"op\": \"add\", \"node\": {\"kind\": \"rect\"}}, ".repeat(1000));
        assert!(list.len() > 40_000);
        let serde = serde_json::from_value::<Vec<u8>>(serde_json::Value::String(list)).unwrap_err();
        let error = BusError::schema("design.apply", serde);
        assert!(error.message.len() <= SCHEMA_WHY_MAX + 100, "{} bytes", error.message.len());
        assert!(error.message.starts_with("payload for design.apply does not match its schema: invalid type: string \"[{"));
        assert!(error.message.ends_with('…'));
        // A reason that fits is left whole.
        let short = BusError::schema("task.get", "missing field `task_id`");
        assert_eq!(short.message, "payload for task.get does not match its schema: missing field `task_id`");
    }

    #[test]
    fn clipping_keeps_whole_characters() {
        assert_eq!(clipped(&"abc", 3), "abc");
        assert_eq!(clipped(&"abcd", 3), "abc…");
        // "é" is two bytes: a cut inside it backs off to before it.
        assert_eq!(clipped(&"abé", 3), "ab…");
        assert_eq!(clipped(&format_args!("{}{}", "ab", "éé"), 5), "abé…");
        let long = "日本語".repeat(200);
        let cut = clipped(&long, SCHEMA_WHY_MAX);
        assert!(cut.len() <= SCHEMA_WHY_MAX + '…'.len_utf8() && cut.ends_with('…'));
    }

    /// E-agent-1: the hint names what an MCP agent can call, then the shell's command.
    #[test]
    fn the_schema_hint_names_the_op_first() {
        let error = BusError::schema("design.apply", "ops[0]: …");
        assert_eq!(error.hint.as_deref(), Some("bus.schema {\"op\":\"design.apply\"} (CLI: relay schema design.apply)"));
        let hint: serde_json::Value = serde_json::from_str(error.hint.unwrap().split(" (CLI").next().unwrap().strip_prefix("bus.schema ").unwrap()).unwrap();
        assert_eq!(hint, serde_json::json!({"op": "design.apply"}), "the payload is JSON an agent can send as it is");
    }
}
