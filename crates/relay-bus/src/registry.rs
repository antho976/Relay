//! The op registry (BUS.md §3). Every op is a type implementing [`Op`]; the registry is the
//! flat list the schema, the doors and `bus.ops` all read.

use crate::envelope::Actor;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    Mutation,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Audit {
    Always,
    AgentOnly,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Undo {
    None,
    Inverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Global,
    Project,
    Session,
}

/// Default allow-set before per-session allowlists (BUS.md §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Actors {
    All,
    /// `user` and `test` (and `system`).
    UserOnly,
    /// Everyone may call it, but the op exists *for* agents (self-report style).
    AgentOnly,
}

impl Actors {
    pub fn admits(self, actor: &Actor) -> bool {
        match self {
            Actors::All | Actors::AgentOnly => true,
            Actors::UserOnly => actor.is_privileged(),
        }
    }
}

/// Whether the asking actor may really call an op, across all three gating layers
/// (registry `actors`, runtime row scope, and the role allowlist). `bus.ops` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Callable {
    /// Callable as asked.
    Yes,
    /// Refused before the handler runs.
    No,
    /// Callable, but only for the actor's own session (or its PAIR partner, for reads).
    SelfOnly,
}

/// Which doors carry an op. A `socket_only` op acts on the connection that sends it (a
/// subscriber, a wait), so the socket door answers it; the engine refuses it with `bus.door`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Doors {
    All,
    SocketOnly,
}

/// Static attributes of an op (BUS.md §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct OpMeta {
    pub kind: OpKind,
    pub audit: Audit,
    pub undo: Undo,
    pub scope: Scope,
    pub actors: Actors,
    pub doors: Doors,
    /// Event names this op can emit.
    pub emits: &'static [&'static str],
    /// Data-plane stream this op attaches, if any (BUS.md §7).
    pub stream: Option<&'static str>,
    /// Schema version the op appeared in.
    pub since: &'static str,
    /// SPEC §17 build-order phase that implements it. `bus.ops` shows it; `not_implemented`
    /// errors carry it.
    pub phase: u8,
    /// One line, for `bus.ops` and MCP tool descriptions.
    pub summary: &'static str,
    pub deprecated: Option<&'static str>,
}

impl OpMeta {
    /// A mutation with the common defaults: audited always, no undo, all actors, all doors.
    pub const fn mutation(scope: Scope, phase: u8, summary: &'static str) -> Self {
        OpMeta {
            kind: OpKind::Mutation,
            audit: Audit::Always,
            undo: Undo::None,
            scope,
            actors: Actors::All,
            doors: Doors::All,
            emits: &[],
            stream: None,
            since: "1.0",
            phase,
            summary,
            deprecated: None,
        }
    }
    /// A query with the common defaults: never audited, all actors, all doors.
    pub const fn query(scope: Scope, phase: u8, summary: &'static str) -> Self {
        OpMeta {
            kind: OpKind::Query,
            audit: Audit::Never,
            undo: Undo::None,
            scope,
            actors: Actors::All,
            doors: Doors::All,
            emits: &[],
            stream: None,
            since: "1.0",
            phase,
            summary,
            deprecated: None,
        }
    }
    pub const fn audit(mut self, a: Audit) -> Self {
        self.audit = a;
        self
    }
    pub const fn undo(mut self, u: Undo) -> Self {
        self.undo = u;
        self
    }
    pub const fn actors(mut self, a: Actors) -> Self {
        self.actors = a;
        self
    }
    pub const fn doors(mut self, d: Doors) -> Self {
        self.doors = d;
        self
    }
    pub const fn emits(mut self, e: &'static [&'static str]) -> Self {
        self.emits = e;
        self
    }
    pub const fn stream(mut self, s: &'static str) -> Self {
        self.stream = Some(s);
        self
    }
    pub const fn deprecated(mut self, why: &'static str) -> Self {
        self.deprecated = Some(why);
        self
    }
}

/// An op is a type: name + meta + payload + result. Handlers in `relay-core` are keyed by
/// `Op::NAME`; the schema is rendered from `Payload` and `Result`.
pub trait Op: 'static {
    const NAME: &'static str;
    const META: OpMeta;
    type Payload: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static;
    type Result: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static;
}

/// An op's explanation of a payload its `validate` refused: `bus.schema`'s reason and `details`.
pub type Locate = fn(&serde_json::Value) -> Option<(String, serde_json::Value)>;

/// A type-erased registry row.
#[derive(Clone)]
pub struct OpEntry {
    pub name: &'static str,
    pub meta: OpMeta,
    pub payload_schema: fn(&mut SchemaGenerator) -> Schema,
    pub result_schema: fn(&mut SchemaGenerator) -> Schema,
    /// Typed validation of a payload (BUS.md §5.2) — available for every op, implemented or
    /// not, so an agent learns about a bad payload before it learns the op is not built yet.
    pub validate: fn(&serde_json::Value) -> Result<(), String>,
    /// Why and where a payload `validate` refused went wrong, for an op that can say better than
    /// serde: `bus.schema`'s reason and `details` (`design.apply`: the edit at fault, its key and
    /// advice). Runs on a refusal only, so the payloads that read pay nothing for it.
    pub locate: Option<Locate>,
}

impl OpEntry {
    pub fn of<O: Op>() -> Self {
        OpEntry {
            name: O::NAME,
            meta: O::META,
            payload_schema: |g| g.subschema_for::<O::Payload>(),
            result_schema: |g| g.subschema_for::<O::Result>(),
            // Deserialized *through* the value, not out of a clone of it: validation runs on
            // every request, implemented op or not, and cloning the payload to throw the copy
            // away doubled what a large `file.write` or `session.report` cost to check.
            validate: |v| serde::Deserialize::deserialize(v).map(|_: O::Payload| ()).map_err(|e: serde_json::Error| e.to_string()),
            locate: None,
        }
    }

    /// This entry, its `bus.schema` refusals explained by `locate`.
    pub fn located(mut self, locate: Locate) -> Self {
        self.locate = Some(locate);
        self
    }

    /// `payload` checked as the bus checks it: `bus.schema` when it does not read, explained
    /// and with `details` where the op can say where.
    pub fn check(&self, payload: &serde_json::Value) -> Result<(), crate::BusError> {
        (self.validate)(payload).map_err(|why| match self.locate.and_then(|locate| locate(payload)) {
            Some((why, details)) => crate::BusError::schema(self.name, why).with_details(details),
            None => crate::BusError::schema(self.name, why),
        })
    }
}

impl std::fmt::Debug for OpEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpEntry").field("name", &self.name).field("meta", &self.meta).finish()
    }
}

/// What `bus.ops` returns per op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpInfo {
    pub name: String,
    pub kind: OpKind,
    pub audit: Audit,
    pub undo: Undo,
    pub scope: Scope,
    pub actors: Actors,
    pub doors: Doors,
    pub emits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    pub since: String,
    pub phase: u8,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Whether the running engine has a handler for it (else `bus.not_implemented`).
    pub implemented: bool,
    /// All-layers verdict for the asking actor. `None` when nothing evaluated it (the static
    /// schema render), so a reader never mistakes "not computed" for "callable".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<Callable>,
    /// Which layer decided `call`, in one phrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

impl OpInfo {
    pub fn from_entry(e: &OpEntry, implemented: bool) -> Self {
        OpInfo {
            name: e.name.to_string(),
            kind: e.meta.kind,
            audit: e.meta.audit,
            undo: e.meta.undo,
            scope: e.meta.scope,
            actors: e.meta.actors,
            doors: e.meta.doors,
            emits: e.meta.emits.iter().map(|s| s.to_string()).collect(),
            stream: e.meta.stream.map(str::to_string),
            since: e.meta.since.to_string(),
            phase: e.meta.phase,
            summary: e.meta.summary.to_string(),
            deprecated: e.meta.deprecated.map(str::to_string),
            implemented,
            call: None,
            why: None,
        }
    }

    /// Record the all-layers verdict (BUS.md §9.1). `bus.ops` is the only honest place to
    /// answer "may I call this?", so it must answer for every layer or not at all.
    pub fn with_call(mut self, call: Callable, why: impl Into<String>) -> Self {
        self.call = Some(call);
        self.why = Some(why.into());
        self
    }
}

/// Per op: every payload field name, and the subset the schema marks required.
type PayloadShapes = HashMap<&'static str, (Vec<String>, Vec<String>)>;

/// The registry: every op, in a stable order (namespace, then declaration order).
pub struct Registry {
    entries: Vec<OpEntry>,
    by_name: HashMap<&'static str, usize>,
}

impl Registry {
    fn build() -> Self {
        let entries = crate::ops::all();
        let mut by_name = HashMap::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            let dup = by_name.insert(e.name, i);
            assert!(dup.is_none(), "duplicate op registered: {}", e.name);
            assert!(valid_name(e.name), "op name violates BUS.md §3.1: {}", e.name);
        }
        Registry { entries, by_name }
    }
    /// The process-wide registry.
    pub fn global() -> &'static Registry {
        static REG: OnceLock<Registry> = OnceLock::new();
        REG.get_or_init(Registry::build)
    }
    pub fn get(&self, name: &str) -> Option<&OpEntry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }
    pub fn entries(&self) -> &[OpEntry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every payload field name and every required one, per op. Built once, lazily: doors
    /// read it to fill in what an agent already carries in its environment rather than making
    /// it guess-and-retry (BUS.md §8.2). Only the global registry exists, so it is the one read.
    fn payload_shapes() -> &'static PayloadShapes {
        static SHAPES: OnceLock<PayloadShapes> = OnceLock::new();
        SHAPES.get_or_init(|| {
            let settings = schemars::generate::SchemaSettings::draft2020_12()
                .with(|s| s.inline_subschemas = true);
            let mut generator = settings.into_generator();
            Registry::global()
                .entries()
                .iter()
                .map(|entry| {
                    let schema = (entry.payload_schema)(&mut generator).to_value();
                    let fields = schema["properties"]
                        .as_object()
                        .map(|properties| properties.keys().cloned().collect())
                        .unwrap_or_default();
                    let required = schema["required"]
                        .as_array()
                        .map(|items| {
                            items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()
                        })
                        .unwrap_or_default();
                    (entry.name, (fields, required))
                })
                .collect()
        })
    }

    /// Does this op's payload have a field named `field`?
    pub fn payload_has(&self, op: &str, field: &str) -> bool {
        Self::payload_shapes()
            .get(op)
            .is_some_and(|(fields, _)| fields.iter().any(|name| name == field))
    }

    /// Is `field` required by this op's payload? Only a required field is safe to fill in
    /// from the caller's identity: an optional one is often half of an either/or.
    pub fn payload_requires(&self, op: &str, field: &str) -> bool {
        Self::payload_shapes()
            .get(op)
            .is_some_and(|(_, required)| required.iter().any(|name| name == field))
    }
}

/// `noun.verb` / `noun.sub.verb`, lower snake, 2–3 segments.
pub fn valid_name(name: &str) -> bool {
    let segs: Vec<&str> = name.split('.').collect();
    (2..=3).contains(&segs.len())
        && segs.iter().all(|s| {
            !s.is_empty()
                && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && !s.starts_with('_')
        })
}

/// Declare an op type. Usage:
/// ```ignore
/// op!(TaskGet, "task.get", TaskGetIn => Task, OpMeta::query(Scope::Project, 7, "One task by id"));
/// ```
#[macro_export]
macro_rules! op {
    ($ty:ident, $name:literal, $payload:ty => $result:ty, $meta:expr) => {
        #[doc = concat!("`", $name, "`")]
        pub struct $ty;
        impl $crate::registry::Op for $ty {
            const NAME: &'static str = $name;
            const META: $crate::registry::OpMeta = $meta;
            type Payload = $payload;
            type Result = $result;
        }
    };
}
