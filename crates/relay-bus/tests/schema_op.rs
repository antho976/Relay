//! `bus.schema {op}` (`schema::render_op`): what an agent is sent to read when a payload does not
//! read, and what MCP lifts out as a tool's `inputSchema`. Each half stands alone, and a type it
//! names more than once is written once (TM-3).

use relay_bus::Registry;
use serde_json::Value;

/// How many times a type with the fields `fields` is written out in `schema`, in place or in a
/// `$defs`: an object schema with exactly those properties.
fn written(schema: &Value, fields: &[&str]) -> usize {
    match schema {
        Value::Object(map) => {
            let names: Option<Vec<&str>> = map.get("properties").and_then(Value::as_object).map(|p| p.keys().map(String::as_str).collect());
            (names.as_deref() == Some(fields)) as usize + map.values().map(|v| written(v, fields)).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(|v| written(v, fields)).sum(),
        _ => 0,
    }
}

/// How many times `text` is in `schema`'s strings.
fn mentions(schema: &Value, text: &str) -> usize {
    match schema {
        Value::String(s) => s.matches(text).count(),
        Value::Object(map) => map.values().map(|v| mentions(v, text)).sum(),
        Value::Array(items) => items.iter().map(|v| mentions(v, text)).sum(),
        _ => 0,
    }
}

/// TM-3: `bus.schema {"op":"design.apply"}`, where every refusal's hint sends an agent, wrote
/// DesignNode out three times (144 KB), past the 25,000 tokens (about 100,000 characters) of a
/// tool's output Claude Code keeps. Each half now writes it at most once, the payload describes
/// it, and the result writes it bare.
#[test]
fn design_apply_writes_each_type_once_and_fits_a_tool_output() {
    let whole = relay_bus::schema::render();
    let node = &whole["$defs"]["DesignNode"];
    let fields: Vec<&str> = node["properties"].as_object().expect("DesignNode has fields").keys().map(String::as_str).collect();
    assert_eq!(written(&whole, &fields), 1);
    let doc = relay_bus::schema::render_op("design.apply").unwrap();
    for half in ["payload", "result"] {
        assert!(written(&doc[half], &fields) <= 1, "{half} writes DesignNode {} times", written(&doc[half], &fields));
    }
    assert_eq!(written(&doc["payload"], &fields), 1, "an add's node is described");
    // Every field's description is there once.
    for (field, schema) in node["properties"].as_object().unwrap() {
        if let Some(about) = schema["description"].as_str() {
            assert_eq!(mentions(&doc, about), mentions(&doc["payload"], about), "{field}'s description is the payload's");
            assert!(mentions(&doc["payload"], about) >= 1, "{field} is described");
        }
    }
    // Under the ~100,000 characters a tool's output keeps, with room to spare: 80 KiB until charts
    // (`DesignChart`) took it to 83.6 KiB.
    let bytes = serde_json::to_string(&doc).unwrap().len();
    assert!(bytes < 90 * 1024, "bus.schema design.apply is {bytes} bytes");
}

/// The result half writes a type the payload describes bare, but whole: it still reads alone, and
/// says which fields an answer carries.
#[test]
fn a_result_writes_what_the_payload_describes_without_its_words() {
    let doc = relay_bus::schema::render_op("design.apply").unwrap();
    let result = &doc["result"];
    let node = &result["$defs"]["DesignNode"];
    assert_eq!(node["description"], "DesignNode, as the payload describes it");
    assert!(node["properties"]["fill"].is_object() && node["properties"]["fill"].get("description").is_none());
    // Its own fields keep theirs.
    assert!(result["properties"]["rev"]["description"].as_str().is_some_and(|d| d.contains("base_rev")));
}

/// MCP takes a payload as a tool's `inputSchema`, which must be an object schema at its root, and
/// the CLI reads `required` there (`requires_project`): a half's root is never a bare `$ref`.
#[test]
fn every_half_is_an_object_schema_at_its_root() {
    for e in Registry::global().entries() {
        let doc = relay_bus::schema::render_op(e.name).unwrap();
        for half in ["payload", "result"] {
            let schema = &doc[half];
            assert!(schema.get("$ref").is_none(), "{} {half} is a bare reference", e.name);
        }
        assert!(doc["payload"].get("properties").is_some() || doc["payload"].get("type").is_some(), "{} payload", e.name);
    }
}
