use relay_bus::{Registry, Request, Actor, Response, BusError};
use relay_bus::registry::{valid_name, OpKind, Audit};

#[test]
fn registry_is_well_formed() {
    let reg = Registry::global();
    assert!(reg.len() >= 150, "expected the full catalogue, got {}", reg.len());
    for e in reg.entries() {
        assert!(valid_name(e.name), "{}", e.name);
        assert!(!e.meta.summary.is_empty(), "{} has no summary", e.name);
        assert!((1..=12).contains(&e.meta.phase), "{} phase {}", e.name, e.meta.phase);
        if e.meta.kind == OpKind::Query {
            assert_eq!(e.meta.audit, Audit::Never, "{} is a query but audited", e.name);
        }
    }
    // Every op named in BUS.md §15 as phase-1 executable is registered.
    for n in ["bus.ping", "bus.schema", "bus.ops", "bus.subscribe", "bus.unsubscribe", "app.version", "app.status",
              "app.reconcile", "audit.list", "audit.get", "settings.get", "settings.set", "settings.reset",
              "workspace.create", "workspace.list", "workspace.update", "workspace.remove",
              "project.add", "project.list", "project.get", "project.update", "project.remove"] {
        assert!(reg.get(n).is_some(), "missing {n}");
    }
}

#[test]
fn schema_renders_and_is_stable() {
    let a = relay_bus::schema::render_pretty();
    let b = relay_bus::schema::render_pretty();
    assert_eq!(a, b, "schema rendering must be deterministic");
    let v: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(v["version"], 1);
    assert_eq!(v["op_count"].as_u64().unwrap() as usize, Registry::global().len());
    assert!(v["ops"]["task.create"]["payload"].is_object());
    assert!(relay_bus::schema::render_op("task.create").is_some());
    assert!(relay_bus::schema::render_op("nope.nope").is_none());
}

#[test]
fn envelope_round_trips() {
    let r = Request::new(Actor::agent("brisk-otter"), "task.get", serde_json::json!({"task_id": 1}));
    let s = serde_json::to_string(&r).unwrap();
    let back: Request = serde_json::from_str(&s).unwrap();
    assert_eq!(back.op, "task.get");
    assert_eq!(back.actor, Actor::Agent("brisk-otter".into()));
    // system is parseable (internal) — doors reject it, the type does not.
    assert_eq!(Actor::parse("system").unwrap(), Actor::System);
    assert!(Actor::parse("agent:").is_err());
    assert!(Actor::parse("root").is_err());
    let e = Response::err(r.id, BusError::not_implemented("task.get", 7));
    let s = serde_json::to_string(&e).unwrap();
    assert!(s.contains("\"ok\":false"));
    let u = Response::unparsed(BusError::parse("eof"));
    assert!(serde_json::to_string(&u).unwrap().contains("\"id\":null"));
}

/// BUS.md §0.5: the committed schema file must equal the generated one.
/// Regenerate with `cargo run -p relay-bus --example dump_schema > schema/bus.v1.json`.
#[test]
fn committed_schema_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schema/bus.v1.json");
    let committed = std::fs::read_to_string(path).expect("schema/bus.v1.json exists");
    let generated = relay_bus::schema::render_pretty();
    assert!(committed == generated,
        "schema/bus.v1.json is stale — regenerate: cargo run -p relay-bus --example dump_schema > schema/bus.v1.json");
}

/// Every `$ref` in `subtree` that points at `#/$defs/<name>` names a definition in `defs`.
fn refs_resolve(subtree: &serde_json::Value, defs: &serde_json::Value, at: &str) {
    match subtree {
        serde_json::Value::Object(map) => {
            if let Some(target) = map.get("$ref").and_then(|r| r.as_str()) {
                let name = target.strip_prefix("#/$defs/").unwrap_or_else(|| panic!("{at}: unexpected $ref {target}"));
                assert!(defs.get(name).is_some(), "{at}: $ref {target} has no definition");
            }
            for value in map.values() {
                refs_resolve(value, defs, at);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| refs_resolve(item, defs, at)),
        _ => {}
    }
}

/// `bus.schema {op}` and MCP's `inputSchema` lift `payload` and `result` out on their own, so
/// each must resolve every reference it makes (recursive `Entry` once left one dangling).
#[test]
fn every_rendered_schema_resolves_its_refs() {
    for e in Registry::global().entries() {
        let doc = relay_bus::schema::render_op(e.name).unwrap();
        for half in ["payload", "result"] {
            refs_resolve(&doc[half], &doc[half]["$defs"], &format!("{} {half}", e.name));
        }
    }
    let whole = relay_bus::schema::render();
    refs_resolve(&whole, &whole["$defs"], "bus.v1.json");
}

/// The Actor schema describes every actor the engine writes into results, and the parser
/// accepts exactly the names its pattern does.
#[test]
fn actor_schema_and_parser_agree() {
    for actor in [Actor::User, Actor::Test, Actor::System, Actor::agent("brisk-otter_2")] {
        assert_eq!(Actor::parse(&actor.to_string()).unwrap(), actor);
    }
    let whole = relay_bus::schema::render();
    let pattern = whole["$defs"]["Actor"]["pattern"].as_str().unwrap();
    assert!(pattern.contains("system"), "events, audit rows and holds carry system: {pattern}");
    for bad in ["agent:two words", "agent:a/b", "agent:a:b", "agent:é", &format!("agent:{}", "a".repeat(65))] {
        assert!(Actor::parse(bad).is_err(), "{bad} is outside the published pattern");
    }
}

/// Server-to-client envelopes ignore unknown fields (BUS.md §12); requests do not.
#[test]
fn only_requests_reject_unknown_envelope_fields() {
    let response = r#"{"v":1,"id":null,"ok":false,"error":{"kind":"internal","code":"internal","message":"x","later":1},"later":true}"#;
    assert!(serde_json::from_str::<Response>(response).is_ok());
    let event = r#"{"v":1,"ev":"task.changed","ts":"2026-01-01T00:00:00Z","actor":"system","payload":{},"later":1}"#;
    assert!(serde_json::from_str::<relay_bus::Event>(event).is_ok());
    let request = r#"{"v":1,"id":"00000000-0000-0000-0000-000000000000","actor":"user","op":"bus.ping","payload":{},"later":1}"#;
    assert!(serde_json::from_str::<Request>(request).is_err());
}

/// `bus.wait {matching}` is an object or absent; anything else used to mean "no filter".
#[test]
fn bus_wait_matching_must_be_an_object() {
    let validate = Registry::global().get("bus.wait").unwrap().validate;
    assert!(validate(&serde_json::json!({"matching": {"request_id": 7}})).is_ok());
    assert!(validate(&serde_json::json!({"matching": null})).is_ok());
    assert!(validate(&serde_json::json!({})).is_ok());
    assert!(validate(&serde_json::json!({"matching": "request_id=7"})).is_err());
    assert!(validate(&serde_json::json!({"matching": [{"request_id": 7}]})).is_err());
}

/// pictures v2 §4.1: only the Pictures plugin's own ops carry it. The key is machine-wide and the
/// person's: its ops carry no plugin, admit no agent, and setting it writes no audit row. The
/// Video plugin brings every `video.*` op but `video.kit.status`, which says whether the machine
/// could render at all (D167).
#[test]
fn only_the_picture_and_video_ops_come_with_a_plugin() {
    use relay_bus::registry::{Actors, Scope};
    let reg = Registry::global();
    for e in reg.entries() {
        let picture = ["design.picture.search", "design.picture.thumb", "design.picture.add"].contains(&e.name);
        let video = e.name.starts_with("video.") && e.name != "video.kit.status";
        let expected = if picture { Some("pictures") } else if video { Some("video") } else { None };
        assert_eq!(e.meta.plugin, expected, "{}", e.name);
    }
    for op in ["design.picture.key_set", "design.picture.key_remove"] {
        let m = reg.get(op).unwrap_or_else(|| panic!("{op} is not registered")).meta;
        assert_eq!((m.kind, m.scope, m.actors), (OpKind::Mutation, Scope::Global, Actors::UserOnly), "{op}");
    }
    assert_eq!(reg.get("design.picture.key_set").unwrap().meta.audit, Audit::Never, "the payload is the key itself");
    assert_eq!(reg.get("design.picture.key_remove").unwrap().meta.audit, Audit::Always);
    // `{}` forgets nothing: the key is required.
    assert!((reg.get("design.picture.key_set").unwrap().validate)(&serde_json::json!({})).is_err());
    // The plugin is not in what bus.ops answers: the schema's OpInfo has no such field.
    let info = &relay_bus::schema::render()["$defs"]["OpInfo"]["properties"];
    assert!(info.get("plugin").is_none());
}

/// MODEL-SPEC §3.3: the new design queries' attributes. A picture's bytes are the person's
/// clients' to read; agents have `assets_dir`.
#[test]
fn the_design_queries_have_their_attributes() {
    use relay_bus::registry::{Actors, Doors, Scope};
    let reg = Registry::global();
    for (op, scope, actors) in [("design.find", Scope::Project, Actors::All), ("design.fonts", Scope::Global, Actors::All),
        ("design.asset.get", Scope::Project, Actors::UserOnly)] {
        let m = reg.get(op).unwrap_or_else(|| panic!("{op} is not registered")).meta;
        assert_eq!((m.kind, m.audit, m.scope, m.actors, m.doors), (OpKind::Query, Audit::Never, scope, actors, Doors::All), "{op}");
    }
    let find = reg.get("design.find").unwrap().validate;
    assert!(find(&serde_json::json!({"design_id": 1, "kind": ["text", "sticky"], "any_color": "#4C9DFF", "with_nodes": true})).is_ok());
    assert!(find(&serde_json::json!({"design_id": 1, "kind": ["button"]})).is_err(), "kinds are the node kinds");
}

/// render v2 §4 (MODEL-SPEC §3.3): design.render is any actor's query of a project, answered by
/// the socket door; serve and answer are the desktop app's (the person's).
#[test]
fn render_ops_have_their_attributes() {
    use relay_bus::registry::{Actors, Doors, Scope};
    let reg = Registry::global();
    let meta = |op: &str| reg.get(op).unwrap_or_else(|| panic!("{op} is not registered")).meta;
    let render = meta("design.render");
    assert_eq!((render.kind, render.scope, render.actors, render.doors), (OpKind::Query, Scope::Project, Actors::All, Doors::SocketOnly));
    let serve = meta("design.render.serve");
    assert_eq!((serve.kind, serve.scope, serve.actors, serve.doors), (OpKind::Query, Scope::Global, Actors::UserOnly, Doors::SocketOnly));
    let answer = meta("design.render.answer");
    assert_eq!((answer.kind, answer.scope, answer.actors, answer.doors), (OpKind::Query, Scope::Global, Actors::UserOnly, Doors::All));
    // The fonts the app reports ride on serve (§1 #18); nothing else does.
    let validate = reg.get("design.render.serve").unwrap().validate;
    assert!(validate(&serde_json::json!({})).is_ok());
    assert!(validate(&serde_json::json!({"fonts": ["Geist", "Inter"]})).is_ok());
    assert!(validate(&serde_json::json!({"renderer": "x"})).is_err());
    assert!(meta("design.apply").summary.ends_with("see the result with design.render"));
}

/// A render asks only what it names: `rev` is no field, `base_rev` is.
#[test]
fn render_in_refuses_unknown_fields() {
    let validate = Registry::global().get("design.render").unwrap().validate;
    assert!(validate(&serde_json::json!({"design_id": 1, "rev": 3})).is_err());
    assert!(validate(&serde_json::json!({"design_id": 1, "base_rev": 3, "page": "Page 1", "at_ms": 500})).is_ok());
    assert!(validate(&serde_json::json!({"design_id": 1, "frames": 6, "png": false})).is_ok());
    assert!(validate(&serde_json::json!({"design_id": 1, "frames": 300})).is_err(), "frames is a u8");
}

/// The request line the engine writes to the app reads back as it was, and an older line (no
/// time, no asker) still reads.
#[test]
fn render_request_round_trips() {
    use relay_bus::ops::design::RenderRequest;
    let request = RenderRequest {
        render_id: "6f0c2b1e-0000-4000-8000-000000000000".into(), design_id: 12, page: "page-1".into(), nodes: Some(vec!["hero".into()]),
        png: true, scale: 1.0, max_side: 1568, boxes_limit: 200, deadline_unix_ms: 1_792_368_000_000, at_ms: None, frames: Some(6),
        motion_checks: Some(relay_bus::ops::design::MotionChecks::End), by: Some("silver-lemur".into()),
    };
    let line = serde_json::to_value(&request).unwrap();
    assert_eq!(serde_json::from_value::<RenderRequest>(line.clone()).unwrap(), request);
    assert!(line.get("at_ms").is_none(), "an absent time is not written");
    assert_eq!(line["motion_checks"], "end");
    let older = serde_json::json!({"render_id": "r", "design_id": 12, "page": "page-1", "nodes": null, "png": true, "scale": 1.0,
        "max_side": 1568, "boxes_limit": 200, "deadline_unix_ms": 1});
    let read: RenderRequest = serde_json::from_value(older).unwrap();
    assert_eq!((read.at_ms, read.frames, read.motion_checks, read.by), (None, None, None, None));
}
